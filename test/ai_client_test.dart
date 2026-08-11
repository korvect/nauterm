import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ai/ai_attachment.dart';
import 'package:nauterm/ai/ai_client.dart';
import 'package:nauterm/ai/ai_config.dart';

void main() {
  test('OpenAI protocol preserves an explicit provider API root', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final requestHandled = server.first.then((request) async {
      expect(request.uri.path, '/v1beta/openai/chat/completions');
      request.response.headers.contentType = ContentType(
        'text',
        'event-stream',
      );
      request.response.write('data: [DONE]\n\n');
      await request.response.close();
    });

    await AiProtocolClient()
        .streamCompletion(
          config: AiAssistantConfig(
            baseUrl:
                'http://${server.address.host}:${server.port}/v1beta/openai/',
            model: 'provider-model',
            apiKey: 'key',
          ),
          messages: const [
            AiChatMessage(role: AiChatRole.user, content: 'hello'),
          ],
          enableTerminalTool: false,
        )
        .drain<void>();

    await requestHandled;
  });

  test('OpenAI protocol sends chat completions request and parses SSE', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final requestHandled = server.first.then((request) async {
      expect(request.method, 'POST');
      expect(request.uri.path, '/v1/chat/completions');
      expect(
        request.headers.value(HttpHeaders.authorizationHeader),
        'Bearer key',
      );
      final body = jsonDecode(await utf8.decoder.bind(request).join()) as Map;
      expect(body['model'], 'openai-model');
      expect(body['stream'], isTrue);
      expect(body.containsKey('max_tokens'), isFalse);
      expect((body['messages'] as List).last, {
        'role': 'user',
        'content': 'hello\n\n<terminal_context>safe output</terminal_context>',
      });
      final terminalTool =
          ((body['tools'] as List).single as Map)['function'] as Map;
      expect(terminalTool['name'], 'run_terminal_command');
      expect(terminalTool['parameters']['required'], [
        'command',
        'explanation',
      ]);
      request.response.headers.contentType = ContentType(
        'text',
        'event-stream',
      );
      request.response.write(
        'data: {"choices":[{"delta":{"content":"hel"}}]}\n\n',
      );
      request.response.write(
        'data: {"choices":[{"delta":{"content":"lo"}}]}\n\n',
      );
      request.response.write(
        'data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"name":"run_terminal_command","arguments":"{\\"command\\":\\"pw"}}]}}]}\n\n',
      );
      request.response.write(
        'data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"d\\"}"}}]}}]}\n\n',
      );
      request.response.write('data: [DONE]\n\n');
      await request.response.close();
    });

    final chunks = await AiProtocolClient()
        .streamCompletion(
          config: AiAssistantConfig(
            baseUrl: 'http://${server.address.host}:${server.port}/v1/',
            model: 'openai-model',
            apiKey: 'key',
          ),
          messages: const [
            AiChatMessage(
              role: AiChatRole.user,
              content: 'hello',
              context: '<terminal_context>safe output</terminal_context>',
            ),
          ],
          enableTerminalTool: true,
        )
        .toList();

    await requestHandled;
    expect(
      chunks.whereType<AiTextDelta>().map((event) => event.text).join(),
      'hello',
    );
    expect(chunks.whereType<AiToolCall>().single.terminalCommand, 'pwd');
  });

  test('Anthropic protocol sends Messages request and parses SSE', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final requestHandled = server.first.then((request) async {
      expect(request.method, 'POST');
      expect(request.uri.path, '/anthropic/v1/messages');
      expect(request.headers.value('x-api-key'), 'anthropic-key');
      expect(request.headers.value('anthropic-version'), '2023-06-01');
      final body = jsonDecode(await utf8.decoder.bind(request).join()) as Map;
      expect(body['model'], 'anthropic-model');
      expect(body['max_tokens'], 2048);
      expect(body['stream'], isTrue);
      expect(body['system'], 'system prompt');
      expect(body['messages'], [
        {'role': 'user', 'content': 'hello'},
      ]);
      expect((body['tools'] as List).single['name'], 'run_terminal_command');
      request.response.headers.contentType = ContentType(
        'text',
        'event-stream',
      );
      request.response.write('event: content_block_delta\n');
      request.response.write(
        'data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}\n\n',
      );
      request.response.write('event: content_block_start\n');
      request.response.write(
        'data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tool-1","name":"run_terminal_command","input":{}}}\n\n',
      );
      request.response.write('event: content_block_delta\n');
      request.response.write(
        'data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\\"command\\":\\"ls\\"}"}}\n\n',
      );
      request.response.write('event: message_stop\n');
      request.response.write('data: {"type":"message_stop"}\n\n');
      await request.response.close();
    });

    final chunks = await AiProtocolClient()
        .streamCompletion(
          config: AiAssistantConfig(
            protocol: AiApiProtocol.anthropic,
            baseUrl: 'http://${server.address.host}:${server.port}/anthropic',
            model: 'anthropic-model',
            apiKey: 'anthropic-key',
            maxTokens: 2048,
          ),
          messages: const [
            AiChatMessage(role: AiChatRole.system, content: 'system prompt'),
            AiChatMessage(role: AiChatRole.user, content: 'hello'),
          ],
          enableTerminalTool: true,
        )
        .toList();

    await requestHandled;
    expect(chunks.whereType<AiTextDelta>().map((event) => event.text), ['hi']);
    expect(chunks.whereType<AiToolCall>().single.terminalCommand, 'ls');
  });

  test('Google protocol streams Gemini content and function calls', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final handled = server.first.then((request) async {
      expect(
        request.uri.path,
        '/v1beta/models/gemini-test:streamGenerateContent',
      );
      expect(request.uri.queryParameters['key'], 'google-key');
      final body = jsonDecode(await utf8.decoder.bind(request).join()) as Map;
      expect(body['generationConfig'], isNull);
      expect(
        (body['tools'] as List).single['functionDeclarations'],
        hasLength(1),
      );
      request.response.headers.contentType = ContentType(
        'text',
        'event-stream',
      );
      request.response.write(
        'data: {"candidates":[{"content":{"role":"model","parts":[{"text":"hi"},{"functionCall":{"name":"run_terminal_command","args":{"command":"pwd"}}}]}}]}\n\n',
      );
      await request.response.close();
    });

    final chunks = await AiProtocolClient()
        .streamCompletion(
          config: AiAssistantConfig(
            protocol: AiApiProtocol.google,
            baseUrl: 'http://${server.address.host}:${server.port}',
            model: 'gemini-test',
            apiKey: 'google-key',
          ),
          messages: const [
            AiChatMessage(role: AiChatRole.user, content: 'hello'),
          ],
          enableTerminalTool: true,
        )
        .toList();
    await handled;
    expect(chunks.whereType<AiTextDelta>().single.text, 'hi');
    expect(chunks.whereType<AiToolCall>().single.terminalCommand, 'pwd');
  });

  test('Ollama protocol supports local auth-free streaming', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final handled = server.first.then((request) async {
      expect(request.uri.path, '/api/chat');
      expect(request.headers.value(HttpHeaders.authorizationHeader), isNull);
      final body = jsonDecode(await utf8.decoder.bind(request).join()) as Map;
      expect(body['options'], isNull);
      expect((body['tools'] as List), hasLength(1));
      request.response.headers.contentType = ContentType.json;
      request.response.write(
        '{"message":{"role":"assistant","content":"hi"},"done":false}\n',
      );
      request.response.write(
        '{"message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":"run_terminal_command","arguments":{"command":"pwd"}}}]},"done":true}\n',
      );
      await request.response.close();
    });

    const config = AiAssistantConfig(
      protocol: AiApiProtocol.ollama,
      baseUrl: 'http://localhost:11434',
      model: 'qwen3',
    );
    expect(config.validationError, isNull);
    final chunks = await AiProtocolClient()
        .streamCompletion(
          config: config.copyWith(
            baseUrl: 'http://${server.address.host}:${server.port}',
          ),
          messages: const [
            AiChatMessage(role: AiChatRole.user, content: 'hello'),
          ],
          enableTerminalTool: true,
        )
        .toList();
    await handled;
    expect(chunks.whereType<AiTextDelta>().single.text, 'hi');
    expect(chunks.whereType<AiToolCall>().single.terminalCommand, 'pwd');
  });

  test('OpenAI protocol encodes tool calls and tool results', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final requestHandled = server.first.then((request) async {
      final body = jsonDecode(await utf8.decoder.bind(request).join()) as Map;
      expect(body.containsKey('max_tokens'), isFalse);
      expect(body['messages'], [
        {
          'role': 'assistant',
          'tool_calls': [
            {
              'id': 'call-1',
              'type': 'function',
              'function': {
                'name': 'run_terminal_command',
                'arguments': '{"command":"pwd"}',
              },
            },
          ],
        },
        {
          'role': 'tool',
          'tool_call_id': 'call-1',
          'content': '{"exit_code":0,"output":"/tmp"}',
        },
      ]);
      request.response.headers.contentType = ContentType(
        'text',
        'event-stream',
      );
      request.response.write('data: [DONE]\n\n');
      await request.response.close();
    });

    await AiProtocolClient()
        .streamCompletion(
          config: AiAssistantConfig(
            baseUrl: 'http://${server.address.host}:${server.port}/v1',
            model: 'model',
            apiKey: 'key',
          ),
          messages: const [
            AiChatMessage(
              role: AiChatRole.assistant,
              content: '',
              toolCalls: [
                AiToolCall(
                  id: 'call-1',
                  name: 'run_terminal_command',
                  arguments: {'command': 'pwd'},
                ),
              ],
            ),
            AiChatMessage(
              role: AiChatRole.tool,
              content: '',
              toolResult: AiToolResult(
                toolCallId: 'call-1',
                name: 'run_terminal_command',
                content: '{"exit_code":0,"output":"/tmp"}',
              ),
            ),
          ],
        )
        .drain<void>();
    await requestHandled;
  });

  test('Anthropic protocol encodes tool use and tool result blocks', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final requestHandled = server.first.then((request) async {
      final body = jsonDecode(await utf8.decoder.bind(request).join()) as Map;
      expect(body['max_tokens'], 4096);
      expect(body['messages'], [
        {
          'role': 'assistant',
          'content': [
            {
              'type': 'tool_use',
              'id': 'tool-1',
              'name': 'run_terminal_command',
              'input': {'command': 'false'},
            },
            {
              'type': 'tool_use',
              'id': 'tool-2',
              'name': 'run_terminal_command',
              'input': {'command': 'pwd'},
            },
          ],
        },
        {
          'role': 'user',
          'content': [
            {
              'type': 'tool_result',
              'tool_use_id': 'tool-1',
              'content': [
                {'type': 'text', 'text': '{"exit_code":1}'},
              ],
              'is_error': true,
            },
            {
              'type': 'tool_result',
              'tool_use_id': 'tool-2',
              'content': [
                {'type': 'text', 'text': '{"exit_code":0}'},
              ],
            },
          ],
        },
      ]);
      request.response.headers.contentType = ContentType(
        'text',
        'event-stream',
      );
      request.response.write('data: {"type":"message_stop"}\n\n');
      await request.response.close();
    });

    await AiProtocolClient()
        .streamCompletion(
          config: AiAssistantConfig(
            protocol: AiApiProtocol.anthropic,
            baseUrl: 'http://${server.address.host}:${server.port}/anthropic',
            model: 'model',
            apiKey: 'key',
          ),
          messages: const [
            AiChatMessage(
              role: AiChatRole.assistant,
              content: '',
              toolCalls: [
                AiToolCall(
                  id: 'tool-1',
                  name: 'run_terminal_command',
                  arguments: {'command': 'false'},
                ),
                AiToolCall(
                  id: 'tool-2',
                  name: 'run_terminal_command',
                  arguments: {'command': 'pwd'},
                ),
              ],
            ),
            AiChatMessage(
              role: AiChatRole.tool,
              content: '',
              toolResult: AiToolResult(
                toolCallId: 'tool-1',
                name: 'run_terminal_command',
                content: '{"exit_code":1}',
                isError: true,
              ),
            ),
            AiChatMessage(
              role: AiChatRole.tool,
              content: '',
              toolResult: AiToolResult(
                toolCallId: 'tool-2',
                name: 'run_terminal_command',
                content: '{"exit_code":0}',
              ),
            ),
          ],
        )
        .drain<void>();
    await requestHandled;
  });

  test('OpenAI and Anthropic encode text and image attachments', () async {
    const textAttachment = AiAttachment(
      id: 'text-1',
      name: 'notes.txt',
      mimeType: 'text/plain',
      size: 5,
      kind: AiAttachmentKind.text,
      text: 'hello',
    );
    final imageAttachment = AiAttachment(
      id: 'image-1',
      name: 'screen.png',
      mimeType: 'image/png',
      size: 4,
      kind: AiAttachmentKind.image,
      bytes: Uint8List.fromList([1, 2, 3, 4]),
    );

    Future<Map<Object?, Object?>> capture(AiApiProtocol protocol) async {
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      addTearDown(() => server.close(force: true));
      late Map<Object?, Object?> body;
      final handled = server.first.then((request) async {
        body = jsonDecode(await utf8.decoder.bind(request).join()) as Map;
        request.response.headers.contentType = ContentType(
          'text',
          'event-stream',
        );
        request.response.write(
          protocol == AiApiProtocol.openAi
              ? 'data: [DONE]\n\n'
              : 'data: {"type":"message_stop"}\n\n',
        );
        await request.response.close();
      });
      await AiProtocolClient()
          .streamCompletion(
            config: AiAssistantConfig(
              protocol: protocol,
              baseUrl: 'http://${server.address.host}:${server.port}',
              model: 'model',
              apiKey: 'key',
            ),
            messages: [
              AiChatMessage(
                role: AiChatRole.user,
                content: 'inspect these',
                attachments: [textAttachment, imageAttachment],
              ),
            ],
          )
          .drain<void>();
      await handled;
      return body;
    }

    final openAi = await capture(AiApiProtocol.openAi);
    final openAiContent =
        (openAi['messages'] as List).single['content'] as List;
    expect(openAiContent.first, {'type': 'text', 'text': 'inspect these'});
    expect(openAiContent[1]['type'], 'text');
    expect(openAiContent[1]['text'], contains('notes.txt'));
    expect(openAiContent[2]['type'], 'image_url');
    expect(
      openAiContent[2]['image_url']['url'],
      startsWith('data:image/png;base64,'),
    );

    final anthropic = await capture(AiApiProtocol.anthropic);
    final anthropicContent =
        (anthropic['messages'] as List).single['content'] as List;
    expect(anthropicContent.first, {'type': 'text', 'text': 'inspect these'});
    expect(anthropicContent[1]['type'], 'text');
    expect(anthropicContent[2]['type'], 'image');
    expect(anthropicContent[2]['source']['media_type'], 'image/png');
  });

  test('standardized generation settings map to each protocol', () async {
    Future<Map<String, dynamic>> capture(AiApiProtocol protocol) async {
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      addTearDown(() => server.close(force: true));
      late Map<String, dynamic> body;
      final handled = server.first.then((request) async {
        body = (jsonDecode(await utf8.decoder.bind(request).join()) as Map)
            .cast<String, dynamic>();
        if (protocol == AiApiProtocol.ollama) {
          request.response.headers.contentType = ContentType.json;
          request.response.write(
            '{"message":{"role":"assistant","content":""},"done":true}\n',
          );
        } else {
          request.response.headers.contentType = ContentType(
            'text',
            'event-stream',
          );
          request.response.write(switch (protocol) {
            AiApiProtocol.openAi => 'data: [DONE]\n\n',
            AiApiProtocol.anthropic => 'data: {"type":"message_stop"}\n\n',
            AiApiProtocol.google => 'data: {"candidates":[]}\n\n',
            AiApiProtocol.ollama => throw StateError('unreachable'),
          });
        }
        await request.response.close();
      });
      final baseUrl = 'http://${server.address.host}:${server.port}';
      await AiProtocolClient()
          .streamCompletion(
            config: AiAssistantConfig(
              protocol: protocol,
              baseUrl: protocol == AiApiProtocol.openAi
                  ? '$baseUrl/v1'
                  : baseUrl,
              model: 'model',
              apiKey: protocol == AiApiProtocol.ollama ? '' : 'key',
              maxTokens: 1024,
              temperature: 0.7,
            ),
            messages: const [
              AiChatMessage(role: AiChatRole.user, content: 'hello'),
            ],
            enableTerminalTool: false,
          )
          .drain<void>();
      await handled;
      return body;
    }

    final openAi = await capture(AiApiProtocol.openAi);
    expect(openAi, containsPair('max_tokens', 1024));
    expect(openAi, containsPair('temperature', 0.7));

    final anthropic = await capture(AiApiProtocol.anthropic);
    expect(anthropic, containsPair('max_tokens', 1024));
    expect(anthropic, containsPair('temperature', 0.7));

    final google = await capture(AiApiProtocol.google);
    expect(google['generationConfig'], {
      'maxOutputTokens': 1024,
      'temperature': 0.7,
    });

    final ollama = await capture(AiApiProtocol.ollama);
    expect(ollama['options'], {'num_predict': 1024, 'temperature': 0.7});
  });

  test('protocol errors expose provider message without credentials', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    unawaited(
      server.first.then((request) async {
        await request.drain<void>();
        request.response.statusCode = HttpStatus.unauthorized;
        request.response.headers.contentType = ContentType.json;
        request.response.write(
          jsonEncode({
            'error': {'message': 'Invalid API key'},
          }),
        );
        await request.response.close();
      }),
    );

    final stream = AiProtocolClient().streamCompletion(
      config: AiAssistantConfig(
        baseUrl: 'http://${server.address.host}:${server.port}/v1',
        model: 'model',
        apiKey: 'secret-that-must-not-leak',
      ),
      messages: const [AiChatMessage(role: AiChatRole.user, content: 'hello')],
    );

    await expectLater(
      stream,
      emitsError(
        isA<AiProtocolException>()
            .having((error) => error.statusCode, 'statusCode', 401)
            .having((error) => error.message, 'message', 'Invalid API key')
            .having(
              (error) => error.toString(),
              'credential leakage',
              isNot(contains('secret-that-must-not-leak')),
            ),
      ),
    );
  });

  test('cancelling a completion aborts a request awaiting response', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final requestReceived = Completer<void>();
    final serverSubscription = server.listen((request) {
      unawaited(request.drain<void>());
      if (!requestReceived.isCompleted) {
        requestReceived.complete();
      }
    });
    addTearDown(serverSubscription.cancel);

    final subscription = AiProtocolClient()
        .streamCompletion(
          config: AiAssistantConfig(
            baseUrl: 'http://${server.address.host}:${server.port}/v1',
            model: 'model',
            apiKey: 'key',
          ),
          messages: const [
            AiChatMessage(role: AiChatRole.user, content: 'think'),
          ],
        )
        .listen((_) {});

    await requestReceived.future.timeout(const Duration(seconds: 2));
    final stopwatch = Stopwatch()..start();
    await subscription.cancel().timeout(const Duration(seconds: 1));

    expect(stopwatch.elapsed, lessThan(const Duration(seconds: 1)));
  });
}
