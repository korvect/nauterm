import 'dart:convert';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:flutter/material.dart';

import 'terminal_models.dart';
import 'terminal_theme.dart';

enum TerminalRecordingEventType { input, connection, output, exit }

const Object _sentinel = Object();

@immutable
class TerminalRecordingEvent {
  const TerminalRecordingEvent({
    required this.timestamp,
    required this.type,
    required this.message,
    this.connectionKind,
    this.data,
  });

  factory TerminalRecordingEvent.fromJson(Map<String, Object?> json) {
    return TerminalRecordingEvent(
      timestamp: DateTime.parse(json['timestamp'] as String),
      type: _recordingEventTypeFromJson(json['type'] as String?),
      message: json['message'] as String? ?? '',
      connectionKind: json['connection_kind'] as String?,
      data: json['data'] as String?,
    );
  }

  final DateTime timestamp;
  final TerminalRecordingEventType type;
  final String message;
  final String? connectionKind;
  final String? data;

  Map<String, Object?> toJson() => {
    'timestamp': timestamp.toIso8601String(),
    'type': type.name,
    'message': message,
    'connection_kind': connectionKind,
    'data': data,
  };
}

@immutable
class TerminalReplayFrame {
  const TerminalReplayFrame({required this.offset, required this.snapshot});

  factory TerminalReplayFrame.fromJson(Map<String, Object?> json) {
    return TerminalReplayFrame(
      offset: Duration(milliseconds: json['offset_ms'] as int? ?? 0),
      snapshot: terminalSnapshotFromJson(
        (json['snapshot'] as Map).cast<String, Object?>(),
      ),
    );
  }

  final Duration offset;
  final TerminalSnapshot snapshot;

  Map<String, Object?> toJson() => {
    'offset_ms': offset.inMilliseconds,
    'snapshot': terminalSnapshotToJson(snapshot),
  };
}

@immutable
class TerminalShellHistoryEntry {
  const TerminalShellHistoryEntry({
    required this.id,
    required this.sessionId,
    required this.timestamp,
    required this.command,
    required this.title,
    this.target,
  });

  factory TerminalShellHistoryEntry.fromJson(Map<String, Object?> json) {
    return TerminalShellHistoryEntry(
      id: json['id'] as String? ?? '',
      sessionId: json['session_id'] as String? ?? '',
      timestamp: DateTime.parse(json['timestamp'] as String),
      command: json['command'] as String? ?? '',
      title: json['title'] as String? ?? 'Terminal',
      target: json['target'] as String?,
    );
  }

  final String id;
  final String sessionId;
  final DateTime timestamp;
  final String command;
  final String title;
  final String? target;

  Map<String, Object?> toJson() => {
    'id': id,
    'session_id': sessionId,
    'timestamp': timestamp.toIso8601String(),
    'command': command,
    'title': title,
    'target': target,
  };
}

@immutable
class TerminalSessionRecording {
  const TerminalSessionRecording({
    required this.id,
    required this.title,
    required this.startedAt,
    required this.frames,
    required this.events,
    required this.shellHistory,
    this.target,
    this.themeId,
    this.captureBase64 = '',
    this.captureBytes,
    this.endedAt,
  });

  factory TerminalSessionRecording.fromJson(Map<String, Object?> json) {
    return TerminalSessionRecording(
      id: json['id'] as String? ?? '',
      title: json['title'] as String? ?? 'Terminal',
      target: json['target'] as String?,
      themeId: json['theme_id'] as String?,
      startedAt: DateTime.parse(json['started_at'] as String),
      endedAt: json['ended_at'] == null
          ? null
          : DateTime.parse(json['ended_at'] as String),
      captureBase64: json['capture_base64'] as String? ?? '',
      captureBytes: json['capture_bytes'] as int?,
      frames: [
        for (final frame in (json['frames'] as List? ?? const []))
          TerminalReplayFrame.fromJson((frame as Map).cast<String, Object?>()),
      ],
      events: [
        for (final event in (json['events'] as List? ?? const []))
          TerminalRecordingEvent.fromJson(
            (event as Map).cast<String, Object?>(),
          ),
      ],
      shellHistory: [
        for (final entry in (json['shell_history'] as List? ?? const []))
          TerminalShellHistoryEntry.fromJson(
            (entry as Map).cast<String, Object?>(),
          ),
      ],
    );
  }

  final String id;
  final String title;
  final String? target;
  final String? themeId;
  final DateTime startedAt;
  final DateTime? endedAt;
  final String captureBase64;
  final int? captureBytes;
  final List<TerminalReplayFrame> frames;
  final List<TerminalRecordingEvent> events;
  final List<TerminalShellHistoryEntry> shellHistory;

  bool get isActive => endedAt == null;

  int get captureByteCount {
    final storedCount = captureBytes;
    if (storedCount != null) {
      return storedCount;
    }
    if (captureBase64.isEmpty) {
      return 0;
    }
    try {
      return base64Decode(captureBase64).length;
    } on Object {
      return 0;
    }
  }

  Duration get duration {
    final end = endedAt ?? DateTime.now();
    return end.difference(startedAt);
  }

  List<TerminalRecordingEvent> get recordedInputEvents {
    return [
      for (final event in events)
        if (event.type == TerminalRecordingEventType.input &&
            event.data != null &&
            event.data!.isNotEmpty)
          event,
    ];
  }

  TerminalSessionRecording copyWith({
    String? title,
    Object? target = _sentinel,
    Object? themeId = _sentinel,
    DateTime? startedAt,
    Object? endedAt = _sentinel,
    String? captureBase64,
    Object? captureBytes = _sentinel,
    List<TerminalReplayFrame>? frames,
    List<TerminalRecordingEvent>? events,
    List<TerminalShellHistoryEntry>? shellHistory,
  }) {
    return TerminalSessionRecording(
      id: id,
      title: title ?? this.title,
      target: identical(target, _sentinel) ? this.target : target as String?,
      themeId: identical(themeId, _sentinel)
          ? this.themeId
          : themeId as String?,
      startedAt: startedAt ?? this.startedAt,
      endedAt: identical(endedAt, _sentinel)
          ? this.endedAt
          : endedAt as DateTime?,
      captureBase64: captureBase64 ?? this.captureBase64,
      captureBytes: identical(captureBytes, _sentinel)
          ? this.captureBytes
          : captureBytes as int?,
      frames: frames ?? this.frames,
      events: events ?? this.events,
      shellHistory: shellHistory ?? this.shellHistory,
    );
  }

  Map<String, Object?> toJson() => {
    'id': id,
    'title': title,
    'target': target,
    'theme_id': themeId,
    'started_at': startedAt.toIso8601String(),
    'ended_at': endedAt?.toIso8601String(),
    'capture_base64': captureBase64,
    'capture_bytes': captureByteCount,
    'frames': [for (final frame in frames) frame.toJson()],
    'events': [for (final event in events) event.toJson()],
    'shell_history': [for (final entry in shellHistory) entry.toJson()],
  };

  Map<String, Object?> toMetadataJson({String? captureFile}) => {
    'id': id,
    'title': title,
    'target': target,
    'theme_id': themeId,
    'started_at': startedAt.toIso8601String(),
    'ended_at': endedAt?.toIso8601String(),
    'capture_file': captureFile,
    'capture_bytes': captureByteCount,
    'events': [for (final event in events) event.toJson()],
    'shell_history': [for (final entry in shellHistory) entry.toJson()],
  };
}

class TerminalSessionRecorder {
  TerminalSessionRecorder({
    required this.title,
    this.target,
    this.themeId,
    this.onChanged,
    this.onConnectionEstablished,
    this.onShellHistoryEntry,
    this.onCaptureBytes,
    this.captureEnabled = true,
  }) : id = _recordingId(),
       startedAt = DateTime.now();

  final String id;
  final String title;
  final String? target;
  final String? themeId;
  final DateTime startedAt;
  final VoidCallback? onChanged;
  final VoidCallback? onConnectionEstablished;
  final ValueChanged<TerminalShellHistoryEntry>? onShellHistoryEntry;
  final ValueChanged<Uint8List>? onCaptureBytes;
  final bool captureEnabled;

  DateTime? _endedAt;
  final List<TerminalReplayFrame> _frames = [];
  List<TerminalRecordingEvent> _events = [];
  List<TerminalShellHistoryEntry> _shellHistory = [];
  final BytesBuilder _capture = BytesBuilder(copy: false);
  int? _storedCaptureBytes;
  String _shellIntegrationOutput = '';
  String? _lastSnapshotSignature;
  bool _connectionEstablishedNotified = false;

  bool get isActive => _endedAt == null;

  TerminalSessionRecording snapshot() {
    return TerminalSessionRecording(
      id: id,
      title: title,
      target: target,
      themeId: themeId,
      startedAt: startedAt,
      endedAt: _endedAt,
      captureBase64: base64Encode(_capture.toBytes()),
      captureBytes: _capture.length == 0
          ? _storedCaptureBytes
          : _capture.length,
      frames: List.unmodifiable(_frames),
      events: List.unmodifiable(_events),
      shellHistory: List.unmodifiable(_shellHistory),
    );
  }

  void recordSnapshot(TerminalSnapshot snapshot) {
    if (!isActive) {
      return;
    }

    final signature = _snapshotSignature(snapshot);
    if (signature == _lastSnapshotSignature) {
      return;
    }
    _lastSnapshotSignature = signature;

    final frame = TerminalReplayFrame(
      offset: DateTime.now().difference(startedAt),
      snapshot: snapshot,
    );
    if (_frames.isEmpty) {
      _frames.add(frame);
    } else {
      _frames[0] = frame;
    }
    onChanged?.call();
  }

  void recordInput(String data, {bool sensitive = false}) {
    if (!isActive || data.isEmpty) {
      return;
    }
    if (sensitive) {
      _discardPendingSensitiveInput();
      return;
    }

    _events.add(
      TerminalRecordingEvent(
        timestamp: DateTime.now(),
        type: TerminalRecordingEventType.input,
        message: _displayInput(data),
        data: data,
      ),
    );
    _trimEvents();
    onChanged?.call();
  }

  void recordOutput(String data) {
    if (!isActive || data.trim().isEmpty) {
      return;
    }

    _events.add(
      TerminalRecordingEvent(
        timestamp: DateTime.now(),
        type: TerminalRecordingEventType.output,
        message: _displayInput(data),
      ),
    );
    _trimEvents();
    onChanged?.call();
  }

  /// Records commands reported by Nauterm shell integration.
  ///
  /// The Nauterm protocol lets an interactive shell emit
  /// `OSC 4545;CommandStarted;[shell instance;]<base64 UTF-8 command>`
  /// immediately before it runs a command. This is authoritative after
  /// completion and editing.
  void recordShellIntegrationOutput(Uint8List bytes) {
    if (!isActive || bytes.isEmpty) {
      return;
    }

    _shellIntegrationOutput += latin1.decode(bytes, allowInvalid: true);
    const prefix = '\x1b]4545;';
    const maxBufferedBytes = 16 * 1024;

    while (true) {
      final start = _shellIntegrationOutput.indexOf(prefix);
      if (start < 0) {
        final retain = prefix.length - 1;
        if (_shellIntegrationOutput.length > retain) {
          _shellIntegrationOutput = _shellIntegrationOutput.substring(
            _shellIntegrationOutput.length - retain,
          );
        }
        return;
      }

      final payloadStart = start + prefix.length;
      final bell = _shellIntegrationOutput.indexOf('\x07', payloadStart);
      final stringTerminator = _shellIntegrationOutput.indexOf(
        '\x1b\\',
        payloadStart,
      );
      final terminator = switch ((bell, stringTerminator)) {
        (-1, -1) => -1,
        (-1, final value) => value,
        (final value, -1) => value,
        (final left, final right) => left < right ? left : right,
      };
      if (terminator < 0) {
        if (_shellIntegrationOutput.length > maxBufferedBytes) {
          _shellIntegrationOutput = _shellIntegrationOutput.substring(start);
        }
        return;
      }

      final payload = _shellIntegrationOutput.substring(
        payloadStart,
        terminator,
      );
      final terminatorLength = terminator == bell ? 1 : 2;
      _shellIntegrationOutput = _shellIntegrationOutput.substring(
        terminator + terminatorLength,
      );
      _recordShellIntegrationPayload(payload);
    }
  }

  void recordCaptureBytes(Uint8List bytes) {
    if (!captureEnabled || !isActive || bytes.isEmpty) {
      return;
    }
    final writer = onCaptureBytes;
    if (writer == null) {
      _capture.add(bytes);
      _storedCaptureBytes = null;
    } else {
      writer(bytes);
      _storedCaptureBytes = (_storedCaptureBytes ?? 0) + bytes.length;
    }
    onChanged?.call();
  }

  void recordConnectionEvent(TerminalConnectionEvent event) {
    if (!isActive) {
      return;
    }

    final line = event.logLine.trim();
    if (line.isEmpty) {
      return;
    }
    _events.add(
      TerminalRecordingEvent(
        timestamp: DateTime.now(),
        type: TerminalRecordingEventType.connection,
        message: line,
        connectionKind: event.kind.name,
      ),
    );
    _trimEvents();
    if (event.kind == TerminalConnectionEventKind.connected &&
        !_connectionEstablishedNotified) {
      _connectionEstablishedNotified = true;
      onConnectionEstablished?.call();
    }
    onChanged?.call();
  }

  void finish({String? message}) {
    if (!isActive) {
      return;
    }
    final now = DateTime.now();
    _endedAt = now;
    _events.add(
      TerminalRecordingEvent(
        timestamp: now,
        type: TerminalRecordingEventType.exit,
        message: message ?? 'Session ended.',
      ),
    );
    _trimEvents();
    onChanged?.call();
  }

  void _discardPendingSensitiveInput() {
    for (var index = _events.length - 1; index >= 0; index--) {
      final event = _events[index];
      if (event.type != TerminalRecordingEventType.input) {
        continue;
      }
      final data = event.data ?? '';
      final boundary = [
        data.lastIndexOf('\r'),
        data.lastIndexOf('\n'),
      ].reduce((left, right) => left > right ? left : right);
      if (boundary == data.length - 1) {
        break;
      }
      _events.removeAt(index);
      if (boundary >= 0) {
        break;
      }
    }
  }

  void _recordShellIntegrationPayload(String payload) {
    const commandStarted = 'CommandStarted;';
    if (!payload.startsWith(commandStarted)) {
      return;
    }
    final value = payload.substring(commandStarted.length);
    final separator = value.indexOf(';');
    final encoded = separator < 0 ? value : value.substring(separator + 1);
    String command;
    try {
      command = utf8.decode(base64Decode(encoded)).trim();
    } on FormatException {
      return;
    }
    if (command.isEmpty || _looksLikeSecret(command)) {
      return;
    }

    final timestamp = DateTime.now();
    final entry = TerminalShellHistoryEntry(
      id: '$id:${timestamp.microsecondsSinceEpoch}',
      sessionId: id,
      timestamp: timestamp,
      command: command,
      title: title,
      target: target,
    );
    _shellHistory.add(entry);
    if (_shellHistory.length > 2000) {
      _shellHistory = _shellHistory.sublist(_shellHistory.length - 2000);
    }
    onShellHistoryEntry?.call(entry);
  }

  bool _looksLikeSecret(String command) {
    final lower = command.toLowerCase();
    return lower.startsWith('password:') ||
        lower.startsWith('passphrase:') ||
        lower.contains(' token=') ||
        lower.contains(' password=') ||
        lower.contains(' passwd=');
  }

  void _trimEvents() {
    if (_events.length > 1200) {
      _events = _events.sublist(_events.length - 1200);
    }
  }
}

Map<String, Object?> terminalSnapshotToJson(TerminalSnapshot snapshot) {
  return {
    'columns': snapshot.columns,
    'rows': snapshot.rows,
    'title': snapshot.title,
    'clipboard': snapshot.clipboardText,
    'bell_count': snapshot.bellCount,
    'cursor': {
      'column': snapshot.cursor.column,
      'row': snapshot.cursor.row,
      'visible': snapshot.cursor.visible,
      'shape': snapshot.cursor.shape.name,
      'color': snapshot.cursor.color.toARGB32(),
      'blinking': snapshot.cursor.blinking,
    },
    'keyboard_mode': {
      'application_cursor': snapshot.keyboardMode.applicationCursor,
      'application_keypad': snapshot.keyboardMode.applicationKeypad,
      'bracketed_paste': snapshot.keyboardMode.bracketedPaste,
      'focus_events': snapshot.keyboardMode.focusEvents,
      'mouse_report_click': snapshot.keyboardMode.mouseReportClick,
      'mouse_drag': snapshot.keyboardMode.mouseDrag,
      'mouse_motion': snapshot.keyboardMode.mouseMotion,
      'sgr_mouse': snapshot.keyboardMode.sgrMouse,
    },
    'input_echo_enabled': snapshot.inputEchoEnabled,
    'alternate_screen': snapshot.alternateScreen,
    'cells': [
      for (final cell in snapshot.cells)
        [
          cell.text,
          cell.foreground.toARGB32(),
          cell.background.toARGB32(),
          cell.flags,
          cell.hyperlink,
        ],
    ],
  };
}

TerminalSnapshot terminalSnapshotFromJson(Map<String, Object?> json) {
  final columns = json['columns'] as int? ?? 80;
  final rows = json['rows'] as int? ?? 24;
  final cursor = (json['cursor'] as Map?)?.cast<String, Object?>();
  final keyboardMode = (json['keyboard_mode'] as Map?)?.cast<String, Object?>();
  final cellsJson = json['cells'] as List? ?? const [];
  final cells = [
    for (final rawCell in cellsJson) _terminalCellFromJson(rawCell),
  ];
  final expectedLength = columns * rows;
  if (cells.length < expectedLength) {
    cells.addAll(
      List<TerminalCell>.filled(
        expectedLength - cells.length,
        const TerminalCell.empty(),
      ),
    );
  }

  return TerminalSnapshot(
    columns: columns,
    rows: rows,
    title: json['title'] as String? ?? '',
    clipboardText: json['clipboard'] as String? ?? '',
    bellCount: json['bell_count'] as int? ?? 0,
    cells: List.unmodifiable(cells.take(expectedLength)),
    cursor: TerminalCursor(
      column: cursor?['column'] as int? ?? 0,
      row: cursor?['row'] as int? ?? 0,
      visible: cursor?['visible'] as bool? ?? true,
      shape: _cursorShapeFromJson(cursor?['shape'] as String?),
      color: Color(
        cursor?['color'] as int? ?? terminalDefaultCursor.toARGB32(),
      ),
      blinking: cursor?['blinking'] as bool? ?? false,
    ),
    keyboardMode: TerminalKeyboardMode(
      applicationCursor: keyboardMode?['application_cursor'] as bool? ?? false,
      applicationKeypad: keyboardMode?['application_keypad'] as bool? ?? false,
      bracketedPaste: keyboardMode?['bracketed_paste'] as bool? ?? false,
      focusEvents: keyboardMode?['focus_events'] as bool? ?? false,
      mouseReportClick: keyboardMode?['mouse_report_click'] as bool? ?? false,
      mouseDrag: keyboardMode?['mouse_drag'] as bool? ?? false,
      mouseMotion: keyboardMode?['mouse_motion'] as bool? ?? false,
      sgrMouse: keyboardMode?['sgr_mouse'] as bool? ?? false,
    ),
    inputEchoEnabled: json['input_echo_enabled'] as bool? ?? true,
    alternateScreen: json['alternate_screen'] as bool? ?? false,
  );
}

TerminalCell _terminalCellFromJson(Object? rawCell) {
  if (rawCell is List) {
    return TerminalCell(
      text: rawCell.elementAtOrNull(0) as String? ?? ' ',
      foreground: Color(
        rawCell.elementAtOrNull(1) as int? ??
            terminalDefaultForeground.toARGB32(),
      ),
      background: Color(
        rawCell.elementAtOrNull(2) as int? ??
            terminalDefaultBackground.toARGB32(),
      ),
      flags: rawCell.elementAtOrNull(3) as int? ?? 0,
      hyperlink: rawCell.elementAtOrNull(4) as String? ?? '',
    );
  }

  final cell = (rawCell as Map?)?.cast<String, Object?>();
  return TerminalCell(
    text: cell?['text'] as String? ?? ' ',
    foreground: Color(
      cell?['foreground'] as int? ?? terminalDefaultForeground.toARGB32(),
    ),
    background: Color(
      cell?['background'] as int? ?? terminalDefaultBackground.toARGB32(),
    ),
    flags: cell?['flags'] as int? ?? 0,
    hyperlink: cell?['hyperlink'] as String? ?? '',
  );
}

TerminalCursorShape _cursorShapeFromJson(String? value) {
  return TerminalCursorShape.values.firstWhere(
    (shape) => shape.name == value,
    orElse: () => TerminalCursorShape.block,
  );
}

TerminalRecordingEventType _recordingEventTypeFromJson(String? value) {
  return TerminalRecordingEventType.values.firstWhere(
    (type) => type.name == value,
    orElse: () => TerminalRecordingEventType.input,
  );
}

String _snapshotSignature(TerminalSnapshot snapshot) {
  final buffer = StringBuffer()
    ..write(snapshot.columns)
    ..write('x')
    ..write(snapshot.rows)
    ..write('|')
    ..write(snapshot.cursor.column)
    ..write(',')
    ..write(snapshot.cursor.row)
    ..write(',')
    ..write(snapshot.cursor.visible)
    ..write(',')
    ..write(snapshot.cursor.shape.name)
    ..write('|')
    ..write(snapshot.title)
    ..write('|')
    ..write(snapshot.clipboardText)
    ..write('|')
    ..write(snapshot.bellCount)
    ..write('|');
  for (final cell in snapshot.cells) {
    buffer
      ..write(cell.text)
      ..write('\u{1f}')
      ..write(cell.foreground.toARGB32())
      ..write('\u{1f}')
      ..write(cell.background.toARGB32())
      ..write('\u{1f}')
      ..write(cell.flags)
      ..write('\u{1f}')
      ..write(cell.hyperlink)
      ..write('\u{1e}');
  }
  return buffer.toString();
}

String _displayInput(String data) {
  final sanitized = data
      .replaceAll('\x1b[200~', '')
      .replaceAll('\x1b[201~', '')
      .replaceAll('\r', r'\r')
      .replaceAll('\n', r'\n');
  if (sanitized.length <= 160) {
    return sanitized;
  }
  return '${sanitized.substring(0, math.min(157, sanitized.length))}...';
}

String _recordingId() {
  final millis = DateTime.now().toUtc().millisecondsSinceEpoch;
  final random = math.Random.secure();
  final bytes = List<int>.generate(16, (_) => random.nextInt(256));
  bytes[0] = (millis >> 40) & 0xff;
  bytes[1] = (millis >> 32) & 0xff;
  bytes[2] = (millis >> 24) & 0xff;
  bytes[3] = (millis >> 16) & 0xff;
  bytes[4] = (millis >> 8) & 0xff;
  bytes[5] = millis & 0xff;
  bytes[6] = 0x70 | (bytes[6] & 0x0f);
  bytes[8] = 0x80 | (bytes[8] & 0x3f);
  final hex = [
    for (final byte in bytes) byte.toRadixString(16).padLeft(2, '0'),
  ].join();
  return '${hex.substring(0, 8)}-${hex.substring(8, 12)}-${hex.substring(12, 16)}-${hex.substring(16, 20)}-${hex.substring(20)}';
}
