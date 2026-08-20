import 'dart:io';

// Native PTY integration coverage; intentionally excluded from `flutter test`.

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_ffi.dart';
import 'package:nauterm/terminal/terminal_theme.dart';

void main() {
  test(
    'native wakeup drains delayed output without periodic polling',
    () async {
      late NativeTerminalDriver driver;
      var wakeupCount = 0;
      driver = NativeTerminalDriver.createCommand(
        columns: 80,
        rows: 8,
        config: defaultTerminalConfig,
        onWakeup: () {
          wakeupCount += 1;
          driver.poll();
        },
        program: '/bin/sh',
        args: const [
          '-lc',
          r'sleep 0.1; i=0; while [ $i -lt 4096 ]; do printf 0123456789abcdef0123456789abcdef; i=$((i+1)); done; printf "\nWAKEUP_READY"',
        ],
      );
      addTearDown(driver.dispose);

      final deadline = DateTime.now().add(const Duration(seconds: 5));
      var visibleText = '';
      while (DateTime.now().isBefore(deadline)) {
        visibleText = driver.snapshot.cells.map((cell) => cell.text).join();
        if (visibleText.contains('WAKEUP_READY')) break;
        await Future<void>.delayed(const Duration(milliseconds: 10));
      }

      expect(wakeupCount, greaterThan(1));
      expect(visibleText, contains('WAKEUP_READY'));
    },
    skip: _windowsPtySkipReason,
  );

  test('repeated local ffi pty shutdown completes', () async {
    for (var index = 0; index < 32; index++) {
      final driver = NativeTerminalDriver.createCommand(
        columns: 20,
        rows: 2,
        config: defaultTerminalConfig,
        onWakeup: () {},
        program: '/bin/sh',
        args: const ['-lc', 'cat'],
      );
      driver.dispose();
    }
  }, skip: _windowsPtySkipReason);

  test('local ffi pty reports echo enabled for interactive shell', () async {
    final driver = NativeTerminalDriver.create(
      columns: 80,
      rows: 8,
      config: defaultTerminalConfig,
      onWakeup: () {},
    );
    addTearDown(driver.dispose);

    final deadline = DateTime.now().add(const Duration(seconds: 3));
    while (DateTime.now().isBefore(deadline)) {
      driver.poll();
      final hasVisibleText = driver.snapshot.cells.any(
        (cell) => cell.text.trim().isNotEmpty,
      );
      if (hasVisibleText) {
        break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }

    expect(driver.snapshot.inputEchoEnabled, isTrue);
  }, skip: _windowsPtySkipReason);

  test('native terminal exports the Nysa ANSI palette', () async {
    final driver = NativeTerminalDriver.createCommand(
      columns: 20,
      rows: 2,
      config: defaultTerminalConfig,
      onWakeup: () {},
      program: '/bin/sh',
      args: const ['-lc', r"printf '\033[31mR'"],
      theme: nysaLightTerminalTheme,
    );
    addTearDown(driver.dispose);

    final deadline = DateTime.now().add(const Duration(seconds: 3));
    while (DateTime.now().isBefore(deadline)) {
      driver.poll();
      if (driver.snapshot.cells.any((cell) => cell.text == 'R')) {
        break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }

    final redCell = driver.snapshot.cells.firstWhere(
      (cell) => cell.text == 'R',
    );
    expect(redCell.foreground, nysaLightTerminalTheme.normal.red);
    expect(redCell.background, nysaLightTerminalTheme.primary.background);
  }, skip: _windowsPtySkipReason);
}

String? get _windowsPtySkipReason => Platform.isWindows
    ? 'Native local PTY integration is not implemented on Windows'
    : null;
