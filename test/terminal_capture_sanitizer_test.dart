import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_capture_sanitizer.dart';

void main() {
  test('hidden shell setup is excluded while following output is captured', () {
    final sanitizer = TerminalCaptureSanitizer();
    sanitizer.suppressUntil(Uint8List.fromList(utf8.encode('ready-marker')));

    expect(sanitizer.add(_bytes('hidden setup ready-')), isEmpty);
    expect(
      utf8.decode(sanitizer.add(_bytes('marker\r\nvisible'))),
      '\r\nvisible',
    );
  });

  test('internal shell OSC is preserved in capture across chunks', () {
    final sanitizer = TerminalCaptureSanitizer();
    final first = sanitizer.add(_bytes('before\x1b]7;file://localhost/t'));
    final second = sanitizer.add(_bytes('mp\x07middle\x1b]133;A\x1b\\after'));

    expect(
      utf8.decode([...first, ...second, ...sanitizer.close()]),
      'before\x1b]7;file://localhost/tmp\x07middle\x1b]133;A\x1b\\after',
    );
  });

  test('unrelated OSC remains in capture', () {
    final sanitizer = TerminalCaptureSanitizer();
    final output = sanitizer.add(_bytes('\x1b]0;terminal title\x07prompt'));

    expect(output, _bytes('\x1b]0;terminal title\x07prompt'));
  });
}

Uint8List _bytes(String value) => Uint8List.fromList(utf8.encode(value));
