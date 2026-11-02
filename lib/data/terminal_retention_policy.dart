import 'package:flutter/foundation.dart';

import '../app/nauterm_log.dart';
import 'nauterm_data_store.dart';
import 'terminal_recording_store.dart';
import '../terminal/terminal_recording_config.dart';

@immutable
class TerminalRetentionPlan {
  const TerminalRetentionPlan({
    this.expiredLogIds = const {},
    this.oversizedLogIds = const {},
    this.capacityLogIds = const [],
  });

  final Set<String> expiredLogIds;
  final Set<String> oversizedLogIds;
  final List<String> capacityLogIds;

  Set<String> get deletedLogIds => {...expiredLogIds, ...capacityLogIds};
}

class TerminalRetentionApplication {
  const TerminalRetentionApplication({
    required this.updatedLogs,
    required this.deletedLogIds,
  });

  final List<TerminalLogEntry> updatedLogs;
  final Set<String> deletedLogIds;
}

Future<TerminalRetentionApplication> applyTerminalRetentionPolicy({
  required NautermDataStore dataStore,
  required TerminalLogCaptureStore captureStore,
  required NautermRecordingConfig config,
  Set<String> activeLogIds = const {},
  void Function(String id)? onLogDeleted,
}) async {
  final logs = <TerminalLogEntry>[];
  var offset = 0;
  while (true) {
    final page = dataStore.listTerminalLogs(limit: 500, offset: offset);
    logs.addAll(page);
    if (page.length < 500) break;
    offset += page.length;
  }
  final plan = planTerminalRetention(
    logs: logs,
    now: DateTime.now().toUtc(),
    retentionDays: config.retentionDays,
    maxSessionBytes: config.maxSessionBytes,
    maxTotalBytes: config.maxTotalBytes,
    activeLogIds: activeLogIds,
  );
  final deletedIds = plan.deletedLogIds;
  final updatedLogs = <TerminalLogEntry>[];

  for (final candidate in logs) {
    if (deletedIds.contains(candidate.id) ||
        !plan.oversizedLogIds.contains(candidate.id) ||
        candidate.captureFile.isEmpty) {
      continue;
    }
    final lease = captureStore.tryAcquireLease(candidate.id);
    if (lease == null) continue;
    try {
      final log = dataStore.getTerminalLog(candidate.id);
      if (log == null ||
          log.endedAt == null ||
          log.captureSha256 == null ||
          log.captureFile != candidate.captureFile ||
          log.captureSha256 != candidate.captureSha256) {
        continue;
      }
      final capture = await captureStore.retainTail(
        logId: log.id,
        captureFile: log.captureFile,
        maxBytes: config.maxSessionBytes,
        expectedSha256: log.captureSha256!,
      );
      final updated = log.copyWith(
        captureFile: capture.fileName,
        captureBytes: capture.bytes,
        captureSha256: capture.sha256,
      );
      dataStore.saveTerminalLog(
        updated,
        events: dataStore.listTerminalLogEvents(log.id),
      );
      updatedLogs.add(updated);
      if (capture.fileName != log.captureFile) {
        try {
          await captureStore.deleteCapture(log.captureFile);
        } on Object catch (error, stackTrace) {
          NautermLog.warning(
            'recording',
            'Unable to remove replaced terminal capture.',
            error: error,
            stackTrace: stackTrace,
          );
        }
      }
    } finally {
      lease.close();
    }
  }
  final removedIds = <String>{};
  for (final id in deletedIds) {
    final lease = captureStore.tryAcquireLease(id);
    if (lease == null) continue;
    try {
      final log = dataStore.getTerminalLog(id);
      if (log == null) continue;
      final expected = logs
          .where((candidate) => candidate.id == id)
          .firstOrNull;
      if (log.endedAt == null ||
          expected == null ||
          log.captureFile != expected.captureFile ||
          log.captureSha256 != expected.captureSha256) {
        continue;
      }
      final captureFile = dataStore.deleteTerminalLog(id);
      if (captureFile != null && captureFile.isNotEmpty) {
        await captureStore.deleteCapture(captureFile);
      }
      removedIds.add(id);
      onLogDeleted?.call(id);
    } finally {
      lease.close();
    }
  }
  return TerminalRetentionApplication(
    updatedLogs: updatedLogs,
    deletedLogIds: removedIds,
  );
}

TerminalRetentionPlan planTerminalRetention({
  required Iterable<TerminalLogEntry> logs,
  required DateTime now,
  required int retentionDays,
  required int maxSessionBytes,
  required int maxTotalBytes,
  Set<String> activeLogIds = const {},
}) {
  final completed = logs
      .where((log) => log.endedAt != null && !activeLogIds.contains(log.id))
      .toList(growable: false);
  final cutoff = now.toUtc().subtract(Duration(days: retentionDays));
  final expired = {
    for (final log in completed)
      if (log.endedAt!.toUtc().isBefore(cutoff)) log.id,
  };
  final retained = completed
      .where((log) => !expired.contains(log.id))
      .toList(growable: false);
  final oversized = {
    for (final log in retained)
      if (log.captureFile.isNotEmpty && log.captureBytes > maxSessionBytes)
        log.id,
  };

  int effectiveBytes(TerminalLogEntry log) {
    if (oversized.contains(log.id)) return maxSessionBytes;
    return log.captureBytes;
  }

  var totalBytes = logs
      .where((log) => !expired.contains(log.id))
      .fold<int>(0, (total, log) => total + effectiveBytes(log));
  final oldestFirst = retained.toList(growable: false)
    ..sort(
      (left, right) => (left.endedAt ?? left.startedAt).compareTo(
        right.endedAt ?? right.startedAt,
      ),
    );
  final capacity = <String>[];
  for (final log in oldestFirst) {
    if (totalBytes <= maxTotalBytes) break;
    final bytes = effectiveBytes(log);
    if (bytes <= 0) continue;
    capacity.add(log.id);
    totalBytes -= bytes;
  }
  return TerminalRetentionPlan(
    expiredLogIds: Set.unmodifiable(expired),
    oversizedLogIds: Set.unmodifiable(oversized),
    capacityLogIds: List.unmodifiable(capacity),
  );
}
