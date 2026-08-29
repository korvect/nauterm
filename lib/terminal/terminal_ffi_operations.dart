part of 'terminal_ffi.dart';

void prepareNativeTerminalCaptureDirectory(String path) {
  final bindings = _TerminalBindings.open();
  final nativePath = path.toNativeUtf8();
  try {
    if (!bindings.capturePrepareDirectory(nativePath)) {
      throw FileSystemException(
        'Unable to protect the terminal capture directory.',
        path,
      );
    }
  } finally {
    malloc.free(nativePath);
  }
}

bool verifyNativeTerminalCaptureComplete({
  required String path,
  required String recordingId,
}) {
  final bindings = _TerminalBindings.open();
  final nativePath = path.toNativeUtf8();
  final nativeId = recordingId.toNativeUtf8();
  try {
    return bindings.captureVerifyComplete(nativePath, nativeId);
  } finally {
    malloc.free(nativePath);
    malloc.free(nativeId);
  }
}

FfiTerminalCaptureFinalized recoverNativeTerminalCapture({
  required String path,
  required String recordingId,
}) {
  final bindings = _TerminalBindings.open();
  final nativePath = path.toNativeUtf8();
  final nativeId = recordingId.toNativeUtf8();
  try {
    final pointer = bindings.captureRecover(nativePath, nativeId);
    if (pointer == nullptr) {
      throw const FormatException(
        'Encrypted terminal capture cannot be recovered.',
      );
    }
    try {
      return FfiTerminalCaptureFinalized.fromJson(
        jsonDecode(pointer.toDartString()) as Map<String, dynamic>,
      );
    } finally {
      bindings.freeString(pointer);
    }
  } finally {
    malloc.free(nativePath);
    malloc.free(nativeId);
  }
}

class FfiTerminalCaptureWriter {
  FfiTerminalCaptureWriter._(this._bindings, this._handle);

  final _TerminalBindings _bindings;
  int _handle;

  static FfiTerminalCaptureWriter open({
    required String path,
    required String recordingId,
  }) {
    final bindings = _TerminalBindings.open();
    final nativePath = path.toNativeUtf8();
    final nativeId = recordingId.toNativeUtf8();
    try {
      final handle = bindings.captureWriterOpen(nativePath, nativeId);
      if (handle == 0) {
        throw StateError('Unable to open encrypted terminal capture.');
      }
      return FfiTerminalCaptureWriter._(bindings, handle);
    } finally {
      malloc.free(nativePath);
      malloc.free(nativeId);
    }
  }

  void add(Uint8List bytes) {
    if (_handle == 0 || bytes.isEmpty) return;
    final pointer = malloc<Uint8>(bytes.length);
    try {
      pointer.asTypedList(bytes.length).setAll(0, bytes);
      if (!_bindings.captureWriterAppend(_handle, pointer, bytes.length)) {
        throw FileSystemException(
          'Unable to write encrypted terminal capture.',
        );
      }
    } finally {
      malloc.free(pointer);
    }
  }

  Future<FfiTerminalCaptureCheckpoint> checkpointAsync() {
    final handle = _handle;
    if (handle == 0) {
      throw StateError('Terminal capture writer is closed.');
    }
    return Isolate.run(() => _checkpointTerminalCaptureWriter(handle));
  }

  Future<FfiTerminalCaptureFinalized> closeAsync() async {
    final handle = _handle;
    if (handle == 0) {
      throw StateError('Terminal capture writer is already closed.');
    }
    _handle = 0;
    try {
      return await Isolate.run(() => _finalizeTerminalCaptureWriter(handle));
    } on Object {
      // If the worker isolate failed before taking ownership, do not leave an
      // unreachable writer in the native handle map. This is harmless when
      // finalize already removed it.
      _bindings.captureWriterAbort(handle);
      rethrow;
    }
  }

  void abort() {
    final handle = _handle;
    if (handle == 0) return;
    _handle = 0;
    if (!_bindings.captureWriterAbort(handle)) {
      throw StateError('Unable to abort terminal capture writer.');
    }
  }
}

FfiTerminalCaptureCheckpoint _checkpointTerminalCaptureWriter(int handle) {
  final bindings = _TerminalBindings.open();
  final pointer = bindings.captureWriterCheckpoint(handle);
  if (pointer == nullptr) {
    throw FileSystemException(
      'Unable to checkpoint encrypted terminal capture.',
    );
  }
  try {
    return FfiTerminalCaptureCheckpoint.fromJson(
      jsonDecode(pointer.toDartString()) as Map<String, dynamic>,
    );
  } finally {
    bindings.freeString(pointer);
  }
}

FfiTerminalCaptureFinalized _finalizeTerminalCaptureWriter(int handle) {
  final bindings = _TerminalBindings.open();
  final pointer = bindings.captureWriterFinalize(handle);
  if (pointer == nullptr) {
    throw FileSystemException('Unable to finalize encrypted terminal capture.');
  }
  try {
    return FfiTerminalCaptureFinalized.fromJson(
      jsonDecode(pointer.toDartString()) as Map<String, dynamic>,
    );
  } finally {
    bindings.freeString(pointer);
  }
}

@immutable
class FfiTerminalCaptureCheckpoint {
  const FfiTerminalCaptureCheckpoint({
    required this.committedChunkCount,
    required this.committedPlaintextBytes,
    required this.committedCiphertextBytes,
    required this.chainHash,
  });

  factory FfiTerminalCaptureCheckpoint.fromJson(Map<String, dynamic> json) {
    return FfiTerminalCaptureCheckpoint(
      committedChunkCount: json['committed_chunk_count'] as int,
      committedPlaintextBytes: json['committed_plaintext_bytes'] as int,
      committedCiphertextBytes: json['committed_ciphertext_bytes'] as int,
      chainHash: json['chain_hash'] as String,
    );
  }

  final int committedChunkCount;
  final int committedPlaintextBytes;
  final int committedCiphertextBytes;
  final String chainHash;
}

@immutable
class FfiTerminalCaptureFinalized {
  const FfiTerminalCaptureFinalized({
    required this.chunkCount,
    required this.plaintextBytes,
    required this.ciphertextBytes,
    required this.chainHash,
    required this.fileSha256,
  });

  factory FfiTerminalCaptureFinalized.fromJson(Map<String, dynamic> json) {
    return FfiTerminalCaptureFinalized(
      chunkCount: json['chunk_count'] as int,
      plaintextBytes: json['plaintext_bytes'] as int,
      ciphertextBytes: json['ciphertext_bytes'] as int,
      chainHash: json['chain_hash'] as String,
      fileSha256: json['file_sha256'] as String,
    );
  }

  final int chunkCount;
  final int plaintextBytes;
  final int ciphertextBytes;
  final String chainHash;
  final String fileSha256;
}

class FfiTerminalCaptureReader {
  FfiTerminalCaptureReader._(this._bindings, this._handle);

  final _TerminalBindings _bindings;
  int _handle;

  static FfiTerminalCaptureReader open({
    required String path,
    required String recordingId,
  }) {
    final bindings = _TerminalBindings.open();
    final nativePath = path.toNativeUtf8();
    final nativeId = recordingId.toNativeUtf8();
    try {
      final handle = bindings.captureReaderOpen(nativePath, nativeId);
      if (handle == 0) {
        throw const FormatException(
          'Unable to open encrypted terminal capture.',
        );
      }
      return FfiTerminalCaptureReader._(bindings, handle);
    } finally {
      malloc.free(nativePath);
      malloc.free(nativeId);
    }
  }

  Uint8List? next() {
    if (_handle == 0) return null;
    final pointer = _bindings.captureReaderNext(_handle);
    if (pointer == nullptr) {
      throw const FormatException('Unable to read encrypted terminal capture.');
    }
    try {
      final value = jsonDecode(pointer.toDartString()) as Map<String, dynamic>;
      final error = value['error'] as String?;
      if (error != null) throw FormatException(error);
      if (value['done'] == true) return null;
      return base64Decode(value['data'] as String);
    } finally {
      _bindings.freeString(pointer);
    }
  }

  void close() {
    final handle = _handle;
    if (handle == 0) return;
    _handle = 0;
    _bindings.captureReaderClose(handle);
  }
}

class FfiTerminalLoadException implements Exception {
  const FfiTerminalLoadException(this.message);

  final String message;

  @override
  String toString() => message;
}

@immutable
class FfiPortForwardStatus {
  const FfiPortForwardStatus({
    required this.id,
    required this.state,
    this.error,
    this.boundPort,
    this.activeConnections = 0,
  });

  factory FfiPortForwardStatus.fromJson(Map<String, Object?> json) {
    return FfiPortForwardStatus(
      id: json['id'] as int? ?? 0,
      state: json['state'] as String? ?? 'unknown',
      error: json['error'] as String?,
      boundPort: json['bound_port'] as int?,
      activeConnections: json['active_connections'] as int? ?? 0,
    );
  }

  final int id;
  final String state;
  final String? error;
  final int? boundPort;
  final int activeConnections;

  bool get isRunning => state == 'running' || state == 'starting';
  bool get isError => state == 'error';
}

class FfiPortForwarding {
  const FfiPortForwarding._();

  static FfiPortForwardStatus start({
    required int id,
    required String type,
    required String sshHost,
    required int sshPort,
    required String username,
    required String knownHostsPath,
    required String bindAddress,
    required int bindPort,
    String? password,
    String? privateKey,
    String? certificate,
    String? passphrase,
    TerminalProxyConfig? proxy,
    String? destinationHost,
    int destinationPort = 0,
  }) {
    final bindings = _TerminalBindings.open();
    final nativeType = type.toNativeUtf8();
    final nativeSshHost = sshHost.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativeBindAddress = bindAddress.toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativeCertificate = certificate == null
        ? nullptr
        : certificate.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    final nativeDestinationHost = destinationHost == null
        ? nullptr
        : destinationHost.toNativeUtf8();
    final nativeProxy = _proxyConfigToNative(proxy);
    Pointer<Utf8> statusPointer = nullptr;
    try {
      statusPointer = bindings.startPortForward(
        id,
        nativeType,
        nativeSshHost,
        sshPort,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativeCertificate,
        nativePassphrase,
        nativeKnownHostsPath,
        nativeBindAddress,
        bindPort,
        nativeDestinationHost,
        destinationPort,
        nativeProxy,
      );
      return _portForwardStatusFromPointer(statusPointer, id);
    } finally {
      malloc.free(nativeType);
      malloc.free(nativeSshHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      malloc.free(nativeBindAddress);
      if (nativePassword != nullptr) {
        malloc.free(nativePassword);
      }
      if (nativePrivateKey != nullptr) {
        malloc.free(nativePrivateKey);
      }
      if (nativeCertificate != nullptr) {
        malloc.free(nativeCertificate);
      }
      if (nativePassphrase != nullptr) {
        malloc.free(nativePassphrase);
      }
      if (nativeDestinationHost != nullptr) {
        malloc.free(nativeDestinationHost);
      }
      if (nativeProxy != nullptr) {
        malloc.free(nativeProxy);
      }
      if (statusPointer != nullptr) {
        bindings.freeString(statusPointer);
      }
    }
  }

  static bool stop(int id) {
    return _TerminalBindings.open().stopPortForward(id);
  }

  static int stopAll() {
    return _TerminalBindings.open().stopAllPortForwards();
  }

  static FfiPortForwardStatus status(int id) {
    final bindings = _TerminalBindings.open();
    final statusPointer = bindings.portForwardStatus(id);
    try {
      return _portForwardStatusFromPointer(statusPointer, id);
    } finally {
      if (statusPointer != nullptr) {
        bindings.freeString(statusPointer);
      }
    }
  }

  static FfiPortForwardStatus _portForwardStatusFromPointer(
    Pointer<Utf8> pointer,
    int id,
  ) {
    if (pointer == nullptr) {
      return FfiPortForwardStatus(
        id: id,
        state: 'error',
        error: 'Port forwarding returned no status.',
      );
    }
    try {
      final decoded = jsonDecode(pointer.toDartString());
      if (decoded is Map<String, Object?>) {
        return FfiPortForwardStatus.fromJson(decoded);
      }
      if (decoded is Map) {
        return FfiPortForwardStatus.fromJson(decoded.cast<String, Object?>());
      }
    } on Object catch (error) {
      return FfiPortForwardStatus(
        id: id,
        state: 'error',
        error: 'Invalid port forwarding status: $error',
      );
    }
    return FfiPortForwardStatus(
      id: id,
      state: 'error',
      error: 'Invalid port forwarding status.',
    );
  }
}

@immutable
class FfiSshDirectoryListingResult {
  const FfiSshDirectoryListingResult({
    required this.entries,
    this.resolvedDirectory,
    this.error,
  });

  final List<String> entries;
  final String? resolvedDirectory;
  final String? error;

  bool get isError => error != null && error!.trim().isNotEmpty;
}

class FfiSshDirectoryListing {
  const FfiSshDirectoryListing._();

  static FfiSshDirectoryListingResult listDirectories({
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    required String directory,
    String? password,
    String? privateKey,
    String? certificate,
    String? passphrase,
  }) {
    final bindings = _TerminalBindings.open();
    final nativeHost = host.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativeDirectory = directory.toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativeCertificate = certificate == null
        ? nullptr
        : certificate.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    Pointer<Utf8> resultPointer = nullptr;
    try {
      resultPointer = bindings.sshListDirectories(
        nativeHost,
        port,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativeCertificate,
        nativePassphrase,
        nativeKnownHostsPath,
        nativeDirectory,
      );
      if (resultPointer == nullptr) {
        return const FfiSshDirectoryListingResult(
          entries: [],
          error: 'SSH completion returned no result.',
        );
      }
      final decoded = jsonDecode(resultPointer.toDartString());
      if (decoded is! Map) {
        return const FfiSshDirectoryListingResult(
          entries: [],
          error: 'SSH completion returned an invalid result.',
        );
      }
      final error = decoded['error'];
      if (error is String && error.trim().isNotEmpty) {
        return FfiSshDirectoryListingResult(entries: const [], error: error);
      }
      final entries = decoded['entries'];
      if (entries is! List) {
        return const FfiSshDirectoryListingResult(
          entries: [],
          error: 'SSH completion returned no entries.',
        );
      }
      return FfiSshDirectoryListingResult(
        entries: [
          for (final entry in entries)
            if (entry is String && entry.trim().isNotEmpty) entry,
        ],
        resolvedDirectory: decoded['directory'] as String?,
      );
    } on Object catch (error) {
      return FfiSshDirectoryListingResult(
        entries: const [],
        error: 'SSH completion failed: $error',
      );
    } finally {
      malloc.free(nativeHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      malloc.free(nativeDirectory);
      if (nativePassword != nullptr) {
        malloc.free(nativePassword);
      }
      if (nativePrivateKey != nullptr) {
        malloc.free(nativePrivateKey);
      }
      if (nativeCertificate != nullptr) {
        malloc.free(nativeCertificate);
      }
      if (nativePassphrase != nullptr) {
        malloc.free(nativePassphrase);
      }
      if (resultPointer != nullptr) {
        bindings.freeString(resultPointer);
      }
    }
  }
}

@immutable
class FfiSshDirectoryEntry {
  const FfiSshDirectoryEntry({
    required this.name,
    required this.isDirectory,
    this.size = 0,
    this.modified,
  });

  final String name;
  final bool isDirectory;
  final int size;
  final DateTime? modified;

  factory FfiSshDirectoryEntry.fromJson(Map<String, Object?> json) {
    final modifiedSeconds = (json['modified'] as num?)?.toInt();
    return FfiSshDirectoryEntry(
      name: json['name'] as String? ?? '',
      isDirectory: json['is_directory'] == true || json['isDirectory'] == true,
      size: (json['size'] as num?)?.toInt() ?? 0,
      modified: modifiedSeconds == null
          ? null
          : DateTime.fromMillisecondsSinceEpoch(
              modifiedSeconds * 1000,
              isUtc: true,
            ),
    );
  }
}

@immutable
class FfiSshDirectoryEntryListingResult {
  const FfiSshDirectoryEntryListingResult({
    required this.entries,
    this.events = const [],
    this.resolvedDirectory,
    this.error,
  });

  final List<FfiSshDirectoryEntry> entries;
  final List<TerminalConnectionEvent> events;
  final String? resolvedDirectory;
  final String? error;

  bool get isError => error != null && error!.trim().isNotEmpty;
}

class FfiSshDirectoryEntryListing {
  const FfiSshDirectoryEntryListing._();

  static FfiSshDirectoryEntryListingResult listDirectoryEntries({
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    required String directory,
    String? password,
    String? privateKey,
    String? certificate,
    String? passphrase,
  }) {
    final bindings = _TerminalBindings.open();
    final nativeHost = host.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativeDirectory = directory.toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativeCertificate = certificate == null
        ? nullptr
        : certificate.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    Pointer<Utf8> resultPointer = nullptr;
    try {
      resultPointer = bindings.sshListDirectoryEntries(
        nativeHost,
        port,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativeCertificate,
        nativePassphrase,
        nativeKnownHostsPath,
        nativeDirectory,
      );
      if (resultPointer == nullptr) {
        return const FfiSshDirectoryEntryListingResult(
          entries: [],
          error: 'SSH completion returned no result.',
        );
      }
      final decoded = jsonDecode(resultPointer.toDartString());
      if (decoded is! Map) {
        return const FfiSshDirectoryEntryListingResult(
          entries: [],
          error: 'SSH completion returned an invalid result.',
        );
      }
      final error = decoded['error'];
      final events = _connectionEventsFromJson(decoded['events']);
      if (error is String && error.trim().isNotEmpty) {
        return FfiSshDirectoryEntryListingResult(
          entries: const [],
          events: events,
          error: error,
        );
      }
      final entries = decoded['entries'];
      if (entries is! List) {
        return const FfiSshDirectoryEntryListingResult(
          entries: [],
          error: 'SSH completion returned no entries.',
        );
      }
      return FfiSshDirectoryEntryListingResult(
        entries: [
          for (final entry in entries)
            if (entry is Map)
              FfiSshDirectoryEntry.fromJson(entry.cast<String, Object?>()),
        ].where((entry) => entry.name.trim().isNotEmpty).toList(),
        events: events,
        resolvedDirectory: decoded['directory'] as String?,
      );
    } on Object catch (error) {
      return FfiSshDirectoryEntryListingResult(
        entries: const [],
        error: 'SSH completion failed: $error',
      );
    } finally {
      malloc.free(nativeHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      malloc.free(nativeDirectory);
      if (nativePassword != nullptr) {
        malloc.free(nativePassword);
      }
      if (nativePrivateKey != nullptr) {
        malloc.free(nativePrivateKey);
      }
      if (nativeCertificate != nullptr) {
        malloc.free(nativeCertificate);
      }
      if (nativePassphrase != nullptr) {
        malloc.free(nativePassphrase);
      }
      if (resultPointer != nullptr) {
        bindings.freeString(resultPointer);
      }
    }
  }
}

List<TerminalConnectionEvent> _connectionEventsFromJson(Object? value) {
  if (value is! List) {
    return const [];
  }
  return [
    for (final event in value)
      if (event is Map)
        TerminalConnectionEvent.fromJson(event.cast<String, Object?>()),
  ];
}

Pointer<Utf8> _proxyConfigToNative(TerminalProxyConfig? proxy) {
  if (proxy == null) {
    return nullptr;
  }
  return jsonEncode(proxy.toJson()).toNativeUtf8();
}

class FfiHostOsDetectionResult {
  const FfiHostOsDetectionResult({
    this.os,
    this.distro,
    this.error,
    this.events = const [],
  });

  final String? os;
  final String? distro;
  final String? error;
  final List<TerminalConnectionEvent> events;

  factory FfiHostOsDetectionResult.fromJson(Map<String, Object?> json) {
    final os = json['os'];
    final distro = json['distro'];
    final error = json['error'];
    return FfiHostOsDetectionResult(
      os: os is String && os.trim().isNotEmpty ? os.trim() : null,
      distro: distro is String && distro.trim().isNotEmpty
          ? distro.trim()
          : null,
      error: error is String && error.trim().isNotEmpty ? error : null,
      events: _connectionEventsFromJson(json['events']),
    );
  }
}

class FfiHostOsDetector {
  const FfiHostOsDetector._();

  static FfiHostOsDetectionResult detect({
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    String? password,
    String? privateKey,
    String? certificate,
    String? passphrase,
    TerminalProxyConfig? proxy,
    SshHostKeyTrustMode hostKeyTrustMode = SshHostKeyTrustMode.strict,
  }) {
    final bindings = _TerminalBindings.open();
    late final _SshDetectHostOsDart detectHostOs;
    try {
      detectHostOs = bindings.library
          .lookupFunction<_SshDetectHostOsNative, _SshDetectHostOsDart>(
            'nauterm_ssh_detect_host_os',
          );
    } on Object catch (error) {
      return FfiHostOsDetectionResult(
        error: 'Host OS detection is not available: $error',
      );
    }
    final nativeHost = host.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativeCertificate = certificate == null
        ? nullptr
        : certificate.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    final nativeProxy = _proxyConfigToNative(proxy);
    Pointer<Utf8> resultPointer = nullptr;
    try {
      resultPointer = detectHostOs(
        nativeHost,
        port,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativeCertificate,
        nativePassphrase,
        nativeKnownHostsPath,
        hostKeyTrustMode.wireValue,
        nativeProxy,
      );
      if (resultPointer == nullptr) {
        return const FfiHostOsDetectionResult(
          error: 'Host OS detection returned no result.',
        );
      }
      final decoded = jsonDecode(resultPointer.toDartString());
      if (decoded is! Map) {
        return const FfiHostOsDetectionResult(
          error: 'Host OS detection returned an invalid result.',
        );
      }
      return FfiHostOsDetectionResult.fromJson(decoded.cast<String, Object?>());
    } on Object catch (error) {
      return FfiHostOsDetectionResult(
        error: 'Host OS detection failed: $error',
      );
    } finally {
      malloc.free(nativeHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      if (nativePassword != nullptr) {
        malloc.free(nativePassword);
      }
      if (nativePrivateKey != nullptr) {
        malloc.free(nativePrivateKey);
      }
      if (nativeCertificate != nullptr) {
        malloc.free(nativeCertificate);
      }
      if (nativePassphrase != nullptr) {
        malloc.free(nativePassphrase);
      }
      if (nativeProxy != nullptr) {
        malloc.free(nativeProxy);
      }
      if (resultPointer != nullptr) {
        bindings.freeString(resultPointer);
      }
    }
  }
}

@immutable
class FfiSshPublicKeyExportResult {
  const FfiSshPublicKeyExportResult({
    required this.ok,
    this.error,
    this.events = const [],
  });

  factory FfiSshPublicKeyExportResult.fromJson(Map<String, Object?> json) {
    final error = json['error'];
    return FfiSshPublicKeyExportResult(
      ok: json['ok'] == true,
      error: error is String && error.trim().isNotEmpty ? error.trim() : null,
      events: _connectionEventsFromJson(json['events']),
    );
  }

  final bool ok;
  final String? error;
  final List<TerminalConnectionEvent> events;

  Map<String, Object?> toJson() => {
    'ok': ok,
    'error': error,
    'events': const [],
  };
}

class FfiSshPublicKeyExporter {
  const FfiSshPublicKeyExporter._();

  static FfiSshPublicKeyExportResult export({
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    required String publicKey,
    required String location,
    required String filename,
    required String script,
    String? password,
    String? privateKey,
    String? certificate,
    String? passphrase,
    TerminalProxyConfig? proxy,
    SshHostKeyTrustMode hostKeyTrustMode = SshHostKeyTrustMode.strict,
  }) {
    final bindings = _TerminalBindings.open();
    late final _SshExportPublicKeyDart exportPublicKey;
    try {
      exportPublicKey = bindings.library
          .lookupFunction<_SshExportPublicKeyNative, _SshExportPublicKeyDart>(
            'nauterm_ssh_export_public_key',
          );
    } on Object catch (error) {
      return FfiSshPublicKeyExportResult(
        ok: false,
        error: 'SSH key export is not available: $error',
      );
    }

    final nativeHost = host.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativePublicKey = publicKey.toNativeUtf8();
    final nativeLocation = location.toNativeUtf8();
    final nativeFilename = filename.toNativeUtf8();
    final nativeScript = script.toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativeCertificate = certificate == null
        ? nullptr
        : certificate.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    final nativeProxy = _proxyConfigToNative(proxy);
    Pointer<Utf8> resultPointer = nullptr;
    try {
      resultPointer = exportPublicKey(
        nativeHost,
        port,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativeCertificate,
        nativePassphrase,
        nativeKnownHostsPath,
        hostKeyTrustMode.wireValue,
        nativeProxy,
        nativePublicKey,
        nativeLocation,
        nativeFilename,
        nativeScript,
      );
      if (resultPointer == nullptr) {
        return const FfiSshPublicKeyExportResult(
          ok: false,
          error: 'SSH key export returned no result.',
        );
      }
      final decoded = jsonDecode(resultPointer.toDartString());
      if (decoded is! Map) {
        return const FfiSshPublicKeyExportResult(
          ok: false,
          error: 'SSH key export returned an invalid result.',
        );
      }
      return FfiSshPublicKeyExportResult.fromJson(
        decoded.cast<String, Object?>(),
      );
    } on Object catch (error) {
      return FfiSshPublicKeyExportResult(
        ok: false,
        error: 'SSH key export failed: $error',
      );
    } finally {
      malloc.free(nativeHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      malloc.free(nativePublicKey);
      malloc.free(nativeLocation);
      malloc.free(nativeFilename);
      malloc.free(nativeScript);
      if (nativePassword != nullptr) {
        malloc.free(nativePassword);
      }
      if (nativePrivateKey != nullptr) {
        malloc.free(nativePrivateKey);
      }
      if (nativeCertificate != nullptr) {
        malloc.free(nativeCertificate);
      }
      if (nativePassphrase != nullptr) {
        malloc.free(nativePassphrase);
      }
      if (nativeProxy != nullptr) {
        malloc.free(nativeProxy);
      }
      if (resultPointer != nullptr) {
        bindings.freeString(resultPointer);
      }
    }
  }
}

@immutable
class FfiHostProcessInfo {
  const FfiHostProcessInfo({
    required this.memoryBytes,
    required this.cpuUsagePercent,
    required this.command,
  });

  final int memoryBytes;
  final double cpuUsagePercent;
  final String command;

  factory FfiHostProcessInfo.fromJson(Map<String, Object?> json) {
    return FfiHostProcessInfo(
      memoryBytes: (json['memory_bytes'] as num?)?.toInt() ?? 0,
      cpuUsagePercent: (json['cpu_usage_percent'] as num?)?.toDouble() ?? 0,
      command: json['command'] as String? ?? '',
    );
  }

  Map<String, Object?> toJson() => {
    'memory_bytes': memoryBytes,
    'cpu_usage_percent': cpuUsagePercent,
    'command': command,
  };
}

@immutable
class FfiHostNetworkInterface {
  const FfiHostNetworkInterface({
    required this.name,
    required this.receivedBytes,
    required this.transmittedBytes,
  });

  final String name;
  final int receivedBytes;
  final int transmittedBytes;

  factory FfiHostNetworkInterface.fromJson(Map<String, Object?> json) {
    return FfiHostNetworkInterface(
      name: json['name'] as String? ?? '',
      receivedBytes: (json['received_bytes'] as num?)?.toInt() ?? 0,
      transmittedBytes: (json['transmitted_bytes'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, Object?> toJson() => {
    'name': name,
    'received_bytes': receivedBytes,
    'transmitted_bytes': transmittedBytes,
  };
}

@immutable
class FfiHostFilesystemInfo {
  const FfiHostFilesystemInfo({
    required this.path,
    required this.totalBytes,
    required this.usedBytes,
  });

  final String path;
  final int totalBytes;
  final int usedBytes;

  factory FfiHostFilesystemInfo.fromJson(Map<String, Object?> json) {
    return FfiHostFilesystemInfo(
      path: json['path'] as String? ?? '',
      totalBytes: (json['total_bytes'] as num?)?.toInt() ?? 0,
      usedBytes: (json['used_bytes'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, Object?> toJson() => {
    'path': path,
    'total_bytes': totalBytes,
    'used_bytes': usedBytes,
  };
}

@immutable
class FfiHostSystemInfoResult {
  const FfiHostSystemInfoResult({
    this.hostname,
    this.osName,
    this.kernel,
    this.architecture,
    this.uptimeSeconds,
    this.loadAverage,
    this.loadAverage5,
    this.loadAverage15,
    this.cpuCount,
    this.cpuUsagePercent,
    this.memoryTotalBytes,
    this.memoryUsedBytes,
    this.swapTotalBytes,
    this.swapUsedBytes,
    this.diskTotalBytes,
    this.diskUsedBytes,
    this.latencyMs,
    this.processes = const [],
    this.networkInterfaces = const [],
    this.filesystems = const [],
    this.error,
    this.events = const [],
  });

  final String? hostname;
  final String? osName;
  final String? kernel;
  final String? architecture;
  final int? uptimeSeconds;
  final double? loadAverage;
  final double? loadAverage5;
  final double? loadAverage15;
  final int? cpuCount;
  final double? cpuUsagePercent;
  final int? memoryTotalBytes;
  final int? memoryUsedBytes;
  final int? swapTotalBytes;
  final int? swapUsedBytes;
  final int? diskTotalBytes;
  final int? diskUsedBytes;
  final double? latencyMs;
  final List<FfiHostProcessInfo> processes;
  final List<FfiHostNetworkInterface> networkInterfaces;
  final List<FfiHostFilesystemInfo> filesystems;
  final String? error;
  final List<TerminalConnectionEvent> events;

  bool get hasData =>
      hostname != null ||
      osName != null ||
      cpuUsagePercent != null ||
      memoryTotalBytes != null ||
      diskTotalBytes != null;

  FfiHostSystemInfoResult withLatency(double? latencyMs) {
    return FfiHostSystemInfoResult(
      hostname: hostname,
      osName: osName,
      kernel: kernel,
      architecture: architecture,
      uptimeSeconds: uptimeSeconds,
      loadAverage: loadAverage,
      loadAverage5: loadAverage5,
      loadAverage15: loadAverage15,
      cpuCount: cpuCount,
      cpuUsagePercent: cpuUsagePercent,
      memoryTotalBytes: memoryTotalBytes,
      memoryUsedBytes: memoryUsedBytes,
      swapTotalBytes: swapTotalBytes,
      swapUsedBytes: swapUsedBytes,
      diskTotalBytes: diskTotalBytes,
      diskUsedBytes: diskUsedBytes,
      latencyMs: latencyMs,
      processes: processes,
      networkInterfaces: networkInterfaces,
      filesystems: filesystems,
      error: error,
      events: events,
    );
  }

  factory FfiHostSystemInfoResult.fromJson(Map<String, Object?> json) {
    String? text(String key) {
      final value = json[key];
      return value is String && value.trim().isNotEmpty ? value.trim() : null;
    }

    int? integer(String key) => (json[key] as num?)?.toInt();
    double? decimal(String key) => (json[key] as num?)?.toDouble();
    List<T> records<T>(
      String key,
      T Function(Map<String, Object?> json) convert,
    ) {
      final values = json[key];
      if (values is! List) {
        return const [];
      }
      return [
        for (final value in values)
          if (value is Map) convert(value.cast<String, Object?>()),
      ];
    }

    return FfiHostSystemInfoResult(
      hostname: text('hostname'),
      osName: text('os_name'),
      kernel: text('kernel'),
      architecture: text('architecture'),
      uptimeSeconds: integer('uptime_seconds'),
      loadAverage: decimal('load_average'),
      loadAverage5: decimal('load_average_5'),
      loadAverage15: decimal('load_average_15'),
      cpuCount: integer('cpu_count'),
      cpuUsagePercent: decimal('cpu_usage_percent'),
      memoryTotalBytes: integer('memory_total_bytes'),
      memoryUsedBytes: integer('memory_used_bytes'),
      swapTotalBytes: integer('swap_total_bytes'),
      swapUsedBytes: integer('swap_used_bytes'),
      diskTotalBytes: integer('disk_total_bytes'),
      diskUsedBytes: integer('disk_used_bytes'),
      latencyMs: decimal('latency_ms'),
      processes: records('processes', FfiHostProcessInfo.fromJson),
      networkInterfaces: records(
        'network_interfaces',
        FfiHostNetworkInterface.fromJson,
      ),
      filesystems: records('filesystems', FfiHostFilesystemInfo.fromJson),
      error: text('error'),
      events: _connectionEventsFromJson(json['events']),
    );
  }

  Map<String, Object?> toJson() {
    return {
      'hostname': hostname,
      'os_name': osName,
      'kernel': kernel,
      'architecture': architecture,
      'uptime_seconds': uptimeSeconds,
      'load_average': loadAverage,
      'load_average_5': loadAverage5,
      'load_average_15': loadAverage15,
      'cpu_count': cpuCount,
      'cpu_usage_percent': cpuUsagePercent,
      'memory_total_bytes': memoryTotalBytes,
      'memory_used_bytes': memoryUsedBytes,
      'swap_total_bytes': swapTotalBytes,
      'swap_used_bytes': swapUsedBytes,
      'disk_total_bytes': diskTotalBytes,
      'disk_used_bytes': diskUsedBytes,
      'latency_ms': latencyMs,
      'processes': [for (final process in processes) process.toJson()],
      'network_interfaces': [
        for (final interface in networkInterfaces) interface.toJson(),
      ],
      'filesystems': [
        for (final filesystem in filesystems) filesystem.toJson(),
      ],
      'error': error,
      'events': const [],
    };
  }
}

class FfiHostSystemInfoCollector {
  const FfiHostSystemInfoCollector._();

  static FfiHostSystemInfoResult collect({
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    String? password,
    String? privateKey,
    String? certificate,
    String? passphrase,
    TerminalProxyConfig? proxy,
    SshHostKeyTrustMode hostKeyTrustMode = SshHostKeyTrustMode.strict,
  }) {
    final bindings = _TerminalBindings.open();
    late final _SshCollectSystemInfoDart collectSystemInfo;
    try {
      collectSystemInfo = bindings.library
          .lookupFunction<
            _SshCollectSystemInfoNative,
            _SshCollectSystemInfoDart
          >('nauterm_ssh_collect_system_info');
    } on Object catch (error) {
      return FfiHostSystemInfoResult(
        error: 'Host system information is not available: $error',
      );
    }
    final nativeHost = host.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativeCertificate = certificate == null
        ? nullptr
        : certificate.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    final nativeProxy = _proxyConfigToNative(proxy);
    Pointer<Utf8> resultPointer = nullptr;
    try {
      resultPointer = collectSystemInfo(
        nativeHost,
        port,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativeCertificate,
        nativePassphrase,
        nativeKnownHostsPath,
        hostKeyTrustMode.wireValue,
        nativeProxy,
      );
      if (resultPointer == nullptr) {
        return const FfiHostSystemInfoResult(
          error: 'Host system information returned no result.',
        );
      }
      final decoded = jsonDecode(resultPointer.toDartString());
      if (decoded is! Map) {
        return const FfiHostSystemInfoResult(
          error: 'Host system information returned an invalid result.',
        );
      }
      return FfiHostSystemInfoResult.fromJson(decoded.cast<String, Object?>());
    } on Object catch (error) {
      return FfiHostSystemInfoResult(
        error: 'Host system information failed: $error',
      );
    } finally {
      malloc.free(nativeHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      if (nativePassword != nullptr) {
        malloc.free(nativePassword);
      }
      if (nativePrivateKey != nullptr) {
        malloc.free(nativePrivateKey);
      }
      if (nativeCertificate != nullptr) {
        malloc.free(nativeCertificate);
      }
      if (nativePassphrase != nullptr) {
        malloc.free(nativePassphrase);
      }
      if (nativeProxy != nullptr) {
        malloc.free(nativeProxy);
      }
      if (resultPointer != nullptr) {
        bindings.freeString(resultPointer);
      }
    }
  }
}

@immutable
class FfiRemoteShellHistoryResult {
  const FfiRemoteShellHistoryResult({
    this.shell,
    this.content = '',
    this.error,
  });

  final String? shell;
  final String content;
  final String? error;
}

class FfiRemoteShellHistoryReader {
  const FfiRemoteShellHistoryReader._();

  static Future<FfiRemoteShellHistoryResult> readSessionInBackground(
    int sessionId,
  ) {
    return compute<int, FfiRemoteShellHistoryResult>(readSession, sessionId);
  }

  static FfiRemoteShellHistoryResult readSession(int sessionId) {
    final bindings = _TerminalBindings.open();
    final pointer = bindings.readSessionShellHistory(sessionId);
    if (pointer == nullptr) {
      return const FfiRemoteShellHistoryResult(
        error: 'No remote history result.',
      );
    }
    try {
      final decoded = jsonDecode(pointer.toDartString());
      if (decoded is! Map) {
        return const FfiRemoteShellHistoryResult(
          error: 'Invalid remote history result.',
        );
      }
      return FfiRemoteShellHistoryResult(
        shell: (decoded['shell'] as String?)?.trim(),
        content: decoded['content'] as String? ?? '',
        error: decoded['error'] as String?,
      );
    } catch (error) {
      return FfiRemoteShellHistoryResult(error: '$error');
    } finally {
      bindings.freeString(pointer);
    }
  }
}

class FfiSftpDirectoryEntryListing {
  const FfiSftpDirectoryEntryListing._();

  static FfiSshDirectoryEntryListingResult decodeResult(Object? decoded) {
    if (decoded is! Map) {
      return const FfiSshDirectoryEntryListingResult(
        entries: [],
        error: 'SFTP listing returned an invalid result.',
      );
    }
    final error = decoded['error'];
    final events = _connectionEventsFromJson(decoded['events']);
    if (error is String && error.trim().isNotEmpty) {
      return FfiSshDirectoryEntryListingResult(
        entries: const [],
        events: events,
        error: error,
      );
    }
    final entries = decoded['entries'];
    if (entries is! List) {
      return FfiSshDirectoryEntryListingResult(
        entries: const [],
        events: events,
        error: 'SFTP listing returned no entries.',
      );
    }
    return FfiSshDirectoryEntryListingResult(
      entries: [
        for (final entry in entries)
          if (entry is Map)
            FfiSshDirectoryEntry.fromJson(entry.cast<String, Object?>()),
      ].where((entry) => entry.name.trim().isNotEmpty).toList(),
      events: events,
      resolvedDirectory: decoded['directory'] as String?,
    );
  }

  static FfiSshDirectoryEntryListingResult listDirectoryEntries({
    required int requestId,
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    required String directory,
    String? password,
    String? privateKey,
    String? certificate,
    String? passphrase,
    TerminalProxyConfig? proxy,
    SshHostKeyTrustMode hostKeyTrustMode = SshHostKeyTrustMode.strict,
  }) {
    final bindings = _TerminalBindings.open();
    final nativeHost = host.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativeDirectory = directory.toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativeCertificate = certificate == null
        ? nullptr
        : certificate.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    final nativeProxy = _proxyConfigToNative(proxy);
    Pointer<Utf8> resultPointer = nullptr;
    try {
      resultPointer = bindings.sftpListDirectoryEntries(
        requestId,
        nativeHost,
        port,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativeCertificate,
        nativePassphrase,
        nativeKnownHostsPath,
        nativeDirectory,
        hostKeyTrustMode.wireValue,
        nativeProxy,
      );
      if (resultPointer == nullptr) {
        return const FfiSshDirectoryEntryListingResult(
          entries: [],
          error: 'SFTP listing returned no result.',
        );
      }
      return decodeResult(jsonDecode(resultPointer.toDartString()));
    } on Object catch (error) {
      return FfiSshDirectoryEntryListingResult(
        entries: const [],
        error: 'SFTP listing failed: $error',
      );
    } finally {
      malloc.free(nativeHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      malloc.free(nativeDirectory);
      if (nativePassword != nullptr) {
        malloc.free(nativePassword);
      }
      if (nativePrivateKey != nullptr) {
        malloc.free(nativePrivateKey);
      }
      if (nativeCertificate != nullptr) {
        malloc.free(nativeCertificate);
      }
      if (nativePassphrase != nullptr) {
        malloc.free(nativePassphrase);
      }
      if (nativeProxy != nullptr) {
        malloc.free(nativeProxy);
      }
      if (resultPointer != nullptr) {
        bindings.freeString(resultPointer);
      }
    }
  }
}

class FfiSftpTaskResult {
  const FfiSftpTaskResult({
    required this.ok,
    required this.bytes,
    required this.itemKind,
    this.error,
    this.events = const [],
  });

  final bool ok;
  final int bytes;
  final String itemKind;
  final String? error;
  final List<TerminalConnectionEvent> events;

  factory FfiSftpTaskResult.fromJson(Map<String, Object?> json) {
    return FfiSftpTaskResult(
      ok: json['ok'] == true,
      bytes: (json['bytes'] as num?)?.toInt() ?? 0,
      itemKind: json['item_kind'] as String? ?? 'unknown',
      error: json['error'] as String?,
      events: _connectionEventsFromJson(json['events']),
    );
  }
}

class FfiSftpTaskProgress {
  const FfiSftpTaskProgress({
    required this.transferredBytes,
    required this.totalBytes,
    required this.currentPath,
  });

  final int transferredBytes;
  final int totalBytes;
  final String currentPath;
}

class FfiSftpTaskExecutor {
  const FfiSftpTaskExecutor._();

  static FfiSftpTaskResult execute({
    required int taskId,
    required String host,
    required int port,
    required String username,
    required String knownHostsPath,
    required Map<String, Object?> operation,
    String? password,
    String? privateKey,
    String? certificate,
    String? passphrase,
    TerminalProxyConfig? proxy,
    SshHostKeyTrustMode hostKeyTrustMode = SshHostKeyTrustMode.strict,
    ValueChanged<FfiSftpTaskProgress>? onProgress,
  }) {
    final bindings = _TerminalBindings.open();
    final nativeHost = host.toNativeUtf8();
    final nativeUsername = username.toNativeUtf8();
    final nativeKnownHostsPath = knownHostsPath.toNativeUtf8();
    final nativeOperation = jsonEncode(operation).toNativeUtf8();
    final nativePassword = password == null ? nullptr : password.toNativeUtf8();
    final nativePrivateKey = privateKey == null
        ? nullptr
        : privateKey.toNativeUtf8();
    final nativeCertificate = certificate == null
        ? nullptr
        : certificate.toNativeUtf8();
    final nativePassphrase = passphrase == null
        ? nullptr
        : passphrase.toNativeUtf8();
    final nativeProxy = _proxyConfigToNative(proxy);
    NativeCallable<_SftpTaskProgressCallbackNative>? progressCallback;
    if (onProgress != null) {
      progressCallback =
          NativeCallable<_SftpTaskProgressCallbackNative>.isolateLocal((
            Pointer<Void> _,
            int transferredBytes,
            int totalBytes,
            Pointer<Utf8> currentPath,
          ) {
            onProgress(
              FfiSftpTaskProgress(
                transferredBytes: transferredBytes,
                totalBytes: totalBytes,
                currentPath: currentPath == nullptr
                    ? ''
                    : currentPath.toDartString(),
              ),
            );
          });
    }
    Pointer<Utf8> resultPointer = nullptr;
    try {
      resultPointer = bindings.sftpExecuteTask(
        taskId,
        nativeHost,
        port,
        nativeUsername,
        nativePassword,
        nativePrivateKey,
        nativeCertificate,
        nativePassphrase,
        nativeKnownHostsPath,
        nativeOperation,
        hostKeyTrustMode.wireValue,
        nativeProxy,
        progressCallback?.nativeFunction ?? nullptr,
        nullptr,
      );
      if (resultPointer == nullptr) {
        return const FfiSftpTaskResult(
          ok: false,
          bytes: 0,
          itemKind: 'unknown',
          error: 'SFTP task returned no result.',
        );
      }
      final decoded = jsonDecode(resultPointer.toDartString());
      if (decoded is! Map) {
        return const FfiSftpTaskResult(
          ok: false,
          bytes: 0,
          itemKind: 'unknown',
          error: 'SFTP task returned an invalid result.',
        );
      }
      return FfiSftpTaskResult.fromJson(decoded.cast<String, Object?>());
    } on Object catch (error) {
      return FfiSftpTaskResult(
        ok: false,
        bytes: 0,
        itemKind: 'unknown',
        error: 'SFTP task failed: $error',
      );
    } finally {
      malloc.free(nativeHost);
      malloc.free(nativeUsername);
      malloc.free(nativeKnownHostsPath);
      malloc.free(nativeOperation);
      if (nativePassword != nullptr) {
        malloc.free(nativePassword);
      }
      if (nativePrivateKey != nullptr) {
        malloc.free(nativePrivateKey);
      }
      if (nativeCertificate != nullptr) {
        malloc.free(nativeCertificate);
      }
      if (nativePassphrase != nullptr) {
        malloc.free(nativePassphrase);
      }
      if (nativeProxy != nullptr) {
        malloc.free(nativeProxy);
      }
      if (resultPointer != nullptr) {
        bindings.freeString(resultPointer);
      }
      progressCallback?.close();
    }
  }

  static bool cancel(int taskId) {
    final bindings = _TerminalBindings.open();
    return bindings.sftpCancelTask(taskId);
  }

  static bool closeSudoSession(String sessionId) {
    try {
      final bindings = _TerminalBindings.open();
      final nativeSessionId = sessionId.toNativeUtf8();
      try {
        return bindings.sftpCloseSudoSession(nativeSessionId);
      } finally {
        malloc.free(nativeSessionId);
      }
    } on Object {
      return false;
    }
  }
}
