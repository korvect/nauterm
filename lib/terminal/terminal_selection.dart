import 'dart:math' as math;

import 'package:flutter/foundation.dart';

import 'terminal_models.dart';

@immutable
class TerminalCellPosition {
  const TerminalCellPosition({required this.row, required this.column});

  final int row;
  final int column;

  int toOffset(int columns) => row * columns + column;

  TerminalCellPosition clampTo(TerminalSnapshot snapshot) {
    return TerminalCellPosition(
      row: row.clamp(0, snapshot.rows - 1).toInt(),
      column: column.clamp(0, snapshot.columns - 1).toInt(),
    );
  }

  @override
  bool operator ==(Object other) {
    return other is TerminalCellPosition &&
        other.row == row &&
        other.column == column;
  }

  @override
  int get hashCode => Object.hash(row, column);
}

@immutable
class TerminalSelection {
  const TerminalSelection({required this.start, required this.end});

  factory TerminalSelection.fromCellRange({
    required TerminalCellPosition anchor,
    required TerminalCellPosition extent,
    required TerminalSnapshot snapshot,
  }) {
    final clampedAnchor = anchor.clampTo(snapshot);
    final clampedExtent = extent.clampTo(snapshot);
    return TerminalSelection.fromOffsets(
      anchor: terminalCellOffset(snapshot, clampedAnchor),
      extent: terminalCellOffset(snapshot, clampedExtent),
    );
  }

  factory TerminalSelection.fromOffsets({
    required int anchor,
    required int extent,
  }) {
    return TerminalSelection(
      start: math.min(anchor, extent),
      end: math.max(anchor, extent) + 1,
    );
  }

  factory TerminalSelection.line({
    required int row,
    required TerminalSnapshot snapshot,
  }) {
    final clampedRow = row.clamp(0, snapshot.rows - 1).toInt();
    final start = (clampedRow - snapshot.displayOffset) * snapshot.columns;
    return TerminalSelection(start: start, end: start + snapshot.columns);
  }

  factory TerminalSelection.all(TerminalSnapshot snapshot) {
    return TerminalSelection(
      start: -snapshot.historyLines * snapshot.columns,
      end: snapshot.rows * snapshot.columns,
    );
  }

  final int start;
  final int end;

  bool get isCollapsed => start >= end;

  bool containsViewportCell({
    required int row,
    required int column,
    required TerminalSnapshot snapshot,
  }) {
    final offset = (row - snapshot.displayOffset) * snapshot.columns + column;
    return offset >= start && offset < end;
  }

  TerminalSelection shift(int cells) {
    return TerminalSelection(start: start + cells, end: end + cells);
  }

  @override
  bool operator ==(Object other) {
    return other is TerminalSelection &&
        other.start == start &&
        other.end == end;
  }

  @override
  int get hashCode => Object.hash(start, end);
}

@immutable
class TerminalCommandBlock {
  const TerminalCommandBlock({
    required this.selection,
    this.id,
    this.inputStart,
    this.workingDirectory,
    this.command,
    this.exitCode,
    this.completed = false,
    this.shellIntegrated = false,
  });

  final TerminalSelection selection;
  final int? id;

  /// Absolute terminal cell offset where editable shell input begins.
  final int? inputStart;
  final String? workingDirectory;
  final String? command;
  final int? exitCode;
  final bool completed;
  final bool shellIntegrated;
}

TerminalSelection? terminalWordSelectionAt(
  TerminalSnapshot snapshot,
  TerminalCellPosition position,
) {
  final normalized = _normalizePosition(snapshot, position);
  final row = normalized.row;
  final column = normalized.column;
  final kind = _selectableKind(snapshot.cellAt(row, column));
  if (kind == _SelectableKind.blank) {
    return null;
  }

  var start = column;
  while (start > 0 &&
      _selectableKind(snapshot.cellAt(row, start - 1)) == kind) {
    start--;
  }

  var end = column + 1;
  while (end < snapshot.columns &&
      _selectableKind(snapshot.cellAt(row, end)) == kind) {
    end++;
  }

  return TerminalSelection(
    start: (row - snapshot.displayOffset) * snapshot.columns + start,
    end: (row - snapshot.displayOffset) * snapshot.columns + end,
  );
}

String terminalSelectedText(
  TerminalSnapshot snapshot,
  TerminalSelection? selection,
) {
  if (selection == null || selection.isCollapsed) {
    return '';
  }

  final viewportStart = -snapshot.displayOffset * snapshot.columns;
  final viewportEnd = viewportStart + snapshot.columns * snapshot.rows;
  final start = selection.start.clamp(viewportStart, viewportEnd).toInt();
  final end = selection.end.clamp(viewportStart, viewportEnd).toInt();
  if (start >= end) {
    return '';
  }

  final localStart = start - viewportStart;
  final localEnd = end - viewportStart;
  final startRow = localStart ~/ snapshot.columns;
  final endRow = (localEnd - 1) ~/ snapshot.columns;
  final text = StringBuffer();

  for (var row = startRow; row <= endRow; row++) {
    if (row > startRow) {
      text.write('\n');
    }

    final rowStart = row * snapshot.columns;
    final startColumn = row == startRow ? localStart - rowStart : 0;
    final endColumn = row == endRow ? localEnd - rowStart : snapshot.columns;
    text.write(
      _trimRightSpaces(_selectedRowText(snapshot, row, startColumn, endColumn)),
    );
  }

  return text.toString();
}

TerminalSelection? terminalVisibleTextSelection(TerminalSnapshot snapshot) {
  final totalCells = snapshot.columns * snapshot.rows;
  for (var offset = totalCells - 1; offset >= 0; offset--) {
    final row = offset ~/ snapshot.columns;
    final column = offset - row * snapshot.columns;
    if (_selectableKind(snapshot.cellAt(row, column)) !=
        _SelectableKind.blank) {
      final viewportStart = -snapshot.displayOffset * snapshot.columns;
      return TerminalSelection(
        start: viewportStart,
        end: viewportStart + offset + 1,
      );
    }
  }

  return null;
}

String _selectedRowText(
  TerminalSnapshot snapshot,
  int row,
  int startColumn,
  int endColumn,
) {
  final text = StringBuffer();
  for (var column = startColumn; column < endColumn; column++) {
    final cell = snapshot.cellAt(row, column);
    if (cell.wideCharSpacer || cell.leadingWideCharSpacer) {
      continue;
    }

    if (cell.hidden || cell.text.isEmpty) {
      text.write(' ');
    } else {
      text.write(cell.text);
    }
  }

  return text.toString();
}

TerminalSelection? terminalCommandBlockAt(
  TerminalSnapshot snapshot,
  TerminalCellPosition position,
) {
  final targetRow = position.row.clamp(0, snapshot.rows - 1).toInt();
  final promptKinds = List<bool?>.generate(
    snapshot.rows,
    (row) => _terminalPromptKind(_terminalRowText(snapshot, row)),
    growable: false,
  );
  final preferStructuredPrompt = promptKinds.contains(true);
  bool isPrompt(int row) {
    final structured = promptKinds[row];
    return structured != null && (!preferStructuredPrompt || structured);
  }

  int? startRow;
  for (var row = targetRow; row >= 0; row--) {
    if (isPrompt(row)) {
      startRow = row;
      break;
    }
  }
  if (startRow == null) {
    return null;
  }

  var endRow = snapshot.rows;
  for (var row = targetRow + 1; row < snapshot.rows; row++) {
    if (isPrompt(row)) {
      endRow = row;
      break;
    }
  }
  final viewportStart = -snapshot.displayOffset * snapshot.columns;
  return TerminalSelection(
    start: viewportStart + startRow * snapshot.columns,
    end: viewportStart + endRow * snapshot.columns,
  );
}

String _terminalRowText(TerminalSnapshot snapshot, int row) {
  return _selectedRowText(snapshot, row, 0, snapshot.columns);
}

bool? _terminalPromptKind(String text) {
  final trimmed = text.trimLeft();
  if (trimmed.isEmpty) {
    return null;
  }
  for (var index = 0; index < trimmed.length; index++) {
    final character = trimmed[index];
    if (!r'$%#>'.contains(character)) {
      continue;
    }
    if (index + 1 < trimmed.length &&
        !RegExp(r'\s').hasMatch(trimmed[index + 1])) {
      continue;
    }
    final prefix = trimmed.substring(0, index).trimRight();
    if (prefix.isEmpty || _structuredPromptPrefix(prefix)) {
      return prefix.isNotEmpty;
    }
  }
  return null;
}

bool _structuredPromptPrefix(String prefix) {
  if (prefix.startsWith('PS ')) {
    return true;
  }
  if (prefix.startsWith('[') && prefix.endsWith(']')) {
    return true;
  }
  if (RegExp(
    r'^[^\s@]+@[^\s:]+(?:(?::|\s+)(?:~|/|[A-Za-z]:[\\/]).*)?$',
  ).hasMatch(prefix)) {
    return true;
  }
  final windowsPath = RegExp(r'^[A-Za-z]:[\\/]').hasMatch(prefix);
  return !prefix.contains(RegExp(r'\s')) &&
      (prefix.startsWith('/') || prefix.startsWith('~/') || windowsPath);
}

TerminalCellPosition _normalizePosition(
  TerminalSnapshot snapshot,
  TerminalCellPosition position,
) {
  final clamped = position.clampTo(snapshot);
  final cell = snapshot.cellAt(clamped.row, clamped.column);
  if (cell.wideCharSpacer && clamped.column > 0) {
    return TerminalCellPosition(row: clamped.row, column: clamped.column - 1);
  }
  if (cell.leadingWideCharSpacer && clamped.column + 1 < snapshot.columns) {
    return TerminalCellPosition(row: clamped.row, column: clamped.column + 1);
  }

  return clamped;
}

enum _SelectableKind { blank, word, symbol }

int terminalCellOffset(
  TerminalSnapshot snapshot,
  TerminalCellPosition position,
) {
  final clamped = position.clampTo(snapshot);
  return (clamped.row - snapshot.displayOffset) * snapshot.columns +
      clamped.column;
}

_SelectableKind _selectableKind(TerminalCell cell) {
  if (cell.hidden ||
      cell.text.isEmpty ||
      cell.wideCharSpacer ||
      cell.leadingWideCharSpacer ||
      _isBlankText(cell.text)) {
    return _SelectableKind.blank;
  }

  return _isWordText(cell.text) ? _SelectableKind.word : _SelectableKind.symbol;
}

bool _isWordText(String text) {
  for (final rune in text.runes) {
    if (_isWordRune(rune)) {
      return true;
    }
  }

  return false;
}

bool _isWordRune(int rune) {
  return (rune >= 0x30 && rune <= 0x39) ||
      (rune >= 0x41 && rune <= 0x5a) ||
      rune == 0x5f ||
      rune == 0x2d ||
      (rune >= 0x61 && rune <= 0x7a) ||
      (rune > 0x7f && !_isWhitespaceRune(rune));
}

bool _isBlankText(String text) {
  for (final rune in text.runes) {
    if (!_isWhitespaceRune(rune)) {
      return false;
    }
  }

  return true;
}

bool _isWhitespaceRune(int rune) {
  return rune == 0x09 || rune == 0x0a || rune == 0x0d || rune == 0x20;
}

String _trimRightSpaces(String value) {
  var end = value.length;
  while (end > 0 && value.codeUnitAt(end - 1) == 0x20) {
    end--;
  }

  return value.substring(0, end);
}
