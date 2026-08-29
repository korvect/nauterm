part of 'nauterm_workspace.dart';

extension _NautermWorkspaceCompletionMethods on _NautermWorkspaceState {
  List<String> _composerDirectorySuggestions(
    _TerminalViewEntry view,
    String input,
    int limit,
  ) {
    final sshProfile = view.controller.sshProfile;
    if (sshProfile != null) {
      final cdSuggestions = _sshCdDirectorySuggestions(
        paneId: view.id,
        profile: sshProfile,
        snapshot: view.controller.snapshot,
        input: input,
        limit: limit,
      );
      if (cdSuggestions.isNotEmpty) {
        return cdSuggestions;
      }
      return _sshPathSuggestions(
        paneId: view.id,
        profile: sshProfile,
        snapshot: view.controller.snapshot,
        input: input,
        limit: limit,
      );
    }
    if (view.controller.serialProfile != null) {
      return const [];
    }
    final workingDirectory = _promptWorkingDirectoryFromSnapshot(
      view.controller.snapshot,
      requireLocalDirectory: true,
      expandHome: true,
    );
    if (workingDirectory == null) {
      return const [];
    }
    final zshSuggestions = _localZshCompletionSuggestions(
      view.controller,
      input,
      workingDirectory: workingDirectory,
      limit: limit,
    );
    if (zshSuggestions.isNotEmpty) {
      return zshSuggestions;
    }
    final shellSuggestions = _localShellCommandSuggestions(
      view.controller,
      input,
      workingDirectory: workingDirectory,
      limit: limit,
    );
    if (shellSuggestions.isNotEmpty) {
      return shellSuggestions;
    }
    final cdSuggestions = _cdDirectorySuggestions(
      input,
      workingDirectory: workingDirectory,
      limit: limit,
    );
    if (cdSuggestions.isNotEmpty) {
      return cdSuggestions;
    }
    return _localShellPathSuggestions(
      input,
      workingDirectory: workingDirectory,
      limit: limit,
    );
  }

  List<String> _localZshCompletionSuggestions(
    TerminalController controller,
    String input, {
    required String workingDirectory,
    required int limit,
  }) {
    if (input.trim().isEmpty || input.contains('\n')) {
      return const [];
    }
    final shellPath =
        _emptyToNull(controller.shellPath) ??
        _emptyToNull(io.Platform.environment['SHELL']);
    if (shellPath == null || _shellCompletionKind(shellPath) != 'zsh') {
      return const [];
    }

    final cacheKey = [
      'zsh-full',
      shellPath,
      workingDirectory,
      input,
    ].join('\u{1f}');
    final cached = _localZshCompletionCache[cacheKey];
    if (cached != null) {
      return cached.take(limit).toList(growable: false);
    }

    _scheduleLocalZshCompletion(
      shellPath: shellPath,
      workingDirectory: workingDirectory,
      input: input,
      cacheKey: cacheKey,
      limit: limit,
    );
    return const [];
  }

  List<String> _localShellCommandSuggestions(
    TerminalController controller,
    String input, {
    required String workingDirectory,
    required int limit,
  }) {
    final shellPath =
        _emptyToNull(controller.shellPath) ??
        _emptyToNull(io.Platform.environment['SHELL']);
    if (shellPath == null) {
      return const [];
    }

    final shellKind = _shellCompletionKind(shellPath);
    if (shellKind == null) {
      return const [];
    }

    final query = WorkspaceShellCommandCompletionQuery.tryParse(input);
    if (query == null) {
      return const [];
    }

    final cacheKey = [
      shellKind,
      shellPath,
      workingDirectory,
      query.prefix,
    ].join('\u{1f}');
    final cachedNames = _localShellCommandCompletionCache[cacheKey];
    if (cachedNames != null) {
      return _shellCommandCandidatesFromNames(query, cachedNames, limit: limit);
    }

    _scheduleLocalShellCommandCompletion(
      shellKind: shellKind,
      shellPath: shellPath,
      workingDirectory: workingDirectory,
      prefix: query.prefix,
      cacheKey: cacheKey,
    );
    return const [];
  }

  String? _shellCompletionKind(String shellPath) {
    final name = _pathBaseName(shellPath).toLowerCase();
    if (name == 'zsh') {
      return 'zsh';
    }
    if (name == 'bash' || name == 'bash.exe') {
      return 'bash';
    }
    return null;
  }

  List<String> _shellCommandCandidatesFromNames(
    WorkspaceShellCommandCompletionQuery query,
    Iterable<String> names, {
    required int limit,
  }) {
    return WorkspaceComposerCompletion.commandCandidates(
      query,
      names,
      limit: limit,
    );
  }

  void _scheduleLocalZshCompletion({
    required String shellPath,
    required String workingDirectory,
    required String input,
    required String cacheKey,
    required int limit,
  }) {
    final scopeKey = [shellPath, workingDirectory].join('\u{1f}');
    _activeLocalZshCompletionKeys[scopeKey] = cacheKey;
    if (_localZshCompletionCache.containsKey(cacheKey) ||
        _localZshCompletionRequests.contains(cacheKey)) {
      return;
    }

    _localZshCompletionDebounceTimers.remove(scopeKey)?.cancel();
    _localZshCompletionDebounceTimers[scopeKey] = Timer(
      _NautermWorkspaceState._completionDebounceDuration,
      () {
        _localZshCompletionDebounceTimers.remove(scopeKey);
        if (!mounted ||
            _activeLocalZshCompletionKeys[scopeKey] != cacheKey ||
            _localZshCompletionCache.containsKey(cacheKey) ||
            !_localZshCompletionRequests.add(cacheKey)) {
          return;
        }
        final request =
            _runLocalZshCompletion(
                  shellPath: shellPath,
                  workingDirectory: workingDirectory,
                  input: input,
                  limit: limit,
                )
                .then((candidates) {
                  if (!mounted) {
                    return;
                  }
                  _setWorkspaceState(() {
                    _localZshCompletionRequests.remove(cacheKey);
                    _localZshCompletionCache[cacheKey] = candidates;
                  });
                })
                .catchError((_) {
                  if (!mounted) {
                    return;
                  }
                  _setWorkspaceState(() {
                    _localZshCompletionRequests.remove(cacheKey);
                    if (_activeLocalZshCompletionKeys[scopeKey] == cacheKey) {
                      _localZshCompletionCache[cacheKey] = const [];
                    }
                  });
                });
        unawaited(request);
      },
    );
  }

  void _scheduleLocalShellCommandCompletion({
    required String shellKind,
    required String shellPath,
    required String workingDirectory,
    required String prefix,
    required String cacheKey,
  }) {
    if (!_localShellCommandCompletionRequests.add(cacheKey)) {
      return;
    }

    final request =
        _runLocalShellCommandCompletion(
              shellKind: shellKind,
              shellPath: shellPath,
              workingDirectory: workingDirectory,
              prefix: prefix,
            )
            .then((names) {
              if (!mounted) {
                return;
              }
              _setWorkspaceState(() {
                _localShellCommandCompletionRequests.remove(cacheKey);
                _localShellCommandCompletionCache[cacheKey] = names;
              });
            })
            .catchError((error) {
              if (!mounted) {
                return;
              }
              _setWorkspaceState(() {
                _localShellCommandCompletionRequests.remove(cacheKey);
                _localShellCommandCompletionCache[cacheKey] = const [];
              });
              if (_localShellCommandCompletionErrorsShown.add(cacheKey)) {
                _showWorkspaceMessage(
                  'Shell completion failed: $error',
                  type: _WorkspaceNotificationType.error,
                );
              }
            });
    unawaited(request);
  }

  List<String> _localShellPathSuggestions(
    String input, {
    required String workingDirectory,
    required int limit,
  }) {
    final query = _shellPathCompletionQuery(
      input,
      workingDirectory: workingDirectory,
      expandHome: true,
    );
    if (query == null) {
      return const [];
    }

    final directory = io.Directory(query.directoryPath);
    if (!directory.existsSync()) {
      return const [];
    }

    final entries = <WorkspacePathCompletionEntry>[];
    try {
      for (final entity in directory.listSync(followLinks: false)) {
        final name = _pathBaseName(entity.path);
        if (name.isEmpty) {
          continue;
        }
        entries.add(
          WorkspacePathCompletionEntry(
            name: name,
            isDirectory: io.FileSystemEntity.isDirectorySync(entity.path),
          ),
        );
      }
    } on io.FileSystemException {
      return const [];
    }

    return _pathCandidatesFromEntries(query, entries, limit: limit);
  }

  WorkspaceShellPathCompletionQuery? _shellPathCompletionQuery(
    String input, {
    required String workingDirectory,
    required bool expandHome,
  }) {
    return WorkspaceComposerCompletion.shellPathQuery(
      input,
      workingDirectory: workingDirectory,
      expandHome: expandHome,
      home: io.Platform.environment['HOME'],
    );
  }

  List<String> _pathCandidatesFromEntries(
    WorkspaceShellPathCompletionQuery query,
    Iterable<WorkspacePathCompletionEntry> entries, {
    required int limit,
  }) {
    return WorkspaceComposerCompletion.pathCandidates(
      query,
      entries,
      limit: limit,
    );
  }

  List<String> _cdDirectorySuggestions(
    String input, {
    required String workingDirectory,
    required int limit,
  }) {
    final query = _cdCompletionQuery(
      input,
      workingDirectory: workingDirectory,
      expandHome: true,
    );
    if (query == null) {
      return const [];
    }

    final directory = io.Directory(query.directoryPath);
    if (!directory.existsSync()) {
      return const [];
    }

    final names = <String>[];
    try {
      for (final entity in directory.listSync(followLinks: false)) {
        if (!io.FileSystemEntity.isDirectorySync(entity.path)) {
          continue;
        }
        final name = _pathBaseName(entity.path);
        if (name.isNotEmpty) {
          names.add(name);
        }
      }
    } on io.FileSystemException {
      return query.parentArgument.isEmpty &&
              (query.namePrefix.isEmpty || '..'.startsWith(query.namePrefix))
          ? const ['cd ..']
          : const [];
    }

    return _directoryCandidatesFromNames(query, names, limit: limit);
  }

  List<String> _sshCdDirectorySuggestions({
    required int paneId,
    required SshConnectionProfile profile,
    required TerminalSnapshot snapshot,
    required String input,
    required int limit,
  }) {
    final workingDirectory = _sshWorkingDirectoryForPane(paneId, snapshot);
    if (workingDirectory == null) {
      return const [];
    }
    final query = _cdCompletionQuery(
      input,
      workingDirectory: workingDirectory,
      expandHome: false,
    );
    if (query == null) {
      return const [];
    }

    final cacheKey = _sshCompletionCacheKey(
      paneId,
      profile,
      query.directoryPath,
    );
    final cachedNames = _sshDirectoryCompletionCache[cacheKey];
    if (cachedNames != null) {
      return _directoryCandidatesFromNames(query, cachedNames, limit: limit);
    }

    _scheduleSshDirectoryCompletion(
      paneId: paneId,
      profile: profile,
      cacheKey: cacheKey,
      directory: query.directoryPath,
    );
    return query.parentArgument.isEmpty &&
            (query.namePrefix.isEmpty || '..'.startsWith(query.namePrefix))
        ? const ['cd ..']
        : const [];
  }

  List<String> _sshPathSuggestions({
    required int paneId,
    required SshConnectionProfile profile,
    required TerminalSnapshot snapshot,
    required String input,
    required int limit,
  }) {
    final workingDirectory = _sshWorkingDirectoryForPane(paneId, snapshot);
    if (workingDirectory == null) {
      return const [];
    }
    final query = _shellPathCompletionQuery(
      input,
      workingDirectory: workingDirectory,
      expandHome: false,
    );
    if (query == null) {
      return const [];
    }

    final cacheKey = _sshCompletionCacheKey(
      paneId,
      profile,
      query.directoryPath,
    );
    final cachedEntries = _sshPathCompletionCache[cacheKey];
    if (cachedEntries != null) {
      return _pathCandidatesFromEntries(query, cachedEntries, limit: limit);
    }

    _scheduleSshPathCompletion(
      paneId: paneId,
      profile: profile,
      cacheKey: cacheKey,
      directory: query.directoryPath,
    );
    return const [];
  }

  _CdCompletionQuery? _cdCompletionQuery(
    String input, {
    required String workingDirectory,
    required bool expandHome,
  }) {
    final match = RegExp(r'^\s*cd(?:\s+(.*))?$').firstMatch(input);
    if (match == null) {
      return null;
    }

    final rawPath = match.group(1) ?? '';
    if (rawPath.contains('\n') ||
        rawPath.contains(';') ||
        rawPath.contains('|') ||
        rawPath.contains('&') ||
        rawPath.startsWith('"') ||
        rawPath.startsWith("'")) {
      return null;
    }

    final unescapedPath = _unescapeShellPath(rawPath);
    final separatorIndex = unescapedPath.lastIndexOf('/');
    final parentArgument = separatorIndex == -1
        ? ''
        : unescapedPath.substring(0, separatorIndex + 1);
    final namePrefix = separatorIndex == -1
        ? unescapedPath
        : unescapedPath.substring(separatorIndex + 1);
    final directoryPath = _resolveShellPath(
      parentArgument,
      workingDirectory: workingDirectory,
      expandHome: expandHome,
    );
    if (directoryPath == null) {
      return null;
    }

    return _CdCompletionQuery(
      directoryPath: directoryPath,
      parentArgument: parentArgument,
      namePrefix: namePrefix,
    );
  }

  List<String> _directoryCandidatesFromNames(
    _CdCompletionQuery query,
    Iterable<String> names, {
    required int limit,
  }) {
    final candidates = <String>[];
    if (query.parentArgument.isEmpty &&
        (query.namePrefix.isEmpty || '..'.startsWith(query.namePrefix))) {
      candidates.add('cd ..');
      if (candidates.length >= limit) {
        return candidates;
      }
    }

    final includeHidden = query.namePrefix.startsWith('.');
    final directories = [
      for (final name in names)
        if (name.isNotEmpty &&
            (includeHidden || !name.startsWith('.')) &&
            name.toLowerCase().startsWith(query.namePrefix.toLowerCase()))
          name,
    ]..sort((a, b) => a.toLowerCase().compareTo(b.toLowerCase()));

    for (final name in directories) {
      final argument = '${query.parentArgument}$name/';
      candidates.add('cd ${_escapeShellPath(argument)}');
      if (candidates.length >= limit) {
        break;
      }
    }
    return candidates;
  }

  String _sshCompletionCacheKey(
    int paneId,
    SshConnectionProfile profile,
    String directory,
  ) {
    return [
      paneId,
      profile.host,
      profile.port,
      profile.username,
      directory,
    ].join('\u{1f}');
  }

  void _scheduleSshDirectoryCompletion({
    required int paneId,
    required SshConnectionProfile profile,
    required String cacheKey,
    required String directory,
  }) {
    _activeSshDirectoryCompletionKeys[paneId] = cacheKey;
    if (_sshDirectoryCompletionCache.containsKey(cacheKey) ||
        _sshDirectoryCompletionRequests.contains(cacheKey)) {
      return;
    }

    _sshDirectoryCompletionDebounceTimers.remove(paneId)?.cancel();
    _sshDirectoryCompletionDebounceTimers[paneId] = Timer(
      _NautermWorkspaceState._completionDebounceDuration,
      () {
        _sshDirectoryCompletionDebounceTimers.remove(paneId);
        if (!mounted ||
            _activeSshDirectoryCompletionKeys[paneId] != cacheKey ||
            _sshDirectoryCompletionCache.containsKey(cacheKey) ||
            !_sshDirectoryCompletionRequests.add(cacheKey)) {
          return;
        }
        _runSshDirectoryCompletion(
          paneId: paneId,
          profile: profile,
          cacheKey: cacheKey,
          directory: directory,
        );
      },
    );
  }

  void _runSshDirectoryCompletion({
    required int paneId,
    required SshConnectionProfile profile,
    required String cacheKey,
    required String directory,
  }) {
    final arguments = <String, Object?>{
      'host': profile.host,
      'port': profile.port,
      'username': profile.username,
      'knownHostsPath': profile.knownHostsPath,
      'directory': directory,
      'password': profile.password,
      'privateKey': profile.privateKey,
      'certificate': profile.certificate,
      'passphrase': profile.passphrase,
    };
    final request = _spawnSshDirectoryListing(arguments)
        .then((result) {
          if (!mounted) {
            return;
          }
          final isActive =
              _activeSshDirectoryCompletionKeys[paneId] == cacheKey;
          _setWorkspaceState(() {
            _sshDirectoryCompletionRequests.remove(cacheKey);
            if (!result.isError || isActive) {
              _sshDirectoryCompletionCache[cacheKey] = result.entries;
              _acceptSshResolvedDirectory(
                paneId: paneId,
                profile: profile,
                resolvedDirectory: result.resolvedDirectory,
                cacheEntries: result.entries,
              );
            }
          });
          if (isActive &&
              result.isError &&
              _sshDirectoryCompletionErrorsShown.add(cacheKey)) {
            _showWorkspaceMessage(
              'SSH completion failed: ${result.error}',
              type: _WorkspaceNotificationType.error,
            );
          }
        })
        .catchError((error) {
          if (!mounted) {
            return;
          }
          final isActive =
              _activeSshDirectoryCompletionKeys[paneId] == cacheKey;
          _setWorkspaceState(() {
            _sshDirectoryCompletionRequests.remove(cacheKey);
            if (isActive) {
              _sshDirectoryCompletionCache[cacheKey] = const [];
            }
          });
          if (isActive && _sshDirectoryCompletionErrorsShown.add(cacheKey)) {
            _showWorkspaceMessage(
              'SSH completion failed: $error',
              type: _WorkspaceNotificationType.error,
            );
          }
        });
    unawaited(request);
  }

  void _scheduleSshPathCompletion({
    required int paneId,
    required SshConnectionProfile profile,
    required String cacheKey,
    required String directory,
  }) {
    _activeSshPathCompletionKeys[paneId] = cacheKey;
    if (_sshPathCompletionCache.containsKey(cacheKey) ||
        _sshPathCompletionRequests.contains(cacheKey)) {
      return;
    }

    _sshPathCompletionDebounceTimers.remove(paneId)?.cancel();
    _sshPathCompletionDebounceTimers[paneId] = Timer(
      _NautermWorkspaceState._completionDebounceDuration,
      () {
        _sshPathCompletionDebounceTimers.remove(paneId);
        if (!mounted ||
            _activeSshPathCompletionKeys[paneId] != cacheKey ||
            _sshPathCompletionCache.containsKey(cacheKey) ||
            !_sshPathCompletionRequests.add(cacheKey)) {
          return;
        }
        _runSshPathCompletion(
          paneId: paneId,
          profile: profile,
          cacheKey: cacheKey,
          directory: directory,
        );
      },
    );
  }

  void _runSshPathCompletion({
    required int paneId,
    required SshConnectionProfile profile,
    required String cacheKey,
    required String directory,
  }) {
    final arguments = <String, Object?>{
      'host': profile.host,
      'port': profile.port,
      'username': profile.username,
      'knownHostsPath': profile.knownHostsPath,
      'directory': directory,
      'password': profile.password,
      'privateKey': profile.privateKey,
      'certificate': profile.certificate,
      'passphrase': profile.passphrase,
    };
    final request = _spawnSshDirectoryEntryListing(arguments)
        .then((result) {
          if (!mounted) {
            return;
          }
          final isActive = _activeSshPathCompletionKeys[paneId] == cacheKey;
          _setWorkspaceState(() {
            _sshPathCompletionRequests.remove(cacheKey);
            if (!result.isError || isActive) {
              final entries = [
                for (final entry in result.entries)
                  WorkspacePathCompletionEntry(
                    name: entry.name,
                    isDirectory: entry.isDirectory,
                  ),
              ];
              _sshPathCompletionCache[cacheKey] = entries;
              _acceptSshResolvedDirectory(
                paneId: paneId,
                profile: profile,
                resolvedDirectory: result.resolvedDirectory,
                cacheEntries: entries,
              );
            }
          });
          if (isActive &&
              result.isError &&
              _sshPathCompletionErrorsShown.add(cacheKey)) {
            _showWorkspaceMessage(
              'SSH completion failed: ${result.error}',
              type: _WorkspaceNotificationType.error,
            );
          }
        })
        .catchError((error) {
          if (!mounted) {
            return;
          }
          final isActive = _activeSshPathCompletionKeys[paneId] == cacheKey;
          _setWorkspaceState(() {
            _sshPathCompletionRequests.remove(cacheKey);
            if (isActive) {
              _sshPathCompletionCache[cacheKey] = const [];
            }
          });
          if (isActive && _sshPathCompletionErrorsShown.add(cacheKey)) {
            _showWorkspaceMessage(
              'SSH completion failed: $error',
              type: _WorkspaceNotificationType.error,
            );
          }
        });
    unawaited(request);
  }

  void _acceptSshResolvedDirectory({
    required int paneId,
    required SshConnectionProfile profile,
    required String? resolvedDirectory,
    required Object cacheEntries,
  }) {
    final directory = resolvedDirectory?.trim();
    if (directory == null || directory.isEmpty) {
      return;
    }

    _sshWorkingDirectories[paneId] = directory;
    _pendingSshWorkingDirectories.remove(paneId);

    final resolvedCacheKey = _sshCompletionCacheKey(paneId, profile, directory);
    if (cacheEntries is List<String>) {
      _sshDirectoryCompletionCache[resolvedCacheKey] = cacheEntries;
    } else if (cacheEntries is List<WorkspacePathCompletionEntry>) {
      _sshPathCompletionCache[resolvedCacheKey] = cacheEntries;
    }
  }

  String? _sshWorkingDirectoryForPane(int paneId, TerminalSnapshot snapshot) {
    final promptDirectory = _promptWorkingDirectoryFromSnapshot(
      snapshot,
      requireLocalDirectory: false,
      expandHome: false,
    );
    if (promptDirectory != null) {
      _sshWorkingDirectories[paneId] = promptDirectory;
      _pendingSshWorkingDirectories.remove(paneId);
      return promptDirectory;
    }
    final promptToken = _promptDirectoryTokenFromSnapshot(snapshot);
    final pendingDirectory = _pendingSshWorkingDirectories[paneId];
    if (promptToken != null &&
        pendingDirectory != null &&
        _remotePathBaseName(pendingDirectory) == promptToken) {
      _pendingSshWorkingDirectories.remove(paneId);
      _sshWorkingDirectories[paneId] = pendingDirectory;
      return pendingDirectory;
    }
    final trackedDirectory = _sshWorkingDirectories[paneId];
    if (promptToken != null &&
        trackedDirectory != null &&
        _remotePathBaseName(trackedDirectory) == promptToken) {
      return trackedDirectory;
    }
    return _sshWorkingDirectories[paneId] ?? '~';
  }

  void _trackSshPaneInput(int paneId, String data) {
    if (data.contains('\x1b')) {
      return;
    }
    var buffer = _sshInputBuffers[paneId] ?? '';
    for (final rune in data.runes) {
      if (rune == 0x0d || rune == 0x0a) {
        _applySshPaneCommand(paneId, buffer.trim());
        buffer = '';
        continue;
      }
      if (rune == 0x08 || rune == 0x7f) {
        if (buffer.isNotEmpty) {
          buffer = buffer.substring(0, buffer.length - 1);
        }
        continue;
      }
      if (rune >= 0x20) {
        buffer += String.fromCharCode(rune);
      }
    }
    if (buffer.isEmpty) {
      _sshInputBuffers.remove(paneId);
    } else {
      _sshInputBuffers[paneId] = buffer;
    }
  }

  void _applySshPaneCommand(int paneId, String command) {
    final match = RegExp(r'^\s*cd(?:\s+(.+))?\s*$').firstMatch(command);
    if (match == null) {
      return;
    }
    final rawArgument = (match.group(1) ?? '~').trim();
    if (rawArgument.isEmpty ||
        rawArgument == '-' ||
        rawArgument.contains(';') ||
        rawArgument.contains('|') ||
        rawArgument.contains('&')) {
      return;
    }
    final argument = _unquoteSimpleShellPath(rawArgument);
    if (argument == null) {
      return;
    }
    final currentDirectory = _sshWorkingDirectories[paneId] ?? '~';
    _pendingSshWorkingDirectories[paneId] = _normalizeRemoteShellPath(
      _resolveShellPath(
        argument,
        workingDirectory: currentDirectory,
        expandHome: false,
      )!,
    );
  }

  String? _promptDirectoryTokenFromSnapshot(TerminalSnapshot snapshot) {
    if (snapshot.rows <= 0 || snapshot.columns <= 0) {
      return null;
    }

    final cursorRow = snapshot.cursor.row.clamp(0, snapshot.rows - 1);
    final firstRow = math.max(0, cursorRow - 2);
    for (var row = cursorRow; row >= firstRow; row--) {
      final token = _extractWorkingDirectoryFromPromptLine(
        _snapshotLineText(snapshot, row),
        allowRelative: true,
      );
      if (token != null && token.isNotEmpty) {
        return token;
      }
    }
    return null;
  }

  String? _unquoteSimpleShellPath(String value) {
    if (value.length >= 2 && value.startsWith("'") && value.endsWith("'")) {
      final inner = value.substring(1, value.length - 1);
      if (inner.contains("'")) {
        return null;
      }
      return inner;
    }
    if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) {
      final inner = value.substring(1, value.length - 1);
      if (inner.contains('"')) {
        return null;
      }
      return _unescapeShellPath(inner);
    }
    if (value.contains("'") || value.contains('"')) {
      return null;
    }
    return _unescapeShellPath(value);
  }

  String _normalizeRemoteShellPath(String path) {
    if (path == '~') {
      return path;
    }
    final prefix = path.startsWith('~/')
        ? '~'
        : path.startsWith('/')
        ? '/'
        : '';
    final rest = switch (prefix) {
      '~' => path.substring(2),
      '/' => path.substring(1),
      _ => path,
    };
    final parts = <String>[];
    for (final part in rest.split('/')) {
      if (part.isEmpty || part == '.') {
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
    if (prefix == '~') {
      return parts.isEmpty ? '~' : '~/${parts.join('/')}';
    }
    if (prefix == '/') {
      return '/${parts.join('/')}';
    }
    return parts.join('/');
  }

  String _remotePathBaseName(String path) {
    if (path == '~' || path == '/') {
      return path;
    }
    final trimmed = path.endsWith('/')
        ? path.substring(0, path.length - 1)
        : path;
    final separatorIndex = trimmed.lastIndexOf('/');
    return separatorIndex == -1
        ? trimmed
        : trimmed.substring(separatorIndex + 1);
  }

  String? _promptWorkingDirectoryFromSnapshot(
    TerminalSnapshot snapshot, {
    required bool requireLocalDirectory,
    required bool expandHome,
  }) {
    if (snapshot.rows <= 0 || snapshot.columns <= 0) {
      return null;
    }

    final cursorRow = snapshot.cursor.row.clamp(0, snapshot.rows - 1);
    final firstRow = math.max(0, cursorRow - 2);
    for (var row = cursorRow; row >= firstRow; row--) {
      final promptPath = _extractWorkingDirectoryFromPromptLine(
        _snapshotLineText(snapshot, row),
      );
      if (promptPath == null) {
        continue;
      }
      final expanded = expandHome ? _expandHomePath(promptPath) : promptPath;
      if (expanded.isEmpty) {
        continue;
      }
      if (!requireLocalDirectory) {
        return expanded;
      }
      if (io.Directory(expanded).existsSync()) {
        return _canonicalDirectoryPath(expanded);
      }
    }
    return null;
  }

  String? _extractWorkingDirectoryFromPromptLine(
    String line, {
    bool allowRelative = false,
  }) {
    final trimmed = line.trimRight();
    if (trimmed.isEmpty) {
      return null;
    }

    final marker = RegExp(r'[\$%#❯➜>]\s*$').firstMatch(trimmed);
    if (marker == null) {
      return null;
    }

    final prompt = trimmed.substring(0, marker.start).trimRight();
    if (prompt.isEmpty) {
      return null;
    }

    final parts = prompt.split(RegExp(r'\s+')).reversed;
    for (final rawPart in parts) {
      var part = rawPart.trim();
      while (part.startsWith('[') || part.startsWith('(')) {
        part = part.substring(1);
      }
      while (part.endsWith(']') || part.endsWith(')')) {
        part = part.substring(0, part.length - 1);
      }
      final colonIndex = part.lastIndexOf(':');
      if (colonIndex != -1 && colonIndex < part.length - 1) {
        part = part.substring(colonIndex + 1);
      }
      if (part == '~' ||
          part.startsWith('~${io.Platform.pathSeparator}') ||
          part.startsWith(io.Platform.pathSeparator)) {
        return part;
      }
      if (allowRelative &&
          part.isNotEmpty &&
          !part.contains('/') &&
          !part.contains(r'\')) {
        return part;
      }
    }
    return null;
  }

  String _snapshotLineText(TerminalSnapshot snapshot, int row) {
    final buffer = StringBuffer();
    for (var column = 0; column < snapshot.columns; column++) {
      final cell = snapshot.cellAt(row, column);
      if (cell.wideCharSpacer || cell.leadingWideCharSpacer) {
        continue;
      }
      buffer.write(cell.text);
    }
    return buffer.toString();
  }

  String? _resolveShellPath(
    String path, {
    required String workingDirectory,
    required bool expandHome,
  }) {
    final expanded = expandHome ? _expandHomePath(path) : path;
    if (expanded.isEmpty) {
      return workingDirectory;
    }
    if (expanded == '~' ||
        expanded.startsWith('~/') ||
        expanded.startsWith('/')) {
      return expanded;
    }
    return _joinPath(workingDirectory, expanded, separator: '/');
  }

  String _expandHomePath(String path) {
    if (path == '~') {
      return io.Platform.environment['HOME'] ?? path;
    }
    if (path.startsWith('~${io.Platform.pathSeparator}')) {
      final home = io.Platform.environment['HOME'];
      if (home == null || home.isEmpty) {
        return path;
      }
      return _joinPath(
        home,
        path.substring(2),
        separator: io.Platform.pathSeparator,
      );
    }
    return path;
  }

  String _canonicalDirectoryPath(String path) {
    try {
      return io.Directory(path).resolveSymbolicLinksSync();
    } on io.FileSystemException {
      return io.Directory(path).absolute.path;
    }
  }

  String _joinPath(String parent, String child, {required String separator}) {
    if (child.isEmpty) {
      return parent;
    }
    if (parent.endsWith(separator)) {
      return '$parent$child';
    }
    return '$parent$separator$child';
  }

  String _pathBaseName(String path) {
    final trimmed = path.endsWith(io.Platform.pathSeparator)
        ? path.substring(0, path.length - 1)
        : path;
    final index = trimmed.lastIndexOf(io.Platform.pathSeparator);
    return index == -1 ? trimmed : trimmed.substring(index + 1);
  }

  String _escapeShellPath(String path) {
    return WorkspaceComposerCompletion.escapeShellPath(path);
  }

  String _unescapeShellPath(String path) {
    final buffer = StringBuffer();
    var escaping = false;
    for (final rune in path.runes) {
      final char = String.fromCharCode(rune);
      if (escaping) {
        buffer.write(char);
        escaping = false;
        continue;
      }
      if (char == r'\') {
        escaping = true;
        continue;
      }
      buffer.write(char);
    }
    if (escaping) {
      buffer.write(r'\');
    }
    return buffer.toString();
  }
}
