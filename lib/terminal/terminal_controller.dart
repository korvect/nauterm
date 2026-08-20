import 'dart:async';
import 'dart:convert';

import 'package:characters/characters.dart';
import 'package:flutter/foundation.dart';

import '../app/nauterm_log.dart';
import 'terminal_driver.dart';
import 'terminal_config.dart';
import 'terminal_capture_sanitizer.dart';
import 'terminal_ffi.dart';
import 'terminal_models.dart';
import 'terminal_recording.dart';
import 'terminal_selection.dart';
import 'terminal_sensitive_input.dart';
import 'terminal_shell_integration.dart';
import 'terminal_ssh_prediction.dart';
import 'terminal_theme.dart';
import 'terminal_text_width.dart';

typedef TerminalInputSink = void Function(String data);

enum MoshPredictionStrategy { text, backspace }

@immutable
class MoshPredictionDebugBatch {
  const MoshPredictionDebugBatch({
    required this.batchId,
    required this.startColumn,
    required this.startRow,
    required this.strategy,
    required this.text,
    this.inputStateNum,
    this.inputAcked = false,
    this.snapshotCommitted = false,
    this.latencyMs,
  });

  final int batchId;
  final int startColumn;
  final int startRow;
  final MoshPredictionStrategy strategy;
  final String text;
  final int? inputStateNum;
  final bool inputAcked;
  final bool snapshotCommitted;
  final double? latencyMs;
}

class _PendingPredictionBatch {
  _PendingPredictionBatch({
    required this.batchId,
    required this.startColumn,
    required this.startRow,
    required this.strategy,
    required this.latencyMs,
    required this.isBackspace,
    this.expectedCursor,
  });

  final int batchId;
  final int startColumn;
  final int startRow;
  final MoshPredictionStrategy strategy;
  final double? latencyMs;
  final bool isBackspace;
  final _PredictionCursorPosition? expectedCursor;
  int? inputStateNum;
  bool inputAcked = false;
  bool snapshotCommitted = false;
}

enum _PredictionSnapshotState { confirmed, pending, mismatch }

class _ConfirmedPredictionLineState {
  const _ConfirmedPredictionLineState({
    required this.row,
    required this.nextColumn,
    required this.strategy,
  });

  final int row;
  final int nextColumn;
  final MoshPredictionStrategy strategy;
}

class _PredictionCursorPosition {
  const _PredictionCursorPosition({required this.column, required this.row});

  final int column;
  final int row;
}

class _PendingPredictionUnit {
  const _PendingPredictionUnit({
    required this.grapheme,
    required this.cellWidth,
    required this.batchId,
  });

  final String grapheme;
  final int cellWidth;
  final int batchId;
}

enum TerminalConnectionPhase {
  idle,
  connecting,
  hostKey,
  authentication,
  connected,
  exited,
  failed,
}

const Object _preserveSshProfileValue = Object();

@immutable
class TerminalConnectionStatus {
  const TerminalConnectionStatus({
    required this.phase,
    this.message,
    this.exitCode,
  });

  const TerminalConnectionStatus.idle()
    : phase = TerminalConnectionPhase.idle,
      message = null,
      exitCode = null;

  final TerminalConnectionPhase phase;
  final String? message;
  final int? exitCode;

  bool get isTerminal =>
      phase == TerminalConnectionPhase.exited ||
      phase == TerminalConnectionPhase.failed;
}

@immutable
class SshConnectionProfile {
  const SshConnectionProfile({
    required this.host,
    required this.port,
    required this.username,
    required this.knownHostsPath,
    this.hostId,
    this.identityId,
    this.label,
    this.password,
    this.privateKey,
    this.passphrase,
    this.proxy,
    this.shellPath,
    this.environment = const {},
    this.encoding = 'UTF-8',
  });

  final String host;
  final int port;
  final String username;
  final String knownHostsPath;
  final int? hostId;
  final int? identityId;
  final String? label;
  final String? password;
  final String? privateKey;
  final String? passphrase;
  final TerminalProxyConfig? proxy;
  final String? shellPath;
  final Map<String, String> environment;
  final String encoding;

  SshConnectionProfile copyWith({
    String? host,
    int? port,
    String? username,
    String? knownHostsPath,
    int? hostId,
    int? identityId,
    Object? label = _preserveSshProfileValue,
    Object? password = _preserveSshProfileValue,
    Object? privateKey = _preserveSshProfileValue,
    Object? passphrase = _preserveSshProfileValue,
    Object? proxy = _preserveSshProfileValue,
    Object? shellPath = _preserveSshProfileValue,
    Map<String, String>? environment,
    String? encoding,
  }) {
    return SshConnectionProfile(
      host: host ?? this.host,
      port: port ?? this.port,
      username: username ?? this.username,
      knownHostsPath: knownHostsPath ?? this.knownHostsPath,
      hostId: hostId ?? this.hostId,
      identityId: identityId ?? this.identityId,
      label: identical(label, _preserveSshProfileValue)
          ? this.label
          : label as String?,
      password: identical(password, _preserveSshProfileValue)
          ? this.password
          : password as String?,
      privateKey: identical(privateKey, _preserveSshProfileValue)
          ? this.privateKey
          : privateKey as String?,
      passphrase: identical(passphrase, _preserveSshProfileValue)
          ? this.passphrase
          : passphrase as String?,
      proxy: identical(proxy, _preserveSshProfileValue)
          ? this.proxy
          : proxy as TerminalProxyConfig?,
      shellPath: identical(shellPath, _preserveSshProfileValue)
          ? this.shellPath
          : shellPath as String?,
      environment: environment ?? this.environment,
      encoding: encoding ?? this.encoding,
    );
  }
}

@immutable
class SerialConnectionProfile {
  const SerialConnectionProfile({required this.config, this.label});

  final SerialConnectionConfig config;
  final String? label;

  String get serialPort => config.serialPort;
  int get baudRate => config.baudRate;
  int get dataBits => config.dataBits;
  SerialParity get parity => config.parity;
  int get stopBits => config.stopBits;
  SerialFlowControl get flowControl => config.flowControl;
}

@immutable
class TelnetConnectionProfile {
  const TelnetConnectionProfile({
    required this.host,
    required this.port,
    this.hostId,
    this.identityId,
    this.label,
    this.username,
    this.password,
    this.encoding = 'UTF-8',
  });

  final String host;
  final int port;
  final int? hostId;
  final int? identityId;
  final String? label;
  final String? username;
  final String? password;
  final String encoding;
}

enum _RemoteTerminalTransport { ssh, mosh }

enum MoshNetworkState { stable, switching, degraded, restored }

class TerminalController extends ChangeNotifier {
  static const Duration _driverWatchdogInterval = Duration(seconds: 2);
  static const Duration _snapshotRefreshInterval = Duration(milliseconds: 16);
  static const double _moshPredictionAdaptiveEnableLatencyMs = 80;
  static const double _moshPredictionAdaptiveDisableLatencyMs = 40;
  static const double _sshPredictionAdaptiveEnableLatencyMs = 80;
  static const double _sshPredictionAdaptiveDisableLatencyMs = 40;
  static const Duration _sshPredictionExpiry = Duration(seconds: 5);

  TerminalController({
    TerminalDriver? driver,
    this._onInput,
    this._onExit,
    int initialColumns = 80,
    int initialRows = 24,
    String? shellPath,
    String? workingDirectory,
    this._startupSnippet,
    Map<String, String> environment = const {},
    TerminalTheme theme = defaultTerminalTheme,
    this._recorder,
    this._onInputSent,
    this.config = defaultTerminalConfig,
  }) : _onConnected = null,
       _remoteTransport = null,
       _moshServerCommand = 'mosh-server new -s -l LANG=en_US.UTF-8',
       _shellPath = shellPath,
       _theme = theme,
       _connectionStatus = const TerminalConnectionStatus(
         phase: TerminalConnectionPhase.connected,
         message: 'Local terminal is ready.',
       ) {
    _driver =
        driver ??
        _createDefaultDriver(
          columns: initialColumns,
          rows: initialRows,
          config: config,
          onWakeup: _handleDriverWakeup,
          shellPath: shellPath,
          workingDirectory: workingDirectory,
          environment: environment,
          theme: theme,
        );
    _snapshot = _driver.snapshot;
    _startDriverWatchdog();
    _recorder?.recordSnapshot(_snapshot);
    _scheduleStartupSnippetIfNeeded();
  }

  TerminalController.ssh({
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    int? hostId,
    int? identityId,
    String? label,
    String? password,
    String? privateKey,
    String? passphrase,
    TerminalProxyConfig? proxy,
    String? shellPath,
    this._startupSnippet,
    Map<String, String> environment = const {},
    String encoding = 'UTF-8',
    TerminalTheme theme = defaultTerminalTheme,
    this._recorder,
    this._onExit,
    this._onConnected,
    this._onInputSent,
    int initialColumns = 80,
    int initialRows = 24,
    TerminalDriver? driver,
    this.config = defaultTerminalConfig,
  }) : _onInput = null,
       _remoteTransport = _RemoteTerminalTransport.ssh,
       _moshServerCommand = 'mosh-server new -s -l LANG=en_US.UTF-8',
       _shellPath = shellPath,
       _theme = theme,
       _sshProfile = SshConnectionProfile(
         host: host,
         port: port,
         username: username,
         knownHostsPath: knownHostsPath,
         hostId: hostId,
         identityId: identityId,
         label: label,
         password: password,
         privateKey: privateKey,
         passphrase: passphrase,
         proxy: proxy,
         shellPath: shellPath,
         environment: environment,
         encoding: encoding,
       ),
       _connectionStatus = TerminalConnectionStatus(
         phase: TerminalConnectionPhase.connecting,
         message: 'Connecting to $username@$host:$port...',
       ) {
    _driver =
        driver ??
        _createSshDriver(
          columns: initialColumns,
          rows: initialRows,
          config: config,
          onWakeup: _handleDriverWakeup,
          host: host,
          port: port,
          username: username,
          knownHostsPath: knownHostsPath,
          password: password,
          privateKey: privateKey,
          passphrase: passphrase,
          proxy: proxy,
          environment: environment,
          encoding: encoding,
          theme: theme,
        );
    _snapshot = _driver.snapshot;
    _startDriverWatchdog();
    _recorder?.recordSnapshot(_snapshot);
    _drainConnectionEvents();
  }

  TerminalController.mosh({
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    int? hostId,
    int? identityId,
    String? label,
    String? password,
    String? privateKey,
    String? passphrase,
    TerminalProxyConfig? proxy,
    String? shellPath,
    this._startupSnippet,
    Map<String, String> environment = const {},
    String serverCommand = 'mosh-server new -s -l LANG=en_US.UTF-8',
    TerminalTheme theme = defaultTerminalTheme,
    this._recorder,
    this._onExit,
    this._onConnected,
    this._onInputSent,
    int initialColumns = 80,
    int initialRows = 24,
    TerminalDriver? driver,
    this.config = defaultTerminalConfig,
  }) : _onInput = null,
       _remoteTransport = _RemoteTerminalTransport.mosh,
       _shellPath = shellPath,
       _theme = theme,
       _moshServerCommand = serverCommand,
       _sshProfile = SshConnectionProfile(
         host: host,
         port: port,
         username: username,
         knownHostsPath: knownHostsPath,
         hostId: hostId,
         identityId: identityId,
         label: label,
         password: password,
         privateKey: privateKey,
         passphrase: passphrase,
         proxy: proxy,
         shellPath: shellPath,
         environment: environment,
       ),
       _connectionStatus = TerminalConnectionStatus(
         phase: TerminalConnectionPhase.connecting,
         message: 'Connecting to $username@$host:$port over Mosh...',
       ) {
    _driver =
        driver ??
        _createMoshDriver(
          columns: initialColumns,
          rows: initialRows,
          config: config,
          onWakeup: _handleDriverWakeup,
          host: host,
          port: port,
          username: username,
          knownHostsPath: knownHostsPath,
          password: password,
          privateKey: privateKey,
          passphrase: passphrase,
          proxy: proxy,
          environment: environment,
          serverCommand: serverCommand,
          theme: theme,
        );
    _snapshot = _driver.snapshot;
    _startDriverWatchdog();
    _recorder?.recordSnapshot(_snapshot);
    _drainConnectionEvents();
  }

  TerminalController.serial({
    required SerialConnectionConfig serialConfig,
    String? label,
    this._recorder,
    this._onExit,
    this._onInputSent,
    int initialColumns = 80,
    int initialRows = 24,
    this.config = defaultTerminalConfig,
    TerminalTheme theme = defaultTerminalTheme,
  }) : _onInput = null,
       _onConnected = null,
       _remoteTransport = null,
       _moshServerCommand = 'mosh-server new -s -l LANG=en_US.UTF-8',
       _shellPath = null,
       _startupSnippet = null,
       _theme = theme,
       _serialProfile = SerialConnectionProfile(
         config: serialConfig,
         label: label,
       ),
       _connectionStatus = TerminalConnectionStatus(
         phase: TerminalConnectionPhase.connecting,
         message:
             'Opening ${serialConfig.serialPort} at ${serialConfig.summary}...',
       ) {
    _driver = _createSerialDriver(
      columns: initialColumns,
      rows: initialRows,
      config: config,
      onWakeup: _handleDriverWakeup,
      serialConfig: serialConfig,
      theme: theme,
    );
    _snapshot = _driver.snapshot;
    _startDriverWatchdog();
    _recorder?.recordSnapshot(_snapshot);
    _drainConnectionEvents();
  }

  TerminalController.telnet({
    required String host,
    required int port,
    int? hostId,
    int? identityId,
    String? label,
    String? username,
    String? password,
    String encoding = 'UTF-8',
    this._startupSnippet,
    Map<String, String> environment = const {},
    TerminalTheme theme = defaultTerminalTheme,
    this._recorder,
    this._onExit,
    this._onInputSent,
    int initialColumns = 80,
    int initialRows = 24,
    this.config = defaultTerminalConfig,
  }) : _onInput = null,
       _onConnected = null,
       _remoteTransport = null,
       _moshServerCommand = 'mosh-server new -s -l LANG=en_US.UTF-8',
       _shellPath = null,
       _theme = theme,
       _telnetProfile = TelnetConnectionProfile(
         host: host,
         port: port,
         hostId: hostId,
         identityId: identityId,
         label: label,
         username: username,
         password: password,
         encoding: encoding,
       ),
       _connectionStatus = TerminalConnectionStatus(
         phase: TerminalConnectionPhase.connecting,
         message: 'Connecting to $host:$port over Telnet...',
       ) {
    _telnetAutoLogin = _TelnetAutoLogin(
      username: username,
      password: password,
      sendSensitiveInput: (data) => sendInput(data, sensitive: true),
      onAuthenticated: _scheduleStartupSnippetIfNeeded,
    );
    _driver = _createTelnetDriver(
      columns: initialColumns,
      rows: initialRows,
      config: config,
      onWakeup: _handleDriverWakeup,
      host: host,
      port: port,
      encoding: encoding,
      environment: environment,
      theme: theme,
    );
    _snapshot = _driver.snapshot;
    _startDriverWatchdog();
    _recorder?.recordSnapshot(_snapshot);
    _drainConnectionEvents();
  }

  late TerminalDriver _driver;
  final TerminalConfig config;
  final TerminalInputSink? _onInput;
  final TerminalInputSink? _onInputSent;
  final VoidCallback? _onExit;
  final VoidCallback? _onConnected;
  final _RemoteTerminalTransport? _remoteTransport;
  final TerminalSessionRecorder? _recorder;
  final String? _shellPath;
  final String? _startupSnippet;
  final TerminalTheme _theme;
  String _moshServerCommand;
  SshConnectionProfile? _sshProfile;
  SerialConnectionProfile? _serialProfile;
  TelnetConnectionProfile? _telnetProfile;
  _TelnetAutoLogin? _telnetAutoLogin;
  TerminalConnectionStatus _connectionStatus;
  late TerminalSnapshot _snapshot;
  bool _snapshotRefreshQueued = false;
  final Stopwatch _snapshotRefreshClock = Stopwatch()..start();
  Duration _lastSnapshotRefreshAt = Duration.zero;
  Timer? _driverWatchdogTimer;
  bool _driverWakeupPending = false;
  Timer? _snapshotRefreshTimer;
  bool _disposed = false;
  bool _exitNotified = false;
  bool _connectedCallbackNotified = false;
  bool _hasConnectedOnce = false;
  bool _reconnectBoundaryWritten = false;
  static const _moshConnectedRevealDelay = Duration(milliseconds: 450);
  Timer? _moshConnectedRevealTimer;
  bool _sensitiveInputInProgress = false;
  bool _activeExitRequested = false;
  String _pendingInputLine = '';
  final List<TerminalConnectionEvent> _connectionEvents = [];
  final Set<ValueChanged<Uint8List>> _outputListeners = {};
  final Set<TerminalInputSink> _inputSentListeners = {};
  final Set<VoidCallback> _disposeListeners = {};
  final TerminalCaptureSanitizer _captureSanitizer = TerminalCaptureSanitizer();
  final TerminalShellAnnouncementParser _shellAnnouncementParser =
      TerminalShellAnnouncementParser();
  final List<_PendingPredictionUnit> _moshPendingPrediction =
      <_PendingPredictionUnit>[];
  final Map<int, _PendingPredictionBatch> _moshPredictionBatches =
      <int, _PendingPredictionBatch>{};
  bool _moshPredictionEnabled = false;
  int _moshPredictionBatchId = 0;
  bool _moshScreenCommitPending = false;
  bool _moshPredictionPausedUntilScreenUpdate = false;
  bool _moshAdaptivePredictionEnabled = true;
  double? _moshLatencyMs;
  double? _sshLatencyMs;
  final SshLocalPredictionState _sshPrediction = SshLocalPredictionState();
  bool _sshAdaptivePredictionEnabled = false;
  Timer? _sshPredictionExpiryTimer;
  int? _moshCommittedScreenStateNum;
  _ConfirmedPredictionLineState? _moshConfirmedLineState;
  MoshNetworkState _moshNetworkState = MoshNetworkState.stable;
  Timer? _moshNetworkRestoredTimer;

  TerminalSnapshot get snapshot => _snapshot;
  String get selectedText => _selectedText;
  TerminalCommandBlock? get selectedCommandBlock => _selectedCommandBlock;

  String selectionText(TerminalSelection selection) {
    return _driver.selectionText(selection);
  }

  TerminalCommandBlock? commandBlockAt(TerminalCellPosition position) {
    return _driver.commandBlockAt(position);
  }

  TerminalPromptClickMove? promptClickMove(TerminalCellPosition position) {
    return _driver.promptClickMove(position);
  }

  bool get isLocalTerminal =>
      _sshProfile == null && _serialProfile == null && _telnetProfile == null;

  bool get supportsShellIntegration => _driver is NativeTerminalDriver;
  String? get shellIntegrationToken => switch (_driver) {
    final NativeTerminalDriver driver => driver.shellIntegrationToken,
    _ => null,
  };

  String _selectedText = '';
  TerminalCommandBlock? _selectedCommandBlock;

  void updateSelectedText(String value) {
    if (_selectedText == value) {
      return;
    }
    _selectedText = value;
    notifyListeners();
  }

  void updateSelectedCommandBlock(TerminalCommandBlock? block) {
    if (_selectedCommandBlock == block) {
      return;
    }
    _selectedCommandBlock = block;
    notifyListeners();
  }

  TerminalConnectionStatus get connectionStatus => _connectionStatus;
  SshConnectionProfile? get sshProfile => _sshProfile;
  SerialConnectionProfile? get serialProfile => _serialProfile;
  TelnetConnectionProfile? get telnetProfile => _telnetProfile;
  String? get shellPath => _shellPath;
  String? get reportedShellPath => _reportedShellPath;
  String? _reportedShellPath;
  bool get shouldCloseOnExit => _activeExitRequested;
  bool get hasConnectedOnce => _hasConnectedOnce;
  bool get showsReconnectStatus {
    if (_sshProfile == null || !_hasConnectedOnce || _activeExitRequested) {
      return false;
    }
    return switch (_connectionStatus.phase) {
      TerminalConnectionPhase.connecting ||
      TerminalConnectionPhase.exited ||
      TerminalConnectionPhase.failed => true,
      _ => false,
    };
  }

  bool get isDisposed => _disposed;
  TerminalSessionRecording? get sessionRecording => _recorder?.snapshot();
  List<TerminalConnectionEvent> get connectionEvents =>
      List.unmodifiable(_connectionEvents);
  double? get sshLatencyMs => _sshLatencyMs;
  int? get nativeSessionId {
    final driver = _driver;
    return driver is NativeTerminalDriver ? driver.sessionId : null;
  }

  bool get isMoshSession => _remoteTransport == _RemoteTerminalTransport.mosh;
  bool get isSshSession => _remoteTransport == _RemoteTerminalTransport.ssh;
  String get moshPrediction =>
      _moshPendingPrediction.map((unit) => unit.grapheme).join();
  String get sshPrediction => _sshPrediction.text;
  String get localPrediction => isMoshSession ? moshPrediction : sshPrediction;

  @visibleForTesting
  List<SshPredictionDebugBatch> get debugSshPredictionBatches =>
      _sshPrediction.debugBatches;
  @visibleForTesting
  List<MoshPredictionDebugBatch> get debugMoshPredictionBatches {
    final ordered = <int>[];
    for (final unit in _moshPendingPrediction) {
      if (!ordered.contains(unit.batchId)) {
        ordered.add(unit.batchId);
      }
    }
    return ordered
        .map((batchId) {
          final batch = _moshPredictionBatches[batchId];
          if (batch == null) {
            return null;
          }
          final text = _moshPendingPrediction
              .where((unit) => unit.batchId == batchId)
              .map((unit) => unit.grapheme)
              .join();
          return MoshPredictionDebugBatch(
            batchId: batch.batchId,
            startColumn: batch.startColumn,
            startRow: batch.startRow,
            strategy: batch.strategy,
            text: text,
            inputStateNum: batch.inputStateNum,
            inputAcked: batch.inputAcked,
            snapshotCommitted: batch.snapshotCommitted,
            latencyMs: batch.latencyMs,
          );
        })
        .whereType<MoshPredictionDebugBatch>()
        .toList(growable: false);
  }

  @visibleForTesting
  double? get debugMoshLatencyMs => _moshLatencyMs;

  @visibleForTesting
  int? get debugMoshCommittedScreenStateNum => _moshCommittedScreenStateNum;

  MoshNetworkState get moshNetworkState => _moshNetworkState;

  void addOutputListener(ValueChanged<Uint8List> listener) {
    if (!_disposed) {
      _outputListeners.add(listener);
    }
  }

  void removeOutputListener(ValueChanged<Uint8List> listener) {
    _outputListeners.remove(listener);
  }

  void addInputSentListener(TerminalInputSink listener) {
    if (!_disposed) {
      _inputSentListeners.add(listener);
    }
  }

  void removeInputSentListener(TerminalInputSink listener) {
    _inputSentListeners.remove(listener);
  }

  bool suppressOutputUntil(String marker) {
    if (_disposed || marker.isEmpty) {
      return false;
    }
    final markerBytes = Uint8List.fromList(utf8.encode(marker));
    final suppressed = _driver.suppressOutputUntil(markerBytes);
    if (suppressed) {
      _captureSanitizer.suppressUntil(markerBytes);
    }
    return suppressed;
  }

  void cancelOutputSuppression() {
    _captureSanitizer.cancelSuppression();
    if (!_disposed && _driver.cancelOutputSuppression()) {
      _refresh();
    }
  }

  void addDisposeListener(VoidCallback listener) {
    if (!_disposed) {
      _disposeListeners.add(listener);
    }
  }

  void removeDisposeListener(VoidCallback listener) {
    _disposeListeners.remove(listener);
  }

  void resize(int columns, int rows, {int cellWidth = 1, int cellHeight = 1}) {
    if (_disposed) {
      return;
    }
    _driver.resize(columns, rows, cellWidth: cellWidth, cellHeight: cellHeight);
    _refresh();
  }

  void write(String data) {
    if (_disposed) {
      return;
    }
    _driver.write(data);
    _emitOutput(Uint8List.fromList(utf8.encode(data)));
    _recorder?.recordOutput(data);
    _drainOutputCapture();
    _refresh();
  }

  void writeBytes(Uint8List bytes, {bool refresh = true}) {
    if (_disposed || bytes.isEmpty) {
      return;
    }
    _driver.writeBytes(bytes);
    _emitOutput(bytes);
    _drainOutputCapture();
    if (refresh) {
      _refresh();
    }
  }

  void refreshSnapshot() {
    if (_disposed) {
      return;
    }
    _refresh();
  }

  void scrollLines(int lines) {
    if (_disposed || lines == 0) {
      return;
    }
    if (_driver.scrollLines(lines)) {
      _refresh();
    }
  }

  void scrollPageUp() {
    if (_disposed) {
      return;
    }
    if (_driver.scrollPageUp()) {
      _refresh();
    }
  }

  void scrollPageDown() {
    if (_disposed) {
      return;
    }
    if (_driver.scrollPageDown()) {
      _refresh();
    }
  }

  void scrollToBottom() {
    if (_disposed) {
      return;
    }
    if (_driver.scrollToBottom()) {
      _refresh();
    }
  }

  TerminalSearchResult search(
    String query, {
    required TerminalSearchDirection direction,
    required TerminalCellPosition origin,
  }) {
    if (_disposed || query.isEmpty) {
      return const TerminalSearchResult.notFound();
    }

    final result = _driver.search(query, direction: direction, origin: origin);
    if (result.found) {
      _refresh();
    }
    return result;
  }

  void clear() {
    if (_disposed) {
      return;
    }
    _driver.clear();
    _refresh();
  }

  void reset() {
    if (_disposed) {
      return;
    }
    _driver.reset();
    _refresh();
  }

  bool reconnectSsh({
    SshConnectionProfile? profile,
    String? host,
    int? port,
    String? username,
    int? identityId,
    Object? password = _preserveSshProfileValue,
    Object? privateKey = _preserveSshProfileValue,
    Object? passphrase = _preserveSshProfileValue,
    String? moshServerCommand,
    SshHostKeyTrustMode hostKeyTrustMode = SshHostKeyTrustMode.strict,
  }) {
    if (_disposed) {
      return false;
    }

    final currentProfile = _sshProfile;
    if (currentProfile == null) {
      return false;
    }
    if (_remoteTransport == _RemoteTerminalTransport.mosh) {
      // A reconnect creates a new Mosh transport state. Predictions from the
      // previous transport cannot be reconciled against its first snapshot.
      _moshPredictionPausedUntilScreenUpdate = true;
      _moshConfirmedLineState = null;
      _moshCommittedScreenStateNum = null;
      _clearMoshPrediction();
    }

    final nextProfile = (profile ?? currentProfile).copyWith(
      host: host,
      port: port,
      username: username,
      identityId: identityId,
      password: password,
      privateKey: privateKey,
      passphrase: passphrase,
    );
    if (moshServerCommand != null) {
      _moshServerCommand = moshServerCommand;
    }
    if (_hasConnectedOnce && !_reconnectBoundaryWritten) {
      _reconnectBoundaryWritten = true;
      final driver = _driver;
      if (driver is NativeTerminalDriver) {
        driver.exitAlternateScreen();
      }
      write('\r\n\x1b[2m[Connection lost. Reconnecting...]\x1b[0m\r\n');
    }
    _sshProfile = nextProfile;
    _exitNotified = false;
    _connectedCallbackNotified = false;
    _activeExitRequested = false;
    _pendingInputLine = '';
    _startupSnippetSent = false;
    _sshLatencyMs = null;
    _sshAdaptivePredictionEnabled = false;
    _clearSshPrediction();
    _connectionEvents.clear();
    _addConnectionEvent(
      TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.retry,
        message:
            'Retrying ${nextProfile.username}@${nextProfile.host}:${nextProfile.port}.',
        host: nextProfile.host,
        port: nextProfile.port,
        username: nextProfile.username,
      ),
    );
    _connectionStatus = TerminalConnectionStatus(
      phase: TerminalConnectionPhase.connecting,
      message:
          'Connecting to ${nextProfile.username}@${nextProfile.host}:${nextProfile.port}'
          '${_remoteTransport == _RemoteTerminalTransport.mosh ? ' over Mosh' : ''}...',
    );
    final driver = _driver;
    final reconnectedInPlace = driver is NativeTerminalDriver
        ? _remoteTransport == _RemoteTerminalTransport.mosh
              ? driver.reconnectMosh(
                  host: nextProfile.host,
                  port: nextProfile.port,
                  username: nextProfile.username,
                  knownHostsPath: nextProfile.knownHostsPath,
                  password: nextProfile.password,
                  privateKey: nextProfile.privateKey,
                  passphrase: nextProfile.passphrase,
                  proxy: nextProfile.proxy,
                  hostKeyTrustMode: hostKeyTrustMode,
                  serverCommand: _moshServerCommand,
                )
              : driver.reconnectSsh(
                  host: nextProfile.host,
                  port: nextProfile.port,
                  username: nextProfile.username,
                  knownHostsPath: nextProfile.knownHostsPath,
                  password: nextProfile.password,
                  privateKey: nextProfile.privateKey,
                  passphrase: nextProfile.passphrase,
                  proxy: nextProfile.proxy,
                  hostKeyTrustMode: hostKeyTrustMode,
                  keepaliveIntervalSeconds: config.sshKeepaliveIntervalSeconds,
                  encoding: nextProfile.encoding,
                )
        : false;
    if (reconnectedInPlace) {
      _refresh();
      return true;
    }

    _driver.dispose();
    if (_remoteTransport == _RemoteTerminalTransport.mosh) {
      _driver = _createMoshDriver(
        columns: _snapshot.columns,
        rows: _snapshot.rows,
        config: config,
        onWakeup: _handleDriverWakeup,
        host: nextProfile.host,
        port: nextProfile.port,
        username: nextProfile.username,
        knownHostsPath: nextProfile.knownHostsPath,
        password: nextProfile.password,
        privateKey: nextProfile.privateKey,
        passphrase: nextProfile.passphrase,
        proxy: nextProfile.proxy,
        hostKeyTrustMode: hostKeyTrustMode,
        environment: nextProfile.environment,
        serverCommand: _moshServerCommand,
        theme: _theme,
      );
    } else {
      _driver = _createSshDriver(
        columns: _snapshot.columns,
        rows: _snapshot.rows,
        config: config,
        onWakeup: _handleDriverWakeup,
        host: nextProfile.host,
        port: nextProfile.port,
        username: nextProfile.username,
        knownHostsPath: nextProfile.knownHostsPath,
        password: nextProfile.password,
        privateKey: nextProfile.privateKey,
        passphrase: nextProfile.passphrase,
        proxy: nextProfile.proxy,
        hostKeyTrustMode: hostKeyTrustMode,
        environment: nextProfile.environment,
        encoding: nextProfile.encoding,
        theme: _theme,
      );
    }
    _startDriverWatchdog();
    _refresh();
    return true;
  }

  void dismissConnectionStatus() {
    if (_disposed || !_connectionStatus.isTerminal) {
      return;
    }

    _connectionStatus = const TerminalConnectionStatus.idle();
    notifyListeners();
  }

  void sendInput(String data, {bool sensitive = false}) {
    if (_disposed) {
      return;
    }
    final effectiveSensitive =
        sensitive ||
        _sensitiveInputInProgress ||
        terminalInputIsSensitive(_driver.snapshot);
    if (effectiveSensitive) {
      _recorder?.recordInput(data, sensitive: true);
    } else {
      _recordInputForExitIntent(data);
      _recorder?.recordInput(data);
      _onInputSent?.call(data);
      for (final listener in List.of(_inputSentListeners)) {
        listener(data);
      }
    }
    final inputSink = _onInput;
    if (inputSink != null) {
      inputSink(data);
      return;
    }

    final inputStatus = _sendInputStatus(data);
    if (isMoshSession &&
        !effectiveSensitive &&
        inputStatus == TerminalInputStatus.accepted) {
      final start = _currentMoshPredictionCursor();
      final strategy = _predictionStrategyForInput(data);
      if (_shouldPredictMoshInput(start: start, strategy: strategy)) {
        _updateMoshPrediction(
          data,
          batchId: _nextMoshPredictionBatchId(),
          startColumn: start.column,
          startRow: start.row,
          strategy: strategy,
        );
      }
    }
    if (isSshSession) {
      if (effectiveSensitive || inputStatus != TerminalInputStatus.accepted) {
        _suspendSshPrediction();
      } else if (_shouldPredictSshInput()) {
        _updateSshPrediction(data);
      } else {
        _clearSshPrediction();
      }
    }
    if (inputStatus != TerminalInputStatus.accepted &&
        !effectiveSensitive &&
        !isMoshSession) {
      _localEcho(data);
    }
    if (data.contains('\r') || data.contains('\n')) {
      _sensitiveInputInProgress = false;
    } else if (effectiveSensitive) {
      _sensitiveInputInProgress = true;
    }
  }

  TerminalInputStatus _sendInputStatus(String data) {
    final driver = _driver;
    if (driver is NativeTerminalDriver) {
      return driver.sendInputStatus(data);
    }
    return driver.sendInput(data)
        ? TerminalInputStatus.accepted
        : TerminalInputStatus.closed;
  }

  bool _shouldPredictMoshInput({
    required _PredictionCursorPosition start,
    required MoshPredictionStrategy strategy,
  }) {
    if (!_moshPredictionEnabled ||
        !_snapshot.inputEchoEnabled ||
        _moshPredictionPausedUntilScreenUpdate) {
      return false;
    }
    return switch (config.moshPredictionMode) {
      TerminalMoshPredictionMode.never => false,
      TerminalMoshPredictionMode.adaptive => _shouldPredictAdaptiveMoshInput(
        start: start,
        strategy: strategy,
      ),
      TerminalMoshPredictionMode.always => true,
    };
  }

  bool _shouldPredictSshInput() {
    if (!isSshSession ||
        _connectionStatus.phase != TerminalConnectionPhase.connected) {
      return false;
    }
    return switch (config.sshPredictionMode) {
      TerminalSshPredictionMode.never => false,
      TerminalSshPredictionMode.adaptive => _sshAdaptivePredictionEnabled,
      TerminalSshPredictionMode.always => true,
    };
  }

  void _updateSshPrediction(String data) {
    final changed = _sshPrediction.addInput(data, _snapshot);
    _syncSshPredictionExpiryTimer();
    if (changed) {
      notifyListeners();
    }
  }

  void _reconcileSshPrediction(TerminalSnapshot snapshot) {
    if (!isSshSession) {
      return;
    }
    _sshPrediction.reconcile(snapshot);
    _syncSshPredictionExpiryTimer();
  }

  void _updateSshLatency(double? latencyMs) {
    if (latencyMs == null || !latencyMs.isFinite || latencyMs < 0) {
      return;
    }
    _sshLatencyMs = latencyMs;
    if (_sshAdaptivePredictionEnabled) {
      if (latencyMs <= _sshPredictionAdaptiveDisableLatencyMs) {
        _sshAdaptivePredictionEnabled = false;
        _clearSshPrediction();
      }
      return;
    }
    if (latencyMs >= _sshPredictionAdaptiveEnableLatencyMs) {
      _sshAdaptivePredictionEnabled = true;
    }
  }

  void _suspendSshPrediction() {
    final changed = _sshPrediction.suspendUntilScreenUpdate();
    _syncSshPredictionExpiryTimer();
    if (changed) {
      notifyListeners();
    }
  }

  void _clearSshPrediction() {
    _sshPrediction.clear();
    _sshPredictionExpiryTimer?.cancel();
    _sshPredictionExpiryTimer = null;
  }

  void _syncSshPredictionExpiryTimer() {
    _sshPredictionExpiryTimer?.cancel();
    _sshPredictionExpiryTimer = null;
    if (_sshPrediction.isEmpty) {
      return;
    }
    _sshPredictionExpiryTimer = Timer(_sshPredictionExpiry, () {
      _sshPredictionExpiryTimer = null;
      if (_disposed || !_sshPrediction.clear()) {
        return;
      }
      notifyListeners();
    });
  }

  bool _shouldPredictAdaptiveMoshInput({
    required _PredictionCursorPosition start,
    required MoshPredictionStrategy strategy,
  }) {
    if (_moshAdaptivePredictionEnabled) {
      return true;
    }
    final lineState = _moshConfirmedLineState;
    if (lineState == null || strategy != MoshPredictionStrategy.text) {
      return false;
    }
    return start.row == lineState.row &&
        start.column == lineState.nextColumn &&
        lineState.strategy == MoshPredictionStrategy.text;
  }

  void _updateMoshLatency(double? latencyMs) {
    if (latencyMs == null || !latencyMs.isFinite || latencyMs < 0) {
      return;
    }
    _moshLatencyMs = latencyMs;
    if (_moshAdaptivePredictionEnabled) {
      if (latencyMs <= _moshPredictionAdaptiveDisableLatencyMs) {
        _moshAdaptivePredictionEnabled = false;
      }
      return;
    }
    if (latencyMs >= _moshPredictionAdaptiveEnableLatencyMs) {
      _moshAdaptivePredictionEnabled = true;
    }
  }

  int _nextMoshPredictionBatchId() {
    _moshPredictionBatchId += 1;
    return _moshPredictionBatchId;
  }

  MoshPredictionStrategy _predictionStrategyForInput(String data) {
    final graphemes = data.characters;
    if (graphemes.isNotEmpty &&
        graphemes.every((grapheme) => grapheme == '\b' || grapheme == '\x7f')) {
      return MoshPredictionStrategy.backspace;
    }
    return MoshPredictionStrategy.text;
  }

  _PredictionCursorPosition _currentMoshPredictionCursor() {
    final liveSnapshot = _driver.snapshot;
    if (_moshPendingPrediction.isEmpty) {
      return _PredictionCursorPosition(
        column: liveSnapshot.cursor.column,
        row: liveSnapshot.cursor.row,
      );
    }
    return _predictionCursorAfterUnits(_moshPendingPrediction);
  }

  _PredictionCursorPosition _predictionCursorAfterUnits(
    List<_PendingPredictionUnit> units,
  ) {
    final firstBatchId = _moshPendingPrediction.first.batchId;
    final firstBatch = _moshPredictionBatches[firstBatchId];
    if (firstBatch == null) {
      final liveSnapshot = _driver.snapshot;
      return _PredictionCursorPosition(
        column: liveSnapshot.cursor.column,
        row: liveSnapshot.cursor.row,
      );
    }
    var column = firstBatch.startColumn;
    var row = firstBatch.startRow;
    for (final unit in units) {
      if (column + unit.cellWidth > _snapshot.columns) {
        column = 0;
        row += 1;
      }
      column += unit.cellWidth;
      if (column >= _snapshot.columns) {
        column = 0;
        row += 1;
      }
    }
    return _PredictionCursorPosition(column: column, row: row);
  }

  void _updateMoshPrediction(
    String data, {
    required int batchId,
    required int startColumn,
    required int startRow,
    required MoshPredictionStrategy strategy,
  }) {
    if (data.runes.any(
      (rune) =>
          rune == 0x1b ||
          (rune < 0x20 && rune != 0x08 && rune != 0x0d && rune != 0x0a),
    )) {
      _moshPredictionPausedUntilScreenUpdate = true;
      _moshConfirmedLineState = null;
      if (_moshPendingPrediction.isNotEmpty) {
        _moshPendingPrediction.clear();
        _moshPredictionBatches.clear();
        notifyListeners();
      }
      return;
    }
    final previousPrediction = List<_PendingPredictionUnit>.of(
      _moshPendingPrediction,
    );
    final expectedBackspaceCursor =
        strategy == MoshPredictionStrategy.backspace &&
            previousPrediction.isNotEmpty
        ? _predictionCursorAfterUnits(
            previousPrediction.sublist(0, previousPrediction.length - 1),
          )
        : null;
    _moshPredictionBatches[batchId] = _PendingPredictionBatch(
      batchId: batchId,
      startColumn: startColumn,
      startRow: startRow,
      strategy: strategy,
      latencyMs: _moshLatencyMs,
      isBackspace: strategy == MoshPredictionStrategy.backspace,
      expectedCursor: expectedBackspaceCursor,
    );
    final prediction = previousPrediction;
    for (final grapheme in data.characters) {
      switch (grapheme) {
        case '\b':
        case '\x7f':
          if (prediction.isNotEmpty) {
            prediction.removeLast();
          }
        case '\r':
        case '\n':
          prediction.clear();
        default:
          final cellWidth = terminalGraphemeCellWidth(grapheme);
          if (cellWidth > 0) {
            prediction.add(
              _PendingPredictionUnit(
                grapheme: grapheme,
                cellWidth: cellWidth,
                batchId: batchId,
              ),
            );
          }
      }
    }
    final width = prediction.fold<int>(
      0,
      (total, unit) => total + unit.cellWidth,
    );
    if (width > 256) {
      // Do not shift an overlay away from the real cursor when a long queued
      // command exceeds its conservative prediction budget.
      prediction.clear();
    }
    if (_samePredictionUnits(prediction, _moshPendingPrediction)) {
      _retainPredictionBatchesFor(prediction);
      return;
    }
    _moshPendingPrediction
      ..clear()
      ..addAll(prediction);
    _retainPredictionBatchesFor(prediction);
    notifyListeners();
  }

  void _clearMoshPrediction() {
    _moshPendingPrediction.clear();
    _moshPredictionBatches.clear();
    _moshScreenCommitPending = false;
  }

  void _bindMoshInputState(int stateNum) {
    for (final batch in _moshPredictionBatches.values) {
      if (batch.inputStateNum == null) {
        batch.inputStateNum = stateNum;
        return;
      }
    }
  }

  void _markMoshPredictionInputAcked({int? stateNum}) {
    for (final batch in _moshPredictionBatches.values) {
      final inputStateNum = batch.inputStateNum;
      if (stateNum == null ||
          (inputStateNum != null && inputStateNum <= stateNum)) {
        batch.inputAcked = true;
      }
    }
  }

  void _confirmMoshPredictionAfterScreenUpdate({int? stateNum}) {
    if (stateNum != null &&
        (_moshCommittedScreenStateNum == null ||
            stateNum > _moshCommittedScreenStateNum!)) {
      _moshCommittedScreenStateNum = stateNum;
    }
    _moshScreenCommitPending = true;
  }

  void _reconcileMoshPredictionBatches() {
    if (!_moshScreenCommitPending) {
      return;
    }
    _moshScreenCommitPending = false;
    int? lastConfirmedBatchId;
    int? mismatchedBatchId;
    final orderedBatchIds = _moshPredictionBatches.keys.toList()..sort();
    for (final batchId in orderedBatchIds) {
      final batch = _moshPredictionBatches[batchId];
      if (batch == null) {
        break;
      }
      final units = _predictionUnitsForBatch(batchId);
      final state = _snapshotStateForBatch(batch, units);
      if (state == _PredictionSnapshotState.confirmed) {
        batch.snapshotCommitted = true;
        if (batch.isBackspace) {
          _moshConfirmedLineState = null;
        } else {
          _recordConfirmedMoshPredictionLineState(batchId);
        }
        lastConfirmedBatchId = batchId;
        continue;
      }
      if (state == _PredictionSnapshotState.mismatch) {
        mismatchedBatchId = batchId;
      }
      break;
    }
    if (lastConfirmedBatchId != null) {
      _retainMoshPredictionAfterBatch(lastConfirmedBatchId);
    }
    if (mismatchedBatchId != null) {
      _retainMoshPredictionBeforeBatch(mismatchedBatchId);
    }
  }

  _PredictionSnapshotState _snapshotStateForBatch(
    _PendingPredictionBatch batch,
    List<_PendingPredictionUnit> units,
  ) {
    if (batch.isBackspace) {
      return _snapshotBackspaceMatches(batch)
          ? _PredictionSnapshotState.confirmed
          : _PredictionSnapshotState.pending;
    }
    if (units.isEmpty) {
      return _PredictionSnapshotState.pending;
    }
    if (_snapshotBatchMatches(batch, units)) {
      return _PredictionSnapshotState.confirmed;
    }
    if (_snapshotBatchMismatch(
      _snapshot,
      startColumn: batch.startColumn,
      startRow: batch.startRow,
      units: units,
    )) {
      return _PredictionSnapshotState.mismatch;
    }
    final expectedCursor = _predictionCursorAfterBatch(batch, units);
    return _compareCursorPositions(
              row: _snapshot.cursor.row,
              column: _snapshot.cursor.column,
              otherRow: expectedCursor.row,
              otherColumn: expectedCursor.column,
            ) >=
            0
        ? _PredictionSnapshotState.mismatch
        : _PredictionSnapshotState.pending;
  }

  _PredictionCursorPosition _predictionCursorAfterBatch(
    _PendingPredictionBatch batch,
    List<_PendingPredictionUnit> units,
  ) {
    var column = batch.startColumn;
    var row = batch.startRow;
    for (final unit in units) {
      if (column + unit.cellWidth > _snapshot.columns) {
        column = 0;
        row += 1;
      }
      column += unit.cellWidth;
      if (column >= _snapshot.columns) {
        column = 0;
        row += 1;
      }
    }
    return _PredictionCursorPosition(column: column, row: row);
  }

  void _recordConfirmedMoshPredictionLineState(int confirmedBatchId) {
    final batch = _moshPredictionBatches[confirmedBatchId];
    if (batch == null) {
      return;
    }
    final units = _predictionUnitsForBatch(confirmedBatchId);
    if (units.isEmpty) {
      return;
    }
    var column = batch.startColumn;
    var row = batch.startRow;
    for (final unit in units) {
      if (column + unit.cellWidth > _snapshot.columns) {
        column = 0;
        row += 1;
      }
      column += unit.cellWidth;
      if (column >= _snapshot.columns) {
        column = 0;
        row += 1;
      }
    }
    _moshConfirmedLineState = _ConfirmedPredictionLineState(
      row: row,
      nextColumn: column,
      strategy: batch.strategy,
    );
  }

  void _retainPredictionBatchesFor(List<_PendingPredictionUnit> units) {
    final activeBatchIds = units.map((unit) => unit.batchId).toSet();
    _moshPredictionBatches.removeWhere(
      (batchId, batch) =>
          !activeBatchIds.contains(batchId) && !batch.isBackspace,
    );
  }

  void _invalidateMoshPredictionAtOrAfterCursor(TerminalCursor cursor) {
    if (_moshPendingPrediction.isEmpty) {
      return;
    }
    int? invalidatedBatchId;
    for (final batch in debugMoshPredictionBatches) {
      if (_compareCursorPositions(
            row: cursor.row,
            column: cursor.column,
            otherRow: batch.startRow,
            otherColumn: batch.startColumn,
          ) <
          0) {
        invalidatedBatchId = batch.batchId;
        break;
      }
    }
    final batchId = invalidatedBatchId;
    if (batchId == null) {
      return;
    }
    _retainMoshPredictionBeforeBatch(batchId);
  }

  void _invalidateMoshPredictionAtOrAfterMismatch(TerminalSnapshot snapshot) {
    if (_moshPendingPrediction.isEmpty) {
      return;
    }
    for (final batch in debugMoshPredictionBatches) {
      final units = _predictionUnitsForBatch(batch.batchId);
      if (units.isEmpty) {
        continue;
      }
      if (_snapshotBatchMismatch(
        snapshot,
        startColumn: batch.startColumn,
        startRow: batch.startRow,
        units: units,
      )) {
        _retainMoshPredictionBeforeBatch(batch.batchId);
        return;
      }
    }
  }

  int _compareCursorPositions({
    required int row,
    required int column,
    required int otherRow,
    required int otherColumn,
  }) {
    if (row != otherRow) {
      return row.compareTo(otherRow);
    }
    return column.compareTo(otherColumn);
  }

  List<_PendingPredictionUnit> _predictionUnitsForBatch(int batchId) {
    return _moshPendingPrediction
        .where((unit) => unit.batchId == batchId)
        .toList(growable: false);
  }

  bool _snapshotBatchMismatch(
    TerminalSnapshot snapshot, {
    required int startColumn,
    required int startRow,
    required List<_PendingPredictionUnit> units,
  }) {
    var column = startColumn;
    var row = startRow;
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

  bool _snapshotBatchMatches(
    _PendingPredictionBatch batch,
    List<_PendingPredictionUnit> units,
  ) {
    var column = batch.startColumn;
    var row = batch.startRow;
    for (final unit in units) {
      if (column + unit.cellWidth > _snapshot.columns) {
        column = 0;
        row += 1;
      }
      if (_snapshot.cellAt(row, column).text != unit.grapheme) {
        return false;
      }
      column += unit.cellWidth;
      if (column >= _snapshot.columns) {
        column = 0;
        row += 1;
      }
    }
    return true;
  }

  bool _snapshotBackspaceMatches(_PendingPredictionBatch batch) {
    final expectedCursor = batch.expectedCursor;
    if (expectedCursor == null ||
        _snapshot.cursor.row != expectedCursor.row ||
        _snapshot.cursor.column != expectedCursor.column) {
      return false;
    }
    return _snapshot.cellAt(expectedCursor.row, expectedCursor.column).text ==
        ' ';
  }

  void _retainMoshPredictionAfterBatch(int batchId) {
    final retained = _moshPendingPrediction
        .where((unit) => unit.batchId > batchId)
        .toList(growable: false);
    _replaceMoshPredictionUnits(retained);
    _moshPredictionBatches.removeWhere((id, _) => id <= batchId);
  }

  void _retainMoshPredictionBeforeBatch(int batchId) {
    final retained = _moshPendingPrediction
        .where((unit) => unit.batchId < batchId)
        .toList(growable: false);
    _replaceMoshPredictionUnits(retained);
    _moshPredictionBatches.removeWhere((id, _) => id >= batchId);
  }

  void _replaceMoshPredictionUnits(List<_PendingPredictionUnit> next) {
    if (_samePredictionUnits(next, _moshPendingPrediction)) {
      return;
    }
    _moshPendingPrediction
      ..clear()
      ..addAll(next);
    _retainPredictionBatchesFor(next);
  }

  bool _samePredictionUnits(
    List<_PendingPredictionUnit> left,
    List<_PendingPredictionUnit> right,
  ) {
    if (left.length != right.length) {
      return false;
    }
    for (var index = 0; index < left.length; index += 1) {
      final leftUnit = left[index];
      final rightUnit = right[index];
      if (leftUnit.grapheme != rightUnit.grapheme ||
          leftUnit.cellWidth != rightUnit.cellWidth ||
          leftUnit.batchId != rightUnit.batchId) {
        return false;
      }
    }
    return true;
  }

  void _setMoshNetworkState(MoshNetworkState state) {
    _moshNetworkRestoredTimer?.cancel();
    _moshNetworkRestoredTimer = null;
    _moshNetworkState = state;
    if (state == MoshNetworkState.restored) {
      _moshNetworkRestoredTimer = Timer(const Duration(seconds: 2), () {
        if (_disposed || _moshNetworkState != MoshNetworkState.restored) {
          return;
        }
        _moshNetworkState = MoshNetworkState.stable;
        notifyListeners();
      });
    }
  }

  void _refresh() {
    if (_disposed) {
      return;
    }

    _drainConnectionEvents();
    _drainOutputCapture();
    _refreshSnapshot();
    notifyListeners();
  }

  void _refreshSnapshot() {
    final nextSnapshot = _driver.snapshot;
    _snapshot = nextSnapshot;
    _reconcileSshPrediction(nextSnapshot);
    _reconcileMoshPredictionBatches();
    _invalidateMoshPredictionAtOrAfterCursor(nextSnapshot.cursor);
    _invalidateMoshPredictionAtOrAfterMismatch(nextSnapshot);
    _pruneConfirmedMoshLineState(nextSnapshot.cursor);
    _recorder?.recordSnapshot(_snapshot);
    _updateFallbackExitStatus();
    _lastSnapshotRefreshAt = _snapshotRefreshClock.elapsed;
  }

  void _pruneConfirmedMoshLineState(TerminalCursor cursor) {
    final lineState = _moshConfirmedLineState;
    if (lineState == null) {
      return;
    }
    if (cursor.row != lineState.row || cursor.column != lineState.nextColumn) {
      _moshConfirmedLineState = null;
    }
  }

  void _localEcho(String data) {
    if (data.startsWith('\x1b')) {
      return;
    }

    final output = StringBuffer();
    for (final rune in data.runes) {
      switch (rune) {
        case 0x0d:
          output.write('\r\n\x1b[32mready\x1b[0m \$ ');
          break;
        case 0x08:
        case 0x7f:
          output.write('\x08 \x08');
          break;
        default:
          output.write(String.fromCharCodes([rune]));
      }
    }

    if (output.isNotEmpty) {
      write(output.toString());
    }
  }

  bool _pollDriver() {
    if (_disposed) {
      return false;
    }

    final changed = _driver.poll();
    final eventChanged = _drainConnectionEvents();
    if (changed) {
      _moshPredictionPausedUntilScreenUpdate = false;
      _drainOutputCapture();
    }
    if ((changed || eventChanged) && _moshScreenCommitPending) {
      // Matching a committed screen must use the snapshot produced by this
      // poll, not the stale snapshot held by the controller.
      _refreshSnapshot();
    }
    if (changed) {
      _scheduleSnapshotRefresh();
    } else if (eventChanged) {
      _scheduleSnapshotRefresh();
    }
    _notifyExitIfNeeded();

    return changed || eventChanged;
  }

  bool _drainConnectionEvents() {
    final events = _driver.drainConnectionEvents();
    if (events.isEmpty) {
      return false;
    }

    for (final event in events) {
      _addConnectionEvent(event);
    }
    return true;
  }

  void _drainOutputCapture() {
    final capture = _driver.drainOutputCapture();
    if (capture.isNotEmpty) {
      _emitOutput(capture);
      _telnetAutoLogin?.handleOutput(capture);
      // Shell integration emits OSC 4545 on the same output stream.  It is
      // deliberately recorded here rather than inferred from keystrokes: a
      // shell may edit, expand, or reject the line before it is executed.
      _recorder?.recordShellIntegrationOutput(capture);
      final recordable = _captureSanitizer.add(capture);
      _recorder?.recordCaptureBytes(recordable);
    }
  }

  void _flushCaptureSanitizer() {
    _recorder?.recordCaptureBytes(_captureSanitizer.close());
  }

  void _emitOutput(Uint8List bytes) {
    if (_disposed || bytes.isEmpty) {
      return;
    }
    _reportedShellPath =
        _shellAnnouncementParser.add(bytes) ?? _reportedShellPath;
    if (_outputListeners.isEmpty) {
      return;
    }
    for (final listener in List.of(_outputListeners)) {
      listener(bytes);
    }
  }

  void _addConnectionEvent(TerminalConnectionEvent event) {
    final stampedEvent = event.timestamp == null
        ? event.copyWith(timestamp: DateTime.now())
        : event;
    _connectionEvents.add(stampedEvent);
    if (_connectionEvents.length > 400) {
      _connectionEvents.removeRange(0, _connectionEvents.length - 400);
    }
    _recorder?.recordConnectionEvent(stampedEvent);
    _logConnectionEvent(stampedEvent);
    _applyConnectionEvent(stampedEvent);
  }

  void _logConnectionEvent(TerminalConnectionEvent event) {
    final protocol = _remoteTransport == _RemoteTerminalTransport.mosh
        ? 'mosh'
        : _remoteTransport == _RemoteTerminalTransport.ssh
        ? 'ssh'
        : _serialProfile != null
        ? 'serial'
        : _telnetProfile != null
        ? 'telnet'
        : 'local';
    final fields = <String, Object?>{
      'protocol': protocol,
      'event': event.kind.name,
      if (event.exitStatus != null) 'exit_status': event.exitStatus,
    };
    switch (event.kind) {
      case TerminalConnectionEventKind.connectStart:
      case TerminalConnectionEventKind.connected:
      case TerminalConnectionEventKind.retry:
      case TerminalConnectionEventKind.exitStatus:
      case TerminalConnectionEventKind.sessionClosed:
      case TerminalConnectionEventKind.moshNetworkSwitching:
      case TerminalConnectionEventKind.moshNetworkRestored:
        NautermLog.info(
          'connection',
          'Connection lifecycle changed.',
          fields: fields,
        );
      case TerminalConnectionEventKind.authFailed:
      case TerminalConnectionEventKind.hostKeyChanged:
      case TerminalConnectionEventKind.hostKeyRejected:
      case TerminalConnectionEventKind.hostKeySaveFailed:
      case TerminalConnectionEventKind.moshNetworkDegraded:
      case TerminalConnectionEventKind.connectionLost:
      case TerminalConnectionEventKind.error:
        NautermLog.warning(
          'connection',
          'Connection lifecycle reported a problem.',
          fields: fields,
        );
      default:
        break;
    }
  }

  void _applyConnectionEvent(TerminalConnectionEvent event) {
    final profile = _sshProfile;
    final serialProfile = _serialProfile;
    final telnetProfile = _telnetProfile;
    if (profile == null && serialProfile == null && telnetProfile == null) {
      return;
    }

    switch (event.kind) {
      case TerminalConnectionEventKind.connectStart:
      case TerminalConnectionEventKind.knownHostCheck:
      case TerminalConnectionEventKind.knownHostVerified:
      case TerminalConnectionEventKind.hostKeyAccepted:
      case TerminalConnectionEventKind.hostKeyAcceptedForSession:
      case TerminalConnectionEventKind.authSuccess:
      case TerminalConnectionEventKind.retry:
        _connectionStatus = TerminalConnectionStatus(
          phase: TerminalConnectionPhase.connecting,
          message: event.message,
        );
      case TerminalConnectionEventKind.knownHostStoreMissing:
      case TerminalConnectionEventKind.hostKeyUnknown:
        _connectionStatus = TerminalConnectionStatus(
          phase: TerminalConnectionPhase.hostKey,
          message: event.message,
        );
      case TerminalConnectionEventKind.authNoneStart:
      case TerminalConnectionEventKind.authNoneRejected:
      case TerminalConnectionEventKind.authNoneFailed:
      case TerminalConnectionEventKind.authPasswordStart:
      case TerminalConnectionEventKind.authPasswordRejected:
      case TerminalConnectionEventKind.authPasswordFailed:
      case TerminalConnectionEventKind.authKeyStart:
      case TerminalConnectionEventKind.authKeyRejected:
      case TerminalConnectionEventKind.authKeyFailed:
      case TerminalConnectionEventKind.authPassphraseRequired:
      case TerminalConnectionEventKind.authAgentStart:
      case TerminalConnectionEventKind.authAgentIdentities:
      case TerminalConnectionEventKind.authAgentIdentityStart:
      case TerminalConnectionEventKind.authAgentIdentityRejected:
      case TerminalConnectionEventKind.authAgentUnavailable:
      case TerminalConnectionEventKind.authAgentFailed:
      case TerminalConnectionEventKind.authFailed:
        _connectionStatus = TerminalConnectionStatus(
          phase: TerminalConnectionPhase.authentication,
          message: event.message,
        );
      case TerminalConnectionEventKind.connected:
        if (_remoteTransport == _RemoteTerminalTransport.mosh &&
            _connectionStatus.phase == TerminalConnectionPhase.connecting) {
          _scheduleMoshConnectedReveal(event.message);
          return;
        }
        _hasConnectedOnce = true;
        _reconnectBoundaryWritten = false;
        if (serialProfile != null) {
          _connectionStatus = TerminalConnectionStatus(
            phase: TerminalConnectionPhase.connected,
            message:
                'Connected to ${serialProfile.serialPort} at ${serialProfile.config.summary}.',
          );
          return;
        }
        if (telnetProfile != null) {
          _connectionStatus = TerminalConnectionStatus(
            phase: TerminalConnectionPhase.connected,
            message:
                'Connected to ${telnetProfile.host}:${telnetProfile.port}.',
          );
          if ((telnetProfile.username == null ||
                  telnetProfile.username!.trim().isEmpty) &&
              (telnetProfile.password == null ||
                  telnetProfile.password!.isEmpty)) {
            _scheduleStartupSnippetIfNeeded();
          }
          return;
        }
        _connectionStatus = TerminalConnectionStatus(
          phase: TerminalConnectionPhase.connected,
          message: 'Connected to ${profile!.username}@${profile.host}.',
        );
        if (!_connectedCallbackNotified) {
          _connectedCallbackNotified = true;
          _onConnected?.call();
        }
        _scheduleStartupSnippetIfNeeded();
      case TerminalConnectionEventKind.sshLatencyUpdated:
        _updateSshLatency(event.latencyMs);
      case TerminalConnectionEventKind.moshEchoEnabled:
        _moshPredictionEnabled = true;
      case TerminalConnectionEventKind.moshEchoDisabled:
        _moshPredictionEnabled = false;
        _moshPredictionPausedUntilScreenUpdate = false;
        _clearMoshPrediction();
      case TerminalConnectionEventKind.moshPredictionConfirmed:
        _markMoshPredictionInputAcked(stateNum: event.stateNum);
      case TerminalConnectionEventKind.moshInputStateQueued:
        if (event.stateNum != null) {
          _bindMoshInputState(event.stateNum!);
        }
      case TerminalConnectionEventKind.moshScreenCommitted:
        _confirmMoshPredictionAfterScreenUpdate(stateNum: event.stateNum);
      case TerminalConnectionEventKind.moshLatencyUpdated:
        _updateMoshLatency(event.latencyMs);
      case TerminalConnectionEventKind.moshUdpPeerConfirmed:
        break;
      case TerminalConnectionEventKind.moshNetworkSwitching:
        _setMoshNetworkState(MoshNetworkState.switching);
      case TerminalConnectionEventKind.moshNetworkDegraded:
        _setMoshNetworkState(MoshNetworkState.degraded);
      case TerminalConnectionEventKind.moshNetworkRestored:
        _setMoshNetworkState(MoshNetworkState.restored);
      case TerminalConnectionEventKind.exitStatus:
        if (_activeExitRequested) {
          return;
        }
        _connectionStatus = TerminalConnectionStatus(
          phase: TerminalConnectionPhase.exited,
          message: serialProfile != null
              ? 'Serial session closed.'
              : telnetProfile != null
              ? 'Telnet session closed.'
              : event.exitStatus == null
              ? 'SSH session exited.'
              : 'SSH session exited with status ${event.exitStatus}.',
          exitCode: event.exitStatus,
        );
      case TerminalConnectionEventKind.sessionClosed:
        if (_activeExitRequested) {
          return;
        }
        if (_connectionStatus.phase == TerminalConnectionPhase.connected ||
            _connectionStatus.phase == TerminalConnectionPhase.connecting) {
          _connectionStatus = TerminalConnectionStatus(
            phase: TerminalConnectionPhase.exited,
            message: event.message.isEmpty
                ? serialProfile != null
                      ? 'Serial session closed.'
                      : telnetProfile != null
                      ? 'Telnet session closed.'
                      : 'SSH session closed.'
                : event.message,
          );
        }
      case TerminalConnectionEventKind.connectionLost:
        if (_activeExitRequested) {
          return;
        }
        _connectionStatus = TerminalConnectionStatus(
          phase: TerminalConnectionPhase.failed,
          message: event.message.isEmpty ? 'Connection lost.' : event.message,
        );
      case TerminalConnectionEventKind.hostKeyChanged:
      case TerminalConnectionEventKind.hostKeyRejected:
      case TerminalConnectionEventKind.hostKeySaveFailed:
      case TerminalConnectionEventKind.error:
        if (_hasActionableHostKeyEvent &&
            event.kind == TerminalConnectionEventKind.error) {
          return;
        }
        if (_hasActionableAuthEvent &&
            event.kind == TerminalConnectionEventKind.error) {
          return;
        }
        _connectionStatus = TerminalConnectionStatus(
          phase: TerminalConnectionPhase.failed,
          message: event.message,
        );
      case TerminalConnectionEventKind.unknown:
        if (event.message.isNotEmpty) {
          _connectionStatus = TerminalConnectionStatus(
            phase: TerminalConnectionPhase.connecting,
            message: event.message,
          );
        }
    }
  }

  bool get _hasActionableHostKeyEvent {
    for (final event in _connectionEvents.reversed) {
      switch (event.kind) {
        case TerminalConnectionEventKind.hostKeyAccepted:
        case TerminalConnectionEventKind.hostKeyAcceptedForSession:
        case TerminalConnectionEventKind.connected:
          return false;
        case TerminalConnectionEventKind.hostKeyUnknown:
        case TerminalConnectionEventKind.knownHostStoreMissing:
          return true;
        default:
          break;
      }
    }
    return false;
  }

  bool get _hasActionableAuthEvent {
    for (final event in _connectionEvents.reversed) {
      switch (event.kind) {
        case TerminalConnectionEventKind.authSuccess:
        case TerminalConnectionEventKind.connected:
          return false;
        case TerminalConnectionEventKind.authFailed:
        case TerminalConnectionEventKind.authPassphraseRequired:
        case TerminalConnectionEventKind.authPasswordRejected:
        case TerminalConnectionEventKind.authKeyRejected:
        case TerminalConnectionEventKind.authAgentUnavailable:
          return true;
        default:
          break;
      }
    }
    return false;
  }

  void _updateFallbackExitStatus() {
    final profile = _sshProfile;
    final serialProfile = _serialProfile;
    final telnetProfile = _telnetProfile;
    if ((profile == null && serialProfile == null && telnetProfile == null) ||
        !_driver.isExited) {
      return;
    }
    if (_activeExitRequested) {
      return;
    }
    if (_connectionStatus.phase == TerminalConnectionPhase.failed ||
        _connectionStatus.phase == TerminalConnectionPhase.hostKey ||
        _connectionStatus.phase == TerminalConnectionPhase.authentication ||
        _connectionStatus.phase == TerminalConnectionPhase.exited) {
      return;
    }

    _connectionStatus = const TerminalConnectionStatus(
      phase: TerminalConnectionPhase.exited,
      message: 'SSH session exited.',
    );
    if (_remoteTransport == _RemoteTerminalTransport.mosh) {
      _connectionStatus = const TerminalConnectionStatus(
        phase: TerminalConnectionPhase.exited,
        message: 'Mosh session exited.',
      );
    } else if (serialProfile != null) {
      _connectionStatus = const TerminalConnectionStatus(
        phase: TerminalConnectionPhase.exited,
        message: 'Serial session closed.',
      );
    } else if (telnetProfile != null) {
      _connectionStatus = const TerminalConnectionStatus(
        phase: TerminalConnectionPhase.exited,
        message: 'Telnet session closed.',
      );
    }
  }

  bool poll() => _pollDriver();

  void _startDriverWatchdog() {
    if (_driver is! NativeTerminalDriver || _driverWatchdogTimer != null) {
      return;
    }
    _driverWatchdogTimer = Timer.periodic(_driverWatchdogInterval, (timer) {
      if (_disposed) {
        timer.cancel();
        return;
      }
      _pollDriver();
    });
  }

  void _handleDriverWakeup() {
    if (_disposed || _driverWakeupPending) {
      return;
    }
    _driverWakeupPending = true;
    scheduleMicrotask(() {
      _driverWakeupPending = false;
      if (_disposed) {
        return;
      }
      _pollDriver();
    });
  }

  void _scheduleSnapshotRefresh() {
    if (_disposed || _snapshotRefreshQueued) {
      return;
    }

    final sinceLastRefresh =
        _snapshotRefreshClock.elapsed - _lastSnapshotRefreshAt;
    if (sinceLastRefresh >= _snapshotRefreshInterval) {
      _refreshSnapshot();
      notifyListeners();
      return;
    }

    _snapshotRefreshQueued = true;
    _snapshotRefreshTimer = Timer(
      _snapshotRefreshInterval - sinceLastRefresh,
      () {
        _snapshotRefreshQueued = false;
        _snapshotRefreshTimer = null;
        if (_disposed) {
          return;
        }
        _refreshSnapshot();
        notifyListeners();
      },
    );
  }

  void _notifyExitIfNeeded() {
    if (_disposed || _exitNotified || !_driver.isExited) {
      return;
    }

    _exitNotified = true;
    if ((_sshProfile != null ||
            _serialProfile != null ||
            _telnetProfile != null) &&
        !_activeExitRequested &&
        _connectionStatus.phase != TerminalConnectionPhase.failed &&
        _connectionStatus.phase != TerminalConnectionPhase.hostKey &&
        _connectionStatus.phase != TerminalConnectionPhase.authentication) {
      final exitCode = _connectionStatus.exitCode;
      _connectionStatus = TerminalConnectionStatus(
        phase: TerminalConnectionPhase.exited,
        message: _serialProfile != null
            ? 'Serial session closed.'
            : _telnetProfile != null
            ? 'Telnet session closed.'
            : _remoteTransport == _RemoteTerminalTransport.mosh
            ? 'Mosh session exited.'
            : exitCode == null
            ? 'SSH session exited.'
            : 'SSH session exited with status $exitCode.',
        exitCode: exitCode,
      );
    }
    _flushCaptureSanitizer();
    _recorder?.finish(message: _connectionStatus.message);
    _onExit?.call();
  }

  void _recordInputForExitIntent(String data) {
    if (data.isEmpty) {
      return;
    }

    if (_activeExitRequested) {
      _activeExitRequested = false;
    }

    for (final codeUnit in data.codeUnits) {
      switch (codeUnit) {
        case 0x04:
          _activeExitRequested = true;
          _pendingInputLine = '';
        case 0x08:
        case 0x7f:
          if (_pendingInputLine.isNotEmpty) {
            _pendingInputLine = _pendingInputLine.substring(
              0,
              _pendingInputLine.length - 1,
            );
          }
        case 0x0d:
        case 0x0a:
          if (_isActiveExitCommand(_pendingInputLine)) {
            _activeExitRequested = true;
          }
          _pendingInputLine = '';
        default:
          if (codeUnit >= 0x20 && codeUnit != 0x7f) {
            _pendingInputLine += String.fromCharCode(codeUnit);
          }
      }
    }
  }

  bool _isActiveExitCommand(String line) {
    final command = line.trim();
    if (command == 'logout') {
      return true;
    }
    if (command == 'exit') {
      return true;
    }

    final parts = command.split(RegExp(r'\s+'));
    if (parts.length != 2 || parts.first != 'exit') {
      return false;
    }

    return int.tryParse(parts.last) != null;
  }

  bool _startupSnippetSent = false;

  void _scheduleMoshConnectedReveal(String message) {
    if (_moshConnectedRevealTimer != null) return;
    _moshConnectedRevealTimer = Timer(_moshConnectedRevealDelay, () {
      _moshConnectedRevealTimer = null;
      if (_disposed) return;
      _hasConnectedOnce = true;
      _reconnectBoundaryWritten = false;
      _connectionStatus = TerminalConnectionStatus(
        phase: TerminalConnectionPhase.connected,
        message: message.isNotEmpty
            ? message
            : 'Connected to ${_sshProfile?.username}@${_sshProfile?.host}.',
      );
      if (!_connectedCallbackNotified) {
        _connectedCallbackNotified = true;
        _onConnected?.call();
      }
      _scheduleStartupSnippetIfNeeded();
      notifyListeners();
    });
  }

  void _scheduleStartupSnippetIfNeeded() {
    if (_startupSnippetSent || _disposed) {
      return;
    }
    scheduleMicrotask(() {
      if (!_disposed) {
        _sendStartupSnippetIfNeeded();
      }
    });
  }

  void _sendStartupSnippetIfNeeded() {
    if (_startupSnippetSent) return;
    final snippet = _startupSnippet
        ?.replaceAll('\r\n', '\n')
        .replaceAll('\n', '\r')
        .trimRight();
    if (snippet == null || snippet.isEmpty) {
      return;
    }
    _startupSnippetSent = true;
    sendInput(snippet.endsWith('\r') ? snippet : '$snippet\r');
  }

  @override
  void dispose() {
    _disposed = true;
    for (final listener in List.of(_disposeListeners)) {
      listener();
    }
    _disposeListeners.clear();
    _outputListeners.clear();
    _inputSentListeners.clear();
    _driverWatchdogTimer?.cancel();
    _driverWatchdogTimer = null;
    _driverWakeupPending = false;
    _snapshotRefreshTimer?.cancel();
    _snapshotRefreshTimer = null;
    _sshPredictionExpiryTimer?.cancel();
    _sshPredictionExpiryTimer = null;
    _moshNetworkRestoredTimer?.cancel();
    _moshNetworkRestoredTimer = null;
    _moshConnectedRevealTimer?.cancel();
    _moshConnectedRevealTimer = null;
    _flushCaptureSanitizer();
    _recorder?.finish(message: _connectionStatus.message);
    _driver.dispose();
    super.dispose();
  }
}

class _TelnetAutoLogin {
  _TelnetAutoLogin({
    required this.username,
    required this.password,
    required this.sendSensitiveInput,
    required this.onAuthenticated,
  });

  final String? username;
  final String? password;
  final TerminalInputSink sendSensitiveInput;
  final VoidCallback onAuthenticated;
  bool _sentUsername = false;
  bool _sentPassword = false;
  bool _authenticated = false;

  void handleOutput(Uint8List bytes) {
    if (bytes.isEmpty || _authenticated) {
      return;
    }
    final text = latin1.decode(bytes, allowInvalid: true).toLowerCase();
    if (!_sentUsername &&
        username != null &&
        username!.trim().isNotEmpty &&
        (text.contains('login:') || text.contains('username:'))) {
      _sentUsername = true;
      sendSensitiveInput('${username!.trim()}\r');
      if (password == null || password!.isEmpty) {
        _markAuthenticated();
      }
      return;
    }
    if (!_sentPassword &&
        password != null &&
        password!.isNotEmpty &&
        text.contains('password:')) {
      _sentPassword = true;
      sendSensitiveInput('$password\r');
      _markAuthenticated();
    }
  }

  void _markAuthenticated() {
    if (_authenticated) {
      return;
    }
    _authenticated = true;
    Timer(const Duration(milliseconds: 450), onAuthenticated);
  }
}

TerminalDriver _createDefaultDriver({
  required int columns,
  required int rows,
  required TerminalConfig config,
  required VoidCallback onWakeup,
  String? shellPath,
  String? workingDirectory,
  Map<String, String> environment = const {},
  TerminalTheme theme = defaultTerminalTheme,
}) {
  try {
    return NativeTerminalDriver.create(
      columns: columns,
      rows: rows,
      config: config,
      onWakeup: onWakeup,
      shellPath: shellPath,
      workingDirectory: workingDirectory,
      environment: environment,
      theme: theme,
    );
  } on Object {
    return MemoryTerminalDriver(columns: columns, rows: rows, config: config);
  }
}

TerminalDriver _createSshDriver({
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
  try {
    return NativeTerminalDriver.createSsh(
      columns: columns,
      rows: rows,
      config: config,
      onWakeup: onWakeup,
      host: host,
      port: port,
      username: username,
      knownHostsPath: knownHostsPath,
      password: password,
      privateKey: privateKey,
      passphrase: passphrase,
      proxy: proxy,
      hostKeyTrustMode: hostKeyTrustMode,
      environment: environment,
      encoding: encoding,
      theme: theme,
    );
  } on Object catch (error) {
    final driver = MemoryTerminalDriver(
      columns: columns,
      rows: rows,
      config: config,
    );
    driver.write('\x1b[31mUnable to create SSH session: $error\x1b[0m\r\n');
    return driver;
  }
}

TerminalDriver _createMoshDriver({
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
  // Mosh synchronizes the visible terminal state instead of preserving local
  // scrollback semantics. Keeping native scrollback enabled causes full-screen
  // apps to leak transient frames into shell history.
  final moshConfig = config.copyWith(scrollbackLines: 0);
  try {
    return NativeTerminalDriver.createMosh(
      columns: columns,
      rows: rows,
      config: moshConfig,
      onWakeup: onWakeup,
      host: host,
      port: port,
      username: username,
      knownHostsPath: knownHostsPath,
      password: password,
      privateKey: privateKey,
      passphrase: passphrase,
      proxy: proxy,
      hostKeyTrustMode: hostKeyTrustMode,
      environment: environment,
      serverCommand: serverCommand,
      theme: theme,
    );
  } on Object catch (error) {
    final driver = MemoryTerminalDriver(
      columns: columns,
      rows: rows,
      config: moshConfig,
    );
    driver.write('\x1b[31mUnable to create Mosh session: $error\x1b[0m\r\n');
    return driver;
  }
}

TerminalDriver _createTelnetDriver({
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
  try {
    return NativeTerminalDriver.createTelnet(
      columns: columns,
      rows: rows,
      config: config,
      onWakeup: onWakeup,
      host: host,
      port: port,
      encoding: encoding,
      environment: environment,
      theme: theme,
    );
  } on Object catch (error) {
    final driver = MemoryTerminalDriver(
      columns: columns,
      rows: rows,
      config: config,
    );
    driver.write('\x1b[31mUnable to create telnet session: $error\x1b[0m\r\n');
    return driver;
  }
}

TerminalDriver _createSerialDriver({
  required int columns,
  required int rows,
  required TerminalConfig config,
  required VoidCallback onWakeup,
  required SerialConnectionConfig serialConfig,
  TerminalTheme theme = defaultTerminalTheme,
}) {
  try {
    return NativeTerminalDriver.createSerial(
      columns: columns,
      rows: rows,
      config: config,
      onWakeup: onWakeup,
      serialConfig: serialConfig,
      theme: theme,
    );
  } on Object catch (error) {
    final driver = MemoryTerminalDriver(
      columns: columns,
      rows: rows,
      config: config,
    );
    driver.write('\x1b[31mUnable to create serial session: $error\x1b[0m\r\n');
    return driver;
  }
}
