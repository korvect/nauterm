import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/recording_service.dart';
import 'package:nauterm/data/terminal_recording_store.dart';
import 'package:nauterm/terminal/terminal_recording.dart';

void main() {
  test(
    'bulk log deletion keeps active recordings and rechecks leases',
    () async {
      final directory = Directory.systemTemp.createTempSync(
        'nauterm-log-delete-',
      );
      addTearDown(() => directory.deleteSync(recursive: true));
      final dataStore = NautermDataStore.openPath(
        '${directory.path}/logs.sqlite',
      );
      addTearDown(dataStore.dispose);
      final captureStore = TerminalLogCaptureStore(
        Directory('${directory.path}/terminal-logs'),
      );
      await captureStore.prepare();
      final service = RecordingService();
      addTearDown(service.dispose);
      final recorder = TerminalSessionRecorder(title: 'Active in this window');
      service.register(
        recorder,
        context: const TerminalLogContext(),
        pending: false,
        captureStore: captureStore,
      );
      const externalId = '01979f62-8548-7000-8000-000000000001';
      const finishedId = '01979f62-8548-7000-8000-000000000002';
      final externalLease = captureStore.tryAcquireLease(externalId)!;
      addTearDown(externalLease.close);
      final capture = captureStore.captureFileName(finishedId);
      final file = File('${captureStore.directory.path}/$capture')
        ..writeAsStringSync('recorded bytes');
      final started = DateTime.utc(2026, 1, 1);
      for (final log in [
        TerminalLogEntry(
          id: recorder.id,
          title: 'Active here',
          captureFile: '',
          startedAt: started,
        ),
        TerminalLogEntry(
          id: externalId,
          title: 'Active elsewhere',
          captureFile: '',
          startedAt: started,
        ),
        TerminalLogEntry(
          id: finishedId,
          title: 'Finished',
          captureFile: capture,
          startedAt: started,
          endedAt: started,
        ),
      ]) {
        dataStore.saveTerminalLog(log);
      }

      final review = service.prepareLogDeletion(dataStore, captureStore);
      expect(review.hasActive, isTrue);
      expect(review.finished.map((entry) => entry.id), [finishedId]);

      final newlyHeld = captureStore.tryAcquireLease(finishedId)!;
      try {
        await expectLater(
          service.deleteFinishedLogs(
            dataStore,
            captureStore,
            review.finished,
            isCurrent: () => true,
          ),
          throwsA(isA<NautermDataException>()),
        );
        expect(dataStore.listTerminalLogs(), hasLength(3));
        expect(file.existsSync(), isTrue);
      } finally {
        newlyHeld.close();
      }

      final deletion = await service.deleteFinishedLogs(
        dataStore,
        captureStore,
        review.finished,
        isCurrent: () => true,
      );
      expect(deletion.deleted, {finishedId});
      expect(deletion.cleanupError, isNull);
      expect(
        dataStore.listTerminalLogs().map((entry) => entry.id),
        unorderedEquals([recorder.id, externalId]),
      );
      expect(service.leaseFor(recorder.id), isNotNull);
      expect(service.isIgnored(recorder.id), isFalse);
      expect(file.existsSync(), isFalse);
    },
  );

  test('recording service owns pending and ignored session state', () {
    final service = RecordingService();
    addTearDown(service.dispose);
    final recorder = TerminalSessionRecorder(title: 'Test');

    service.register(
      recorder,
      pending: true,
      context: const TerminalLogContext(host: 'example.com', port: 22),
    );

    expect(service.recorders, [recorder]);
    expect(service.isPending(recorder.id), isTrue);
    expect(service.contextFor(recorder.id).host, 'example.com');
    expect(service.confirm(recorder.id), isTrue);
    expect(service.confirm(recorder.id), isFalse);

    service.markCaptureReferenced(recorder.id);
    expect(service.hasCaptureReference(recorder.id), isTrue);
    service.ignore(recorder.id);
    expect(service.isIgnored(recorder.id), isTrue);

    service.removeRecording(recorder.id);
    expect(service.recorders, isEmpty);
    expect(service.hasCaptureReference(recorder.id), isFalse);
  });

  test('recording saves are serialized in submission order', () async {
    final service = RecordingService();
    addTearDown(service.dispose);
    final releaseFirst = Completer<void>();
    final order = <String>[];

    final first = service.enqueueSave(() async {
      order.add('first-start');
      await releaseFirst.future;
      order.add('first-end');
    });
    final second = service.enqueueSave(() async {
      order.add('second');
    });

    await Future<void>.delayed(Duration.zero);
    expect(order, ['first-start']);
    releaseFirst.complete();
    await Future.wait([first, second]);
    expect(order, ['first-start', 'first-end', 'second']);
  });

  test(
    'capture writes queue behind checkpoint and close maintenance',
    () async {
      final writer = _ControlledCaptureWriter();
      final service = RecordingService(
        captureWriterFactory: (_, _) async => writer,
      );
      addTearDown(service.dispose);
      final store = TerminalLogCaptureStore(Directory.systemTemp);
      final errors = <Object>[];

      service.writeCapture(
        recordingId: 'capture-1',
        bytes: Uint8List.fromList([1]),
        captureStore: store,
        onError: errors.add,
      );
      await writer.firstAdd.future;

      final checkpoint = service.checkpointCaptures(
        recordingIds: {'capture-1'},
        activeRecordingIds: {'capture-1'},
      );
      await writer.checkpointStarted.future;
      service.writeCapture(
        recordingId: 'capture-1',
        bytes: Uint8List.fromList([2]),
        captureStore: store,
        onError: errors.add,
      );
      expect(writer.bytes, [1]);

      writer.releaseCheckpoint.complete();
      await checkpoint;
      expect(writer.bytes, [1, 2]);

      final close = service.closeCapture('capture-1');
      await writer.closeStarted.future;
      var closeCompleted = false;
      close.whenComplete(() => closeCompleted = true);
      await Future<void>.delayed(Duration.zero);
      expect(closeCompleted, isFalse);
      writer.releaseClose.complete();
      await close;

      expect(closeCompleted, isTrue);
      expect(errors, isEmpty);
    },
  );
}

class _ControlledCaptureWriter implements TerminalCaptureWriteHandle {
  final List<int> bytes = [];
  final Completer<void> firstAdd = Completer<void>();
  final Completer<void> checkpointStarted = Completer<void>();
  final Completer<void> releaseCheckpoint = Completer<void>();
  final Completer<void> closeStarted = Completer<void>();
  final Completer<void> releaseClose = Completer<void>();

  @override
  void add(Uint8List value) {
    bytes.addAll(value);
    if (!firstAdd.isCompleted) firstAdd.complete();
  }

  @override
  void abort() {}

  @override
  Future<TerminalCaptureCheckpoint?> checkpoint() async {
    checkpointStarted.complete();
    await releaseCheckpoint.future;
    return null;
  }

  @override
  Future<void> close() async {
    closeStarted.complete();
    await releaseClose.future;
  }
}
