import 'dart:io';
import 'dart:convert';

// Real-shell integration coverage; intentionally excluded from `flutter test`.

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ai/ai_context.dart';
import 'package:nauterm/ai/ai_terminal_runner.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_shell_integration.dart';

void main() {
  for (final shell in <String, String>{
    'bash': '/bin/bash',
    'dash': '/bin/dash',
    'fish': _firstExistingPath(const [
      '/opt/homebrew/bin/fish',
      '/usr/local/bin/fish',
      '/usr/bin/fish',
    ]),
    'ksh': '/bin/ksh',
    'sh': '/bin/sh',
  }.entries) {
    test('runs through a real ${shell.key} PTY', () async {
      final controller = TerminalController(
        shellPath: shell.value,
        environment: const {'TERM': 'xterm-256color'},
        config: defaultTerminalConfig,
      );
      addTearDown(controller.dispose);
      final output = <int>[];
      controller.addOutputListener(output.addAll);
      await _waitForInitialPrompt(controller);
      final isFish = shell.key == 'fish';
      final shellKind = terminalShellKindFromPath(shell.value);
      final tracksResult =
          shellKind != null &&
          terminalShellSupportsStructuredIntegration(shellKind);
      final draft = isFish
          ? 'set -g NAUTERM_DRAFT_EXECUTED 1'
          : 'NAUTERM_DRAFT_EXECUTED=1';
      controller.sendInput(draft);
      if (tracksResult) {
        await _waitForPtyEcho(output, draft);
      } else {
        await Future<void>.delayed(const Duration(milliseconds: 80));
      }
      final command = isFish
          ? 'if set -q NAUTERM_DRAFT_EXECUTED; echo dirty; else; echo clean; end'
          : r'''printf "${NAUTERM_DRAFT_EXECUTED:-clean}"''';

      late final AiTerminalExecutionResult result;
      try {
        result = await AiTerminalCommandRunner()
            .run(controller: controller, command: command)
            .timeout(const Duration(seconds: 12));
      } on Object catch (error) {
        fail(
          '${shell.key} failed: $error\n'
          'Output: ${utf8.decode(output, allowMalformed: true)}',
        );
      }

      final diagnostic =
          '${shell.key} result: '
          'tracked=${result.resultTracked}, '
          'exitCode=${result.exitCode}, '
          'resultOutput=${result.output}\n'
          'PTY output: ${utf8.decode(output, allowMalformed: true)}';
      if (tracksResult) {
        expect(result.resultTracked, isTrue, reason: diagnostic);
        expect(result.exitCode, 0, reason: diagnostic);
        expect(result.output, 'clean', reason: diagnostic);
      } else {
        expect(result.submitted, isTrue, reason: diagnostic);
        expect(result.exitCode, isNull, reason: diagnostic);
      }
      final visibleText = controller.snapshot.cells
          .map((cell) => cell.text)
          .join();
      final observedText = AiContextSanitizer.plainTerminalText(
        '${utf8.decode(output, allowMalformed: true)}\n$visibleText',
      ).replaceAll(RegExp(r'\s+'), '');
      expect(
        observedText,
        contains(draft.replaceAll(RegExp(r'\s+'), '')),
        reason: diagnostic,
      );
    }, skip: _shellSkipReason(shell.value));
  }

  test(
    'macOS login shell preserves the configured working directory',
    () async {
      final directory = Directory.systemTemp.createTempSync(
        'nauterm-pty-working-directory-',
      );
      addTearDown(() => directory.deleteSync(recursive: true));
      final controller = TerminalController(
        shellPath: '/bin/bash',
        workingDirectory: directory.path,
        environment: const {'TERM': 'xterm-256color'},
        config: defaultTerminalConfig,
      );
      addTearDown(controller.dispose);
      await _waitForInitialPrompt(controller);

      final result = await AiTerminalCommandRunner()
          .run(controller: controller, command: 'pwd')
          .timeout(const Duration(seconds: 12));

      expect(result.exitCode, 0);
      expect(result.output, directory.resolveSymbolicLinksSync());
    },
    skip: Platform.isMacOS ? null : 'macOS login wrapper coverage',
  );
}

String _firstExistingPath(List<String> candidates) {
  for (final path in candidates) {
    if (File(path).existsSync()) return path;
  }
  return candidates.first;
}

String? _shellSkipReason(String path) {
  if (Platform.isWindows) return 'Unix PTY integration test';
  if (!File(path).existsSync()) return 'Shell is not installed: $path';
  return null;
}

Future<void> _waitForInitialPrompt(TerminalController controller) async {
  final deadline = DateTime.now().add(const Duration(seconds: 8));
  while (DateTime.now().isBefore(deadline)) {
    if (controller.connectionStatus.phase ==
            TerminalConnectionPhase.connected &&
        controller.snapshot.cells.any((cell) => cell.text.trim().isNotEmpty)) {
      await Future<void>.delayed(const Duration(milliseconds: 120));
      return;
    }
    await Future<void>.delayed(const Duration(milliseconds: 20));
  }
  throw StateError('The test shell did not produce its first prompt.');
}

Future<void> _waitForPtyEcho(List<int> output, String value) async {
  final expected = utf8.encode(value);
  final deadline = DateTime.now().add(const Duration(seconds: 3));
  while (DateTime.now().isBefore(deadline)) {
    if (_containsBytes(output, expected)) return;
    await Future<void>.delayed(const Duration(milliseconds: 10));
  }
  throw StateError('The test shell did not echo the draft input.');
}

bool _containsBytes(List<int> bytes, List<int> pattern) {
  if (pattern.isEmpty) return true;
  for (var start = 0; start + pattern.length <= bytes.length; start++) {
    var matches = true;
    for (var offset = 0; offset < pattern.length; offset++) {
      if (bytes[start + offset] != pattern[offset]) {
        matches = false;
        break;
      }
    }
    if (matches) return true;
  }
  return false;
}
