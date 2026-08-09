import 'package:characters/characters.dart';
import 'package:flutter/foundation.dart';

import 'terminal_models.dart';
import 'terminal_text_width.dart';

const int sshLocalPredictionCellBudget = 256;

@immutable
class SshPredictionDebugBatch {
  const SshPredictionDebugBatch({
    required this.batchId,
    required this.startColumn,
    required this.startRow,
    required this.text,
  });

  final int batchId;
  final int startColumn;
  final int startRow;
  final String text;
}

class _SshPredictionBatch {
  const _SshPredictionBatch({
    required this.id,
    required this.startColumn,
    required this.startRow,
  });

  final int id;
  final int startColumn;
  final int startRow;
}

class _SshPredictionUnit {
  const _SshPredictionUnit({
    required this.grapheme,
    required this.cellWidth,
    required this.batchId,
  });

  final String grapheme;
  final int cellWidth;
  final int batchId;
}

class _SshPredictionCursor {
  const _SshPredictionCursor(this.column, this.row);

  final int column;
  final int row;
}

enum _SshPredictionSnapshotState { confirmed, pending, mismatch }

/// A conservative visual prediction layer for SSH input.
///
/// The remote terminal snapshot remains authoritative. Predictions are removed
/// as soon as matching remote echo arrives and are discarded on any conflict.
class SshLocalPredictionState {
  final List<_SshPredictionUnit> _units = [];
  final Map<int, _SshPredictionBatch> _batches = {};
  var _nextBatchId = 0;
  var _pausedUntilScreenUpdate = false;

  String get text => _units.map((unit) => unit.grapheme).join();
  bool get isEmpty => _units.isEmpty;
  bool get pausedUntilScreenUpdate => _pausedUntilScreenUpdate;

  List<SshPredictionDebugBatch> get debugBatches {
    return [
      for (final batch in _batches.values)
        SshPredictionDebugBatch(
          batchId: batch.id,
          startColumn: batch.startColumn,
          startRow: batch.startRow,
          text: _units
              .where((unit) => unit.batchId == batch.id)
              .map((unit) => unit.grapheme)
              .join(),
        ),
    ];
  }

  bool addInput(String data, TerminalSnapshot snapshot) {
    if (_pausedUntilScreenUpdate || !_screenAllowsPrediction(snapshot)) {
      return clear();
    }
    if (_containsUnsupportedInput(data)) {
      final changed = clear();
      _pausedUntilScreenUpdate = true;
      return changed;
    }

    final before = text;
    for (final grapheme in data.characters) {
      if (grapheme == '\b' || grapheme == '\x7f') {
        if (_units.isNotEmpty) {
          _units.removeLast();
        }
      } else {
        final width = terminalGraphemeCellWidth(grapheme);
        if (width > 0) {
          // Keep one user-visible character per batch so a remote peer that
          // echoes a paste or IME commit incrementally can confirm only the
          // prefix it has actually rendered.
          final start = _cursorAfterPending(snapshot);
          final inputBatch = _SshPredictionBatch(
            id: ++_nextBatchId,
            startColumn: start.column,
            startRow: start.row,
          );
          _batches[inputBatch.id] = inputBatch;
          _units.add(
            _SshPredictionUnit(
              grapheme: grapheme,
              cellWidth: width,
              batchId: inputBatch.id,
            ),
          );
        }
      }
    }

    _pruneEmptyBatches();
    final predictedCells = _units.fold<int>(
      0,
      (total, unit) => total + unit.cellWidth,
    );
    if (predictedCells > sshLocalPredictionCellBudget) {
      clear();
      _pausedUntilScreenUpdate = true;
    }
    return before != text;
  }

  bool reconcile(TerminalSnapshot snapshot) {
    if (_pausedUntilScreenUpdate) {
      _pausedUntilScreenUpdate = false;
      return clear();
    }
    if (_units.isEmpty) {
      return false;
    }
    if (!_screenAllowsPrediction(snapshot)) {
      return clear();
    }

    int? lastConfirmedBatchId;
    int? mismatchedBatchId;
    final ordered = _batches.keys.toList()..sort();
    for (final batchId in ordered) {
      final batch = _batches[batchId];
      if (batch == null) {
        continue;
      }
      final units = _units
          .where((unit) => unit.batchId == batchId)
          .toList(growable: false);
      if (units.isEmpty) {
        continue;
      }
      switch (_snapshotState(snapshot, batch, units)) {
        case _SshPredictionSnapshotState.confirmed:
          lastConfirmedBatchId = batchId;
        case _SshPredictionSnapshotState.pending:
          break;
        case _SshPredictionSnapshotState.mismatch:
          mismatchedBatchId = batchId;
      }
      if (mismatchedBatchId != null || lastConfirmedBatchId != batchId) {
        break;
      }
    }

    final before = text;
    if (lastConfirmedBatchId != null) {
      _units.removeWhere((unit) => unit.batchId <= lastConfirmedBatchId!);
      _batches.removeWhere((id, _) => id <= lastConfirmedBatchId!);
    }
    if (mismatchedBatchId != null) {
      _units.removeWhere((unit) => unit.batchId >= mismatchedBatchId!);
      _batches.removeWhere((id, _) => id >= mismatchedBatchId!);
    }
    _pruneEmptyBatches();
    return before != text;
  }

  bool suspendUntilScreenUpdate() {
    final changed = clear();
    _pausedUntilScreenUpdate = true;
    return changed;
  }

  bool clear() {
    if (_units.isEmpty && _batches.isEmpty) {
      return false;
    }
    _units.clear();
    _batches.clear();
    return true;
  }

  bool _screenAllowsPrediction(TerminalSnapshot snapshot) {
    return snapshot.inputEchoEnabled &&
        !snapshot.alternateScreen &&
        snapshot.displayOffset == 0 &&
        snapshot.cursor.visible &&
        !snapshot.keyboardMode.mouseReporting;
  }

  bool _containsUnsupportedInput(String data) {
    if (data.isEmpty) {
      return true;
    }
    return data.runes.any(
      (rune) =>
          rune == 0x1b ||
          rune == 0x0d ||
          rune == 0x0a ||
          (rune < 0x20 && rune != 0x08),
    );
  }

  _SshPredictionCursor _cursorAfterPending(TerminalSnapshot snapshot) {
    if (_units.isEmpty) {
      return _SshPredictionCursor(snapshot.cursor.column, snapshot.cursor.row);
    }
    final firstBatch = _batches[_units.first.batchId];
    if (firstBatch == null) {
      return _SshPredictionCursor(snapshot.cursor.column, snapshot.cursor.row);
    }
    return _cursorAfterUnits(
      columns: snapshot.columns,
      startColumn: firstBatch.startColumn,
      startRow: firstBatch.startRow,
      units: _units,
    );
  }

  _SshPredictionSnapshotState _snapshotState(
    TerminalSnapshot snapshot,
    _SshPredictionBatch batch,
    List<_SshPredictionUnit> units,
  ) {
    final expected = _cursorAfterUnits(
      columns: snapshot.columns,
      startColumn: batch.startColumn,
      startRow: batch.startRow,
      units: units,
    );
    final cursorComparison = _comparePositions(
      snapshot.cursor.column,
      snapshot.cursor.row,
      expected.column,
      expected.row,
    );
    if (_snapshotMatches(snapshot, batch, units)) {
      return cursorComparison >= 0
          ? _SshPredictionSnapshotState.confirmed
          : _SshPredictionSnapshotState.pending;
    }
    if (_snapshotConflicts(snapshot, batch, units)) {
      return _SshPredictionSnapshotState.mismatch;
    }
    return cursorComparison >= 0
        ? _SshPredictionSnapshotState.mismatch
        : _SshPredictionSnapshotState.pending;
  }

  bool _snapshotMatches(
    TerminalSnapshot snapshot,
    _SshPredictionBatch batch,
    List<_SshPredictionUnit> units,
  ) {
    var column = batch.startColumn;
    var row = batch.startRow;
    for (final unit in units) {
      if (column + unit.cellWidth > snapshot.columns) {
        column = 0;
        row += 1;
      }
      if (snapshot.cellAt(row, column).text != unit.grapheme) {
        return false;
      }
      column += unit.cellWidth;
      if (column >= snapshot.columns) {
        column = 0;
        row += 1;
      }
    }
    return true;
  }

  bool _snapshotConflicts(
    TerminalSnapshot snapshot,
    _SshPredictionBatch batch,
    List<_SshPredictionUnit> units,
  ) {
    var column = batch.startColumn;
    var row = batch.startRow;
    for (final unit in units) {
      if (column + unit.cellWidth > snapshot.columns) {
        column = 0;
        row += 1;
      }
      final actual = snapshot.cellAt(row, column).text;
      if (actual != ' ' && actual != unit.grapheme) {
        return true;
      }
      column += unit.cellWidth;
      if (column >= snapshot.columns) {
        column = 0;
        row += 1;
      }
    }
    return false;
  }

  _SshPredictionCursor _cursorAfterUnits({
    required int columns,
    required int startColumn,
    required int startRow,
    required Iterable<_SshPredictionUnit> units,
  }) {
    var column = startColumn;
    var row = startRow;
    for (final unit in units) {
      if (column + unit.cellWidth > columns) {
        column = 0;
        row += 1;
      }
      column += unit.cellWidth;
      if (column >= columns) {
        column = 0;
        row += 1;
      }
    }
    return _SshPredictionCursor(column, row);
  }

  int _comparePositions(int column, int row, int otherColumn, int otherRow) {
    return row == otherRow
        ? column.compareTo(otherColumn)
        : row.compareTo(otherRow);
  }

  void _pruneEmptyBatches() {
    final activeIds = _units.map((unit) => unit.batchId).toSet();
    _batches.removeWhere((id, _) => !activeIds.contains(id));
  }
}
