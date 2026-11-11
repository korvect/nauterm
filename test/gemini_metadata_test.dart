import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ai/ai_client.dart';
import 'package:nauterm/ai/ai_config.dart';
import 'package:nauterm/ai/ai_conversation.dart';
import 'package:nauterm/data/nauterm_data_store.dart';

void main() {
  test(
    'streamed Google Parts retain signatures, boundaries and whitespace',
    () async {
      final parts = <Map<String, Object?>>[
        {
          'text': 'Thinking summary',
          'thought': true,
          'thoughtSignature': 'dGhpbms=',
        },
        {'text': 'Hello'},
        {'text': ' '},
        {
          'text': 'world',
          'thoughtSignature': 'dGV4dA==',
          'partMetadata': {'source': 'test'},
        },
        {'text': '', 'thoughtSignature': 'dGFpbA=='},
      ];
      final fixture = await _GeminiFixture.start([
        [
          for (final part in parts) _chunk([part]),
        ],
        [
          _chunk([
            {'text': 'Continued.'},
          ]),
        ],
      ]);
      final events = await AiProtocolClient()
          .streamCompletion(
            config: fixture.config,
            messages: const [
              AiChatMessage(role: AiChatRole.user, content: 'Hello'),
            ],
          )
          .toList();
      expect(
        events.whereType<AiTextDelta>().map((e) => e.text).join(),
        'Hello world',
      );
      final metadataEvents = events.whereType<AiProviderMetadata>().toList();
      expect((metadataEvents.first.metadata['google'] as Map)['parts'], [
        parts.first,
      ]);
      expect(metadataEvents.last.metadata, {
        'google': {'parts': parts},
      });

      await AiProtocolClient()
          .streamCompletion(
            config: fixture.config.copyWith(model: 'another-gemini-model'),
            messages: [
              const AiChatMessage(role: AiChatRole.user, content: 'Hello'),
              AiChatMessage(
                role: AiChatRole.assistant,
                content: 'Hello world',
                providerMetadata: metadataEvents.last.metadata,
              ),
              const AiChatMessage(role: AiChatRole.user, content: 'Continue'),
            ],
          )
          .drain<void>();
      expect((fixture.requests.last['contents'] as List)[1], {
        'role': 'model',
        'parts': parts,
      });
    },
  );

  test('signature-only replies survive conversation snapshots', () async {
    final fixture = await _GeminiFixture.start([
      [
        _chunk([
          {'thoughtSignature': 'c2lnbmF0dXJl'},
        ]),
      ],
    ]);
    final controller = AiConversationController();
    addTearDown(controller.dispose);
    await controller.send('Hello', config: fixture.config);
    expect(controller.error, isNull);
    final restored = AiConversationController(
      initialConversation: controller.toEntry(scope: 'terminal'),
    );
    addTearDown(restored.dispose);
    expect(restored.messages.last.role, AiChatRole.assistant);
    expect(restored.messages.last.content, isEmpty);
    expect(restored.messages.last.providerMetadata, {
      'google': {
        'parts': [
          {'thoughtSignature': 'c2lnbmF0dXJl'},
        ],
      },
    });
  });

  test('legacy messages and metadata from other protocols use ordinary Google content', () async {
    final fixture = await _GeminiFixture.start([
      [
        _chunk([
          {'text': 'OK'},
        ]),
      ],
    ]);
    final legacy = AiMessageEntry.fromJson({
      'role': 'assistant',
      'content': 'Legacy answer',
      'sequence': 0,
      'tool_calls': [],
      'attachments': [],
    });
    expect(legacy.providerMetadata, isEmpty);
    await AiProtocolClient()
        .streamCompletion(
          config: fixture.config,
          messages: [
            AiChatMessage(
              role: AiChatRole.assistant,
              content: legacy.content,
              providerMetadata: const {
                'anthropic': {'opaque': 'retained elsewhere'},
              },
            ),
            const AiChatMessage(role: AiChatRole.user, content: 'Continue'),
          ],
        )
        .drain<void>();
    expect((fixture.requests.single['contents'] as List).first, {
      'role': 'model',
      'parts': [
        {'text': 'Legacy answer'},
      ],
    });
  });

  for (final withIds in [false, true]) {
    test(
      'Google parallel and sequential tool rounds resume after database reload, ids=$withIds',
      () async {
        Map<String, Object?> call(
          String id,
          String command, {
          bool signed = true,
        }) => {
          'functionCall': {
            if (withIds) 'id': id,
            'name': 'run_terminal_command',
            'args': {
              'command': command,
              'explanation': 'Inspect the terminal.',
            },
          },
          if (signed) 'thoughtSignature': base64Encode(utf8.encode(id)),
        };
        final firstParts = <Map<String, Object?>>[
          {'text': 'Inspecting.'},
          call('first', 'pwd'),
          call('second', 'whoami', signed: false),
        ];
        final nextParts = [call('third', 'ls')];
        final finalParts = <Map<String, Object?>>[
          {'text': 'Finished.', 'thoughtSignature': 'ZG9uZQ=='},
        ];
        final fixture = await _GeminiFixture.start([
          [_chunk(firstParts)],
          [_chunk(nextParts)],
          [_chunk(finalParts)],
        ]);
        final directory = Directory.systemTemp.createTempSync(
          'nauterm-gemini-history-',
        );
        addTearDown(() => directory.deleteSync(recursive: true));
        final path = '${directory.path}/history.sqlite';
        var store = NautermDataStore.openPath(path);
        addTearDown(() => store.dispose());
        final original = AiConversationController();
        addTearDown(original.dispose);
        await original.send(
          'Inspect',
          config: fixture.config,
          enableTerminalTool: true,
        );
        expect(original.error, isNull);
        final saved = original.saveTo(store, scope: 'terminal');
        expect(original.messages.last.providerMetadata, {
          'google': {'parts': firstParts},
        });
        store.dispose();
        store = NautermDataStore.openPath(path);
        final restored = AiConversationController(
          initialConversation: store.getAiConversation(saved.uuid!)!,
        );
        addTearDown(restored.dispose);
        final firstIds = restored.terminalCommands
            .map((command) => command.id)
            .toList();
        expect(firstIds.toSet(), hasLength(2));
        await restored.skipTerminalCommand(firstIds[0], config: fixture.config);
        expect(fixture.requests, hasLength(1));
        await restored.skipTerminalCommand(firstIds[1], config: fixture.config);
        expect(restored.error, isNull);
        final contents = fixture.requests[1]['contents'] as List;
        expect(contents[1], {'role': 'model', 'parts': firstParts});
        final responses = (contents[2] as Map)['parts'] as List;
        expect(responses, hasLength(2));
        for (var index = 0; index < responses.length; index++) {
          final response = responses[index]['functionResponse'] as Map;
          expect(response['name'], 'run_terminal_command');
          if (withIds) {
            expect(response['id'], index == 0 ? 'first' : 'second');
          } else {
            expect(response.containsKey('id'), isFalse);
          }
        }
        final thirdId = restored.terminalCommands.last.id;
        expect(firstIds, isNot(contains(thirdId)));
        await restored.skipTerminalCommand(thirdId, config: fixture.config);
        expect(restored.error, isNull);
        final nextContents = fixture.requests[2]['contents'] as List;
        expect(nextContents[1]['parts'], firstParts);
        expect(nextContents[3]['parts'], nextParts);
        expect(restored.messages.last.content, 'Finished.');
        restored.saveTo(store, scope: 'terminal');
        final persisted = store.getAiConversation(saved.uuid!)!;
        expect(
          persisted.messages
              .where((m) => m.role == 'assistant')
              .map((m) => m.providerMetadata),
          [
            {
              'google': {'parts': firstParts},
            },
            {
              'google': {'parts': nextParts},
            },
            {
              'google': {'parts': finalParts},
            },
          ],
        );
      },
    );
  }
}

Map<String, Object?> _chunk(List<Map<String, Object?>> parts) => {
  'candidates': [
    {
      'index': 0,
      'content': {'role': 'model', 'parts': parts},
    },
  ],
};

class _GeminiFixture {
  _GeminiFixture(this.server);

  final HttpServer server;
  final requests = <Map<String, dynamic>>[];

  AiAssistantConfig get config => AiAssistantConfig(
    protocol: AiApiProtocol.google,
    baseUrl: 'http://${server.address.host}:${server.port}',
    model: 'gemini-test',
    apiKey: 'test-key',
  );

  static Future<_GeminiFixture> start(
    List<List<Map<String, Object?>>> responses,
  ) async {
    final fixture = _GeminiFixture(
      await HttpServer.bind(InternetAddress.loopbackIPv4, 0),
    );
    addTearDown(() => fixture.server.close(force: true));
    fixture.server.listen((request) async {
      final index = fixture.requests.length;
      fixture.requests.add(
        jsonDecode(await utf8.decoder.bind(request).join())
            as Map<String, dynamic>,
      );
      if (index >= responses.length) {
        request.response.statusCode = 500;
      } else {
        request.response.headers.contentType = ContentType(
          'text',
          'event-stream',
        );
        for (final chunk in responses[index]) {
          request.response.write('data: ${jsonEncode(chunk)}\n\n');
        }
      }
      await request.response.close();
    });
    return fixture;
  }
}
