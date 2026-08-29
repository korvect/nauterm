part of 'nauterm_workspace.dart';

const _replayYieldChunkInterval = 8;

extension _NautermWorkspaceEditorActions on _NautermWorkspaceState {
  void _openEditor(_WorkspaceEditorRequest request) {
    _setWorkspaceState(() => _editorRequest = request);
  }

  void _pushEditor(
    _WorkspaceEditorRequest request, {
    ValueChanged<Object>? onSaved,
  }) {
    _setWorkspaceState(() {
      _editorStack.add(
        _WorkspaceEditorStackEntry(request: request, onSaved: onSaved),
      );
    });
  }

  void _closeEditor() {
    _setWorkspaceState(() {
      if (_editorStack.length > 1) {
        _editorStack.removeLast();
      } else {
        _editorRequest = null;
      }
    });
  }

  void _completeEditorSave(Object saved) {
    ValueChanged<Object>? onSaved;
    _setWorkspaceState(() {
      if (_editorStack.length > 1) {
        onSaved = _editorStack.removeLast().onSaved;
      } else {
        _editorRequest = null;
      }
    });
    onSaved?.call(saved);
  }

  void _createGroupForEditor(String initialName, ValueChanged<int> onCreated) {
    _pushEditor(
      _GroupEditorRequest(initialName: initialName),
      onSaved: (saved) {
        if (saved case HostGroup(id: final id?)) {
          onCreated(id);
        }
      },
    );
  }

  void _createGroupFromProtocol(HostGroup template) {
    _pushEditor(_GroupEditorRequest(template: template));
  }

  void _createCredentialForEditor(
    String initialName, {
    required bool certificate,
    required ValueChanged<int> onCreated,
  }) {
    _pushEditor(
      _KeyEditorRequest(
        initialName: initialName,
        certificateMode: certificate,
        credentialCreation: true,
      ),
      onSaved: (saved) {
        if (saved case KeyEntry(id: final id?)) {
          onCreated(id);
        }
      },
    );
  }

  void _createIdentityForEditor(
    String initialName,
    ValueChanged<int> onCreated,
  ) {
    _pushEditor(
      _IdentityEditorRequest(initialName: initialName),
      onSaved: (saved) {
        if (saved case IdentityEntry(id: final id?)) {
          onCreated(id);
        }
      },
    );
  }

  void _createProxyForEditor(String initialName, ValueChanged<int> onCreated) {
    _pushEditor(
      _ProxyEditorRequest(initialName: initialName),
      onSaved: (saved) {
        if (saved case ProxyEntry(id: final id?)) {
          onCreated(id);
        }
      },
    );
  }

  void _createSnippetForEditor(
    String initialName,
    ValueChanged<int> onCreated,
  ) {
    _pushEditor(
      _SnippetEditorRequest(initialScript: initialName),
      onSaved: (saved) {
        if (saved case _SnippetItem(id: final id)) {
          onCreated(id);
        }
      },
    );
  }

  void _editHostEnvironmentForEditor(
    String hostLabel,
    List<HostEnvironmentVariable> variables,
    ValueChanged<List<HostEnvironmentVariable>> onSaved,
  ) {
    _pushEditor(
      _HostEnvironmentEditorRequest(hostLabel: hostLabel, variables: variables),
      onSaved: (saved) {
        if (saved case List<HostEnvironmentVariable> variables) {
          onSaved(variables);
        }
      },
    );
  }

  void _saveHostEnvironmentForEditor(List<HostEnvironmentVariable> variables) {
    _completeEditorSave(List<HostEnvironmentVariable>.unmodifiable(variables));
  }

  void _createGroup([int? parentId]) {
    if (parentId != null && !_canUseGroupAsParent(_groupEntries, parentId)) {
      _showWorkspaceMessage('Groups can only be nested two levels deep.');
      return;
    }
    _openEditor(_GroupEditorRequest(initialParentId: parentId));
  }

  void _editGroup(_GroupItem item) {
    final existing = _groupEntries
        .where((group) => group.id == item.id)
        .firstOrNull;
    if (existing == null) {
      return;
    }
    _openEditor(_GroupEditorRequest(initial: existing));
  }

  Future<void> _saveGroup(HostGroup group) async {
    final parentId = group.parentId;
    if (parentId != null && !_canUseGroupAsParent(_groupEntries, parentId)) {
      _showWorkspaceMessage('Groups can only be nested two levels deep.');
      return;
    }
    final store = _dataStore;
    if (store == null) {
      _showWorkspaceMessage('Database is not ready.');
      return;
    }
    final id = store.saveGroup(group);
    if (mounted) {
      final saved =
          store.getGroup(id) ??
          HostGroup(id: id, name: group.name, parentId: group.parentId);
      _setWorkspaceState(() {
        _groupEntries = [
          for (final entry in _groupEntries)
            if (entry.id != id) entry,
          saved,
        ];
        _groups = _mapGroups(_groupEntries, _hostEntries);
      });
      _completeEditorSave(saved);
    }
  }

  void _createHost([int? groupId]) {
    _openEditor(_HostEditorRequest(initialGroupId: groupId));
  }

  Future<void> _importHosts() async {
    final selection = await _showWorkspaceDialog<_HostImportSelection>(
      barrierDismissible: false,
      builder: (context) =>
          _HostImportDialog(onLoadSource: _loadHostImportSource),
    );
    if (selection == null ||
        (selection.hosts.isEmpty && selection.keys.isEmpty) ||
        !mounted) {
      return;
    }

    final store = _dataStore;
    if (store == null) {
      _showWorkspaceMessage('Database is not ready.');
      return;
    }

    var importedHosts = 0;
    var importedKeys = 0;
    var skipped = 0;
    final existingSignatures = <String>{
      for (final host in _hostEntries)
        _hostImportSignature(
          host.host ?? '',
          host.port,
          host.username,
          host.name,
        ),
    };
    final groups = [..._groupEntries];
    final tags = [..._tagEntries];
    final keyIdsByPath = <String, int>{};

    for (final candidate in selection.keys) {
      try {
        final existing = _matchingImportedKey(store, candidate);
        final id =
            existing?.id ??
            store.saveKey(
              KeyEntry(
                name: _uniqueImportedKeyName(candidate.name),
                privateKey: candidate.privateKey,
                publicKey: candidate.publicKey,
              ),
            );
        if (existing == null) importedKeys++;
        for (final path in [candidate.privatePath, candidate.publicPath]) {
          final normalized = _normalizeImportPath(path);
          if (normalized != null) keyIdsByPath[normalized] = id;
        }
      } catch (_) {
        skipped++;
      }
    }

    for (final candidate in selection.hosts) {
      final signature = _hostImportSignature(
        candidate.host,
        candidate.port,
        candidate.username,
        candidate.name,
      );
      if (!existingSignatures.add(signature)) {
        skipped++;
        continue;
      }
      try {
        final groupId = _ensureImportedGroup(
          store,
          groups,
          candidate.groupPath,
        );
        final tagUuids = _ensureImportedTags(store, tags, candidate.tags);
        final identityPath = _normalizeImportPath(candidate.identityFile);
        final keyId = identityPath == null ? null : keyIdsByPath[identityPath];
        final protocol = candidate.protocol.toLowerCase();
        store.saveHost(
          HostEntry(
            name: candidate.name,
            groupId: groupId,
            host: candidate.host,
            port: candidate.port,
            username: protocol == 'telnet' ? null : candidate.username,
            password: protocol == 'telnet' ? null : candidate.password,
            type: NautermHostType.remote,
            keyId: keyId,
            moshEnabled: protocol == 'mosh',
            telnetEnabled: protocol == 'telnet',
            telnetUsername: protocol == 'telnet' ? candidate.username : null,
            telnetPassword: protocol == 'telnet' ? candidate.password : null,
            telnetPort: protocol == 'telnet' ? candidate.port : null,
            tagUuids: tagUuids,
          ),
        );
        importedHosts++;
      } catch (_) {
        skipped++;
      }
    }

    await _loadWorkspaceData();
    if (!mounted) return;
    final importedParts = [
      if (importedHosts > 0)
        '$importedHosts ${importedHosts == 1 ? 'host' : 'hosts'}',
      if (importedKeys > 0)
        '$importedKeys ${importedKeys == 1 ? 'key' : 'keys'}',
    ];
    final suffix = skipped == 0 ? '' : ' $skipped skipped.';
    _showWorkspaceMessage(
      'Imported ${importedParts.isEmpty ? '0 items' : importedParts.join(' and ')}.$suffix',
    );
  }

  Future<void> _exportHosts() async {
    final store = _dataStore;
    if (store == null) {
      _showWorkspaceMessage('Database is not ready.');
      return;
    }
    final hosts = store.listHosts();
    if (hosts.isEmpty) {
      _showWorkspaceMessage('No hosts to export.');
      return;
    }
    final groups = store.listGroups();
    final tags = store.listTags();
    final csv = buildHostCsv(hosts, groups, tags);
    final location = await getSaveLocation(
      acceptedTypeGroups: [
        XTypeGroup(label: 'CSV file', extensions: ['csv']),
      ],
      initialDirectory: await _terminalLogExportInitialDirectory(),
      suggestedName: 'nauterm-hosts',
    );
    if (location == null || location.path.trim().isEmpty) return;
    final path = location.path.toLowerCase().endsWith('.csv')
        ? location.path
        : '${location.path}.csv';
    try {
      await io.File(path).writeAsString(csv, encoding: utf8, flush: true);
      if (mounted) {
        _showWorkspaceMessage('Exported ${hosts.length} hosts to CSV.');
      }
    } on Object catch (error) {
      if (mounted) {
        _showWorkspaceMessage('Failed to export hosts: $error');
      }
    }
  }

  Future<HostImportBundle?> _loadHostImportSource(
    HostImportSource source,
  ) async {
    switch (source) {
      case HostImportSource.csv:
        final file = await _pickHostImportFile(
          label: 'Import hosts from CSV',
          extensions: const ['csv'],
        );
        return file == null
            ? null
            : HostImportBundle(
                hosts: parseHostCsv(await _readImportText(file)),
              );
      case HostImportSource.openSsh:
        return _loadOpenSshDirectory();
      case HostImportSource.putty:
        final file = await _pickHostImportFile(
          label: 'Import PuTTY sessions',
          extensions: const ['reg'],
        );
        return file == null
            ? null
            : HostImportBundle(
                hosts: parsePuttyRegistry(await _readImportText(file)),
              );
      case HostImportSource.mobaXterm:
        final file = await _pickHostImportFile(
          label: 'Import MobaXterm sessions',
          extensions: const ['mxtsessions', 'ini'],
        );
        return file == null
            ? null
            : HostImportBundle(
                hosts: parseMobaXtermSessions(await _readImportText(file)),
              );
      case HostImportSource.secureCrt:
        return _loadSecureCrtHosts();
    }
  }

  Future<io.File?> _pickHostImportFile({
    required String label,
    required List<String> extensions,
  }) {
    return _runExclusiveFilePicker(() async {
      final file = await openFile(
        acceptedTypeGroups: [XTypeGroup(label: label, extensions: extensions)],
      );
      return file == null ? null : io.File(file.path);
    });
  }

  Future<HostImportBundle?> _loadOpenSshDirectory() async {
    var directory = _defaultSshDirectory();
    if (directory == null) {
      final path = await _runExclusiveFilePicker(getDirectoryPath);
      if (path == null) return null;
      directory = io.Directory(path);
    }

    final config = io.File(
      '${directory.path}${io.Platform.pathSeparator}config',
    );
    final configHosts = await config.exists()
        ? parseOpenSshConfig(await _readImportText(config))
        : <HostImportCandidate>[];
    final knownHostsFile = io.File(
      '${directory.path}${io.Platform.pathSeparator}known_hosts',
    );
    final knownHosts = await knownHostsFile.exists()
        ? parseOpenSshKnownHosts(await _readImportText(knownHostsFile))
        : <HostImportCandidate>[];
    final hostEndpoints = <String>{
      for (final host in configHosts)
        '${host.host.toLowerCase()}|${host.port ?? 22}',
    };
    final hosts = [
      ...configHosts,
      for (final host in knownHosts)
        if (hostEndpoints.add('${host.host.toLowerCase()}|${host.port ?? 22}'))
          host,
    ];
    final keyFiles = <OpenSshImportFile>[];
    await for (final entity in directory.list(followLinks: false)) {
      if (entity is! io.File) continue;
      final name = entity.path.split(io.Platform.pathSeparator).last;
      if (_ignoredOpenSshImportFile(name)) continue;
      try {
        final stat = await entity.stat();
        if (stat.size > 1024 * 1024) continue;
        final contents = await _readImportText(entity);
        if (isOpenSshPrivateKey(contents) || isOpenSshPublicKey(contents)) {
          keyFiles.add(
            OpenSshImportFile(path: entity.path, contents: contents),
          );
        }
      } catch (_) {
        // A single unreadable file should not block the rest of ~/.ssh.
      }
    }
    return HostImportBundle(
      hosts: _deduplicateImportCandidates(hosts),
      keys: collectOpenSshKeys(keyFiles),
    );
  }

  Future<HostImportBundle?> _loadSecureCrtHosts() async {
    final path = await _runExclusiveFilePicker(getDirectoryPath);
    if (path == null) return null;
    final result = <HostImportCandidate>[];
    await for (final entity in io.Directory(path).list(recursive: true)) {
      if (entity is! io.File || !entity.path.toLowerCase().endsWith('.ini')) {
        continue;
      }
      final separator = io.Platform.pathSeparator;
      final relative = entity.path.startsWith('$path$separator')
          ? entity.path.substring(path.length + 1)
          : entity.path;
      final withoutExtension = relative.replaceFirst(
        RegExp(r'\.ini$', caseSensitive: false),
        '',
      );
      final parts = withoutExtension.split(separator);
      final candidate = parseSecureCrtSession(
        await _readImportText(entity),
        name: parts.last,
      );
      if (candidate == null) continue;
      result.add(
        HostImportCandidate(
          source: candidate.source,
          name: candidate.name,
          host: candidate.host,
          protocol: candidate.protocol,
          port: candidate.port,
          username: candidate.username,
          identityFile: candidate.identityFile,
          groupPath: parts.length > 1
              ? parts.take(parts.length - 1).join('/')
              : null,
        ),
      );
    }
    return HostImportBundle(hosts: _deduplicateImportCandidates(result));
  }

  Future<String> _readImportText(io.File file) async {
    final bytes = await file.readAsBytes();
    if (bytes.length >= 2 && bytes[0] == 0xff && bytes[1] == 0xfe) {
      final units = <int>[];
      for (var index = 2; index + 1 < bytes.length; index += 2) {
        units.add(bytes[index] | (bytes[index + 1] << 8));
      }
      return String.fromCharCodes(units);
    }
    if (bytes.length >= 2 && bytes[0] == 0xfe && bytes[1] == 0xff) {
      final units = <int>[];
      for (var index = 2; index + 1 < bytes.length; index += 2) {
        units.add((bytes[index] << 8) | bytes[index + 1]);
      }
      return String.fromCharCodes(units);
    }
    return utf8.decode(bytes, allowMalformed: true).replaceFirst('\ufeff', '');
  }

  List<HostImportCandidate> _deduplicateImportCandidates(
    List<HostImportCandidate> candidates,
  ) {
    final seen = <String>{};
    return [
      for (final candidate in candidates)
        if (seen.add(
          _hostImportSignature(
            candidate.host,
            candidate.port,
            candidate.username,
            candidate.name,
          ),
        ))
          candidate,
    ];
  }

  String _hostImportSignature(
    String host,
    int? port,
    String? username,
    String name,
  ) =>
      '${host.toLowerCase()}|${port ?? 22}|${username ?? ''}|${name.toLowerCase()}';

  int? _ensureImportedGroup(
    NautermDataStore store,
    List<HostGroup> groups,
    String? rawPath,
  ) {
    final path = rawPath
        ?.split(RegExp(r'[/\\]'))
        .map((part) => part.trim())
        .where((part) => part.isNotEmpty)
        .toList(growable: false);
    if (path == null || path.isEmpty) return null;
    final names = path.length <= 2
        ? path
        : [path.first, path.skip(1).join(' / ')];
    int? parentId;
    for (final name in names) {
      final existing = groups
          .where(
            (group) =>
                group.parentId == parentId &&
                group.name.toLowerCase() == name.toLowerCase(),
          )
          .firstOrNull;
      if (existing?.id != null) {
        parentId = existing!.id;
        continue;
      }
      final id = store.saveGroup(HostGroup(name: name, parentId: parentId));
      final saved =
          store.getGroup(id) ??
          HostGroup(id: id, name: name, parentId: parentId);
      groups.add(saved);
      parentId = id;
    }
    return parentId;
  }

  List<String> _ensureImportedTags(
    NautermDataStore store,
    List<TagEntry> tags,
    List<String> names,
  ) {
    final uuids = <String>[];
    for (final name in names) {
      var tag = tags
          .where((entry) => entry.name.toLowerCase() == name.toLowerCase())
          .firstOrNull;
      if (tag == null) {
        final id = store.saveTag(TagEntry(name: name));
        tag = store.getTag(id);
        if (tag != null) tags.add(tag);
      }
      if (tag?.uuid case final uuid?) uuids.add(uuid);
    }
    return uuids;
  }

  KeyEntry? _matchingImportedKey(
    NautermDataStore store,
    HostImportKeyCandidate candidate,
  ) {
    for (final summary in _keyEntries) {
      if (summary.name.toLowerCase() != candidate.name.toLowerCase() ||
          summary.id == null) {
        continue;
      }
      final detail = store.getKey(summary.id!);
      if (detail?.privateKey == candidate.privateKey &&
          detail?.publicKey == candidate.publicKey) {
        return detail;
      }
    }
    return null;
  }

  String _uniqueImportedKeyName(String preferred) {
    final names = _dataStore
        ?.listKeys()
        .map((key) => key.name.toLowerCase())
        .toSet();
    if (names == null || !names.contains(preferred.toLowerCase())) {
      return preferred;
    }
    var suffix = 2;
    while (names.contains('$preferred $suffix'.toLowerCase())) {
      suffix++;
    }
    return '$preferred $suffix';
  }

  String? _normalizeImportPath(String? rawPath) {
    if (rawPath == null || rawPath.trim().isEmpty) return null;
    var path = rawPath.trim();
    final home =
        io.Platform.environment[io.Platform.isWindows ? 'USERPROFILE' : 'HOME'];
    if (home != null && path == '~') {
      path = home;
    } else if (home != null &&
        (path.startsWith('~/') || path.startsWith(r'~\'))) {
      path = '$home${io.Platform.pathSeparator}${path.substring(2)}';
    } else if (home != null && !io.File(path).isAbsolute) {
      path =
          '$home${io.Platform.pathSeparator}.ssh${io.Platform.pathSeparator}$path';
    }
    return io.File(path).absolute.path;
  }

  bool _ignoredOpenSshImportFile(String name) {
    return const {
      'config',
      'known_hosts',
      'known_hosts.old',
      'authorized_keys',
      'authorized_keys2',
      '.ds_store',
    }.contains(name.toLowerCase());
  }

  void _editHost(_HostItem item) {
    final existing = _dataStore?.getHost(item.id);
    if (existing == null) {
      _showWorkspaceMessage('Host is not available.');
      return;
    }
    _openEditor(_HostEditorRequest(initial: existing));
  }

  Future<void> _saveHost(HostEntry host) async {
    final store = _dataStore;
    if (store == null) {
      throw StateError('Database is not ready.');
    }
    final id = store.saveHost(host);
    final saved = store.getHost(id);
    if (saved != null && mounted) {
      _upsertHostSummary(saved);
    }
    if (mounted) {
      _closeEditor();
    }
  }

  void _upsertHostSummary(HostEntry detail) {
    final summary = _hostSummary(detail);
    _setWorkspaceState(() {
      _hostEntries = [
        for (final entry in _hostEntries)
          if (entry.id != summary.id) entry,
        summary,
      ];
      _groups = _mapGroups(_groupEntries, _hostEntries);
      _hosts = _mapHosts(
        _hostEntries,
        _groupEntries,
        _identityEntries,
        _tagEntries,
      );
    });
  }

  HostEntry _hostSummary(
    HostEntry host, {
    List<String>? tagUuids,
    bool clearGroup = false,
  }) {
    return HostEntry(
      id: host.id,
      uuid: host.uuid,
      name: host.name,
      groupId: clearGroup ? null : host.groupId,
      groupUuid: clearGroup ? null : host.groupUuid,
      identityId: host.identityId,
      identityUuid: host.identityUuid,
      proxyId: host.proxyId,
      proxyUuid: host.proxyUuid,
      host: host.host,
      port: host.port,
      username: host.username,
      themeId: host.themeId,
      startupSnippetId: host.startupSnippetId,
      startupSnippetUuid: host.startupSnippetUuid,
      sshEnabled: host.sshEnabledOverride,
      moshEnabled: host.moshEnabledOverride,
      moshServerCommand: host.moshServerCommandOverride,
      telnetEnabled: host.telnetEnabledOverride,
      telnetIdentityId: host.telnetIdentityId,
      telnetIdentityUuid: host.telnetIdentityUuid,
      telnetUsername: host.telnetUsername,
      telnetPort: host.telnetPort,
      telnetThemeId: host.telnetThemeId,
      encoding: host.encodingOverride,
      telnetEncoding: host.telnetEncodingOverride,
      type: host.type,
      keyId: host.keyId,
      keyUuid: host.keyUuid,
      shellPath: host.shellPath,
      workDir: host.workDir,
      os: host.os,
      distro: host.distro,
      tagUuids: tagUuids ?? host.tagUuids,
      createdAt: host.createdAt,
      updatedAt: host.updatedAt,
      deletedAt: host.deletedAt,
      version: host.version,
      createdDeviceId: host.createdDeviceId,
      updatedDeviceId: host.updatedDeviceId,
    );
  }

  void _saveTag(TagEntry tag) {
    _persistTag(tag);
  }

  TagEntry? _createTag(String name) {
    return _persistTag(TagEntry(name: name));
  }

  TagEntry? _persistTag(TagEntry tag) {
    final store = _dataStore;
    if (store == null) {
      return null;
    }
    try {
      final id = store.saveTag(tag);
      final saved = store.getTag(id);
      if (saved == null || !mounted) {
        return saved;
      }
      _setWorkspaceState(() {
        _tagEntries = [
          for (final entry in _tagEntries)
            if (entry.id != id) entry,
          saved,
        ]..sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
        _hosts = _mapHosts(
          _hostEntries,
          _groupEntries,
          _identityEntries,
          _tagEntries,
        );
      });
      return saved;
    } catch (error) {
      _showWorkspaceMessage(
        'Unable to save tag: $error',
        type: _WorkspaceNotificationType.error,
      );
      return null;
    }
  }

  void _deleteTag(TagEntry tag) {
    final id = tag.id;
    final uuid = tag.uuid;
    final store = _dataStore;
    if (id == null || store == null) {
      return;
    }
    try {
      store.deleteTag(id);
      if (!mounted) {
        return;
      }
      _setWorkspaceState(() {
        _tagEntries = [
          for (final entry in _tagEntries)
            if (entry.id != id) entry,
        ];
        if (uuid != null) {
          _hostEntries = [
            for (final host in _hostEntries)
              if (host.tagUuids.contains(uuid))
                _hostSummary(
                  host,
                  tagUuids: [
                    for (final value in host.tagUuids)
                      if (value != uuid) value,
                  ],
                )
              else
                host,
          ];
          _hosts = _mapHosts(
            _hostEntries,
            _groupEntries,
            _identityEntries,
            _tagEntries,
          );
        }
      });
    } catch (error) {
      _showWorkspaceMessage(
        'Unable to delete tag: $error',
        type: _WorkspaceNotificationType.error,
      );
    }
  }

  void _createKey() {
    _openEditor(const _KeyEditorRequest());
  }

  void _createCertificate() {
    _openEditor(const _KeyEditorRequest(certificateMode: true));
  }

  void _generateKey() {
    _openEditor(const _KeyEditorRequest(generate: true));
  }

  Future<void> _saveGeneratedKey(KeyEntry key) async {
    final store = _dataStore;
    if (store == null) {
      throw StateError('Database is not ready.');
    }
    final id = store.saveKey(key);
    final saved = store.getKey(id) ?? KeyEntry(id: id, name: key.name);
    if (!mounted || _editorStack.isEmpty) {
      return;
    }
    _setWorkspaceState(() {
      _keyEntries = [
        for (final entry in _keyEntries)
          if (entry.id != id) entry,
        KeyEntry(
          id: saved.id,
          uuid: saved.uuid,
          name: saved.name,
          publicKey: saved.publicKey,
          certificate: _sshCertificateSummaryMarker(saved.certificate),
          createdAt: saved.createdAt,
          updatedAt: saved.updatedAt,
          version: saved.version,
          createdDeviceId: saved.createdDeviceId,
          updatedDeviceId: saved.updatedDeviceId,
        ),
      ];
      _keys = _keyEntries.map(_mapKey).toList(growable: false);
      final current = _editorStack.last;
      _editorStack[_editorStack.length - 1] = _WorkspaceEditorStackEntry(
        request: _KeyEditorRequest(initial: saved),
        onSaved: current.onSaved,
      );
    });
  }

  void _editKey(_KeyItem item) {
    final existing = _dataStore?.getKey(item.id);
    if (existing == null) {
      return;
    }
    _openEditor(_KeyEditorRequest(initial: existing));
  }

  void _exportKey(_KeyItem item) {
    final existing = _dataStore?.getKey(item.id);
    if (existing == null) {
      _showWorkspaceMessage('Key is not available.');
      return;
    }
    _openEditor(_KeyExportEditorRequest(key: existing));
  }

  Future<void> _exportKeyToFile(KeyEntry key) async {
    final materials = <String, String>{
      '': ?_emptyToNull(key.privateKey),
      '.pub': ?_emptyToNull(key.publicKey),
      '-cert.pub': ?_emptyToNull(key.certificate),
    };
    if (materials.isEmpty) {
      _showWorkspaceMessage('No key material is available to export.');
      return;
    }

    try {
      final initialDirectory = await _prepareSshKeyExportDirectory();
      final directoryPath = await _runExclusiveFilePicker(
        () => getDirectoryPath(
          initialDirectory: initialDirectory,
          confirmButtonText: 'Export',
        ),
      );
      if (directoryPath == null || directoryPath.trim().isEmpty) {
        return;
      }

      final directory = io.Directory(directoryPath.trim());
      final basePath = await _availableSshKeyBasePath(
        directory,
        _suggestedSshKeyFilename(key.name),
        materials.keys,
      );
      final outputs = <io.File>[];
      for (final entry in materials.entries) {
        final output = io.File('$basePath${entry.key}');
        final contents = entry.value.endsWith('\n')
            ? entry.value
            : '${entry.value}\n';
        await output.writeAsString(contents, encoding: utf8, flush: true);
        if (io.Platform.isMacOS || io.Platform.isLinux) {
          await _setSshExportFileMode(
            output,
            entry.key.isEmpty ? '600' : '644',
          );
        }
        outputs.add(output);
      }
      if (mounted) {
        _showWorkspaceMessage(
          'Exported ${outputs.length} key ${outputs.length == 1 ? 'file' : 'files'} to ${directory.path}.',
        );
      }
    } on Object catch (error) {
      if (mounted) {
        _showWorkspaceMessage('Failed to export key: $error');
      }
    }
  }

  Future<String> _availableSshKeyBasePath(
    io.Directory directory,
    String filename,
    Iterable<String> suffixes,
  ) async {
    var attempt = 1;
    while (true) {
      final suffix = attempt == 1 ? '' : '-$attempt';
      final basePath =
          '${directory.path}${io.Platform.pathSeparator}$filename$suffix';
      var available = true;
      for (final extension in suffixes) {
        if (await io.File('$basePath$extension').exists()) {
          available = false;
          break;
        }
      }
      if (available) {
        return basePath;
      }
      attempt++;
    }
  }

  Future<void> _setSshExportFileMode(io.File file, String mode) async {
    final result = await io.Process.run('/bin/chmod', [mode, file.path]);
    if (result.exitCode == 0) {
      return;
    }
    final detail = result.stderr.toString().trim();
    throw io.FileSystemException(
      detail.isEmpty ? 'Unable to set key file permissions.' : detail,
      file.path,
    );
  }

  Future<String?> _prepareSshKeyExportDirectory() async {
    final home = io.Platform.isWindows
        ? io.Platform.environment['USERPROFILE'] ??
              io.Platform.environment['HOME']
        : io.Platform.environment['HOME'] ??
              io.Platform.environment['USERPROFILE'];
    if (home == null || home.trim().isEmpty) {
      return null;
    }

    final directory = io.Directory(
      '${home.trim()}${io.Platform.pathSeparator}.ssh',
    );
    try {
      await directory.create(recursive: true);
      if (io.Platform.isMacOS || io.Platform.isLinux) {
        await io.Process.run('/bin/chmod', ['700', directory.path]);
      }
      return directory.path;
    } on Object {
      return home.trim();
    }
  }

  String _suggestedSshKeyFilename(String name) {
    var filename = name
        .trim()
        .replaceAll(RegExp(r'(?:-cert\.pub|\.pub)$', caseSensitive: false), '')
        .replaceAll(RegExp(r'[\\/:*?"<>|\x00-\x1f]+'), '-')
        .replaceAll(RegExp(r'\s+'), '-')
        .replaceAll(RegExp(r'^\.+|[. -]+$'), '');
    if (RegExp(
      r'^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])$',
      caseSensitive: false,
    ).hasMatch(filename)) {
      filename = '_$filename';
    }
    return filename.isEmpty ? 'id_key' : filename;
  }

  Future<void> _exportKeyToHost(KeyEntry key, _KeyExportDraft draft) async {
    final publicKey = _emptyToNull(key.publicKey);
    if (publicKey == null) {
      throw StateError('A public key is required.');
    }
    final host = _hostEntries
        .where((entry) => entry.id == draft.hostId)
        .firstOrNull;
    final auth = _sshAuthForHost(host, feature: 'Key export');
    if (auth == null) {
      throw StateError('Host authentication is not available.');
    }
    final result = await _spawnSshPublicKeyExport({
      'host': auth.host,
      'port': auth.port,
      'username': auth.username,
      'knownHostsPath': NautermPaths.resolve().knownHostsFile.path,
      'password': auth.password,
      'privateKey': auth.privateKey,
      'certificate': auth.certificate,
      'passphrase': null,
      'proxy': auth.proxy?.toJson(),
      'hostKeyTrustMode': SshHostKeyTrustMode.strict.wireValue,
      'publicKey': publicKey,
      'location': draft.location,
      'filename': draft.filename,
      'script': draft.script,
    });
    final error = _emptyToNull(result.error);
    if (!result.ok || error != null) {
      throw StateError(error ?? 'The host rejected the key export.');
    }
    if (!mounted) {
      return;
    }
    _closeEditor();
    _showWorkspaceMessage('Key exported to ${host?.name ?? auth.host}.');
  }

  Future<void> _saveKey(KeyEntry key) async {
    final store = _dataStore;
    if (store == null) {
      _showWorkspaceMessage('Database is not ready.');
      return;
    }
    final id = store.saveKey(key);
    final detail = store.getKey(id);
    if (mounted) {
      final saved = detail ?? KeyEntry(id: id, name: key.name);
      final summary = KeyEntry(
        id: saved.id,
        uuid: saved.uuid,
        name: saved.name,
        publicKey: saved.publicKey,
        certificate: _sshCertificateSummaryMarker(saved.certificate),
        createdAt: saved.createdAt,
        updatedAt: saved.updatedAt,
        version: saved.version,
        createdDeviceId: saved.createdDeviceId,
        updatedDeviceId: saved.updatedDeviceId,
      );
      _setWorkspaceState(() {
        _keyEntries = [
          for (final entry in _keyEntries)
            if (entry.id != id) entry,
          summary,
        ];
        _keys = _keyEntries.map(_mapKey).toList(growable: false);
      });
      _completeEditorSave(saved);
    }
  }

  void _createIdentity() {
    _openEditor(const _IdentityEditorRequest());
  }

  void _editIdentity(_IdentityItem item) {
    final existing = _dataStore?.getIdentity(item.id);
    if (existing == null) {
      return;
    }
    _openEditor(_IdentityEditorRequest(initial: existing));
  }

  Future<void> _saveIdentity(IdentityEntry identity) async {
    final store = _dataStore;
    if (store == null) {
      _showWorkspaceMessage('Database is not ready.');
      return;
    }
    final id = store.saveIdentity(identity);
    final detail = store.getIdentity(id);
    if (mounted) {
      final saved = detail ?? IdentityEntry(id: id, name: identity.name);
      final summary = IdentityEntry(
        id: saved.id,
        uuid: saved.uuid,
        name: saved.name,
        username: saved.username,
        keyId: saved.keyId,
        keyUuid: saved.keyUuid,
        createdAt: saved.createdAt,
        updatedAt: saved.updatedAt,
        version: saved.version,
        createdDeviceId: saved.createdDeviceId,
        updatedDeviceId: saved.updatedDeviceId,
      );
      _setWorkspaceState(() {
        _identityEntries = [
          for (final entry in _identityEntries)
            if (entry.id != id) entry,
          summary,
        ];
        _identities = _identityEntries
            .map(_mapIdentity)
            .toList(growable: false);
        _hosts = _mapHosts(
          _hostEntries,
          _groupEntries,
          _identityEntries,
          _tagEntries,
        );
      });
      _completeEditorSave(saved);
    }
  }

  void _createProxy() {
    _openEditor(const _ProxyEditorRequest());
  }

  void _editProxy(_ProxyItem item) {
    final id = item.id;
    final existing =
        _dataStore?.getProxy(id) ??
        _proxyEntries.where((proxy) => proxy.id == id).firstOrNull;
    if (existing == null) {
      _showWorkspaceMessage('Proxy is not available.');
      return;
    }
    _openEditor(_ProxyEditorRequest(initial: existing));
  }

  Future<void> _saveProxy(ProxyEntry proxy) async {
    final store = _dataStore;
    if (store == null) {
      _showWorkspaceMessage('Database is not ready.');
      return;
    }
    final id = store.saveProxy(proxy);
    if (mounted) {
      final saved =
          store.getProxy(id) ??
          ProxyEntry(
            id: id,
            name: proxy.name,
            type: proxy.type,
            host: proxy.host,
            port: proxy.port,
          );
      final summary = ProxyEntry(
        id: saved.id,
        uuid: saved.uuid,
        name: saved.name,
        type: saved.type,
        host: saved.host,
        port: saved.port,
        identityId: saved.identityId,
        identityUuid: saved.identityUuid,
        username: saved.username,
        createdAt: saved.createdAt,
        updatedAt: saved.updatedAt,
        version: saved.version,
        createdDeviceId: saved.createdDeviceId,
        updatedDeviceId: saved.updatedDeviceId,
      );
      _setWorkspaceState(() {
        _proxyEntries = [
          for (final entry in _proxyEntries)
            if (entry.id != id) entry,
          summary,
        ];
        _proxies = _proxyEntries
            .map((entry) => _mapProxy(entry, _identityEntries))
            .toList(growable: false);
      });
      _completeEditorSave(saved);
    }
  }

  Future<void> _saveTerminalAuth(
    SshConnectionProfile profile, {
    String? password,
    TerminalConnectionKeyOption? key,
  }) async {
    final store = _dataStore;
    final hostId = profile.hostId;
    if (store == null || hostId == null) {
      return;
    }

    final identityId = profile.identityId;
    if (identityId != null) {
      final identity = store.getIdentity(identityId);
      if (identity != null) {
        store.saveIdentity(
          IdentityEntry(
            id: identity.id,
            name: identity.name,
            username: identity.username,
            password: password,
            keyId: key?.id,
          ),
        );
        final saved = store.getIdentity(identityId);
        if (saved != null && mounted) {
          _setWorkspaceState(() {
            _identityEntries = [
              for (final entry in _identityEntries)
                if (entry.id != identityId) entry,
              IdentityEntry(
                id: saved.id,
                uuid: saved.uuid,
                name: saved.name,
                username: saved.username,
                keyId: saved.keyId,
                keyUuid: saved.keyUuid,
                createdAt: saved.createdAt,
                updatedAt: saved.updatedAt,
                version: saved.version,
              ),
            ];
            _identities = _identityEntries
                .map(_mapIdentity)
                .toList(growable: false);
          });
        }
        return;
      }
    }

    final host = store.getHost(hostId);
    if (host == null) {
      return;
    }

    store.saveHost(
      HostEntry(
        id: host.id,
        name: host.name,
        groupId: host.groupId,
        identityId: host.identityId,
        proxyId: host.proxyId,
        host: host.host,
        port: host.port,
        username: host.username,
        password: password,
        themeId: host.themeId,
        startupSnippetId: host.startupSnippetId,
        startupSnippetUuid: host.startupSnippetUuid,
        sshEnabled: host.sshEnabledOverride,
        moshEnabled: host.moshEnabledOverride,
        moshServerCommand: host.moshServerCommandOverride,
        telnetEnabled: host.telnetEnabledOverride,
        telnetIdentityId: host.telnetIdentityId,
        telnetUsername: host.telnetUsername,
        telnetPassword: host.telnetPassword,
        telnetPort: host.telnetPort,
        telnetThemeId: host.telnetThemeId,
        environmentVariables: host.environmentVariables,
        encoding: host.encodingOverride,
        telnetEncoding: host.telnetEncodingOverride,
        type: host.type,
        keyId: key?.id,
        shellPath: host.shellPath,
        workDir: host.workDir,
        os: host.os,
        distro: host.distro,
        tagUuids: host.tagUuids,
      ),
    );
    final savedHost = store.getHost(hostId);
    if (savedHost != null && mounted) {
      _upsertHostSummary(savedHost);
    }
  }

  Future<void> _createSnippet([int? packageId]) async {
    _openEditor(_SnippetEditorRequest(initialPackageId: packageId));
  }

  void _createSnippetPackage() {
    _openEditor(const _SnippetPackageEditorRequest());
  }

  void _editSnippetPackage(_SnippetPackageItem package) {
    _openEditor(_SnippetPackageEditorRequest(initial: package));
  }

  void _createSnippetPackageForEditor(
    String initialName,
    ValueChanged<int> onCreated,
  ) {
    final name = initialName.trim();
    if (name.isEmpty) {
      return;
    }
    try {
      final saved = _persistSnippetPackage(SnippetPackageEntry(name: name));
      if (mounted && saved.id != null) {
        onCreated(saved.id!);
      }
    } catch (error) {
      if (mounted) {
        _showWorkspaceMessage(
          'Failed to create snippet package: $error',
          type: _WorkspaceNotificationType.error,
        );
      }
    }
  }

  Future<void> _saveSnippetPackage(SnippetPackageEntry package) async {
    final saved = _persistSnippetPackage(package);
    if (mounted) {
      _completeEditorSave(saved);
    }
  }

  SnippetPackageEntry _persistSnippetPackage(SnippetPackageEntry package) {
    final store = _dataStore;
    if (store == null) {
      throw StateError('Database is not ready.');
    }
    final existing = package.id == null
        ? null
        : _snippetPackageEntries
              .where((entry) => entry.id == package.id)
              .firstOrNull;
    final persisted = existing == null
        ? package
        : SnippetPackageEntry(
            id: existing.id,
            uuid: existing.uuid,
            name: package.name,
            createdAt: existing.createdAt,
            updatedAt: existing.updatedAt,
            deletedAt: existing.deletedAt,
            version: existing.version,
            createdDeviceId: existing.createdDeviceId,
            updatedDeviceId: existing.updatedDeviceId,
          );
    final id = store.saveSnippetPackage(persisted);
    final saved = SnippetPackageEntry(
      id: id,
      uuid: persisted.uuid,
      name: persisted.name,
      createdAt: persisted.createdAt,
      updatedAt: persisted.updatedAt,
      version: persisted.version,
      createdDeviceId: persisted.createdDeviceId,
      updatedDeviceId: persisted.updatedDeviceId,
    );
    if (mounted) {
      _setWorkspaceState(() {
        _snippetPackageEntries = [
          for (final entry in _snippetPackageEntries)
            if (entry.id != id) entry,
          saved,
        ];
      });
    }
    return saved;
  }

  void _createPortForward([String type = 'local']) {
    _openEditor(_PortForwardEditorRequest(initialType: type));
  }

  void _editPortForward(_PortForwardItem item) {
    final entry = _portForwardEntries
        .where((entry) => entry.id == item.id)
        .firstOrNull;
    if (entry == null) {
      _showWorkspaceMessage('Forwarding rule is not available.');
      return;
    }
    _openEditor(_PortForwardEditorRequest(initial: entry));
  }

  Future<void> _savePortForward(PortForwardEntry portForward) async {
    final store = _dataStore;
    if (store == null) {
      return;
    }
    final id = portForward.id;
    if (id != null) {
      FfiPortForwarding.stop(id);
      _runningPortForwardIds.remove(id);
      _portForwardStatuses.remove(id);
    }
    final savedId = store.savePortForward(portForward);
    final saved = store.getPortForward(savedId);
    _setWorkspaceState(() {
      if (saved != null) {
        _portForwardEntries = [
          for (final entry in _portForwardEntries)
            if (entry.id != savedId) entry,
          saved,
        ];
      }
      _portForwards = _portForwardEntries
          .map(
            (portForward) => _mapPortForward(
              portForward,
              _hostEntries,
              _runningPortForwardIds,
              _portForwardStatuses,
            ),
          )
          .toList(growable: false);
      _editorRequest = null;
    });
  }

  void _deletePortForward(PortForwardEntry portForward) {
    unawaited(_confirmAndDeletePortForward(portForward));
  }

  Future<void> _confirmAndDeletePortForward(
    PortForwardEntry portForward,
  ) async {
    final id = portForward.id;
    if (id == null) {
      return;
    }
    final confirmed = await _confirmDelete(
      title: 'Delete Forward?',
      message: 'Delete "${portForward.name}"? This cannot be undone.',
    );
    if (!confirmed) {
      return;
    }
    final store = _dataStore;
    if (store == null) {
      return;
    }
    FfiPortForwarding.stop(id);
    _runningPortForwardIds.remove(id);
    _portForwardStatuses.remove(id);
    store.deletePortForward(id);
    _setWorkspaceState(() {
      _portForwardEntries = [
        for (final entry in _portForwardEntries)
          if (entry.id != id) entry,
      ];
      _portForwards = _portForwardEntries
          .map(
            (portForward) => _mapPortForward(
              portForward,
              _hostEntries,
              _runningPortForwardIds,
              _portForwardStatuses,
            ),
          )
          .toList(growable: false);
      _editorRequest = null;
    });
  }

  void _togglePortForward(_PortForwardItem item, bool enabled) {
    if (item.id <= 0) {
      return;
    }
    if (enabled) {
      final status = _startPortForward(item);
      if (status == null || status.isError) {
        if (status != null) {
          _portForwardStatuses[item.id] = status;
        }
        _setWorkspaceState(() {
          _portForwards = _portForwardEntries
              .map(
                (portForward) => _mapPortForward(
                  portForward,
                  _hostEntries,
                  _runningPortForwardIds,
                  _portForwardStatuses,
                ),
              )
              .toList(growable: false);
        });
        return;
      }
      _runningPortForwardIds.add(item.id);
      _portForwardStatuses[item.id] = status;
    } else {
      FfiPortForwarding.stop(item.id);
      _runningPortForwardIds.remove(item.id);
      _portForwardStatuses.remove(item.id);
    }
    _setWorkspaceState(() {
      _portForwards = _portForwardEntries
          .map(
            (portForward) => _mapPortForward(
              portForward,
              _hostEntries,
              _runningPortForwardIds,
              _portForwardStatuses,
            ),
          )
          .toList(growable: false);
    });
  }

  void _syncPortForwardStatuses() {
    if (_runningPortForwardIds.isEmpty) {
      return;
    }

    final stoppedIds = <int>{};
    String? errorMessage;
    var changed = false;
    for (final id in _runningPortForwardIds.toList(growable: false)) {
      final status = FfiPortForwarding.status(id);
      final previous = _portForwardStatuses[id];
      final statusChanged = !_samePortForwardStatus(previous, status);
      if (statusChanged) {
        _portForwardStatuses[id] = status;
        changed = true;
        final statusError = status.error;
        if (statusError != null &&
            statusError.trim().isNotEmpty &&
            errorMessage == null) {
          errorMessage = statusError;
        }
      }
      if (status.isRunning) {
        continue;
      }
      stoppedIds.add(id);
      if (status.isError && errorMessage == null) {
        errorMessage = status.error ?? 'Port forwarding stopped unexpectedly.';
      }
    }
    if (stoppedIds.isEmpty && !changed) {
      return;
    }

    _runningPortForwardIds.removeAll(stoppedIds);
    for (final id in stoppedIds) {
      final status = _portForwardStatuses[id];
      if (status?.isError != true) {
        _portForwardStatuses.remove(id);
      }
    }
    _setWorkspaceState(() {
      _portForwards = _portForwardEntries
          .map(
            (portForward) => _mapPortForward(
              portForward,
              _hostEntries,
              _runningPortForwardIds,
              _portForwardStatuses,
            ),
          )
          .toList(growable: false);
    });
    if (errorMessage != null) {
      _showWorkspaceMessage(
        errorMessage,
        type: _WorkspaceNotificationType.error,
      );
    }
  }

  FfiPortForwardStatus? _startPortForward(_PortForwardItem item) {
    final entry = _portForwardEntries
        .where((entry) => entry.id == item.id)
        .firstOrNull;
    if (entry == null) {
      _showWorkspaceMessage('Forwarding rule is not available.');
      return null;
    }
    final host = _hostEntries
        .where((host) => host.id == entry.connectionId)
        .firstOrNull;
    final auth = _sshAuthForHost(host);
    if (auth == null) {
      return null;
    }
    final type = entry.type.trim().isEmpty ? 'local' : entry.type.toLowerCase();
    final destinationHost = type == 'dynamic'
        ? null
        : _emptyToNull(entry.destinationHost);
    if (type != 'dynamic' && destinationHost == null) {
      _showWorkspaceMessage('Destination host is required.');
      return null;
    }
    final status = FfiPortForwarding.start(
      id: item.id,
      type: type,
      sshHost: auth.host,
      sshPort: auth.port,
      username: auth.username,
      password: auth.password,
      privateKey: auth.privateKey,
      certificate: auth.certificate,
      proxy: auth.proxy,
      knownHostsPath: NautermPaths.resolve().knownHostsFile.path,
      bindAddress: entry.bindAddress,
      bindPort: entry.bindPort,
      destinationHost: destinationHost,
      destinationPort: type == 'dynamic' ? 0 : entry.destinationPort,
    );
    if (status.isError) {
      _showWorkspaceMessage(status.error ?? 'Failed to start port forwarding.');
      return status;
    }
    _showWorkspaceMessage(
      status.boundPort == null
          ? 'Port forwarding started.'
          : 'Port forwarding started on port ${status.boundPort}.',
    );
    return status;
  }

  _PortForwardSshAuth? _sshAuthForHost(
    HostEntry? host, {
    String feature = 'Port forwarding',
    int? portOverride,
  }) {
    if (host == null) {
      _showWorkspaceMessage('$feature host is not available.');
      return null;
    }
    final hostId = host.id;
    if (hostId != null) {
      host = _dataStore?.getHost(hostId) ?? host;
    }
    if (host.type != NautermHostType.remote) {
      _showWorkspaceMessage('$feature requires an SSH host.');
      return null;
    }
    if (!host.sshEnabled) {
      _showWorkspaceMessage('$feature requires SSH to be enabled.');
      return null;
    }
    final address = _emptyToNull(host.host);
    if (address == null) {
      _showWorkspaceMessage('$feature host address is required.');
      return null;
    }
    final port = portOverride ?? host.port ?? 22;
    if (port < 1 || port > 65535) {
      _showWorkspaceMessage('$feature host port must be between 1 and 65535.');
      return null;
    }
    final identity = host.identityId == null
        ? null
        : _dataStore?.getIdentity(host.identityId!);
    final username =
        _firstNonEmpty([identity?.username, host.username]) ?? 'user';
    final keyId = identity?.keyId ?? host.keyId;
    final privateKey = keyId == null
        ? null
        : _emptyToNull(_dataStore?.getKey(keyId)?.privateKey);
    final certificate = keyId == null
        ? null
        : _emptyToNull(_dataStore?.getKey(keyId)?.certificate);
    final password = privateKey == null
        ? (identity == null
              ? _emptyToNull(host.password)
              : _emptyToNull(identity.password))
        : null;
    final proxy = _terminalProxyConfigForHost(host, feature: feature);
    if (host.proxyId != null && proxy == null) {
      return null;
    }
    return _PortForwardSshAuth(
      host: address,
      port: port,
      username: username,
      password: password,
      privateKey: privateKey,
      certificate: certificate,
      proxy: proxy,
    );
  }

  TerminalProxyConfig? _terminalProxyConfigForHost(
    HostEntry host, {
    String feature = 'SSH',
  }) {
    final proxyId = host.proxyId;
    if (proxyId == null) {
      return null;
    }
    final proxy =
        _dataStore?.getProxy(proxyId) ??
        _proxyEntries.where((proxy) => proxy.id == proxyId).firstOrNull;
    if (proxy == null) {
      _showWorkspaceMessage('$feature proxy is not available.');
      return null;
    }
    final type = _normalizeProxyType(proxy.type);
    final proxyHost = _emptyToNull(proxy.host);
    if (proxyHost == null) {
      _showWorkspaceMessage('$feature proxy host is required.');
      return null;
    }
    if (proxy.port < 1 || proxy.port > 65535) {
      _showWorkspaceMessage('$feature proxy port must be between 1 and 65535.');
      return null;
    }
    final identity = proxy.identityId == null
        ? null
        : _dataStore?.getIdentity(proxy.identityId!);
    return TerminalProxyConfig(
      type: type,
      host: proxyHost,
      port: proxy.port,
      username: _firstNonEmpty([proxy.username, identity?.username]),
      password: _firstNonEmpty([proxy.password, identity?.password]),
    );
  }

  Future<void> _editSnippet(_SnippetItem item) async {
    _openEditor(_SnippetEditorRequest(initial: item));
  }

  Future<void> _saveSnippet(_SnippetItem? initial, _SnippetDraft draft) async {
    final saved = await _persistSnippet(initial, draft);
    if (mounted) {
      if (saved == null) {
        _closeEditor();
      } else {
        _completeEditorSave(saved);
      }
    }
  }

  Future<void> _saveTerminalSnippet(_SnippetDraft draft) async {
    await _persistSnippet(null, draft);
  }

  Future<_SnippetItem?> _persistSnippet(
    _SnippetItem? initial,
    _SnippetDraft draft,
  ) async {
    final store = _dataStore;
    if (store == null) {
      throw StateError('Database is not ready.');
    }
    final id = store.saveSnippet(
      SnippetEntry(
        id: initial?.id,
        packageId: draft.packageId,
        scope: draft.scope,
        description: draft.description,
        script: draft.script,
        targetGroupIds: draft.targetGroupIds,
        targetHostIds: draft.targetHostIds,
      ),
    );
    final saved = store.getSnippet(id);
    _SnippetItem? savedItem;
    if (saved != null && mounted) {
      _setWorkspaceState(() {
        final item = _mapSnippet(saved);
        savedItem = item;
        _snippets = [
          for (final entry in _snippets)
            if (entry.id != id) entry,
          item,
        ];
      });
    }
    return savedItem;
  }

  void _runSnippet(_SnippetItem item) {
    late final TerminalController controller;

    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      final id = _nextTerminalId++;
      controller = TerminalController(
        config: currentTerminalConfig(),
        shellPath: _resolvedLocalShellPath(null),
        environment: _terminalEnvironmentForTheme(defaultTerminalTheme),
        theme: defaultTerminalTheme,
        recorder: _createTerminalRecorder(
          title: item.description,
          username: _localShellUsername(),
          shellPath: _resolvedLocalShellPath(null),
        ),
        onExit: () => _closeTerminalViewTab(id, id, id),
      );
      _terminalTabs.add(
        _TerminalTab(
          id: id,
          title: item.description,
          theme: defaultTerminalTheme,
          controller: controller,
        ),
      );
      _selectedTerminalId = id;
      _selectedTerminalViewId = id;
      _editorRequest = null;
    });
    widget.controller?._notifyTabsChanged();

    final normalized = item.script
        .replaceAll('\r\n', '\n')
        .replaceAll('\n', '\r');
    controller.sendInput(
      normalized.endsWith('\r') ? normalized : '$normalized\r',
    );
  }

  void _duplicateSnippet(_SnippetItem item) {
    unawaited(_duplicateSnippetInStore(item));
  }

  Future<void> _duplicateSnippetInStore(_SnippetItem item) async {
    final store = _dataStore;
    if (store == null) {
      _showWorkspaceMessage(
        'Database is not ready.',
        type: _WorkspaceNotificationType.error,
      );
      return;
    }
    try {
      final id = store.saveSnippet(
        SnippetEntry(
          packageId: item.packageId,
          scope: item.scope,
          description: '${item.description} copy',
          script: item.script,
          targetGroupIds: item.targetGroupIds,
          targetHostIds: item.targetHostIds,
        ),
      );
      final saved = store.getSnippet(id);
      if (saved != null && mounted) {
        _setWorkspaceState(() {
          _snippets = [..._snippets, _mapSnippet(saved)];
        });
      }
    } catch (error) {
      if (!mounted) {
        return;
      }
      _showWorkspaceMessage(
        'Failed to duplicate snippet: $error',
        type: _WorkspaceNotificationType.error,
      );
    }
  }

  void _deleteSnippet(_SnippetItem item) {
    unawaited(_confirmAndDeleteSnippet(item));
  }

  void _handleSnippetPackageContextAction(
    _SnippetPackageItem package,
    _ContextMenuActionId action,
  ) {
    switch (action) {
      case _ContextMenuActionId.edit:
        _editSnippetPackage(package);
        break;
      case _ContextMenuActionId.delete:
        unawaited(_confirmAndDeleteSnippetPackage(package));
        break;
      case _ContextMenuActionId.open:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.duplicate:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  void _handleSnippetPackageContextActions(
    List<_SnippetPackageItem> packages,
    _ContextMenuActionId action,
  ) {
    final targets = {for (final package in packages) package.id: package}.values
        .toList();
    if (targets.length < 2) {
      if (targets.isNotEmpty) {
        _handleSnippetPackageContextAction(targets.single, action);
      }
      return;
    }
    if (action == _ContextMenuActionId.delete) {
      unawaited(_confirmAndDeleteSnippetPackages(targets));
    }
  }

  Future<void> _confirmAndDeleteSnippetPackage(
    _SnippetPackageItem package,
  ) async {
    final confirmed = await _confirmDelete(
      title: 'Delete snippet package?',
      message:
          'Delete "${package.name}"? Its snippets will be kept without a '
          'package.',
    );
    if (!confirmed || !mounted) {
      return;
    }
    final store = _dataStore;
    if (store == null) {
      _showWorkspaceMessage(
        'Database is not ready.',
        type: _WorkspaceNotificationType.error,
      );
      return;
    }
    try {
      store.deleteSnippetPackage(package.id);
      if (!mounted) {
        return;
      }
      _setWorkspaceState(() {
        _snippetPackageEntries = [
          for (final entry in _snippetPackageEntries)
            if (entry.id != package.id) entry,
        ];
        _snippets = [
          for (final snippet in _snippets)
            if (snippet.packageId == package.id)
              snippet.copyWith(packageId: null)
            else
              snippet,
        ];
      });
    } catch (error) {
      if (mounted) {
        _showWorkspaceMessage(
          'Failed to delete snippet package: $error',
          type: _WorkspaceNotificationType.error,
        );
      }
    }
  }

  Future<void> _confirmAndDeleteSnippetPackages(
    List<_SnippetPackageItem> packages,
  ) async {
    final ids = packages.map((package) => package.id).toSet();
    if (ids.isEmpty) {
      return;
    }
    final confirmed = await _confirmDelete(
      title: 'Delete snippet packages?',
      message:
          'Delete ${ids.length} packages? Their snippets will be kept without '
          'a package.',
    );
    if (!confirmed || !mounted) {
      return;
    }
    final store = _dataStore;
    if (store == null) {
      _showWorkspaceMessage(
        'Database is not ready.',
        type: _WorkspaceNotificationType.error,
      );
      return;
    }
    try {
      for (final id in ids) {
        store.deleteSnippetPackage(id);
      }
      if (!mounted) {
        return;
      }
      _setWorkspaceState(() {
        _snippetPackageEntries = [
          for (final entry in _snippetPackageEntries)
            if (!ids.contains(entry.id)) entry,
        ];
        _snippets = [
          for (final snippet in _snippets)
            if (ids.contains(snippet.packageId))
              snippet.copyWith(packageId: null)
            else
              snippet,
        ];
      });
    } catch (error) {
      if (mounted) {
        _showWorkspaceMessage(
          'Failed to delete snippet packages: $error',
          type: _WorkspaceNotificationType.error,
        );
      }
    }
  }

  Future<void> _confirmAndDeleteSnippet(_SnippetItem item) async {
    final confirmed = await _confirmDelete(
      title: 'Delete Snippet?',
      message: 'Delete "${item.description}"? This cannot be undone.',
    );
    if (!confirmed) {
      return;
    }
    if (!mounted) {
      return;
    }
    final store = _dataStore;
    if (store == null) {
      _showWorkspaceMessage(
        'Database is not ready.',
        type: _WorkspaceNotificationType.error,
      );
      return;
    }
    store.deleteSnippet(item.id);
    if (!mounted) {
      return;
    }
    _setWorkspaceState(() {
      _snippets = [
        for (final entry in _snippets)
          if (entry.id != item.id) entry,
      ];
      if (_editorRequest case _SnippetEditorRequest(initial: final initial?)
          when initial.id == item.id) {
        _editorRequest = null;
      }
    });
  }

  void _handleSnippetContextAction(
    _SnippetItem item,
    _ContextMenuActionId action,
  ) {
    switch (action) {
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.open:
        _runSnippet(item);
        break;
      case _ContextMenuActionId.edit:
        _editSnippet(item);
        break;
      case _ContextMenuActionId.duplicate:
        _duplicateSnippet(item);
        break;
      case _ContextMenuActionId.delete:
        _deleteSnippet(item);
        break;
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  void _handleSnippetContextActions(
    List<_SnippetItem> items,
    _ContextMenuActionId action,
  ) {
    final targets = {for (final item in items) item.id: item}.values.toList();
    if (targets.length < 2) {
      if (targets.isNotEmpty) {
        _handleSnippetContextAction(targets.single, action);
      }
      return;
    }
    switch (action) {
      case _ContextMenuActionId.duplicate:
        unawaited(
          Future.wait([
            for (final item in targets) _duplicateSnippetInStore(item),
          ]),
        );
        break;
      case _ContextMenuActionId.delete:
        unawaited(_confirmAndDeleteSnippets(targets));
        break;
      case _ContextMenuActionId.open:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.edit:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  Future<void> _confirmAndDeleteSnippets(List<_SnippetItem> items) async {
    final ids = items.map((item) => item.id).toSet();
    if (ids.isEmpty) {
      return;
    }
    final confirmed = await _confirmDelete(
      title: 'Delete Snippets?',
      message: 'Delete ${ids.length} snippets? This cannot be undone.',
    );
    if (!confirmed || !mounted) {
      return;
    }
    final store = _dataStore;
    if (store == null) {
      _showWorkspaceMessage(
        'Database is not ready.',
        type: _WorkspaceNotificationType.error,
      );
      return;
    }
    for (final id in ids) {
      store.deleteSnippet(id);
    }
    if (!mounted) {
      return;
    }
    _setWorkspaceState(() {
      _snippets = [
        for (final entry in _snippets)
          if (!ids.contains(entry.id)) entry,
      ];
      if (_editorRequest case _SnippetEditorRequest(initial: final initial?)
          when ids.contains(initial.id)) {
        _editorRequest = null;
      }
    });
  }

  void _showShellHistory() {
    _setWorkspaceState(() {
      _editorRequest = const _ShellHistoryDrawerRequest();
    });
  }

  void _createSnippetFromShellHistory(String command) {
    final script = command.trim();
    if (script.isEmpty) {
      return;
    }
    _openEditor(
      _SnippetEditorRequest(initialDescription: script, initialScript: script),
    );
  }

  void _clearShellHistory() {
    unawaited(_confirmAndClearShellHistory());
  }

  Future<void> _confirmAndClearShellHistory() async {
    final confirmed = await _confirmDelete(
      title: 'Clear shell history?',
      message:
          'This clears all shell history saved by Nauterm. It does not modify '
          'history files managed by local or remote shells.',
      confirmLabel: 'Clear all',
    );
    if (!confirmed || !mounted) {
      return;
    }

    final operation = _shellHistoryPersistence.then(
      (_) =>
          ShellHistoryFileStore(NautermPaths.resolve().shellHistoryFile)
              .clear(),
    );
    _shellHistoryPersistence = operation;
    try {
      await operation;
      if (!mounted) {
        return;
      }
      _setWorkspaceState(() {
        _shellHistory = const [];
        _shellHistoryByController.clear();
      });
    } catch (error) {
      if (!mounted) {
        return;
      }
      _showWorkspaceMessage(
        'Failed to clear shell history: $error',
        type: _WorkspaceNotificationType.error,
      );
    }
  }

  void _selectTerminalLog(TerminalLogEntry log) {
    _setWorkspaceState(() {
      _selectedLogId = log.id;
    });
  }

  Future<void> _deleteTerminalLog(TerminalLogEntry log) async {
    final confirmed = await _confirmDelete(
      title: 'Delete terminal history?',
      message:
          'This permanently deletes the session metadata and its encrypted '
          'capture file.',
    );
    if (!confirmed) return;

    _recordingService.ignore(log.id);
    await _recordingService.closeCapture(log.id);
    _recordingService.removePending(log.id);
    _recordingService.removeCaptureReference(log.id);
    final store = _terminalLogCaptureStore;
    final dataStore = _dataStore;
    try {
      if (store != null && log.captureFile.isNotEmpty) {
        await store.deleteCapture(log.captureFile);
      }
      dataStore?.deleteTerminalLog(log.id);
      final usage = await store?.diskUsage() ?? 0;
      if (!mounted) return;
      _setWorkspaceState(() {
        _terminalLogs = [
          for (final entry in _terminalLogs)
            if (entry.id != log.id) entry,
        ];
        _recordingService.removeRecording(log.id);
        _terminalCaptureDiskUsage = usage;
        if (_selectedLogId == log.id) _selectedLogId = null;
      });
    } on Object catch (error) {
      if (mounted) {
        _showWorkspaceMessage('Failed to delete terminal log: $error');
      }
    }
  }

  Future<void> _clearTerminalLogs() async {
    final confirmed = await _confirmDelete(
      title: 'Clear all terminal history?',
      message:
          'This permanently deletes all session metadata and encrypted capture '
          'files. Active sessions will stop being recorded.',
      confirmLabel: 'Clear all',
    );
    if (!confirmed) return;

    try {
      _recordingService.stopAllRecordings();
      await _recordingService.closeAllCaptures();
      await _terminalLogCaptureStore?.clear();
      _dataStore?.clearTerminalLogs();
      if (!mounted) return;
      _setWorkspaceState(() {
        _terminalLogs = const [];
        _recordingService.clearRecordings();
        _selectedLogId = null;
        _terminalLogsHasMore = false;
        _terminalCaptureDiskUsage = 0;
      });
    } on Object catch (error) {
      if (mounted) {
        _showWorkspaceMessage('Failed to clear terminal logs: $error');
      }
    }
  }

  Future<void> _exportTerminalLog(
    TerminalLogEntry log,
    _TerminalLogExportFormat format,
  ) async {
    final store = _terminalLogCaptureStore;
    if (store == null || log.captureFile.isEmpty) {
      _showWorkspaceMessage('This session has no terminal capture.');
      return;
    }
    if (log.captureSha256 == null) {
      _showWorkspaceMessage(
        'This terminal capture is still active or awaiting recovery.',
      );
      return;
    }
    final info = await store.captureInfo(
      log.id,
      captureFile: log.captureFile,
      includeHash: true,
    );
    if (info.sha256 != log.captureSha256) {
      _showWorkspaceMessage(
        'The encrypted terminal capture is incomplete or has been modified.',
      );
      return;
    }

    final safeTitle = log.title
        .replaceAll(RegExp(r'[^A-Za-z0-9._-]+'), '-')
        .replaceAll(RegExp(r'^-+|-+$'), '');
    final extension = switch (format) {
      _TerminalLogExportFormat.readableText => 'txt',
      _TerminalLogExportFormat.rawAnsi => 'ansi',
    };
    final typeLabel = switch (format) {
      _TerminalLogExportFormat.readableText => 'Plain text',
      _TerminalLogExportFormat.rawAnsi => 'ANSI terminal stream',
    };
    final location = await getSaveLocation(
      acceptedTypeGroups: [
        XTypeGroup(label: typeLabel, extensions: [extension]),
      ],
      initialDirectory: await _terminalLogExportInitialDirectory(),
      suggestedName: '${safeTitle.isEmpty ? 'terminal' : safeTitle}-${log.id}',
    );
    if (location == null || location.path.trim().isEmpty) return;
    final outputPath = _terminalLogExportPath(location.path, extension);

    try {
      switch (format) {
        case _TerminalLogExportFormat.readableText:
          final text = await _renderTerminalCaptureAsText(store, log);
          await io.File(outputPath)
              .writeAsString(text, encoding: utf8, flush: true);
        case _TerminalLogExportFormat.rawAnsi:
          final output = io.File(outputPath).openWrite();
          try {
            await for (final chunk in store.readDecryptedChunks(
              logId: log.id,
              captureFile: log.captureFile,
            )) {
              output.add(chunk);
            }
            await output.flush();
          } finally {
            await output.close();
          }
      }
      if (mounted) {
        _showWorkspaceMessage(
          format == _TerminalLogExportFormat.readableText
              ? 'Readable terminal transcript exported.'
              : 'Raw terminal stream exported.',
        );
      }
    } on Object catch (error) {
      if (mounted) {
        _showWorkspaceMessage('Failed to export terminal capture: $error');
      }
    }
  }

  String _terminalLogExportPath(String path, String extension) {
    final suffix = '.$extension';
    var normalized = path;
    while (normalized.toLowerCase().endsWith('$suffix$suffix')) {
      normalized = normalized.substring(0, normalized.length - suffix.length);
    }
    return normalized.toLowerCase().endsWith(suffix)
        ? normalized
        : '$normalized$suffix';
  }

  Future<String?> _terminalLogExportInitialDirectory() async {
    final home = io.Platform.environment['HOME']?.trim().isNotEmpty == true
        ? io.Platform.environment['HOME']!.trim()
        : io.Platform.environment['USERPROFILE']?.trim();
    if (home == null || home.isEmpty) return null;

    for (final name in const ['Downloads', 'Documents']) {
      final directory = io.Directory('$home${io.Platform.pathSeparator}$name');
      if (await directory.exists()) return directory.path;
    }
    final homeDirectory = io.Directory(home);
    return await homeDirectory.exists() ? homeDirectory.path : null;
  }

  Future<String> _renderTerminalCaptureAsText(
    TerminalLogCaptureStore store,
    TerminalLogEntry log,
  ) async {
    final driver = NativeReplayTerminalDriver.create(
      columns: log.columns ?? 100,
      rows: log.rows ?? 30,
      config: currentTerminalConfig(),
    );
    final sanitizer = TerminalReplaySanitizer();
    try {
      await for (final chunk in store.readDecryptedChunks(
        logId: log.id,
        captureFile: log.captureFile,
      )) {
        final sanitized = sanitizer.add(chunk);
        if (sanitized.isNotEmpty) driver.writeBytes(sanitized);
        await Future<void>.delayed(Duration.zero);
      }
      final remaining = sanitizer.close(restorePrimaryScreen: true);
      if (remaining.isNotEmpty) driver.writeBytes(remaining);
      return driver.plainText;
    } finally {
      driver.dispose();
    }
  }

  void _replayTerminalLog(TerminalLogEntry log) {
    unawaited(
      _openReplayTerminalLog(log).catchError((Object error) {
        if (mounted) {
          _showWorkspaceMessage('Failed to replay terminal log: $error');
        }
      }),
    );
  }

  Future<void> _openReplayTerminalLog(TerminalLogEntry log) async {
    final store = _terminalLogCaptureStore;
    if (store == null || log.captureFile.isEmpty) {
      _showWorkspaceMessage('This session has no terminal capture.');
      return;
    }
    if (log.captureSha256 == null) {
      _showWorkspaceMessage(
        'This terminal capture is still active or awaiting recovery.',
      );
      return;
    }
    final controller = TerminalController(
      driver: NativeReplayTerminalDriver.create(
        columns: log.columns ?? 100,
        rows: log.rows ?? 30,
        config: currentTerminalConfig(),
      ),
      config: currentTerminalConfig(),
    );
    final id = _nextTerminalId++;
    final title = 'Replay: ${log.title}';
    final workspace = _activeSessionWorkspace;
    final replayTab = _TerminalTab(
      id: id,
      title: title,
      theme: _defaultThemeForSettings(),
      controller: controller,
      replay: true,
      replayLoading: true,
    );
    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      _terminalTabs.add(replayTab);
      _selectedTerminalId = id;
      _selectedTerminalViewId = id;
      _editorRequest = null;
    });
    widget.controller?._notifyTabsChanged();

    try {
      final results = await Future.wait<Object>([
        store.captureInfo(
          log.id,
          captureFile: log.captureFile,
          includeHash: log.endedAt != null && log.captureSha256 != null,
        ),
        _themeForId(log.themeId),
      ]);
      final captureInfo = results[0] as TerminalLogCaptureInfo;
      final theme = results[1] as TerminalTheme;
      if (mounted &&
          !controller.isDisposed &&
          workspace.terminalTabs.contains(replayTab)) {
        _setWorkspaceState(() {
          replayTab.primaryView.activeTab.theme = theme;
        });
      }

      if (captureInfo.bytes <= 0) {
        throw StateError('This session has no terminal capture.');
      }
      if (captureInfo.sha256 != log.captureSha256) {
        throw const FormatException(
          'The encrypted terminal capture is incomplete or has been modified.',
        );
      }
      await _loadReplayCapture(controller, store, log.id, captureInfo.fileName);
    } on Object catch (error) {
      if (!controller.isDisposed) {
        controller.write(
          '\r\nUnable to replay this terminal session: $error\r\n',
        );
      }
      rethrow;
    } finally {
      if (mounted &&
          !controller.isDisposed &&
          workspace.terminalTabs.contains(replayTab)) {
        _setWorkspaceState(() => replayTab.replayLoading = false);
      }
    }
  }

  Future<void> _loadReplayCapture(
    TerminalController controller,
    TerminalLogCaptureStore store,
    String logId,
    String captureFile,
  ) async {
    var chunks = 0;
    final sanitizer = TerminalReplaySanitizer();
    await for (final chunk in store.readDecryptedChunks(
      logId: logId,
      captureFile: captureFile,
    )) {
      if (!mounted || controller.isDisposed) {
        return;
      }

      chunks += 1;
      final sanitized = sanitizer.add(chunk);
      if (sanitized.isNotEmpty) {
        controller.writeBytes(sanitized, refresh: false);
      }

      if (chunks % _replayYieldChunkInterval == 0) {
        await Future<void>.delayed(Duration.zero);
      }
    }

    final remaining = sanitizer.close(restorePrimaryScreen: true);
    if (remaining.isNotEmpty && mounted && !controller.isDisposed) {
      controller.writeBytes(remaining, refresh: false);
    }

    if (mounted && !controller.isDisposed) {
      controller.refreshSnapshot();
    }
  }

  void _handleGroupContextAction(_GroupItem item, _ContextMenuActionId action) {
    switch (action) {
      case _ContextMenuActionId.edit:
        _editGroup(item);
        break;
      case _ContextMenuActionId.duplicate:
        final group = _groupEntries
            .where((group) => group.id == item.id)
            .firstOrNull;
        if (group != null) {
          final store = _dataStore;
          final id = store?.saveGroup(
            HostGroup(name: '${group.name} copy', parentId: group.parentId),
          );
          final saved = id == null ? null : store?.getGroup(id);
          if (saved != null) {
            _setWorkspaceState(() {
              _groupEntries = [..._groupEntries, saved];
              _groups = _mapGroups(_groupEntries, _hostEntries);
            });
          }
        }
        break;
      case _ContextMenuActionId.newHostInGroup:
        _openEditor(_HostEditorRequest(initialGroupId: item.id));
        break;
      case _ContextMenuActionId.delete:
        unawaited(_deleteGroup(item));
        break;
      case _ContextMenuActionId.open:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  void _handleGroupContextActions(
    List<_GroupItem> items,
    _ContextMenuActionId action,
  ) {
    final targets = {for (final item in items) item.id: item}.values.toList();
    if (targets.length < 2) {
      if (targets.isNotEmpty) {
        _handleGroupContextAction(targets.single, action);
      }
      return;
    }
    switch (action) {
      case _ContextMenuActionId.duplicate:
        for (final item in targets) {
          _handleGroupContextAction(item, action);
        }
        break;
      case _ContextMenuActionId.delete:
        unawaited(_deleteGroups(targets));
        break;
      case _ContextMenuActionId.open:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.edit:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  void _handleHostContextActions(
    List<_HostItem> items,
    _ContextMenuActionId action,
  ) {
    final targets = {for (final item in items) item.id: item}.values.toList();
    if (targets.isEmpty) {
      return;
    }
    if (targets.length == 1) {
      _handleHostContextAction(targets.single, action);
      return;
    }

    switch (action) {
      case _ContextMenuActionId.open:
        unawaited(
          Future.wait([for (final item in targets) _connectHost(item)]),
        );
        break;
      case _ContextMenuActionId.hostWithSsh:
        unawaited(
          Future.wait([
            for (final item in targets)
              _connectHost(item, protocol: _HostConnectProtocol.ssh),
          ]),
        );
        break;
      case _ContextMenuActionId.hostWithMosh:
        unawaited(
          Future.wait([
            for (final item in targets)
              _connectHost(item, protocol: _HostConnectProtocol.mosh),
          ]),
        );
        break;
      case _ContextMenuActionId.hostWithTelnet:
        unawaited(
          Future.wait([
            for (final item in targets)
              _connectHost(item, protocol: _HostConnectProtocol.telnet),
          ]),
        );
        break;
      case _ContextMenuActionId.hostWithSftp:
        for (final item in targets) {
          _connectSftpHost(item);
        }
        break;
      case _ContextMenuActionId.openInCurrentWorkspace:
        unawaited(
          Future.wait([
            for (final item in targets)
              _connectHostInWorkspace(item, _selectedWorkspace),
          ]),
        );
        break;
      case _ContextMenuActionId.openInNewWorkspace:
        unawaited(_connectHostsInNewWorkspace(targets));
        break;
      case _ContextMenuActionId.duplicate:
        for (final item in targets) {
          _handleHostContextAction(item, action);
        }
        break;
      case _ContextMenuActionId.delete:
        unawaited(_deleteHosts(targets));
        break;
      case _ContextMenuActionId.edit:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  void _handleHostContextAction(_HostItem item, _ContextMenuActionId action) {
    switch (action) {
      case _ContextMenuActionId.open:
        _connectHost(item);
        break;
      case _ContextMenuActionId.hostWithSsh:
        _connectHost(item, protocol: _HostConnectProtocol.ssh);
        break;
      case _ContextMenuActionId.hostWithMosh:
        _connectHost(item, protocol: _HostConnectProtocol.mosh);
        break;
      case _ContextMenuActionId.hostWithTelnet:
        _connectHost(item, protocol: _HostConnectProtocol.telnet);
        break;
      case _ContextMenuActionId.hostWithSftp:
        _connectSftpHost(item);
        break;
      case _ContextMenuActionId.openInCurrentWorkspace:
        unawaited(_connectHostInWorkspace(item, _selectedWorkspace));
        break;
      case _ContextMenuActionId.openInNewWorkspace:
        unawaited(_connectHostInNewWorkspace(item));
        break;
      case _ContextMenuActionId.edit:
        _editHost(item);
        break;
      case _ContextMenuActionId.duplicate:
        final host = _dataStore?.getHost(item.id);
        if (host != null) {
          final store = _dataStore!;
          final id = store.saveHost(
            HostEntry(
              name: '${host.name} copy',
              groupId: host.groupId,
              identityId: host.identityId,
              proxyId: host.proxyId,
              host: host.host,
              port: host.port,
              username: host.username,
              password: host.password,
              themeId: host.themeId,
              startupSnippetId: host.startupSnippetId,
              startupSnippetUuid: host.startupSnippetUuid,
              sshEnabled: host.sshEnabledOverride,
              moshEnabled: host.moshEnabledOverride,
              moshServerCommand: host.moshServerCommandOverride,
              telnetEnabled: host.telnetEnabledOverride,
              telnetIdentityId: host.telnetIdentityId,
              telnetUsername: host.telnetUsername,
              telnetPassword: host.telnetPassword,
              telnetPort: host.telnetPort,
              telnetThemeId: host.telnetThemeId,
              environmentVariables: host.environmentVariables,
              encoding: host.encodingOverride,
              telnetEncoding: host.telnetEncodingOverride,
              type: host.type,
              keyId: host.keyId,
              shellPath: host.shellPath,
              workDir: host.workDir,
              os: host.os,
              distro: host.distro,
              tagUuids: host.tagUuids,
            ),
          );
          final saved = store.getHost(id);
          if (saved != null) {
            _upsertHostSummary(saved);
          }
        }
        break;
      case _ContextMenuActionId.copySshCommand:
        _copySshCommand(item);
        break;
      case _ContextMenuActionId.delete:
        unawaited(_deleteHost(item));
        break;
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  Future<void> _connectHostInWorkspace(
    _HostItem item,
    _WorkspaceRuntimeState workspace,
  ) async {
    _setWorkspaceState(() {
      _selectedWorkspaceId = workspace.id;
      _tab = _WorkspaceTab.sessions;
      _selectedTerminalId = null;
      _selectedTerminalViewId = null;
      _editorRequest = null;
    });
    await _connectHost(item);
  }

  Future<void> _connectHostInNewWorkspace(_HostItem item) async {
    await _connectHostsInNewWorkspace([item]);
  }

  Future<void> _connectHostsInNewWorkspace(List<_HostItem> items) async {
    if (items.isEmpty) {
      return;
    }
    final workspace = _addRuntimeWorkspace(items.first.name);
    _setWorkspaceState(() {
      _selectedWorkspaceId = workspace.id;
      _tab = _WorkspaceTab.sessions;
      _selectedTerminalId = null;
      _selectedTerminalViewId = null;
      _editorRequest = null;
    });
    await Future.wait([for (final item in items) _connectHost(item)]);
  }

  Future<void> _deleteGroup(_GroupItem item) async {
    final confirmed = await _confirmDelete(
      title: 'Delete Group?',
      message: 'Delete "${item.name}"? Hosts in this group will be ungrouped.',
    );
    if (!confirmed) {
      return;
    }
    _dataStore?.deleteGroup(item.id);
    if (mounted) {
      _setWorkspaceState(() {
        _groupEntries = [
          for (final group in _groupEntries)
            if (group.id != item.id)
              if (group.parentId == item.id)
                HostGroup(
                  id: group.id,
                  uuid: group.uuid,
                  name: group.name,
                  createdAt: group.createdAt,
                  updatedAt: group.updatedAt,
                  version: group.version,
                  createdDeviceId: group.createdDeviceId,
                  updatedDeviceId: group.updatedDeviceId,
                )
              else
                group,
        ];
        _hostEntries = [
          for (final host in _hostEntries)
            if (host.groupId == item.id)
              _hostSummary(host, clearGroup: true)
            else
              host,
        ];
        _groups = _mapGroups(_groupEntries, _hostEntries);
        _hosts = _mapHosts(
          _hostEntries,
          _groupEntries,
          _identityEntries,
          _tagEntries,
        );
      });
    }
  }

  Future<void> _deleteGroups(List<_GroupItem> items) async {
    final targets = {for (final item in items) item.id: item}.values.toList();
    if (targets.isEmpty) {
      return;
    }
    final confirmed = await _confirmDelete(
      title: 'Delete Groups?',
      message:
          'Delete ${targets.length} groups? Hosts in these groups will be '
          'ungrouped.',
    );
    if (!confirmed) {
      return;
    }
    final ids = targets.map((item) => item.id).toSet();
    for (final id in ids) {
      _dataStore?.deleteGroup(id);
    }
    if (!mounted) {
      return;
    }
    _setWorkspaceState(() {
      _groupEntries = [
        for (final group in _groupEntries)
          if (!ids.contains(group.id))
            if (ids.contains(group.parentId))
              HostGroup(
                id: group.id,
                uuid: group.uuid,
                name: group.name,
                createdAt: group.createdAt,
                updatedAt: group.updatedAt,
                version: group.version,
                createdDeviceId: group.createdDeviceId,
                updatedDeviceId: group.updatedDeviceId,
              )
            else
              group,
      ];
      _hostEntries = [
        for (final host in _hostEntries)
          if (ids.contains(host.groupId))
            _hostSummary(host, clearGroup: true)
          else
            host,
      ];
      _groups = _mapGroups(_groupEntries, _hostEntries);
      _hosts = _mapHosts(
        _hostEntries,
        _groupEntries,
        _identityEntries,
        _tagEntries,
      );
    });
  }

  Future<void> _deleteHost(_HostItem item) async {
    final confirmed = await _confirmDelete(
      title: 'Delete Host?',
      message: 'Delete "${item.name}"? This cannot be undone.',
    );
    if (!confirmed) {
      return;
    }
    _dataStore?.deleteHost(item.id);
    if (mounted) {
      _setWorkspaceState(() {
        _hostEntries = [
          for (final host in _hostEntries)
            if (host.id != item.id) host,
        ];
        _hosts = [
          for (final host in _hosts)
            if (host.id != item.id) host,
        ];
        _groups = _mapGroups(_groupEntries, _hostEntries);
        _portForwardEntries = [
          for (final entry in _portForwardEntries)
            if (entry.connectionId != item.id) entry,
        ];
        _portForwards = _portForwardEntries
            .map(
              (entry) => _mapPortForward(
                entry,
                _hostEntries,
                _runningPortForwardIds,
                _portForwardStatuses,
              ),
            )
            .toList(growable: false);
      });
    }
  }

  Future<void> _deleteHosts(List<_HostItem> items) async {
    final targets = {for (final item in items) item.id: item}.values.toList();
    if (targets.isEmpty) {
      return;
    }
    final confirmed = await _confirmDelete(
      title: 'Delete Hosts?',
      message: 'Delete ${targets.length} hosts? This cannot be undone.',
    );
    if (!confirmed) {
      return;
    }
    final ids = targets.map((item) => item.id).toSet();
    for (final id in ids) {
      _dataStore?.deleteHost(id);
    }
    if (mounted) {
      _setWorkspaceState(() {
        _hostEntries = [
          for (final host in _hostEntries)
            if (!ids.contains(host.id)) host,
        ];
        _hosts = [
          for (final host in _hosts)
            if (!ids.contains(host.id)) host,
        ];
        _groups = _mapGroups(_groupEntries, _hostEntries);
        _portForwardEntries = [
          for (final entry in _portForwardEntries)
            if (!ids.contains(entry.connectionId)) entry,
        ];
        _portForwards = _portForwardEntries
            .map(
              (entry) => _mapPortForward(
                entry,
                _hostEntries,
                _runningPortForwardIds,
                _portForwardStatuses,
              ),
            )
            .toList(growable: false);
      });
    }
  }

  void _handleKnownHostContextAction(
    _KnownHostItem item,
    _ContextMenuActionId action,
  ) {
    switch (action) {
      case _ContextMenuActionId.convertToHost:
        _convertKnownHostToHost(item);
        break;
      case _ContextMenuActionId.delete:
        unawaited(_deleteKnownHost(item));
        break;
      case _ContextMenuActionId.open:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.edit:
      case _ContextMenuActionId.duplicate:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  void _handleKnownHostContextActions(
    List<_KnownHostItem> items,
    _ContextMenuActionId action,
  ) {
    final targets = {for (final item in items) item.lineIndex: item}.values
        .toList();
    if (targets.length < 2) {
      if (targets.isNotEmpty) {
        _handleKnownHostContextAction(targets.single, action);
      }
      return;
    }
    if (action == _ContextMenuActionId.delete) {
      unawaited(_deleteKnownHosts(targets));
    }
  }

  Future<void> _importKnownHosts() async {
    final file = await _runExclusiveFilePicker(_pickKnownHostsImportFile);
    if (file == null) {
      return;
    }

    try {
      final importedText = await file.readAsString();
      final merge = _mergeKnownHostsText(_knownHostsText, importedText);
      if (merge.added == 0) {
        _showWorkspaceMessage('No new known hosts to import.');
        return;
      }

      await KnownHostsStore(NautermPaths.resolve().knownHostsFile)
          .writeText(merge.text);
      if (!mounted) {
        return;
      }
      _setWorkspaceState(() {
        _knownHostsText = merge.text;
      });
      _showWorkspaceMessage(
        'Imported ${merge.added} known ${merge.added == 1 ? 'host' : 'hosts'}.',
      );
    } catch (error) {
      _showWorkspaceMessage('Failed to import known hosts: $error');
    }
  }

  Future<T?> _runExclusiveFilePicker<T>(Future<T?> Function() pick) async {
    if (_filePickerInFlight) {
      return null;
    }
    _filePickerInFlight = true;
    try {
      return await pick();
    } finally {
      _filePickerInFlight = false;
    }
  }

  Future<io.File?> _pickKnownHostsImportFile() async {
    final defaultDirectory = _defaultSshDirectory();

    if (io.Platform.isMacOS) {
      final script = defaultDirectory == null
          ? 'POSIX path of (choose file with prompt "Import known_hosts")'
          : 'POSIX path of (choose file with prompt "Import known_hosts" default location POSIX file "${_escapeAppleScriptString(defaultDirectory.path)}")';
      final result = await io.Process.run('osascript', ['-e', script]);
      return _fileFromPickerResult(result);
    }

    if (io.Platform.isLinux) {
      final result = await io.Process.run('zenity', [
        '--file-selection',
        '--title=Import known_hosts',
        if (defaultDirectory != null)
          '--filename=${defaultDirectory.path}${io.Platform.pathSeparator}',
      ]);
      return _fileFromPickerResult(result);
    }

    if (io.Platform.isWindows) {
      final initialDirectory = defaultDirectory == null
          ? ''
          : '\$dialog.InitialDirectory = \'${_escapePowerShellSingleQuoted(defaultDirectory.path)}\'';
      final result = await io.Process.run('powershell', [
        '-NoProfile',
        '-Command',
        '''
Add-Type -AssemblyName System.Windows.Forms
\$dialog = New-Object System.Windows.Forms.OpenFileDialog
\$dialog.Title = "Import known_hosts"
${initialDirectory.isEmpty ? '' : initialDirectory}
if (\$dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  \$dialog.FileName
}
''',
      ]);
      return _fileFromPickerResult(result);
    }

    _showWorkspaceMessage('Import is not available on this platform.');
    return null;
  }

  io.Directory? _defaultSshDirectory() {
    final home = io.Platform.isWindows
        ? io.Platform.environment['USERPROFILE']
        : io.Platform.environment['HOME'];
    if (home == null || home.trim().isEmpty) {
      return null;
    }

    final directory = io.Directory('$home${io.Platform.pathSeparator}.ssh');
    return directory.existsSync() ? directory : null;
  }

  String _escapeAppleScriptString(String value) {
    return value.replaceAll(r'\', r'\\').replaceAll('"', r'\"');
  }

  String _escapePowerShellSingleQuoted(String value) {
    return value.replaceAll("'", "''");
  }

  io.File? _fileFromPickerResult(io.ProcessResult result) {
    if (result.exitCode != 0) {
      return null;
    }

    final path = result.stdout.toString().trim();
    if (path.isEmpty) {
      return null;
    }
    return io.File(path);
  }

  Future<void> _convertKnownHostToHost(_KnownHostItem item) async {
    if (!item.canConvertToHost) {
      _showWorkspaceMessage('This known host entry cannot be converted.');
      return;
    }

    final store = _dataStore;
    if (store == null) {
      return;
    }
    final id = store.saveHost(
      HostEntry(
        name: item.host!,
        host: item.host,
        port: item.port ?? 22,
        type: NautermHostType.remote,
      ),
    );
    final saved = store.getHost(id);
    if (saved != null && mounted) {
      _upsertHostSummary(saved);
    }
    _showWorkspaceMessage('Known host converted to host.');
  }

  Future<void> _deleteKnownHost(_KnownHostItem item) async {
    final confirmed = await _confirmDelete(
      title: 'Delete Known Host?',
      message: 'Delete "${item.name}" from known hosts?',
    );
    if (!confirmed) {
      return;
    }
    final lines = _knownHostsText.split('\n');
    if (item.lineIndex < 0 || item.lineIndex >= lines.length) {
      _showWorkspaceMessage('Known host entry is no longer available.');
      return;
    }

    lines.removeAt(item.lineIndex);
    final updatedText = lines.join('\n');
    await KnownHostsStore(NautermPaths.resolve().knownHostsFile)
        .writeText(updatedText);
    if (!mounted) {
      return;
    }
    _setWorkspaceState(() {
      _knownHostsText = updatedText;
    });
    _showWorkspaceMessage('Known host deleted.');
  }

  Future<void> _deleteKnownHosts(List<_KnownHostItem> items) async {
    final lineIndexes = items.map((item) => item.lineIndex).toSet();
    if (lineIndexes.isEmpty) {
      return;
    }
    final confirmed = await _confirmDelete(
      title: 'Delete Known Hosts?',
      message: 'Delete ${lineIndexes.length} entries from known hosts?',
    );
    if (!confirmed) {
      return;
    }
    final lines = _knownHostsText.split('\n');
    if (lineIndexes.any((index) => index < 0 || index >= lines.length)) {
      _showWorkspaceMessage('A known host entry is no longer available.');
      return;
    }
    final updatedText = [
      for (var index = 0; index < lines.length; index++)
        if (!lineIndexes.contains(index)) lines[index],
    ].join('\n');
    await KnownHostsStore(NautermPaths.resolve().knownHostsFile)
        .writeText(updatedText);
    if (!mounted) {
      return;
    }
    _setWorkspaceState(() {
      _knownHostsText = updatedText;
    });
    _showWorkspaceMessage('Known hosts deleted.');
  }

  void _handleKeyContextActions(
    List<_KeyItem> items,
    _ContextMenuActionId action,
  ) {
    final targets = {for (final item in items) item.id: item}.values.toList();
    if (targets.length < 2) {
      if (targets.isNotEmpty) {
        _handleKeyContextAction(targets.single, action);
      }
      return;
    }
    switch (action) {
      case _ContextMenuActionId.duplicate:
        for (final item in targets) {
          _handleKeyContextAction(item, action);
        }
        break;
      case _ContextMenuActionId.delete:
        unawaited(_deleteKeys(targets));
        break;
      case _ContextMenuActionId.open:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.edit:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  void _handleKeyContextAction(_KeyItem item, _ContextMenuActionId action) {
    switch (action) {
      case _ContextMenuActionId.edit:
        _editKey(item);
        break;
      case _ContextMenuActionId.duplicate:
        final key = _dataStore?.getKey(item.id);
        if (key != null) {
          final store = _dataStore!;
          final id = store.saveKey(
            KeyEntry(
              name: '${key.name} copy',
              privateKey: key.privateKey,
              publicKey: key.publicKey,
              certificate: key.certificate,
            ),
          );
          final saved = store.getKey(id);
          if (saved != null) {
            _setWorkspaceState(() {
              _keyEntries = [
                ..._keyEntries,
                KeyEntry(
                  id: saved.id,
                  uuid: saved.uuid,
                  name: saved.name,
                  publicKey: saved.publicKey,
                  certificate: _sshCertificateSummaryMarker(saved.certificate),
                  createdAt: saved.createdAt,
                  updatedAt: saved.updatedAt,
                  version: saved.version,
                ),
              ];
              _keys = _keyEntries.map(_mapKey).toList(growable: false);
            });
          }
        }
        break;
      case _ContextMenuActionId.exportToHost:
        _exportKey(item);
        break;
      case _ContextMenuActionId.exportToFile:
        final key = _dataStore?.getKey(item.id);
        if (key == null) {
          _showWorkspaceMessage('Key is not available.');
          break;
        }
        unawaited(_exportKeyToFile(key));
        break;
      case _ContextMenuActionId.delete:
        unawaited(_deleteKey(item));
        break;
      case _ContextMenuActionId.open:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
        break;
    }
  }

  Future<void> _deleteKey(_KeyItem item) async {
    final confirmed = await _confirmDelete(
      title: 'Delete Key?',
      message: 'Delete "${item.name}"? This cannot be undone.',
    );
    if (!confirmed) {
      return;
    }
    _dataStore?.deleteKey(item.id);
    if (mounted) {
      _setWorkspaceState(() {
        _keyEntries = [
          for (final key in _keyEntries)
            if (key.id != item.id) key,
        ];
        _keys = _keyEntries.map(_mapKey).toList(growable: false);
      });
    }
  }

  Future<void> _deleteKeys(List<_KeyItem> items) async {
    final ids = items.map((item) => item.id).toSet();
    if (ids.isEmpty) {
      return;
    }
    final confirmed = await _confirmDelete(
      title: 'Delete Keys?',
      message: 'Delete ${ids.length} keys? This cannot be undone.',
    );
    if (!confirmed) {
      return;
    }
    for (final id in ids) {
      _dataStore?.deleteKey(id);
    }
    if (!mounted) {
      return;
    }
    _setWorkspaceState(() {
      _keyEntries = [
        for (final key in _keyEntries)
          if (!ids.contains(key.id)) key,
      ];
      _keys = _keyEntries.map(_mapKey).toList(growable: false);
    });
  }

  void _handleIdentityContextActions(
    List<_IdentityItem> items,
    _ContextMenuActionId action,
  ) {
    final targets = {for (final item in items) item.id: item}.values.toList();
    if (targets.length < 2) {
      if (targets.isNotEmpty) {
        _handleIdentityContextAction(targets.single, action);
      }
      return;
    }
    switch (action) {
      case _ContextMenuActionId.duplicate:
        for (final item in targets) {
          _handleIdentityContextAction(item, action);
        }
        break;
      case _ContextMenuActionId.delete:
        unawaited(_deleteIdentities(targets));
        break;
      case _ContextMenuActionId.open:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.edit:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  void _handleIdentityContextAction(
    _IdentityItem item,
    _ContextMenuActionId action,
  ) {
    switch (action) {
      case _ContextMenuActionId.edit:
        _editIdentity(item);
        break;
      case _ContextMenuActionId.duplicate:
        final identity = _dataStore?.getIdentity(item.id);
        if (identity != null) {
          final store = _dataStore!;
          final id = store.saveIdentity(
            IdentityEntry(
              name: '${identity.name} copy',
              username: identity.username,
              password: identity.password,
              keyId: identity.keyId,
            ),
          );
          final saved = store.getIdentity(id);
          if (saved != null) {
            _setWorkspaceState(() {
              _identityEntries = [
                ..._identityEntries,
                IdentityEntry(
                  id: saved.id,
                  uuid: saved.uuid,
                  name: saved.name,
                  username: saved.username,
                  keyId: saved.keyId,
                  keyUuid: saved.keyUuid,
                  createdAt: saved.createdAt,
                  updatedAt: saved.updatedAt,
                  version: saved.version,
                ),
              ];
              _identities = _identityEntries
                  .map(_mapIdentity)
                  .toList(growable: false);
            });
          }
        }
        break;
      case _ContextMenuActionId.delete:
        unawaited(_deleteIdentity(item));
        break;
      case _ContextMenuActionId.open:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  Future<void> _deleteIdentity(_IdentityItem item) async {
    final confirmed = await _confirmDelete(
      title: 'Delete Identity?',
      message: 'Delete "${item.name}"? This cannot be undone.',
    );
    if (!confirmed) {
      return;
    }
    _dataStore?.deleteIdentity(item.id);
    if (mounted) {
      _setWorkspaceState(() {
        _identityEntries = [
          for (final identity in _identityEntries)
            if (identity.id != item.id) identity,
        ];
        _identities = _identityEntries
            .map(_mapIdentity)
            .toList(growable: false);
        _hosts = _mapHosts(
          _hostEntries,
          _groupEntries,
          _identityEntries,
          _tagEntries,
        );
      });
    }
  }

  Future<void> _deleteIdentities(List<_IdentityItem> items) async {
    final ids = items.map((item) => item.id).toSet();
    if (ids.isEmpty) {
      return;
    }
    final confirmed = await _confirmDelete(
      title: 'Delete Identities?',
      message: 'Delete ${ids.length} identities? This cannot be undone.',
    );
    if (!confirmed) {
      return;
    }
    for (final id in ids) {
      _dataStore?.deleteIdentity(id);
    }
    if (!mounted) {
      return;
    }
    _setWorkspaceState(() {
      _identityEntries = [
        for (final identity in _identityEntries)
          if (!ids.contains(identity.id)) identity,
      ];
      _identities = _identityEntries.map(_mapIdentity).toList(growable: false);
      _hosts = _mapHosts(
        _hostEntries,
        _groupEntries,
        _identityEntries,
        _tagEntries,
      );
    });
  }

  void _handleProxyContextAction(_ProxyItem item, _ContextMenuActionId action) {
    switch (action) {
      case _ContextMenuActionId.edit:
        _editProxy(item);
        break;
      case _ContextMenuActionId.duplicate:
        final id = item.id;
        final proxy =
            _dataStore?.getProxy(id) ??
            _proxyEntries.where((proxy) => proxy.id == id).firstOrNull;
        if (proxy != null) {
          final store = _dataStore!;
          final id = store.saveProxy(
            ProxyEntry(
              name: '${proxy.name} copy',
              type: proxy.type,
              host: proxy.host,
              port: proxy.port,
              identityId: proxy.identityId,
              username: proxy.username,
              password: proxy.password,
            ),
          );
          final saved = store.getProxy(id);
          if (saved != null) {
            _setWorkspaceState(() {
              _proxyEntries = [
                ..._proxyEntries,
                ProxyEntry(
                  id: saved.id,
                  uuid: saved.uuid,
                  name: saved.name,
                  type: saved.type,
                  host: saved.host,
                  port: saved.port,
                  identityId: saved.identityId,
                  identityUuid: saved.identityUuid,
                  username: saved.username,
                  createdAt: saved.createdAt,
                  updatedAt: saved.updatedAt,
                  version: saved.version,
                ),
              ];
              _proxies = _proxyEntries
                  .map((entry) => _mapProxy(entry, _identityEntries))
                  .toList(growable: false);
            });
          }
        }
        break;
      case _ContextMenuActionId.delete:
        unawaited(_deleteProxyItem(item));
        break;
      case _ContextMenuActionId.open:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  void _handleProxyContextActions(
    List<_ProxyItem> items,
    _ContextMenuActionId action,
  ) {
    final targets = {for (final item in items) item.id: item}.values.toList();
    if (targets.length < 2) {
      if (targets.isNotEmpty) {
        _handleProxyContextAction(targets.single, action);
      }
      return;
    }
    switch (action) {
      case _ContextMenuActionId.duplicate:
        for (final item in targets) {
          _handleProxyContextAction(item, action);
        }
        break;
      case _ContextMenuActionId.delete:
        unawaited(_deleteProxies(targets));
        break;
      case _ContextMenuActionId.open:
      case _ContextMenuActionId.run:
      case _ContextMenuActionId.hostWithSsh:
      case _ContextMenuActionId.hostWithMosh:
      case _ContextMenuActionId.hostWithTelnet:
      case _ContextMenuActionId.hostWithSftp:
      case _ContextMenuActionId.openInCurrentWorkspace:
      case _ContextMenuActionId.openInNewWorkspace:
      case _ContextMenuActionId.convertToHost:
      case _ContextMenuActionId.edit:
      case _ContextMenuActionId.newHostInGroup:
      case _ContextMenuActionId.copySshCommand:
      case _ContextMenuActionId.exportToHost:
      case _ContextMenuActionId.exportToFile:
        break;
    }
  }

  Future<void> _deleteProxyItem(_ProxyItem item) async {
    await _confirmAndDeleteProxy(item.id, item.name);
  }

  Future<void> _deleteProxies(List<_ProxyItem> items) async {
    final ids = items.map((item) => item.id).where((id) => id > 0).toSet();
    if (ids.isEmpty) {
      return;
    }
    final confirmed = await _confirmDelete(
      title: 'Delete Proxies?',
      message: 'Delete ${ids.length} proxies? This cannot be undone.',
    );
    if (!confirmed) {
      return;
    }
    for (final id in ids) {
      _dataStore?.deleteProxy(id);
    }
    if (!mounted) {
      return;
    }
    _setWorkspaceState(() {
      _proxyEntries = [
        for (final proxy in _proxyEntries)
          if (!ids.contains(proxy.id)) proxy,
      ];
      _proxies = _proxyEntries
          .map((proxy) => _mapProxy(proxy, _identityEntries))
          .toList(growable: false);
    });
  }

  Future<void> _confirmAndDeleteProxy(int? id, String name) async {
    if (id == null || id <= 0) {
      return;
    }
    final confirmed = await _confirmDelete(
      title: 'Delete Proxy?',
      message: 'Delete "$name"? This cannot be undone.',
    );
    if (!confirmed) {
      return;
    }
    _dataStore?.deleteProxy(id);
    if (mounted) {
      _setWorkspaceState(() {
        _proxyEntries = [
          for (final proxy in _proxyEntries)
            if (proxy.id != id) proxy,
        ];
        _proxies = _proxyEntries
            .map((proxy) => _mapProxy(proxy, _identityEntries))
            .toList(growable: false);
      });
    }
  }

  void _copySshCommand(_HostItem item) {
    final host = _hostEntries.where((host) => host.id == item.id).firstOrNull;
    final address = _emptyToNull(host?.host);
    if (host == null ||
        address == null ||
        host.type != NautermHostType.remote) {
      _showWorkspaceMessage('SSH command is not available.');
      return;
    }

    final identity = host.identityId == null
        ? null
        : _identityEntries
              .where((identity) => identity.id == host.identityId)
              .firstOrNull;
    final username = identity == null
        ? _firstNonEmpty([host.username])
        : _firstNonEmpty([identity.username]);
    final destination = username == null ? address : '$username@$address';
    final port = host.port ?? 22;
    final command = port == 22
        ? 'ssh $destination'
        : 'ssh -p $port $destination';

    Clipboard.setData(ClipboardData(text: command));
    _showWorkspaceMessage('SSH command copied.');
  }
}

bool _samePortForwardStatus(
  FfiPortForwardStatus? left,
  FfiPortForwardStatus right,
) {
  return left != null &&
      left.id == right.id &&
      left.state == right.state &&
      left.error == right.error &&
      left.boundPort == right.boundPort &&
      left.activeConnections == right.activeConnections;
}
