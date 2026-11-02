import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/terminal_recording_store.dart';
import 'package:nauterm/data/terminal_retention_policy.dart';

void main() {
  TerminalLogEntry log({
    required String id,
    required DateTime startedAt,
    DateTime? endedAt,
    int captureBytes = 0,
  }) {
    return TerminalLogEntry(
      id: id,
      title: id,
      captureFile: captureBytes == 0 ? '' : '$id.ntrcap',
      captureBytes: captureBytes,
      startedAt: startedAt,
      endedAt: endedAt,
    );
  }

  test(
    'retention expires old logs, trims large captures, and frees capacity',
    () {
      final now = DateTime.utc(2026, 7, 26);
      final plan = planTerminalRetention(
        logs: [
          log(
            id: 'expired',
            startedAt: now.subtract(const Duration(days: 41)),
            endedAt: now.subtract(const Duration(days: 40)),
            captureBytes: 10,
          ),
          log(
            id: 'oldest-retained',
            startedAt: now.subtract(const Duration(days: 3)),
            endedAt: now.subtract(const Duration(days: 2)),
            captureBytes: 80,
          ),
          log(
            id: 'oversized',
            startedAt: now.subtract(const Duration(days: 2)),
            endedAt: now.subtract(const Duration(days: 1)),
            captureBytes: 120,
          ),
          log(id: 'active', startedAt: now, captureBytes: 70),
        ],
        now: now,
        retentionDays: 30,
        maxSessionBytes: 100,
        maxTotalBytes: 200,
        activeLogIds: const {'active'},
      );

      expect(plan.expiredLogIds, {'expired'});
      expect(plan.oversizedLogIds, {'oversized'});
      expect(plan.capacityLogIds, ['oldest-retained']);
      expect(plan.deletedLogIds, {'expired', 'oldest-retained'});
    },
  );

  test('capacity cleanup never selects an active session', () {
    final now = DateTime.utc(2026, 7, 26);
    final plan = planTerminalRetention(
      logs: [log(id: 'active', startedAt: now, captureBytes: 500)],
      now: now,
      retentionDays: 30,
      maxSessionBytes: 100,
      maxTotalBytes: 10,
      activeLogIds: const {'active'},
    );

    expect(plan.deletedLogIds, isEmpty);
    expect(plan.oversizedLogIds, isEmpty);
  });

  test(
    'tail compaction preserves its source until metadata publication',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'nauterm-retention-',
      );
      addTearDown(() => directory.delete(recursive: true));
      const logId = '12345678-1234-1234-1234-123456789abc';
      final source = File('${directory.path}/$logId.ntrcap');
      final originalBytes = List<int>.filled(200, 42);
      await source.writeAsBytes(originalBytes);
      final store = TerminalLogCaptureStore(
        directory,
        tailWriter:
            ({
              required source,
              required destination,
              required logId,
              required maxBytes,
              required expectedSha256,
            }) async {
              expect(source, endsWith('$logId.ntrcap'));
              expect(expectedSha256, 'original-hash');
              expect(maxBytes, 150);
              await File(destination).writeAsBytes([1, 2, 3]);
              return const TerminalCaptureFinalized(
                chunkCount: 1,
                plaintextBytes: 7,
                ciphertextBytes: 150,
                chainHash: 'chain',
                fileSha256: 'new-hash',
              );
            },
      );
      final compacted = await store.retainTail(
        logId: logId,
        captureFile: '$logId.ntrcap',
        maxBytes: 150,
        expectedSha256: 'original-hash',
      );
      expect(compacted.fileName, isNot('$logId.ntrcap'));
      expect(recordingIdForCaptureFile(compacted.fileName), logId);
      expect(compacted.sha256, 'new-hash');
      expect(await source.readAsBytes(), originalBytes);
      expect(
        await File('${directory.path}/${compacted.fileName}').readAsBytes(),
        [1, 2, 3],
      );

      final failing = TerminalLogCaptureStore(
        directory,
        tailWriter:
            ({
              required source,
              required destination,
              required logId,
              required maxBytes,
              required expectedSha256,
            }) async {
              throw const FileSystemException('Synthetic compaction failure');
            },
      );
      await expectLater(
        failing.retainTail(
          logId: logId,
          captureFile: '$logId.ntrcap',
          maxBytes: 150,
          expectedSha256: 'original-hash',
        ),
        throwsA(isA<FileSystemException>()),
      );
      expect(await source.readAsBytes(), originalBytes);
    },
  );
}
