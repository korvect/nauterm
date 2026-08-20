import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../terminal/terminal_controller.dart';
import '../data/nauterm_data_store.dart';
import 'ai_attachment.dart';
import 'ai_client.dart';
import 'ai_config.dart';
import 'ai_context.dart';
import 'ai_terminal_runner.dart';

enum AiTerminalCommandStatus {
  pending,
  running,
  submitted,
  succeeded,
  failed,
  cancelled,
  skipped,
}

sealed class AiConversationItem {
  const AiConversationItem(this.sequence);

  final int sequence;
}

class AiConversationMessageItem extends AiConversationItem {
  AiConversationMessageItem(this.message) : super(message.sequence);

  final AiChatMessage message;
}

class AiConversationCommandItem extends AiConversationItem {
  AiConversationCommandItem(this.command) : super(command.sequence);

  final AiTerminalCommand command;
}

const Object _preserveCommandValue = Object();

class AiTerminalCommand {
  const AiTerminalCommand({
    this.persistenceId,
    required this.id,
    required this.command,
    required this.explanation,
    this.status = AiTerminalCommandStatus.pending,
    this.sequence = 0,
    this.output,
    this.exitCode,
    this.error,
    this.startedAt,
    this.finishedAt,
    this.cancellationRequested = false,
  });

  final String? persistenceId;
  final String id;
  final String command;
  final String explanation;
  final AiTerminalCommandStatus status;
  final int sequence;
  final String? output;
  final int? exitCode;
  final String? error;
  final DateTime? startedAt;
  final DateTime? finishedAt;
  final bool cancellationRequested;

  AiTerminalCommand copyWith({
    AiTerminalCommandStatus? status,
    Object? output = _preserveCommandValue,
    Object? exitCode = _preserveCommandValue,
    Object? error = _preserveCommandValue,
    Object? startedAt = _preserveCommandValue,
    Object? finishedAt = _preserveCommandValue,
    bool? cancellationRequested,
  }) {
    return AiTerminalCommand(
      persistenceId: persistenceId,
      id: id,
      command: command,
      explanation: explanation,
      status: status ?? this.status,
      sequence: sequence,
      output: identical(output, _preserveCommandValue)
          ? this.output
          : output as String?,
      exitCode: identical(exitCode, _preserveCommandValue)
          ? this.exitCode
          : exitCode as int?,
      error: identical(error, _preserveCommandValue)
          ? this.error
          : error as String?,
      startedAt: identical(startedAt, _preserveCommandValue)
          ? this.startedAt
          : startedAt as DateTime?,
      finishedAt: identical(finishedAt, _preserveCommandValue)
          ? this.finishedAt
          : finishedAt as DateTime?,
      cancellationRequested:
          cancellationRequested ?? this.cancellationRequested,
    );
  }
}

class AiConversationController extends ChangeNotifier {
  AiConversationController({
    AiProtocolClient? client,
    AiTerminalCommandRunner? terminalRunner,
    AiConversationEntry? initialConversation,
    this._config,
  }) : _client = client ?? AiProtocolClient(),
       _terminalRunner = terminalRunner ?? AiTerminalCommandRunner() {
    if (initialConversation != null) {
      _restore(initialConversation);
    }
  }

  final AiProtocolClient _client;
  final AiTerminalCommandRunner _terminalRunner;
  AiAssistantConfig? _config;
  String? _providerUuid;
  String? _currentModel;
  final List<AiChatMessage> _messages = [];
  final List<AiTerminalCommand> _terminalCommands = [];
  final Map<String, TerminalController> _terminalCommandControllers = {};
  final List<AiAttachment> _pendingAttachments = [];
  final Set<AiContextKind> _disabledContextKinds = {};
  StreamSubscription<AiStreamEvent>? _activeSubscription;
  Completer<void>? _activeCompleter;
  bool _sending = false;
  bool _stopping = false;
  bool _disposed = false;
  String? _error;
  int _nextSequence = 0;
  String? _persistenceId;
  String _title = 'New conversation';

  String? get persistenceId => _persistenceId;
  String get title => _title;
  String? get providerUuid => _providerUuid;
  String? get currentModel => _currentModel;
  AiAssistantConfig get effectiveConfig => _config ?? aiAssistantConfig;
  AiAssistantConfig get config => effectiveConfig;

  void updateConfig(AiAssistantConfig config) {
    _config = config;
    if (_error != null && config.validationError == null) {
      _error = null;
    }
    notifyListeners();
  }

  void updateProvider(String? providerUuid, String? model) {
    _providerUuid = providerUuid;
    _currentModel = model;
    notifyListeners();
  }

  List<AiChatMessage> get messages => List.unmodifiable(_messages);
  List<AiTerminalCommand> get terminalCommands =>
      List.unmodifiable(_terminalCommands);
  List<AiAttachment> get pendingAttachments =>
      List.unmodifiable(_pendingAttachments);
  List<AiConversationItem> get timeline {
    final items = <AiConversationItem>[
      for (final message in _messages)
        if (message.role != AiChatRole.tool) AiConversationMessageItem(message),
      for (final command in _terminalCommands)
        AiConversationCommandItem(command),
    ]..sort((left, right) => left.sequence.compareTo(right.sequence));
    return List.unmodifiable(items);
  }

  bool get sending => _sending;
  bool get stopping => _stopping;
  bool get hasRunningTerminalCommand => _terminalCommands.any(
    (command) => command.status == AiTerminalCommandStatus.running,
  );
  bool get hasUnresolvedTerminalCommands => _terminalCommands.any(
    (command) =>
        command.status == AiTerminalCommandStatus.pending ||
        command.status == AiTerminalCommandStatus.running,
  );
  String? get error => _error;

  bool isContextKindEnabled(AiContextKind kind) {
    return !_disabledContextKinds.contains(kind);
  }

  Set<AiContextKind> get enabledContextKinds => {
    for (final kind in AiContextKind.values)
      if (isContextKindEnabled(kind)) kind,
  };

  Future<void> send(
    String text, {
    AiAssistantConfig? config,
    String? systemPrompt,
    bool enableTerminalTool = false,
    AiTerminalContext? terminalContext,
  }) async {
    final effectiveConfig = config ?? this.effectiveConfig;
    final content = text.trim();
    if ((content.isEmpty && _pendingAttachments.isEmpty) ||
        _sending ||
        hasUnresolvedTerminalCommands ||
        _disposed) {
      return;
    }
    final validationError = effectiveConfig.validationError;
    if (validationError != null) {
      _error = validationError;
      notifyListeners();
      return;
    }

    final createdAt = DateTime.now();
    _messages.add(
      AiChatMessage(
        role: AiChatRole.user,
        content: content,
        context: terminalContext?.toPromptText() ?? '',
        sequence: _nextSequence++,
        attachments: List.unmodifiable(_pendingAttachments),
        createdAt: createdAt,
      ),
    );
    if (_messages.where((message) => message.role == AiChatRole.user).length ==
        1) {
      _title = _conversationTitle(content, _pendingAttachments);
    }
    _pendingAttachments.clear();
    await _requestAssistant(
      effectiveConfig,
      systemPrompt: systemPrompt,
      enableTerminalTool: enableTerminalTool,
    );
  }

  Future<bool> editUserMessage(
    int sequence,
    String text, {
    AiAssistantConfig? config,
    String? systemPrompt,
    bool enableTerminalTool = false,
  }) async {
    final effectiveConfig = config ?? this.effectiveConfig;
    final content = text.trim();
    final index = _messages.indexWhere(
      (message) =>
          message.sequence == sequence && message.role == AiChatRole.user,
    );
    if (index == -1 || _sending || hasUnresolvedTerminalCommands || _disposed) {
      return false;
    }
    final original = _messages[index];
    if (content.isEmpty && original.attachments.isEmpty) {
      return false;
    }
    final validationError = effectiveConfig.validationError;
    if (validationError != null) {
      _error = validationError;
      notifyListeners();
      return false;
    }

    final firstUserSequence = _messages
        .firstWhere((message) => message.role == AiChatRole.user)
        .sequence;
    _messages.removeWhere((message) => message.sequence > sequence);
    _terminalCommands.removeWhere((command) => command.sequence > sequence);
    final retainedIndex = _messages.indexWhere(
      (message) => message.sequence == sequence,
    );
    _messages[retainedIndex] = original.copyWith(
      content: content,
      updatedAt: DateTime.now(),
    );
    _nextSequence = sequence + 1;
    _error = null;
    if (sequence == firstUserSequence) {
      _title = _conversationTitle(content, original.attachments);
    }

    await _requestAssistant(
      effectiveConfig,
      systemPrompt: systemPrompt,
      enableTerminalTool: enableTerminalTool,
    );
    return true;
  }

  Future<void> executeTerminalCommand(
    String id, {
    required TerminalController controller,
    AiAssistantConfig? config,
    String? systemPrompt,
    bool enableTerminalTool = true,
  }) async {
    final effectiveConfig = config ?? this.effectiveConfig;
    final index = _terminalCommands.indexWhere((command) => command.id == id);
    if (index == -1 ||
        _disposed ||
        _sending ||
        _terminalCommands[index].status != AiTerminalCommandStatus.pending) {
      return;
    }
    final command = _terminalCommands[index];
    _terminalCommands[index] = command.copyWith(
      status: AiTerminalCommandStatus.running,
      startedAt: DateTime.now(),
      output: null,
      exitCode: null,
      error: null,
      finishedAt: null,
      cancellationRequested: false,
    );
    _error = null;
    notifyListeners();

    _terminalCommandControllers[id] = controller;
    try {
      final result = await _terminalRunner.run(
        controller: controller,
        command: command.command,
      );
      if (_disposed) {
        _terminalCommandControllers.remove(id);
        return;
      }
      final currentIndex = _terminalCommands.indexWhere(
        (candidate) => candidate.id == id,
      );
      if (currentIndex == -1) {
        _terminalCommandControllers.remove(id);
        return;
      }
      _terminalCommands[currentIndex] = _terminalCommands[currentIndex]
          .copyWith(
            status: result.cancelled
                ? AiTerminalCommandStatus.cancelled
                : result.submitted
                ? AiTerminalCommandStatus.submitted
                : result.succeeded
                ? AiTerminalCommandStatus.succeeded
                : AiTerminalCommandStatus.failed,
            output: result.output,
            exitCode: result.exitCode,
            finishedAt: result.finishedAt,
          );
    } on Object catch (error) {
      if (_disposed) {
        _terminalCommandControllers.remove(id);
        return;
      }
      final cancelled = error is AiTerminalCommandCancelled;
      final message = cancelled
          ? 'Stopped by user.'
          : _terminalExecutionError(error);
      final currentIndex = _terminalCommands.indexWhere(
        (candidate) => candidate.id == id,
      );
      if (currentIndex == -1) {
        _terminalCommandControllers.remove(id);
        return;
      }
      _terminalCommands[currentIndex] = _terminalCommands[currentIndex]
          .copyWith(
            status: cancelled
                ? AiTerminalCommandStatus.cancelled
                : AiTerminalCommandStatus.failed,
            error: message,
            finishedAt: DateTime.now(),
          );
    }

    _terminalCommandControllers.remove(id);
    notifyListeners();
    await _submitResolvedTerminalCommandBatch(
      id,
      effectiveConfig,
      systemPrompt: systemPrompt,
      enableTerminalTool: enableTerminalTool && !controller.isDisposed,
    );
  }

  Future<void> skipTerminalCommand(
    String id, {
    AiAssistantConfig? config,
    String? systemPrompt,
    bool enableTerminalTool = true,
  }) async {
    final effectiveConfig = config ?? this.effectiveConfig;
    final index = _terminalCommands.indexWhere((command) => command.id == id);
    if (index == -1 ||
        _disposed ||
        _sending ||
        _terminalCommands[index].status != AiTerminalCommandStatus.pending) {
      return;
    }
    _terminalCommands[index] = _terminalCommands[index].copyWith(
      status: AiTerminalCommandStatus.skipped,
      error: 'Skipped by user.',
      finishedAt: DateTime.now(),
    );
    notifyListeners();
    await _submitResolvedTerminalCommandBatch(
      id,
      effectiveConfig,
      systemPrompt: systemPrompt,
      enableTerminalTool: enableTerminalTool,
    );
  }

  Future<void> _submitResolvedTerminalCommandBatch(
    String commandId,
    AiAssistantConfig config, {
    String? systemPrompt,
    required bool enableTerminalTool,
  }) async {
    AiChatMessage? assistant;
    for (final message in _messages.reversed) {
      if (message.role == AiChatRole.assistant &&
          message.toolCalls.any((call) => call.id == commandId)) {
        assistant = message;
        break;
      }
    }
    if (assistant == null) {
      return;
    }
    final commandIds = assistant.toolCalls
        .where((call) => call.name == 'run_terminal_command')
        .map((call) => call.id)
        .toList(growable: false);
    final commands = <AiTerminalCommand>[];
    for (final id in commandIds) {
      final command = _terminalCommands
          .where((candidate) => candidate.id == id)
          .firstOrNull;
      if (command == null || !_terminalCommandResolved(command.status)) {
        return;
      }
      commands.add(command);
    }
    final alreadySubmitted = _messages.any(
      (message) =>
          message.toolResult != null &&
          commandIds.contains(message.toolResult!.toolCallId),
    );
    if (alreadySubmitted || commands.isEmpty) {
      return;
    }
    for (final command in commands) {
      _messages.add(
        AiChatMessage(
          role: AiChatRole.tool,
          content: '',
          sequence: _nextSequence++,
          toolResult: _terminalCommandToolResult(command),
          createdAt: DateTime.now(),
        ),
      );
    }
    notifyListeners();
    await _requestAssistant(
      config,
      systemPrompt: systemPrompt,
      enableTerminalTool: enableTerminalTool,
    );
  }

  bool cancelTerminalCommand(String id) {
    final index = _terminalCommands.indexWhere((command) => command.id == id);
    final controller = _terminalCommandControllers[id];
    if (index == -1 ||
        controller == null ||
        _disposed ||
        _terminalCommands[index].status != AiTerminalCommandStatus.running ||
        _terminalCommands[index].cancellationRequested ||
        !_terminalRunner.cancel(controller)) {
      return false;
    }
    _terminalCommands[index] = _terminalCommands[index].copyWith(
      cancellationRequested: true,
    );
    notifyListeners();
    return true;
  }

  Future<bool> stopGenerating() async {
    if (_disposed || !_sending || _stopping) {
      return false;
    }
    _stopping = true;
    notifyListeners();

    final subscription = _activeSubscription;
    final completer = _activeCompleter;
    if (completer != null && !completer.isCompleted) {
      completer.complete();
    }
    try {
      await subscription?.cancel();
    } on Object {
      // The conversation is already stopped even if socket cleanup fails.
    }
    return true;
  }

  Future<void> _requestAssistant(
    AiAssistantConfig config, {
    String? systemPrompt,
    required bool enableTerminalTool,
  }) async {
    if (_sending || _disposed) {
      return;
    }
    _error = null;
    _sending = true;
    _stopping = false;
    final assistantIndex = _messages.length;
    _messages.add(
      AiChatMessage(
        role: AiChatRole.assistant,
        content: '',
        sequence: _nextSequence++,
        createdAt: DateTime.now(),
      ),
    );
    notifyListeners();

    final completer = Completer<void>();
    _activeCompleter = completer;
    final requestMessages = <AiChatMessage>[
      if (systemPrompt != null && systemPrompt.trim().isNotEmpty)
        AiChatMessage(role: AiChatRole.system, content: systemPrompt.trim()),
      ...List.of(_messages)..removeLast(),
    ];
    _activeSubscription = _client
        .streamCompletion(
          config: config,
          messages: requestMessages,
          enableTerminalTool: enableTerminalTool,
        )
        .listen(
          (event) {
            if (_disposed || assistantIndex >= _messages.length) {
              return;
            }
            switch (event) {
              case AiTextDelta(:final text):
                final message = _messages[assistantIndex];
                _messages[assistantIndex] = message.copyWith(
                  content: '${message.content}$text',
                );
                notifyListeners();
              case AiToolCall():
                final id = event.id.isEmpty
                    ? 'terminal-command-${_terminalCommands.length + 1}'
                    : event.id;
                final toolCall = AiToolCall(
                  id: id,
                  name: event.name,
                  arguments: event.arguments,
                );
                final message = _messages[assistantIndex];
                final explanation = toolCall.terminalCommandExplanation?.trim();
                _messages[assistantIndex] = message.copyWith(
                  content:
                      message.content.trim().isEmpty &&
                          explanation != null &&
                          explanation.isNotEmpty
                      ? explanation
                      : message.content,
                  toolCalls: [...message.toolCalls, toolCall],
                );
                final command = toolCall.terminalCommand?.trim();
                if (command != null && command.isNotEmpty) {
                  _terminalCommands.add(
                    AiTerminalCommand(
                      id: id,
                      command: command,
                      explanation: explanation == null || explanation.isEmpty
                          ? 'Run this command in the active terminal.'
                          : explanation,
                      sequence: _nextSequence++,
                    ),
                  );
                }
                notifyListeners();
            }
          },
          onError: (Object error, StackTrace stackTrace) {
            if (!_disposed) {
              _error = error is AiProtocolException
                  ? error.message
                  : 'Unable to reach the AI provider.';
            }
            if (!completer.isCompleted) {
              completer.complete();
            }
          },
          onDone: () {
            if (!completer.isCompleted) {
              completer.complete();
            }
          },
          cancelOnError: true,
        );

    await completer.future;
    _activeSubscription = null;
    _activeCompleter = null;
    if (_disposed || assistantIndex >= _messages.length) {
      return;
    }
    final assistant = _messages[assistantIndex];
    if (assistant.content.isEmpty && assistant.toolCalls.isEmpty) {
      _messages.removeAt(assistantIndex);
    }
    _stopping = false;
    _sending = false;
    notifyListeners();
  }

  void clear() {
    if (_sending ||
        hasRunningTerminalCommand ||
        (_messages.isEmpty &&
            _terminalCommands.isEmpty &&
            _pendingAttachments.isEmpty) ||
        _disposed) {
      return;
    }
    _messages.clear();
    _terminalCommands.clear();
    _pendingAttachments.clear();
    _disabledContextKinds.clear();
    _nextSequence = 0;
    _persistenceId = null;
    _title = 'New conversation';
    _error = null;
    notifyListeners();
  }

  AiConversationEntry toEntry({required String scope, String? hostUuid}) {
    return AiConversationEntry(
      uuid: _persistenceId,
      title: _title,
      scope: scope,
      hostUuid: hostUuid,
      providerUuid: _providerUuid,
      model: _currentModel ?? '',
      messages: [for (final message in _messages) _messageToEntry(message)],
      commandBlocks: [
        for (final command in _terminalCommands) _commandToEntry(command),
      ],
    );
  }

  AiConversationEntry saveTo(
    NautermDataStore store, {
    required String scope,
    String? hostUuid,
  }) {
    final saved = store.saveAiConversation(
      toEntry(scope: scope, hostUuid: hostUuid),
    );
    _applyPersistenceIds(saved);
    return saved;
  }

  bool load(AiConversationEntry conversation) {
    if (_disposed || _sending || hasRunningTerminalCommand) {
      return false;
    }
    _pendingAttachments.clear();
    _disabledContextKinds.clear();
    _error = null;
    _restore(conversation);
    notifyListeners();
    return true;
  }

  void _restore(AiConversationEntry conversation) {
    _persistenceId = conversation.uuid;
    _title = conversation.title;
    _providerUuid = conversation.providerUuid;
    _currentModel = conversation.model.isNotEmpty ? conversation.model : null;
    _messages
      ..clear()
      ..addAll(conversation.messages.map(_messageFromEntry));
    _terminalCommands
      ..clear()
      ..addAll(conversation.commandBlocks.map(_commandFromEntry));
    _nextSequence =
        [
          for (final message in _messages) message.sequence,
          for (final command in _terminalCommands) command.sequence,
        ].fold<int>(-1, (highest, value) => value > highest ? value : highest) +
        1;
  }

  void _applyPersistenceIds(AiConversationEntry saved) {
    _persistenceId = saved.uuid;
    _title = saved.title;
    final savedMessages = {
      for (final item in saved.messages) item.sequence: item,
    };
    for (var index = 0; index < _messages.length; index += 1) {
      final message = _messages[index];
      final savedMessage = savedMessages[message.sequence];
      _messages[index] = AiChatMessage(
        persistenceId: savedMessage?.uuid ?? message.persistenceId,
        role: message.role,
        content: message.content,
        context: message.context,
        sequence: message.sequence,
        toolCalls: message.toolCalls,
        toolResult: message.toolResult,
        attachments: message.attachments,
        createdAt: savedMessage?.createdAt ?? message.createdAt,
        updatedAt: savedMessage?.updatedAt ?? message.updatedAt,
      );
    }
    final commandIds = {
      for (final item in saved.commandBlocks) item.toolCallId: item.uuid,
    };
    for (var index = 0; index < _terminalCommands.length; index += 1) {
      final command = _terminalCommands[index];
      _terminalCommands[index] = AiTerminalCommand(
        persistenceId: commandIds[command.id] ?? command.persistenceId,
        id: command.id,
        command: command.command,
        explanation: command.explanation,
        status: command.status,
        sequence: command.sequence,
        output: command.output,
        exitCode: command.exitCode,
        error: command.error,
        startedAt: command.startedAt,
        finishedAt: command.finishedAt,
        cancellationRequested: command.cancellationRequested,
      );
    }
  }

  String? addAttachments(Iterable<AiAttachment> attachments) {
    if (_disposed) {
      return 'The conversation is closed.';
    }
    final additions = attachments.toList(growable: false);
    if (_pendingAttachments.length + additions.length >
        AiAttachment.maximumCount) {
      return 'Attach up to ${AiAttachment.maximumCount} files per message.';
    }
    final existingIds = _pendingAttachments.map((item) => item.id).toSet();
    _pendingAttachments.addAll(
      additions.where((attachment) => existingIds.add(attachment.id)),
    );
    notifyListeners();
    return null;
  }

  void removePendingAttachment(String id) {
    if (_disposed) {
      return;
    }
    final previousLength = _pendingAttachments.length;
    _pendingAttachments.removeWhere((attachment) => attachment.id == id);
    if (_pendingAttachments.length != previousLength) {
      notifyListeners();
    }
  }

  void setContextKindEnabled(AiContextKind kind, bool enabled) {
    if (_disposed) {
      return;
    }
    final changed = enabled
        ? _disabledContextKinds.remove(kind)
        : _disabledContextKinds.add(kind);
    if (changed) {
      notifyListeners();
    }
  }

  void restoreTerminalContext() {
    if (_disposed || _disabledContextKinds.isEmpty) {
      return;
    }
    _disabledContextKinds.clear();
    notifyListeners();
  }

  void prepareForClose() {
    if (_disposed) {
      return;
    }
    final subscription = _activeSubscription;
    _activeSubscription = null;
    unawaited(_cancelSubscription(subscription));
    if (_activeCompleter case final completer? when !completer.isCompleted) {
      completer.complete();
    }
    _activeCompleter = null;

    final assistant = _messages.lastOrNull;
    if (_sending &&
        assistant != null &&
        assistant.role == AiChatRole.assistant &&
        assistant.content.isEmpty &&
        assistant.toolCalls.isEmpty) {
      _messages.removeLast();
    }
    _sending = false;
    _stopping = false;

    final finishedAt = DateTime.now();
    for (final entry in _terminalCommandControllers.entries) {
      _terminalRunner.cancel(entry.value);
      final index = _terminalCommands.indexWhere(
        (command) => command.id == entry.key,
      );
      if (index == -1 ||
          _terminalCommands[index].status != AiTerminalCommandStatus.running) {
        continue;
      }
      _terminalCommands[index] = _terminalCommands[index].copyWith(
        status: AiTerminalCommandStatus.cancelled,
        finishedAt: finishedAt,
        cancellationRequested: true,
      );
    }
    _terminalCommandControllers.clear();
  }

  @override
  void dispose() {
    prepareForClose();
    _disposed = true;
    super.dispose();
  }
}

Future<void> _cancelSubscription(
  StreamSubscription<AiStreamEvent>? subscription,
) async {
  try {
    await subscription?.cancel();
  } on Object {
    // Closing the conversation already made this request irrelevant.
  }
}

bool _terminalCommandResolved(AiTerminalCommandStatus status) {
  return switch (status) {
    AiTerminalCommandStatus.pending || AiTerminalCommandStatus.running => false,
    AiTerminalCommandStatus.submitted ||
    AiTerminalCommandStatus.succeeded ||
    AiTerminalCommandStatus.failed ||
    AiTerminalCommandStatus.cancelled ||
    AiTerminalCommandStatus.skipped => true,
  };
}

AiToolResult _terminalCommandToolResult(AiTerminalCommand command) {
  final skipped = command.status == AiTerminalCommandStatus.skipped;
  final cancelled = command.status == AiTerminalCommandStatus.cancelled;
  final submitted = command.status == AiTerminalCommandStatus.submitted;
  final succeeded = command.status == AiTerminalCommandStatus.succeeded;
  return AiToolResult(
    toolCallId: command.id,
    name: 'run_terminal_command',
    content: jsonEncode(<String, Object?>{
      'command': command.command,
      'explanation': command.explanation,
      if (command.exitCode != null) 'exit_code': command.exitCode,
      if (command.output != null) 'output': command.output,
      if (command.error != null) 'error': command.error,
      if (command.startedAt != null)
        'started_at': command.startedAt!.toIso8601String(),
      if (command.finishedAt != null)
        'finished_at': command.finishedAt!.toIso8601String(),
      'cancelled': cancelled,
      'skipped': skipped,
      'submitted': submitted,
      'result_tracked': !submitted,
    }),
    isError: !succeeded && !submitted,
  );
}

String _terminalExecutionError(Object error) {
  if (error is StateError) {
    return error.message.toString();
  }
  return 'Unable to run the command in the active terminal.';
}

String _conversationTitle(String content, List<AiAttachment> attachments) {
  final firstLine = content
      .split('\n')
      .map((line) => line.trim())
      .firstWhere((line) => line.isNotEmpty, orElse: () => '');
  final source = firstLine.isNotEmpty
      ? firstLine
      : attachments.isEmpty
      ? 'New conversation'
      : attachments.first.name;
  return source.length <= 60 ? source : '${source.substring(0, 57)}...';
}

AiMessageEntry _messageToEntry(AiChatMessage message) {
  return AiMessageEntry(
    uuid: message.persistenceId,
    role: message.role.name,
    content: message.content,
    context: message.context,
    sequence: message.sequence,
    toolCalls: [
      for (final call in message.toolCalls)
        {'id': call.id, 'name': call.name, 'arguments': call.arguments},
    ],
    toolResult: switch (message.toolResult) {
      final result? => {
        'tool_call_id': result.toolCallId,
        'name': result.name,
        'content': result.content,
        'is_error': result.isError,
      },
      null => null,
    },
    attachments: [
      for (final attachment in message.attachments)
        {
          'id': attachment.id,
          'name': attachment.name,
          'mime_type': attachment.mimeType,
          'size': attachment.size,
          'kind': attachment.kind.name,
          'text': attachment.text,
          'bytes': attachment.base64Data,
          'redacted': attachment.redacted,
        },
    ],
    createdAt: message.createdAt,
    updatedAt: message.updatedAt,
  );
}

AiChatMessage _messageFromEntry(AiMessageEntry entry) {
  return AiChatMessage(
    persistenceId: entry.uuid,
    role: AiChatRole.values.byName(entry.role),
    content: entry.content,
    context: entry.context,
    sequence: entry.sequence,
    toolCalls: [
      for (final call in entry.toolCalls)
        AiToolCall(
          id: call['id'] as String? ?? '',
          name: call['name'] as String? ?? '',
          arguments: switch (call['arguments']) {
            final Map value => value.map(
              (key, value) => MapEntry(key.toString(), value),
            ),
            _ => const {},
          },
        ),
    ],
    toolResult: switch (entry.toolResult) {
      final result? => AiToolResult(
        toolCallId: result['tool_call_id'] as String? ?? '',
        name: result['name'] as String? ?? '',
        content: result['content'] as String? ?? '',
        isError: result['is_error'] as bool? ?? false,
      ),
      null => null,
    },
    attachments: [
      for (final attachment in entry.attachments)
        AiAttachment(
          id: attachment['id'] as String? ?? '',
          name: attachment['name'] as String? ?? '',
          mimeType: attachment['mime_type'] as String? ?? '',
          size: (attachment['size'] as num?)?.toInt() ?? 0,
          kind: AiAttachmentKind.values.byName(
            attachment['kind'] as String? ?? AiAttachmentKind.text.name,
          ),
          text: attachment['text'] as String?,
          bytes: switch (attachment['bytes']) {
            final String value when value.isNotEmpty => base64Decode(value),
            _ => null,
          },
          redacted: attachment['redacted'] as bool? ?? false,
        ),
    ],
    createdAt: entry.createdAt,
    updatedAt: entry.updatedAt,
  );
}

AiCommandBlockEntry _commandToEntry(AiTerminalCommand command) {
  return AiCommandBlockEntry(
    uuid: command.persistenceId,
    toolCallId: command.id,
    command: command.command,
    explanation: command.explanation,
    status: command.status.name,
    sequence: command.sequence,
    output: command.output,
    exitCode: command.exitCode,
    error: command.error,
    startedAt: command.startedAt,
    finishedAt: command.finishedAt,
  );
}

AiTerminalCommand _commandFromEntry(AiCommandBlockEntry entry) {
  return AiTerminalCommand(
    persistenceId: entry.uuid,
    id: entry.toolCallId,
    command: entry.command,
    explanation: entry.explanation,
    status: AiTerminalCommandStatus.values.byName(entry.status),
    sequence: entry.sequence,
    output: entry.output,
    exitCode: entry.exitCode,
    error: entry.error,
    startedAt: entry.startedAt,
    finishedAt: entry.finishedAt,
    cancellationRequested: false,
  );
}
