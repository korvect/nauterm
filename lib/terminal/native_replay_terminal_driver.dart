part of 'terminal_ffi.dart';

class NativeReplayTerminalDriver implements TerminalDriver {
  NativeReplayTerminalDriver._(this._bindings, this._handle);

  factory NativeReplayTerminalDriver.create({
    required int columns,
    required int rows,
    required TerminalConfig config,
  }) {
    final bindings = _TerminalBindings.open();
    final terminalType = config.emulation.type.term.toNativeUtf8();
    final Pointer<Void> handle;
    try {
      handle = bindings.createTerminalConfigured(
        columns,
        rows,
        config.emulatorBackend.nativeValue,
        config.scrollbackLines,
        terminalType,
        _colorTermToNative(config.emulation.colorTerm),
        _osc52ModeToNative(config.osc52Mode),
        _cursorShapeToNative(config.cursor.shape),
        config.cursor.blink,
        _colorToNativeRgb(defaultTerminalTheme.primary.foreground),
        _colorToNativeRgb(defaultTerminalTheme.primary.background),
        _colorToNativeRgb(defaultTerminalTheme.cursor.cursor),
      );
    } finally {
      malloc.free(terminalType);
    }

    if (handle == nullptr) {
      throw const FfiTerminalLoadException('Unable to create replay terminal.');
    }
    return NativeReplayTerminalDriver._(bindings, handle);
  }

  final _TerminalBindings _bindings;
  Pointer<Void> _handle;

  @override
  bool get isExited => false;

  @override
  TerminalSnapshot get snapshot {
    if (_handle == nullptr) {
      return TerminalSnapshot.blank();
    }

    final snapshotPointer = _bindings.terminalSnapshot(_handle);
    if (snapshotPointer == nullptr) {
      return TerminalSnapshot.blank();
    }

    try {
      return _snapshotFromNative(snapshotPointer.ref);
    } finally {
      _bindings.freeSnapshot(snapshotPointer);
    }
  }

  String get plainText {
    if (_handle == nullptr) return '';
    final pointer = _bindings.terminalPlainText(_handle);
    if (pointer == nullptr) {
      throw const FfiTerminalLoadException(
        'Unable to export replay terminal text.',
      );
    }
    try {
      return pointer.toDartString();
    } finally {
      _bindings.freeString(pointer);
    }
  }

  @override
  void resize(int columns, int rows, {int cellWidth = 1, int cellHeight = 1}) {
    if (_handle == nullptr) {
      return;
    }
    _bindings.terminalResize(_handle, columns, rows, cellWidth, cellHeight);
  }

  @override
  void write(String data) {
    if (_handle == nullptr) {
      return;
    }

    for (final rune in data.runes) {
      _bindings.terminalWriteCodepoint(_handle, rune);
    }
  }

  @override
  void writeBytes(Uint8List bytes) {
    if (_handle == nullptr || bytes.isEmpty) {
      return;
    }

    _withNativeBytes(bytes, (pointer, length) {
      _bindings.terminalWriteBytes(_handle, pointer, length);
    });
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
  bool scrollLines(int lines) {
    if (_handle == nullptr || lines == 0) {
      return false;
    }
    return _bindings.terminalScrollLines(_handle, lines);
  }

  @override
  bool scrollPageUp() {
    if (_handle == nullptr) {
      return false;
    }
    return _bindings.terminalScrollPageUp(_handle);
  }

  @override
  bool scrollPageDown() {
    if (_handle == nullptr) {
      return false;
    }
    return _bindings.terminalScrollPageDown(_handle);
  }

  @override
  bool scrollToBottom() => scrollLines(1 << 30);

  @override
  TerminalSearchResult search(
    String query, {
    required TerminalSearchDirection direction,
    required TerminalCellPosition origin,
  }) {
    if (_handle == nullptr || query.isEmpty) {
      return const TerminalSearchResult.notFound();
    }

    final nativeQuery = query.toNativeUtf8();
    final Pointer<Utf8> resultPointer;
    try {
      resultPointer = _bindings.terminalSearch(
        _handle,
        nativeQuery,
        _searchDirectionToNative(direction),
        origin.row,
        origin.column,
      );
    } finally {
      malloc.free(nativeQuery);
    }

    return _searchResultFromNative(
      _bindings,
      resultPointer,
    ).relativeToSnapshot(snapshot);
  }

  @override
  String selectionText(TerminalSelection selection) {
    if (_handle == nullptr || selection.isCollapsed) {
      return '';
    }
    final pointer = _bindings.terminalSelectionText(
      _handle,
      selection.start,
      selection.end,
    );
    if (pointer == nullptr) {
      return '';
    }
    try {
      return pointer.toDartString();
    } finally {
      _bindings.freeString(pointer);
    }
  }

  @override
  TerminalCommandBlock? commandBlockAt(TerminalCellPosition position) {
    if (_handle == nullptr) {
      return null;
    }
    final currentSnapshot = snapshot;
    return _commandBlockFromNative(
      _bindings,
      _bindings.terminalCommandBlockAt(
        _handle,
        terminalCellOffset(currentSnapshot, position),
      ),
    );
  }

  @override
  void clear() {
    write('\x1b[2J\x1b[3J\x1b[H');
  }

  @override
  void reset() {
    write('\x1bc');
  }

  @override
  List<TerminalConnectionEvent> drainConnectionEvents() => const [];

  @override
  bool poll() => false;

  @override
  void dispose() {
    if (_handle == nullptr) {
      return;
    }
    _bindings.destroyTerminal(_handle);
    _handle = nullptr;
  }
}
