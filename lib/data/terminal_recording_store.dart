import 'dart:async';
import 'dart:collection';
import 'dart:io';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:flutter/foundation.dart';

import '../terminal/terminal_ffi.dart';

class TerminalLogCaptureStore {
  TerminalLogCaptureStore(this.directory);

  final Directory directory;
  final Map<String, TerminalCaptureFinalized> _finalized = {};

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

  Future<TerminalLogCaptureInfo> retainTail({
    required String logId,
    required String captureFile,
    required int maxBytes,
  }) async {
    final fileName = _safeFileName(captureFile);
    final original = File(_join(directory.path, fileName));
    if (!await original.exists()) {
      return TerminalLogCaptureInfo(fileName: fileName);
    }
    if (await original.length() <= maxBytes) {
      return captureInfo(logId, captureFile: fileName, includeHash: true);
    }

    final retained = ListQueue<Uint8List>();
    await for (final chunk in readDecryptedChunks(
      logId: logId,
      captureFile: fileName,
    )) {
      retained.add(Uint8List.fromList(chunk));
      _trimCaptureChunksToSize(retained, maxBytes);
    }

    final token = DateTime.now().microsecondsSinceEpoch;
    final temporaryName = '$fileName.compact.$token.tmp';
    final backupName = '$fileName.compact.$token.bak';
    final temporary = File(_join(directory.path, temporaryName));
    final backup = File(_join(directory.path, backupName));
    FfiTerminalCaptureWriter? writer;
    try {
      writer = FfiTerminalCaptureWriter.open(
        path: temporary.path,
        recordingId: logId,
      );
      for (final chunk in _coalescedCaptureChunks(retained)) {
        writer.add(chunk);
      }
      final finalized = await writer.closeAsync();
      writer = null;

      await original.rename(backup.path);
      try {
        await temporary.rename(original.path);
      } on Object {
        await backup.rename(original.path);
        rethrow;
      }
      if (await backup.exists()) await backup.delete();
      final state = File(_join(directory.path, '$fileName.state'));
      if (await state.exists()) await state.delete();
      _finalized[logId] = TerminalCaptureFinalized(
        chunkCount: finalized.chunkCount,
        plaintextBytes: finalized.plaintextBytes,
        ciphertextBytes: finalized.ciphertextBytes,
        chainHash: finalized.chainHash,
        fileSha256: finalized.fileSha256,
      );
      return TerminalLogCaptureInfo(
        fileName: fileName,
        bytes: finalized.ciphertextBytes,
        sha256: finalized.fileSha256,
      );
    } finally {
      writer?.abort();
      if (await temporary.exists()) await temporary.delete();
      if (await backup.exists() && !await original.exists()) {
        await backup.rename(original.path);
      }
    }
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

  Future<void> clear() async {
    _finalized.clear();
    if (!await directory.exists()) return;
    await for (final entity in directory.list()) {
      if (entity is File) await entity.delete();
    }
  }

  Future<int> diskUsage() async {
    if (!await directory.exists()) return 0;
    var bytes = 0;
    await for (final entity in directory.list()) {
      if (entity is File) bytes += await entity.length();
    }
    return bytes;
  }

  Future<void> cleanupOrphans(
    Set<String> referencedFiles, {
    Set<String> referencedStateFiles = const {},
  }) async {
    if (!await directory.exists()) return;
    final safeReferences = referencedFiles.map(_safeFileName).toSet();
    final safeStateReferences = referencedStateFiles.map(_safeFileName).toSet();
    await for (final entity in directory.list()) {
      if (entity is! File) continue;
      final name = entity.uri.pathSegments.last;
      final referencedCapture =
          name.endsWith('.ntrcap') && safeReferences.contains(name);
      final referencedState =
          name.endsWith('.ntrcap.state') &&
          safeStateReferences.contains(
            name.substring(0, name.length - '.state'.length),
          );
      if (!referencedCapture && !referencedState) {
        await entity.delete();
      }
    }
  }
}

const int _captureHeaderBytes = 29;
const int _captureRecordOverheadBytes = 29;
const int _captureFooterBytes = 85;
const int _maximumCaptureChunkBytes = 16 * 1024 * 1024;

@visibleForTesting
List<Uint8List> retainTerminalCaptureTailChunks(
  Iterable<Uint8List> chunks,
  int maxBytes,
) {
  final retained = ListQueue<Uint8List>.from(chunks.map(Uint8List.fromList));
  _trimCaptureChunksToSize(retained, maxBytes);
  return retained.toList(growable: false);
}

void _trimCaptureChunksToSize(ListQueue<Uint8List> chunks, int maxBytes) {
  final maximumPlaintext = _maximumRetainedPlaintextBytes(maxBytes);
  var plaintextBytes = chunks.fold<int>(
    0,
    (total, chunk) => total + chunk.length,
  );
  while (plaintextBytes > maximumPlaintext && chunks.isNotEmpty) {
    final first = chunks.removeFirst();
    plaintextBytes -= first.length;
    final needed = maximumPlaintext - plaintextBytes;
    if (needed > 0) {
      final keptLength = needed.clamp(0, first.length);
      final tail = Uint8List.fromList(first.sublist(first.length - keptLength));
      chunks.addFirst(tail);
      plaintextBytes += tail.length;
    }
  }
}

int _maximumRetainedPlaintextBytes(int maxBytes) {
  final payloadBudget = maxBytes - _captureHeaderBytes - _captureFooterBytes;
  if (payloadBudget <= _captureRecordOverheadBytes) return 0;
  var plaintext = payloadBudget - _captureRecordOverheadBytes;
  while (true) {
    final chunkCount =
        (plaintext + _maximumCaptureChunkBytes - 1) ~/
        _maximumCaptureChunkBytes;
    final next = payloadBudget - chunkCount * _captureRecordOverheadBytes;
    if (next >= plaintext) return plaintext;
    plaintext = next.clamp(0, payloadBudget);
  }
}

Iterable<Uint8List> _coalescedCaptureChunks(Iterable<Uint8List> chunks) sync* {
  final builder = BytesBuilder(copy: false);
  for (final chunk in chunks) {
    var offset = 0;
    while (offset < chunk.length) {
      final available = _maximumCaptureChunkBytes - builder.length;
      final length = (chunk.length - offset).clamp(0, available);
      builder.add(Uint8List.sublistView(chunk, offset, offset + length));
      offset += length;
      if (builder.length == _maximumCaptureChunkBytes) {
        yield builder.takeBytes();
      }
    }
  }
  if (builder.isNotEmpty) yield builder.takeBytes();
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
