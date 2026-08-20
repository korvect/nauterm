import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ai/ai_terminal_runner.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_models.dart';

void main() {
  test('runner captures output and exit code across output chunks', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: _SuppressingMemoryDriver(columns: 80, rows: 24),
      shellPath: '/bin/zsh',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner();

    final resultFuture = runner.run(
      controller: controller,
      command: 'printf hello',
    );
    final token = RegExp(r'nauterm-integration-ready=([a-f0-9]+)')
        .firstMatch(inputs.single)!
        .group(1)!;
    controller.write('\x1b]777;nauterm-integration-ready=$token\x07');
    await _waitForInput(inputs, '\x18\x1d');
    controller.write('\x1b]777;nauterm-line-ready=$token\x07');
    await Future<void>.delayed(Duration.zero);

    controller.write(
      '\x1b[32mhello\x1b[0m'
      '\x1b]133;C\x1b\\'
      '\x9b31m\x9d0;ignored\x9c'
      '\r\nTOKEN=secret-value',
    );
    controller.write('\x1b]777;nauterm-command-end=$token;7\x07');

    final result = await resultFuture;
    expect(result.exitCode, 7);
    expect(result.succeeded, isFalse);
    expect(result.output, contains('hello'));
    expect(result.output, contains('TOKEN=[REDACTED]'));
    expect(result.output, isNot(contains('\x1b[32m')));
    expect(result.output, isNot(contains('\x1b\\')));
    expect(result.output, isNot(contains('\x9b')));
    expect(result.output, isNot(contains('\x9d')));
  });

  test('runner rejects concurrent commands in the same terminal', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: _SuppressingMemoryDriver(columns: 80, rows: 24),
      shellPath: '/bin/zsh',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner();

    final first = runner.run(controller: controller, command: 'sleep 1');
    await expectLater(
      runner.run(controller: controller, command: 'pwd'),
      throwsStateError,
    );
    final token = RegExp(r'nauterm-integration-ready=([a-f0-9]+)')
        .firstMatch(inputs.single)!
        .group(1)!;
    controller.write('\x1b]777;nauterm-integration-ready=$token\x07');
    await _waitForInput(inputs, '\x18\x1d');
    controller.write('\x1b]777;nauterm-line-ready=$token\x07');
    await Future<void>.delayed(Duration.zero);
    controller.write('\x1b]777;nauterm-command-end=$token;0\x07');
    await first;
  });

  test('runner sends ctrl-c and reports a cancelled command', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: _SuppressingMemoryDriver(columns: 80, rows: 24),
      shellPath: '/bin/zsh',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner();

    final resultFuture = runner.run(
      controller: controller,
      command: 'sleep 30',
    );
    final token = RegExp(r'nauterm-integration-ready=([a-f0-9]+)')
        .firstMatch(inputs.single)!
        .group(1)!;
    controller.write('\x1b]777;nauterm-integration-ready=$token\x07');
    await _waitForInput(inputs, '\x18\x1d');
    controller.write('\x1b]777;nauterm-line-ready=$token\x07');
    await Future<void>.delayed(Duration.zero);
    controller.write('working');

    expect(runner.cancel(controller), isTrue);
    expect(runner.cancel(controller), isFalse);
    expect(inputs.last, '\x03');
    controller.write('\x1b]777;nauterm-command-end=$token;130\x07');

    final result = await resultFuture;
    expect(result.cancelled, isTrue);
    expect(result.succeeded, isFalse);
    expect(result.exitCode, 130);
  });

  test(
    'runner releases a manually interrupted command when no marker arrives',
    () async {
      final inputs = <String>[];
      final controller = TerminalController(
        driver: _SuppressingMemoryDriver(columns: 80, rows: 24),
        shellPath: '/bin/zsh',
        onInput: inputs.add,
      );
      addTearDown(controller.dispose);
      final runner = AiTerminalCommandRunner(
        cancellationGracePeriod: Duration.zero,
      );

      final resultFuture = runner.run(
        controller: controller,
        command: 'sleep 30',
      );
      final token = RegExp(r'nauterm-integration-ready=([a-f0-9]+)')
          .firstMatch(inputs.single)!
          .group(1)!;
      controller.write('\x1b]777;nauterm-integration-ready=$token\x07');
      await _waitForInput(inputs, '\x18\x1d');
      controller.write('\x1b]777;nauterm-line-ready=$token\x07');
      await Future<void>.delayed(Duration.zero);
      controller.sendInput('\x03');

      await expectLater(
        resultFuture,
        throwsA(isA<AiTerminalCommandCancelled>()),
      );
      expect(runner.isBusy(controller), isFalse);
      expect(inputs.last, '\x03');
    },
  );

  test('manual ctrl-c can cancel shell integration setup', () async {
    final controller = TerminalController(
      driver: _SuppressingMemoryDriver(columns: 80, rows: 24),
      shellPath: '/bin/zsh',
      onInput: (_) {},
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner(
      cancellationGracePeriod: Duration.zero,
    );

    final resultFuture = runner.run(controller: controller, command: 'pwd');
    await Future<void>.delayed(Duration.zero);
    controller.sendInput('\x03');

    await expectLater(resultFuture, throwsA(isA<AiTerminalCommandCancelled>()));
    expect(runner.isBusy(controller), isFalse);
  });

  test('failed integration falls back to the original command only', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 24),
      shellPath: '/bin/zsh',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner();

    final result = await runner.run(controller: controller, command: 'pwd');

    expect(result.submitted, isTrue);
    expect(result.exitCode, isNull);
    expect(inputs, ['pwd\r']);
  });

  test('plain sh and dash are not treated as bash', () async {
    for (final shellPath in ['/bin/sh', '/bin/dash']) {
      final inputs = <String>[];
      final controller = TerminalController(
        driver: MemoryTerminalDriver(columns: 80, rows: 24),
        shellPath: shellPath,
        onInput: inputs.add,
      );
      addTearDown(controller.dispose);

      final result = await AiTerminalCommandRunner().run(
        controller: controller,
        command: 'pwd',
      );
      expect(result.submitted, isTrue);
      expect(inputs, ['pwd\r']);
    }
  });

  test(
    'runner completes with an error when the terminal is disposed',
    () async {
      final controller = TerminalController(
        driver: MemoryTerminalDriver(columns: 80, rows: 24),
        onInput: (_) {},
      );
      final runner = AiTerminalCommandRunner();

      final result = runner.run(controller: controller, command: 'sleep 10');
      final expectation = expectLater(result, throwsStateError);
      controller.dispose();

      await expectation;
    },
  );

  test('PowerShell falls back without injecting a wrapper', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 24),
      shellPath: r'C:\Program Files\PowerShell\7\pwsh.exe',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner();

    final result = await runner.run(
      controller: controller,
      command: 'Get-Location',
    );

    expect(result.submitted, isTrue);
    expect(inputs, ['Get-Location\r']);
  });

  test('runner uses fish status syntax for fish sessions', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: _SuppressingMemoryDriver(columns: 80, rows: 24),
      shellPath: '/opt/homebrew/bin/fish',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner();

    final result = runner.run(controller: controller, command: 'pwd');
    final setupToken = RegExp(r'nauterm-integration-ready=([a-f0-9]+)')
        .firstMatch(inputs.single)!
        .group(1)!;
    expect(inputs.single, contains('fish_postexec'));
    expect(inputs.single, contains('fish_preexec'));
    expect(inputs.single, contains('__nauterm_shell_prompt'));
    expect(inputs.single, contains(r'\033]133;A'));
    controller.write('\x1b]777;nauterm-integration-ready=$setupToken\x07');
    await _waitForInput(inputs, '\x18\x1d');
    expect(inputs.last, '\x18\x1d');
    controller.write('\x1b]777;nauterm-line-ready=$setupToken\x07');
    await Future<void>.delayed(Duration.zero);
    expect(inputs.last, 'pwd\r');
    controller.write(
      'fish prompt repaint pwd pwd\r\n'
      '\x1b]4545;CommandStarted;cHdk\x07\x1b]133;C\x07'
      '/tmp'
      '\x1b]777;nauterm-command-end=$setupToken;0\x07',
    );

    final completed = await result;
    expect(completed.exitCode, 0);
    expect(completed.output, '/tmp');
  });

  test('shell integration sends only the original zsh command', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: _SuppressingMemoryDriver(columns: 80, rows: 24),
      shellPath: '/bin/zsh',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner();

    final resultFuture = runner.run(
      controller: controller,
      command: 'printf hello',
    );
    final setupToken = RegExp(r'nauterm-integration-ready=([a-f0-9]+)')
        .firstMatch(inputs.single)!
        .group(1)!;
    controller.write('\x1b]777;nauterm-integration-ready=$setupToken\x07');
    await _waitForInput(inputs, '\x18\x1d');

    expect(inputs.last, '\x18\x1d');
    controller.write('\x1b]777;nauterm-line-ready=$setupToken\x07');
    await Future<void>.delayed(Duration.zero);

    expect(inputs.last, 'printf hello\r');
    controller.write(
      'printf hello\r\nhello\r\n'
      '\x1b]777;nauterm-command-end=$setupToken;0\x07',
    );

    final result = await resultFuture;
    expect(result.exitCode, 0);
    expect(result.output, 'hello');
  });

  test('integrated run removes a command echo split by a soft wrap', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: _SuppressingMemoryDriver(columns: 80, rows: 24),
      shellPath: '/bin/bash',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner();
    const command = r'''printf "${NAUTERM_DRAFT_EXECUTED:-clean}"''';

    final resultFuture = runner.run(controller: controller, command: command);
    final token = RegExp(r'nauterm-integration-ready=([a-f0-9]+)')
        .firstMatch(inputs.single)!
        .group(1)!;
    controller.write('\x1b]777;nauterm-integration-ready=$token\x07');
    await _waitForInput(inputs, '$command\r');
    expect(inputs, containsAllInOrder(['\x18\x1d', '$command\r']));

    controller.write(
      'printf " \${NAUTERM_DRAFT_EXECUTED:-clean}"\r\n'
      'clean'
      '\x1b]777;nauterm-command-end=$token;0\x07',
    );

    final result = await resultFuture;
    expect(result.exitCode, 0);
    expect(result.output, 'clean');
  });

  test(
    'integrated run submits multiline commands as bracketed paste',
    () async {
      final inputs = <String>[];
      final controller = TerminalController(
        driver: _SuppressingMemoryDriver(
          columns: 80,
          rows: 24,
          bracketedPaste: true,
        ),
        shellPath: '/bin/zsh',
        onInput: inputs.add,
      );
      addTearDown(controller.dispose);
      final runner = AiTerminalCommandRunner();
      const command = '''python3 -c "
import socket
print(b'\\r\\n')
" &
sleep 1
nc -zv 127.0.0.1 2323 2>&1''';

      final resultFuture = runner.run(controller: controller, command: command);
      final setupToken = RegExp(r'nauterm-integration-ready=([a-f0-9]+)')
          .firstMatch(inputs.single)!
          .group(1)!;
      controller.write('\x1b]777;nauterm-integration-ready=$setupToken\x07');
      await _waitForInput(inputs, '\x18\x1d');
      expect(inputs.last, '\x18\x1d');
      controller.write('\x1b]777;nauterm-line-ready=$setupToken\x07');
      await Future<void>.delayed(Duration.zero);

      expect(
        inputs.last,
        '\x1b[200~${command.replaceAll('\n', '\r')}\x1b[201~\r',
      );
      controller.write('\x1b]777;nauterm-command-end=$setupToken;0\x07');
      expect((await resultFuture).exitCode, 0);
    },
  );

  test('bash integration installs an idempotent prompt hook', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: _SuppressingMemoryDriver(columns: 80, rows: 24),
      shellPath: '/bin/bash',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner();

    final resultFuture = runner.run(controller: controller, command: 'pwd');
    final setup = inputs.single;
    final setupToken = RegExp(r'nauterm-integration-ready=([a-f0-9]+)')
        .firstMatch(setup)!
        .group(1)!;
    expect(setup, contains('__nauterm_ai_prompt_commands'));
    expect(setup, contains(r'\C-x\C-]":"\C-a\C-k'));
    expect(setup, isNot(contains(r'\C-u')));
    expect(setup, isNot(contains('READLINE_LINE')));
    expect(setup, isNot(contains(r'bind -x')));
    expect(setup, contains(r'\033]7;file://localhost%s'));
    expect(setup, contains(r'\033]133;A'));
    controller.write('\x1b]777;nauterm-integration-ready=$setupToken\x07');
    await _waitForInput(inputs, 'pwd\r');

    expect(inputs, containsAllInOrder(['\x18\x1d', 'pwd\r']));
    controller.write('\x1b]777;nauterm-command-end=$setupToken;0\x1b\\');

    expect((await resultFuture).exitCode, 0);
  });

  test('ksh falls back without installing shell hooks', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 24),
      shellPath: '/bin/ksh',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner();

    final result = await runner.run(controller: controller, command: 'false');

    expect(result.submitted, isTrue);
    expect(inputs, ['false\r']);
  });

  test('tcsh falls back without installing shell hooks', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 24),
      shellPath: '/bin/tcsh',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner();

    final result = await runner.run(controller: controller, command: 'pwd');

    expect(result.submitted, isTrue);
    expect(inputs, ['pwd\r']);
  });

  test('cancelled integrated command reuses integration next time', () async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: _SuppressingMemoryDriver(columns: 80, rows: 24),
      shellPath: '/bin/zsh',
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final runner = AiTerminalCommandRunner(
      cancellationGracePeriod: Duration.zero,
    );

    final first = runner.run(controller: controller, command: 'sleep 30');
    final setupToken = RegExp(r'nauterm-integration-ready=([a-f0-9]+)')
        .firstMatch(inputs.single)!
        .group(1)!;
    controller.write('\x1b]777;nauterm-integration-ready=$setupToken\x07');
    await _waitForInput(inputs, '\x18\x1d');
    expect(inputs.last, '\x18\x1d');
    controller.write('\x1b]777;nauterm-line-ready=$setupToken\x07');
    await Future<void>.delayed(Duration.zero);
    expect(inputs.last, 'sleep 30\r');

    controller.sendInput('\x03');
    await expectLater(first, throwsA(isA<AiTerminalCommandCancelled>()));
    expect(runner.isBusy(controller), isFalse);

    final second = runner.run(controller: controller, command: 'pwd');
    await _waitForInput(inputs, '\x18\x1d', after: inputs.length - 1);
    expect(inputs.last, '\x18\x1d');
    controller.sendInput('\x03');
    await expectLater(second, throwsA(isA<AiTerminalCommandCancelled>()));
  });
}

Future<void> _waitForInput(
  List<String> inputs,
  String expected, {
  int after = -1,
}) async {
  final deadline = DateTime.now().add(const Duration(seconds: 2));
  while (DateTime.now().isBefore(deadline)) {
    for (var index = after + 1; index < inputs.length; index++) {
      if (inputs[index] == expected) {
        return;
      }
    }
    await Future<void>.delayed(const Duration(milliseconds: 5));
  }
  throw StateError('Timed out waiting for terminal input: $expected');
}

class _SuppressingMemoryDriver extends MemoryTerminalDriver {
  _SuppressingMemoryDriver({
    required super.columns,
    required super.rows,
    this.bracketedPaste = false,
  });

  final bool bracketedPaste;

  @override
  TerminalSnapshot get snapshot {
    final value = super.snapshot;
    return TerminalSnapshot(
      columns: value.columns,
      rows: value.rows,
      cells: value.cells,
      cursor: value.cursor,
      keyboardMode: TerminalKeyboardMode(bracketedPaste: bracketedPaste),
      title: value.title,
      inputEchoEnabled: value.inputEchoEnabled,
    );
  }

  @override
  bool suppressOutputUntil(Uint8List marker) => marker.isNotEmpty;

  @override
  bool cancelOutputSuppression() => true;
}
