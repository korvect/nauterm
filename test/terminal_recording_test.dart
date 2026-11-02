import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/terminal_recording_store.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/terminal/terminal_recording.dart';
import 'package:nauterm/terminal/terminal_theme.dart';
import 'package:nauterm/terminal/terminal_ffi.dart' show TerminalRecordingLease;

void main() {
  test('terminal recorder uses a uuid v7 recording id', () {
    final recorder = TerminalSessionRecorder(title: 'Local');

    expect(recorder.id, isNot(startsWith('session-')));
    expect(
      recorder.id,
      matches(
        RegExp(
          r'^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$',
        ),
      ),
    );
  });

  test('terminal recorder confirms a connection only once', () {
    var confirmations = 0;
    final recorder = TerminalSessionRecorder(
      title: 'Remote',
      onConnectionEstablished: () => confirmations += 1,
    );

    recorder.recordConnectionEvent(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.connectStart,
        message: 'Connecting',
      ),
    );
    expect(confirmations, 0);

    recorder.recordConnectionEvent(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.connected,
        message: 'Connected',
      ),
    );
    recorder.recordConnectionEvent(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.connected,
        message: 'Reconnected',
      ),
    );

    expect(confirmations, 1);
  });

  test('terminal recorder can disable raw capture completely', () {
    final recorder = TerminalSessionRecorder(
      title: 'Private',
      captureEnabled: false,
    );

    recorder.recordCaptureBytes(Uint8List.fromList(utf8.encode('secret')));

    expect(recorder.snapshot().captureByteCount, 0);
    expect(recorder.snapshot().captureBase64, isEmpty);
  });

  test('terminal recorder only records shell integration commands', () {
    final recorder = TerminalSessionRecorder(title: 'Local');

    recorder.recordInput('echo hello');
    recorder.recordInput('\r');
    recorder.recordShellIntegrationOutput(
      Uint8List.fromList(
        latin1.encode('\x1b]4545;CommandStarted;ZWNobyBoZWxsbw==\x07'),
      ),
    );

    final recording = recorder.snapshot();

    expect(recording.shellHistory.map((entry) => entry.command), [
      'echo hello',
    ]);
    expect(recording.recordedInputEvents.map((event) => event.data), [
      'echo hello',
      '\r',
    ]);
  });

  test('records commands carrying a nested shell instance id', () {
    final recorder = TerminalSessionRecorder(title: 'Local');

    recorder.recordShellIntegrationOutput(
      Uint8List.fromList(
        latin1.encode('\x1b]4545;CommandStarted;child-42;ZWNobyBuZXN0ZWQ=\x07'),
      ),
    );

    expect(recorder.snapshot().shellHistory.single.command, 'echo nested');
  });

  test('does not record input that was never executed by the shell', () {
    final recorder = TerminalSessionRecorder(title: 'Local');

    recorder.recordInput('unfinished');
    recorder.recordInput('\x03');
    recorder.recordInput('echo ok\r');

    expect(recorder.snapshot().shellHistory, isEmpty);
  });

  test('parses a command split across terminal output chunks', () {
    final recorder = TerminalSessionRecorder(title: 'Local');

    recorder.recordShellIntegrationOutput(
      Uint8List.fromList(latin1.encode('\x1b]4545;CommandStarted;Z2l0IGNo')),
    );
    recorder.recordShellIntegrationOutput(
      Uint8List.fromList(latin1.encode('ZWNrb3V0IG1haW4=\x1b\\')),
    );

    expect(recorder.snapshot().shellHistory.map((entry) => entry.command), [
      'git checkout main',
    ]);
  });

  test('retains repeated accepted commands for autocomplete ranking', () {
    final recorder = TerminalSessionRecorder(title: 'Local');
    final event = Uint8List.fromList(
      latin1.encode('\x1b]4545;CommandStarted;bHM=\x07'),
    );

    recorder.recordShellIntegrationOutput(event);
    recorder.recordShellIntegrationOutput(event);

    expect(recorder.snapshot().shellHistory.map((entry) => entry.command), [
      'ls',
      'ls',
    ]);
  });

  test('ignores malformed and non-command shell integration events', () {
    final recorder = TerminalSessionRecorder(title: 'Remote');

    recorder.recordShellIntegrationOutput(
      Uint8List.fromList(
        latin1.encode(
          '\x1b]4545;CommandExited;0\x07'
          '\x1b]4545;CommandStarted;not-base64\x07',
        ),
      ),
    );

    expect(recorder.snapshot().shellHistory, isEmpty);
  });

  test('sensitive terminal input is not recorded as events or history', () {
    final recorder = TerminalSessionRecorder(title: 'Local');
    final sent = <String>[];
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 8),
      onInput: sent.add,
      recorder: recorder,
    );
    addTearDown(controller.dispose);

    controller.sendInput('secret\r', sensitive: true);
    controller.sendInput('echo visible\r');
    recorder.recordShellIntegrationOutput(
      Uint8List.fromList(
        latin1.encode('\x1b]4545;CommandStarted;ZWNobyB2aXNpYmxl\x07'),
      ),
    );

    final recording = recorder.snapshot();
    expect(sent, ['secret\r', 'echo visible\r']);
    expect(recording.recordedInputEvents.map((event) => event.data), [
      'echo visible\r',
    ]);
    expect(recording.shellHistory.map((entry) => entry.command), [
      'echo visible',
    ]);
  });

  test('password prompt input is automatically excluded from history', () {
    final recorder = TerminalSessionRecorder(title: 'Remote');
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final sent = <String>[];
    final controller = TerminalController(
      driver: driver,
      onInput: sent.add,
      recorder: recorder,
    );
    addTearDown(controller.dispose);
    controller.write('[sudo] password for admin:');

    controller.sendInput('secret');
    controller.sendInput('\r');

    expect(sent, ['secret', '\r']);
    expect(recorder.snapshot().recordedInputEvents, isEmpty);
    expect(recorder.snapshot().shellHistory, isEmpty);
  });

  test(
    'echo-off input is never recorded without an explicit sensitive flag',
    () {
      final recorder = TerminalSessionRecorder(title: 'Remote');
      final sent = <String>[];
      final driver = _EchoStateDriver();
      final controller = TerminalController(
        driver: driver,
        onInput: sent.add,
        recorder: recorder,
      );
      addTearDown(controller.dispose);

      controller.sendInput('s');
      controller.sendInput('ecret');
      controller.sendInput('\r');
      driver.inputEchoEnabled = true;
      controller.sendInput('echo visible\r');
      recorder.recordShellIntegrationOutput(
        Uint8List.fromList(
          latin1.encode('\x1b]4545;CommandStarted;ZWNobyB2aXNpYmxl\x07'),
        ),
      );

      final recording = recorder.snapshot();
      expect(sent, ['s', 'ecret', '\r', 'echo visible\r']);
      expect(recording.recordedInputEvents.map((event) => event.data), [
        'echo visible\r',
      ]);
      expect(recording.shellHistory.map((entry) => entry.command), [
        'echo visible',
      ]);
    },
  );

  test('sensitive input discards an already pending recorder line', () {
    final recorder = TerminalSessionRecorder(title: 'Remote');

    recorder.recordInput('accidentally-buffered');
    recorder.recordInput('secret', sensitive: true);
    recorder.recordInput('\r');

    expect(recorder.snapshot().recordedInputEvents.map((event) => event.data), [
      '\r',
    ]);
    expect(recorder.snapshot().shellHistory, isEmpty);
  });

  test('terminal replay frame snapshots round trip through json', () {
    final snapshot = TerminalSnapshot(
      columns: 4,
      rows: 2,
      clipboardText: 'copied text',
      bellCount: 3,
      cells: [
        const TerminalCell(
          text: 'L',
          foreground: terminalDefaultForeground,
          background: terminalDefaultBackground,
          flags: 0,
          hyperlink: 'https://example.com',
        ),
        ...List<TerminalCell>.filled(7, const TerminalCell.empty()),
      ],
      cursor: const TerminalCursor(
        column: 0,
        row: 0,
        visible: true,
        shape: TerminalCursorShape.block,
        color: terminalDefaultCursor,
        blinking: false,
      ),
      keyboardMode: const TerminalKeyboardMode(),
      inputEchoEnabled: false,
    );
    final json = terminalSnapshotToJson(snapshot);
    final decoded = terminalSnapshotFromJson(json);

    expect(decoded.columns, snapshot.columns);
    expect(decoded.rows, snapshot.rows);
    expect(decoded.cells.length, snapshot.cells.length);
    expect(decoded.cells.first.hyperlink, 'https://example.com');
    expect(decoded.clipboardText, 'copied text');
    expect(decoded.bellCount, 3);
    expect(decoded.cursor.column, snapshot.cursor.column);
    expect(decoded.keyboardMode.bracketedPaste, isFalse);
    expect(decoded.inputEchoEnabled, isFalse);
  });

  test('terminal recorder retains only the latest screen snapshot', () {
    final recorder = TerminalSessionRecorder(title: 'Local');

    for (var index = 0; index < 20; index++) {
      recorder.recordSnapshot(
        TerminalSnapshot(
          columns: 4,
          rows: 2,
          title: 'frame-$index',
          cells: List<TerminalCell>.filled(8, const TerminalCell.empty()),
          cursor: const TerminalCursor(
            column: 0,
            row: 0,
            visible: true,
            shape: TerminalCursorShape.block,
            color: terminalDefaultCursor,
            blinking: false,
          ),
          keyboardMode: const TerminalKeyboardMode(),
        ),
      );
    }

    final frames = recorder.snapshot().frames;
    expect(frames, hasLength(1));
    expect(frames.single.snapshot.title, 'frame-19');
  });

  test('terminal log cleanup preserves live leases, checkpoints and unrelated files', () async {
    final directory = await Directory.systemTemp.createTemp(
      'nauterm-recording-',
    );
    addTearDown(() => directory.delete(recursive: true));
    const keptId = '12345678-1234-1234-1234-123456789abc';
    const orphanId = '12345678-1234-1234-1234-123456789def';
    const busyId = '12345678-1234-1234-1234-123456789aaa';
    final store = TerminalLogCaptureStore(
      directory,
      leaseFactory: (_, id) => id == busyId ? null : _MemoryRecordingLease(id),
    );
    final files = {
      'notes.bin': [9],
      '$keptId.ntrcap': [1, 2],
      '$keptId.ntrcap.state': [6],
      '$keptId.ntrcap.state.0123456789abcdef.tmp': [7],
      '$orphanId.ntrcap': [3, 4, 5],
      '$busyId.ntrcap': [8],
      '.nauterm-recording.$busyId.lock': <int>[],
    };
    for (final entry in files.entries) {
      await File('${directory.path}/${entry.key}').writeAsBytes(entry.value);
    }
    await store.cleanupOrphans(
      (id) => id == keptId ? (file: '$keptId.ntrcap', unfinished: true) : null,
    );
    for (final name in files.keys) {
      expect(
        await File('${directory.path}/$name').exists(),
        name != '$orphanId.ntrcap',
      );
    }
    expect(await store.diskUsage(), 6);
    expect(recordingIdForCaptureFile('../$orphanId.ntrcap'), isNull);
    expect(
      recordingIdForCaptureFile('.nauterm-recording.$busyId.lock'),
      isNull,
    );
  });
}

class _EchoStateDriver extends MemoryTerminalDriver {
  _EchoStateDriver() : super(columns: 80, rows: 8);

  bool inputEchoEnabled = false;

  @override
  TerminalSnapshot get snapshot {
    final current = super.snapshot;
    return TerminalSnapshot(
      columns: current.columns,
      rows: current.rows,
      historyLines: current.historyLines,
      displayOffset: current.displayOffset,
      title: current.title,
      cells: current.cells,
      cursor: current.cursor,
      keyboardMode: current.keyboardMode,
      inputEchoEnabled: inputEchoEnabled,
      clipboardText: current.clipboardText,
      bellCount: current.bellCount,
    );
  }
}

class _MemoryRecordingLease implements TerminalRecordingLease {
  _MemoryRecordingLease(this.recordingId);
  @override
  final String recordingId;
  @override
  void close() {}
}
