import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/terminal/terminal_selection.dart';
import 'package:nauterm/terminal/terminal_ssh_prediction.dart';
import 'package:nauterm/terminal/terminal_theme.dart';

void main() {
  group('SSH local prediction state', () {
    test('retains only input that has not appeared in the remote snapshot', () {
      final state = SshLocalPredictionState();
      final blank = _snapshot(column: 0);

      expect(state.addInput('a', blank), isTrue);
      expect(state.addInput('b', blank), isTrue);
      expect(state.text, 'ab');

      state.reconcile(_snapshot(column: 1, text: 'a'));
      expect(state.text, 'b');

      state.reconcile(_snapshot(column: 2, text: 'ab'));
      expect(state.text, isEmpty);
    });

    test('partially confirms a multi-character input submission', () {
      final state = SshLocalPredictionState();
      state.addInput('abc', _snapshot(column: 0));

      state.reconcile(_snapshot(column: 1, text: 'a'));

      expect(state.text, 'bc');
      expect(
        state.debugBatches.map((batch) => batch.text),
        orderedEquals(<String>['b', 'c']),
      );
    });

    test('discards predictions when remote output conflicts', () {
      final state = SshLocalPredictionState();
      state.addInput('abc', _snapshot(column: 0));

      state.reconcile(_snapshot(column: 3, text: 'axc'));

      expect(state.text, isEmpty);
      expect(state.debugBatches, isEmpty);
    });

    test('keeps confirmed prefix and removes a rewritten suffix', () {
      final state = SshLocalPredictionState();
      state.addInput('abc', _snapshot(column: 0));

      state.reconcile(_snapshot(column: 2, text: 'aX'));

      expect(state.text, isEmpty);
      expect(state.debugBatches, isEmpty);
    });

    test('backspace removes an unconfirmed grapheme cluster', () {
      final state = SshLocalPredictionState();
      final blank = _snapshot(column: 0);

      state.addInput('e\u0301🙂', blank);
      state.addInput('\x7f', blank);

      expect(state.text, 'e\u0301');
    });

    test('control input pauses prediction until the next screen update', () {
      final state = SshLocalPredictionState();
      final blank = _snapshot(column: 0);
      state.addInput('pending', blank);

      state.addInput('\r', blank);
      expect(state.text, isEmpty);
      expect(state.pausedUntilScreenUpdate, isTrue);
      expect(state.addInput('x', blank), isFalse);

      state.reconcile(blank);
      expect(state.pausedUntilScreenUpdate, isFalse);
      expect(state.addInput('x', blank), isTrue);
    });

    test('never predicts in an alternate screen', () {
      final state = SshLocalPredictionState();

      expect(
        state.addInput('x', _snapshot(column: 0, alternateScreen: true)),
        isFalse,
      );
      expect(state.text, isEmpty);
    });

    test('prediction budget fails closed', () {
      final state = SshLocalPredictionState();

      state.addInput(
        List.filled(sshLocalPredictionCellBudget + 1, 'x').join(),
        _snapshot(column: 0, columns: 400),
      );

      expect(state.text, isEmpty);
      expect(state.pausedUntilScreenUpdate, isTrue);
    });
  });

  group('SSH controller prediction policy', () {
    test('adaptive mode predicts after a high-latency measurement', () {
      final (controller, driver) = _connectedController(
        mode: TerminalSshPredictionMode.adaptive,
        latencyMs: 120,
      );

      controller.sendInput('a');
      expect(controller.sshPrediction, 'a');
      expect(controller.localPrediction, 'a');

      driver.currentSnapshot = _snapshot(column: 1, text: 'a');
      controller.refreshSnapshot();
      expect(controller.sshPrediction, isEmpty);
      controller.dispose();
    });

    test('adaptive mode stays off on a low-latency connection', () {
      final (controller, _) = _connectedController(
        mode: TerminalSshPredictionMode.adaptive,
        latencyMs: 20,
      );

      controller.sendInput('a');

      expect(controller.sshPrediction, isEmpty);
      controller.dispose();
    });

    test(
      'never mode keeps prediction disabled on a high-latency connection',
      () {
        final (controller, _) = _connectedController(
          mode: TerminalSshPredictionMode.never,
          latencyMs: 120,
        );

        controller.sendInput('a');

        expect(controller.sshPrediction, isEmpty);
        expect(controller.localPrediction, isEmpty);
        controller.dispose();
      },
    );

    test('always mode still blocks prediction for sensitive input', () {
      final (controller, driver) = _connectedController(
        mode: TerminalSshPredictionMode.always,
      );
      driver.currentSnapshot = _snapshot(column: 9, text: 'Password:');
      controller.refreshSnapshot();

      controller.sendInput('secret');

      expect(controller.sshPrediction, isEmpty);
      controller.dispose();
    });
  });
}

(TerminalController, _PredictionDriver) _connectedController({
  required TerminalSshPredictionMode mode,
  double? latencyMs,
}) {
  final driver = _PredictionDriver();
  final controller = TerminalController.ssh(
    host: 'example.test',
    port: 22,
    username: 'user',
    knownHostsPath: '/tmp/known-hosts',
    driver: driver,
    config: TerminalConfig(sshPredictionMode: mode),
  );
  driver.events.add(
    const TerminalConnectionEvent(
      kind: TerminalConnectionEventKind.connected,
      message: 'connected',
    ),
  );
  if (latencyMs != null) {
    driver.events.add(
      TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.sshLatencyUpdated,
        message: 'latency',
        latencyMs: latencyMs,
      ),
    );
  }
  controller.poll();
  return (controller, driver);
}

TerminalSnapshot _snapshot({
  required int column,
  String text = '',
  int columns = 80,
  int rows = 24,
  bool alternateScreen = false,
}) {
  final cells = List<TerminalCell>.filled(
    columns * rows,
    const TerminalCell.empty(),
    growable: false,
  );
  for (var index = 0; index < text.length && index < cells.length; index++) {
    cells[index] = TerminalCell(
      text: text[index],
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
      row: 0,
      visible: true,
      shape: TerminalCursorShape.block,
      color: terminalDefaultCursor,
      blinking: false,
    ),
    keyboardMode: const TerminalKeyboardMode(),
    alternateScreen: alternateScreen,
  );
}

class _PredictionDriver implements TerminalDriver {
  final List<TerminalConnectionEvent> events = [];
  TerminalSnapshot currentSnapshot = _snapshot(column: 0);

  @override
  bool get isExited => false;

  @override
  TerminalSnapshot get snapshot => currentSnapshot;

  @override
  bool poll() => false;

  @override
  List<TerminalConnectionEvent> drainConnectionEvents() {
    final result = List<TerminalConnectionEvent>.of(events);
    events.clear();
    return result;
  }

  @override
  bool sendInput(String data) => true;

  @override
  Uint8List drainOutputCapture() => Uint8List(0);

  @override
  void clear() {}

  @override
  void dispose() {}

  @override
  void reset() {}

  @override
  void resize(int columns, int rows, {int cellWidth = 1, int cellHeight = 1}) {}

  @override
  bool cancelOutputSuppression() => false;

  @override
  bool scrollLines(int lines) => false;

  @override
  bool scrollPageDown() => false;

  @override
  bool scrollPageUp() => false;

  @override
  bool scrollToBottom() => false;

  @override
  TerminalSearchResult search(
    String query, {
    required TerminalSearchDirection direction,
    required TerminalCellPosition origin,
  }) => const TerminalSearchResult.notFound();

  @override
  String selectionText(TerminalSelection selection) =>
      terminalSelectedText(currentSnapshot, selection);

  @override
  TerminalCommandBlock? commandBlockAt(TerminalCellPosition position) => null;

  @override
  bool suppressOutputUntil(Uint8List marker) => false;

  @override
  void write(String data) {}

  @override
  void writeBytes(Uint8List bytes) {}
}
