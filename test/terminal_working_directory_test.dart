import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_working_directory.dart';

void main() {
  test('only a changed directory emits a notification', () {
    final directory = TerminalWorkingDirectory();
    addTearDown(directory.dispose);
    final changes = <String?>[];
    directory.addListener(() => changes.add(directory.value));
    directory.addOutput(utf8.encode('\x1b]7;file://localhost/tmp\x07'));
    directory.addOutput(utf8.encode('ordinary command output\r\n'));
    directory.addOutput(utf8.encode('\x1b]7;file://localhost/tmp\x1b\\'));
    directory.addOutput(utf8.encode('\x1b]7;file://localhost/dev\x07'));
    expect(changes, ['/tmp', '/dev']);
  });

  test('OSC reports support split chunks, UTF-8 and percent-encoded paths', () {
    final directory = TerminalWorkingDirectory();
    addTearDown(directory.dispose);
    final paths = ['/tmp/中文 space', '/tmp/literal%20name', '/tmp/雪'];
    final changes = <String?>[];
    directory.addListener(() => changes.add(directory.value));
    for (final path in paths) {
      final encoded = path.split('/').map(Uri.encodeComponent).join('/');
      final report = utf8.encode('\x1b]7;file://localhost$encoded\x1b\\');
      for (final byte in report) {
        directory.addOutput([byte]);
      }
    }
    expect(changes, paths);
    for (final byte in utf8.encode('\x1b]7;file://localhost/tmp/raw雪\x07')) {
      directory.addOutput([byte]);
    }
    expect(directory.value, '/tmp/raw雪');
  });

  test('structured reports take precedence over prompt fallback', () {
    final directory = TerminalWorkingDirectory();
    addTearDown(directory.dispose);
    final driver = MemoryTerminalDriver(columns: 80, rows: 24);
    driver.write('root@pve:~# ');
    directory.updateSnapshot(driver.snapshot);
    expect(directory.value, '~');
    directory.addOutput(utf8.encode('\x1b]7;file://localhost/home/root\x07'));
    directory.updateSnapshot(driver.snapshot);
    directory.addOutput(utf8.encode('\x1b]7;https://example.com/bad\x07'));
    directory.addOutput(utf8.encode('\x1b]7;file://localhost/tmp%00bad\x07'));
    expect(directory.value, '/home/root');
  });

  test('terminal directory signal ignores output and selection changes', () {
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 24),
    );
    addTearDown(controller.dispose);
    final changes = <String?>[];
    controller.workingDirectoryListenable.addListener(
      () => changes.add(controller.workingDirectory),
    );
    controller.write('root@pve:/tmp# ');
    controller.updateSelectedText('selection');
    controller.write('\r\noutput\r\nroot@pve:/tmp# ');
    controller.write('\r\nroot@pve:/dev# ');
    expect(changes, ['/tmp', '/dev']);
    expect(controller.snapshot.cursor.visible, isTrue);
  });
}
