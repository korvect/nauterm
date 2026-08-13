import 'dart:convert';
import 'dart:math' as math;
import 'dart:typed_data';

import 'terminal_config.dart';
import 'terminal_models.dart';
import 'terminal_selection.dart';
import 'terminal_theme.dart';

enum TerminalSearchDirection { next, previous }

enum TerminalInputStatus {
  accepted,
  backpressure,
  closed,
  invalid;

  static TerminalInputStatus fromNative(int value) {
    return switch (value) {
      0 => TerminalInputStatus.accepted,
      1 => TerminalInputStatus.backpressure,
      2 => TerminalInputStatus.closed,
      _ => TerminalInputStatus.invalid,
    };
  }
}

class TerminalSearchResult {
  const TerminalSearchResult({this.selection, this.error});

  const TerminalSearchResult.notFound({this.error}) : selection = null;

  final TerminalSelection? selection;
  final String? error;

  bool get found => selection != null;
}

abstract interface class TerminalDriver {
  TerminalSnapshot get snapshot;

  bool get isExited;

  void resize(int columns, int rows, {int cellWidth = 1, int cellHeight = 1});

  void write(String data);

  void writeBytes(Uint8List bytes);

  bool sendInput(String data);

  Uint8List drainOutputCapture();

  bool suppressOutputUntil(Uint8List marker);

  bool cancelOutputSuppression();

  bool scrollLines(int lines);

  bool scrollPageUp();

  bool scrollPageDown();

  bool scrollToBottom();

  TerminalSearchResult search(
    String query, {
    required TerminalSearchDirection direction,
    required TerminalCellPosition origin,
  });

  String selectionText(TerminalSelection selection);

  TerminalCommandBlock? commandBlockAt(TerminalCellPosition position);

  TerminalPromptClickMove? promptClickMove(TerminalCellPosition position);

  void clear();

  void reset();

  List<TerminalConnectionEvent> drainConnectionEvents();

  bool poll();

  void dispose();
}

class MemoryTerminalDriver implements TerminalDriver {
  MemoryTerminalDriver({
    required int columns,
    required int rows,
    this.config = defaultTerminalConfig,
  }) : _columns = math.max(2, columns),
       _rows = math.max(1, rows) {
    _cells = _emptyCells();
  }

  final TerminalConfig config;
  late List<TerminalCell> _cells;
  int _columns;
  int _rows;
  int _cursorColumn = 0;
  int _cursorRow = 0;
  bool _inEscape = false;

  @override
  TerminalSnapshot get snapshot => TerminalSnapshot(
    columns: _columns,
    rows: _rows,
    title: '',
    cells: List<TerminalCell>.unmodifiable(_cells),
    cursor: TerminalCursor(
      column: _cursorColumn,
      row: _cursorRow,
      visible: true,
      shape: config.cursor.shape,
      color: terminalDefaultCursor,
      blinking: config.cursor.blink,
    ),
    keyboardMode: const TerminalKeyboardMode(),
    inputEchoEnabled: true,
  );

  @override
  bool get isExited => false;

  @override
  void resize(int columns, int rows, {int cellWidth = 1, int cellHeight = 1}) {
    final nextColumns = math.max(2, columns);
    final nextRows = math.max(1, rows);
    if (nextColumns == _columns && nextRows == _rows) {
      return;
    }

    final oldCells = _cells;
    final oldColumns = _columns;
    final oldRows = _rows;
    _columns = nextColumns;
    _rows = nextRows;
    _cells = _emptyCells();

    final copiedRows = math.min(oldRows, _rows);
    final copiedColumns = math.min(oldColumns, _columns);
    for (var row = 0; row < copiedRows; row++) {
      for (var column = 0; column < copiedColumns; column++) {
        _cells[row * _columns + column] = oldCells[row * oldColumns + column];
      }
    }

    _cursorColumn = math.min(_cursorColumn, _columns - 1);
    _cursorRow = math.min(_cursorRow, _rows - 1);
  }

  @override
  void write(String data) {
    for (final rune in data.runes) {
      _writeRune(rune);
    }
  }

  @override
  void writeBytes(Uint8List bytes) {
    write(utf8.decode(bytes, allowMalformed: true));
  }

  @override
  bool sendInput(String data) => false;

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
    if (selection == TerminalSelection.all(snapshot)) {
      return terminalSelectedText(
        snapshot,
        terminalVisibleTextSelection(snapshot),
      );
    }
    return terminalSelectedText(snapshot, selection);
  }

  @override
  TerminalCommandBlock? commandBlockAt(TerminalCellPosition position) {
    final selection = terminalCommandBlockAt(snapshot, position);
    return selection == null
        ? null
        : TerminalCommandBlock(selection: selection);
  }

  @override
  TerminalPromptClickMove? promptClickMove(TerminalCellPosition position) =>
      null;

  @override
  void clear() {
    _cells = _emptyCells();
    _cursorColumn = 0;
    _cursorRow = 0;
  }

  @override
  void reset() {
    clear();
    _inEscape = false;
  }

  @override
  List<TerminalConnectionEvent> drainConnectionEvents() => const [];

  @override
  bool poll() => false;

  @override
  void dispose() {}

  List<TerminalCell> _emptyCells() {
    return List<TerminalCell>.filled(
      _columns * _rows,
      const TerminalCell.empty(),
    );
  }

  void _writeRune(int rune) {
    if (_inEscape) {
      _inEscape = rune >= 0x40 && rune <= 0x7e;
      return;
    }

    switch (rune) {
      case 0x1b:
        _inEscape = true;
        return;
      case 0x08:
      case 0x7f:
        _cursorColumn = math.max(0, _cursorColumn - 1);
        _putCell(' ');
        _cursorColumn = math.max(0, _cursorColumn - 1);
        return;
      case 0x09:
        final spaces = 4 - (_cursorColumn % 4);
        for (var i = 0; i < spaces; i++) {
          _putCell(' ');
        }
        return;
      case 0x0d:
        _cursorColumn = 0;
        return;
      case 0x0a:
        _lineFeed();
        return;
      default:
        _putCell(String.fromCharCodes([rune]));
    }
  }

  void _putCell(String text) {
    _cells[_cursorRow * _columns + _cursorColumn] = TerminalCell(
      text: text,
      foreground: terminalDefaultForeground,
      background: terminalDefaultBackground,
      flags: 0,
    );
    _cursorColumn++;
    if (_cursorColumn >= _columns) {
      _cursorColumn = 0;
      _lineFeed();
    }
  }

  void _lineFeed() {
    if (_cursorRow < _rows - 1) {
      _cursorRow++;
      return;
    }

    final retainedCellCount = _cells.length - _columns;
    _cells.setRange(0, retainedCellCount, _cells, _columns);
    _cells.fillRange(
      retainedCellCount,
      _cells.length,
      const TerminalCell.empty(),
    );
  }
}
