import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:flutter/foundation.dart';

import '../terminal/terminal_controller.dart';
import '../terminal/terminal_key_encoder.dart';
import '../terminal/terminal_models.dart';
import '../terminal/terminal_shell_integration.dart';
import 'ai_context.dart';

class AiTerminalExecutionResult {
  const AiTerminalExecutionResult({
    required this.output,
    required this.exitCode,
    required this.startedAt,
    required this.finishedAt,
    this.cancelled = false,
    this.resultTracked = true,
  });

  factory AiTerminalExecutionResult.submitted({required DateTime submittedAt}) {
    return AiTerminalExecutionResult(
      output: '',
      exitCode: null,
      startedAt: submittedAt,
      finishedAt: submittedAt,
      resultTracked: false,
    );
  }

  final String output;
  final int? exitCode;
  final DateTime startedAt;
  final DateTime finishedAt;
  final bool cancelled;
  final bool resultTracked;

  bool get submitted => !resultTracked;
  bool get succeeded => resultTracked && !cancelled && exitCode == 0;
}

class AiTerminalCommandCancelled implements Exception {
  const AiTerminalCommandCancelled();
}

class AiTerminalCommandRunner {
  AiTerminalCommandRunner({
    Random? random,
    Duration cancellationGracePeriod = const Duration(milliseconds: 1500),
  }) : _random = random ?? Random.secure(),
       _cancellationGracePeriod = cancellationGracePeriod;

  static const int _maximumCapturedBytes = 1024 * 1024;
  static const Duration _integrationTimeout = Duration(seconds: 4);
  static final Expando<Future<TerminalShellIntegration?>> _integrations =
      Expando();

  final Random _random;
  final Duration _cancellationGracePeriod;
  final Map<TerminalController, _ActiveTerminalCommand> _activeCommands = {};

  bool isBusy(TerminalController controller) =>
      _activeCommands.containsKey(controller);

  bool cancel(TerminalController controller) {
    final active = _activeCommands[controller];
    if (active == null || active.cancelRequested || controller.isDisposed) {
      return false;
    }
    active.requestCancellation(_cancellationGracePeriod);
    controller.cancelOutputSuppression();
    controller.sendInput('\x03');
    return true;
  }

  Future<AiTerminalExecutionResult> run({
    required TerminalController controller,
    required String command,
  }) async {
    if (controller.isDisposed) {
      throw StateError('The terminal session is closed.');
    }
    if (_activeCommands.containsKey(controller)) {
      throw StateError('Another AI command is already running.');
    }
    final active = _ActiveTerminalCommand();
    _activeCommands[controller] = active;
    void inputSentListener(String data) {
      if (data.contains('\x03') && !active.cancelRequested) {
        active.requestCancellation(_cancellationGracePeriod);
      }
    }

    controller.addInputSentListener(inputSentListener);

    try {
      TerminalShellIntegration? integration;
      try {
        integration = await _waitForForcedCancellation(
          _shellIntegration(controller),
          active,
        );
      } on AiTerminalCommandCancelled {
        _invalidateShellIntegration(controller);
        rethrow;
      } on Object {
        _invalidateShellIntegration(controller);
        try {
          await _waitForTerminalQuiet(controller);
          integration = await _waitForForcedCancellation(
            _shellIntegration(controller),
            active,
          );
        } on AiTerminalCommandCancelled {
          _invalidateShellIntegration(controller);
          rethrow;
        } on Object {
          integration = null;
        }
      }
      if (controller.isDisposed) {
        throw StateError('The terminal session is closed.');
      }
      if (active.cancelRequested) {
        throw const AiTerminalCommandCancelled();
      }
      if (integration != null) {
        try {
          return await _runIntegrated(
            controller: controller,
            command: command,
            integration: integration,
            active: active,
          );
        } on AiTerminalCommandCancelled {
          rethrow;
        } on StateError {
          rethrow;
        }
      }
      return _submitWithoutTracking(controller: controller, command: command);
    } finally {
      controller.removeInputSentListener(inputSentListener);
      _activeCommands.remove(controller);
      active.dispose();
    }
  }

  Future<TerminalShellIntegration?> _shellIntegration(
    TerminalController controller,
  ) {
    final existing = _integrations[controller];
    if (existing != null) {
      return existing;
    }
    final installing = _installShellIntegration(controller);
    _integrations[controller] = installing;
    unawaited(
      installing.then<void>((_) {}, onError: (Object _, StackTrace _) {}),
    );
    return installing;
  }

  Future<void> _waitForInitialShellOutput(TerminalController controller) async {
    const quietPeriod = Duration(milliseconds: 250);
    final completer = Completer<void>();
    Timer? quietTimer;
    late final ValueChanged<Uint8List> outputListener;
    late final VoidCallback controllerListener;
    late final VoidCallback disposeListener;
    void scheduleReadyIfPromptVisible() {
      quietTimer?.cancel();
      quietTimer = Timer(quietPeriod, () {
        if (!completer.isCompleted &&
            _currentTerminalLine(controller.snapshot).trim().isNotEmpty) {
          completer.complete();
        }
      });
    }

    outputListener = (bytes) {
      if (bytes.isNotEmpty) {
        scheduleReadyIfPromptVisible();
      }
    };
    controllerListener = () {
      final phase = controller.connectionStatus.phase;
      if ((phase == TerminalConnectionPhase.failed ||
              phase == TerminalConnectionPhase.exited) &&
          !completer.isCompleted) {
        completer.completeError(
          StateError('The shell exited before its first prompt.'),
        );
      }
    };
    disposeListener = () {
      if (!completer.isCompleted) {
        completer.completeError(StateError('The terminal session is closed.'));
      }
    };
    controller.addOutputListener(outputListener);
    controller.addListener(controllerListener);
    controller.addDisposeListener(disposeListener);
    try {
      controllerListener();
      if (_currentTerminalLine(controller.snapshot).trim().isNotEmpty) {
        scheduleReadyIfPromptVisible();
      }
      await completer.future.timeout(_integrationTimeout);
    } finally {
      quietTimer?.cancel();
      controller.removeOutputListener(outputListener);
      controller.removeListener(controllerListener);
      controller.removeDisposeListener(disposeListener);
    }
  }

  void _invalidateShellIntegration(TerminalController controller) {
    _integrations[controller] = null;
  }

  Future<T> _waitForForcedCancellation<T>(
    Future<T> future,
    _ActiveTerminalCommand active,
  ) {
    final completer = Completer<T>();
    active.attachForceCancellation(() {
      if (!completer.isCompleted) {
        completer.completeError(const AiTerminalCommandCancelled());
      }
    });
    future.then(
      (value) {
        if (!completer.isCompleted) {
          completer.complete(value);
        }
      },
      onError: (Object error, StackTrace stackTrace) {
        if (!completer.isCompleted) {
          completer.completeError(error, stackTrace);
        }
      },
    );
    return completer.future.whenComplete(active.detachForceCancellation);
  }

  Future<TerminalShellIntegration?> _installShellIntegration(
    TerminalController controller,
  ) async {
    final remoteSession =
        controller.sshProfile != null || controller.isMoshSession;
    final kind = terminalShellKindFromPath(
      remoteSession
          ? controller.reportedShellPath ?? controller.shellPath
          : controller.shellPath,
    );
    if (kind == null || !terminalShellSupportsStructuredIntegration(kind)) {
      return null;
    }
    if (!remoteSession) {
      final token = controller.shellIntegrationToken;
      if (token != null) {
        if (_currentTerminalLine(controller.snapshot).trim().isEmpty) {
          await _waitForInitialShellOutput(controller);
        }
        return TerminalShellIntegration(kind, token);
      }
    }
    if (controller.supportsShellIntegration &&
        _currentTerminalLine(controller.snapshot).trim().isEmpty) {
      await _waitForInitialShellOutput(controller);
    }
    if (_currentTerminalLine(controller.snapshot).isNotEmpty) {
      await _preparePosixCommandLine(controller);
    }

    final token = _token();
    final marker = '\x1b]777;nauterm-integration-ready=$token\x07';
    final output = await _sendHiddenCommand(
      controller: controller,
      command: terminalShellSetupCommand(kind, token),
      marker: marker,
    );
    if (output == null) {
      throw StateError('Unable to initialize shell integration.');
    }
    return TerminalShellIntegration(kind, token);
  }

  Future<Uint8List?> _sendHiddenCommand({
    required TerminalController controller,
    required String command,
    required String marker,
  }) async {
    if (!controller.suppressOutputUntil(marker)) {
      return null;
    }
    final markerBytes = utf8.encode(marker);
    final captured = <int>[];
    final completer = Completer<Uint8List>();
    late final ValueChanged<Uint8List> outputListener;
    late final VoidCallback disposeListener;

    outputListener = (bytes) {
      captured.addAll(bytes);
      if (_indexOf(captured, markerBytes) != -1 && !completer.isCompleted) {
        completer.complete(Uint8List.fromList(captured));
      } else if (captured.length > 64 * 1024) {
        captured.removeRange(0, captured.length - 64 * 1024);
      }
    };
    disposeListener = () {
      if (!completer.isCompleted) {
        completer.completeError(StateError('The terminal session is closed.'));
      }
    };

    controller.addOutputListener(outputListener);
    controller.addDisposeListener(disposeListener);
    try {
      controller.sendInput(
        command.endsWith('\r') ? command : '$command\r',
        sensitive: true,
      );
      return await completer.future.timeout(_integrationTimeout);
    } on Object {
      controller.cancelOutputSuppression();
      return null;
    } finally {
      controller.removeOutputListener(outputListener);
      controller.removeDisposeListener(disposeListener);
    }
  }

  Future<AiTerminalExecutionResult> _runIntegrated({
    required TerminalController controller,
    required String command,
    required TerminalShellIntegration integration,
    required _ActiveTerminalCommand active,
  }) async {
    final token = integration.token;
    await _waitForTerminalQuiet(controller);
    await _waitForForcedCancellation(
      _prepareCommandLine(controller, integration),
      active,
    );

    final parser = _IntegratedCommandParser(token, _maximumCapturedBytes);
    final completer = Completer<AiTerminalExecutionResult>();
    final startedAt = DateTime.now();
    late final ValueChanged<Uint8List> outputListener;
    late final VoidCallback controllerListener;
    late final VoidCallback disposeListener;

    void finishWithError(Object error) {
      if (!completer.isCompleted) {
        completer.completeError(error);
      }
    }

    outputListener = (bytes) {
      final parsed = parser.add(bytes);
      if (parsed == null || completer.isCompleted) {
        return;
      }
      var output = _plainTerminalOutput(
        utf8.decode(parsed.output, allowMalformed: true),
      );
      output = _removeCommandEcho(output, command);
      final sanitized = AiContextSanitizer.redact(output);
      completer.complete(
        AiTerminalExecutionResult(
          output: _summarizeOutput(sanitized.text),
          exitCode: parsed.exitCode,
          startedAt: startedAt,
          finishedAt: DateTime.now(),
          cancelled: active.cancelRequested,
        ),
      );
    };
    controllerListener = () {
      if (controller.isDisposed ||
          controller.connectionStatus.phase == TerminalConnectionPhase.exited ||
          controller.connectionStatus.phase == TerminalConnectionPhase.failed) {
        finishWithError(
          StateError(
            'The terminal session closed before the command finished.',
          ),
        );
      }
    };
    disposeListener = () {
      finishWithError(
        StateError('The terminal session closed before the command finished.'),
      );
    };

    controller.addOutputListener(outputListener);
    controller.addListener(controllerListener);
    controller.addDisposeListener(disposeListener);
    active.attachForceCancellation(
      () => finishWithError(const AiTerminalCommandCancelled()),
    );
    try {
      final normalized = command.replaceAll('\r\n', '\n').trimRight();
      final input = terminalPasteSequence(
        normalized,
        controller.snapshot.keyboardMode,
      );
      controller.sendInput(input.endsWith('\r') ? input : '$input\r');
      return await completer.future;
    } finally {
      controller.removeOutputListener(outputListener);
      controller.removeListener(controllerListener);
      controller.removeDisposeListener(disposeListener);
      active.detachForceCancellation();
    }
  }

  Future<void> _waitForTerminalQuiet(TerminalController controller) async {
    const quietPeriod = Duration(milliseconds: 60);
    final completer = Completer<void>();
    Timer? timer;
    late final ValueChanged<Uint8List> outputListener;
    late final VoidCallback disposeListener;
    void schedule() {
      timer?.cancel();
      timer = Timer(quietPeriod, () {
        if (!completer.isCompleted) {
          completer.complete();
        }
      });
    }

    outputListener = (bytes) {
      if (bytes.isNotEmpty) {
        schedule();
      }
    };
    disposeListener = () {
      if (!completer.isCompleted) {
        completer.completeError(StateError('The terminal session is closed.'));
      }
    };
    controller.addOutputListener(outputListener);
    controller.addDisposeListener(disposeListener);
    try {
      schedule();
      await completer.future.timeout(_integrationTimeout);
    } finally {
      timer?.cancel();
      controller.removeOutputListener(outputListener);
      controller.removeDisposeListener(disposeListener);
    }
  }

  Future<void> _prepareCommandLine(
    TerminalController controller,
    TerminalShellIntegration integration,
  ) async {
    if (integration.kind == TerminalShellKind.bash) {
      // Bash 3.2 starts a fresh display line whenever a `bind -x` command
      // writes the ready marker. Its Readline macro is synchronous, so a
      // pure editing macro can clear the line without a visible prompt redraw.
      controller.sendInput(integration.parkLineSequence, sensitive: true);
      return;
    }
    final token = integration.token;
    final marker = utf8.encode('\x1b]777;nauterm-line-ready=$token\x07');
    final pending = <int>[];
    final completer = Completer<void>();
    late final ValueChanged<Uint8List> outputListener;
    late final VoidCallback disposeListener;
    outputListener = (bytes) {
      pending.addAll(bytes);
      if (_indexOf(pending, marker) != -1 && !completer.isCompleted) {
        completer.complete();
      } else if (pending.length > marker.length * 2) {
        pending.removeRange(0, pending.length - marker.length * 2);
      }
    };
    disposeListener = () {
      if (!completer.isCompleted) {
        completer.completeError(StateError('The terminal session is closed.'));
      }
    };
    controller.addOutputListener(outputListener);
    controller.addDisposeListener(disposeListener);
    try {
      controller.sendInput(integration.parkLineSequence, sensitive: true);
      await completer.future.timeout(_integrationTimeout);
    } on TimeoutException {
      throw StateError('Unable to prepare a clean shell prompt.');
    } finally {
      controller.removeOutputListener(outputListener);
      controller.removeDisposeListener(disposeListener);
    }
  }

  Future<void> _preparePosixCommandLine(TerminalController controller) async {
    final before = _currentTerminalLine(controller.snapshot);
    final output = Completer<void>();
    late final ValueChanged<Uint8List> outputListener;
    outputListener = (bytes) {
      if (bytes.isNotEmpty && !output.isCompleted) {
        output.complete();
      }
    };
    controller.addOutputListener(outputListener);
    try {
      controller.sendInput('\x15', sensitive: true);
      await Future.any([
        output.future,
        Future<void>.delayed(const Duration(milliseconds: 60)),
      ]);
      await Future<void>.delayed(const Duration(milliseconds: 20));
      controller.refreshSnapshot();
    } finally {
      controller.removeOutputListener(outputListener);
    }

    final prompt = _currentTerminalLine(controller.snapshot);
    if (prompt.isEmpty || !before.startsWith(prompt)) {
      // A long prompt or draft can wrap onto the previous terminal row. In
      // that case the current row is insufficient to split prompt from draft,
      // but Ctrl-U has still cleared the shell's editable line. Preserve the
      // visible tail and continue installing integration on the clean line.
      if (before.isNotEmpty) {
        controller.write('$before\r\n$prompt');
      }
      return;
    }
    final draft = before.substring(prompt.length);
    if (draft.isNotEmpty) {
      controller.write('$draft\r\n$prompt');
    }
  }

  AiTerminalExecutionResult _submitWithoutTracking({
    required TerminalController controller,
    required String command,
  }) {
    final normalized = command.replaceAll('\r\n', '\n').trimRight();
    if (normalized.isEmpty) {
      throw StateError('The terminal command is empty.');
    }
    final input = terminalPasteSequence(
      normalized,
      controller.snapshot.keyboardMode,
    );
    final submittedAt = DateTime.now();
    controller.sendInput(input.endsWith('\r') ? input : '$input\r');
    return AiTerminalExecutionResult.submitted(submittedAt: submittedAt);
  }

  String _token() {
    final time = DateTime.now().microsecondsSinceEpoch.toRadixString(16);
    final random = List.generate(
      4,
      (_) => _random.nextInt(0x100000000).toRadixString(16).padLeft(8, '0'),
    ).join();
    return '$time$random';
  }
}

class _ActiveTerminalCommand {
  bool cancelRequested = false;
  Timer? _forceCancellationTimer;
  VoidCallback? _forceCancellation;
  bool _forceCancellationDue = false;

  void requestCancellation(Duration gracePeriod) {
    cancelRequested = true;
    _forceCancellationTimer = Timer(gracePeriod, () {
      _forceCancellationDue = true;
      _forceCancellation?.call();
    });
  }

  void attachForceCancellation(VoidCallback callback) {
    _forceCancellation = callback;
    if (_forceCancellationDue) {
      callback();
    }
  }

  void detachForceCancellation() {
    _forceCancellation = null;
  }

  void dispose() {
    _forceCancellationTimer?.cancel();
    _forceCancellationTimer = null;
    _forceCancellation = null;
  }
}

class _IntegratedCommandParser {
  _IntegratedCommandParser(String token, this.maximumBytes)
    : _endPrefix = utf8.encode('\x1b]777;nauterm-command-end=$token;');

  final List<int> _endPrefix;
  final int maximumBytes;
  final List<int> _pending = [];

  _ParsedTerminalResult? add(Uint8List bytes) {
    _pending.addAll(bytes);
    final endIndex = _indexOf(_pending, _endPrefix);
    if (endIndex == -1) {
      _trimCapturedOutput(_pending, maximumBytes);
      return null;
    }
    final codeStart = endIndex + _endPrefix.length;
    final bellIndex = _pending.indexOf(0x07, codeStart);
    var stringTerminatorIndex = -1;
    for (var index = codeStart; index + 1 < _pending.length; index++) {
      if (_pending[index] == 0x1b && _pending[index + 1] == 0x5c) {
        stringTerminatorIndex = index;
        break;
      }
    }
    final terminatorIndex = switch ((bellIndex, stringTerminatorIndex)) {
      (-1, -1) => -1,
      (-1, final index) => index,
      (final index, -1) => index,
      (final bell, final stringTerminator) =>
        bell < stringTerminator ? bell : stringTerminator,
    };
    if (terminatorIndex == -1) {
      return null;
    }
    final exitCode = int.tryParse(
      ascii.decode(_pending.sublist(codeStart, terminatorIndex)).trim(),
    );
    if (exitCode == null) {
      return null;
    }
    return _ParsedTerminalResult(
      output: Uint8List.fromList(_pending.sublist(0, endIndex)),
      exitCode: exitCode,
    );
  }
}

class _ParsedTerminalResult {
  const _ParsedTerminalResult({required this.output, required this.exitCode});

  final Uint8List output;
  final int exitCode;
}

String _currentTerminalLine(TerminalSnapshot snapshot) {
  final row = snapshot.cursor.row.clamp(0, snapshot.rows - 1);
  final column = snapshot.cursor.column.clamp(0, snapshot.columns);
  final start = row * snapshot.columns;
  final end = start + column;
  final buffer = StringBuffer();
  for (final cell in snapshot.cells.sublist(start, end)) {
    if (!cell.wideCharSpacer && !cell.leadingWideCharSpacer) {
      buffer.write(cell.text);
    }
  }
  return buffer.toString();
}

int _indexOf(List<int> bytes, List<int> pattern) {
  if (pattern.isEmpty || bytes.length < pattern.length) {
    return -1;
  }
  final lastStart = bytes.length - pattern.length;
  for (var index = 0; index <= lastStart; index++) {
    var matched = true;
    for (var offset = 0; offset < pattern.length; offset++) {
      if (bytes[index + offset] != pattern[offset]) {
        matched = false;
        break;
      }
    }
    if (matched) {
      return index;
    }
  }
  return -1;
}

void _trimCapturedOutput(List<int> pending, int maximumBytes) {
  if (pending.length > maximumBytes) {
    pending.removeRange(0, pending.length - maximumBytes);
  }
}

String _plainTerminalOutput(String value) {
  return AiContextSanitizer.plainTerminalText(value).trim();
}

String _removeCommandEcho(String output, String command) {
  var candidate = output;
  // Some interactive shells first echo the submitted line and then repaint it
  // as wrapped physical lines. Strip both representations, but keep the loop
  // bounded so command-like program output is not consumed indefinitely.
  for (var attempt = 0; attempt < 3; attempt++) {
    final stripped = _removeOneCommandEcho(candidate, command);
    if (stripped == candidate) {
      break;
    }
    candidate = stripped;
  }
  return candidate;
}

String _removeOneCommandEcho(String output, String command) {
  if (output.isEmpty) {
    return output;
  }
  final normalizedCommand = command.replaceAll('\r\n', '\n').trim();
  if (normalizedCommand.isEmpty) {
    return output;
  }
  final firstLineEnd = output.indexOf('\n');
  final firstLine = firstLineEnd == -1
      ? output
      : output.substring(0, firstLineEnd);
  var candidate = output;
  if (firstLine.trimRight().endsWith(normalizedCommand)) {
    candidate = firstLineEnd == -1
        ? ''
        : output.substring(firstLineEnd + 1).trim();
    if (candidate.isEmpty) {
      return candidate;
    }
  }

  // Interactive shells can repaint and wrap a long input line. Fish in
  // particular may emit a duplicated prefix before the complete wrapped
  // command, so comparing only the first physical line leaves the command in
  // captured output. Match a whitespace-normalized command near the start and
  // map its end back to the original output.
  final collapsedEnd = _normalizedCommandEchoEnd(
    candidate,
    normalizedCommand,
    preserveWhitespace: true,
  );
  if (collapsedEnd != null) {
    return candidate.substring(collapsedEnd).trim();
  }

  // Terminal plain-text export cannot distinguish a real space from the
  // padding inserted at a soft-wrap boundary. Retry without whitespace so a
  // wrapped `printf "${value}"` echo such as `printf " ${value}"` is removed.
  final compactEnd = _normalizedCommandEchoEnd(
    candidate,
    normalizedCommand,
    preserveWhitespace: false,
  );
  if (compactEnd != null) {
    return candidate.substring(compactEnd).trim();
  }
  return candidate;
}

int? _normalizedCommandEchoEnd(
  String output,
  String command, {
  required bool preserveWhitespace,
}) {
  final normalizedCommand = preserveWhitespace
      ? command.replaceAll(RegExp(r'\s+'), ' ').trim()
      : command.replaceAll(RegExp(r'\s+'), '');
  if (normalizedCommand.isEmpty) {
    return null;
  }
  final scanLength = min(
    output.length,
    max(4096, normalizedCommand.length * 4),
  );
  final normalized = StringBuffer();
  final originalOffsets = <int>[];
  var previousWasWhitespace = false;
  for (var index = 0; index < scanLength; index++) {
    final character = output[index];
    final whitespace = RegExp(r'\s').hasMatch(character);
    if (whitespace) {
      if (preserveWhitespace && !previousWasWhitespace) {
        normalized.write(' ');
        originalOffsets.add(index);
      }
    } else {
      normalized.write(character);
      originalOffsets.add(index);
    }
    previousWasWhitespace = whitespace;
  }
  final normalizedOutput = normalized.toString();
  final match = normalizedOutput.indexOf(normalizedCommand);
  if (match == -1 ||
      normalizedOutput.substring(0, match).trim().length >
          normalizedCommand.length) {
    return null;
  }
  final matchEnd = match + normalizedCommand.length - 1;
  if (matchEnd >= originalOffsets.length) {
    return null;
  }
  return originalOffsets[matchEnd] + 1;
}

String _summarizeOutput(String value) {
  const retainedCharacters = 6000;
  if (value.length <= retainedCharacters * 2) {
    return value;
  }
  return '${value.substring(0, retainedCharacters)}\n'
      '[Middle of terminal output omitted]\n'
      '${value.substring(value.length - retainedCharacters)}';
}
