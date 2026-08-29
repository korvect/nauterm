part of 'nauterm_workspace.dart';

FfiHostOsDetectionResult _runHostOsDetection(Map<String, Object?> arguments) {
  return FfiHostOsDetector.detect(
    host: arguments['host'] as String,
    port: arguments['port'] as int,
    username: arguments['username'] as String,
    knownHostsPath: arguments['knownHostsPath'] as String,
    password: arguments['password'] as String?,
    privateKey: arguments['privateKey'] as String?,
    certificate: arguments['certificate'] as String?,
    passphrase: arguments['passphrase'] as String?,
    proxy: _terminalProxyFromArguments(arguments['proxy']),
    hostKeyTrustMode: SshHostKeyTrustMode.fromWireValue(
      arguments['hostKeyTrustMode'],
    ),
  );
}

FfiHostSystemInfoResult _runHostSystemInfo(Map<String, Object?> arguments) {
  return FfiHostSystemInfoCollector.collect(
    host: arguments['host'] as String,
    port: arguments['port'] as int,
    username: arguments['username'] as String,
    knownHostsPath: arguments['knownHostsPath'] as String,
    password: arguments['password'] as String?,
    privateKey: arguments['privateKey'] as String?,
    certificate: arguments['certificate'] as String?,
    passphrase: arguments['passphrase'] as String?,
    proxy: _terminalProxyFromArguments(arguments['proxy']),
    hostKeyTrustMode: SshHostKeyTrustMode.fromWireValue(
      arguments['hostKeyTrustMode'],
    ),
  );
}

FfiSshPublicKeyExportResult _runSshPublicKeyExport(
  Map<String, Object?> arguments,
) {
  return FfiSshPublicKeyExporter.export(
    host: arguments['host'] as String,
    port: arguments['port'] as int,
    username: arguments['username'] as String,
    knownHostsPath: arguments['knownHostsPath'] as String,
    publicKey: arguments['publicKey'] as String,
    location: arguments['location'] as String,
    filename: arguments['filename'] as String,
    script: arguments['script'] as String,
    password: arguments['password'] as String?,
    privateKey: arguments['privateKey'] as String?,
    certificate: arguments['certificate'] as String?,
    passphrase: arguments['passphrase'] as String?,
    proxy: _terminalProxyFromArguments(arguments['proxy']),
    hostKeyTrustMode: SshHostKeyTrustMode.fromWireValue(
      arguments['hostKeyTrustMode'],
    ),
  );
}

FfiSshDirectoryListingResult _runSshDirectoryListing(
  Map<String, Object?> arguments,
) {
  return FfiSshDirectoryListing.listDirectories(
    host: arguments['host'] as String,
    port: arguments['port'] as int,
    username: arguments['username'] as String,
    knownHostsPath: arguments['knownHostsPath'] as String,
    directory: arguments['directory'] as String,
    password: arguments['password'] as String?,
    privateKey: arguments['privateKey'] as String?,
    certificate: arguments['certificate'] as String?,
    passphrase: arguments['passphrase'] as String?,
  );
}

FfiSshDirectoryEntryListingResult _runSshDirectoryEntryListing(
  Map<String, Object?> arguments,
) {
  return FfiSshDirectoryEntryListing.listDirectoryEntries(
    host: arguments['host'] as String,
    port: arguments['port'] as int,
    username: arguments['username'] as String,
    knownHostsPath: arguments['knownHostsPath'] as String,
    directory: arguments['directory'] as String,
    password: arguments['password'] as String?,
    privateKey: arguments['privateKey'] as String?,
    certificate: arguments['certificate'] as String?,
    passphrase: arguments['passphrase'] as String?,
  );
}

FfiSshDirectoryEntryListingResult _runSftpDirectoryEntryListing(
  Map<String, Object?> arguments,
) {
  final requestId = (arguments['requestId'] as num?)?.toInt();
  if (requestId == null) {
    return const FfiSshDirectoryEntryListingResult(
      entries: [],
      error: 'SFTP listing request ID is missing.',
    );
  }
  return FfiSftpDirectoryEntryListing.listDirectoryEntries(
    requestId: requestId,
    host: arguments['host'] as String,
    port: arguments['port'] as int,
    username: arguments['username'] as String,
    knownHostsPath: arguments['knownHostsPath'] as String,
    directory: arguments['directory'] as String,
    password: arguments['password'] as String?,
    privateKey: arguments['privateKey'] as String?,
    certificate: arguments['certificate'] as String?,
    passphrase: arguments['passphrase'] as String?,
    proxy: _terminalProxyFromArguments(arguments['proxy']),
    hostKeyTrustMode: SshHostKeyTrustMode.fromWireValue(
      arguments['hostKeyTrustMode'],
    ),
  );
}

TerminalProxyConfig? _terminalProxyFromArguments(Object? value) {
  if (value is! Map) {
    return null;
  }
  final map = value.cast<String, Object?>();
  final type = (map['type'] as String?)?.trim();
  final host = (map['host'] as String?)?.trim();
  final port = map['port'];
  if (type == null || type.isEmpty || host == null || host.isEmpty) {
    return null;
  }
  final proxyPort = port is int ? port : int.tryParse('${port ?? ''}');
  if (proxyPort == null || proxyPort < 1 || proxyPort > 65535) {
    return null;
  }
  return TerminalProxyConfig(
    type: type,
    host: host,
    port: proxyPort,
    username: (map['username'] as String?)?.trim(),
    password: map['password'] as String?,
  );
}

Future<FfiHostOsDetectionResult> _spawnHostOsDetection(
  Map<String, Object?> arguments,
) async {
  final receivePort = ReceivePort();
  try {
    await Isolate.spawn(_hostOsDetectionIsolateMain, [
      receivePort.sendPort,
      arguments,
    ]);
    final message = await receivePort.first;
    if (message is Map) {
      return FfiHostOsDetectionResult.fromJson(message.cast<String, Object?>());
    }
    return const FfiHostOsDetectionResult(
      error: 'Host OS detection isolate returned an invalid message.',
    );
  } finally {
    receivePort.close();
  }
}

Future<FfiHostSystemInfoResult> _spawnHostSystemInfo(
  Map<String, Object?> arguments,
) async {
  final receivePort = ReceivePort();
  try {
    await Isolate.spawn(_hostSystemInfoIsolateMain, [
      receivePort.sendPort,
      arguments,
    ]);
    final message = await receivePort.first;
    if (message is Map) {
      return FfiHostSystemInfoResult.fromJson(message.cast<String, Object?>());
    }
    return const FfiHostSystemInfoResult(
      error: 'Host system information isolate returned an invalid message.',
    );
  } finally {
    receivePort.close();
  }
}

Future<FfiSshPublicKeyExportResult> _spawnSshPublicKeyExport(
  Map<String, Object?> arguments,
) async {
  final receivePort = ReceivePort();
  try {
    await Isolate.spawn(_sshPublicKeyExportIsolateMain, [
      receivePort.sendPort,
      arguments,
    ]);
    final message = await receivePort.first;
    if (message is Map) {
      return FfiSshPublicKeyExportResult.fromJson(
        message.cast<String, Object?>(),
      );
    }
    return const FfiSshPublicKeyExportResult(
      ok: false,
      error: 'SSH key export isolate returned an invalid message.',
    );
  } finally {
    receivePort.close();
  }
}

Future<FfiSshDirectoryListingResult> _spawnSshDirectoryListing(
  Map<String, Object?> arguments,
) async {
  final receivePort = ReceivePort();
  try {
    await Isolate.spawn(_sshDirectoryListingIsolateMain, [
      receivePort.sendPort,
      arguments,
    ]);
    final message = await receivePort.first;
    if (message is Map) {
      final entries = message['entries'];
      final error = message['error'];
      return FfiSshDirectoryListingResult(
        entries: [
          if (entries is List)
            for (final entry in entries)
              if (entry is String && entry.trim().isNotEmpty) entry,
        ],
        resolvedDirectory: message['directory'] as String?,
        error: error is String && error.trim().isNotEmpty ? error : null,
      );
    }
    return const FfiSshDirectoryListingResult(
      entries: [],
      error: 'SSH completion isolate returned an invalid message.',
    );
  } finally {
    receivePort.close();
  }
}

Future<FfiSshDirectoryEntryListingResult> _spawnSshDirectoryEntryListing(
  Map<String, Object?> arguments,
) async {
  final receivePort = ReceivePort();
  try {
    await Isolate.spawn(_sshDirectoryEntryListingIsolateMain, [
      receivePort.sendPort,
      arguments,
    ]);
    final message = await receivePort.first;
    if (message is Map) {
      final entries = message['entries'];
      final error = message['error'];
      return FfiSshDirectoryEntryListingResult(
        entries: [
          if (entries is List)
            for (final entry in entries)
              if (entry is Map)
                FfiSshDirectoryEntry.fromJson(entry.cast<String, Object?>()),
        ].where((entry) => entry.name.trim().isNotEmpty).toList(),
        events: [
          if (message['events'] is List)
            for (final event in message['events'] as List)
              if (event is Map)
                TerminalConnectionEvent.fromJson(event.cast<String, Object?>()),
        ],
        resolvedDirectory: message['directory'] as String?,
        error: error is String && error.trim().isNotEmpty ? error : null,
      );
    }
    return const FfiSshDirectoryEntryListingResult(
      entries: [],
      error: 'SSH completion isolate returned an invalid message.',
    );
  } finally {
    receivePort.close();
  }
}

Future<FfiSshDirectoryEntryListingResult> _spawnSftpDirectoryEntryListing(
  Map<String, Object?> arguments, {
  required int requestId,
  _SftpListingCancellation? cancellation,
}) async {
  final receivePort = ReceivePort();
  arguments = Map<String, Object?>.of(arguments)..['requestId'] = requestId;
  cancellation?.attachReceivePort(receivePort);
  Isolate? isolate;
  try {
    isolate = await Isolate.spawn(_sftpDirectoryEntryListingIsolateMain, [
      receivePort.sendPort,
      arguments,
    ]);
    cancellation?.attachIsolate(isolate);
    final message = await receivePort.first;
    if (cancellation?.isCancelled == true) {
      return const FfiSshDirectoryEntryListingResult(
        entries: [],
        error: 'SFTP listing cancelled.',
      );
    }
    if (message is Map) {
      final entries = message['entries'];
      final error = message['error'];
      return FfiSshDirectoryEntryListingResult(
        entries: [
          if (entries is List)
            for (final entry in entries)
              if (entry is Map)
                FfiSshDirectoryEntry.fromJson(entry.cast<String, Object?>()),
        ].where((entry) => entry.name.trim().isNotEmpty).toList(),
        events: [
          if (message['events'] is List)
            for (final event in message['events'] as List)
              if (event is Map)
                TerminalConnectionEvent.fromJson(event.cast<String, Object?>()),
        ],
        resolvedDirectory: message['directory'] as String?,
        error: error is String && error.trim().isNotEmpty ? error : null,
      );
    }
    return const FfiSshDirectoryEntryListingResult(
      entries: [],
      error: 'SFTP listing isolate returned an invalid message.',
    );
  } on StateError {
    if (cancellation?.isCancelled == true) {
      return const FfiSshDirectoryEntryListingResult(
        entries: [],
        error: 'SFTP listing cancelled.',
      );
    }
    rethrow;
  } finally {
    cancellation?.detach(isolate, receivePort);
    receivePort.close();
  }
}

class _SftpListingCancellation {
  _SftpListingCancellation(this.requestId);

  final int requestId;
  bool _cancelled = false;
  Isolate? _isolate;
  ReceivePort? _receivePort;

  bool get isCancelled => _cancelled;

  void attachIsolate(Isolate isolate) {
    if (_cancelled) {
      isolate.kill(priority: Isolate.immediate);
      return;
    }
    _isolate = isolate;
  }

  void attachReceivePort(ReceivePort receivePort) {
    if (_cancelled) {
      receivePort.close();
      return;
    }
    _receivePort = receivePort;
  }

  void detach(Isolate? isolate, ReceivePort receivePort) {
    if (identical(_isolate, isolate)) {
      _isolate = null;
    }
    if (identical(_receivePort, receivePort)) {
      _receivePort = null;
    }
  }

  void cancel() {
    if (_cancelled) {
      return;
    }
    _cancelled = true;
    FfiSftpTaskExecutor.cancel(requestId);
    _receivePort?.close();
    _receivePort = null;
    _isolate?.kill(priority: Isolate.immediate);
    _isolate = null;
  }
}

Future<List<String>> _runLocalShellCommandCompletion({
  required String shellKind,
  required String shellPath,
  required String workingDirectory,
  required String prefix,
}) async {
  final script = shellKind == 'zsh'
      ? r'''
emulate -L zsh
autoload -Uz compinit
compinit -C -i >/dev/null 2>&1
prefix=$1
{
  print -rl -- ${(k)commands}
  print -rl -- ${(k)aliases}
  print -rl -- ${(k)builtins}
  print -rl -- ${(k)functions}
} | awk -v p="$prefix" 'index($0, p) == 1 { print $0 }' | sort -u | head -n 80
'''
      : r'''
prefix=$1
compgen -A command -- "$prefix" | sort -u | head -n 80
''';
  final result = await io.Process.run(
    shellPath,
    ['-ic', script, 'nauterm-complete', prefix],
    workingDirectory: workingDirectory,
  ).timeout(const Duration(milliseconds: 900));
  if (result.exitCode != 0) {
    return const [];
  }
  final commandName = RegExp(r'''^[A-Za-z0-9_+.,:@%/\[\]-]+$''');
  final seen = <String>{};
  return [
    for (final line in result.stdout.toString().split('\n'))
      if (line.startsWith(prefix) &&
          line != prefix &&
          commandName.hasMatch(line) &&
          seen.add(line))
        line,
  ];
}

Future<List<String>> _runLocalZshCompletion({
  required String shellPath,
  required String workingDirectory,
  required String input,
  required int limit,
}) async {
  final script = r'''
emulate -L zsh
NAUTERM_INPUT=$1
NAUTERM_LIMIT=${2:-999}

nauterm_comptest() {
  emulate -L zsh
  setopt no_beep
  autoload -Uz compinit
  compinit -C -i >/dev/null 2>&1
  typeset -gaU nauterm_completions=()

  compadd() {
    local -a reply
    builtin compadd -O reply "$@" 2>/dev/null
    nauterm_completions+=("${reply[@]}")
    builtin compadd "$@" 2>/dev/null
  }

  bindkey "^I" complete-word
  zle -C complete-word complete-word complete-word
  complete-word() {
    unset 'compstate[vared]'
    _main_complete "$@" >/dev/null 2>&1
    print -n $'\002'
    print -nlr -- "${nauterm_completions[@]:0:$NAUTERM_LIMIT}"
    print -n $'\003'
    exit
  }

  vared -c nauterm_buffer
}

zmodload zsh/zpty || exit 1
zpty -b nauterm_comptest nauterm_comptest || exit 1
sleep 0.03
while zpty -rt nauterm_comptest nauterm_chunk; do :; done
zpty -w -n nauterm_comptest "$NAUTERM_INPUT"$'\t'

nauterm_output=''
for _ in {1..24}; do
  sleep 0.04
  while zpty -rt nauterm_comptest nauterm_chunk; do
    nauterm_output+=$nauterm_chunk$'\n'
  done
  [[ $nauterm_output == *$'\003'* ]] && break
done
zpty -d nauterm_comptest

nauterm_output=${nauterm_output#*$'\002'}
nauterm_output=${nauterm_output%%$'\003'*}
print -rn -- "$nauterm_output"
''';
  final result = await io.Process.run(
    shellPath,
    ['-ic', script, 'nauterm-zsh-complete', input, '$limit'],
    workingDirectory: workingDirectory,
  ).timeout(const Duration(milliseconds: 1300));
  if (result.exitCode != 0) {
    return const [];
  }

  final seen = <String>{};
  final candidates = <String>[];
  for (final line in result.stdout.toString().split('\n')) {
    final candidate = _zshCompletionCandidateToInput(
      input: input,
      completion: line.trimRight(),
      workingDirectory: workingDirectory,
    );
    if (candidate == null ||
        candidate == input ||
        candidate.contains('\n') ||
        !seen.add(candidate)) {
      continue;
    }
    candidates.add(candidate);
    if (candidates.length >= limit) {
      break;
    }
  }
  return candidates;
}

String? _zshCompletionCandidateToInput({
  required String input,
  required String completion,
  required String workingDirectory,
}) {
  if (completion.trim().isEmpty) {
    return null;
  }
  final pathQuery = WorkspaceComposerCompletion.shellPathQuery(
    input,
    workingDirectory: workingDirectory,
    expandHome: true,
    home: io.Platform.environment['HOME'],
  );
  if (pathQuery != null) {
    final completionName = completion.endsWith('/')
        ? completion.substring(0, completion.length - 1)
        : completion;
    final directoryPath = _resolveLocalCompletionCandidatePath(
      pathQuery.directoryPath,
      completionName,
    );
    final exists =
        directoryPath != null &&
        io.FileSystemEntity.typeSync(directoryPath, followLinks: false) !=
            io.FileSystemEntityType.notFound;
    if (completion.endsWith('/') || exists) {
      final candidates = WorkspaceComposerCompletion.pathCandidates(pathQuery, [
        WorkspacePathCompletionEntry(
          name: completionName,
          isDirectory:
              completion.endsWith('/') ||
              (directoryPath != null &&
                  io.Directory(directoryPath).existsSync()),
        ),
      ], limit: 1);
      if (candidates.isNotEmpty) {
        return candidates.first;
      }
    }
  }
  final tokenStart = _currentSimpleShellTokenStart(input);
  return '${input.substring(0, tokenStart)}$completion';
}

int _currentSimpleShellTokenStart(String input) {
  var tokenStart = 0;
  var escaping = false;
  String? quote;
  for (var index = 0; index < input.length; index++) {
    final char = input[index];
    if (escaping) {
      escaping = false;
      continue;
    }
    if (quote == "'") {
      if (char == "'") {
        quote = null;
      }
      continue;
    }
    if (quote == '"') {
      if (char == r'\') {
        escaping = true;
        continue;
      }
      if (char == '"') {
        quote = null;
      }
      continue;
    }
    if (char == r'\') {
      escaping = true;
      continue;
    }
    if (char == "'" || char == '"') {
      quote = char;
      continue;
    }
    if (char.trim().isEmpty) {
      tokenStart = index + 1;
    }
  }
  return tokenStart;
}

String? _resolveLocalCompletionCandidatePath(
  String directoryPath,
  String completion,
) {
  if (completion.isEmpty) {
    return null;
  }
  if (completion.startsWith('/')) {
    return completion;
  }
  final separator = io.Platform.pathSeparator;
  if (directoryPath.endsWith(separator)) {
    return '$directoryPath$completion';
  }
  return '$directoryPath$separator$completion';
}

void _sshDirectoryEntryListingIsolateMain(List<Object?> message) {
  final sendPort = message[0] as SendPort;
  final arguments = (message[1] as Map).cast<String, Object?>();
  try {
    final result = _runSshDirectoryEntryListing(arguments);
    sendPort.send({
      'directory': result.resolvedDirectory,
      'entries': [
        for (final entry in result.entries)
          {
            'name': entry.name,
            'is_directory': entry.isDirectory,
            'size': entry.size,
            'modified': entry.modified == null
                ? null
                : entry.modified!.millisecondsSinceEpoch ~/ 1000,
          },
      ],
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
      'error': result.error,
    });
  } on Object catch (error) {
    sendPort.send({
      'entries': const <Map<String, Object?>>[],
      'error': 'SSH completion failed: $error',
    });
  }
}

void _sftpDirectoryEntryListingIsolateMain(List<Object?> message) {
  final sendPort = message[0] as SendPort;
  final arguments = (message[1] as Map).cast<String, Object?>();
  try {
    final result = _runSftpDirectoryEntryListing(arguments);
    sendPort.send({
      'directory': result.resolvedDirectory,
      'entries': [
        for (final entry in result.entries)
          {
            'name': entry.name,
            'is_directory': entry.isDirectory,
            'size': entry.size,
            'modified': entry.modified == null
                ? null
                : entry.modified!.millisecondsSinceEpoch ~/ 1000,
          },
      ],
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
      'error': result.error,
    });
  } on Object catch (error) {
    sendPort.send({
      'entries': const <Map<String, Object?>>[],
      'error': 'SFTP listing failed: $error',
      'events': const [],
    });
  }
}

String _terminalConnectionEventKindToWire(TerminalConnectionEventKind kind) {
  return switch (kind) {
    TerminalConnectionEventKind.connectStart => 'connect_start',
    TerminalConnectionEventKind.knownHostCheck => 'known_host_check',
    TerminalConnectionEventKind.knownHostVerified => 'known_host_verified',
    TerminalConnectionEventKind.knownHostStoreMissing =>
      'known_host_store_missing',
    TerminalConnectionEventKind.hostKeyUnknown => 'host_key_unknown',
    TerminalConnectionEventKind.hostKeyAccepted => 'host_key_accepted',
    TerminalConnectionEventKind.hostKeyAcceptedForSession =>
      'host_key_accepted_for_session',
    TerminalConnectionEventKind.hostKeyChanged => 'host_key_changed',
    TerminalConnectionEventKind.hostKeyRejected => 'host_key_rejected',
    TerminalConnectionEventKind.hostKeySaveFailed => 'host_key_save_failed',
    TerminalConnectionEventKind.authNoneStart => 'auth_none_start',
    TerminalConnectionEventKind.authNoneRejected => 'auth_none_rejected',
    TerminalConnectionEventKind.authNoneFailed => 'auth_none_failed',
    TerminalConnectionEventKind.authPasswordStart => 'auth_password_start',
    TerminalConnectionEventKind.authPasswordRejected =>
      'auth_password_rejected',
    TerminalConnectionEventKind.authPasswordFailed => 'auth_password_failed',
    TerminalConnectionEventKind.authKeyStart => 'auth_key_start',
    TerminalConnectionEventKind.authKeyRejected => 'auth_key_rejected',
    TerminalConnectionEventKind.authKeyFailed => 'auth_key_failed',
    TerminalConnectionEventKind.authPassphraseRequired =>
      'auth_passphrase_required',
    TerminalConnectionEventKind.authAgentStart => 'auth_agent_start',
    TerminalConnectionEventKind.authAgentIdentities => 'auth_agent_identities',
    TerminalConnectionEventKind.authAgentIdentityStart =>
      'auth_agent_identity_start',
    TerminalConnectionEventKind.authAgentIdentityRejected =>
      'auth_agent_identity_rejected',
    TerminalConnectionEventKind.authAgentUnavailable =>
      'auth_agent_unavailable',
    TerminalConnectionEventKind.authAgentFailed => 'auth_agent_failed',
    TerminalConnectionEventKind.authSuccess => 'auth_success',
    TerminalConnectionEventKind.authFailed => 'auth_failed',
    TerminalConnectionEventKind.connected => 'connected',
    TerminalConnectionEventKind.moshEchoEnabled => 'mosh_echo_enabled',
    TerminalConnectionEventKind.moshEchoDisabled => 'mosh_echo_disabled',
    TerminalConnectionEventKind.moshPredictionConfirmed =>
      'mosh_prediction_confirmed',
    TerminalConnectionEventKind.moshInputStateQueued =>
      'mosh_input_state_queued',
    TerminalConnectionEventKind.moshScreenCommitted => 'mosh_screen_committed',
    TerminalConnectionEventKind.moshLatencyUpdated => 'mosh_latency_updated',
    TerminalConnectionEventKind.sshLatencyUpdated => 'ssh_latency_updated',
    TerminalConnectionEventKind.moshUdpPeerConfirmed =>
      'mosh_udp_peer_confirmed',
    TerminalConnectionEventKind.moshNetworkSwitching =>
      'mosh_network_switching',
    TerminalConnectionEventKind.moshNetworkDegraded => 'mosh_network_degraded',
    TerminalConnectionEventKind.moshNetworkRestored => 'mosh_network_restored',
    TerminalConnectionEventKind.retry => 'retry',
    TerminalConnectionEventKind.exitStatus => 'exit_status',
    TerminalConnectionEventKind.sessionClosed => 'session_closed',
    TerminalConnectionEventKind.connectionLost => 'connection_lost',
    TerminalConnectionEventKind.error => 'error',
    TerminalConnectionEventKind.unknown => 'unknown',
  };
}

void _hostOsDetectionIsolateMain(List<Object?> message) {
  final sendPort = message[0] as SendPort;
  final arguments = (message[1] as Map).cast<String, Object?>();
  try {
    final result = _runHostOsDetection(arguments);
    sendPort.send({
      'os': result.os,
      'distro': result.distro,
      'error': result.error,
      'events': const [],
    });
  } on Object catch (error) {
    sendPort.send({
      'os': null,
      'distro': null,
      'error': '$error',
      'events': const [],
    });
  }
}

void _hostSystemInfoIsolateMain(List<Object?> message) {
  final sendPort = message[0] as SendPort;
  final arguments = (message[1] as Map).cast<String, Object?>();
  try {
    sendPort.send(_runHostSystemInfo(arguments).toJson());
  } on Object catch (error) {
    sendPort.send({'error': '$error', 'events': const []});
  }
}

void _sshPublicKeyExportIsolateMain(List<Object?> message) {
  final sendPort = message[0] as SendPort;
  final arguments = (message[1] as Map).cast<String, Object?>();
  try {
    sendPort.send(_runSshPublicKeyExport(arguments).toJson());
  } on Object catch (error) {
    sendPort.send({'ok': false, 'error': '$error', 'events': const []});
  }
}

void _sshDirectoryListingIsolateMain(List<Object?> message) {
  final sendPort = message[0] as SendPort;
  final arguments = (message[1] as Map).cast<String, Object?>();
  try {
    final result = _runSshDirectoryListing(arguments);
    sendPort.send({
      'directory': result.resolvedDirectory,
      'entries': result.entries,
      'error': result.error,
    });
  } on Object catch (error) {
    sendPort.send({
      'entries': const <String>[],
      'error': 'SSH completion failed: $error',
    });
  }
}
