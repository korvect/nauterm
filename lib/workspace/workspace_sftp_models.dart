part of 'nauterm_workspace.dart';

class _SftpConnectRequest {
  const _SftpConnectRequest({
    required this.id,
    required this.host,
    required this.auth,
  });

  final int id;
  final _HostItem host;
  final _SftpRemoteAuth auth;
}

enum _SftpConnectionPhase { connecting, hostKey, connected, failed }

class _SftpConnectionState {
  const _SftpConnectionState({
    required this.host,
    required this.phase,
    this.auth,
    this.fingerprint,
    this.message,
  });

  factory _SftpConnectionState.connecting(
    _HostItem host, {
    _SftpRemoteAuth? auth,
  }) {
    return _SftpConnectionState(
      host: host,
      phase: _SftpConnectionPhase.connecting,
      auth: auth,
      message:
          'Connecting to ${_sftpConnectionTarget(host)} and opening SFTP...',
    );
  }

  factory _SftpConnectionState.connected(_HostItem host, _SftpRemoteAuth auth) {
    return _SftpConnectionState(
      host: host,
      phase: _SftpConnectionPhase.connected,
      auth: auth,
    );
  }

  factory _SftpConnectionState.hostKey(
    _HostItem host,
    String message, {
    required _SftpRemoteAuth auth,
    String? fingerprint,
  }) {
    return _SftpConnectionState(
      host: host,
      phase: _SftpConnectionPhase.hostKey,
      auth: auth,
      fingerprint: fingerprint,
      message: message,
    );
  }

  factory _SftpConnectionState.failed(
    _HostItem host,
    String message, {
    _SftpRemoteAuth? auth,
  }) {
    return _SftpConnectionState(
      host: host,
      phase: _SftpConnectionPhase.failed,
      auth: auth,
      message: message,
    );
  }

  final _HostItem host;
  final _SftpConnectionPhase phase;
  final _SftpRemoteAuth? auth;
  final String? fingerprint;
  final String? message;
}

class _SftpRemoteAuth {
  const _SftpRemoteAuth({
    required this.host,
    required this.port,
    required this.username,
    required this.knownHostsPath,
    this.password,
    this.privateKey,
    this.certificate,
    this.passphrase,
    this.proxy,
    this.hostKeyTrustMode = SshHostKeyTrustMode.strict,
  });

  final String host;
  final int port;
  final String username;
  final String knownHostsPath;
  final String? password;
  final String? privateKey;
  final String? certificate;
  final String? passphrase;
  final TerminalProxyConfig? proxy;
  final SshHostKeyTrustMode hostKeyTrustMode;

  _SftpRemoteAuth copyWith({SshHostKeyTrustMode? hostKeyTrustMode}) {
    return _SftpRemoteAuth(
      host: host,
      port: port,
      username: username,
      knownHostsPath: knownHostsPath,
      password: password,
      privateKey: privateKey,
      certificate: certificate,
      passphrase: passphrase,
      proxy: proxy,
      hostKeyTrustMode: hostKeyTrustMode ?? this.hostKeyTrustMode,
    );
  }

  Map<String, Object?> toArguments(String directory) {
    return <String, Object?>{
      'host': host,
      'port': port,
      'username': username,
      'knownHostsPath': knownHostsPath,
      'directory': directory,
      'password': password,
      'privateKey': privateKey,
      'certificate': certificate,
      'passphrase': passphrase,
      'proxy': proxy?.toJson(),
      'hostKeyTrustMode': hostKeyTrustMode.wireValue,
    };
  }
}

class _SftpRemoteListingException implements Exception {
  const _SftpRemoteListingException(this.message, this.events);

  final String message;
  final List<TerminalConnectionEvent> events;

  @override
  String toString() => message;
}

class _SftpRemoteListing {
  const _SftpRemoteListing({required this.path, required this.entries});

  final String path;
  final List<_SftpFileEntry> entries;
}

class _SftpFileEntry {
  const _SftpFileEntry({
    required this.path,
    required this.name,
    required this.modified,
    required this.size,
    required this.kind,
    required this.permissions,
    required this.isDirectory,
    required this.isParent,
  });

  factory _SftpFileEntry.parent({
    required String path,
    required String separator,
  }) {
    return _SftpFileEntry(
      path: path,
      name: '..',
      modified: DateTime.fromMillisecondsSinceEpoch(0),
      size: 0,
      kind: 'folder',
      permissions: 'drwxr-xr-x@',
      isDirectory: true,
      isParent: true,
    );
  }

  final String path;
  final String name;
  final DateTime modified;
  final int size;
  final String kind;
  final String permissions;
  final bool isDirectory;
  final bool isParent;
}

class _SftpBreadcrumbPart {
  const _SftpBreadcrumbPart({
    required this.label,
    required this.path,
    this.isOverflow = false,
    this.isDrive = false,
  });

  final String label;
  final String path;
  final bool isOverflow;
  final bool isDrive;
}

enum _SftpSortColumn { name, modified, size, kind }

enum _SftpPaneEndpoint { local, remote }

enum _SftpPaneSlot { left, right }

enum _SftpAction {
  open,
  openWith,
  openWithSshEditor,
  openWithExternalEditor,
  openWithOtherApplication,
  copyToTarget,
  uploadFiles,
  rename,
  moveTo,
  copyTo,
  delete,
  refresh,
  newFolder,
  showHiddenFiles,
  withSudo,
  sudoOpen,
  sudoOpenWith,
  sudoOpenWithSshEditor,
  sudoDownload,
  sudoUpload,
  sudoRename,
  sudoMoveTo,
  sudoCopyTo,
  sudoDelete,
  sudoNewFolder,
  selectAll,
  close,
}

class _SftpMenuEntry {
  const _SftpMenuEntry({
    required this.action,
    required this.label,
    this.enabled = true,
    this.destructive = false,
    this.children = const [],
  });

  final _SftpAction action;
  final String label;
  final bool enabled;
  final bool destructive;
  final List<_SftpMenuEntry> children;
}

List<_SftpMenuEntry> _sftpActionMenuEntries({
  required bool hasSelection,
  required bool showHiddenFiles,
  required bool remote,
  required bool showCloseAction,
  bool sshEditorAvailable = false,
  bool selectionIsFile = false,
  bool selectionCanOpenWithSystemDefault = true,
}) {
  if (remote) {
    return [
      if (!selectionIsFile || selectionCanOpenWithSystemDefault)
        _sftpMenuItem(_SftpAction.open, 'Open', enabled: hasSelection),
      if (sshEditorAvailable)
        _sftpMenuItem(
          _SftpAction.openWithSshEditor,
          'Open with SSH Editor',
          enabled: hasSelection,
        ),
      _sftpMenuItem(
        _SftpAction.copyToTarget,
        'sftp.action.download',
        enabled: hasSelection,
      ),
      _sftpMenuItem(_SftpAction.uploadFiles, 'sftp.action.upload'),
      _sftpMenuItem(_SftpAction.rename, 'Rename', enabled: hasSelection),
      _sftpMenuItem(
        _SftpAction.moveTo,
        'sftp.action.moveTo',
        enabled: hasSelection,
      ),
      _sftpMenuItem(
        _SftpAction.copyTo,
        'sftp.action.copyTo',
        enabled: hasSelection,
      ),
      _sftpMenuItem(
        _SftpAction.delete,
        'Delete',
        enabled: hasSelection,
        destructive: true,
      ),
      _sftpMenuItem(_SftpAction.refresh, 'Refresh'),
      _sftpMenuItem(_SftpAction.newFolder, 'sftp.action.newFolder'),
      _sudoSftpMenu(
        children: [
          _sftpMenuItem(_SftpAction.sudoOpen, 'Open', enabled: hasSelection),
          if (sshEditorAvailable)
            _sftpMenuItem(
              _SftpAction.sudoOpenWithSshEditor,
              'Open with SSH Editor',
              enabled: hasSelection,
            ),
          _sftpMenuItem(
            _SftpAction.sudoDownload,
            'sftp.action.download',
            enabled: hasSelection,
          ),
          _sftpMenuItem(_SftpAction.sudoUpload, 'sftp.action.upload'),
          _sftpMenuItem(
            _SftpAction.sudoRename,
            'Rename',
            enabled: hasSelection,
          ),
          _sftpMenuItem(
            _SftpAction.sudoMoveTo,
            'sftp.action.moveTo',
            enabled: hasSelection,
          ),
          _sftpMenuItem(
            _SftpAction.sudoCopyTo,
            'sftp.action.copyTo',
            enabled: hasSelection,
          ),
          _sftpMenuItem(
            _SftpAction.sudoDelete,
            'Delete',
            enabled: hasSelection,
            destructive: true,
          ),
          _sftpMenuItem(_SftpAction.sudoNewFolder, 'sftp.action.newFolder'),
        ],
      ),
      _sftpMenuItem(
        _SftpAction.showHiddenFiles,
        showHiddenFiles ? 'Hide Hidden Files' : 'Show Hidden Files',
      ),
      _sftpMenuItem(_SftpAction.selectAll, 'Select All'),
      if (showCloseAction)
        _sftpMenuItem(_SftpAction.close, 'Close', destructive: true),
    ];
  }

  return [
    if (!selectionIsFile || selectionCanOpenWithSystemDefault)
      _sftpMenuItem(_SftpAction.open, 'Open', enabled: hasSelection),
    _sftpMenuItem(_SftpAction.refresh, 'Refresh'),
    _sftpMenuItem(_SftpAction.newFolder, 'sftp.action.newFolder'),
    _sftpMenuItem(
      _SftpAction.showHiddenFiles,
      showHiddenFiles ? 'Hide Hidden Files' : 'Show Hidden Files',
    ),
    _sftpMenuItem(_SftpAction.selectAll, 'Select All'),
    if (showCloseAction)
      _sftpMenuItem(_SftpAction.close, 'Close', destructive: true),
  ];
}

List<_SftpMenuEntry> _sftpRowMenuEntries(
  _SftpFileEntry entry, {
  required bool remote,
  bool sshEditorAvailable = false,
}) {
  final canOpenFile = !entry.isParent && !entry.isDirectory;
  final canOpenWithSystemDefault =
      canOpenFile &&
      canOpenFileWithSystemDefaultApplication(
        entry.name,
        permissions: entry.permissions,
      );
  if (remote) {
    return [
      if (!canOpenFile || canOpenWithSystemDefault)
        _sftpMenuItem(_SftpAction.open, 'Open', enabled: !entry.isParent),
      _sftpMenuItem(_SftpAction.openWith, 'Open With', enabled: canOpenFile),
      if (sshEditorAvailable)
        _sftpMenuItem(
          _SftpAction.openWithSshEditor,
          'Open with SSH Editor',
          enabled: canOpenFile,
        ),
      _sftpMenuItem(
        _SftpAction.copyToTarget,
        'sftp.action.download',
        enabled: !entry.isParent,
      ),
      _sftpMenuItem(_SftpAction.uploadFiles, 'sftp.action.upload'),
      _sftpMenuItem(_SftpAction.rename, 'Rename', enabled: !entry.isParent),
      _sftpMenuItem(
        _SftpAction.moveTo,
        'sftp.action.moveTo',
        enabled: !entry.isParent,
      ),
      _sftpMenuItem(
        _SftpAction.copyTo,
        'sftp.action.copyTo',
        enabled: !entry.isParent,
      ),
      _sftpMenuItem(
        _SftpAction.delete,
        'Delete',
        enabled: !entry.isParent,
        destructive: true,
      ),
      _sftpMenuItem(_SftpAction.refresh, 'Refresh'),
      _sftpMenuItem(_SftpAction.newFolder, 'sftp.action.newFolder'),
      _sudoSftpMenu(
        children: [
          _sftpMenuItem(_SftpAction.sudoOpen, 'Open'),
          _sftpMenuItem(
            _SftpAction.sudoOpenWith,
            'Open With',
            enabled: canOpenFile,
          ),
          if (sshEditorAvailable)
            _sftpMenuItem(
              _SftpAction.sudoOpenWithSshEditor,
              'Open with SSH Editor',
              enabled: canOpenFile,
            ),
          _sftpMenuItem(_SftpAction.sudoDownload, 'sftp.action.download'),
          _sftpMenuItem(_SftpAction.sudoUpload, 'sftp.action.upload'),
          _sftpMenuItem(_SftpAction.sudoRename, 'Rename'),
          _sftpMenuItem(_SftpAction.sudoMoveTo, 'sftp.action.moveTo'),
          _sftpMenuItem(_SftpAction.sudoCopyTo, 'sftp.action.copyTo'),
          _sftpMenuItem(_SftpAction.sudoDelete, 'Delete', destructive: true),
          _sftpMenuItem(_SftpAction.sudoNewFolder, 'sftp.action.newFolder'),
        ],
      ),
    ];
  }

  return [
    if (!canOpenFile || canOpenWithSystemDefault)
      _sftpMenuItem(_SftpAction.open, 'Open', enabled: !entry.isParent),
    _sftpMenuItem(_SftpAction.openWith, 'Open With', enabled: canOpenFile),
    _sftpMenuItem(_SftpAction.refresh, 'Refresh'),
    _sftpMenuItem(_SftpAction.newFolder, 'sftp.action.newFolder'),
  ];
}

_SftpMenuEntry _sftpMenuItem(
  _SftpAction action,
  String label, {
  bool enabled = true,
  bool destructive = false,
  List<_SftpMenuEntry> children = const [],
}) {
  return _SftpMenuEntry(
    action: action,
    label: label,
    enabled: enabled,
    destructive: destructive,
    children: children,
  );
}

_SftpMenuEntry _sudoSftpMenu({required List<_SftpMenuEntry> children}) {
  return _sftpMenuItem(
    _SftpAction.withSudo,
    'sftp.action.withSudo',
    destructive: true,
    children: [
      for (final child in children)
        _SftpMenuEntry(
          action: child.action,
          label: child.label,
          enabled: child.enabled,
          destructive: true,
          children: child.children,
        ),
    ],
  );
}

class _SftpMenuCommand {
  const _SftpMenuCommand(
    this.action, {
    this.application,
    this.withSudo = false,
  });

  final _SftpAction action;
  final SftpExternalEditorCommand? application;
  final bool withSudo;
}

List<NautermContextMenuEntry<_SftpAction>> _sftpContextMenuEntries(
  Iterable<_SftpMenuEntry> entries,
) {
  return [
    for (final entry in entries)
      NautermContextMenuAction<_SftpAction>(
        value: entry.action,
        label: entry.label,
        enabled: entry.enabled,
        destructive: entry.destructive,
        children: _sftpContextMenuEntries(entry.children),
      ),
  ];
}

List<NautermContextMenuEntry<_SftpMenuCommand>> _sftpFileContextMenuEntries(
  Iterable<_SftpMenuEntry> entries, {
  required String fileName,
  required List<SystemFileApplication> applications,
  required bool sshEditorAvailable,
}) {
  final defaultApplication = applications
      .where((application) => application.isDefault)
      .firstOrNull;
  final configuredEditor = sftpExternalEditorSupportsFileName(fileName)
      ? sftpExternalEditor
      : null;
  final configuredEditorIcon = configuredEditor == null
      ? null
      : applications
            .where(
              (application) =>
                  application.command.id == configuredEditor.id ||
                  application.name.trim().toLowerCase() ==
                      configuredEditor.label.trim().toLowerCase(),
            )
            .firstOrNull
            ?.iconBytes;
  final recommendedApplications = applications
      .where(
        (application) =>
            !application.isDefault &&
            (configuredEditor == null ||
                (application.command.id != configuredEditor.id &&
                    application.name.trim().toLowerCase() !=
                        configuredEditor.label.trim().toLowerCase())),
      )
      .toList(growable: false);
  final canOpenWithSystemDefault = entries.any(
    (entry) => entry.action == _SftpAction.open && entry.enabled,
  );
  final canChooseOtherApplication = io.Platform.isMacOS;
  final hasRecommendedApplication = recommendedApplications.isNotEmpty;
  final hasConfiguredEditor = configuredEditor != null || sshEditorAvailable;
  final hasOpenWithAction =
      canOpenWithSystemDefault ||
      hasRecommendedApplication ||
      hasConfiguredEditor ||
      canChooseOtherApplication;
  List<NautermContextMenuEntry<_SftpMenuCommand>> openWithEntries(
    bool withSudo,
  ) {
    return [
      if (canOpenWithSystemDefault)
        NautermContextMenuAction<_SftpMenuCommand>(
          value: _SftpMenuCommand(_SftpAction.open, withSudo: withSudo),
          label: defaultApplication == null
              ? 'System Default Application'
              : '${defaultApplication.name} (default)',
          icon: defaultApplication == null ? LucideIcons.externalLink : null,
          iconBytes: defaultApplication?.iconBytes,
          destructive: withSudo,
        ),
      if (canOpenWithSystemDefault &&
          (hasRecommendedApplication ||
              hasConfiguredEditor ||
              canChooseOtherApplication))
        const NautermContextMenuDivider<_SftpMenuCommand>(),
      if (configuredEditor != null)
        NautermContextMenuAction<_SftpMenuCommand>(
          value: _SftpMenuCommand(
            _SftpAction.openWithExternalEditor,
            application: configuredEditor,
            withSudo: withSudo,
          ),
          label: configuredEditor.label,
          icon: configuredEditorIcon == null ? LucideIcons.appWindow : null,
          iconBytes: configuredEditorIcon,
          destructive: withSudo,
        ),
      if (configuredEditor != null &&
          (hasRecommendedApplication || canChooseOtherApplication))
        const NautermContextMenuDivider<_SftpMenuCommand>(),
      for (final application in recommendedApplications)
        NautermContextMenuAction<_SftpMenuCommand>(
          value: _SftpMenuCommand(
            _SftpAction.openWithExternalEditor,
            application: application.command,
            withSudo: withSudo,
          ),
          label: application.name,
          iconBytes: application.iconBytes,
          destructive: withSudo,
        ),
      if (hasRecommendedApplication && canChooseOtherApplication)
        const NautermContextMenuDivider<_SftpMenuCommand>(),
      if (canChooseOtherApplication)
        NautermContextMenuAction<_SftpMenuCommand>(
          value: _SftpMenuCommand(
            _SftpAction.openWithOtherApplication,
            withSudo: withSudo,
          ),
          label: 'Other...',
          icon: LucideIcons.ellipsis,
          destructive: withSudo,
        ),
    ];
  }

  return [
    for (final entry in entries)
      NautermContextMenuAction<_SftpMenuCommand>(
        value: _SftpMenuCommand(entry.action),
        label: entry.label,
        enabled:
            entry.enabled &&
            (entry.action != _SftpAction.openWith || hasOpenWithAction),
        destructive: entry.destructive,
        children: entry.children.isNotEmpty
            ? [
                for (final child in entry.children)
                  NautermContextMenuAction<_SftpMenuCommand>(
                    value: _SftpMenuCommand(
                      child.action,
                      withSudo: entry.action == _SftpAction.withSudo,
                    ),
                    label: child.label,
                    enabled: child.enabled,
                    destructive: child.destructive,
                    children: child.action == _SftpAction.sudoOpenWith
                        ? openWithEntries(true)
                        : const [],
                  ),
              ]
            : entry.action != _SftpAction.openWith
            ? const []
            : openWithEntries(false),
      ),
  ];
}

String _defaultSftpPath() {
  return _normalizeLocalSftpSeparators(sftpEffectiveLocalDirectory());
}

String _localSftpHomePath() {
  final home =
      io.Platform.environment['HOME'] ?? io.Platform.environment['USERPROFILE'];
  return home == null || home.trim().isEmpty
      ? _defaultSftpPath()
      : _normalizeLocalSftpSeparators(home);
}

String _normalizeSftpPath(String path) {
  final trimmed = _normalizeLocalSftpSeparators(path.trim());
  if (trimmed.isEmpty || trimmed == '~') {
    return _defaultSftpPath();
  }
  if (trimmed.startsWith('~${io.Platform.pathSeparator}')) {
    return _joinSftpPath(_defaultSftpPath(), trimmed.substring(2));
  }
  return io.Directory(trimmed).absolute.path;
}

String _normalizeLocalSftpSeparators(String path) {
  if (!io.Platform.isWindows) {
    return path;
  }
  return path.replaceAll(_remoteSftpSeparator, io.Platform.pathSeparator);
}

List<String> _availableLocalSftpDriveRoots() {
  if (!io.Platform.isWindows) {
    return const [];
  }
  final roots = <String>[];
  for (var code = 65; code <= 90; code++) {
    final root = '${String.fromCharCode(code)}:${io.Platform.pathSeparator}';
    try {
      if (io.Directory(root).existsSync()) {
        roots.add(root);
      }
    } on io.FileSystemException {
      // Unavailable removable and network drives are omitted.
    }
  }
  return roots;
}

String _joinSftpPath(String left, String right) {
  final separator = io.Platform.pathSeparator;
  if (left.endsWith(separator)) {
    return '$left$right';
  }
  return '$left$separator$right';
}

Future<String> _uniqueLocalSftpPath(String directory, String name) {
  return _uniqueLocalSftpPathWithReserved(directory, name, const <String>{});
}

Future<String> _uniqueLocalSftpPathWithReserved(
  String directory,
  String name,
  Set<String> reservedPaths,
) async {
  var candidate = _joinSftpPath(directory, name);
  if (reservedPaths.contains(candidate) ||
      await io.FileSystemEntity.type(candidate) !=
          io.FileSystemEntityType.notFound) {
    final dotIndex = name.lastIndexOf('.');
    final hasExtension = dotIndex > 0 && dotIndex < name.length - 1;
    final stem = hasExtension ? name.substring(0, dotIndex) : name;
    final extension = hasExtension ? name.substring(dotIndex) : '';
    var suffix = 2;
    do {
      candidate = _joinSftpPath(directory, '$stem $suffix$extension');
      suffix += 1;
    } while (reservedPaths.contains(candidate) ||
        await io.FileSystemEntity.type(candidate) !=
            io.FileSystemEntityType.notFound);
  }
  return candidate;
}

Future<io.Directory?> _prepareSftpDownloadsDirectory() async {
  io.Directory? directory;
  try {
    directory = await getDownloadsDirectory();
  } on Object catch (error, stackTrace) {
    NautermLog.warning(
      'sftp',
      'Unable to resolve the system Downloads directory.',
      error: error,
      stackTrace: stackTrace,
    );
  }
  directory ??= io.Directory(fallbackDownloadsDirectory());
  try {
    await directory.create(recursive: true);
    return directory;
  } on Object catch (error, stackTrace) {
    NautermLog.warning(
      'sftp',
      'Unable to prepare the system Downloads directory.',
      error: error,
      stackTrace: stackTrace,
      fields: {'path': directory.path},
    );
    return null;
  }
}

const _remoteSftpSeparator = '/';

String _normalizeRemoteSftpPath(String path, {required String base}) {
  final trimmed = path.trim();
  if (trimmed.isEmpty || trimmed == '~') {
    return '~';
  }
  if (trimmed.startsWith('~/') || trimmed.startsWith('/')) {
    return _collapseRemoteSftpPath(trimmed);
  }
  return _collapseRemoteSftpPath(_joinRemoteSftpPath(base, trimmed));
}

String _joinRemoteSftpPath(String left, String right) {
  final cleanRight = right.trim();
  if (cleanRight.isEmpty) {
    return left;
  }
  if (cleanRight.startsWith('/') || cleanRight.startsWith('~/')) {
    return _collapseRemoteSftpPath(cleanRight);
  }
  if (left == '/' || left.endsWith('/')) {
    return _collapseRemoteSftpPath('$left$cleanRight');
  }
  return _collapseRemoteSftpPath('$left/$cleanRight');
}

String _collapseRemoteSftpPath(String path) {
  final trimmed = path.trim();
  if (trimmed.isEmpty) {
    return '~';
  }
  final homeRelative = trimmed == '~' || trimmed.startsWith('~/');
  final absolute = trimmed.startsWith('/');
  final rawParts = trimmed
      .replaceFirst(RegExp(r'^~/?'), '')
      .split('/')
      .where((part) => part.isNotEmpty)
      .toList();
  final parts = <String>[];
  for (final part in rawParts) {
    if (part == '.') {
      continue;
    }
    if (part == '..') {
      if (parts.isNotEmpty) {
        parts.removeLast();
      }
      continue;
    }
    parts.add(part);
  }
  if (homeRelative) {
    return parts.isEmpty ? '~' : '~/${parts.join('/')}';
  }
  if (absolute) {
    return parts.isEmpty ? '/' : '/${parts.join('/')}';
  }
  return parts.isEmpty ? '~' : parts.join('/');
}

String? _remoteParentPath(String path) {
  final collapsed = _collapseRemoteSftpPath(path);
  if (collapsed == '/' || collapsed == '~') {
    return null;
  }
  final prefix = collapsed.startsWith('~/') ? '~/' : '/';
  final body = collapsed.startsWith('~/')
      ? collapsed.substring(2)
      : collapsed.startsWith('/')
      ? collapsed.substring(1)
      : collapsed;
  final index = body.lastIndexOf('/');
  if (index < 0) {
    return prefix == '~/' ? '~' : '/';
  }
  final parentBody = body.substring(0, index);
  if (parentBody.isEmpty) {
    return prefix == '~/' ? '~' : '/';
  }
  return prefix == '~/' ? '~/$parentBody' : '/$parentBody';
}

String _basename(String path) {
  final separator = io.Platform.pathSeparator;
  var normalized = path;
  while (normalized.length > 1 && normalized.endsWith(separator)) {
    normalized = normalized.substring(0, normalized.length - 1);
  }
  final index = normalized.lastIndexOf(separator);
  return index < 0 ? normalized : normalized.substring(index + 1);
}

List<_SftpBreadcrumbPart> _sftpBreadcrumbParts(
  String path, {
  required bool remote,
}) {
  final separator = remote ? _remoteSftpSeparator : io.Platform.pathSeparator;
  final trimmed = remote
      ? path.trim()
      : _normalizeLocalSftpSeparators(path.trim());
  return remote
      ? _remoteSftpBreadcrumbParts(trimmed)
      : _localSftpBreadcrumbParts(trimmed, separator);
}

List<_SftpBreadcrumbPart> _remoteSftpBreadcrumbParts(String trimmed) {
  if (trimmed.isEmpty) {
    return const [];
  }
  if (trimmed == '~') {
    return const [_SftpBreadcrumbPart(label: '~', path: '~')];
  }
  if (trimmed == _remoteSftpSeparator) {
    return const [
      _SftpBreadcrumbPart(
        label: _remoteSftpSeparator,
        path: _remoteSftpSeparator,
      ),
    ];
  }

  if (trimmed.startsWith('~/')) {
    final parts = trimmed
        .substring(2)
        .split(_remoteSftpSeparator)
        .where((part) => part.trim().isNotEmpty)
        .toList();
    var current = '~';
    return [
      const _SftpBreadcrumbPart(label: '~', path: '~'),
      for (final part in parts)
        _SftpBreadcrumbPart(
          label: part,
          path: current = _joinRemoteSftpPath(current, part),
        ),
    ];
  }

  final absolute = trimmed.startsWith(_remoteSftpSeparator);
  final parts = trimmed
      .split(_remoteSftpSeparator)
      .where((part) => part.trim().isNotEmpty)
      .toList();
  if (parts.isEmpty) {
    return const [
      _SftpBreadcrumbPart(
        label: _remoteSftpSeparator,
        path: _remoteSftpSeparator,
      ),
    ];
  }

  var current = absolute ? _remoteSftpSeparator : '';
  return [
    for (final part in parts)
      _SftpBreadcrumbPart(
        label: part,
        path: current = current.isEmpty
            ? part
            : _joinRemoteSftpPath(current, part),
      ),
  ];
}

List<_SftpBreadcrumbPart> _localSftpBreadcrumbParts(
  String trimmed,
  String separator,
) {
  if (trimmed.isEmpty) {
    return const [];
  }
  if (trimmed == separator) {
    return [_SftpBreadcrumbPart(label: separator, path: separator)];
  }

  final unc = io.Platform.isWindows && trimmed.startsWith(r'\\');
  final absolute = unc || trimmed.startsWith(separator);
  final parts = trimmed
      .split(separator)
      .where((part) => part.trim().isNotEmpty)
      .toList();
  if (parts.isEmpty) {
    return [_SftpBreadcrumbPart(label: separator, path: separator)];
  }

  var current = unc ? r'\\' : (absolute ? separator : '');
  return [
    for (var index = 0; index < parts.length; index++)
      _SftpBreadcrumbPart(
        label: parts[index],
        path: current =
            io.Platform.isWindows &&
                index == 0 &&
                RegExp(r'^[A-Za-z]:$').hasMatch(parts[index])
            ? '${parts[index]}$separator'
            : current.isEmpty
            ? parts[index]
            : _joinSftpPath(current, parts[index]),
        isDrive:
            io.Platform.isWindows &&
            index == 0 &&
            RegExp(r'^[A-Za-z]:$').hasMatch(parts[index]),
      ),
  ];
}

List<_SftpBreadcrumbPart> _visibleSftpBreadcrumbParts(
  List<_SftpBreadcrumbPart> parts,
  double maxWidth,
  TextStyle labelStyle,
  TextDirection textDirection,
) {
  if (parts.isEmpty) {
    return const [];
  }
  if (_sftpBreadcrumbWidth(parts, labelStyle, textDirection) <= maxWidth) {
    return parts;
  }

  const overflow = _SftpBreadcrumbPart(
    label: '...',
    path: '',
    isOverflow: true,
  );
  for (var start = 1; start < parts.length; start++) {
    final candidate = [overflow, ...parts.sublist(start)];
    if (_sftpBreadcrumbWidth(candidate, labelStyle, textDirection) <=
        maxWidth) {
      return candidate;
    }
  }
  return [overflow, parts.last];
}

double _sftpBreadcrumbWidth(
  List<_SftpBreadcrumbPart> parts,
  TextStyle labelStyle,
  TextDirection textDirection,
) {
  var width = 0.0;
  for (var i = 0; i < parts.length; i++) {
    if (i > 0) {
      width += 15;
    }
    width += _sftpBreadcrumbPartWidth(parts[i], labelStyle, textDirection);
  }
  return width;
}

double _sftpBreadcrumbPartWidth(
  _SftpBreadcrumbPart part,
  TextStyle labelStyle,
  TextDirection textDirection,
) {
  final painter = TextPainter(
    text: TextSpan(text: part.label, style: labelStyle),
    textDirection: textDirection,
    maxLines: 1,
  )..layout();
  if (part.isOverflow) {
    return painter.width + 4;
  }
  return painter.width + 23;
}

String _formatSftpModified(DateTime date) {
  final hour = date.hour % 12 == 0 ? 12 : date.hour % 12;
  final minute = date.minute.toString().padLeft(2, '0');
  final period = date.hour >= 12 ? 'PM' : 'AM';
  return '${date.month}/${date.day}/${date.year}, $hour:$minute $period';
}

String _formatSftpSize(int size) {
  if (size < 1024) {
    return '$size B';
  }
  if (size < 1024 * 1024) {
    return '${(size / 1024).toStringAsFixed(1)} KB';
  }
  if (size < 1024 * 1024 * 1024) {
    return '${(size / (1024 * 1024)).toStringAsFixed(1)} MB';
  }
  return '${(size / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
}

String _sftpTaskSubtitle(_SftpTask task, {int? queuePosition}) {
  final status = switch (task.status) {
    _SftpTaskStatus.queued =>
      queuePosition == null ? 'queued' : 'queued #$queuePosition',
    _SftpTaskStatus.running =>
      task.pauseRequested
          ? 'pausing'
          : task.cancelRequested
          ? 'cancelling'
          : 'running',
    _SftpTaskStatus.paused => 'paused',
    _SftpTaskStatus.completed => 'completed',
    _SftpTaskStatus.failed => 'failed',
    _SftpTaskStatus.cancelled => 'cancelled',
  };
  final timestamp = _formatSftpTaskTimestamp(task.createdAt);
  if (task.status == _SftpTaskStatus.failed) {
    return task.error == null || task.error!.trim().isEmpty
        ? '$timestamp  $status'
        : '$timestamp  $status: ${task.error}';
  }
  final total = task.totalBytes;
  if (total > 0) {
    return '$timestamp  $status  ${_formatSftpSize(task.bytes)} / ${_formatSftpSize(total)}';
  }
  final bytes = task.bytes > 0 ? '  ${_formatSftpSize(task.bytes)}' : '';
  return '$timestamp  $status$bytes';
}

String _formatSftpTaskTimestamp(DateTime timestamp) {
  final local = timestamp.toLocal();
  return '${local.year.toString().padLeft(4, '0')}-'
      '${local.month.toString().padLeft(2, '0')}-'
      '${local.day.toString().padLeft(2, '0')} '
      '${local.hour.toString().padLeft(2, '0')}:'
      '${local.minute.toString().padLeft(2, '0')}:'
      '${local.second.toString().padLeft(2, '0')}';
}

String _sftpKindForName(
  String name, {
  required bool isDirectory,
  bool isLink = false,
}) {
  if (isDirectory) {
    return 'folder';
  }
  if (isLink) {
    return 'symlink';
  }

  final lowerName = name.toLowerCase();
  final lastDot = lowerName.lastIndexOf('.');
  if (lastDot <= 0 || lastDot == lowerName.length - 1) {
    return 'file';
  }
  return lowerName.substring(lastDot + 1);
}

List<_SftpFileEntry> _filterSftpEntries(
  List<_SftpFileEntry> entries,
  String query,
) {
  final normalized = query.trim().toLowerCase();
  if (normalized.isEmpty) {
    return entries;
  }
  return [
    for (final entry in entries)
      if (!entry.isParent &&
          (entry.name.toLowerCase().contains(normalized) ||
              entry.kind.toLowerCase().contains(normalized)))
        entry,
  ];
}

List<_SftpFileEntry> _sortSftpEntries(
  List<_SftpFileEntry> entries,
  _SftpSortColumn column, {
  required bool ascending,
}) {
  final sorted = entries.toList();
  sorted.sort((left, right) {
    if (left.isParent != right.isParent) {
      return left.isParent ? -1 : 1;
    }
    if (left.isDirectory != right.isDirectory) {
      return left.isDirectory ? -1 : 1;
    }

    final primary = switch (column) {
      _SftpSortColumn.name => _compareText(left.name, right.name),
      _SftpSortColumn.modified => left.modified.compareTo(right.modified),
      _SftpSortColumn.size => _compareSftpSize(left, right),
      _SftpSortColumn.kind => _compareText(left.kind, right.kind),
    };
    final directed = ascending ? primary : -primary;
    if (directed != 0) {
      return directed;
    }

    final nameTie = _compareText(left.name, right.name);
    return ascending ? nameTie : -nameTie;
  });
  return sorted;
}

int _compareSftpSize(_SftpFileEntry left, _SftpFileEntry right) {
  return left.size.compareTo(right.size);
}

int _compareText(String left, String right) {
  return left.toLowerCase().compareTo(right.toLowerCase());
}

String _sftpConnectionTarget(_HostItem host) {
  final username = host.username?.trim();
  final hostAddress = host.host?.trim();
  final port = host.port ?? 22;
  final destination = hostAddress == null || hostAddress.isEmpty
      ? host.name
      : hostAddress;
  if (username == null || username.isEmpty) {
    return '$destination:$port';
  }
  return '$username@$destination:$port';
}

bool _hasSftpHostKeyEvent(List<TerminalConnectionEvent> events) {
  for (final event in events.reversed) {
    switch (event.kind) {
      case TerminalConnectionEventKind.hostKeyUnknown:
      case TerminalConnectionEventKind.knownHostStoreMissing:
        return true;
      case TerminalConnectionEventKind.hostKeyAccepted:
      case TerminalConnectionEventKind.hostKeyAcceptedForSession:
      case TerminalConnectionEventKind.hostKeyChanged:
      case TerminalConnectionEventKind.hostKeyRejected:
      case TerminalConnectionEventKind.hostKeySaveFailed:
      case TerminalConnectionEventKind.authNoneStart:
      case TerminalConnectionEventKind.authPasswordStart:
      case TerminalConnectionEventKind.authKeyStart:
      case TerminalConnectionEventKind.authAgentStart:
      case TerminalConnectionEventKind.authSuccess:
        return false;
      default:
        break;
    }
  }
  return false;
}

String? _sftpLatestFingerprint(List<TerminalConnectionEvent> events) {
  for (final event in events.reversed) {
    final fingerprint = event.fingerprint;
    if (fingerprint != null && fingerprint.trim().isNotEmpty) {
      return fingerprint;
    }
  }
  return null;
}
