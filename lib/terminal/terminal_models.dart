import 'dart:typed_data';

import 'package:flutter/material.dart';

import 'terminal_theme.dart';

const int terminalFlagInverse = 0x0001;
const int terminalFlagBold = 0x0002;
const int terminalFlagItalic = 0x0004;
const int terminalFlagUnderline = 0x0008;
const int terminalFlagWideChar = 0x0020;
const int terminalFlagWideCharSpacer = 0x0040;
const int terminalFlagDim = 0x0080;
const int terminalFlagHidden = 0x0100;
const int terminalFlagStrikeout = 0x0200;
const int terminalFlagLeadingWideCharSpacer = 0x0400;
const int terminalFlagDoubleUnderline = 0x0800;
const int terminalFlagUndercurl = 0x1000;
const int terminalFlagDottedUnderline = 0x2000;
const int terminalFlagDashedUnderline = 0x4000;

enum TerminalCursorShape { block, underline, beam, hollowBlock }

enum TerminalEmulatorBackend {
  alacritty,
  ghostty;

  static TerminalEmulatorBackend fromString(String? value) => switch (value) {
    'ghostty' => TerminalEmulatorBackend.ghostty,
    _ => TerminalEmulatorBackend.alacritty,
  };

  int get nativeValue => index;
}

enum SerialParity { none, even, odd }

enum SerialFlowControl { none, software, hardware }

@immutable
class TerminalProxyConfig {
  const TerminalProxyConfig({
    required this.type,
    required this.host,
    required this.port,
    this.username,
    this.password,
  });

  final String type;
  final String host;
  final int port;
  final String? username;
  final String? password;

  Map<String, Object?> toJson() => {
    'type': type,
    'host': host,
    'port': port,
    'username': username,
    'password': password,
  };
}

@immutable
class SerialConnectionConfig {
  const SerialConnectionConfig({
    required this.serialPort,
    required this.baudRate,
    this.dataBits = 8,
    this.parity = SerialParity.none,
    this.stopBits = 1,
    this.flowControl = SerialFlowControl.none,
  });

  final String serialPort;
  final int baudRate;
  final int dataBits;
  final SerialParity parity;
  final int stopBits;
  final SerialFlowControl flowControl;

  String get framingLabel {
    final parityLabel = switch (parity) {
      SerialParity.none => 'N',
      SerialParity.even => 'E',
      SerialParity.odd => 'O',
    };
    return '$dataBits$parityLabel$stopBits';
  }

  String get flowControlLabel => switch (flowControl) {
    SerialFlowControl.none => 'none',
    SerialFlowControl.software => 'software',
    SerialFlowControl.hardware => 'hardware',
  };

  String get summary => '$baudRate baud, $framingLabel, flow $flowControlLabel';
}

@immutable
class SerialPortInfo {
  const SerialPortInfo({
    required this.path,
    this.displayName,
    this.description,
  });

  factory SerialPortInfo.fromJson(Map<String, Object?> json) {
    return SerialPortInfo(
      path: json['path'] as String? ?? '',
      displayName: json['display_name'] as String?,
      description: json['description'] as String?,
    );
  }

  final String path;
  final String? displayName;
  final String? description;

  String get label {
    final name = displayName?.trim();
    if (name != null && name.isNotEmpty && name != path) {
      return '$name  $path';
    }
    return path;
  }
}

enum TerminalConnectionEventKind {
  connectStart,
  knownHostCheck,
  knownHostVerified,
  knownHostStoreMissing,
  hostKeyUnknown,
  hostKeyAccepted,
  hostKeyAcceptedForSession,
  hostKeyChanged,
  hostKeyRejected,
  hostKeySaveFailed,
  authNoneStart,
  authNoneRejected,
  authNoneFailed,
  authPasswordStart,
  authPasswordRejected,
  authPasswordFailed,
  authKeyStart,
  authKeyRejected,
  authKeyFailed,
  authPassphraseRequired,
  authAgentStart,
  authAgentIdentities,
  authAgentIdentityStart,
  authAgentIdentityRejected,
  authAgentUnavailable,
  authAgentFailed,
  authSuccess,
  authFailed,
  connected,
  sshLatencyUpdated,
  moshEchoEnabled,
  moshEchoDisabled,
  moshPredictionConfirmed,
  moshInputStateQueued,
  moshScreenCommitted,
  moshLatencyUpdated,
  moshUdpPeerConfirmed,
  moshNetworkSwitching,
  moshNetworkDegraded,
  moshNetworkRestored,
  retry,
  exitStatus,
  sessionClosed,
  connectionLost,
  error,
  unknown,
}

enum SshHostKeyTrustMode {
  strict(0),
  acceptAndSave(1),
  acceptOnce(2);

  const SshHostKeyTrustMode(this.wireValue);

  final int wireValue;

  static SshHostKeyTrustMode fromWireValue(Object? value) {
    return switch (value) {
      1 => SshHostKeyTrustMode.acceptAndSave,
      2 => SshHostKeyTrustMode.acceptOnce,
      _ => SshHostKeyTrustMode.strict,
    };
  }
}

@immutable
class TerminalConnectionEvent {
  const TerminalConnectionEvent({
    required this.kind,
    required this.message,
    this.timestamp,
    this.host,
    this.port,
    this.serialPort,
    this.baudRate,
    this.username,
    this.fingerprint,
    this.method,
    this.exitStatus,
    this.latencyMs,
    this.stateNum,
  });

  factory TerminalConnectionEvent.fromJson(Map<String, Object?> json) {
    return TerminalConnectionEvent(
      kind: _connectionEventKindFromWire(json['kind'] as String?),
      message: json['message'] as String? ?? '',
      timestamp: _dateTimeFromJson(json['timestamp']),
      host: json['host'] as String?,
      port: json['port'] as int?,
      serialPort: json['serial_port'] as String?,
      baudRate: json['baud_rate'] as int?,
      username: json['username'] as String?,
      fingerprint: json['fingerprint'] as String?,
      method: json['method'] as String?,
      exitStatus: json['exit_status'] as int?,
      latencyMs: (json['latency_ms'] as num?)?.toDouble(),
      stateNum: json['state_num'] as int?,
    );
  }

  final TerminalConnectionEventKind kind;
  final String message;
  final DateTime? timestamp;
  final String? host;
  final int? port;
  final String? serialPort;
  final int? baudRate;
  final String? username;
  final String? fingerprint;
  final String? method;
  final int? exitStatus;
  final double? latencyMs;
  final int? stateNum;

  String get logLine {
    final methodSuffix = method == null ? '' : ' [$method]';
    return '$message$methodSuffix';
  }

  TerminalConnectionEvent copyWith({
    TerminalConnectionEventKind? kind,
    String? message,
    Object? timestamp = _preserveConnectionEventTimestamp,
    Object? host = _preserveConnectionEventValue,
    Object? port = _preserveConnectionEventValue,
    Object? serialPort = _preserveConnectionEventValue,
    Object? baudRate = _preserveConnectionEventValue,
    Object? username = _preserveConnectionEventValue,
    Object? fingerprint = _preserveConnectionEventValue,
    Object? method = _preserveConnectionEventValue,
    Object? exitStatus = _preserveConnectionEventValue,
    Object? latencyMs = _preserveConnectionEventValue,
    Object? stateNum = _preserveConnectionEventValue,
  }) {
    return TerminalConnectionEvent(
      kind: kind ?? this.kind,
      message: message ?? this.message,
      timestamp: identical(timestamp, _preserveConnectionEventTimestamp)
          ? this.timestamp
          : timestamp as DateTime?,
      host: identical(host, _preserveConnectionEventValue)
          ? this.host
          : host as String?,
      port: identical(port, _preserveConnectionEventValue)
          ? this.port
          : port as int?,
      serialPort: identical(serialPort, _preserveConnectionEventValue)
          ? this.serialPort
          : serialPort as String?,
      baudRate: identical(baudRate, _preserveConnectionEventValue)
          ? this.baudRate
          : baudRate as int?,
      username: identical(username, _preserveConnectionEventValue)
          ? this.username
          : username as String?,
      fingerprint: identical(fingerprint, _preserveConnectionEventValue)
          ? this.fingerprint
          : fingerprint as String?,
      method: identical(method, _preserveConnectionEventValue)
          ? this.method
          : method as String?,
      exitStatus: identical(exitStatus, _preserveConnectionEventValue)
          ? this.exitStatus
          : exitStatus as int?,
      latencyMs: identical(latencyMs, _preserveConnectionEventValue)
          ? this.latencyMs
          : latencyMs as double?,
      stateNum: identical(stateNum, _preserveConnectionEventValue)
          ? this.stateNum
          : stateNum as int?,
    );
  }
}

const Object _preserveConnectionEventValue = Object();
const Object _preserveConnectionEventTimestamp = Object();

@immutable
class TerminalCell {
  const TerminalCell({
    required this.text,
    required this.foreground,
    required this.background,
    required this.flags,
    this.hyperlink = '',
  });

  const TerminalCell.empty()
    : text = ' ',
      foreground = terminalDefaultForeground,
      background = terminalDefaultBackground,
      flags = 0,
      hyperlink = '';

  final String text;
  final Color foreground;
  final Color background;
  final int flags;
  final String hyperlink;

  bool get inverse => flags & terminalFlagInverse != 0;
  bool get bold => flags & terminalFlagBold != 0;
  bool get italic => flags & terminalFlagItalic != 0;
  bool get dim => flags & terminalFlagDim != 0;
  bool get hidden => flags & terminalFlagHidden != 0;
  bool get wideChar => flags & terminalFlagWideChar != 0;
  bool get wideCharSpacer => flags & terminalFlagWideCharSpacer != 0;
  bool get leadingWideCharSpacer =>
      flags & terminalFlagLeadingWideCharSpacer != 0;
  bool get underlined => flags & terminalFlagUnderline != 0;
  bool get doubleUnderlined => flags & terminalFlagDoubleUnderline != 0;
  bool get undercurl => flags & terminalFlagUndercurl != 0;
  bool get dottedUnderline => flags & terminalFlagDottedUnderline != 0;
  bool get dashedUnderline => flags & terminalFlagDashedUnderline != 0;
  bool get strikeout => flags & terminalFlagStrikeout != 0;

  bool get hasUnderline =>
      underlined ||
      doubleUnderlined ||
      undercurl ||
      dottedUnderline ||
      dashedUnderline;

  Color get effectiveForeground {
    final adjustedForeground = dim
        ? Color.lerp(background, foreground, 0.62) ?? foreground
        : foreground;
    return inverse ? background : adjustedForeground;
  }

  Color get effectiveBackground => inverse ? foreground : background;
}

@immutable
class TerminalCursor {
  const TerminalCursor({
    required this.column,
    required this.row,
    required this.visible,
    required this.shape,
    required this.color,
    required this.blinking,
  });

  final int column;
  final int row;
  final bool visible;
  final TerminalCursorShape shape;
  final Color color;
  final bool blinking;
}

@immutable
class TerminalKeyboardMode {
  const TerminalKeyboardMode({
    this.applicationCursor = false,
    this.applicationKeypad = false,
    this.bracketedPaste = false,
    this.focusEvents = false,
    this.mouseReportClick = false,
    this.mouseDrag = false,
    this.mouseMotion = false,
    this.sgrMouse = false,
  });

  final bool applicationCursor;
  final bool applicationKeypad;
  final bool bracketedPaste;
  final bool focusEvents;
  final bool mouseReportClick;
  final bool mouseDrag;
  final bool mouseMotion;
  final bool sgrMouse;

  bool get mouseReporting => mouseReportClick || mouseDrag || mouseMotion;
}

@immutable
class TerminalSnapshot {
  const TerminalSnapshot({
    required this.columns,
    required this.rows,
    required this.cells,
    required this.cursor,
    required this.keyboardMode,
    this.emulatorBackend = TerminalEmulatorBackend.alacritty,
    this.graphicImages = const [],
    this.graphicPlacements = const [],
    this.historyLines = 0,
    this.displayOffset = 0,
    this.title = '',
    this.inputEchoEnabled = true,
    this.clipboardText = '',
    this.bellCount = 0,
  });

  factory TerminalSnapshot.blank({
    int columns = 80,
    int rows = 24,
    int historyLines = 0,
    int displayOffset = 0,
    TerminalKeyboardMode keyboardMode = const TerminalKeyboardMode(),
    bool inputEchoEnabled = true,
  }) {
    return TerminalSnapshot(
      columns: columns,
      rows: rows,
      historyLines: historyLines,
      displayOffset: displayOffset,
      cells: List<TerminalCell>.filled(
        columns * rows,
        const TerminalCell.empty(),
        growable: false,
      ),
      cursor: const TerminalCursor(
        column: 0,
        row: 0,
        visible: true,
        shape: TerminalCursorShape.block,
        color: terminalDefaultCursor,
        blinking: false,
      ),
      keyboardMode: keyboardMode,
      title: '',
      inputEchoEnabled: inputEchoEnabled,
    );
  }

  final int columns;
  final int rows;
  final int historyLines;
  final int displayOffset;
  final String title;
  final List<TerminalCell> cells;
  final TerminalCursor cursor;
  final TerminalKeyboardMode keyboardMode;
  final TerminalEmulatorBackend emulatorBackend;
  final List<TerminalGraphicImage> graphicImages;
  final List<TerminalGraphicPlacement> graphicPlacements;
  final bool inputEchoEnabled;
  final String clipboardText;
  final int bellCount;

  TerminalSnapshot copyWith({String? clipboardText, int? bellCount}) {
    return TerminalSnapshot(
      columns: columns,
      rows: rows,
      cells: cells,
      cursor: cursor,
      keyboardMode: keyboardMode,
      emulatorBackend: emulatorBackend,
      graphicImages: graphicImages,
      graphicPlacements: graphicPlacements,
      historyLines: historyLines,
      displayOffset: displayOffset,
      title: title,
      inputEchoEnabled: inputEchoEnabled,
      clipboardText: clipboardText ?? this.clipboardText,
      bellCount: bellCount ?? this.bellCount,
    );
  }

  TerminalCell cellAt(int row, int column) {
    if (row < 0 || row >= rows || column < 0 || column >= columns) {
      return const TerminalCell.empty();
    }

    return cells[row * columns + column];
  }
}

@immutable
class TerminalGraphicImage {
  const TerminalGraphicImage({
    required this.id,
    required this.generation,
    required this.width,
    required this.height,
    required this.rgba,
  });

  final int id;
  final int generation;
  final int width;
  final int height;
  final Uint8List rgba;
}

@immutable
class TerminalGraphicPlacement {
  const TerminalGraphicPlacement({
    required this.imageId,
    required this.placementId,
    required this.zIndex,
    required this.viewportColumn,
    required this.viewportRow,
    required this.columns,
    required this.rows,
    required this.sourceX,
    required this.sourceY,
    required this.sourceWidth,
    required this.sourceHeight,
  });

  final int imageId;
  final int placementId;
  final int zIndex;
  final int viewportColumn;
  final int viewportRow;
  final int columns;
  final int rows;
  final int sourceX;
  final int sourceY;
  final int sourceWidth;
  final int sourceHeight;
}

TerminalConnectionEventKind _connectionEventKindFromWire(String? value) {
  return switch (value) {
    'connect_start' => TerminalConnectionEventKind.connectStart,
    'known_host_check' => TerminalConnectionEventKind.knownHostCheck,
    'known_host_verified' => TerminalConnectionEventKind.knownHostVerified,
    'known_host_store_missing' =>
      TerminalConnectionEventKind.knownHostStoreMissing,
    'host_key_unknown' => TerminalConnectionEventKind.hostKeyUnknown,
    'host_key_accepted' => TerminalConnectionEventKind.hostKeyAccepted,
    'host_key_accepted_for_session' =>
      TerminalConnectionEventKind.hostKeyAcceptedForSession,
    'host_key_changed' => TerminalConnectionEventKind.hostKeyChanged,
    'host_key_rejected' => TerminalConnectionEventKind.hostKeyRejected,
    'host_key_save_failed' => TerminalConnectionEventKind.hostKeySaveFailed,
    'auth_none_start' => TerminalConnectionEventKind.authNoneStart,
    'auth_none_rejected' => TerminalConnectionEventKind.authNoneRejected,
    'auth_none_failed' => TerminalConnectionEventKind.authNoneFailed,
    'auth_password_start' => TerminalConnectionEventKind.authPasswordStart,
    'auth_password_rejected' =>
      TerminalConnectionEventKind.authPasswordRejected,
    'auth_password_failed' => TerminalConnectionEventKind.authPasswordFailed,
    'auth_key_start' => TerminalConnectionEventKind.authKeyStart,
    'auth_key_rejected' => TerminalConnectionEventKind.authKeyRejected,
    'auth_key_failed' => TerminalConnectionEventKind.authKeyFailed,
    'auth_passphrase_required' =>
      TerminalConnectionEventKind.authPassphraseRequired,
    'auth_agent_start' => TerminalConnectionEventKind.authAgentStart,
    'auth_agent_identities' => TerminalConnectionEventKind.authAgentIdentities,
    'auth_agent_identity_start' =>
      TerminalConnectionEventKind.authAgentIdentityStart,
    'auth_agent_identity_rejected' =>
      TerminalConnectionEventKind.authAgentIdentityRejected,
    'auth_agent_unavailable' =>
      TerminalConnectionEventKind.authAgentUnavailable,
    'auth_agent_failed' => TerminalConnectionEventKind.authAgentFailed,
    'auth_success' => TerminalConnectionEventKind.authSuccess,
    'auth_failed' => TerminalConnectionEventKind.authFailed,
    'connected' => TerminalConnectionEventKind.connected,
    'ssh_latency_updated' => TerminalConnectionEventKind.sshLatencyUpdated,
    'mosh_echo_enabled' => TerminalConnectionEventKind.moshEchoEnabled,
    'mosh_echo_disabled' => TerminalConnectionEventKind.moshEchoDisabled,
    'mosh_prediction_confirmed' =>
      TerminalConnectionEventKind.moshPredictionConfirmed,
    'mosh_input_state_queued' =>
      TerminalConnectionEventKind.moshInputStateQueued,
    'mosh_screen_committed' => TerminalConnectionEventKind.moshScreenCommitted,
    'mosh_latency_updated' => TerminalConnectionEventKind.moshLatencyUpdated,
    'mosh_udp_peer_confirmed' =>
      TerminalConnectionEventKind.moshUdpPeerConfirmed,
    'mosh_network_switching' =>
      TerminalConnectionEventKind.moshNetworkSwitching,
    'mosh_network_degraded' => TerminalConnectionEventKind.moshNetworkDegraded,
    'mosh_network_restored' => TerminalConnectionEventKind.moshNetworkRestored,
    'retry' => TerminalConnectionEventKind.retry,
    'exit_status' => TerminalConnectionEventKind.exitStatus,
    'session_closed' => TerminalConnectionEventKind.sessionClosed,
    'connection_lost' => TerminalConnectionEventKind.connectionLost,
    'error' => TerminalConnectionEventKind.error,
    _ => TerminalConnectionEventKind.unknown,
  };
}

DateTime? _dateTimeFromJson(Object? value) {
  if (value is String) {
    return DateTime.tryParse(value);
  }
  if (value is int) {
    return DateTime.fromMillisecondsSinceEpoch(value, isUtc: true);
  }
  return null;
}
