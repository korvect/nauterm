import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';

enum NautermLogLevel { debug, info, warning, error }

/// Privacy-conscious runtime logging for local diagnostics.
///
/// Debug builds record every level. Release builds omit only debug entries.
/// Log files stay on the device and are never uploaded by this class.
class NautermLog {
  NautermLog._();

  static const int _maximumFileBytes = 1024 * 1024;
  static const int _archiveCount = 3;
  static const String _fileName = 'nauterm.log';

  static Directory? _directory;
  static Future<void> _writes = Future<void>.value();
  static int _operationSequence = 0;

  static void initialize(Directory directory) {
    _directory = directory;
    _writes = _writes.then((_) => directory.create(recursive: true));
  }

  static NautermLogOperation begin(
    String area,
    String action, {
    Map<String, Object?> fields = const {},
  }) {
    final operation = NautermLogOperation._(
      id: 'op-${++_operationSequence}',
      area: area,
      action: action,
      fields: fields,
    );
    info(area, '$action started.', fields: operation._startFields);
    return operation;
  }

  static void debug(
    String area,
    String message, {
    Map<String, Object?> fields = const {},
  }) => _record(NautermLogLevel.debug, area, message, fields: fields);

  static void info(
    String area,
    String message, {
    Map<String, Object?> fields = const {},
  }) => _record(NautermLogLevel.info, area, message, fields: fields);

  static void warning(
    String area,
    String message, {
    Object? error,
    StackTrace? stackTrace,
    Map<String, Object?> fields = const {},
  }) => _record(
    NautermLogLevel.warning,
    area,
    message,
    error: error,
    stackTrace: stackTrace,
    fields: fields,
  );

  static void error(
    String area,
    String message, {
    Object? error,
    StackTrace? stackTrace,
    Map<String, Object?> fields = const {},
  }) => _record(
    NautermLogLevel.error,
    area,
    message,
    error: error,
    stackTrace: stackTrace,
    fields: fields,
  );

  static Future<void> flush() => _writes;

  static void _record(
    NautermLogLevel level,
    String area,
    String message, {
    Object? error,
    StackTrace? stackTrace,
    Map<String, Object?> fields = const {},
  }) {
    if (kReleaseMode && level == NautermLogLevel.debug) {
      return;
    }
    final timestamp = DateTime.now().toUtc().toIso8601String();
    final safeFields = _safeFields(fields);
    if (error != null) {
      safeFields['error_type'] = error.runtimeType.toString();
    }
    final suffix = safeFields.isEmpty ? '' : ' ${jsonEncode(safeFields)}';
    final line =
        '$timestamp ${level.name.toUpperCase()} [$area] '
        '$message$suffix';

    if (kDebugMode) {
      debugPrint(line);
      if (stackTrace != null) {
        debugPrintStack(stackTrace: stackTrace);
      }
    }

    final directory = _directory;
    if (directory == null) {
      return;
    }
    _writes = _writes.then((_) => _append(directory, '$line\n')).catchError((
      _,
    ) {
      // Logging must never affect application behavior.
    });
  }

  static Map<String, Object?> _safeFields(Map<String, Object?> fields) {
    final result = <String, Object?>{};
    for (final entry in fields.entries) {
      final key = entry.key.trim();
      if (key.isEmpty) {
        continue;
      }
      if (_sensitiveField.hasMatch(key)) {
        result[key] = '[redacted]';
        continue;
      }
      final value = entry.value;
      result[key] = switch (value) {
        null || bool() || num() || String() => value,
        Enum() => value.name,
        _ => value.runtimeType.toString(),
      };
    }
    return result;
  }

  static final RegExp _sensitiveField = RegExp(
    r'(secret|token|password|passphrase|master.?key|private.?key|credential|'
    r'authorization|clipboard|command|content|output|host(name)?|ip|path|url|'
    r'user(name)?|gist.?id)',
    caseSensitive: false,
  );

  static Future<void> _append(Directory directory, String line) async {
    await directory.create(recursive: true);
    final file = File('${directory.path}${Platform.pathSeparator}$_fileName');
    final currentLength = await file.exists() ? await file.length() : 0;
    if (currentLength + utf8.encode(line).length > _maximumFileBytes) {
      await _rotate(directory, file);
    }
    await file.writeAsString(line, mode: FileMode.append, flush: true);
  }

  static Future<void> _rotate(Directory directory, File current) async {
    final oldest = File(
      '${directory.path}${Platform.pathSeparator}$_fileName.$_archiveCount',
    );
    if (await oldest.exists()) {
      await oldest.delete();
    }
    for (var index = _archiveCount - 1; index >= 1; index -= 1) {
      final source = File(
        '${directory.path}${Platform.pathSeparator}$_fileName.$index',
      );
      if (await source.exists()) {
        await source.rename(
          '${directory.path}${Platform.pathSeparator}$_fileName.${index + 1}',
        );
      }
    }
    if (await current.exists()) {
      await current.rename(
        '${directory.path}${Platform.pathSeparator}$_fileName.1',
      );
    }
  }
}

class NautermLogOperation {
  NautermLogOperation._({
    required this.id,
    required this.area,
    required this.action,
    required this._fields,
  }) : _stopwatch = Stopwatch()..start();

  final String id;
  final String area;
  final String action;
  final Map<String, Object?> _fields;
  final Stopwatch _stopwatch;
  bool _finished = false;

  Map<String, Object?> get _startFields => {'operation_id': id, ..._fields};

  void succeed({Map<String, Object?> fields = const {}}) {
    if (_finished) return;
    _finished = true;
    _stopwatch.stop();
    NautermLog.info(
      area,
      '$action completed.',
      fields: {
        'operation_id': id,
        'duration_ms': _stopwatch.elapsedMilliseconds,
        ..._fields,
        ...fields,
      },
    );
  }

  void fail(
    Object error, {
    StackTrace? stackTrace,
    Map<String, Object?> fields = const {},
  }) {
    if (_finished) return;
    _finished = true;
    _stopwatch.stop();
    NautermLog.error(
      area,
      '$action failed.',
      error: error,
      stackTrace: stackTrace,
      fields: {
        'operation_id': id,
        'duration_ms': _stopwatch.elapsedMilliseconds,
        ..._fields,
        ...fields,
      },
    );
  }

  void warn({Map<String, Object?> fields = const {}}) {
    if (_finished) return;
    _finished = true;
    _stopwatch.stop();
    NautermLog.warning(
      area,
      '$action did not complete.',
      fields: {
        'operation_id': id,
        'duration_ms': _stopwatch.elapsedMilliseconds,
        ..._fields,
        ...fields,
      },
    );
  }
}
