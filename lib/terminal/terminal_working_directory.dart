import 'dart:convert';

import 'package:flutter/foundation.dart';

import 'terminal_models.dart';

/// Reports only directory changes, preferring the shell's OSC 7 reports.
class TerminalWorkingDirectory extends ValueNotifier<String?> {
  TerminalWorkingDirectory() : super(null);

  static const _prefix = '\x1b]7;';
  String _pending = '';
  bool _structured = false;

  void addOutput(List<int> bytes) {
    _pending += String.fromCharCodes(bytes);
    while (true) {
      final start = _pending.indexOf(_prefix);
      if (start < 0) {
        if (_pending.length >= _prefix.length) {
          _pending = _pending.substring(_pending.length - _prefix.length + 1);
        }
        return;
      }
      final valueStart = start + _prefix.length;
      final bell = _pending.indexOf('\x07', valueStart);
      final st = _pending.indexOf('\x1b\\', valueStart);
      final end = bell < 0
          ? st
          : st < 0
          ? bell
          : (bell < st ? bell : st);
      if (end < 0) {
        _pending = _pending.length > 65536 ? '' : _pending.substring(start);
        return;
      }
      final payload = _pending.substring(valueStart, end);
      _pending = _pending.substring(end + (end == bell ? 1 : 2));
      try {
        final uri = Uri.parse(utf8.decode(payload.codeUnits));
        if (uri.scheme != 'file' || !uri.path.startsWith('/')) continue;
        final directory = Uri.decodeComponent(uri.path);
        if (directory.contains(RegExp(r'[\x00-\x1f\x7f]'))) continue;
        _structured = true;
        value = directory;
      } on FormatException {
        // Keep the last known directory when a report is malformed.
      }
    }
  }

  void updateSnapshot(TerminalSnapshot snapshot) {
    if (_structured ||
        snapshot.alternateScreen ||
        snapshot.displayOffset != 0) {
      return;
    }
    final directory = terminalPromptWorkingDirectory(snapshot);
    if (directory != null) value = directory;
  }
}

String? terminalPromptWorkingDirectory(TerminalSnapshot snapshot) {
  if (snapshot.rows <= 0 || snapshot.columns <= 0) return null;
  final cursorRow = snapshot.cursor.row.clamp(0, snapshot.rows - 1);
  final firstRow = (cursorRow - 2).clamp(0, cursorRow);
  for (var row = cursorRow; row >= firstRow; row--) {
    final line = StringBuffer();
    for (var column = 0; column < snapshot.columns; column++) {
      final cell = snapshot.cellAt(row, column);
      if (!cell.wideCharSpacer && !cell.leadingWideCharSpacer) {
        line.write(cell.text);
      }
    }
    final text = line.toString().trimRight();
    final marker = RegExp(r'[\$%#❯➜>]\s*$').firstMatch(text);
    if (marker == null) continue;
    final prompt = text.substring(0, marker.start).trimRight();
    for (var part in prompt.split(RegExp(r'\s+')).reversed) {
      part = part.replaceAll(RegExp(r'^[\[(]+|[\])]+$'), '');
      if (!part.startsWith('/') && !part.startsWith('~')) {
        final colon = part.indexOf(':');
        if (colon >= 0) part = part.substring(colon + 1);
      }
      if (part == '~' || part.startsWith('~/') || part.startsWith('/')) {
        return part;
      }
    }
  }
  return null;
}
