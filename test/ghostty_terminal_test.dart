import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_ffi.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/terminal/terminal_selection.dart';

void main() {
  test('Ghostty replay terminal crosses FFI with Kitty graphics', () {
    final driver = NativeReplayTerminalDriver.create(
      columns: 8,
      rows: 2,
      config: defaultTerminalConfig.copyWith(
        emulatorBackend: TerminalEmulatorBackend.ghostty,
      ),
    );
    addTearDown(driver.dispose);

    driver.resize(8, 2, cellWidth: 8, cellHeight: 16);
    driver.writeBytes(
      ascii.encode(
        '\x1b_Ga=T,f=100,q=2;'
        'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAA'
        'DUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=='
        '\x1b\\',
      ),
    );

    final snapshot = driver.snapshot;
    expect(snapshot.emulatorBackend, TerminalEmulatorBackend.ghostty);
    expect(snapshot.graphicImages, hasLength(1));
    expect(snapshot.graphicImages.single.rgba, hasLength(4));
    expect(snapshot.graphicPlacements, hasLength(1));
  });

  test('Ghostty text, search, selection, modes and shell blocks cross FFI', () {
    final driver = NativeReplayTerminalDriver.create(
      columns: 20,
      rows: 4,
      config: defaultTerminalConfig.copyWith(
        emulatorBackend: TerminalEmulatorBackend.ghostty,
      ),
    );
    addTearDown(driver.dispose);

    driver.writeBytes(
      Uint8List.fromList(
        utf8.encode(
          '\x1b]2;Ghostty FFI\x07'
          '\x1b[?1h\x1b[?2004h'
          '\x1b]7;file://localhost/tmp/project\x07'
          '\x1b]133;A\x07\$ '
          '\x1b]4545;CommandStarted;ZWNobyBoaQ==\x07'
          'echo hi\r\nhi\r\n'
          '\x1b]4545;CommandExited;0\x07',
        ),
      ),
    );

    final snapshot = driver.snapshot;
    expect(snapshot.emulatorBackend, TerminalEmulatorBackend.ghostty);
    expect(snapshot.title, 'Ghostty FFI');
    expect(snapshot.keyboardMode.applicationCursor, isTrue);
    expect(snapshot.keyboardMode.bracketedPaste, isTrue);
    expect(driver.plainText, contains('echo hi'));

    final search = driver.search(
      'echo hi',
      direction: TerminalSearchDirection.next,
      origin: const TerminalCellPosition(row: 0, column: 0),
    );
    expect(search.found, isTrue);
    expect(
      driver.selectionText(const TerminalSelection(start: 2, end: 9)),
      'echo hi',
    );

    final block = driver.commandBlockAt(
      const TerminalCellPosition(row: 0, column: 0),
    );
    expect(block, isNotNull);
    expect(block!.shellIntegrated, isTrue);
    expect(block.command, 'echo hi');
    expect(block.workingDirectory, '/tmp/project');
    expect(block.exitCode, 0);
  });
}
