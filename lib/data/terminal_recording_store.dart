import 'dart:async';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';

import '../terminal/terminal_ffi.dart';

typedef TerminalRecordingLeaseFactory = TerminalRecordingLease? Function(
  String directory,
  String recordingId,
);
typedef TerminalCaptureTailWriter = Future<TerminalCaptureFinalized> Function({
  required String source,
  required String destination,
  required String logId,
  required int maxBytes,
  required String expectedSha256,
});

typedef TerminalCaptureReference = ({String file, bool unfinished});

class TerminalLogCaptureStore {
  TerminalLogCaptureStore(
    this.directory, {
    TerminalRecordingLeaseFactory? leaseFactory,
    TerminalCaptureTailWriter? tailWriter,
  }) : _tailWriter = tailWriter ?? _writeCaptureTail,
       _leaseFactory =
           leaseFactory ??
           ((directory, id) => FfiTerminalRecordingLease.tryAcquire(
             directory: directory,
             recordingId: id,
           )),
       _existingLeaseFactory =
           leaseFactory ??
           ((directory, id) => FfiTerminalRecordingLease.tryAcquire(
             directory: directory,
             recordingId: id,
             existingOnly: true,
           ));

  final TerminalCaptureTailWriter _tailWriter;
  final TerminalRecordingLeaseFactory _leaseFactory;
  final TerminalRecordingLeaseFactory _existingLeaseFactory;

  final Directory directory;
  final Map<String, TerminalCaptureFinalized> _finalized = {};

  TerminalRecordingLease? tryAcquireLease(
    String logId, {
    bool existingOnly = false,
  }) => (existingOnly ? _existingLeaseFactory : _leaseFactory)(
    directory.path,
    logId,
  );

  String captureFileName(String logId) => '${_safeFileName(logId)}.ntrcap';

  Future<void> prepare() async {
    prepareNativeTerminalCaptureDirectory(directory.path);
  }

  Future<TerminalCaptureWriter> openWriter(String logId) async {
    await prepare();
    final fileName = captureFileName(logId);
    await deleteCapture(fileName);
    _finalized.remove(logId);
    return TerminalCaptureWriter._(
      logId: logId,
      fileName: fileName,
      writer: FfiTerminalCaptureWriter.open(
        path: _join(directory.path, fileName),
        recordingId: logId,
      ),
      onFinalized: (value) => _finalized[logId] = value,
    );
  }

  Future<TerminalCaptureFinalized> recover({
    required String logId,
    required String captureFile,
  }) async {
    final fileName = _safeFileName(captureFile);
    final file = File(_join(directory.path, fileName));
    final value = recoverNativeTerminalCapture(
      path: file.path,
      recordingId: logId,
    );
    final finalized = TerminalCaptureFinalized(
      chunkCount: value.chunkCount,
      plaintextBytes: value.plaintextBytes,
      ciphertextBytes: value.ciphertextBytes,
      chainHash: value.chainHash,
      fileSha256: value.fileSha256,
    );
    _finalized[logId] = finalized;
    return finalized;
  }

  Future<bool> captureExists(String captureFile) {
    final fileName = _safeFileName(captureFile);
    return File(_join(directory.path, fileName)).exists();
  }

  Future<TerminalLogCaptureInfo> captureInfo(
    String logId, {
    String? captureFile,
    bool includeHash = false,
    bool useFinalizedHash = false,
  }) async {
    final fileName = captureFile?.trim().isNotEmpty == true
        ? _safeFileName(captureFile!)
        : captureFileName(logId);
    final file = File(_join(directory.path, fileName));
    if (!await file.exists()) {
      return TerminalLogCaptureInfo(fileName: fileName);
    }
    final bytes = await file.length();
    final finalized = _finalized[logId];
    final digest = includeHash && !useFinalizedHash
        ? await sha256.bind(file.openRead()).first
        : null;
    return TerminalLogCaptureInfo(
      fileName: fileName,
      bytes: bytes,
      sha256: useFinalizedHash ? finalized?.fileSha256 : digest?.toString(),
    );
  }

  /// The caller holds the recording lease through the database update. This
  /// creates an immutable revision; delete the source only after that commits.
  Future<TerminalLogCaptureInfo> retainTail({
    required String logId,
    required String captureFile,
    required int maxBytes,
    required String expectedSha256,
  }) async {
    final fileName = _safeFileName(captureFile);
    if (recordingIdForCaptureFile(fileName) != logId) {
      throw const FormatException('Invalid recording capture reference.');
    }
    final original = File(_join(directory.path, fileName));
    if (await original.length() <= maxBytes) {
      final info = await captureInfo(
        logId,
        captureFile: fileName,
        includeHash: true,
      );
      if (info.sha256 != expectedSha256) {
        throw const FileSystemException('Recording changed before compaction.');
      }
      return info;
    }
    final random = Random.secure();
    final token = List.generate(
      16,
      (_) => random.nextInt(256).toRadixString(16).padLeft(2, '0'),
    ).join();
    final revision = '$logId.$token.ntrcap';
    final finalized = await _tailWriter(
      source: original.path,
      destination: _join(directory.path, revision),
      logId: logId,
      maxBytes: maxBytes,
      expectedSha256: expectedSha256,
    );
    return TerminalLogCaptureInfo(
      fileName: revision,
      bytes: finalized.ciphertextBytes,
      sha256: finalized.fileSha256,
    );
  }

  Stream<Uint8List> readDecryptedChunks({
    required String logId,
    required String captureFile,
  }) async* {
    final fileName = _safeFileName(captureFile);
    final file = File(_join(directory.path, fileName));
    if (!await file.exists()) return;
    final reader = FfiTerminalCaptureReader.open(
      path: file.path,
      recordingId: logId,
    );
    try {
      while (true) {
        final chunk = reader.next();
        if (chunk == null) return;
        yield chunk;
        await Future<void>.delayed(Duration.zero);
      }
    } finally {
      reader.close();
    }
  }

  Future<void> deleteCapture(String captureFile) async {
    final fileName = _safeFileName(captureFile);
    final file = File(_join(directory.path, fileName));
    if (await file.exists()) await file.delete();
    final state = File(_join(directory.path, '$fileName.state'));
    if (await state.exists()) await state.delete();
    if (await directory.exists()) {
      await for (final entity in directory.list()) {
        if (entity is! File) continue;
        final name = entity.uri.pathSegments.last;
        if (name.startsWith('$fileName.state.') && name.endsWith('.tmp')) {
          await entity.delete();
        }
      }
    }
  }

  /// Remove managed captures that are not held by a live client. Database
  /// deletion is coordinated by the caller; lease files are never removed.
  Future<void> clear() => cleanupOrphans((_) => null);

  Future<int> diskUsage() async {
    if (!await directory.exists()) return 0;
    var bytes = 0;
    await for (final entity in directory.list()) {
      if (entity is File) bytes += await entity.length();
    }
    return bytes;
  }

  /// Refresh the exact DB reference after acquiring each lease. A directory
  /// or catalog snapshot taken before the lease can miss a new publication.
  Future<void> cleanupOrphans(
    TerminalCaptureReference? Function(String id) lookup,
  ) async {
    if (!await directory.exists()) return;
    await for (final entity in directory.list(followLinks: false)) {
      if (entity is! File) continue;
      final name = entity.uri.pathSegments.last;
      final id = recordingIdForCaptureFile(name);
      if (id == null) continue;
      final initial = lookup(id);
      final lease = tryAcquireLease(
        id,
        existingOnly: initial?.unfinished == true,
      );
      if (lease == null) continue;
      try {
        final reference = lookup(id);
        final captureEnd = name.indexOf('.ntrcap') + '.ntrcap'.length;
        final capture = name.substring(0, captureEnd);
        final tail = name.substring(captureEnd);
        final keep =
            reference?.file == capture &&
            (tail.isEmpty ||
                tail.startsWith('.state') && reference!.unfinished);
        if (!keep && await entity.exists()) await entity.delete();
      } finally {
        lease.close();
      }
    }
  }
}

/// Shared UUID/revision capture names, checkpoint files and compaction files.
/// Unknown files (including stable lease files) are never cleanup candidates.
String? recordingIdForCaptureFile(String name) {
  final match = _managedCaptureName.firstMatch(name);
  return match?.group(1);
}

final _managedCaptureName = RegExp(
  r'^([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})\.(?:[0-9a-fA-F]{32}\.)?ntrcap(?:\.state(?:\.[0-9a-fA-F]{16}\.tmp)?|\.compact\.[0-9]+\.(?:tmp|bak))?$',
);

Future<TerminalCaptureFinalized> _writeCaptureTail({
  required String source,
  required String destination,
  required String logId,
  required int maxBytes,
  required String expectedSha256,
}) async {
  final value = await writeNativeTerminalCaptureTail(
    source: source,
    destination: destination,
    recordingId: logId,
    maxBytes: maxBytes,
    expectedSha256: expectedSha256,
  );
  return TerminalCaptureFinalized(
    chunkCount: value.chunkCount,
    plaintextBytes: value.plaintextBytes,
    ciphertextBytes: value.ciphertextBytes,
    chainHash: value.chainHash,
    fileSha256: value.fileSha256,
  );
}

abstract interface class TerminalCaptureWriteHandle {
  void add(Uint8List bytes);

  Future<TerminalCaptureCheckpoint?> checkpoint();

  Future<void> close();

  void abort();
}

class TerminalCaptureWriter implements TerminalCaptureWriteHandle {
  TerminalCaptureWriter._({
    required this.logId,
    required this.fileName,
    required FfiTerminalCaptureWriter this._writer,
    required this.onFinalized,
  });

  final String logId;
  final String fileName;
  FfiTerminalCaptureWriter? _writer;
  final void Function(TerminalCaptureFinalized) onFinalized;

  @override
  void add(Uint8List bytes) {
    _writer?.add(bytes);
  }

  @override
  Future<TerminalCaptureCheckpoint?> checkpoint() async {
    final writer = _writer;
    if (writer == null) return null;
    final value = await writer.checkpointAsync();
    return TerminalCaptureCheckpoint(
      committedChunkCount: value.committedChunkCount,
      committedPlaintextBytes: value.committedPlaintextBytes,
      committedCiphertextBytes: value.committedCiphertextBytes,
      chainHash: value.chainHash,
    );
  }

  @override
  Future<void> close() async {
    final writer = _writer;
    if (writer == null) return;
    _writer = null;
    final value = await writer.closeAsync();
    onFinalized(
      TerminalCaptureFinalized(
        chunkCount: value.chunkCount,
        plaintextBytes: value.plaintextBytes,
        ciphertextBytes: value.ciphertextBytes,
        chainHash: value.chainHash,
        fileSha256: value.fileSha256,
      ),
    );
  }

  @override
  void abort() {
    final writer = _writer;
    if (writer == null) return;
    _writer = null;
    writer.abort();
  }
}

class TerminalCaptureCheckpoint {
  const TerminalCaptureCheckpoint({
    required this.committedChunkCount,
    required this.committedPlaintextBytes,
    required this.committedCiphertextBytes,
    required this.chainHash,
  });

  final int committedChunkCount;
  final int committedPlaintextBytes;
  final int committedCiphertextBytes;
  final String chainHash;
}

class TerminalCaptureFinalized {
  const TerminalCaptureFinalized({
    required this.chunkCount,
    required this.plaintextBytes,
    required this.ciphertextBytes,
    required this.chainHash,
    required this.fileSha256,
  });

  final int chunkCount;
  final int plaintextBytes;
  final int ciphertextBytes;
  final String chainHash;
  final String fileSha256;
}

class TerminalLogCaptureInfo {
  const TerminalLogCaptureInfo({
    required this.fileName,
    this.bytes = 0,
    this.sha256,
  });

  final String fileName;
  final int bytes;
  final String? sha256;
}

String _safeFileName(String value) {
  final name = value.split(RegExp(r'[/\\]')).last.trim();
  if (name.isEmpty) return 'terminal-log.ntrcap';
  return name.replaceAll(RegExp(r'[^A-Za-z0-9._-]'), '_');
}

String _join(String left, String right) {
  if (left.endsWith(Platform.pathSeparator)) return '$left$right';
  return '$left${Platform.pathSeparator}$right';
}
