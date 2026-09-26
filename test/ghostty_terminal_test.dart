import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_ffi.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/terminal/terminal_selection.dart';

void main() {
  test('custom word boundaries update existing sessions on both backends', () {
    final original = terminalWordBoundaries;
    addTearDown(() => terminalWordBoundaries = original);
    for (final backend in [
      TerminalEmulatorBackend.ghostty,
      TerminalEmulatorBackend.alacritty,
    ]) {
      final driver = NativeReplayTerminalDriver.create(
        columns: 40,
        rows: 2,
        config: defaultTerminalConfig.copyWith(emulatorBackend: backend),
      );
      final controller = TerminalController(driver: driver);
      addTearDown(controller.dispose);
      driver.writeBytes(utf8.encode('foo/bar中baz qux'));
      const position = TerminalCellPosition(row: 0, column: 5);
      for (final entry in <String, String>{
        defaultTerminalWordBoundaries: 'foo/bar中baz',
        ' /': 'bar中baz',
        '中': 'foo/bar',
        '': 'foo/bar中baz qux',
      }.entries) {
        terminalWordBoundaries = entry.key;
        expect(
          controller.selectionText(controller.wordSelectionAt(position)!),
          entry.value,
        );
      }
    }
  });

  test('both backends select native words through the controller', () {
    for (final backend in [
      TerminalEmulatorBackend.ghostty,
      TerminalEmulatorBackend.alacritty,
    ]) {
      final driver = NativeReplayTerminalDriver.create(
        columns: 5,
        rows: 2,
        config: defaultTerminalConfig.copyWith(emulatorBackend: backend),
      );
      final controller = TerminalController(driver: driver);
      addTearDown(controller.dispose);
      expect(
        controller.wordSelectionAt(
          const TerminalCellPosition(row: 0, column: 0),
        ),
        isNull,
      );
      driver.writeBytes(utf8.encode('/usr/local/bin'));
      final selected = controller.wordSelectionAt(
        const TerminalCellPosition(row: 0, column: 1),
      );
      expect(selected, const TerminalSelection(start: -5, end: 9));
      expect(controller.selectionText(selected!), '/usr/local/bin');
      driver.scrollLines(1);
      expect(
        controller.wordSelectionAt(
          const TerminalCellPosition(row: 0, column: 0),
        ),
        selected,
      );
    }
  });

  test('controller sends Ghostty paste through its input sink once', () {
    final driver = NativeReplayTerminalDriver.create(
      columns: 20,
      rows: 4,
      config: defaultTerminalConfig.copyWith(
        emulatorBackend: TerminalEmulatorBackend.ghostty,
      ),
    );
    final sent = <String>[];
    final controller = TerminalController(driver: driver, onInput: sent.add);
    addTearDown(controller.dispose);
    driver.writeBytes(utf8.encode('\x1b[?2004h'));
    expect(controller.paste('a\x1b[201~b'), isTrue);
    expect(sent, hasLength(1));
    expect(sent.single, driver.encodePaste('a\x1b[201~b'));
    expect(controller.paste(''), isFalse);
    expect(sent, hasLength(1));
  });

  test('native paste crosses FFI without truncating NUL or sending twice', () {
    for (final backend in [
      TerminalEmulatorBackend.ghostty,
      TerminalEmulatorBackend.alacritty,
    ]) {
      final driver = NativeReplayTerminalDriver.create(
        columns: 20,
        rows: 4,
        config: defaultTerminalConfig.copyWith(emulatorBackend: backend),
      );
      addTearDown(driver.dispose);
      expect(driver.encodePaste('one\r\ntwo'), 'one\rtwo');
      driver.writeBytes(utf8.encode('\x1b[?2004h'));
      final encoded = driver.encodePaste('中文\u0000tail\x1b[201~');
      expect(encoded, startsWith('\x1b[200~中文'));
      expect(encoded, endsWith('\x1b[201~'));
      expect(encoded, contains('tail'));
      if (backend == TerminalEmulatorBackend.ghostty) {
        expect(encoded, isNot(contains('\u0000')));
        expect('\x1b[201~'.allMatches(encoded), hasLength(1));
      }
      expect(driver.plainText.trim(), isEmpty);
    }
  });

  test('Ghostty search preserves counts in scrollback and at the bottom', () {
    final driver = NativeReplayTerminalDriver.create(
      columns: 20,
      rows: 4,
      config: defaultTerminalConfig.copyWith(
        emulatorBackend: TerminalEmulatorBackend.ghostty,
      ),
    );
    addTearDown(driver.dispose);
    driver.writeBytes(
      utf8.encode(List.generate(8, (index) => 'match $index').join('\r\n')),
    );
    driver.scrollLines(driver.snapshot.historyLines);

    var origin = const TerminalCellPosition(row: 0, column: 0);
    var sawScrollback = false;
    var sawBottom = false;
    for (var index = 0; index < 8; index++) {
      final result = driver.search(
        'MATCH',
        direction: TerminalSearchDirection.next,
        origin: origin,
      );
      final snapshot = driver.snapshot;
      expect(result.found, isTrue);
      expect(result.matchIndex, index);
      expect(result.totalMatches, 8);
      expect(driver.selectionText(result.selection!), 'match');
      sawScrollback |= snapshot.displayOffset > 0;
      sawBottom |= snapshot.displayOffset == 0;
      final end =
          result.selection!.end + snapshot.displayOffset * snapshot.columns;
      origin = TerminalCellPosition(
        row: end ~/ snapshot.columns,
        column: end % snapshot.columns,
      );
    }
    expect(sawScrollback, isTrue);
    expect(sawBottom, isTrue);
  });

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
