import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_replay_sanitizer.dart';

void main() {
  const marker = '\x1b[7m%\x1b[27m';

  test(
    'removes zsh prompt EOL markers without removing normal percent signs',
    () {
      final sanitizer = TerminalReplaySanitizer();
      final output = sanitizer.add(
        Uint8List.fromList(utf8.encode('100%\r\n$marker\$ prompt')),
      );

      expect(
        utf8.decode([...output, ...sanitizer.close()]),
        '100%\r\n\$ prompt',
      );
    },
  );

  test('removes a marker split across capture chunks', () {
    for (var split = 1; split < marker.length; split++) {
      final sanitizer = TerminalReplaySanitizer();
      final first = sanitizer.add(
        Uint8List.fromList(utf8.encode('before${marker.substring(0, split)}')),
      );
      final second = sanitizer.add(
        Uint8List.fromList(utf8.encode('${marker.substring(split)}after')),
      );

      expect(
        utf8.decode([...first, ...second, ...sanitizer.close()]),
        'beforeafter',
      );
    }
  });

  test('preserves an incomplete marker at end of capture', () {
    final sanitizer = TerminalReplaySanitizer();
    final output = sanitizer.add(
      Uint8List.fromList(utf8.encode('before\x1b[7m')),
    );

    expect(utf8.decode([...output, ...sanitizer.close()]), 'before\x1b[7m');
  });

  test('removes internal shell OSC across capture chunks', () {
    final sanitizer = TerminalReplaySanitizer();
    final first = sanitizer.add(
      Uint8List.fromList(utf8.encode('before\x1b]7;file://localhost/t')),
    );
    final second = sanitizer.add(
      Uint8List.fromList(utf8.encode('mp\x07middle\x1b]133;A\x1b\\after')),
    );

    expect(
      utf8.decode([...first, ...second, ...sanitizer.close()]),
      'beforemiddleafter',
    );
  });

  test('preserves unrelated OSC during replay', () {
    final sanitizer = TerminalReplaySanitizer();
    final output = sanitizer.add(
      Uint8List.fromList(utf8.encode('\x1b]0;terminal title\x07prompt')),
    );

    expect(
      utf8.decode([...output, ...sanitizer.close()]),
      '\x1b]0;terminal title\x07prompt',
    );
  });

  test('can force an unfinished alternate screen back to primary', () {
    final sanitizer = TerminalReplaySanitizer();

    expect(
      sanitizer.close(restorePrimaryScreen: true),
      Uint8List.fromList(utf8.encode('\x1b[?1049l')),
    );
  });
}
