part of 'nauterm_workspace.dart';

bool _sftpPermissionDenied(String? error) {
  final normalized = error?.trim().toLowerCase() ?? '';
  return normalized.contains('permission denied') ||
      normalized.contains('permissiondenied') ||
      normalized.contains('permission_denied') ||
      normalized.contains('operation not permitted') ||
      normalized.contains('status code: 3');
}

bool _sftpSudoAuthenticationFailed(String? error) {
  final normalized = error?.trim().toLowerCase() ?? '';
  return normalized.contains('incorrect password') ||
      normalized.contains('authentication failure') ||
      normalized.contains('a password is required') ||
      normalized.contains('sudo password') ||
      normalized.contains('sudo authentication failed') ||
      normalized.contains('sudo sftp session expired') ||
      normalized.contains('sudo sftp session is not authenticated');
}

enum _SftpTaskType {
  download('download', Icons.file_download_rounded),
  upload('upload', Icons.file_upload_rounded),
  transferDownload('download', Icons.file_download_rounded),
  transferUpload('upload', Icons.file_upload_rounded),
  edit('upload', LucideIcons.pencil),
  move('move', Icons.drive_file_move_rounded),
  copy('copy', Icons.copy_rounded),
  newFolder('mkdir', Icons.create_new_folder_rounded),
  delete('delete', LucideIcons.trash2);

  const _SftpTaskType(this.wireName, this.icon);

  final String wireName;
  final IconData icon;
}

enum _SftpTaskStatus { queued, running, paused, completed, failed, cancelled }

bool _isPausableSftpTask(_SftpTask task) {
  return switch (task.type) {
    _SftpTaskType.download ||
    _SftpTaskType.upload ||
    _SftpTaskType.transferDownload ||
    _SftpTaskType.transferUpload ||
    _SftpTaskType.edit => true,
    _ => false,
  };
}

_SftpTaskType? _sftpTaskTypeFromHistory(String value) {
  return _SftpTaskType.values.where((type) => type.name == value).firstOrNull;
}

String _persistedSftpTaskType(_SftpTaskType type) {
  return switch (type) {
    _SftpTaskType.transferDownload => _SftpTaskType.download.name,
    _SftpTaskType.transferUpload => _SftpTaskType.upload.name,
    _ => type.name,
  };
}

_SftpTaskStatus? _sftpTaskStatusFromHistory(String value) {
  return _SftpTaskStatus.values
      .where((status) => status.name == value)
      .firstOrNull;
}

const Object _sftpTaskUnchanged = Object();

class _SftpTask {
  const _SftpTask({
    required this.id,
    required this.nativeTaskId,
    required this.slot,
    required this.type,
    required this.status,
    required this.displayName,
    required this.sourcePath,
    required this.targetPath,
    required this.createdAt,
    this.bytes = 0,
    this.totalBytes = 0,
    this.currentPath = '',
    this.cancelRequested = false,
    this.pauseRequested = false,
    this.itemKind = 'unknown',
    this.error,
    this.historyId,
    this.finishedAt,
  });

  final int id;
  final int nativeTaskId;
  final _SftpPaneSlot slot;
  final _SftpTaskType type;
  final _SftpTaskStatus status;
  final String displayName;
  final String sourcePath;
  final String targetPath;
  final DateTime createdAt;
  final int bytes;
  final int totalBytes;
  final String currentPath;
  final bool cancelRequested;
  final bool pauseRequested;
  final String itemKind;
  final String? error;
  final int? historyId;
  final DateTime? finishedAt;

  _SftpTask copyWith({
    int? nativeTaskId,
    _SftpTaskStatus? status,
    int? bytes,
    int? totalBytes,
    String? currentPath,
    bool? cancelRequested,
    bool? pauseRequested,
    String? itemKind,
    Object? error = _sftpTaskUnchanged,
    Object? historyId = _sftpTaskUnchanged,
    Object? finishedAt = _sftpTaskUnchanged,
  }) {
    return _SftpTask(
      id: id,
      nativeTaskId: nativeTaskId ?? this.nativeTaskId,
      slot: slot,
      type: type,
      status: status ?? this.status,
      displayName: displayName,
      sourcePath: sourcePath,
      targetPath: targetPath,
      createdAt: createdAt,
      bytes: bytes ?? this.bytes,
      totalBytes: totalBytes ?? this.totalBytes,
      currentPath: currentPath ?? this.currentPath,
      cancelRequested: cancelRequested ?? this.cancelRequested,
      pauseRequested: pauseRequested ?? this.pauseRequested,
      itemKind: itemKind ?? this.itemKind,
      error: identical(error, _sftpTaskUnchanged)
          ? this.error
          : error as String?,
      historyId: identical(historyId, _sftpTaskUnchanged)
          ? this.historyId
          : historyId as int?,
      finishedAt: identical(finishedAt, _sftpTaskUnchanged)
          ? this.finishedAt
          : finishedAt as DateTime?,
    );
  }
}

class _QueuedSftpTaskExecution {
  const _QueuedSftpTaskExecution({
    required this.nativeTaskId,
    required this.auth,
    required this.operation,
    required this.refreshLocal,
    required this.refreshRemote,
    this.localSession,
    this.remoteSession,
    this.persistHistory = true,
    this.withSudo = false,
  });

  final int nativeTaskId;
  final _SftpRemoteAuth auth;
  final Map<String, Object?> operation;
  final bool refreshLocal;
  final bool refreshRemote;
  final _SftpLocalPaneSession? localSession;
  final _SftpRemotePaneSession? remoteSession;
  final bool persistHistory;
  final bool withSudo;

  _QueuedSftpTaskExecution withNativeTaskId(int value) {
    return _QueuedSftpTaskExecution(
      nativeTaskId: value,
      auth: auth,
      operation: operation,
      refreshLocal: refreshLocal,
      refreshRemote: refreshRemote,
      localSession: localSession,
      remoteSession: remoteSession,
      persistHistory: persistHistory,
      withSudo: withSudo,
    );
  }
}

FfiSftpTaskResult _runSftpTask(
  Map<String, Object?> arguments, {
  required ValueChanged<FfiSftpTaskProgress> onProgress,
}) {
  return FfiSftpTaskExecutor.execute(
    host: arguments['host'] as String,
    taskId: arguments['taskId'] as int,
    port: arguments['port'] as int,
    username: arguments['username'] as String,
    knownHostsPath: arguments['knownHostsPath'] as String,
    operation: (arguments['operation'] as Map).cast<String, Object?>(),
    password: arguments['password'] as String?,
    privateKey: arguments['privateKey'] as String?,
    certificate: arguments['certificate'] as String?,
    passphrase: arguments['passphrase'] as String?,
    proxy: _terminalProxyFromArguments(arguments['proxy']),
    hostKeyTrustMode: SshHostKeyTrustMode.fromWireValue(
      arguments['hostKeyTrustMode'],
    ),
    onProgress: onProgress,
  );
}

Future<FfiSftpTaskResult> _spawnSftpTask(
  Map<String, Object?> arguments, {
  required ValueChanged<_SftpTaskProgressUpdate> onProgress,
}) async {
  final receivePort = ReceivePort();
  final completer = Completer<FfiSftpTaskResult>();
  StreamSubscription<Object?>? subscription;
  try {
    await Isolate.spawn(_sftpTaskIsolateMain, [
      receivePort.sendPort,
      arguments,
    ]);
    subscription = receivePort.listen((message) {
      if (message is! Map) {
        if (!completer.isCompleted) {
          completer.complete(
            const FfiSftpTaskResult(
              ok: false,
              bytes: 0,
              itemKind: 'unknown',
              error: 'SFTP task isolate returned an invalid message.',
            ),
          );
        }
        return;
      }
      final typedMessage = message.cast<String, Object?>();
      switch (typedMessage['type']) {
        case 'progress':
          onProgress(_SftpTaskProgressUpdate.fromJson(typedMessage));
        case 'result':
          if (!completer.isCompleted) {
            completer.complete(FfiSftpTaskResult.fromJson(typedMessage));
          }
        default:
          if (!completer.isCompleted) {
            completer.complete(
              const FfiSftpTaskResult(
                ok: false,
                bytes: 0,
                itemKind: 'unknown',
                error: 'SFTP task isolate returned an unknown message.',
              ),
            );
          }
      }
    });
    return await completer.future;
  } catch (error) {
    return FfiSftpTaskResult(
      ok: false,
      bytes: 0,
      itemKind: 'unknown',
      error: 'SFTP task failed: $error',
    );
  } finally {
    await subscription?.cancel();
    receivePort.close();
  }
}

void _sftpTaskIsolateMain(List<Object?> message) {
  final sendPort = message[0] as SendPort;
  final arguments = (message[1] as Map).cast<String, Object?>();
  try {
    final result = _runSftpTask(
      arguments,
      onProgress: (progress) {
        sendPort.send({
          'type': 'progress',
          'transferred_bytes': progress.transferredBytes,
          'total_bytes': progress.totalBytes,
          'current_path': progress.currentPath,
        });
      },
    );
    sendPort.send({
      'type': 'result',
      'ok': result.ok,
      'bytes': result.bytes,
      'item_kind': result.itemKind,
      'error': result.error,
      'events': [
        for (final event in result.events)
          {
            'kind': _terminalConnectionEventKindToWire(event.kind),
            'message': event.message,
            'host': event.host,
            'port': event.port,
            'serial_port': event.serialPort,
            'baud_rate': event.baudRate,
            'username': event.username,
            'fingerprint': event.fingerprint,
            'method': event.method,
            'exit_status': event.exitStatus,
          },
      ],
    });
  } on Object catch (error) {
    sendPort.send({
      'type': 'result',
      'ok': false,
      'bytes': 0,
      'item_kind': 'unknown',
      'error': 'SFTP task failed: $error',
      'events': const [],
    });
  }
}

class _SftpTaskProgressUpdate {
  const _SftpTaskProgressUpdate({
    required this.transferredBytes,
    required this.totalBytes,
    required this.currentPath,
  });

  factory _SftpTaskProgressUpdate.fromJson(Map<String, Object?> json) {
    return _SftpTaskProgressUpdate(
      transferredBytes: (json['transferred_bytes'] as num?)?.toInt() ?? 0,
      totalBytes: (json['total_bytes'] as num?)?.toInt() ?? 0,
      currentPath: json['current_path'] as String? ?? '',
    );
  }

  final int transferredBytes;
  final int totalBytes;
  final String currentPath;
}
