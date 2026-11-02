import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ai/ai_client.dart';
import 'package:nauterm/ai/ai_attachment.dart';
import 'package:nauterm/ai/ai_config.dart';
import 'package:nauterm/ai/ai_conversation.dart';
import 'package:nauterm/ai/ai_context.dart';
import 'package:nauterm/ai/ai_terminal_runner.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';

void main() {
  const config = AiAssistantConfig(model: 'test-model', apiKey: 'test-key');

  test(
    'signed text metadata survives streaming, saving and follow-up',
    () async {
      const metadata = {
        'google_parts': [
          {
            'text': 'private thought',
            'thought': true,
            'thoughtSignature': 'signed-thought',
          },
          {'text': 'Hello', 'thoughtSignature': 'signed-text'},
          {'thoughtSignature': 'signed-tail'},
        ],
      };
      final requests = <List<AiChatMessage>>[];
      final controller = AiConversationController(
        client: _FakeAiProtocolClient((messages) {
          requests.add(messages);
          return Stream.fromIterable(const [
            AiTextDelta('Hello'),
            AiProviderMetadata(metadata),
          ]);
        }),
      );
      addTearDown(controller.dispose);
      await controller.send('Question', config: config);
      final saved = controller.toEntry(scope: 'terminal');
      expect(saved.messages.last.providerMetadata, metadata);
      final decoded = AiMessageEntry.fromJson(saved.messages.last.toJson());
      expect(decoded.providerMetadata, metadata);
      final restored = AiConversationController(
        initialConversation: saved,
        client: _FakeAiProtocolClient((messages) {
          requests.add(messages);
          return Stream.value(const AiTextDelta('Follow-up'));
        }),
      );
      addTearDown(restored.dispose);
      await restored.send('Next', config: config);
      final previous = requests.last.firstWhere(
        (m) => m.role == AiChatRole.assistant,
      );
      expect(previous.providerMetadata, metadata);
      expect(previous.copyWith(content: 'Edited').providerMetadata, isEmpty);
    },
  );

  test(
    'shared tool metadata survives Flutter history rewrites and Skip',
    () async {
      const call = {
        'id': 'rs-internal-call',
        'name': 'run_terminal_command',
        'arguments': {'command': 'pwd', 'explanation': 'Read directory.'},
        'omit_provider_id': true,
        'thought_signature': 'opaque-signature',
        'future_provider_field': {'retained': true},
        'google_parts': [
          {'text': 'Review the command.', 'thoughtSignature': 'signed-text'},
          {
            'functionCall': {
              'name': 'run_terminal_command',
              'args': {'command': 'pwd', 'explanation': 'Read directory.'},
            },
          },
          {'thoughtSignature': 'signed-tail'},
        ],
      };
      final requests = <List<AiChatMessage>>[];
      final controller = AiConversationController(
        initialConversation: const AiConversationEntry(
          title: 'Shared conversation',
          scope: 'terminal',
          messages: [
            AiMessageEntry(
              role: 'assistant',
              content: 'Review.',
              sequence: 0,
              toolCalls: [call],
            ),
          ],
          commandBlocks: [
            AiCommandBlockEntry(
              toolCallId: 'rs-internal-call',
              command: 'pwd',
              explanation: 'Read directory.',
              status: 'pending',
              sequence: 1,
            ),
          ],
        ),
        client: _FakeAiProtocolClient((messages) {
          requests.add(messages);
          return Stream.value(const AiTextDelta('Skipped.'));
        }),
      );
      addTearDown(controller.dispose);
      expect(
        controller.toEntry(scope: 'terminal').messages.first.toolCalls.single,
        call,
      );
      await controller.skipTerminalCommand('rs-internal-call', config: config);
      final messages = controller.toEntry(scope: 'terminal').messages;
      expect(messages.first.toolCalls.single, call);
      expect(
        messages
            .firstWhere((message) => message.role == 'tool')
            .toolResult?['omit_provider_id'],
        true,
      );
      expect(
        requests.single.first.toolCalls.single.metadata['google_parts'],
        call['google_parts'],
      );
      final result = {
        'tool_call_id': 'rs-internal-call',
        'name': 'run_terminal_command',
        'content': 'skipped',
        'is_error': true,
        'omit_provider_id': true,
        'future_result_field': {'retained': true},
      };
      expect(AiToolResult.fromJson(result).toJson(), result);
    },
  );

  test('conversation appends streamed assistant text', () async {
    final controller = AiConversationController(
      client: _FakeAiProtocolClient(
        (_) =>
            Stream.fromIterable(const [AiTextDelta('hel'), AiTextDelta('lo')]),
      ),
    );
    addTearDown(controller.dispose);

    await controller.send('question', config: config);

    expect(controller.sending, isFalse);
    expect(controller.error, isNull);
    expect(controller.messages, hasLength(2));
    expect(controller.messages.first.role, AiChatRole.user);
    expect(controller.messages.first.content, 'question');
    expect(controller.messages.last.role, AiChatRole.assistant);
    expect(controller.messages.last.content, 'hello');
  });

  test(
    'editing a user message truncates later messages and regenerates',
    () async {
      var requestCount = 0;
      final requests = <List<AiChatMessage>>[];
      final controller = AiConversationController(
        client: _FakeAiProtocolClient((messages) {
          requests.add(messages);
          requestCount += 1;
          return Stream.value(AiTextDelta('answer $requestCount'));
        }),
      );
      addTearDown(controller.dispose);

      await controller.send('first question', config: config);
      await controller.send('follow-up', config: config);
      final createdAt = controller.messages.first.createdAt;

      expect(
        await controller.editUserMessage(0, 'edited question', config: config),
        isTrue,
      );

      expect(controller.messages, hasLength(2));
      expect(controller.messages.first.content, 'edited question');
      expect(controller.messages.first.createdAt, createdAt);
      expect(controller.messages.first.updatedAt, isNotNull);
      expect(controller.messages.last.content, 'answer 3');
      expect(controller.title, 'edited question');
      expect(
        requests.last
            .where((message) => message.role == AiChatRole.user)
            .map((message) => message.content),
        ['edited question'],
      );

      final snapshot = controller.toEntry(scope: 'terminal');
      expect(snapshot.messages.first.createdAt, createdAt);
      expect(snapshot.messages.first.updatedAt, isNotNull);
    },
  );

  test(
    'conversation generation can be stopped and keeps partial text',
    () async {
      final stream = StreamController<AiStreamEvent>();
      final controller = AiConversationController(
        client: _FakeAiProtocolClient((_) => stream.stream),
      );
      addTearDown(controller.dispose);
      addTearDown(stream.close);

      final sending = controller.send('question', config: config);
      await Future<void>.delayed(Duration.zero);
      stream.add(const AiTextDelta('partial answer'));
      await Future<void>.delayed(Duration.zero);

      expect(controller.sending, isTrue);
      expect(await controller.stopGenerating(), isTrue);
      await sending;

      expect(controller.sending, isFalse);
      expect(controller.stopping, isFalse);
      expect(controller.error, isNull);
      expect(controller.messages, hasLength(2));
      expect(controller.messages.last.content, 'partial answer');
      expect(await controller.stopGenerating(), isFalse);
    },
  );

  test('stopping generation removes an empty assistant placeholder', () async {
    final stream = StreamController<AiStreamEvent>();
    final controller = AiConversationController(
      client: _FakeAiProtocolClient((_) => stream.stream),
    );
    addTearDown(controller.dispose);
    addTearDown(stream.close);

    final sending = controller.send('question', config: config);
    await Future<void>.delayed(Duration.zero);
    expect(await controller.stopGenerating(), isTrue);
    await sending;

    expect(controller.messages, hasLength(1));
    expect(controller.messages.single.role, AiChatRole.user);
  });

  test('preparing for close keeps partial output in a savable state', () async {
    final stream = StreamController<AiStreamEvent>();
    final controller = AiConversationController(
      client: _FakeAiProtocolClient((_) => stream.stream),
    );
    addTearDown(controller.dispose);
    addTearDown(stream.close);

    final sending = controller.send('question', config: config);
    await Future<void>.delayed(Duration.zero);
    stream.add(const AiTextDelta('partial'));
    await Future<void>.delayed(Duration.zero);

    controller.prepareForClose();
    await sending;

    expect(controller.sending, isFalse);
    expect(controller.messages.last.content, 'partial');
    expect(controller.toEntry(scope: 'terminal').messages, hasLength(2));
  });

  test(
    'conversation reports configuration errors without adding messages',
    () async {
      final controller = AiConversationController(
        client: _FakeAiProtocolClient((_) => const Stream.empty()),
      );
      addTearDown(controller.dispose);

      await controller.send('question', config: const AiAssistantConfig());

      expect(controller.messages, isEmpty);
      expect(controller.error, 'Configure an AI model in Settings.');
    },
  );

  test(
    'conversation removes an empty assistant message after an error',
    () async {
      final controller = AiConversationController(
        client: _FakeAiProtocolClient(
          (_) => Stream.error(const AiProtocolException('provider failed')),
        ),
      );
      addTearDown(controller.dispose);

      await controller.send('question', config: config);

      expect(controller.sending, isFalse);
      expect(controller.error, 'provider failed');
      expect(controller.messages, hasLength(1));
      expect(controller.messages.single.role, AiChatRole.user);
    },
  );

  test(
    'conversation stores terminal tool calls for explicit approval',
    () async {
      final controller = AiConversationController(
        client: _FakeAiProtocolClient(
          (_) => Stream.value(
            const AiToolCall(
              id: 'tool-1',
              name: 'run_terminal_command',
              arguments: {
                'command': 'pwd',
                'explanation': 'Show the current working directory.',
              },
            ),
          ),
        ),
      );
      addTearDown(controller.dispose);

      await controller.send(
        'where am I',
        config: config,
        enableTerminalTool: true,
      );

      expect(controller.terminalCommands, hasLength(1));
      expect(controller.terminalCommands.single.command, 'pwd');
      expect(
        controller.terminalCommands.single.explanation,
        'Show the current working directory.',
      );
      expect(
        controller.terminalCommands.single.status,
        AiTerminalCommandStatus.pending,
      );

      expect(controller.timeline, [
        isA<AiConversationMessageItem>(),
        isA<AiConversationMessageItem>(),
        isA<AiConversationCommandItem>(),
      ]);
      expect(controller.messages.last.toolCalls.single.id, 'tool-1');
      expect(
        controller.messages.last.content,
        'Show the current working directory.',
      );
    },
  );

  test('terminal result is returned to the model as a tool result', () async {
    var requestCount = 0;
    final requests = <List<AiChatMessage>>[];
    final controller = AiConversationController(
      terminalRunner: _ImmediateTerminalRunner(),
      client: _FakeAiProtocolClient((messages) {
        requests.add(messages);
        requestCount += 1;
        if (requestCount == 1) {
          return Stream.value(
            const AiToolCall(
              id: 'tool-1',
              name: 'run_terminal_command',
              arguments: {
                'command': 'pwd',
                'explanation': 'Show the current working directory.',
              },
            ),
          );
        }
        return Stream.value(const AiTextDelta('The command completed.'));
      }),
    );
    final terminal = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 24),
      onInput: (_) {},
    );
    addTearDown(controller.dispose);
    addTearDown(terminal.dispose);

    await controller.send(
      'where am I',
      config: config,
      enableTerminalTool: true,
    );
    await controller.executeTerminalCommand(
      'tool-1',
      controller: terminal,
      config: config,
      enableTerminalTool: true,
    );

    final command = controller.terminalCommands.single;
    expect(command.status, AiTerminalCommandStatus.succeeded);
    expect(command.exitCode, 0);
    expect(command.output, '/tmp/project');
    expect(requests, hasLength(2));
    expect(
      requests.last.any((message) => message.toolCalls.isNotEmpty),
      isTrue,
    );
    final resultMessage = requests.last.singleWhere(
      (message) => message.role == AiChatRole.tool,
    );
    expect(resultMessage.toolResult?.toolCallId, 'tool-1');
    expect(resultMessage.toolResult?.content, contains('/tmp/project'));
    expect(controller.messages.last.content, 'The command completed.');
  });

  test('unintegrated terminal command is reported as submitted', () async {
    var requestCount = 0;
    final requests = <List<AiChatMessage>>[];
    final inputs = <String>[];
    final controller = AiConversationController(
      client: _FakeAiProtocolClient((messages) {
        requests.add(messages);
        requestCount += 1;
        if (requestCount == 1) {
          return Stream.value(
            const AiToolCall(
              id: 'tool-submit',
              name: 'run_terminal_command',
              arguments: {
                'command': 'pwd',
                'explanation': 'Submit the command.',
              },
            ),
          );
        }
        return Stream.value(const AiTextDelta('The command was submitted.'));
      }),
    );
    final terminal = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 24),
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    addTearDown(terminal.dispose);

    await controller.send('run it', config: config, enableTerminalTool: true);
    await controller.executeTerminalCommand(
      'tool-submit',
      controller: terminal,
      config: config,
      enableTerminalTool: true,
    );

    final command = controller.terminalCommands.single;
    expect(command.status, AiTerminalCommandStatus.submitted);
    expect(command.exitCode, isNull);
    expect(command.output, isEmpty);
    expect(inputs, ['pwd\r']);
    final result = requests.last
        .singleWhere((message) => message.role == AiChatRole.tool)
        .toolResult!;
    expect(result.isError, isFalse);
    expect(result.content, contains('"submitted":true'));
    expect(result.content, contains('"result_tracked":false'));
  });

  test('multiple terminal commands submit only after every decision', () async {
    var requestCount = 0;
    final requests = <List<AiChatMessage>>[];
    final controller = AiConversationController(
      terminalRunner: _ImmediateTerminalRunner(),
      client: _FakeAiProtocolClient((messages) {
        requests.add(messages);
        requestCount += 1;
        if (requestCount == 1) {
          return Stream.fromIterable(const [
            AiToolCall(
              id: 'tool-1',
              name: 'run_terminal_command',
              arguments: {
                'command': 'pwd',
                'explanation': 'Show the current directory.',
              },
            ),
            AiToolCall(
              id: 'tool-2',
              name: 'run_terminal_command',
              arguments: {
                'command': 'git status',
                'explanation': 'Inspect repository status.',
              },
            ),
          ]);
        }
        return Stream.value(const AiTextDelta('Batch reviewed.'));
      }),
    );
    final terminal = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 24),
      onInput: (_) {},
    );
    addTearDown(controller.dispose);
    addTearDown(terminal.dispose);

    await controller.send('inspect', config: config, enableTerminalTool: true);
    await controller.executeTerminalCommand(
      'tool-1',
      controller: terminal,
      config: config,
    );

    expect(requestCount, 1);
    expect(
      controller.terminalCommands[0].status,
      AiTerminalCommandStatus.succeeded,
    );
    expect(
      controller.terminalCommands[1].status,
      AiTerminalCommandStatus.pending,
    );
    expect(
      controller.messages.where((message) => message.role == AiChatRole.tool),
      isEmpty,
    );

    await controller.send('do something else', config: config);
    expect(
      controller.messages.where((message) => message.role == AiChatRole.user),
      hasLength(1),
    );

    await controller.skipTerminalCommand('tool-2', config: config);

    expect(requestCount, 2);
    expect(
      controller.terminalCommands[1].status,
      AiTerminalCommandStatus.skipped,
    );
    final results = requests.last
        .where((message) => message.role == AiChatRole.tool)
        .map((message) => message.toolResult!)
        .toList(growable: false);
    expect(results.map((result) => result.toolCallId), ['tool-1', 'tool-2']);
    expect(results.first.isError, isFalse);
    expect(results.last.isError, isTrue);
    expect(results.last.content, contains('"skipped":true'));
    expect(controller.messages.last.content, 'Batch reviewed.');
  });

  test('running terminal commands can be cancelled', () async {
    var requestCount = 0;
    final runner = _CancellableTerminalRunner();
    final controller = AiConversationController(
      terminalRunner: runner,
      client: _FakeAiProtocolClient((_) {
        requestCount += 1;
        return requestCount == 1
            ? Stream.value(
                const AiToolCall(
                  id: 'tool-cancel',
                  name: 'run_terminal_command',
                  arguments: {
                    'command': 'sleep 30',
                    'explanation': 'Wait for completion.',
                  },
                ),
              )
            : const Stream.empty();
      }),
    );
    final terminal = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 24),
      onInput: (_) {},
    );
    addTearDown(controller.dispose);
    addTearDown(terminal.dispose);

    await controller.send('wait', config: config, enableTerminalTool: true);
    final execution = controller.executeTerminalCommand(
      'tool-cancel',
      controller: terminal,
      config: config,
    );
    await Future<void>.delayed(Duration.zero);

    expect(controller.cancelTerminalCommand('tool-cancel'), isTrue);
    expect(controller.terminalCommands.single.cancellationRequested, isTrue);
    await execution;

    expect(
      controller.terminalCommands.single.status,
      AiTerminalCommandStatus.cancelled,
    );
    expect(controller.terminalCommands.single.exitCode, 130);
    expect(controller.hasRunningTerminalCommand, isFalse);
    expect(controller.sending, isFalse);
    expect(requestCount, 2);
    expect(
      controller.messages
          .singleWhere((message) => message.role == AiChatRole.tool)
          .toolResult
          ?.content,
      contains('"cancelled":true'),
    );
  });

  test('conversation keeps terminal context on the originating turn', () async {
    late List<AiChatMessage> requestMessages;
    final controller = AiConversationController(
      client: _FakeAiProtocolClient((messages) {
        requestMessages = messages;
        return const Stream.empty();
      }),
    );
    addTearDown(controller.dispose);

    await controller.send(
      'explain this',
      config: config,
      terminalContext: const AiTerminalContext(
        terminalLabel: 'server',
        attachments: [
          AiContextAttachment(
            kind: AiContextKind.recentOutput,
            label: 'Recent output',
            content: 'command failed',
          ),
        ],
      ),
    );

    final userMessage = requestMessages.singleWhere(
      (message) => message.role == AiChatRole.user,
    );
    expect(userMessage.content, 'explain this');
    expect(userMessage.requestContent, contains('command failed'));
    expect(controller.messages.first.context, contains('command failed'));
  });

  test('conversation context attachments can be disabled and restored', () {
    final controller = AiConversationController(
      client: _FakeAiProtocolClient((_) => const Stream.empty()),
    );
    addTearDown(controller.dispose);

    controller.setContextKindEnabled(AiContextKind.recentOutput, false);
    expect(
      controller.isContextKindEnabled(AiContextKind.recentOutput),
      isFalse,
    );

    controller.restoreTerminalContext();
    expect(controller.isContextKindEnabled(AiContextKind.recentOutput), isTrue);
  });

  test('pending attachments move onto the submitted user message', () async {
    late List<AiChatMessage> requestMessages;
    final controller = AiConversationController(
      client: _FakeAiProtocolClient((messages) {
        requestMessages = messages;
        return const Stream.empty();
      }),
    );
    addTearDown(controller.dispose);
    const attachment = AiAttachment(
      id: 'file-1',
      name: 'notes.txt',
      mimeType: 'text/plain',
      size: 5,
      kind: AiAttachmentKind.text,
      text: 'notes',
    );

    expect(controller.addAttachments(const [attachment]), isNull);
    await controller.send('', config: config);

    expect(controller.pendingAttachments, isEmpty);
    expect(controller.messages.first.attachments, [attachment]);
    expect(
      requestMessages
          .singleWhere((message) => message.role == AiChatRole.user)
          .attachments,
      [attachment],
    );
  });

  test('conversation snapshot restores messages and command blocks', () {
    const entry = AiConversationEntry(
      uuid: '01979f62-8548-7000-8000-000000000001',
      title: 'Inspect service',
      scope: 'terminal',
      messages: [
        AiMessageEntry(
          uuid: '01979f62-8548-7000-8000-000000000002',
          role: 'assistant',
          content: 'I will inspect the service.',
          sequence: 0,
          toolCalls: [
            {
              'id': 'tool-1',
              'name': 'run_terminal_command',
              'arguments': {
                'command': 'systemctl status app',
                'explanation': 'Inspect the service status.',
              },
            },
          ],
        ),
      ],
      commandBlocks: [
        AiCommandBlockEntry(
          uuid: '01979f62-8548-7000-8000-000000000003',
          toolCallId: 'tool-1',
          command: 'systemctl status app',
          explanation: 'Inspect the service status.',
          status: 'succeeded',
          sequence: 1,
          output: 'active',
          exitCode: 0,
        ),
      ],
    );
    final controller = AiConversationController(initialConversation: entry);
    addTearDown(controller.dispose);

    expect(controller.persistenceId, entry.uuid);
    expect(controller.title, 'Inspect service');
    expect(controller.messages.single.persistenceId, isNotNull);
    expect(controller.messages.single.toolCalls.single.id, 'tool-1');
    expect(controller.terminalCommands.single.persistenceId, isNotNull);
    expect(
      controller.terminalCommands.single.status,
      AiTerminalCommandStatus.succeeded,
    );
    expect(controller.terminalCommands.single.output, 'active');

    final snapshot = controller.toEntry(scope: 'terminal');
    expect(snapshot.uuid, entry.uuid);
    expect(snapshot.messages.single.toolCalls.single['id'], 'tool-1');
    expect(snapshot.commandBlocks.single.exitCode, 0);

    controller.clear();
    expect(controller.persistenceId, isNull);
    expect(controller.messages, isEmpty);
    expect(controller.terminalCommands, isEmpty);

    expect(
      controller.addAttachments(const [
        AiAttachment(
          id: 'pending',
          name: 'pending.txt',
          mimeType: 'text/plain',
          size: 4,
          kind: AiAttachmentKind.text,
          text: 'test',
        ),
      ]),
      isNull,
    );
    expect(controller.load(entry), isTrue);
    expect(controller.persistenceId, entry.uuid);
    expect(controller.pendingAttachments, isEmpty);
  });
}

class _FakeAiProtocolClient extends AiProtocolClient {
  _FakeAiProtocolClient(this.handler);

  final Stream<AiStreamEvent> Function(List<AiChatMessage> messages) handler;

  @override
  Stream<AiStreamEvent> streamCompletion({
    required AiAssistantConfig config,
    required List<AiChatMessage> messages,
    bool enableTerminalTool = false,
  }) {
    return handler(messages);
  }
}

class _ImmediateTerminalRunner extends AiTerminalCommandRunner {
  @override
  Future<AiTerminalExecutionResult> run({
    required TerminalController controller,
    required String command,
  }) async {
    final startedAt = DateTime.now();
    return AiTerminalExecutionResult(
      output: command == 'pwd' ? '/tmp/project' : '',
      exitCode: 0,
      startedAt: startedAt,
      finishedAt: DateTime.now(),
    );
  }
}

class _CancellableTerminalRunner extends AiTerminalCommandRunner {
  final Completer<AiTerminalExecutionResult> _result = Completer();
  DateTime? _startedAt;

  @override
  Future<AiTerminalExecutionResult> run({
    required TerminalController controller,
    required String command,
  }) {
    _startedAt = DateTime.now();
    return _result.future;
  }

  @override
  bool cancel(TerminalController controller) {
    if (_result.isCompleted) {
      return false;
    }
    final startedAt = _startedAt ?? DateTime.now();
    _result.complete(
      AiTerminalExecutionResult(
        output: '',
        exitCode: 130,
        startedAt: startedAt,
        finishedAt: DateTime.now(),
        cancelled: true,
      ),
    );
    return true;
  }
}
