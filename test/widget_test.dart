import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/terminal/terminal_selection.dart';
import 'package:nauterm/terminal/terminal_theme.dart';
import 'package:nauterm/terminal/terminal_widget.dart';
import 'package:nauterm/ui/terminal_theme_preview.dart';
import 'package:nauterm/ui/nauterm_context_menu.dart';
import 'package:nauterm/window/native_windowing.dart';

void main() {
  setUp(() async {
    final binding = TestWidgetsFlutterBinding.ensureInitialized();
    binding.platformDispatcher.localeTestValue = const Locale('en');
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
  });
  tearDown(() {
    TestWidgetsFlutterBinding.instance.platformDispatcher
        .clearLocaleTestValue();
  });

  test(
    'mosh keeps prediction until a screen update arrives after confirmation',
    () {
      final driver = _ExitingTerminalDriver();
      final controller = TerminalController.mosh(
        host: 'example.test',
        port: 22,
        username: 'user',
        knownHostsPath: '/tmp/known-hosts',
        driver: driver,
      );

      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshEchoEnabled,
          message: 'echo enabled',
        ),
      );
      controller.poll();
      controller.sendInput('abc');
      expect(controller.moshPrediction, 'abc');

      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshPredictionConfirmed,
          message: 'confirmed',
        ),
      );
      controller.poll();
      expect(controller.moshPrediction, 'abc');

      driver.currentSnapshot = _snapshotWithCursor(
        column: 3,
        row: 0,
        textCells: {(0, 0): 'a', (0, 1): 'b', (0, 2): 'c'},
      );
      driver.events.add(_moshScreenCommittedEvent(1));
      driver.changed = true;
      controller.poll();
      expect(controller.moshPrediction, isEmpty);

      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshNetworkSwitching,
          message: 'switching',
        ),
      );
      controller.poll();
      expect(controller.moshNetworkState, MoshNetworkState.switching);

      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshNetworkRestored,
          message: 'restored',
        ),
      );
      controller.poll();
      expect(controller.moshNetworkState, MoshNetworkState.restored);
      controller.dispose();
    },
  );

  test('mosh prediction requires exact committed screen content', () {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();
    controller.sendInput('abc');

    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshInputStateQueued,
        message: 'state 1',
        stateNum: 1,
      ),
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshPredictionConfirmed,
        message: 'input confirmed',
        stateNum: 1,
      ),
    );
    driver.changed = true;
    controller.poll();
    expect(controller.moshPrediction, 'abc');

    driver.currentSnapshot = _snapshotWithCursor(
      column: 3,
      row: 0,
      textCells: {(0, 0): 'a', (0, 1): 'b', (0, 2): 'c'},
    );
    driver.events.add(_moshScreenCommittedEvent(42));
    driver.changed = true;
    controller.poll();

    expect(controller.moshPrediction, isEmpty);
    expect(controller.debugMoshCommittedScreenStateNum, 42);
    controller.dispose();
  });

  test(
    'mosh prediction stays pending when committed screen does not match',
    () {
      final driver = _ExitingTerminalDriver();
      final controller = TerminalController.mosh(
        host: 'example.test',
        port: 22,
        username: 'user',
        knownHostsPath: '/tmp/known-hosts',
        driver: driver,
      );
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshEchoEnabled,
          message: 'echo enabled',
        ),
      );
      controller.poll();
      controller.sendInput('abc');
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshInputStateQueued,
          message: 'state 1',
          stateNum: 1,
        ),
      );
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshPredictionConfirmed,
          message: 'input confirmed',
          stateNum: 1,
        ),
      );
      driver.currentSnapshot = _snapshotWithCursor(column: 3, row: 0);
      driver.events.add(_moshScreenCommittedEvent(7));
      driver.changed = true;
      controller.poll();

      expect(controller.moshPrediction, isEmpty);
      controller.dispose();
    },
  );

  test('mosh prediction backspace requires cursor and cleared cell', () {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();
    controller.sendInput('a');
    controller.sendInput('');
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshInputStateQueued,
        message: 'state 1',
        stateNum: 1,
      ),
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshPredictionConfirmed,
        message: 'input confirmed',
        stateNum: 1,
      ),
    );
    driver.currentSnapshot = _snapshotWithCursor(column: 0, row: 0);
    driver.events.add(_moshScreenCommittedEvent(8));
    driver.changed = true;
    controller.poll();

    expect(controller.moshPrediction, isEmpty);
    controller.dispose();
  });

  test('mosh prediction does not apply input ACK without screen commit', () {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();
    controller.sendInput('abc');
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshInputStateQueued,
        message: 'state 1',
        stateNum: 1,
      ),
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshPredictionConfirmed,
        message: 'input confirmed',
        stateNum: 1,
      ),
    );
    driver.changed = true;
    controller.poll();

    expect(controller.moshPrediction, 'abc');
    controller.dispose();
  });

  test('old confirmation clears only the confirmed mosh prediction batch', () {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();

    controller.sendInput('abc');
    expect(controller.moshPrediction, 'abc');

    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshPredictionConfirmed,
        message: 'confirmed',
      ),
    );
    controller.poll();
    expect(controller.moshPrediction, 'abc');

    driver.currentSnapshot = _snapshotWithCursor(
      column: 3,
      row: 0,
      textCells: {(0, 0): 'a', (0, 1): 'b', (0, 2): 'c'},
    );
    driver.events.add(_moshScreenCommittedEvent(9));
    driver.changed = true;
    controller.poll();

    controller.sendInput('d');
    expect(controller.moshPrediction, 'd');
    controller.dispose();
  });

  test('Mosh committed snapshot clears only batches through the acknowledged input state', () {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();

    controller.sendInput('abc');
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshInputStateQueued,
        message: 'state 1',
        stateNum: 1,
      ),
    );
    controller.poll();
    controller.sendInput('def');
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshInputStateQueued,
        message: 'state 2',
        stateNum: 2,
      ),
    );
    controller.poll();
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshPredictionConfirmed,
        message: 'state 1 confirmed',
        stateNum: 1,
      ),
    );
    driver.currentSnapshot = _snapshotWithCursor(
      column: 3,
      row: 0,
      textCells: {(0, 0): 'a', (0, 1): 'b', (0, 2): 'c'},
    );
    driver.events.add(_moshScreenCommittedEvent(10));
    driver.changed = true;
    controller.poll();

    expect(controller.moshPrediction, 'def');
    expect(controller.debugMoshPredictionBatches.single.inputStateNum, 2);
    controller.dispose();
  });

  test('rejected mosh input does not local-echo or create prediction', () {
    final driver = _BackpressuredTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );

    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();
    controller.sendInput('abc');

    expect(controller.moshPrediction, isEmpty);
    expect(
      controller.snapshot.cells.any((cell) => cell.text.contains('abc')),
      isFalse,
    );
    controller.dispose();
  });

  test('mosh prediction removes a whole grapheme on backspace', () {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();

    controller.sendInput('你e\u0301🙂');
    expect(controller.moshPrediction, '你e\u0301🙂');
    controller.sendInput('\b');
    expect(controller.moshPrediction, '你e\u0301');
    controller.sendInput('\b');
    expect(controller.moshPrediction, '你');
    controller.dispose();
  });

  test(
    'mosh prediction clears when the unconfirmed overlay exceeds its budget',
    () {
      final driver = _ExitingTerminalDriver();
      final controller = TerminalController.mosh(
        host: 'example.test',
        port: 22,
        username: 'user',
        knownHostsPath: '/tmp/known-hosts',
        driver: driver,
      );
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshEchoEnabled,
          message: 'echo enabled',
        ),
      );
      controller.poll();

      controller.sendInput('a' * 300);
      expect(controller.moshPrediction, isEmpty);
      controller.dispose();
    },
  );

  test('Mosh input ACK without a committed screen snapshot retains every prediction batch', () {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();

    controller.sendInput('abc');
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshInputStateQueued,
        message: 'state 1',
        stateNum: 1,
      ),
    );
    controller.poll();
    controller.sendInput('def');
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshInputStateQueued,
        message: 'state 2',
        stateNum: 2,
      ),
    );
    controller.poll();
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshPredictionConfirmed,
        message: 'state 1 confirmed',
        stateNum: 1,
      ),
    );
    controller.poll();
    driver.changed = true;
    controller.poll();

    expect(controller.moshPrediction, 'abcdef');
    final batches = controller.debugMoshPredictionBatches;
    expect(batches, hasLength(2));
    expect(batches.first.inputAcked, isTrue);
    expect(batches.last.inputAcked, isFalse);
    controller.dispose();
  });

  test('mosh prediction pauses after unsupported control input until screen update', () {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();

    controller.sendInput('ab');
    expect(controller.moshPrediction, 'ab');

    controller.sendInput('\x03');
    expect(controller.moshPrediction, isEmpty);

    controller.sendInput('c');
    expect(controller.moshPrediction, isEmpty);

    driver.changed = true;
    controller.poll();

    controller.sendInput('d');
    expect(controller.moshPrediction, 'd');
    controller.dispose();
  });

  test(
    'mosh prediction batches capture cursor origin and strategy metadata',
    () {
      final driver = _ExitingTerminalDriver();
      driver.currentSnapshot = _snapshotWithCursor(column: 3, row: 4);
      final controller = TerminalController.mosh(
        host: 'example.test',
        port: 22,
        username: 'user',
        knownHostsPath: '/tmp/known-hosts',
        driver: driver,
      );
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshEchoEnabled,
          message: 'echo enabled',
        ),
      );
      controller.poll();

      controller.sendInput('ab');
      driver.currentSnapshot = _snapshotWithCursor(column: 5, row: 4);
      controller.sendInput('\b');

      final batches = controller.debugMoshPredictionBatches;
      expect(batches, hasLength(1));
      expect(batches.single.startColumn, 3);
      expect(batches.single.startRow, 4);
      expect(batches.single.strategy, MoshPredictionStrategy.text);
      expect(batches.single.text, 'a');
      controller.dispose();
    },
  );

  test('mosh prediction batches retain RTT at creation time', () {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshLatencyUpdated,
        message: 'latency 42',
        latencyMs: 42,
      ),
    );
    controller.poll();

    controller.sendInput('a');
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshLatencyUpdated,
        message: 'latency 90',
        latencyMs: 90,
      ),
    );
    controller.poll();
    controller.sendInput('b');

    final batches = controller.debugMoshPredictionBatches;
    expect(batches, hasLength(2));
    expect(batches.first.latencyMs, 42);
    expect(batches.last.latencyMs, 90);
    controller.dispose();
  });

  test(
    'later mosh prediction batches start at the predicted overlay cursor',
    () {
      final driver = _ExitingTerminalDriver();
      driver.currentSnapshot = _snapshotWithCursor(column: 3, row: 4);
      final controller = TerminalController.mosh(
        host: 'example.test',
        port: 22,
        username: 'user',
        knownHostsPath: '/tmp/known-hosts',
        driver: driver,
      );
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshEchoEnabled,
          message: 'echo enabled',
        ),
      );
      controller.poll();

      controller.sendInput('ab');
      controller.sendInput('d');

      final batches = controller.debugMoshPredictionBatches;
      expect(batches, hasLength(2));
      expect(batches.first.startColumn, 3);
      expect(batches.first.startRow, 4);
      expect(batches.first.text, 'ab');
      expect(batches.last.startColumn, 5);
      expect(batches.last.startRow, 4);
      expect(batches.last.text, 'd');
      controller.dispose();
    },
  );

  test('Mosh cursor at the next prediction batch origin does not discard that pending batch', () {
    final driver = _ExitingTerminalDriver();
    driver.currentSnapshot = _snapshotWithCursor(column: 3, row: 4);
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();

    controller.sendInput('ab');
    controller.sendInput('cd');
    expect(controller.moshPrediction, 'abcd');

    driver.currentSnapshot = _snapshotWithCursor(column: 5, row: 4);
    controller.refreshSnapshot();

    expect(controller.moshPrediction, 'abcd');
    final batches = controller.debugMoshPredictionBatches;
    expect(batches, hasLength(2));
    expect(batches.first.text, 'ab');
    expect(batches.last.text, 'cd');
    controller.dispose();
  });

  test(
    'screen content mismatch invalidates only affected mosh prediction batches',
    () {
      final driver = _ExitingTerminalDriver();
      driver.currentSnapshot = _snapshotWithCursor(column: 3, row: 4);
      final controller = TerminalController.mosh(
        host: 'example.test',
        port: 22,
        username: 'user',
        knownHostsPath: '/tmp/known-hosts',
        driver: driver,
      );
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshEchoEnabled,
          message: 'echo enabled',
        ),
      );
      controller.poll();

      controller.sendInput('ab');
      controller.sendInput('cd');
      expect(controller.moshPrediction, 'abcd');

      driver.currentSnapshot = _snapshotWithCursor(
        column: 7,
        row: 4,
        textCells: const {(4, 5): 'X'},
      );
      controller.refreshSnapshot();

      expect(controller.moshPrediction, 'ab');
      final batches = controller.debugMoshPredictionBatches;
      expect(batches, hasLength(1));
      expect(batches.single.text, 'ab');
      expect(batches.single.startColumn, 3);
      expect(batches.single.startRow, 4);
      controller.dispose();
    },
  );

  test('screen content mismatch inside a batch invalidates that batch and later ones', () {
    final driver = _ExitingTerminalDriver();
    driver.currentSnapshot = _snapshotWithCursor(column: 3, row: 4);
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();

    controller.sendInput('ab');
    controller.sendInput('cd');
    expect(controller.moshPrediction, 'abcd');

    driver.currentSnapshot = _snapshotWithCursor(
      column: 7,
      row: 4,
      textCells: const {(4, 5): 'c', (4, 6): 'X'},
    );
    controller.refreshSnapshot();

    expect(controller.moshPrediction, 'ab');
    final batches = controller.debugMoshPredictionBatches;
    expect(batches, hasLength(1));
    expect(batches.single.text, 'ab');
    controller.dispose();
  });

  test('mosh prediction mode never disables local prediction overlay', () {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
      config: defaultTerminalConfig.copyWith(
        moshPredictionMode: TerminalMoshPredictionMode.never,
      ),
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    controller.poll();

    controller.sendInput('abc');
    expect(controller.moshPrediction, isEmpty);
    expect(controller.debugMoshPredictionBatches, isEmpty);
    controller.dispose();
  });

  test(
    'mosh prediction mode always keeps current prediction behavior enabled',
    () {
      final driver = _ExitingTerminalDriver();
      driver.currentSnapshot = _snapshotWithCursor(column: 2, row: 1);
      final controller = TerminalController.mosh(
        host: 'example.test',
        port: 22,
        username: 'user',
        knownHostsPath: '/tmp/known-hosts',
        driver: driver,
        config: defaultTerminalConfig.copyWith(
          moshPredictionMode: TerminalMoshPredictionMode.always,
        ),
      );
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshEchoEnabled,
          message: 'echo enabled',
        ),
      );
      controller.poll();

      controller.sendInput('ab');
      expect(controller.moshPrediction, 'ab');
      final batches = controller.debugMoshPredictionBatches;
      expect(batches, hasLength(1));
      expect(batches.single.startColumn, 2);
      expect(batches.single.startRow, 1);
      controller.dispose();
    },
  );

  test(
    'mosh adaptive prediction pauses on low RTT and resumes on higher RTT',
    () {
      final driver = _ExitingTerminalDriver();
      final controller = TerminalController.mosh(
        host: 'example.test',
        port: 22,
        username: 'user',
        knownHostsPath: '/tmp/known-hosts',
        driver: driver,
      );
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshEchoEnabled,
          message: 'echo enabled',
        ),
      );
      controller.poll();

      controller.sendInput('a');
      expect(controller.moshPrediction, 'a');

      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshLatencyUpdated,
          message: 'latency 30',
          latencyMs: 30,
        ),
      );
      controller.poll();
      expect(controller.debugMoshLatencyMs, 30);

      controller.sendInput('b');
      expect(controller.moshPrediction, 'a');

      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshLatencyUpdated,
          message: 'latency 90',
          latencyMs: 90,
        ),
      );
      controller.poll();
      expect(controller.debugMoshLatencyMs, 90);

      controller.sendInput('c');
      expect(controller.moshPrediction, 'ac');
      controller.dispose();
    },
  );

  test('mosh adaptive prediction can continue on a low RTT line after confirmed same-row history', () {
    final driver = _ExitingTerminalDriver();
    driver.currentSnapshot = _snapshotWithCursor(column: 0, row: 0);
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshLatencyUpdated,
        message: 'latency 90',
        latencyMs: 90,
      ),
    );
    controller.poll();

    controller.sendInput('a');
    expect(controller.moshPrediction, 'a');

    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshPredictionConfirmed,
        message: 'confirmed',
      ),
    );
    driver.currentSnapshot = _snapshotWithCursor(
      column: 1,
      row: 0,
      textCells: {(0, 0): 'a'},
    );
    driver.events.add(_moshScreenCommittedEvent(11));
    driver.changed = true;
    controller.poll();
    expect(controller.moshPrediction, isEmpty);

    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshLatencyUpdated,
        message: 'latency 30',
        latencyMs: 30,
      ),
    );
    controller.poll();

    controller.sendInput('b');
    expect(controller.moshPrediction, 'b');
    controller.dispose();
  });

  test('mosh adaptive low RTT confidence does not survive leaving the confirmed cursor position', () {
    final driver = _ExitingTerminalDriver();
    driver.currentSnapshot = _snapshotWithCursor(column: 0, row: 0);
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshEchoEnabled,
        message: 'echo enabled',
      ),
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshLatencyUpdated,
        message: 'latency 90',
        latencyMs: 90,
      ),
    );
    controller.poll();

    controller.sendInput('a');

    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshPredictionConfirmed,
        message: 'confirmed',
      ),
    );
    driver.currentSnapshot = _snapshotWithCursor(
      column: 0,
      row: 1,
      textCells: {(0, 0): 'a'},
    );
    driver.events.add(_moshScreenCommittedEvent(12));
    driver.changed = true;
    controller.poll();

    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshLatencyUpdated,
        message: 'latency 30',
        latencyMs: 30,
      ),
    );
    controller.poll();

    controller.sendInput('b');
    expect(controller.moshPrediction, isEmpty);
    controller.dispose();
  });

  testWidgets('mosh network transitions are shown inside the terminal', (
    tester,
  ) async {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController.mosh(
      host: 'example.test',
      port: 22,
      username: 'user',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    await tester.pumpWidget(
      MaterialApp(
        home: SizedBox(
          width: 640,
          height: 360,
          child: TerminalWidget(controller: controller),
        ),
      ),
    );

    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshNetworkDegraded,
        message: 'degraded',
      ),
    );
    controller.poll();
    await tester.pump();
    expect(find.text('Connection interrupted · input queued'), findsOneWidget);

    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.moshNetworkRestored,
        message: 'restored',
      ),
    );
    controller.poll();
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text('Connection restored'), findsOneWidget);
    controller.dispose();
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('read-only terminal ignores keyboard input', (tester) async {
    final driver = _ExitingTerminalDriver();
    final controller = TerminalController(driver: driver);
    await tester.pumpWidget(
      MaterialApp(
        home: SizedBox(
          width: 640,
          height: 360,
          child: TerminalWidget(controller: controller, readOnly: true),
        ),
      ),
    );

    await tester.tap(find.byType(TerminalWidget));
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);

    expect(driver.inputs, isEmpty);
    controller.dispose();
    await tester.pumpWidget(const SizedBox.shrink());
  });

  test('terminal controller notifies exit after driver exits', () {
    final driver = _ExitingTerminalDriver();
    var exitCount = 0;
    final controller = TerminalController(
      driver: driver,
      onExit: () => exitCount++,
    );

    driver.exited = true;
    driver.changed = true;

    expect(controller.poll(), isTrue);
    expect(exitCount, 1);

    driver.changed = false;

    expect(controller.poll(), isFalse);
    expect(exitCount, 1);

    controller.dispose();
  });

  test('terminal controller marks explicit shell exit for tab close', () {
    final driver = _ExitingTerminalDriver();
    var exitCount = 0;
    final controller = TerminalController(
      driver: driver,
      onExit: () => exitCount++,
    );

    controller.sendInput('exit');
    expect(controller.shouldCloseOnExit, isFalse);

    controller.sendInput('\r');
    expect(controller.shouldCloseOnExit, isTrue);

    driver.exited = true;
    driver.changed = true;

    expect(controller.poll(), isTrue);
    expect(exitCount, 1);
    expect(controller.shouldCloseOnExit, isTrue);

    controller.dispose();
  });

  test(
    'terminal controller clears stale explicit exit intent on more input',
    () {
      final driver = _ExitingTerminalDriver();
      final controller = TerminalController(driver: driver);

      controller.sendInput('exit\r');
      expect(controller.shouldCloseOnExit, isTrue);

      controller.sendInput('pwd');
      expect(controller.shouldCloseOnExit, isFalse);

      controller.dispose();
    },
  );

  testWidgets('terminal workspace renders', (WidgetTester tester) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    expect(find.text('New host'), findsOneWidget);
    expect(
      find.ancestor(
        of: find.byKey(const ValueKey('workspace-top-page:vaults')),
        matching: find.byType(AnimatedSwitcher),
      ),
      findsNothing,
    );
    expect(find.text('Local Terminal'), findsNothing);

    await _openTraditionalLocalTerminal(tester);

    expect(find.byType(TerminalView), findsOneWidget);

    await tester.tap(find.byType(TerminalView));
    await tester.pump();

    await tester.tap(find.byIcon(Icons.folder_rounded).first);
    await tester.pump();

    expect(find.byType(TerminalView), findsNothing);
    expect(find.text('Connect to host'), findsOneWidget);
    expect(find.text('Select host'), findsOneWidget);

    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit3);
    await tester.tap(find.text('Local Terminal').first);
    await tester.pump();

    expect(find.byType(TerminalView), findsOneWidget);

    await tester.tap(find.byIcon(Icons.close_rounded).first);
    await tester.pump();
    final closeConfirmation = find.text('Close');
    if (closeConfirmation.evaluate().isNotEmpty) {
      await tester.tap(closeConfirmation.last);
      await tester.pump();
    }

    expect(find.byType(TerminalView), findsNothing);
    expect(find.text('Local Terminal'), findsNothing);
    expect(find.text('New host'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('workspace section title follows language changes', (
    WidgetTester tester,
  ) async {
    final english = NautermLocalizations.current;
    await tester.runAsync(
      () => NautermLocalizations.load(const Locale('zh', 'CN')),
    );
    addTearDown(() {
      setAppLanguage(AppLanguage.english);
      NautermLocalizations.current = english;
    });

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('Known Hosts'));
    await tester.pump();

    final sectionTitle = find.byKey(
      const ValueKey('workspace-section-title:Known Hosts'),
    );
    expect(tester.widget<Text>(sectionTitle).data, 'Known Hosts');

    setAppLanguage(AppLanguage.simplifiedChinese);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));

    expect(tester.widget<Text>(sectionTitle).data, '已知主机');

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('workspace chrome renders in Simplified Chinese', (
    WidgetTester tester,
  ) async {
    setAppLanguage(AppLanguage.simplifiedChinese);
    await tester.runAsync(() async {
      NautermLocalizations.current = await NautermLocalizations.load(
        const Locale('zh', 'CN'),
      );
    });
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 50)),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));

    expect(find.text('新建主机'), findsOneWidget);
    expect(find.text('终端'), findsOneWidget);
    expect(find.text('串口'), findsOneWidget);
    expect(find.text('密钥链'), findsOneWidget);
    expect(find.text('端口转发'), findsOneWidget);
    expect(find.text('已知主机'), findsOneWidget);
  });

  testWidgets('terminal tools panel slides with the terminal boundary', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await _openTraditionalLocalTerminal(tester);

    final workspaceBody = find.byKey(const ValueKey('nauterm-workspace-body'));
    final bodyBefore = tester.getRect(workspaceBody);

    await tester.tap(find.byTooltip('Terminal tools'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 90));

    final toolsRegion = find.byKey(const ValueKey('terminal-tools-region'));
    final bodyDuringOpen = tester.getRect(workspaceBody);
    final toolsDuringOpen = tester.getRect(toolsRegion);
    expect(bodyDuringOpen.right, lessThan(bodyBefore.right));
    expect(bodyDuringOpen.right, greaterThan(bodyBefore.right - 360));
    expect(bodyDuringOpen.right, closeTo(toolsDuringOpen.left, 0.1));

    await tester.pumpAndSettle();
    final bodyOpen = tester.getRect(workspaceBody);
    expect(bodyOpen.right, lessThan(bodyDuringOpen.right));

    await tester.tap(find.byTooltip('Terminal tools'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 90));

    final bodyDuringClose = tester.getRect(workspaceBody);
    final toolsDuringClose = tester.getRect(toolsRegion);
    expect(bodyDuringClose.right, greaterThan(bodyOpen.right));
    expect(bodyDuringClose.right, lessThan(bodyBefore.right));
    expect(bodyDuringClose.right, closeTo(toolsDuringClose.left, 0.1));

    await tester.pumpAndSettle();
    expect(tester.getRect(workspaceBody), bodyBefore);
    expect(toolsRegion, findsNothing);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('AI Assistant opens only for the selected terminal page', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    expect(find.byTooltip('AI Assistant'), findsNothing);
    expect(find.byTooltip('Terminal tools'), findsNothing);

    await _openTraditionalLocalTerminal(tester);
    expect(find.byTooltip('Terminal tools'), findsOneWidget);

    await tester.tap(find.byTooltip('AI Assistant'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    for (final tooltip in const ['Remove selection', 'Remove recent output']) {
      final removeContext = find.byTooltip(tooltip);
      if (removeContext.evaluate().isNotEmpty) {
        await tester.tap(removeContext.first);
        await tester.pump();
      }
    }

    expect(find.text('Start a conversation'), findsOneWidget);
    expect(find.byKey(const ValueKey('ai-assistant-composer')), findsOneWidget);
    expect(
      find.byKey(const ValueKey('ai-context-terminalSelection')),
      findsNothing,
    );
    expect(find.byKey(const ValueKey('ai-context-recentOutput')), findsNothing);
    final panel = tester.widget<Material>(
      find.byKey(const ValueKey('ai-assistant-panel')),
    );
    expect(
      panel.color,
      Color.lerp(
        defaultTerminalTheme.primary.background,
        defaultTerminalTheme.primary.foreground,
        0.04,
      ),
    );
    expect(panel.elevation, 0);
    expect(panel.shape, isA<RoundedRectangleBorder>());
    expect(
      tester.getSize(find.byKey(const ValueKey('terminal-tool-mode:ai'))),
      const Size.square(26),
    );
    final composerSurface = tester.widget<Container>(
      find.byKey(const ValueKey('ai-assistant-composer-surface')),
    );
    expect(
      tester
          .getSize(find.byKey(const ValueKey('ai-assistant-composer-surface')))
          .height,
      lessThan(109),
    );
    final composerDecoration = composerSurface.decoration! as BoxDecoration;
    expect(composerDecoration.borderRadius, BorderRadius.circular(18));
    expect(composerDecoration.boxShadow, isNotEmpty);
    final providerControls = find.byKey(
      const ValueKey('ai-composer-provider-controls'),
    );
    if (providerControls.evaluate().isNotEmpty) {
      expect(
        find.descendant(
          of: find.byKey(const ValueKey('ai-assistant-composer-surface')),
          matching: providerControls,
        ),
        findsOneWidget,
      );
      final providerModelLabel = find.byKey(
        const ValueKey('ai-provider-model-label'),
      );
      expect(providerModelLabel, findsOneWidget);
      expect(
        find.descendant(
          of: providerControls,
          matching: find.byWidgetPredicate(
            (widget) =>
                widget is Icon &&
                (widget.icon == LucideIcons.chevronDown ||
                    widget.icon == LucideIcons.chevronUp),
          ),
        ),
        findsOneWidget,
      );
      final providerRect = tester.getRect(providerControls);
      final sendRect = tester.getRect(
        find
            .descendant(
              of: find.byKey(const ValueKey('ai-assistant-composer-surface')),
              matching: find.byTooltip('Send'),
            )
            .first,
      );
      expect(sendRect.left - providerRect.right, greaterThanOrEqualTo(8));
      final composerRect = tester.getRect(
        find.byKey(const ValueKey('ai-assistant-composer-surface')),
      );
      final footerRect = tester.getRect(
        find.byKey(const ValueKey('ai-assistant-composer-footer')),
      );
      expect(composerRect.bottom - footerRect.bottom, lessThanOrEqualTo(1.1));
      await tester.tap(providerModelLabel);
      await tester.pump(const Duration(milliseconds: 120));
      expect(
        find.byKey(const ValueKey('ai-provider-model-menu')),
        findsOneWidget,
      );
      final providerMenu = find.byKey(const ValueKey('ai-provider-model-menu'));
      expect(tester.getSize(providerMenu).width, lessThanOrEqualTo(320));
      final selectedCheck = find.descendant(
        of: providerMenu,
        matching: find.byIcon(LucideIcons.check),
      );
      expect(selectedCheck, findsOneWidget);
      expect(
        tester.getRect(providerMenu).right -
            tester.getRect(selectedCheck).right,
        lessThanOrEqualTo(17),
      );
      await tester.tapAt(const Offset(8, 8));
      await tester.pump();
    }
    final toolbarRect = tester.getRect(
      find.byKey(const ValueKey('terminal-view-toolbar')),
    );
    final rendererRect = tester.getRect(
      find.byKey(const ValueKey('terminal-renderer-region')),
    );
    final toolsRect = tester.getRect(
      find.byKey(const ValueKey('terminal-tools-region')),
    );
    expect(toolbarRect.bottom, rendererRect.top);
    expect(toolbarRect.top, toolsRect.top);
    expect(rendererRect.right, toolsRect.left);
    expect(toolbarRect.left, rendererRect.left);
    expect(toolbarRect.right, toolsRect.left);

    await tester.tap(find.byTooltip('Split right'));
    await tester.pump();
    expect(find.byType(TerminalView), findsNWidgets(2));
    expect(find.byKey(const ValueKey('terminal-tools-region')), findsOneWidget);
    final splitToolsRect = tester.getRect(
      find.byKey(const ValueKey('terminal-tools-region')),
    );
    for (final terminal in tester.widgetList<TerminalView>(
      find.byType(TerminalView),
    )) {
      final terminalRect = tester.getRect(find.byWidget(terminal));
      expect(terminalRect.right, lessThanOrEqualTo(splitToolsRect.left));
    }

    for (final mode in [
      'ai',
      'sftp',
      'systemInfo',
      'snippets',
      'shellHistory',
      'settings',
    ]) {
      expect(find.byKey(ValueKey('terminal-tool-mode:$mode')), findsOneWidget);
    }

    await tester.tap(find.byKey(const ValueKey('terminal-tool-mode:sftp')));
    await tester.pump();
    expect(
      find.byKey(const ValueKey('terminal-sftp-tool-panel')),
      findsOneWidget,
    );
    expect(find.text('No SSH session'), findsOneWidget);

    await tester.tap(
      find.byKey(const ValueKey('terminal-tool-mode:systemInfo')),
    );
    await tester.pump();
    expect(
      find.byKey(const ValueKey('terminal-system-info-panel')),
      findsOneWidget,
    );
    expect(find.text('No SSH session'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('terminal-tool-mode:settings')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(
      find.byKey(const ValueKey('terminal-theme-gallery')),
      findsOneWidget,
    );
    expect(find.byKey(const ValueKey('terminal-font-family')), findsOneWidget);
    expect(find.byKey(const ValueKey('terminal-font-size')), findsOneWidget);
    final terminalFontRect = tester.getRect(
      find.byKey(const ValueKey('terminal-font-family')),
    );
    for (final x in [terminalFontRect.right - 17, terminalFontRect.right - 2]) {
      await tester.tapAt(Offset(x, terminalFontRect.center.dy));
      await tester.pump(const Duration(milliseconds: 150));
      expect(
        find.byKey(const ValueKey('terminal-tool-dropdown-menu')),
        findsOneWidget,
      );
      await tester.tapAt(const Offset(4, 4));
      await tester.pumpAndSettle();
      expect(
        find.byKey(const ValueKey('terminal-tool-dropdown-menu')),
        findsNothing,
      );
    }
    await tester.tap(find.byKey(const ValueKey('terminal-font-family')));
    await tester.pump(const Duration(milliseconds: 150));
    await tester.tap(find.text('Menlo').last);
    await tester.pump();
    expect(
      tester
          .widgetList<TerminalView>(find.byType(TerminalView))
          .every((terminal) => terminal.config?.font.family == 'Menlo'),
      isTrue,
    );
    await tester.tap(find.byKey(const ValueKey('terminal-font-family')));
    await tester.pump(const Duration(milliseconds: 150));
    await tester.enterText(
      find.byKey(const ValueKey('terminal-tool-dropdown-input')),
      'MesloLGS NF Custom',
    );
    await tester.pump();
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('terminal-tool-dropdown-menu')),
        matching: find.text('MesloLGS NF Custom'),
      ),
      findsOneWidget,
    );
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(
      tester
          .widgetList<TerminalView>(find.byType(TerminalView))
          .every(
            (terminal) => terminal.config?.font.family == 'MesloLGS NF Custom',
          ),
      isTrue,
    );
    await tester.tap(find.byKey(const ValueKey('terminal-font-family')));
    await tester.pump(const Duration(milliseconds: 150));
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('terminal-tool-dropdown-menu')),
        matching: find.text('Menlo'),
      ),
      findsOneWidget,
    );
    await tester.enterText(
      find.byKey(const ValueKey('terminal-tool-dropdown-input')),
      'Cascadia',
    );
    await tester.pump();
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('terminal-tool-dropdown-menu')),
        matching: find.text('Menlo'),
      ),
      findsNothing,
    );
    await tester.tapAt(const Offset(4, 4));
    await tester.pump();
    expect(
      tester
          .widget<TextField>(
            find.byKey(const ValueKey('terminal-tool-dropdown-input')),
          )
          .controller
          ?.text,
      'MesloLGS NF Custom',
    );
    await tester.tap(find.byKey(const ValueKey('terminal-font-size')));
    await tester.pump(const Duration(milliseconds: 150));
    expect(
      find.byKey(const ValueKey('terminal-tool-dropdown-menu-scroll')),
      findsOneWidget,
    );
    expect(
      tester
          .getSize(find.byKey(const ValueKey('terminal-tool-dropdown-menu')))
          .height,
      lessThanOrEqualTo(238),
    );
    await tester.tap(find.text('14').last);
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('terminal-font-size')));
    await tester.pump(const Duration(milliseconds: 150));
    final sizeMenuScrollable = tester.state<ScrollableState>(
      find
          .descendant(
            of: find.byKey(
              const ValueKey('terminal-tool-dropdown-menu-scroll'),
            ),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    expect(sizeMenuScrollable.position.pixels, greaterThan(0));
    await tester.tap(find.text('14').last);
    await tester.pump(const Duration(milliseconds: 300));
    expect(
      tester
          .widgetList<TerminalView>(find.byType(TerminalView))
          .every((terminal) => terminal.config?.font.size == 14),
      isTrue,
    );
    expect(find.byType(TerminalThemePreviewCard), findsWidgets);
    final themeOptions = find.byWidgetPredicate(
      (widget) =>
          widget.key is ValueKey<String> &&
          (widget.key! as ValueKey<String>).value.startsWith(
            'terminal-theme-option:',
          ),
    );
    expect(themeOptions, findsWidgets);
    final selectedPreview = tester.widget<TerminalThemePreviewCard>(
      find
          .descendant(
            of: themeOptions.last,
            matching: find.byType(TerminalThemePreviewCard),
          )
          .first,
    );
    await tester.ensureVisible(themeOptions.last);
    await tester.tap(themeOptions.last);
    await tester.pump();
    expect(
      tester
          .widgetList<TerminalView>(find.byType(TerminalView))
          .every(
            (terminal) => terminal.theme.name == selectedPreview.theme.name,
          ),
      isTrue,
    );

    await tester.tap(find.byKey(const ValueKey('terminal-tool-mode:ai')));
    await tester.pump();
    await tester.enterText(
      find.byKey(const ValueKey('ai-assistant-composer')),
      'draft question',
    );

    await tester.tap(find.byKey(const ValueKey('terminal-tool-mode:snippets')));
    await tester.pump();
    expect(
      find.byKey(const ValueKey('terminal-snippets-panel')),
      findsOneWidget,
    );
    expect(find.text('New snippet'), findsOneWidget);
    expect(find.byKey(const ValueKey('ai-assistant-composer')), findsNothing);

    await tester.tap(
      find.byKey(const ValueKey('terminal-tool-mode:shellHistory')),
    );
    await tester.pump();
    expect(
      find.byKey(const ValueKey('terminal-shell-history-panel')),
      findsOneWidget,
    );

    await tester.tap(find.byKey(const ValueKey('terminal-tool-mode:snippets')));
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('terminal-tool-mode:ai')));
    await tester.pump();
    expect(
      tester
          .widget<TextField>(
            find.byKey(const ValueKey('ai-assistant-composer')),
          )
          .controller!
          .text,
      'draft question',
    );
    await tester.tap(find.byKey(const ValueKey('terminal-tool-mode:snippets')));
    await tester.pump();

    await tester.tap(find.byTooltip('Terminal tools'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('ai-assistant-panel')), findsNothing);

    await tester.tap(find.byTooltip('Terminal tools'));
    await tester.pump();
    expect(
      find.byKey(const ValueKey('terminal-snippets-panel')),
      findsOneWidget,
    );

    await tester.tap(find.byTooltip('AI Assistant'));
    await tester.pump();
    expect(find.byKey(const ValueKey('ai-assistant-composer')), findsOneWidget);

    expect(find.byTooltip('Close AI Assistant'), findsNothing);
    await tester.tap(find.byTooltip('AI Assistant'));
    await tester.pumpAndSettle();
    expect(find.text('Start a conversation'), findsNothing);

    await tester.tap(find.byTooltip('AI Assistant'));
    await tester.pump();
    expect(find.text('Start a conversation'), findsOneWidget);

    await tester.tap(find.byTooltip('Quick Connect'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    final shellButtons = find.descendant(
      of: find.byType(GridView),
      matching: find.byType(Tooltip),
    );
    await tester.tap(shellButtons.first);
    await tester.pump(const Duration(milliseconds: 200));
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('Start a conversation'), findsNothing);

    await tester.tap(_terminalTopTabs().first);
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('Start a conversation'), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('AI Assistant state is isolated for each workspace', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('Default').first);
    await tester.pump();

    await tester.tap(find.byTooltip('AI Assistant'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text('Start a conversation'), findsOneWidget);
    expect(
      tester
          .widget<Material>(find.byKey(const ValueKey('ai-assistant-panel')))
          .color,
      Colors.white,
    );
    final overviewFeedback = tester.widget<AnimatedContainer>(
      find.byKey(const ValueKey('ai-assistant-resize-feedback')),
    );
    final overviewDecoration = overviewFeedback.decoration! as BoxDecoration;
    final overviewBorder = overviewDecoration.border! as Border;
    expect(overviewBorder.left.color, isNot(Colors.transparent));
    expect(overviewBorder.top.style, BorderStyle.none);

    await tester.tap(find.text('New workspace'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    final shellButtons = find.descendant(
      of: find.byType(GridView),
      matching: find.byType(Tooltip),
    );
    await tester.tap(shellButtons.first);
    await tester.pumpAndSettle();

    expect(find.text('Start a conversation'), findsNothing);
    expect(find.byTooltip('AI Assistant'), findsOneWidget);

    await tester.tap(find.text('Default').first);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('Start a conversation'), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('AI Assistant panel can be resized from its left edge', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await _openTraditionalLocalTerminal(tester);
    await tester.tap(find.byTooltip('AI Assistant'));
    await tester.pumpAndSettle();

    final panel = find.byKey(const ValueKey('ai-assistant-panel'));
    final handle = find.byKey(const ValueKey('ai-assistant-resize-handle'));
    final initialWidth = tester.getSize(panel).width;
    expect(handle, findsOneWidget);
    expect(
      find.byKey(const ValueKey('ai-assistant-add-attachment')),
      findsOneWidget,
    );
    final feedback = find.byKey(const ValueKey('ai-assistant-resize-feedback'));
    final idleFeedbackDecoration =
        tester.widget<AnimatedContainer>(feedback).decoration as BoxDecoration;
    final idleFeedbackBorder = idleFeedbackDecoration.border! as Border;
    expect(idleFeedbackBorder.left.color, Colors.transparent);
    expect(idleFeedbackBorder.top.color, Colors.transparent);

    await tester.drag(handle, const Offset(60, 0));
    await tester.pumpAndSettle();
    final resizedWidth = tester.getSize(panel).width;
    expect(resizedWidth, lessThan(initialWidth));

    await tester.drag(handle, const Offset(-40, 0));
    await tester.pumpAndSettle();
    expect(tester.getSize(panel).width, greaterThan(resizedWidth));
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('composer visibility is remembered by its terminal split', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await _openTraditionalLocalTerminal(tester);
    await tester.pump(const Duration(milliseconds: 200));

    await tester.tap(find.byTooltip('Hide Composer'));
    await tester.pump();
    expect(
      find.byKey(const ValueKey('terminal-composer-surface')),
      findsNothing,
    );

    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit1);
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.byType(TerminalView), findsNothing);

    await tester.tap(find.text('Local Terminal').first);
    await tester.pump(const Duration(milliseconds: 200));
    expect(
      find.byKey(const ValueKey('terminal-composer-surface')),
      findsNothing,
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('empty port forwarding page uses the shared empty state', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await tester.tap(find.text('Port Forwarding'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('No forwarding rules yet'), findsOneWidget);
    expect(find.text('Add forwarding'), findsOneWidget);
    expect(find.text('Port Forwarding'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('empty logs page shows an empty state in the table body', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await tester.tap(find.text('Logs'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));

    final tableBody = find.byKey(const ValueKey('logs-table-body'));
    final emptyState = find.byKey(const ValueKey('logs-table-empty-state'));
    expect(tableBody, findsOneWidget);
    expect(emptyState, findsOneWidget);
    expect(
      find.descendant(of: tableBody, matching: emptyState),
      findsOneWidget,
    );
    expect(find.text('No terminal sessions recorded yet.'), findsOneWidget);
    for (final label in const ['Date', 'User', 'Host', 'Actions']) {
      expect(find.text(label), findsOneWidget);
    }

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('settings button invokes opener', (WidgetTester tester) async {
    var openedSettings = false;

    await tester.pumpWidget(
      NautermApp(
        onOpenSettings: () {
          openedSettings = true;
        },
      ),
    );

    await tester.tap(find.byKey(const ValueKey('sidebar-settings-button')));
    await tester.pump();

    expect(openedSettings, isTrue);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('double clicking top bar toggles window maximize', (
    WidgetTester tester,
  ) async {
    var toggleCount = 0;
    fullscreenNotifier.value = false;

    await tester.pumpWidget(
      NautermApp(
        onOpenSettings: () {},
        onStartWindowDrag: () {},
        onToggleWindowMaximized: () => toggleCount++,
      ),
    );

    await _doubleClickWithMouse(tester, const Offset(700, 22));

    expect(toggleCount, 1);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('double clicking a top bar tab does not drag or maximize', (
    WidgetTester tester,
  ) async {
    var dragCount = 0;
    var toggleCount = 0;
    fullscreenNotifier.value = false;

    await tester.pumpWidget(
      NautermApp(
        onOpenSettings: () {},
        onStartWindowDrag: () => dragCount++,
        onToggleWindowMaximized: () => toggleCount++,
      ),
    );

    final vaultsTab = find
        .ancestor(
          of: find.text('Vaults'),
          matching: find.byType(GestureDetector),
        )
        .first;
    final tabRect = tester.getRect(vaultsTab);
    await _doubleClickWithMouse(tester, Offset(tabRect.center.dx, 2));

    expect(dragCount, 0);
    expect(toggleCount, 0);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('double clicking top bar button regions does not maximize', (
    WidgetTester tester,
  ) async {
    var dragCount = 0;
    var toggleCount = 0;
    fullscreenNotifier.value = false;

    await tester.pumpWidget(
      NautermApp(
        onOpenSettings: () {},
        onStartWindowDrag: () => dragCount++,
        onToggleWindowMaximized: () => toggleCount++,
      ),
    );

    await _openTraditionalLocalTerminal(tester);
    for (final tooltip in const [
      'Quick Connect',
      'AI Assistant',
      'Terminal tools',
    ]) {
      final buttonRect = tester.getRect(find.byTooltip(tooltip));
      await _doubleClickWithMouse(tester, Offset(buttonRect.center.dx, 2));
    }

    expect(dragCount, 0);
    expect(toggleCount, 0);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('terminal top bar buttons expose clear interaction states', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await _openTraditionalLocalTerminal(tester);

    final quickConnectButton = find
        .ancestor(
          of: find.byTooltip('Quick Connect'),
          matching: find.byType(IconButton),
        )
        .first;
    final quickConnect = tester.widget<IconButton>(quickConnectButton);
    final interactionOverlay = quickConnect.style!.overlayColor!;
    final idle = interactionOverlay.resolve(<WidgetState>{});
    final hovered = interactionOverlay.resolve(<WidgetState>{
      WidgetState.hovered,
    });
    final pressed = interactionOverlay.resolve(<WidgetState>{
      WidgetState.pressed,
    });

    expect(idle, isNull);
    expect(hovered, isNot(idle));
    expect(pressed, isNot(hovered));

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('right top bar blank area can drag and maximize the window', (
    WidgetTester tester,
  ) async {
    var dragCount = 0;
    var toggleCount = 0;
    fullscreenNotifier.value = false;

    await tester.pumpWidget(
      NautermApp(
        onOpenSettings: () {},
        onStartWindowDrag: () => dragCount++,
        onToggleWindowMaximized: () => toggleCount++,
      ),
    );

    await _doubleClickWithMouse(tester, const Offset(797, 22));

    expect(dragCount, 1);
    expect(toggleCount, 1);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('fullscreen top bar cannot drag or maximize the window', (
    WidgetTester tester,
  ) async {
    var dragCount = 0;
    var toggleCount = 0;
    fullscreenNotifier.value = true;
    addTearDown(() => fullscreenNotifier.value = false);

    await tester.pumpWidget(
      NautermApp(
        onOpenSettings: () {},
        onStartWindowDrag: () => dragCount++,
        onToggleWindowMaximized: () => toggleCount++,
      ),
    );

    await _doubleClickWithMouse(tester, const Offset(700, 22));

    expect(dragCount, 0);
    expect(toggleCount, 0);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('file drop is enabled only while SFTP page is visible', (
    WidgetTester tester,
  ) async {
    const channel = MethodChannel('com.korvect.nauterm/file_drop');
    final enabledStates = <bool>[];
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
          if (call.method == 'setEnabled') {
            final arguments = call.arguments as Map<Object?, Object?>;
            enabledStates.add(arguments['enabled'] as bool);
          }
          return null;
        });
    addTearDown(() {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null);
    });

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit2);
    await tester.pump(const Duration(milliseconds: 200));

    expect(enabledStates, contains(true));

    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit1);
    await tester.pump(const Duration(milliseconds: 200));

    expect(enabledStates.last, isFalse);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('SFTP page keeps state when switching away', (
    WidgetTester tester,
  ) async {
    const channel = MethodChannel('com.korvect.nauterm/file_drop');
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (_) async => null);
    addTearDown(() {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null);
    });

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit2);
    await tester.pump(const Duration(milliseconds: 200));

    await tester.tap(find.text('Filter').first);
    await tester.pump();
    await tester.enterText(find.byType(TextField).first, 'needle');
    await tester.pump();

    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit1);
    await tester.pump(const Duration(milliseconds: 200));
    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit2);
    await tester.pump(const Duration(milliseconds: 200));

    final filter = tester.widget<TextField>(find.byType(TextField).first);
    expect(filter.controller?.text, 'needle');

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('open SFTP actions menu follows language changes', (
    WidgetTester tester,
  ) async {
    tester.view.physicalSize = const Size(1600, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    const channel = MethodChannel('com.korvect.nauterm/file_drop');
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (_) async => null);
    addTearDown(() {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null);
    });

    final english = NautermLocalizations.current;
    await tester.runAsync(
      () => NautermLocalizations.load(const Locale('zh', 'CN')),
    );
    setAppLanguage(AppLanguage.simplifiedChinese);
    addTearDown(() {
      setAppLanguage(AppLanguage.english);
      NautermLocalizations.current = english;
    });

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit2);
    await tester.pump(const Duration(milliseconds: 200));
    await tester.tap(find.byKey(const ValueKey('sftp-empty-use-local-button')));
    await tester.pump(const Duration(milliseconds: 200));

    await tester.tap(find.text('操作').first);
    await tester.pump();
    expect(find.text('新建文件夹'), findsOneWidget);

    setAppLanguage(AppLanguage.english);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('New Folder'), findsOneWidget);
    expect(find.text('新建文件夹'), findsNothing);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('SFTP local panes keep independent state', (
    WidgetTester tester,
  ) async {
    tester.view.physicalSize = const Size(1600, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    const channel = MethodChannel('com.korvect.nauterm/file_drop');
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (_) async => null);
    addTearDown(() {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, null);
    });

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit2);
    await tester.pump(const Duration(milliseconds: 200));

    await tester.tap(find.byKey(const ValueKey('sftp-empty-use-local-button')));
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('Connect to host'), findsNothing);
    expect(find.byTooltip('Tasks'), findsNothing);
    expect(find.byTooltip('Favorite current path'), findsNothing);
    expect(find.byTooltip('Show favorite paths'), findsNothing);
    expect(
      find.byKey(const ValueKey('sftp-transfer-target:left')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('sftp-transfer-target:right')),
      findsOneWidget,
    );
    final leftPane = find.byKey(const ValueKey('sftp-transfer-target:left'));
    final leftHome = find.descendant(
      of: leftPane,
      matching: find.byTooltip('Home'),
    );
    final leftBack = find.descendant(
      of: leftPane,
      matching: find.byTooltip('Back'),
    );
    expect(leftHome, findsOneWidget);
    expect(leftBack, findsOneWidget);
    expect(
      tester.getCenter(leftHome).dx,
      lessThan(tester.getCenter(leftBack).dx),
    );
    if (Platform.isWindows) {
      final driveSelectors = find.byKey(
        ValueKey('sftp-local-drive-selector:C:${Platform.pathSeparator}'),
      );
      expect(driveSelectors, findsNWidgets(2));
      tester.widget<InkWell>(driveSelectors.first).onTap!();
      await tester.pump();
      expect(find.byType(NautermDropdownSurface), findsOneWidget);
      expect(
        tester.getSize(find.byType(NautermDropdownSurface)).width,
        greaterThanOrEqualTo(180),
      );
      final cDriveOption = find
          .descendant(
            of: find.byType(NautermDropdownSurface),
            matching: find.text('C:${Platform.pathSeparator}'),
          )
          .first;
      expect(cDriveOption, findsOneWidget);
      await tester.tap(cDriveOption);
      await tester.pump(const Duration(milliseconds: 300));
      expect(find.byType(NautermDropdownSurface), findsNothing);
    }
    await tester.tap(find.text('Filter').first);
    await tester.pump();
    await tester.enterText(find.byType(TextField).first, 'left-only');
    await tester.pump();

    await tester.tap(find.text('Filter').last);
    await tester.pump();
    await tester.enterText(find.byType(TextField).last, 'right-only');
    await tester.pump();

    final filterValues = tester
        .widgetList<TextField>(find.byType(TextField))
        .map((field) => field.controller?.text)
        .toList();
    expect(filterValues, containsAll(<String>['left-only', 'right-only']));

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('terminal SFTP shortcut requires an SSH session', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await _openTraditionalLocalTerminal(tester);
    expect(find.byType(TerminalView), findsOneWidget);

    await _sendShowSftpShortcut(tester);

    expect(find.text('SFTP is available for SSH sessions.'), findsOneWidget);
    expect(find.byType(TerminalView), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('quick connect opens from tab plus and shortcut', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await tester.tap(find.byTooltip('Quick Connect'));
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('Quick Connect'), findsOneWidget);
    expect(find.text('Shells'), findsOneWidget);
    expect(find.byType(GridView), findsOneWidget);

    await tester.tap(find.byTooltip('Close'));
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('Quick Connect'), findsNothing);

    await _sendNewTabShortcut(tester);

    expect(find.text('Quick Connect'), findsOneWidget);

    await tester.tap(find.byTooltip('Close'));
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('Quick Connect'), findsNothing);

    await _sendQuickConnectShortcut(tester);

    expect(find.text('Quick Connect'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('quick connect opens the first shell directly', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await tester.tap(find.byTooltip('Quick Connect'));
    await tester.pump(const Duration(milliseconds: 200));

    final shellButtons = find.descendant(
      of: find.byType(GridView),
      matching: find.byType(Tooltip),
    );
    expect(shellButtons, findsWidgets);
    await tester.tap(shellButtons.first);
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('Quick Connect'), findsNothing);
    expect(find.byType(TerminalView), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('quick connect shell grid adapts columns and previews two rows', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await tester.tap(find.byTooltip('Quick Connect'));
    await tester.pump(const Duration(milliseconds: 200));

    final shellGridFinder = find.byType(GridView);
    final shellGrid = tester.widget<GridView>(shellGridFinder);
    final delegate =
        shellGrid.gridDelegate as SliverGridDelegateWithFixedCrossAxisCount;
    final shellTooltips = find.descendant(
      of: shellGridFinder,
      matching: find.byType(Tooltip),
    );
    final shellLabels = shellTooltips
        .evaluate()
        .map((element) => (element.widget as Tooltip).message)
        .whereType<String>()
        .toSet();
    final hasLongWindowsLabel =
        shellLabels.contains('Windows PowerShell') ||
        shellLabels.contains('Command Prompt');
    expect(delegate.crossAxisCount, hasLongWindowsLabel ? 3 : 4);

    final previewCount = delegate.crossAxisCount * 2;
    final showAll = find.text('Show All');
    if (showAll.evaluate().isNotEmpty) {
      expect(shellTooltips, findsNWidgets(previewCount));
      await tester.tap(showAll);
      await tester.pump();
      expect(shellTooltips.evaluate().length, greaterThan(previewCount));
    } else {
      expect(shellTooltips.evaluate().length, lessThanOrEqualTo(previewCount));
    }

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('temporary SSH tabs use the host title', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await tester.tap(find.byTooltip('Quick Connect'));
    await tester.pump(const Duration(milliseconds: 200));
    await tester.enterText(find.byType(TextField).last, 'demo@example.invalid');
    await tester.pump();

    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump(const Duration(milliseconds: 200));

    final hostTitles = find.byWidgetPredicate(
      (widget) => widget is Text && widget.data == 'example.invalid',
    );
    expect(hostTitles, findsNWidgets(2));

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('workspace tab shortcuts switch visible tabs', (
    WidgetTester tester,
  ) async {
    final previousSftpTabEnabled = sftpTabEnabled;
    final previousWorkspacePageEnabled = workspacePageEnabled;
    setSftpTabEnabled(true);
    setWorkspacePageEnabled(true);
    addTearDown(() {
      setSftpTabEnabled(previousSftpTabEnabled);
      setWorkspacePageEnabled(previousWorkspacePageEnabled);
    });
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    expect(find.text('New host'), findsOneWidget);

    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit2);

    expect(find.text('Connect to host'), findsOneWidget);

    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit1);

    expect(find.text('New host'), findsOneWidget);

    await _sendNextTabShortcut(tester);

    expect(find.text('Connect to host'), findsOneWidget);

    await _sendNextTabShortcut(tester);

    expect(find.text('No terminal sessions'), findsOneWidget);
    expect(
      find.ancestor(
        of: find.byKey(const ValueKey('workspace-top-page:sessions')),
        matching: find.byType(AnimatedSwitcher),
      ),
      findsNothing,
    );

    await _sendNextTabShortcut(tester);

    expect(find.text('New host'), findsOneWidget);

    await _sendPreviousTabShortcut(tester);

    expect(find.text('No terminal sessions'), findsOneWidget);

    await _sendPreviousTabShortcut(tester);

    expect(find.text('Connect to host'), findsOneWidget);

    await _sendPreviousTabShortcut(tester);

    expect(find.text('New host'), findsOneWidget);

    await _sendPreviousTabShortcut(
      tester,
      logicalKey: LogicalKeyboardKey.braceLeft,
    );

    expect(find.text('No terminal sessions'), findsOneWidget);

    await _sendNextTabShortcut(
      tester,
      logicalKey: LogicalKeyboardKey.braceRight,
    );
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('New host'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('shifted bracket aliases switch workspace tabs', (
    WidgetTester tester,
  ) async {
    final previousSftpTabEnabled = sftpTabEnabled;
    final previousWorkspacePageEnabled = workspacePageEnabled;
    setSftpTabEnabled(true);
    setWorkspacePageEnabled(true);
    addTearDown(() {
      setSftpTabEnabled(previousSftpTabEnabled);
      setWorkspacePageEnabled(previousWorkspacePageEnabled);
    });
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    expect(find.text('New host'), findsOneWidget);

    await _sendNextTabShortcut(
      tester,
      logicalKey: LogicalKeyboardKey.braceRight,
    );
    expect(find.text('Connect to host'), findsOneWidget);

    await _sendNextTabShortcut(
      tester,
      logicalKey: LogicalKeyboardKey.braceRight,
    );
    expect(find.text('No terminal sessions'), findsOneWidget);

    await _sendPreviousTabShortcut(
      tester,
      logicalKey: LogicalKeyboardKey.braceLeft,
    );
    expect(find.text('Connect to host'), findsOneWidget);

    await _sendPreviousTabShortcut(
      tester,
      logicalKey: LogicalKeyboardKey.braceLeft,
    );
    expect(find.text('New host'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('tab shortcuts skip pages hidden from the top bar', (
    WidgetTester tester,
  ) async {
    final previousSftpTabEnabled = sftpTabEnabled;
    final previousWorkspacePageEnabled = workspacePageEnabled;
    setSftpTabEnabled(true);
    setWorkspacePageEnabled(false);
    addTearDown(() {
      setSftpTabEnabled(previousSftpTabEnabled);
      setWorkspacePageEnabled(previousWorkspacePageEnabled);
    });
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    expect(find.text('New host'), findsOneWidget);

    await _sendNextTabShortcut(tester);
    expect(find.text('Connect to host'), findsOneWidget);

    await _sendNextTabShortcut(tester);
    expect(find.text('New host'), findsOneWidget);
    expect(find.text('No terminal sessions'), findsNothing);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('workspace layout can switch back to traditional tabs', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    expect(find.text('New host'), findsOneWidget);

    await _switchToMultiWorkspace(tester);

    expect(find.text('No terminal sessions'), findsOneWidget);

    await _switchToSingleWorkspace(tester);

    expect(find.text('New host'), findsOneWidget);

    await _openTraditionalLocalTerminal(tester);

    expect(find.byType(TerminalView), findsOneWidget);

    await tester.tap(find.byIcon(Icons.folder_rounded).first);
    await tester.pump();

    expect(find.byType(TerminalView), findsNothing);
    expect(find.text('Connect to host'), findsOneWidget);

    await tester.tap(find.text('Local Terminal'));
    await tester.pump();

    expect(find.byType(TerminalView), findsOneWidget);

    await _switchToMultiWorkspace(tester);

    expect(find.byType(TerminalView), findsOneWidget);
    expect(_terminalTopTabs(), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('opening a terminal does not reparent overlays during layout', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1036, 739));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await _openTraditionalLocalTerminal(tester);
    await tester.pump(const Duration(milliseconds: 200));

    expect(tester.takeException(), isNull);
    expect(find.byType(TerminalView), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('terminal view toolbar new tab stays inside terminal view', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await _switchToMultiWorkspace(tester);

    await _openLocalTerminal(tester);
    await _openLocalTerminal(tester);

    expect(find.byType(TerminalView), findsOneWidget);
    expect(_terminalTopTabs(), findsNWidgets(2));

    await tester.tap(find.byIcon(Icons.folder_rounded).first);
    await tester.pump();

    expect(find.byType(TerminalView), findsNothing);

    await _sendSelectTabShortcut(tester, LogicalKeyboardKey.digit4);

    expect(find.byType(TerminalView), findsOneWidget);
    expect(_terminalTopTabs(), findsNWidgets(2));

    final selectedTopTab = _terminalTopTabs().last;
    await tester.tap(
      find.descendant(
        of: selectedTopTab,
        matching: find.byIcon(Icons.close_rounded),
      ),
    );
    await tester.pump();
    if (find.text('Close Terminal').evaluate().isNotEmpty) {
      await tester.tap(find.text('Close').last);
      await tester.pump();
    }

    expect(find.byType(TerminalView), findsOneWidget);
    expect(_terminalTopTabs(), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('close shortcut does not stack terminal confirmation dialogs', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await _switchToMultiWorkspace(tester);
    await _openLocalTerminal(tester);
    await _openLocalTerminal(tester);

    await _sendCloseTabShortcut(tester);
    expect(find.text('Close Terminal'), findsOneWidget);

    await _sendCloseTabShortcut(tester);
    expect(find.text('Close Terminal'), findsOneWidget);
    expect(_terminalTopTabs(), findsNWidgets(2));

    await tester.tap(find.text('Cancel'));
    await tester.pump();
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('workspace split creates another terminal view', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await _switchToMultiWorkspace(tester);

    await _openLocalTerminal(tester);

    expect(find.byType(TerminalView), findsOneWidget);

    await tester.tap(find.byTooltip('Split right'));
    await tester.pump();

    expect(find.byType(TerminalView), findsNWidgets(2));

    await tester.tap(
      find.byType(TerminalWidget).last,
      buttons: kSecondaryMouseButton,
    );
    await tester.pump();
    await tester.tap(find.text('Close').last);
    await tester.pump();
    if (find.text('Close Terminal').evaluate().isNotEmpty) {
      await tester.tap(find.text('Close').last);
      await tester.pump();
    }

    expect(find.byType(TerminalView), findsOneWidget);
    expect(find.text('No terminal sessions'), findsNothing);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('terminal cursor blinks when focused', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await _openTraditionalLocalTerminal(tester);
    await tester.tap(_terminalPaintFinder());
    await tester.pump();

    expect(_terminalPainter(tester).showCursor, isTrue);

    await tester.pump(
      defaultTerminalConfig.cursor.blinkInterval +
          const Duration(milliseconds: 1),
    );

    expect(_terminalPainter(tester).showCursor, isFalse);

    await tester.pumpWidget(const SizedBox.shrink());
  });
}

class _ExitingTerminalDriver implements TerminalDriver {
  bool changed = false;
  bool exited = false;
  final List<TerminalConnectionEvent> events = [];
  final List<String> inputs = [];
  TerminalSnapshot currentSnapshot = TerminalSnapshot.blank();

  @override
  bool get isExited => exited;

  @override
  TerminalSnapshot get snapshot => currentSnapshot;

  @override
  void resize(int columns, int rows, {int cellWidth = 1, int cellHeight = 1}) {}

  @override
  void write(String data) {}

  @override
  void writeBytes(Uint8List bytes) {}

  @override
  bool sendInput(String data) {
    inputs.add(data);
    return true;
  }

  @override
  Uint8List drainOutputCapture() => Uint8List(0);

  @override
  bool suppressOutputUntil(Uint8List marker) => false;

  @override
  bool cancelOutputSuppression() => false;

  @override
  bool scrollLines(int lines) => false;

  @override
  bool scrollPageUp() => false;

  @override
  bool scrollPageDown() => false;

  @override
  bool scrollToBottom() => false;

  @override
  TerminalSearchResult search(
    String query, {
    required TerminalSearchDirection direction,
    required TerminalCellPosition origin,
  }) {
    return const TerminalSearchResult.notFound();
  }

  @override
  String selectionText(TerminalSelection selection) {
    return terminalSelectedText(currentSnapshot, selection);
  }

  @override
  TerminalCommandBlock? commandBlockAt(TerminalCellPosition position) {
    final selection = terminalCommandBlockAt(currentSnapshot, position);
    return selection == null
        ? null
        : TerminalCommandBlock(selection: selection);
  }

  @override
  TerminalPromptClickMove? promptClickMove(TerminalCellPosition position) =>
      null;

  @override
  void clear() {}

  @override
  void reset() {}

  @override
  List<TerminalConnectionEvent> drainConnectionEvents() {
    final drained = List<TerminalConnectionEvent>.of(events);
    events.clear();
    return drained;
  }

  @override
  bool poll() {
    final didChange = changed;
    changed = false;
    return didChange;
  }

  @override
  void dispose() {}
}

class _BackpressuredTerminalDriver extends _ExitingTerminalDriver {
  @override
  bool sendInput(String data) => false;
}

TerminalConnectionEvent _moshScreenCommittedEvent(int stateNum) {
  return TerminalConnectionEvent(
    kind: TerminalConnectionEventKind.moshScreenCommitted,
    message: 'screen state $stateNum committed',
    stateNum: stateNum,
  );
}

TerminalSnapshot _snapshotWithCursor({
  required int column,
  required int row,
  int columns = 80,
  int rows = 24,
  Map<(int, int), String> textCells = const {},
}) {
  final cells = List<TerminalCell>.filled(
    columns * rows,
    const TerminalCell.empty(),
    growable: false,
  );
  for (final entry in textCells.entries) {
    final (cellRow, cellColumn) = entry.key;
    cells[cellRow * columns + cellColumn] = TerminalCell(
      text: entry.value,
      foreground: terminalDefaultForeground,
      background: terminalDefaultBackground,
      flags: 0,
    );
  }
  return TerminalSnapshot(
    columns: columns,
    rows: rows,
    cells: cells,
    cursor: TerminalCursor(
      column: column,
      row: row,
      visible: true,
      shape: TerminalCursorShape.block,
      color: terminalDefaultCursor,
      blinking: false,
    ),
    keyboardMode: const TerminalKeyboardMode(),
    inputEchoEnabled: true,
  );
}

Future<void> _sendNewTabShortcut(WidgetTester tester) async {
  final modifier = await _pressCommandModifier(tester);
  await tester.sendKeyEvent(LogicalKeyboardKey.keyT);
  await tester.sendKeyUpEvent(modifier);
  await tester.pump(const Duration(milliseconds: 200));
}

Future<void> _sendCloseTabShortcut(WidgetTester tester) async {
  final modifier = await _pressCommandModifier(tester);
  await tester.sendKeyEvent(LogicalKeyboardKey.keyW);
  await tester.sendKeyUpEvent(modifier);
  await tester.pump();
}

Future<void> _sendQuickConnectShortcut(WidgetTester tester) async {
  final modifier = await _pressCommandModifier(tester);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(LogicalKeyboardKey.keyP);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyUpEvent(modifier);
  await tester.pump(const Duration(milliseconds: 200));
}

Future<void> _sendPreviousTabShortcut(
  WidgetTester tester, {
  LogicalKeyboardKey logicalKey = LogicalKeyboardKey.bracketLeft,
}) async {
  final modifier = await _pressCommandModifier(tester);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(
    logicalKey,
    platform: logicalKey == LogicalKeyboardKey.braceLeft ? 'macos' : null,
    physicalKey: logicalKey == LogicalKeyboardKey.braceLeft
        ? PhysicalKeyboardKey.bracketLeft
        : null,
  );
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyUpEvent(modifier);
  await tester.pump(const Duration(milliseconds: 200));
}

Future<void> _sendNextTabShortcut(
  WidgetTester tester, {
  LogicalKeyboardKey logicalKey = LogicalKeyboardKey.bracketRight,
}) async {
  final modifier = await _pressCommandModifier(tester);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(
    logicalKey,
    platform: logicalKey == LogicalKeyboardKey.braceRight ? 'macos' : null,
    physicalKey: logicalKey == LogicalKeyboardKey.braceRight
        ? PhysicalKeyboardKey.bracketRight
        : null,
  );
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyUpEvent(modifier);
  await tester.pump();
}

Future<void> _sendSelectTabShortcut(
  WidgetTester tester,
  LogicalKeyboardKey digit,
) async {
  final modifier = await _pressCommandModifier(tester);
  await tester.sendKeyEvent(digit);
  await tester.sendKeyUpEvent(modifier);
  await tester.pump();
}

Future<void> _sendShowSftpShortcut(WidgetTester tester) async {
  final modifier = await _pressCommandModifier(tester);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(LogicalKeyboardKey.keyS);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyUpEvent(modifier);
  await tester.pump(const Duration(milliseconds: 200));
}

Future<void> _switchToMultiWorkspace(WidgetTester tester) async {
  final previous = workspacePageEnabled;
  addTearDown(() => setWorkspacePageEnabled(previous));
  setWorkspacePageEnabled(true);
  await tester.pump();
  await tester.tap(find.text('Default').first);
  await tester.pump();
}

Future<void> _switchToSingleWorkspace(WidgetTester tester) async {
  final previous = workspacePageEnabled;
  addTearDown(() => setWorkspacePageEnabled(previous));
  setWorkspacePageEnabled(false);
  await tester.pump();
}

Future<LogicalKeyboardKey> _pressCommandModifier(WidgetTester tester) async {
  final modifier = defaultTargetPlatform == TargetPlatform.macOS
      ? LogicalKeyboardKey.metaLeft
      : LogicalKeyboardKey.controlLeft;
  await tester.sendKeyDownEvent(modifier);
  return modifier;
}

Future<void> _openLocalTerminal(WidgetTester tester) async {
  final toolbarButton = find.byTooltip('New tab');
  if (toolbarButton.evaluate().isNotEmpty) {
    await tester.tap(toolbarButton.last);
  } else {
    await tester.tap(find.byTooltip('Quick Connect'));
    await tester.pump(const Duration(milliseconds: 200));
    final shellButtons = find.descendant(
      of: find.byType(GridView),
      matching: find.byType(Tooltip),
    );
    await tester.tap(shellButtons.first);
  }
  await tester.pump(const Duration(milliseconds: 200));
}

Finder _terminalTopTabs() {
  return find.byWidgetPredicate(
    (widget) =>
        widget.key is ValueKey<String> &&
        (widget.key! as ValueKey<String>).value.startsWith('terminal-top-tab:'),
  );
}

Future<void> _openTraditionalLocalTerminal(WidgetTester tester) async {
  await tester.tap(find.text('Terminal'));
  await tester.pump();
}

Future<void> _doubleClickWithMouse(WidgetTester tester, Offset position) async {
  final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
  await mouse.addPointer(location: position);
  await mouse.down(position);
  await mouse.up();
  await tester.pump(const Duration(milliseconds: 50));
  await mouse.down(position);
  await mouse.up();
  await tester.pump(const Duration(milliseconds: 350));
  await mouse.removePointer();
}

TerminalPainter _terminalPainter(WidgetTester tester) {
  final customPaint = tester.widget<CustomPaint>(_terminalPaintFinder());

  return customPaint.painter! as TerminalPainter;
}

Finder _terminalPaintFinder() {
  return find.byWidgetPredicate(
    (widget) => widget is CustomPaint && widget.painter is TerminalPainter,
  );
}
