import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/terminal/terminal_selection.dart';
import 'package:nauterm/terminal/terminal_theme.dart';

void main() {
  group('Mosh prediction batch state machine', () {
    test('ACK alone marks input but keeps the predicted overlay', () {
      final (controller, driver) = _connectedController();

      _sendBatch(controller, driver, 'abc', 1);
      _ack(driver, controller, 1);

      expect(controller.moshPrediction, 'abc');
      expect(controller.debugMoshPredictionBatches.single.inputAcked, isTrue);
      expect(
        controller.debugMoshPredictionBatches.single.snapshotCommitted,
        isFalse,
      );
      controller.dispose();
    });

    test('state 1 and state 2 are committed in order', () {
      final (controller, driver) = _connectedController();

      _sendBatch(controller, driver, 'abc', 1);
      _sendBatch(controller, driver, 'def', 2);
      _ack(driver, controller, 2);
      _commit(driver, controller, 1, _snapshot(column: 3, text: 'abc'));

      expect(controller.moshPrediction, 'def');
      expect(controller.debugMoshPredictionBatches.single.inputStateNum, 2);

      _commit(driver, controller, 2, _snapshot(column: 6, text: 'abcdef'));
      expect(controller.moshPrediction, isEmpty);
      controller.dispose();
    });

    test('one snapshot can commit multiple acknowledged batches', () {
      final (controller, driver) = _connectedController();

      _sendBatch(controller, driver, 'abc', 1);
      _sendBatch(controller, driver, 'def', 2);
      _ack(driver, controller, 2);
      _commit(driver, controller, 2, _snapshot(column: 6, text: 'abcdef'));

      expect(controller.moshPrediction, isEmpty);
      expect(controller.debugMoshPredictionBatches, isEmpty);
      controller.dispose();
    });

    test('snapshot match commits a batch even when its ACK is delayed', () {
      final (controller, driver) = _connectedController();

      _sendBatch(controller, driver, 'abc', 1);
      _commit(driver, controller, 1, _snapshot(column: 3, text: 'abc'));

      expect(controller.moshPrediction, isEmpty);
      expect(controller.debugMoshPredictionBatches, isEmpty);
      controller.dispose();
    });

    test('mismatch rolls back from the first mismatching batch', () {
      final (controller, driver) = _connectedController();

      _sendBatch(controller, driver, 'abc', 1);
      _sendBatch(controller, driver, 'def', 2);
      _ack(driver, controller, 2);
      _commit(driver, controller, 2, _snapshot(column: 6, text: 'abcXef'));

      expect(controller.snapshot.cellAt(0, 0).text, 'a');
      expect(controller.moshPrediction, isEmpty);
      expect(controller.debugMoshPredictionBatches, isEmpty);
      controller.dispose();
    });

    test('cursor at the next batch origin does not discard that batch', () {
      final (controller, driver) = _connectedController(column: 3);

      controller.sendInput('ab');
      controller.sendInput('cd');
      driver.currentSnapshot = _snapshot(column: 5);
      controller.refreshSnapshot();

      expect(controller.moshPrediction, 'abcd');
      expect(controller.debugMoshPredictionBatches, hasLength(2));
      controller.dispose();
    });

    test('network switching retains predictions for the same transport', () {
      final (controller, driver) = _connectedController();

      controller.sendInput('pending');
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.moshNetworkSwitching,
          message: 'switching',
        ),
      );
      controller.poll();

      expect(controller.moshNetworkState, MoshNetworkState.switching);
      expect(controller.moshPrediction, 'pending');
      controller.dispose();
    });

    test('backspace removes one complete grapheme cluster', () {
      final (controller, _) = _connectedController();

      controller.sendInput('e\u0301🙂');
      controller.sendInput('\b');

      expect(controller.moshPrediction, 'e\u0301');
      controller.dispose();
    });

    test('disposing after many predictions is safe', () {
      final (controller, _) = _connectedController();

      for (var index = 0; index < 300; index += 1) {
        controller.sendInput('x');
      }

      expect(() => controller.dispose(), returnsNormally);
    });
  });
}

(TerminalController, _PredictionDriver) _connectedController({int column = 0}) {
  final driver = _PredictionDriver()
    ..currentSnapshot = _snapshot(column: column);
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
  return (controller, driver);
}

void _sendBatch(
  TerminalController controller,
  _PredictionDriver driver,
  String text,
  int stateNum,
) {
  controller.sendInput(text);
  driver.events.add(
    TerminalConnectionEvent(
      kind: TerminalConnectionEventKind.moshInputStateQueued,
      message: 'state $stateNum queued',
      stateNum: stateNum,
    ),
  );
  controller.poll();
}

void _ack(
  _PredictionDriver driver,
  TerminalController controller,
  int stateNum,
) {
  driver.events.add(
    TerminalConnectionEvent(
      kind: TerminalConnectionEventKind.moshPredictionConfirmed,
      message: 'state $stateNum acknowledged',
      stateNum: stateNum,
    ),
  );
  controller.poll();
}

void _commit(
  _PredictionDriver driver,
  TerminalController controller,
  int stateNum,
  TerminalSnapshot snapshot,
) {
  driver.currentSnapshot = snapshot;
  driver.events.add(
    TerminalConnectionEvent(
      kind: TerminalConnectionEventKind.moshScreenCommitted,
      message: 'screen $stateNum committed',
      stateNum: stateNum,
    ),
  );
  driver.changed = true;
  controller.poll();
}

TerminalSnapshot _snapshot({
  required int column,
  String text = '',
  int columns = 80,
  int rows = 24,
}) {
  final cells = List<TerminalCell>.filled(
    columns * rows,
    const TerminalCell.empty(),
    growable: false,
  );
  for (var index = 0; index < text.length; index += 1) {
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
    inputEchoEnabled: true,
  );
}

class _PredictionDriver implements TerminalDriver {
  bool changed = false;
  final List<TerminalConnectionEvent> events = [];
  TerminalSnapshot currentSnapshot = TerminalSnapshot.blank();

  @override
  bool get isExited => false;

  @override
  TerminalSnapshot get snapshot => currentSnapshot;

  @override
  bool poll() {
    final result = changed;
    changed = false;
    return result;
  }

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
  bool suppressOutputUntil(Uint8List marker) => false;

  @override
  void write(String data) {}

  @override
  void writeBytes(Uint8List bytes) {}
}
