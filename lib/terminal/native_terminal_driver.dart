part of 'terminal_ffi.dart';

class _NetworkInterfaceChangeMonitor {
  static final _NetworkInterfaceChangeMonitor instance =
      _NetworkInterfaceChangeMonitor._();

  _NetworkInterfaceChangeMonitor._();

  final Set<VoidCallback> _listeners = <VoidCallback>{};
  Timer? _timer;
  String? _fingerprint;
  bool _sampling = false;

  void add(VoidCallback listener) {
    _listeners.add(listener);
    if (_timer == null) {
      _timer = Timer.periodic(const Duration(seconds: 1), (_) => _sample());
      _sample();
    }
  }

  void remove(VoidCallback listener) {
    _listeners.remove(listener);
    if (_listeners.isEmpty) {
      _timer?.cancel();
      _timer = null;
      _fingerprint = null;
    }
  }

  Future<void> _sample() async {
    if (_sampling || _listeners.isEmpty) {
      return;
    }
    _sampling = true;
    try {
      final fingerprint = await _readFingerprint();
      if (fingerprint == null) {
        return;
      }
      final previous = _fingerprint;
      _fingerprint = fingerprint;
      if (previous == null || previous == fingerprint) {
        return;
      }
      for (final listener in List<VoidCallback>.of(_listeners)) {
        try {
          listener();
        } on Object {
          // A disposed session must not stop monitoring other sessions.
        }
      }
    } finally {
      _sampling = false;
    }
  }

  Future<String?> _readFingerprint() async {
    try {
      final interfaces = await NetworkInterface.list(
        includeLoopback: true,
        includeLinkLocal: true,
      );
      final entries = <String>[];
      for (final networkInterface in interfaces) {
        for (final address in networkInterface.addresses) {
          entries.add(
            '${networkInterface.name}|${address.type}|${address.address}',
          );
        }
      }
      entries.sort();
      return entries.join('\n');
    } on Object {
      return null;
    }
  }
}

class NativeTerminalDriver implements TerminalDriver {
  NativeTerminalDriver._(this._bindings, this._sessionId);

  NativeCallable<_WakeupNative>? _wakeupCallable;

  factory NativeTerminalDriver.create({
    required int columns,
    required int rows,
    required TerminalConfig config,
    required VoidCallback onWakeup,
    String? shellPath,
    String? workingDirectory,
    Map<String, String> environment = const {},
    TerminalTheme theme = defaultTerminalTheme,
  }) {
    final bindings = _TerminalBindings.open();
    final terminalType = config.emulation.type.term.toNativeUtf8();
    final nativeShellPath = shellPath == null
        ? nullptr
        : shellPath.toNativeUtf8();
    final nativeWorkingDirectory = workingDirectory == null
        ? nullptr
        : workingDirectory.toNativeUtf8();
    final nativeEnvironment = _environmentToNative(environment);
    final int sessionId;
    try {
      sessionId = bindings.createLocalSessionConfigured(
        columns,
        rows,
        config.emulatorBackend.nativeValue,
        config.scrollbackLines,
        terminalType,
        _colorTermToNative(config.emulation.colorTerm),
        _osc52ModeToNative(config.osc52Mode),
        _cursorShapeToNative(config.cursor.shape),
        config.cursor.blink,
        _colorToNativeRgb(theme.primary.foreground),
        _colorToNativeRgb(theme.primary.background),
        _colorToNativeRgb(theme.cursor.cursor),
        nativeShellPath,
        nativeWorkingDirectory,
        nativeEnvironment,
      );
    } finally {
      malloc.free(terminalType);
      if (nativeShellPath != nullptr) {
        malloc.free(nativeShellPath);
      }
      if (nativeWorkingDirectory != nullptr) {
        malloc.free(nativeWorkingDirectory);
      }
      if (nativeEnvironment != nullptr) {
        malloc.free(nativeEnvironment);
      }
    }
    if (sessionId == 0) {
      throw const FfiTerminalLoadException(
        'Unable to create local terminal session.',
      );
    }

    final driver = NativeTerminalDriver._(bindings, sessionId);
    driver._setWakeupCallback(onWakeup);
    return driver;
  }

  factory NativeTerminalDriver.createCommand({
    required int columns,
    required int rows,
    required TerminalConfig config,
    required VoidCallback onWakeup,
    required String program,
    List<String> args = const [],
    String? workingDirectory,
    Map<String, String> environment = const {},
    TerminalTheme theme = defaultTerminalTheme,
  }) {
    final bindings = _TerminalBindings.open();
    final terminalType = config.emulation.type.term.toNativeUtf8();
    final nativeProgram = program.toNativeUtf8();
    final nativeArgs = jsonEncode(args).toNativeUtf8();
    final nativeWorkingDirectory = workingDirectory == null
        ? nullptr
        : workingDirectory.toNativeUtf8();
    final nativeEnvironment = _environmentToNative(environment);
    final int sessionId;
    try {
      sessionId = bindings.createCommandSessionConfigured(
        columns,
        rows,
        config.emulatorBackend.nativeValue,
        config.scrollbackLines,
        terminalType,
        _colorTermToNative(config.emulation.colorTerm),
        _osc52ModeToNative(config.osc52Mode),
        _cursorShapeToNative(config.cursor.shape),
        config.cursor.blink,
        _colorToNativeRgb(theme.primary.foreground),
        _colorToNativeRgb(theme.primary.background),
        _colorToNativeRgb(theme.cursor.cursor),
        nativeProgram,
        nativeArgs,
        nativeWorkingDirectory,
        nativeEnvironment,
      );
    } finally {
      malloc.free(terminalType);
      malloc.free(nativeProgram);
      malloc.free(nativeArgs);
      if (nativeWorkingDirectory != nullptr) {
        malloc.free(nativeWorkingDirectory);
      }
      if (nativeEnvironment != nullptr) {
        malloc.free(nativeEnvironment);
      }
    }
    if (sessionId == 0) {
      throw FfiTerminalLoadException('Unable to create $program session.');
    }

    final driver = NativeTerminalDriver._(bindings, sessionId);
    driver._setWakeupCallback(onWakeup);
    return driver;
  }

  factory NativeTerminalDriver.createSsh({
    required int columns,
    required int rows,
    required TerminalConfig config,
    required VoidCallback onWakeup,
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    String? password,
    String? privateKey,
    String? passphrase,
    TerminalProxyConfig? proxy,
    SshHostKeyTrustMode hostKeyTrustMode = SshHostKeyTrustMode.strict,
    Map<String, String> environment = const {},
    String encoding = 'UTF-8',
    TerminalTheme theme = defaultTerminalTheme,
  }) {
    final bindings = _TerminalBindings.open();
    final terminalType = config.emulation.type.term.toNativeUtf8();
    final nativeHost = host.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    final nativeProxy = _proxyConfigToNative(proxy);
    final nativeEnvironment = _environmentToNative(environment);
    final nativeEncoding = encoding.toNativeUtf8();
    final int sessionId;
    try {
      sessionId = bindings.createSshSessionConfigured(
        columns,
        rows,
        config.emulatorBackend.nativeValue,
        config.scrollbackLines,
        terminalType,
        _colorTermToNative(config.emulation.colorTerm),
        _osc52ModeToNative(config.osc52Mode),
        _cursorShapeToNative(config.cursor.shape),
        config.cursor.blink,
        _colorToNativeRgb(theme.primary.foreground),
        _colorToNativeRgb(theme.primary.background),
        _colorToNativeRgb(theme.cursor.cursor),
        config.sshKeepaliveIntervalSeconds,
        nativeHost,
        port,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativePassphrase,
        nativeKnownHostsPath,
        hostKeyTrustMode.wireValue,
        nativeProxy,
        nativeEnvironment,
        nativeEncoding,
      );
    } finally {
      malloc.free(terminalType);
      malloc.free(nativeHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      if (nativePassword != nullptr) {
        malloc.free(nativePassword);
      }
      if (nativePrivateKey != nullptr) {
        malloc.free(nativePrivateKey);
      }
      if (nativePassphrase != nullptr) {
        malloc.free(nativePassphrase);
      }
      if (nativeProxy != nullptr) {
        malloc.free(nativeProxy);
      }
      if (nativeEnvironment != nullptr) {
        malloc.free(nativeEnvironment);
      }
      malloc.free(nativeEncoding);
    }
    if (sessionId == 0) {
      throw const FfiTerminalLoadException('Unable to create SSH session.');
    }

    final driver = NativeTerminalDriver._(bindings, sessionId);
    driver._setWakeupCallback(onWakeup);
    return driver;
  }

  factory NativeTerminalDriver.createMosh({
    required int columns,
    required int rows,
    required TerminalConfig config,
    required VoidCallback onWakeup,
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    String? password,
    String? privateKey,
    String? passphrase,
    TerminalProxyConfig? proxy,
    SshHostKeyTrustMode hostKeyTrustMode = SshHostKeyTrustMode.strict,
    Map<String, String> environment = const {},
    String serverCommand = 'mosh-server new -s -l LANG=en_US.UTF-8',
    TerminalTheme theme = defaultTerminalTheme,
  }) {
    final bindings = _TerminalBindings.open();
    final terminalType = config.emulation.type.term.toNativeUtf8();
    final nativeHost = host.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    final nativeProxy = _proxyConfigToNative(proxy);
    final nativeEnvironment = _environmentToNative(environment);
    final normalizedServerCommand = serverCommand.trim().isEmpty
        ? 'mosh-server new -s -l LANG=en_US.UTF-8'
        : serverCommand.trim();
    final nativeServerCommand = normalizedServerCommand.toNativeUtf8();
    final int sessionId;
    try {
      sessionId = bindings.createMoshSessionConfigured(
        columns,
        rows,
        config.emulatorBackend.nativeValue,
        config.scrollbackLines,
        terminalType,
        _colorTermToNative(config.emulation.colorTerm),
        _osc52ModeToNative(config.osc52Mode),
        _cursorShapeToNative(config.cursor.shape),
        config.cursor.blink,
        _colorToNativeRgb(theme.primary.foreground),
        _colorToNativeRgb(theme.primary.background),
        _colorToNativeRgb(theme.cursor.cursor),
        nativeHost,
        port,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativePassphrase,
        nativeKnownHostsPath,
        hostKeyTrustMode.wireValue,
        nativeProxy,
        nativeEnvironment,
        nativeServerCommand,
      );
    } finally {
      malloc.free(terminalType);
      malloc.free(nativeHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      malloc.free(nativeServerCommand);
      if (nativePassword != nullptr) {
        malloc.free(nativePassword);
      }
      if (nativePrivateKey != nullptr) {
        malloc.free(nativePrivateKey);
      }
      if (nativePassphrase != nullptr) {
        malloc.free(nativePassphrase);
      }
      if (nativeProxy != nullptr) {
        malloc.free(nativeProxy);
      }
      if (nativeEnvironment != nullptr) {
        malloc.free(nativeEnvironment);
      }
    }
    if (sessionId == 0) {
      throw const FfiTerminalLoadException('Unable to create Mosh session.');
    }

    final driver = NativeTerminalDriver._(bindings, sessionId);
    driver._setWakeupCallback(onWakeup);
    driver._startNetworkChangeMonitor();
    return driver;
  }

  bool reconnectSsh({
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    required int keepaliveIntervalSeconds,
    String? password,
    String? privateKey,
    String? passphrase,
    TerminalProxyConfig? proxy,
    SshHostKeyTrustMode hostKeyTrustMode = SshHostKeyTrustMode.strict,
    String encoding = 'UTF-8',
  }) {
    if (_sessionId == 0) {
      return false;
    }
    final nativeHost = host.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    final nativeProxy = _proxyConfigToNative(proxy);
    final nativeEncoding = encoding.toNativeUtf8();
    try {
      return _bindings.reconnectSshSession(
        _sessionId,
        nativeHost,
        port,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativePassphrase,
        nativeKnownHostsPath,
        hostKeyTrustMode.wireValue,
        nativeProxy,
        keepaliveIntervalSeconds,
        nativeEncoding,
      );
    } finally {
      malloc.free(nativeHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      if (nativePassword != nullptr) malloc.free(nativePassword);
      if (nativePrivateKey != nullptr) malloc.free(nativePrivateKey);
      if (nativePassphrase != nullptr) malloc.free(nativePassphrase);
      if (nativeProxy != nullptr) malloc.free(nativeProxy);
      malloc.free(nativeEncoding);
    }
  }

  bool reconnectMosh({
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    required String serverCommand,
    String? password,
    String? privateKey,
    String? passphrase,
    TerminalProxyConfig? proxy,
    SshHostKeyTrustMode hostKeyTrustMode = SshHostKeyTrustMode.strict,
  }) {
    if (_sessionId == 0) {
      return false;
    }
    final nativeHost = host.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    final nativeProxy = _proxyConfigToNative(proxy);
    final nativeServerCommand = serverCommand.toNativeUtf8();
    try {
      return _bindings.reconnectMoshSession(
        _sessionId,
        nativeHost,
        port,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativePassphrase,
        nativeKnownHostsPath,
        hostKeyTrustMode.wireValue,
        nativeProxy,
        nativeServerCommand,
      );
    } finally {
      malloc.free(nativeHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      malloc.free(nativeServerCommand);
      if (nativePassword != nullptr) malloc.free(nativePassword);
      if (nativePrivateKey != nullptr) malloc.free(nativePrivateKey);
      if (nativePassphrase != nullptr) malloc.free(nativePassphrase);
      if (nativeProxy != nullptr) malloc.free(nativeProxy);
    }
  }

  factory NativeTerminalDriver.createTelnet({
    required int columns,
    required int rows,
    required TerminalConfig config,
    required VoidCallback onWakeup,
    required String host,
    required int port,
    String encoding = 'UTF-8',
    Map<String, String> environment = const {},
    TerminalTheme theme = defaultTerminalTheme,
  }) {
    final bindings = _TerminalBindings.open();
    final terminalType = config.emulation.type.term.toNativeUtf8();
    final nativeHost = host.toNativeUtf8();
    final nativeEncoding = encoding.toNativeUtf8();
    final nativeEnvironment = _environmentToNative(environment);
    final int sessionId;
    try {
      sessionId = bindings.createTelnetSessionConfigured(
        columns,
        rows,
        config.emulatorBackend.nativeValue,
        config.scrollbackLines,
        terminalType,
        _colorTermToNative(config.emulation.colorTerm),
        _osc52ModeToNative(config.osc52Mode),
        _cursorShapeToNative(config.cursor.shape),
        config.cursor.blink,
        _colorToNativeRgb(theme.primary.foreground),
        _colorToNativeRgb(theme.primary.background),
        _colorToNativeRgb(theme.cursor.cursor),
        nativeHost,
        port,
        nativeEncoding,
        nativeEnvironment,
      );
    } finally {
      malloc.free(terminalType);
      malloc.free(nativeHost);
      malloc.free(nativeEncoding);
      if (nativeEnvironment != nullptr) {
        malloc.free(nativeEnvironment);
      }
    }
    if (sessionId == 0) {
      throw const FfiTerminalLoadException('Unable to create telnet session.');
    }

    final driver = NativeTerminalDriver._(bindings, sessionId);
    driver._setWakeupCallback(onWakeup);
    return driver;
  }

  factory NativeTerminalDriver.createSerial({
    required int columns,
    required int rows,
    required TerminalConfig config,
    required VoidCallback onWakeup,
    required SerialConnectionConfig serialConfig,
    TerminalTheme theme = defaultTerminalTheme,
  }) {
    final bindings = _TerminalBindings.open();
    final terminalType = config.emulation.type.term.toNativeUtf8();
    final nativeSerialPort = serialConfig.serialPort.toNativeUtf8();
    final int sessionId;
    try {
      sessionId = bindings.createSerialSessionConfigured(
        columns,
        rows,
        config.emulatorBackend.nativeValue,
        config.scrollbackLines,
        terminalType,
        _colorTermToNative(config.emulation.colorTerm),
        _osc52ModeToNative(config.osc52Mode),
        _cursorShapeToNative(config.cursor.shape),
        config.cursor.blink,
        _colorToNativeRgb(theme.primary.foreground),
        _colorToNativeRgb(theme.primary.background),
        _colorToNativeRgb(theme.cursor.cursor),
        nativeSerialPort,
        serialConfig.baudRate,
        serialConfig.dataBits,
        _serialParityToNative(serialConfig.parity),
        serialConfig.stopBits,
        _serialFlowControlToNative(serialConfig.flowControl),
      );
    } finally {
      malloc.free(terminalType);
      malloc.free(nativeSerialPort);
    }
    if (sessionId == 0) {
      throw const FfiTerminalLoadException('Unable to create serial session.');
    }

    final driver = NativeTerminalDriver._(bindings, sessionId);
    driver._setWakeupCallback(onWakeup);
    return driver;
  }

  static List<SerialPortInfo> listSerialPorts() {
    final bindings = _TerminalBindings.open();
    final resultPointer = bindings.listSerialPorts();
    if (resultPointer == nullptr) {
      return const [];
    }

    try {
      final decoded = jsonDecode(resultPointer.toDartString());
      if (decoded is! List<Object?>) {
        return const [];
      }

      return decoded
          .whereType<Map<Object?, Object?>>()
          .map((port) => SerialPortInfo.fromJson(port.cast<String, Object?>()))
          .where((port) => port.path.trim().isNotEmpty)
          .toList(growable: false);
    } on Object {
      return const [];
    } finally {
      bindings.freeString(resultPointer);
    }
  }

  final _TerminalBindings _bindings;
  int _sessionId;
  int get sessionId => _sessionId;
  bool _networkMonitorRegistered = false;

  @override
  bool get isExited => _sessionId == 0 || _bindings.isExited(_sessionId);

  @override
  TerminalSnapshot get snapshot {
    if (_sessionId == 0) {
      return TerminalSnapshot.blank();
    }

    final snapshotPointer = _bindings.snapshot(_sessionId);
    if (snapshotPointer == nullptr) {
      return TerminalSnapshot.blank();
    }

    try {
      final snapshot = _snapshotFromNative(snapshotPointer.ref);
      return snapshot;
    } finally {
      _bindings.freeSnapshot(snapshotPointer);
    }
  }

  @override
  void resize(int columns, int rows, {int cellWidth = 1, int cellHeight = 1}) {
    if (_sessionId == 0) {
      return;
    }
    _bindings.resize(_sessionId, columns, rows, cellWidth, cellHeight);
  }

  bool notifyNetworkChanged() {
    if (_sessionId == 0) {
      return false;
    }
    return _bindings.notifyNetworkChanged(_sessionId);
  }

  bool exitAlternateScreen() {
    return _sessionId != 0 && _bindings.exitAlternateScreen(_sessionId);
  }

  void _startNetworkChangeMonitor() {
    if (_networkMonitorRegistered) {
      return;
    }
    _networkMonitorRegistered = true;
    _NetworkInterfaceChangeMonitor.instance.add(notifyNetworkChanged);
  }

  @override
  bool scrollLines(int lines) {
    if (_sessionId == 0 || lines == 0) {
      return false;
    }

    return _bindings.scrollLines(_sessionId, lines);
  }

  @override
  bool scrollPageUp() {
    if (_sessionId == 0) {
      return false;
    }

    return _bindings.scrollPageUp(_sessionId);
  }

  @override
  bool scrollPageDown() {
    if (_sessionId == 0) {
      return false;
    }

    return _bindings.scrollPageDown(_sessionId);
  }

  @override
  bool scrollToBottom() {
    if (_sessionId == 0) {
      return false;
    }

    return _bindings.scrollToBottom(_sessionId);
  }

  @override
  TerminalSearchResult search(
    String query, {
    required TerminalSearchDirection direction,
    required TerminalCellPosition origin,
  }) {
    if (_sessionId == 0 || query.isEmpty) {
      return const TerminalSearchResult.notFound();
    }

    final nativeQuery = query.toNativeUtf8();
    final Pointer<Utf8> resultPointer;
    try {
      resultPointer = _bindings.search(
        _sessionId,
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
    if (_sessionId == 0 || selection.isCollapsed) {
      return '';
    }
    final pointer = _bindings.selectionText(
      _sessionId,
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
    if (_sessionId == 0) {
      return null;
    }
    final currentSnapshot = snapshot;
    return _commandBlockFromNative(
      _bindings,
      _bindings.commandBlockAt(
        _sessionId,
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
  List<TerminalConnectionEvent> drainConnectionEvents() {
    if (_sessionId == 0) {
      return const [];
    }

    final eventsPointer = _bindings.drainConnectionEvents(_sessionId);
    if (eventsPointer == nullptr) {
      return const [];
    }

    try {
      final decoded = jsonDecode(eventsPointer.toDartString());
      if (decoded is! List<Object?>) {
        return const [];
      }

      return decoded
          .whereType<Map<Object?, Object?>>()
          .map(
            (event) =>
                TerminalConnectionEvent.fromJson(event.cast<String, Object?>()),
          )
          .toList(growable: false);
    } on Object {
      return const [];
    } finally {
      _bindings.freeString(eventsPointer);
    }
  }

  @override
  bool poll() {
    if (_sessionId == 0) {
      return false;
    }

    return _bindings.poll(_sessionId);
  }

  @override
  void write(String data) {
    if (_sessionId == 0) {
      return;
    }

    for (final rune in data.runes) {
      _bindings.writeCodepoint(_sessionId, rune);
    }
  }

  @override
  void writeBytes(Uint8List bytes) {
    if (_sessionId == 0 || bytes.isEmpty) {
      return;
    }

    _withNativeBytes(bytes, (pointer, length) {
      _bindings.writeSessionBytes(_sessionId, pointer, length);
    });
  }

  @override
  bool sendInput(String data) {
    return sendInputStatus(data) == TerminalInputStatus.accepted;
  }

  TerminalInputStatus sendInputStatus(String data) {
    if (_sessionId == 0) {
      return TerminalInputStatus.closed;
    }
    final bytes = Uint8List.fromList(utf8.encode(data));
    if (bytes.isEmpty) {
      return TerminalInputStatus.invalid;
    }
    final status = _withNativeBytesResult(
      bytes,
      (pointer, length) =>
          _bindings.sendInputBytesStatus(_sessionId, pointer, length),
    );
    return TerminalInputStatus.fromNative(status);
  }

  @override
  Uint8List drainOutputCapture() {
    if (_sessionId == 0) {
      return Uint8List(0);
    }

    final capturePointer = _bindings.drainOutputCapture(_sessionId);
    if (capturePointer == nullptr) {
      return Uint8List(0);
    }

    try {
      final encoded = capturePointer.toDartString();
      if (encoded.isEmpty) {
        return Uint8List(0);
      }
      return _hexDecode(encoded);
    } on Object {
      return Uint8List(0);
    } finally {
      _bindings.freeString(capturePointer);
    }
  }

  @override
  bool suppressOutputUntil(Uint8List marker) {
    if (_sessionId == 0 || marker.isEmpty) {
      return false;
    }
    return _withNativeBytesResult(
      marker,
      (pointer, length) =>
          _bindings.suppressOutputUntil(_sessionId, pointer, length),
    );
  }

  @override
  bool cancelOutputSuppression() {
    return _sessionId != 0 && _bindings.cancelOutputSuppression(_sessionId);
  }

  @override
  void dispose() {
    if (_sessionId == 0) {
      return;
    }

    if (_networkMonitorRegistered) {
      _NetworkInterfaceChangeMonitor.instance.remove(notifyNetworkChanged);
      _networkMonitorRegistered = false;
    }
    _bindings.setWakeupCallback(
      _sessionId,
      nullptr.cast<NativeFunction<_WakeupNative>>(),
      nullptr,
    );
    _bindings.close(_sessionId);
    _sessionId = 0;
    _wakeupCallable?.close();
    _wakeupCallable = null;
  }

  void _setWakeupCallback(VoidCallback callback) {
    final wakeupCallable = NativeCallable<_WakeupNative>.listener((
      Pointer<Void> _,
    ) {
      callback();
    });
    final registered = _bindings.setWakeupCallback(
      _sessionId,
      wakeupCallable.nativeFunction,
      nullptr,
    );
    if (!registered) {
      wakeupCallable.close();
      _bindings.close(_sessionId);
      _sessionId = 0;
      throw const FfiTerminalLoadException(
        'Unable to register the terminal wakeup callback.',
      );
    }
    _wakeupCallable = wakeupCallable;
  }

  TerminalSnapshot _snapshotFromNative(_NativeTerminalSnapshot native) {
    final titleBytes = native.titleLength == 0
        ? Uint8List(0)
        : native.title.asTypedList(native.titleLength);
    final title = titleBytes.isEmpty
        ? ''
        : utf8.decode(titleBytes, allowMalformed: true);
    final clipboardBytes = native.clipboardLength == 0
        ? Uint8List(0)
        : native.clipboard.asTypedList(native.clipboardLength);
    final clipboard = clipboardBytes.isEmpty
        ? ''
        : utf8.decode(clipboardBytes, allowMalformed: true);
    final textBytes = native.textLength == 0
        ? Uint8List(0)
        : native.text.asTypedList(native.textLength);
    final hyperlinkBytes = native.hyperlinkTextLength == 0
        ? Uint8List(0)
        : native.hyperlinkText.asTypedList(native.hyperlinkTextLength);
    final cells = List<TerminalCell>.generate(native.cellsLength, (index) {
      final cell = (native.cells + index).ref;
      final start = cell.textOffset;
      final end = start + cell.textLength;
      final hyperlinkStart = cell.hyperlinkOffset;
      final hyperlinkEnd = hyperlinkStart + cell.hyperlinkLength;
      final text = start >= 0 && end <= textBytes.length && end >= start
          ? utf8.decode(Uint8List.sublistView(textBytes, start, end))
          : ' ';
      final hyperlink =
          hyperlinkStart >= 0 &&
              hyperlinkEnd <= hyperlinkBytes.length &&
              hyperlinkEnd >= hyperlinkStart
          ? utf8.decode(
              Uint8List.sublistView(
                hyperlinkBytes,
                hyperlinkStart,
                hyperlinkEnd,
              ),
              allowMalformed: true,
            )
          : '';

      return TerminalCell(
        text: text.isEmpty ? ' ' : text,
        foreground: _colorFromRgb(cell.foreground),
        background: _colorFromRgb(cell.background),
        flags: cell.flags,
        hyperlink: hyperlink,
      );
    }, growable: false);
    final graphics = _graphicsFromNative(native);

    return TerminalSnapshot(
      emulatorBackend: _emulatorBackendFromNative(native.emulatorBackend),
      graphicImages: graphics.$1,
      graphicPlacements: graphics.$2,
      columns: native.columns,
      rows: native.rows,
      historyLines: native.historyLines,
      displayOffset: native.displayOffset,
      title: title,
      clipboardText: clipboard,
      bellCount: native.bellCount,
      cells: cells,
      cursor: TerminalCursor(
        column: native.cursorColumn,
        row: native.cursorRow,
        visible: native.cursorVisible != 0,
        shape: _cursorShapeFromNative(native.cursorShape),
        color: _colorFromRgb(native.cursorColor),
        blinking: native.cursorBlinking != 0,
      ),
      keyboardMode: _keyboardModeFromNative(native.keyboardMode),
      inputEchoEnabled: native.inputEchoEnabled != 0,
    );
  }
}

Pointer<Utf8> _environmentToNative(Map<String, String> environment) {
  final entries = [
    for (final entry in environment.entries)
      if (entry.key.trim().isNotEmpty)
        {'variable': entry.key.trim(), 'value': entry.value},
  ];
  return jsonEncode(entries).toNativeUtf8();
}
