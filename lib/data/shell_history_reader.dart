import 'dart:convert';
import 'dart:io';

import 'nauterm_data_store.dart';

enum ShellHistoryFormat { zsh, bash, fish, powerShell, ksh, csh, nushell }

class ShellHistoryReader {
  const ShellHistoryReader._();

  static Future<List<ShellHistoryEntry>> readLocal({
    required String shellPath,
    String? homeDirectory,
    int limit = 2000,
  }) async {
    final home =
        homeDirectory ??
        Platform.environment['HOME'] ??
        (Platform.isWindows ? Platform.environment['USERPROFILE'] : null);
    if (home == null || home.isEmpty) return const [];
    final format = formatForShell(shellPath);
    if (format == null) return const [];
    if (format == ShellHistoryFormat.ksh ||
        format == ShellHistoryFormat.csh ||
        format == ShellHistoryFormat.nushell) {
      return _readLocalShellManaged(
        shellPath: shellPath,
        format: format,
        limit: limit,
      );
    }
    if (format == ShellHistoryFormat.powerShell) {
      final configured = await _powerShellHistoryPath(shellPath);
      if (configured != null) {
        final configuredFile = File(configured);
        if (await configuredFile.exists()) {
          return parse(
            await _readHistoryFile(configuredFile),
            format: format,
            readAt: DateTime.now(),
            shellPath: shellPath,
            limit: limit,
          );
        }
      }
    }
    final path = switch (format) {
      ShellHistoryFormat.zsh => '$home/.zsh_history',
      ShellHistoryFormat.bash => '$home/.bash_history',
      ShellHistoryFormat.fish => '$home/.local/share/fish/fish_history',
      ShellHistoryFormat.powerShell =>
        Platform.isWindows
            ? '${Platform.environment['APPDATA'] ?? home}\\Microsoft\\Windows\\PowerShell\\PSReadLine\\ConsoleHost_history.txt'
            : '$home/.local/share/powershell/PSReadLine/ConsoleHost_history.txt',
      ShellHistoryFormat.ksh => '$home/.sh_history',
      ShellHistoryFormat.csh => '$home/.history',
      ShellHistoryFormat.nushell => '',
    };
    final file = File(path);
    if (!await file.exists()) return const [];
    return parse(
      await _readHistoryFile(file),
      format: format,
      readAt: DateTime.now(),
      shellPath: shellPath,
      limit: limit,
    );
  }

  static Future<List<ShellHistoryEntry>> _readLocalShellManaged({
    required String shellPath,
    required ShellHistoryFormat format,
    required int limit,
  }) async {
    final arguments = switch (format) {
      ShellHistoryFormat.ksh => ['-ic', 'fc -l -$limit'],
      ShellHistoryFormat.csh => ['-ic', 'history -h $limit'],
      ShellHistoryFormat.nushell => [
        '-c',
        'history --long | last $limit | to json -r',
      ],
      _ => const <String>[],
    };
    final result = await Process.run(shellPath, arguments);
    if (result.exitCode != 0) return const [];
    final readAt = DateTime.now();
    if (format == ShellHistoryFormat.nushell) {
      return parse(
        result.stdout.toString(),
        format: format,
        readAt: readAt,
        shellPath: shellPath,
        limit: limit,
      );
    }
    final entries = <ShellHistoryEntry>[];
    for (final rawLine in result.stdout.toString().split('\n')) {
      final line = format == ShellHistoryFormat.ksh
          ? rawLine.replaceFirst(RegExp(r'^\s*\d+\s+'), '')
          : rawLine;
      _add(entries, line, readAt, shellPath);
    }
    return entries.length <= limit
        ? entries
        : entries.sublist(entries.length - limit);
  }

  static Future<String?> _powerShellHistoryPath(String shellPath) async {
    final result = await Process.run(shellPath, [
      '-NoLogo',
      '-NoProfile',
      '-Command',
      '(Get-PSReadLineOption).HistorySavePath',
    ]);
    if (result.exitCode != 0) return null;
    final path = result.stdout.toString().trim();
    return path.isEmpty ? null : path;
  }

  // Shell history can contain commands pasted in a legacy locale. Preserve
  // readable entries and replace only malformed byte sequences instead of
  // letting a single command prevent history from loading.
  static Future<String> _readHistoryFile(File file) {
    return file.readAsString(encoding: const Utf8Codec(allowMalformed: true));
  }

  static ShellHistoryFormat? formatForShell(String? shellPath) {
    final name = shellPath
        ?.trim()
        .replaceAll('\\', '/')
        .split('/')
        .last
        .toLowerCase();
    return switch (name) {
      'zsh' => ShellHistoryFormat.zsh,
      'bash' || 'bash.exe' => ShellHistoryFormat.bash,
      'fish' => ShellHistoryFormat.fish,
      'pwsh' ||
      'powershell' ||
      'powershell.exe' => ShellHistoryFormat.powerShell,
      'ksh' || 'ksh93' || 'mksh' => ShellHistoryFormat.ksh,
      'csh' || 'tcsh' => ShellHistoryFormat.csh,
      'nu' => ShellHistoryFormat.nushell,
      _ => null,
    };
  }

  static List<ShellHistoryEntry> parse(
    String content, {
    required ShellHistoryFormat format,
    required DateTime readAt,
    String? shellPath,
    int limit = 2000,
  }) {
    final entries = switch (format) {
      ShellHistoryFormat.zsh => _parseZsh(content, readAt, shellPath),
      ShellHistoryFormat.bash => _parseBash(content, readAt, shellPath),
      ShellHistoryFormat.fish => _parseFish(content, readAt, shellPath),
      ShellHistoryFormat.powerShell ||
      ShellHistoryFormat.csh => _parsePlaintext(content, readAt, shellPath),
      ShellHistoryFormat.ksh => _parseKshListing(content, readAt, shellPath),
      ShellHistoryFormat.nushell => _parseNushellJson(
        content,
        readAt,
        shellPath,
      ),
    };
    return entries.length <= limit
        ? entries
        : entries.sublist(entries.length - limit);
  }

  /// Converts the chronological order used by shell history files into the
  /// newest-first order expected by history navigation and display surfaces.
  /// When a command occurs more than once, the newest occurrence wins.
  static List<ShellHistoryEntry> newestFirst(
    Iterable<ShellHistoryEntry> chronologicalEntries,
  ) {
    final entries = chronologicalEntries is List<ShellHistoryEntry>
        ? chronologicalEntries
        : chronologicalEntries.toList(growable: false);
    final seen = <String>{};
    return [
      for (final entry in entries.reversed)
        if (entry.command.trim().isNotEmpty && seen.add(entry.command.trim()))
          entry,
    ];
  }

  static List<ShellHistoryEntry> _parseZsh(
    String text,
    DateTime readAt,
    String? shellPath,
  ) {
    final result = <ShellHistoryEntry>[];
    final lines = text.split('\n');
    final marker = RegExp(r'^: (\d+):\d+;(.*)$');
    for (var index = 0; index < lines.length; index++) {
      final match = marker.firstMatch(lines[index]);
      if (match == null) {
        _add(result, lines[index], readAt, shellPath);
        continue;
      }
      var command = match.group(2)!;
      while (command.endsWith('\\') && index + 1 < lines.length) {
        command =
            '${command.substring(0, command.length - 1)}\n${lines[++index]}';
      }
      _add(result, command, _seconds(match.group(1), readAt), shellPath);
    }
    return result;
  }

  static List<ShellHistoryEntry> _parseBash(
    String text,
    DateTime readAt,
    String? shellPath,
  ) {
    final result = <ShellHistoryEntry>[];
    DateTime timestamp = readAt;
    for (final line in text.split('\n')) {
      final stamp = RegExp(r'^#(\d+)$').firstMatch(line);
      if (stamp != null) {
        timestamp = _seconds(stamp.group(1), readAt);
      } else {
        _add(result, line, timestamp, shellPath);
      }
    }
    return result;
  }

  static List<ShellHistoryEntry> _parseFish(
    String text,
    DateTime readAt,
    String? shellPath,
  ) {
    final result = <ShellHistoryEntry>[];
    String? command;
    DateTime timestamp = readAt;
    for (final line in text.split('\n')) {
      final cmd = RegExp(r'^- cmd: ?(.*)$').firstMatch(line);
      if (cmd != null) {
        if (command != null) _add(result, command, timestamp, shellPath);
        command = cmd.group(1)!;
        timestamp = readAt;
        continue;
      }
      final when = RegExp(r'^\s+when: (\d+)$').firstMatch(line);
      if (when != null && command != null) {
        timestamp = _seconds(when.group(1), readAt);
      }
    }
    if (command != null) _add(result, command, timestamp, shellPath);
    return result;
  }

  static List<ShellHistoryEntry> _parsePlaintext(
    String text,
    DateTime readAt,
    String? shellPath,
  ) {
    final result = <ShellHistoryEntry>[];
    for (final line in text.split('\n')) {
      _add(result, line, readAt, shellPath);
    }
    return result;
  }

  static List<ShellHistoryEntry> _parseKshListing(
    String text,
    DateTime readAt,
    String? shellPath,
  ) {
    final result = <ShellHistoryEntry>[];
    for (final rawLine in text.split('\n')) {
      _add(
        result,
        rawLine.replaceFirst(RegExp(r'^\s*\d+\s+'), ''),
        readAt,
        shellPath,
      );
    }
    return result;
  }

  static List<ShellHistoryEntry> _parseNushellJson(
    String text,
    DateTime readAt,
    String? shellPath,
  ) {
    final Object? decoded;
    try {
      decoded = jsonDecode(text);
    } on FormatException {
      return const [];
    }
    if (decoded is! List) return const [];
    final result = <ShellHistoryEntry>[];
    for (final value in decoded) {
      if (value is! Map) continue;
      final command = value['command'];
      if (command is! String) continue;
      final timestamp = _nushellTimestamp(value['start_timestamp'], readAt);
      _add(result, command, timestamp, shellPath);
    }
    return result;
  }

  static DateTime _nushellTimestamp(Object? value, DateTime fallback) {
    if (value is int) {
      return DateTime.fromMillisecondsSinceEpoch(value);
    }
    if (value is String) {
      return DateTime.tryParse(value) ?? fallback;
    }
    return fallback;
  }

  static DateTime _seconds(String? value, DateTime fallback) {
    final seconds = int.tryParse(value ?? '');
    return seconds == null
        ? fallback
        : DateTime.fromMillisecondsSinceEpoch(seconds * 1000);
  }

  static void _add(
    List<ShellHistoryEntry> result,
    String command,
    DateTime timestamp,
    String? shellPath,
  ) {
    final normalized = command.trim();
    if (normalized.isEmpty) return;
    result.add(
      ShellHistoryEntry(
        command: normalized,
        shellPath: shellPath,
        createdAt: timestamp,
      ),
    );
  }
}
