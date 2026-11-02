import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'nauterm_data_store.dart';

/// Persistent, display-oriented command history for snippets.
///
/// Each row is `millisecondsSinceEpoch;command`.  It intentionally contains
/// no host credentials or JSON metadata; connection-specific, non-deduped
/// history stays in memory on the active terminal controller.
class ShellHistoryFileStore {
  const ShellHistoryFileStore(this.file);

  static const int defaultLimit = 10000;
  static const int _maximumBytes = 16 * 1024 * 1024;
  static const int _maximumCommandBytes = 16 * 1024;
  static final Map<String, Future<void>> _operations = {};

  final File file;

  Future<List<ShellHistoryEntry>> read({int limit = defaultLimit}) =>
      _withLock(() => _read(limit: limit));

  Future<List<ShellHistoryEntry>> _read({required int limit}) async {
    if (!await file.exists()) {
      return const [];
    }
    if (await file.length() > _maximumBytes) {
      throw const FormatException('The shell history file is too large.');
    }
    final text = await file.readAsString();
    final entries = <ShellHistoryEntry>[];
    for (final line in text.split('\n')) {
      final separator = line.indexOf(';');
      if (separator <= 0) continue;
      final timestamp = int.tryParse(line.substring(0, separator));
      final command = _unescape(line.substring(separator + 1)).trim();
      if (timestamp == null || command.isEmpty) continue;
      entries.add(
        ShellHistoryEntry(
          sourceId: 'file:$timestamp:$command',
          command: command,
          createdAt: DateTime.fromMillisecondsSinceEpoch(timestamp),
        ),
      );
    }
    return _dedupeNewest(entries, limit: limit);
  }

  Future<List<ShellHistoryEntry>> append(
    ShellHistoryEntry entry, {
    int limit = defaultLimit,
  }) async {
    return merge([entry], limit: limit);
  }

  Future<void> clear() => _withLock(() async {
    if (await file.exists()) await file.delete();
  });

  Future<List<ShellHistoryEntry>> merge(
    Iterable<ShellHistoryEntry> entries, {
    int limit = defaultLimit,
  }) => _withLock(() async {
    final current = await _read(limit: limit);
    final incoming = entries.toList(growable: false).reversed;
    final updated = _dedupeNewest([...incoming, ...current], limit: limit);
    await file.parent.create(recursive: true);
    final rows = [
      for (final item in updated)
        '${(item.createdAt ?? DateTime.now()).millisecondsSinceEpoch};${_escape(item.command.trim())}',
    ].join('\n');
    // Rename under the same sidecar lock as RS. Readers cannot observe a
    // truncated file and concurrent writers merge the latest committed rows.
    final temporary = File(
      '${file.path}.${DateTime.now().microsecondsSinceEpoch}.$pid.tmp',
    );
    try {
      // Preserve existing permissions/ACLs when replacing a history file that
      // may have been created with a private mode by the Rust client.
      if (await file.exists()) await file.copy(temporary.path);
      await temporary.writeAsString('$rows\n', flush: true);
      await temporary.rename(file.path);
    } finally {
      if (await temporary.exists()) await temporary.delete();
    }
    return updated;
  });

  Future<T> _withLock<T>(Future<T> Function() operation) async {
    final key = file.absolute.path;
    final previous = _operations[key] ?? Future<void>.value();
    final completion = Completer<void>();
    final queued = completion.future;
    _operations[key] = queued;
    await previous;
    try {
      await file.parent.create(recursive: true);
      final lock = await File('${file.path}.lock').open(mode: FileMode.append);
      try {
        await lock.lock(FileLock.blockingExclusive);
        try {
          return await operation();
        } finally {
          await lock.unlock();
        }
      } finally {
        await lock.close();
      }
    } finally {
      completion.complete();
      if (identical(_operations[key], queued)) _operations.remove(key);
    }
  }

  List<ShellHistoryEntry> _dedupeNewest(
    Iterable<ShellHistoryEntry> entries, {
    required int limit,
  }) {
    final count = limit.clamp(0, defaultLimit);
    if (count == 0) return const [];
    final now = DateTime.now().millisecondsSinceEpoch;
    final ordered = entries.indexed.toList(growable: false)
      ..sort((left, right) {
        final time = (right.$2.createdAt?.millisecondsSinceEpoch ?? now)
            .compareTo(left.$2.createdAt?.millisecondsSinceEpoch ?? now);
        return time == 0 ? left.$1.compareTo(right.$1) : time;
      });
    final seen = <String>{};
    final result = <ShellHistoryEntry>[];
    var bytes = 0;
    for (final (_, entry) in ordered) {
      final command = entry.command.trim();
      if (command.isEmpty ||
          command.contains('\u0000') ||
          command.length > _maximumCommandBytes ||
          utf8.encode(command).length > _maximumCommandBytes ||
          !seen.add(command)) {
        continue;
      }
      bytes += utf8.encode(_escape(command)).length + 32;
      if (bytes > _maximumBytes) break;
      result.add(entry);
      if (result.length == count) break;
    }
    return result;
  }

  String _escape(String value) => value
      .replaceAll('\\', '\\\\')
      .replaceAll('\n', '\\n')
      .replaceAll('\r', '\\r');

  String _unescape(String value) {
    final output = StringBuffer();
    var escaped = false;
    for (final codeUnit in value.codeUnits) {
      if (escaped) {
        switch (codeUnit) {
          case 0x6e:
            output.write('\n');
          case 0x72:
            output.write('\r');
          case 0x5c:
            output.write('\\');
          default:
            output.write('\\');
            output.writeCharCode(codeUnit);
        }
        escaped = false;
      } else if (codeUnit == 0x5c) {
        escaped = true;
      } else {
        output.writeCharCode(codeUnit);
      }
    }
    if (escaped) output.write('\\');
    return output.toString();
  }
}
