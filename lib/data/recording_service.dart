import 'dart:async';

import 'package:flutter/foundation.dart';

import '../terminal/terminal_recording.dart';
import '../terminal/terminal_ffi.dart' show TerminalRecordingLease;
import 'terminal_recording_store.dart';

typedef TerminalCaptureWriterFactory =
    Future<TerminalCaptureWriteHandle> Function(
      String recordingId,
      TerminalLogCaptureStore captureStore,
    );

@immutable
class TerminalLogContext {
  const TerminalLogContext({
    this.hostId,
    this.host,
    this.port,
    this.username,
    this.shellPath,
    this.workDir,
    this.cwdResolver,
  });

  final int? hostId;
  final String? host;
  final int? port;
  final String? username;
  final String? shellPath;
  final String? workDir;
  final String? Function()? cwdResolver;
}

/// Owns recording classification, capture writers, and serialized saves.
///
/// Database-to-view-model mapping remains in the workspace. Capture writer
/// ownership and shutdown ordering live here so UI code cannot mutate sink
/// state directly.
class RecordingService {
  RecordingService({TerminalCaptureWriterFactory? captureWriterFactory})
    : _captureWriterFactory =
          captureWriterFactory ??
          ((recordingId, captureStore) => captureStore.openWriter(recordingId));

  static const Duration _saveDebounce = Duration(seconds: 1);

  final TerminalCaptureWriterFactory _captureWriterFactory;

  final List<TerminalSessionRecorder> recorders = [];
  final Map<String, _TerminalCaptureSink> _captureSinks = {};
  final Set<String> _ignoredIds = {};
  final Set<String> _finalizedIds = {};
  final Set<String> _pendingIds = {};
  final Set<String> _captureReferences = {};
  final Map<String, TerminalLogContext> _contexts = {};
  final Map<String, TerminalRecordingLease> _leases = {};
  Timer? _saveTimer;
  Future<void> _saveQueue = Future<void>.value();

  void register(
    TerminalSessionRecorder recorder, {
    required TerminalLogContext context,
    required bool pending,
    TerminalLogCaptureStore? captureStore,
  }) {
    if (captureStore != null) {
      final lease = captureStore.tryAcquireLease(recorder.id);
      if (lease == null) throw StateError('This recording is already in use.');
      _leases[recorder.id] = lease;
    }
    recorders.add(recorder);
    _contexts[recorder.id] = context;
    if (pending) _pendingIds.add(recorder.id);
  }

  bool confirm(String recordingId) => _pendingIds.remove(recordingId);

  bool isPending(String recordingId) => _pendingIds.contains(recordingId);

  bool isIgnored(String recordingId) => _ignoredIds.contains(recordingId);

  void ignore(String recordingId) => _ignoredIds.add(recordingId);

  void removePending(String recordingId) => _pendingIds.remove(recordingId);

  bool hasCaptureReference(String recordingId) =>
      _captureReferences.contains(recordingId);

  void markCaptureReferenced(String recordingId) =>
      _captureReferences.add(recordingId);

  void removeCaptureReference(String recordingId) =>
      _captureReferences.remove(recordingId);

  TerminalLogContext contextFor(String recordingId) =>
      _contexts[recordingId] ?? const TerminalLogContext();

  void writeCapture({
    required String recordingId,
    required Uint8List bytes,
    required TerminalLogCaptureStore captureStore,
    required ValueChanged<Object> onError,
  }) {
    if (bytes.isEmpty || isIgnored(recordingId)) return;
    final sink = _captureSinks.putIfAbsent(
      recordingId,
      () => _TerminalCaptureSink(
        _captureWriterFactory(recordingId, captureStore),
        onError: onError,
      ),
    );
    sink.add(bytes);
  }

  Future<void> checkpointCaptures({
    required Set<String> recordingIds,
    required Set<String> activeRecordingIds,
  }) async {
    final sinks = _captureSinks.entries
        .where((entry) => recordingIds.contains(entry.key))
        .toList(growable: false);
    await Future.wait([
      for (final entry in sinks)
        if (activeRecordingIds.contains(entry.key))
          entry.value.checkpoint()
        else
          entry.value.close(),
    ]);
    for (final entry in sinks) {
      if (!activeRecordingIds.contains(entry.key) &&
          identical(_captureSinks[entry.key], entry.value)) {
        _captureSinks.remove(entry.key);
      }
    }
  }

  Future<void> closeCapture(String recordingId) async {
    await _captureSinks.remove(recordingId)?.close();
  }

  Future<void> closeAllCaptures() async {
    final sinks = _captureSinks.values.toList(growable: false);
    _captureSinks.clear();
    await Future.wait([for (final sink in sinks) sink.close()]);
  }

  void removeRecording(String recordingId) {
    releaseLease(recordingId);
    _finalizedIds.remove(recordingId);
    recorders.removeWhere((entry) => entry.id == recordingId);
    _contexts.remove(recordingId);
    _pendingIds.remove(recordingId);
    _captureReferences.remove(recordingId);
  }

  void stopAllRecordings() {
    _ignoredIds.addAll(recorders.map((entry) => entry.id));
    _pendingIds.clear();
    _captureReferences.clear();
  }

  void clearRecordings() {
    releaseAllLeases();
    _finalizedIds.clear();
    recorders.clear();
    _contexts.clear();
  }

  void ignoreAllRecorders() {
    _ignoredIds.addAll(recorders.map((entry) => entry.id));
  }

  bool isFinalized(String id) => _finalizedIds.contains(id);

  /// The final metadata is committed. Keep in-memory replay frames, but later
  /// save batches must not recreate a log another client deleted/compacted.
  void markFinalized(String id) {
    _finalizedIds.add(id);
    releaseLease(id);
  }

  TerminalRecordingLease? leaseFor(String id) => _leases[id];

  void releaseLease(String id) => _leases.remove(id)?.close();

  void releaseAllLeases() {
    for (final lease in _leases.values) {
      lease.close();
    }
    _leases.clear();
  }

  void scheduleSave(Future<void> Function() save) {
    _saveTimer ??= Timer(_saveDebounce, () => unawaited(save()));
  }

  Future<void> enqueueSave(Future<void> Function() save) {
    final queued = _saveQueue.then((_) => save());
    // Report this operation's error to its caller without poisoning later
    // saves/deletions queued by the same recording service.
    _saveQueue = queued.then<void>(
      (_) {},
      onError: (Object _, StackTrace _) {},
    );
    return queued;
  }

  void cancelScheduledSave() {
    _saveTimer?.cancel();
    _saveTimer = null;
  }

  void dispose() {
    cancelScheduledSave();
    for (final sink in _captureSinks.values) {
      sink.abort();
    }
    _captureSinks.clear();
    releaseAllLeases();
    _pendingIds.clear();
    _captureReferences.clear();
    _contexts.clear();
    _finalizedIds.clear();
    recorders.clear();
  }
}

class _TerminalCaptureSink {
  _TerminalCaptureSink(this._sinkFuture, {required this.onError});

  final Future<TerminalCaptureWriteHandle> _sinkFuture;
  final ValueChanged<Object> onError;
  final List<Uint8List> _pending = [];
  TerminalCaptureWriteHandle? _sink;
  bool _opening = false;
  bool _failed = false;
  bool _closing = false;
  Future<void>? _maintenance;
  Future<void>? _closeFuture;

  void add(Uint8List bytes) {
    if (_failed || _closing || bytes.isEmpty) return;
    final sink = _sink;
    if (sink != null && _maintenance == null) {
      try {
        sink.add(bytes);
      } on Object catch (error) {
        _fail(error);
      }
      return;
    }
    _pending.add(Uint8List.fromList(bytes));
    if (_opening) return;
    _opening = true;
    unawaited(
      (() async {
        try {
          final sink = await _sinkFuture;
          if (_failed) {
            sink.abort();
            return;
          }
          _sink = sink;
          if (_maintenance == null) {
            _drainPending(sink);
          }
        } on Object catch (error) {
          _fail(error);
        }
      })(),
    );
  }

  Future<void> checkpoint() async {
    if (_failed || _closing) return;
    await _scheduleMaintenance((sink) => sink.checkpoint());
  }

  Future<void> close() {
    if (_failed) return Future<void>.value();
    final existing = _closeFuture;
    if (existing != null) return existing;
    _closing = true;
    return _closeFuture = _scheduleMaintenance(
      (sink) => sink.close(),
      finalize: true,
    );
  }

  void abort() {
    _failed = true;
    _closing = true;
    _pending.clear();
    _abortSink(_sink);
    _sink = null;
  }

  Future<void> _scheduleMaintenance(
    Future<void> Function(TerminalCaptureWriteHandle sink) operation, {
    bool finalize = false,
  }) {
    final previous = _maintenance ?? Future<void>.value();
    final completer = Completer<void>();
    final current = completer.future;
    _maintenance = current;
    unawaited(
      (() async {
        try {
          await previous;
          if (_failed) return;
          final sink = await _readySink();
          _drainPending(sink);
          await operation(sink);
          if (!finalize && !_failed) {
            _drainPending(sink);
          }
        } on Object catch (error) {
          _fail(error);
        } finally {
          if (identical(_maintenance, current)) {
            _maintenance = null;
          }
          completer.complete();
        }
      })(),
    );
    return current;
  }

  Future<TerminalCaptureWriteHandle> _readySink() async {
    final sink = _sink ?? await _sinkFuture;
    if (_failed) {
      sink.abort();
      throw StateError('Terminal capture sink is closed.');
    }
    _sink = sink;
    return sink;
  }

  void _drainPending(TerminalCaptureWriteHandle sink) {
    if (_pending.isEmpty) return;
    final chunks = _pending.toList(growable: false);
    _pending.clear();
    for (final chunk in chunks) {
      sink.add(chunk);
    }
  }

  void _abortSink(TerminalCaptureWriteHandle? sink) {
    if (sink == null) return;
    final maintenance = _maintenance;
    if (maintenance == null) {
      try {
        sink.abort();
      } on Object {
        // Abort is best-effort during disposal or after an I/O failure.
      }
      return;
    }
    unawaited(
      maintenance.whenComplete(() {
        try {
          sink.abort();
        } on Object {
          // The original write error remains the actionable failure.
        }
      }),
    );
  }

  void _fail(Object error) {
    if (_failed) return;
    _failed = true;
    _pending.clear();
    final sink = _sink;
    _sink = null;
    _abortSink(sink);
    onError(error);
  }
}
