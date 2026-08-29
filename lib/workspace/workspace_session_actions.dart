part of 'nauterm_workspace.dart';

extension _NautermWorkspaceSessionActions on _NautermWorkspaceState {
  static const int _terminalLogPageSize = 40;

  String? _startupSnippetForHost(HostEntry host) {
    final id = host.startupSnippetId;
    return id == null ? null : _emptyToNull(_dataStore?.getSnippet(id)?.script);
  }

  Future<void> _loadWorkspaceData() async {
    NautermDataStore? pendingDataStore;
    try {
      final paths = NautermPaths.resolve();
      final configStore = NautermConfigStore(paths);
      await configStore.ensureDefaultConfig();
      final settings = await configStore.loadRuntimeSettings();
      applyNautermRuntimeSettings(settings);
      _lastRecordingConfig = terminalRecordingConfig;
      final provider = AiProviderStore(paths).load(settings.aiAssistant);
      setAiAssistantConfig(provider.config);

      final dataStore = NautermDataStore.openPath(paths.databasePath);
      pendingDataStore = dataStore;
      final captureStore = TerminalLogCaptureStore(paths.terminalLogsDirectory);
      await captureStore.prepare();
      for (final log in dataStore.listIncompleteTerminalCaptures()) {
        try {
          if (!await captureStore.captureExists(log.captureFile)) {
            dataStore.clearMissingTerminalCapture(log.id);
            continue;
          }
          final recovered = await captureStore.recover(
            logId: log.id,
            captureFile: log.captureFile,
          );
          dataStore.finalizeRecoveredTerminalCapture(
            logId: log.id,
            captureBytes: recovered.ciphertextBytes,
            captureSha256: recovered.fileSha256,
            endedAt: DateTime.now().toUtc(),
          );
        } on Object catch (error, stackTrace) {
          NautermLog.warning(
            'recording',
            'Unable to recover terminal capture.',
            error: error,
            stackTrace: stackTrace,
          );
        }
      }
      await captureStore.cleanupOrphans(
        dataStore.listTerminalCaptureFiles().toSet(),
        referencedStateFiles: dataStore
            .listIncompleteTerminalCaptures()
            .map((log) => log.captureFile)
            .toSet(),
      );
      await _applyTerminalRetentionPolicy(
        dataStore: dataStore,
        captureStore: captureStore,
      );
      final groups = dataStore.listGroups();
      final hosts = dataStore.listHosts();
      final keys = dataStore.listKeys();
      final identities = dataStore.listIdentities();
      final tags = dataStore.listTags();
      final portForwards = dataStore.listPortForwards();
      final proxies = dataStore.listProxies();
      final snippetPackages = dataStore.listSnippetPackages();
      final snippets = dataStore.listSnippets();
      final knownHostsText = await KnownHostsStore(paths.knownHostsFile)
          .readText();
      final terminalLogs = dataStore.listTerminalLogs(
        limit: _terminalLogPageSize,
      );
      final shellHistory = await ShellHistoryFileStore(paths.shellHistoryFile)
          .read();
      final terminalCaptureDiskUsage = await captureStore.diskUsage();

      if (!mounted) {
        dataStore.dispose();
        pendingDataStore = null;
        return;
      }

      _dataStore?.dispose();
      _setWorkspaceState(() {
        final portForwardIds = {
          for (final portForward in portForwards)
            if (portForward.id != null) portForward.id!,
        };
        _runningPortForwardIds.retainAll(portForwardIds);
        _portForwardStatuses.removeWhere(
          (id, _) => !portForwardIds.contains(id),
        );
        _dataStore = dataStore;
        _groupEntries = groups;
        _hostEntries = hosts;
        _keyEntries = keys;
        _identityEntries = identities;
        _tagEntries = tags;
        _portForwardEntries = portForwards;
        _proxyEntries = proxies;
        _snippetPackageEntries = snippetPackages;
        _groups = _mapGroups(groups, hosts);
        _hosts = _mapHosts(hosts, groups, identities, tags);
        _keys = keys.map(_mapKey).toList(growable: false);
        _identities = identities.map(_mapIdentity).toList(growable: false);
        _portForwards = portForwards
            .map(
              (portForward) => _mapPortForward(
                portForward,
                hosts,
                _runningPortForwardIds,
                _portForwardStatuses,
              ),
            )
            .toList(growable: false);
        _proxies = proxies
            .map((proxy) => _mapProxy(proxy, identities))
            .toList(growable: false);
        _snippets = snippets.map(_mapSnippet).toList(growable: false);
        _knownHostsText = knownHostsText;
        _terminalThemeCatalog = TerminalThemeCatalog(
          paths.themesDirectory,
          additionalDirectories: paths.additionalThemeDirectories,
        );
        _terminalLogCaptureStore = captureStore;
        _terminalLogs = terminalLogs;
        _terminalCaptureDiskUsage = terminalCaptureDiskUsage;
        _terminalLogsHasMore = terminalLogs.length == _terminalLogPageSize;
        _terminalLogsLoading = false;
        _shellHistory = shellHistory;
        _loadingData = false;
      });
      pendingDataStore = null;
    } catch (error, stackTrace) {
      pendingDataStore?.dispose();
      NautermLog.error(
        'workspace',
        'Workspace data load failed.',
        error: error,
        stackTrace: stackTrace,
      );
      if (!mounted) {
        return;
      }
      _setWorkspaceState(() {
        _loadingData = false;
      });
      _showWorkspaceMessage(
        _workspaceDataLoadNotification(error),
        type: _WorkspaceNotificationType.error,
      );
    }
  }

  void _loadMoreTerminalLogs() {
    final store = _dataStore;
    if (store == null || _terminalLogsLoading || !_terminalLogsHasMore) {
      return;
    }
    _setWorkspaceState(() => _terminalLogsLoading = true);
    try {
      final page = store.listTerminalLogs(
        limit: _terminalLogPageSize,
        offset: _terminalLogs.length,
      );
      if (!mounted) {
        return;
      }
      _setWorkspaceState(() {
        final existingIds = _terminalLogs.map((log) => log.id).toSet();
        _terminalLogs = [
          ..._terminalLogs,
          for (final log in page)
            if (existingIds.add(log.id)) log,
        ];
        _terminalLogsHasMore = page.length == _terminalLogPageSize;
        _terminalLogsLoading = false;
      });
    } catch (error) {
      if (mounted) {
        _setWorkspaceState(() => _terminalLogsLoading = false);
        _showWorkspaceMessage('Failed to load more terminal logs: $error');
      }
    }
  }

  void _detectHostOsAfterConnect(HostEntry host, _PortForwardSshAuth auth) {
    final hostId = host.id;
    if (hostId == null || host.type != NautermHostType.remote) {
      return;
    }
    if (!_hostOsDetectionRequests.add(hostId)) {
      return;
    }
    final arguments = <String, Object?>{
      'host': auth.host,
      'port': auth.port,
      'username': auth.username,
      'knownHostsPath': NautermPaths.resolve().knownHostsFile.path,
      'password': auth.password,
      'privateKey': auth.privateKey,
      'certificate': auth.certificate,
      'proxy': auth.proxy?.toJson(),
      'hostKeyTrustMode': SshHostKeyTrustMode.acceptOnce.wireValue,
    };
    unawaited(() async {
      try {
        final result = await _spawnHostOsDetection(arguments);
        final detectedOs = result.os?.trim();
        final detectedDistro = result.distro?.trim();
        if (detectedOs == null || detectedOs.isEmpty || !mounted) {
          return;
        }
        final current =
            _dataStore?.getHost(hostId) ??
            _hostEntries.where((entry) => entry.id == hostId).firstOrNull;
        if (current == null ||
            (current.os == detectedOs && current.distro == detectedDistro)) {
          return;
        }
        final updated = current.withPlatform(
          os: detectedOs,
          distro: detectedDistro,
        );
        _dataStore?.saveHost(updated);
        if (mounted) {
          _upsertHostSummary(updated);
        }
      } catch (_) {
        // Host OS detection is opportunistic and should never disrupt a session.
      } finally {
        _hostOsDetectionRequests.remove(hostId);
      }
    }());
  }

  void _handleSftpRemoteConnected(_HostItem item, _SftpRemoteAuth auth) {
    final hostEntry = _hostEntries
        .where((entry) => entry.id != null && entry.id == item.id)
        .firstOrNull;
    if (hostEntry == null) {
      return;
    }
    _detectHostOsAfterConnect(
      hostEntry,
      _PortForwardSshAuth(
        host: auth.host,
        port: auth.port,
        username: auth.username,
        password: auth.password,
        privateKey: auth.privateKey,
        certificate: auth.certificate,
        proxy: auth.proxy,
      ),
    );
  }

  void _selectWorkspaceTab(_WorkspaceTab tab) {
    if (tab == _WorkspaceTab.sftp && !sftpTabEnabled) {
      return;
    }
    _setWorkspaceState(() {
      _tab = tab;
      if (tab == _WorkspaceTab.sftp) {
        _sftpPaneMounted = true;
      }
      _workspaceOverviewActive = tab == _WorkspaceTab.sessions;
      _editorRequest = null;
    });
    // Force terminal resize when switching to sessions tab
    if (tab == _WorkspaceTab.sessions) {
      terminalConfigNotifier.value++;
    }
  }

  void _selectRuntimeWorkspace(int workspaceId) {
    _setWorkspaceState(() {
      _selectedWorkspaceId = workspaceId;
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = true;
      final workspace = _selectedWorkspace;
      if (workspace.selectedTerminalId == null &&
          workspace.terminalTabs.isNotEmpty) {
        final firstTab = workspace.terminalTabs.first;
        workspace.selectedTerminalId = firstTab.id;
        workspace.selectedTerminalViewId =
            firstTab.rootLayout.views.firstOrNull?.id;
      }
      _editorRequest = null;
    });
  }

  Future<void> _renameRuntimeWorkspace(int workspaceId) async {
    final workspace = _workspaces
        .where((workspace) => workspace.id == workspaceId)
        .firstOrNull;
    if (workspace == null) {
      return;
    }

    final name = await _showWorkspaceDialog<String>(
      builder: (context) {
        return _WorkspaceRenameDialog(initialName: workspace.name);
      },
    );
    final nextName = _emptyToNull(name);
    if (nextName == null || !mounted) {
      return;
    }

    _setWorkspaceState(() {
      workspace.name = nextName;
      _editorRequest = null;
    });
    widget.controller?._notifyTabsChanged();
  }

  Future<void> _confirmAndCloseRuntimeWorkspace(int workspaceId) async {
    final workspace = _workspaces
        .where((workspace) => workspace.id == workspaceId)
        .firstOrNull;
    if (workspace == null) {
      return;
    }

    final sessionCount = workspace.sessionCount;
    final confirmed = await _confirmDelete(
      title: 'Close Workspace?',
      message: sessionCount == 0
          ? 'Close "${workspace.name}"?'
          : 'Close "${workspace.name}" and its $sessionCount ${sessionCount == 1 ? 'session' : 'sessions'}?',
      confirmLabel: 'Close',
    );
    if (!confirmed || !mounted) {
      return;
    }

    _closeRuntimeWorkspace(workspaceId);
  }

  void _closeRuntimeWorkspace(int workspaceId) {
    final index = _workspaces.indexWhere(
      (workspace) => workspace.id == workspaceId,
    );
    if (index == -1) {
      return;
    }

    final closedWorkspace = _workspaces[index];
    final controllers = [
      for (final tab in closedWorkspace.terminalTabs) ...tab.controllers,
    ];
    final viewIds = [
      for (final tab in closedWorkspace.terminalTabs)
        for (final view in tab.rootLayout.views) view.id,
    ];
    _setWorkspaceState(() {
      for (final viewId in viewIds) {
        _clearSshPaneCompletionState(viewId);
      }
      _workspaces.removeAt(index);
      if (_workspaces.isEmpty) {
        _workspaces.add(
          _WorkspaceRuntimeState(
            id: _nextWorkspaceId++,
            name: 'Default',
            icon: Icons.dashboard_rounded,
            color: const Color(0xff075e92),
          ),
        );
      }
      if (_selectedWorkspaceId == workspaceId ||
          !_workspaces.any(
            (workspace) => workspace.id == _selectedWorkspaceId,
          )) {
        final nextIndex = math.min(index, _workspaces.length - 1);
        final nextWorkspace = _workspaces[nextIndex];
        _selectedWorkspaceId = nextWorkspace.id;
        final firstTab = nextWorkspace.terminalTabs.firstOrNull;
        nextWorkspace.selectedTerminalId ??= firstTab?.id;
        nextWorkspace.selectedTerminalViewId ??=
            firstTab?.rootLayout.views.firstOrNull?.id;
        _tab = _WorkspaceTab.sessions;
        _workspaceOverviewActive = true;
      }
      _editorRequest = null;
    });
    widget.controller?._notifyTabsChanged();
    _disposeAiConversation(closedWorkspace.aiConversation);
    for (final tab in closedWorkspace.terminalTabs) {
      _disposeAiConversation(tab.aiConversation);
    }
    for (final controller in controllers) {
      _disposeTerminalController(controller);
    }
  }

  void _showSelectedWorkspaceSessions() {
    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = true;
      _editorRequest = null;
    });
  }

  void _createRuntimeWorkspace() {
    unawaited(_createRuntimeWorkspaceFromQuickConnect());
  }

  Future<void> _createRuntimeWorkspaceFromQuickConnect() async {
    final result = await _showQuickConnectDialog();
    if (result == null || !mounted) {
      return;
    }

    final workspace = _addRuntimeWorkspace(
      _workspaceNameForQuickConnect(result),
    );
    _setWorkspaceState(() {
      _selectedWorkspaceId = workspace.id;
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = true;
      _selectedTerminalId = null;
      _selectedTerminalViewId = null;
      _editorRequest = null;
    });
    await _handleQuickConnectResult(result, keepWorkspaceOverview: true);
  }

  _WorkspaceRuntimeState _addRuntimeWorkspace(String? name) {
    final id = _nextWorkspaceId++;
    final workspace = _WorkspaceRuntimeState(
      id: id,
      name: _emptyToNull(name) ?? 'Workspace $id',
      icon: Icons.dashboard_rounded,
      color: const Color(0xff075e92),
    );
    _workspaces.add(workspace);
    return workspace;
  }

  void _selectTerminalTab(int terminalId) {
    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      _selectedTerminalId = terminalId;
      for (final tab in _terminalTabs) {
        if (tab.id == terminalId) tab.bellIndicator = false;
      }
      _selectedTerminalViewId = _terminalTabs
          .where((tab) => tab.id == terminalId)
          .firstOrNull
          ?.rootLayout
          .views
          .firstOrNull
          ?.id;
      _editorRequest = null;
    });
    // Force terminal resize when switching tabs
    terminalConfigNotifier.value++;
  }

  void _focusWorkspaceSession(int terminalId) {
    _setWorkspaceState(() {
      final keepWorkspaceOverview =
          _tab == _WorkspaceTab.sessions && _workspaceOverviewActive;
      final tab = _terminalTabs
          .where((tab) => tab.id == terminalId)
          .firstOrNull;
      if (tab == null) {
        return;
      }
      tab.bellIndicator = false;
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = keepWorkspaceOverview;
      _selectedTerminalId = terminalId;
      _selectedTerminalViewId = tab.rootLayout.views.firstOrNull?.id;
      _editorRequest = null;
    });
    // Force terminal resize when focusing a session.
    terminalConfigNotifier.value++;
  }

  void _selectRelativeTopBarTab(int delta) {
    final fixedTabCount =
        1 + (sftpTabEnabled ? 1 : 0) + (workspacePageEnabled ? 1 : 0);
    final tabCount = _terminalTabs.length + fixedTabCount;
    if (tabCount == 0) {
      return;
    }
    final current = _selectedTopBarTabIndex();
    final next = (current + delta) % tabCount;
    _selectTopBarTabIndex(next < 0 ? next + tabCount : next);
  }

  void _selectTopBarTabIndex(int index) {
    var cursor = 0;
    if (index == cursor) {
      _selectWorkspaceTab(_WorkspaceTab.vaults);
      return;
    }
    cursor++;
    if (sftpTabEnabled) {
      if (index == cursor) {
        _selectWorkspaceTab(_WorkspaceTab.sftp);
        return;
      }
      cursor++;
    }
    if (workspacePageEnabled) {
      if (index == cursor) {
        _showSelectedWorkspaceSessions();
        return;
      }
      cursor++;
    }

    final terminalIndex = index - cursor;
    if (terminalIndex < 0 || terminalIndex >= _terminalTabs.length) {
      return;
    }
    _selectTerminalTab(_terminalTabs[terminalIndex].id);
  }

  int _selectedTopBarTabIndex() {
    final selectedTerminalId = _selectedTerminalId;
    if (_tab == _WorkspaceTab.sessions &&
        !_workspaceOverviewActive &&
        selectedTerminalId != null) {
      final terminalIndex = _terminalTabs.indexWhere(
        (tab) => tab.id == selectedTerminalId,
      );
      if (terminalIndex != -1) {
        return terminalIndex +
            1 +
            (sftpTabEnabled ? 1 : 0) +
            (workspacePageEnabled ? 1 : 0);
      }
    }
    return switch (_tab) {
      _WorkspaceTab.vaults => 0,
      _WorkspaceTab.sftp => 1,
      _WorkspaceTab.sessions when workspacePageEnabled =>
        1 + (sftpTabEnabled ? 1 : 0),
      _WorkspaceTab.sessions => 0,
    };
  }

  void _openLocalTerminalTab({
    String? title,
    String? shellPath,
    String? workingDirectory,
    String? startupSnippet,
    Map<String, String> environment = const {},
    TerminalTheme? theme,
    String? themeId,
  }) {
    final resolvedShellPath = _resolvedLocalShellPath(shellPath);
    _setWorkspaceState(() {
      final tabTheme = theme ?? _defaultThemeForSettings();
      final terminalEnvironment = _terminalEnvironmentForTheme(
        tabTheme,
        environment,
      );
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      final id = _nextTerminalId++;
      final tabTitle =
          title ??
          (_terminalTabs.isEmpty
              ? 'Local Terminal'
              : 'Local Terminal ${_terminalTabs.length + 1}');
      final controller = TerminalController(
        config: currentTerminalConfig(),
        shellPath: resolvedShellPath,
        workingDirectory: workingDirectory,
        startupSnippet: startupSnippet,
        environment: terminalEnvironment,
        theme: tabTheme,
        recorder: _createTerminalRecorder(
          title: tabTitle,
          target: workingDirectory,
          themeId: themeId,
          username: _localShellUsername(),
          shellPath: resolvedShellPath,
          workDir: workingDirectory,
          cwdResolver: () => workingDirectory,
        ),
        onExit: () => _closeTerminalViewTab(id, id, id),
      );
      _terminalTabs.add(
        _TerminalTab(
          id: id,
          title: tabTitle,
          theme: tabTheme,
          controller: controller,
        ),
      );
      _selectedTerminalId = id;
      _selectedTerminalViewId = id;
      _editorRequest = null;
    });
    widget.controller?._notifyTabsChanged();
  }

  Future<void> _openSerialTerminalDialog() async {
    final draft = await _showWorkspaceDialog<_SerialConnectionDraft>(
      builder: (context) => const _SerialConnectionDialog(),
    );
    if (draft == null || !mounted) {
      return;
    }

    _openSerialTerminalTab(draft);
  }

  void _openSerialTerminalTab(_SerialConnectionDraft draft) {
    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      final id = _nextTerminalId++;
      final tabTitle = draft.serialPort;
      final serialConfig = draft.config;
      late final TerminalController controller;
      controller = TerminalController.serial(
        serialConfig: serialConfig,
        label: tabTitle,
        config: currentTerminalConfig(),
        theme: defaultTerminalTheme,
        recorder: _createTerminalRecorder(
          title: tabTitle,
          target: '${serialConfig.serialPort} @ ${serialConfig.summary}',
          awaitConnection: true,
        ),
        onExit: () {
          if (controller.shouldCloseOnExit) {
            _closeTerminalViewTab(id, id, id);
          }
        },
      );
      _terminalTabs.add(
        _TerminalTab(
          id: id,
          title: tabTitle,
          theme: defaultTerminalTheme,
          controller: controller,
        ),
      );
      _selectedTerminalId = id;
      _selectedTerminalViewId = id;
      _editorRequest = null;
    });
    widget.controller?._notifyTabsChanged();
  }

  Future<void> _openQuickConnectDialog() async {
    final result = await _showQuickConnectDialog();
    if (result == null || !mounted) {
      return;
    }

    await _handleQuickConnectResult(result);
  }

  Future<void> _openWorkspaceQuickConnectDialog() async {
    final result = await _showQuickConnectDialog();
    if (result == null || !mounted) {
      return;
    }

    await _handleQuickConnectResult(result, keepWorkspaceOverview: true);
  }

  Future<_QuickConnectResult?> _showQuickConnectDialog() {
    if (_quickConnectDialogOpen) {
      return Future<_QuickConnectResult?>.value();
    }
    _quickConnectDialogOpen = true;
    return _showWorkspaceDialog<_QuickConnectResult>(
      builder: (context) => _QuickConnectDialog(
        hosts: _hosts,
        groups: _groups,
        tags: _tagEntries,
        loading: _loadingData,
      ),
    ).whenComplete(() {
      _quickConnectDialogOpen = false;
    });
  }

  Future<void> _handleQuickConnectResult(
    _QuickConnectResult result, {
    bool keepWorkspaceOverview = false,
  }) async {
    switch (result) {
      case _QuickConnectLocalShell(:final shell):
        _openLocalTerminalTab(title: shell.label, shellPath: shell.path);
      case _QuickConnectHostOpen(:final host):
        await _connectHost(host);
      case _QuickConnectHostSsh(:final host):
        await _connectHost(host, protocol: _HostConnectProtocol.ssh);
      case _QuickConnectHostMosh(:final host):
        await _connectHost(host, protocol: _HostConnectProtocol.mosh);
      case _QuickConnectHostTelnet(:final host):
        await _connectHost(host, protocol: _HostConnectProtocol.telnet);
      case _QuickConnectHostSftp(:final host):
        _connectSftpHost(host);
      case _QuickConnectDirectSsh(:final query):
        _connectHostQuery(query.label);
    }
    if (keepWorkspaceOverview &&
        mounted &&
        _quickConnectCreatesTerminal(result)) {
      _setWorkspaceState(() {
        _tab = _WorkspaceTab.sessions;
        _workspaceOverviewActive = true;
        _editorRequest = null;
      });
    }
  }

  bool _quickConnectCreatesTerminal(_QuickConnectResult result) {
    return switch (result) {
      _QuickConnectLocalShell() ||
      _QuickConnectHostOpen() ||
      _QuickConnectHostSsh() ||
      _QuickConnectHostMosh() ||
      _QuickConnectHostTelnet() ||
      _QuickConnectDirectSsh() => true,
      _QuickConnectHostSftp() => false,
    };
  }

  String _workspaceNameForQuickConnect(_QuickConnectResult result) {
    return switch (result) {
      _QuickConnectLocalShell(:final shell) => shell.label,
      _QuickConnectHostOpen(:final host) => host.name,
      _QuickConnectHostSsh(:final host) => host.name,
      _QuickConnectHostMosh(:final host) => host.name,
      _QuickConnectHostTelnet(:final host) => host.name,
      _QuickConnectHostSftp(:final host) => host.name,
      _QuickConnectDirectSsh(:final query) => query.host,
    };
  }

  Future<void> _connectHost(
    _HostItem item, {
    _HostConnectProtocol? protocol,
  }) async {
    final rawHost = _dataStore?.getHost(item.id);
    if (rawHost == null) {
      _showWorkspaceMessage('Host is not available.');
      return;
    }
    final host = rawHost;
    if (host.type == NautermHostType.local) {
      final theme = await _themeForId(host.themeId);
      if (!mounted) {
        return;
      }
      _openLocalTerminalTab(
        title: host.name,
        shellPath: _emptyToNull(host.shellPath),
        workingDirectory: _emptyToNull(host.workDir),
        theme: theme,
        themeId: host.themeId,
      );
      return;
    }

    if (protocol == null &&
        [
              host.sshEnabled,
              host.moshEnabled,
              host.telnetEnabled,
            ].where((enabled) => enabled).length >
            1) {
      await _openHostProtocolConnection(host, item);
      return;
    }
    final selectedProtocol =
        protocol ??
        (host.sshEnabled
            ? _HostConnectProtocol.ssh
            : host.moshEnabled
            ? _HostConnectProtocol.mosh
            : host.telnetEnabled
            ? _HostConnectProtocol.telnet
            : null);
    if (selectedProtocol == null) {
      _showWorkspaceMessage('Add SSH or Telnet before connecting.');
      return;
    }
    final selection = _HostProtocolSelection(
      protocol: selectedProtocol,
      port: selectedProtocol == _HostConnectProtocol.telnet
          ? host.telnetPort ?? 23
          : host.port ?? 22,
      moshServerCommand: host.moshServerCommand,
    );
    switch (selection.protocol) {
      case _HostConnectProtocol.ssh:
        await _connectSshHost(host, port: selection.port);
      case _HostConnectProtocol.mosh:
        await _connectMoshHost(
          host,
          port: selection.port,
          command: selection.moshServerCommand,
        );
      case _HostConnectProtocol.telnet:
        await _connectTelnetHost(host, port: selection.port);
    }
  }

  Future<void> _openHostProtocolConnection(
    HostEntry host,
    _HostItem item,
  ) async {
    final theme = await _themeForId(host.themeId);
    if (!mounted) {
      return;
    }
    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      final id = _nextTerminalId++;
      final controller = TerminalController(
        driver: MemoryTerminalDriver(
          columns: 80,
          rows: 24,
          config: currentTerminalConfig(),
        ),
        theme: theme,
        config: currentTerminalConfig(),
      );
      _terminalTabs.add(
        _TerminalTab(
          id: id,
          title: host.name,
          theme: theme,
          controller: controller,
          pendingConnection: _PendingHostConnection(host: host, item: item),
        ),
      );
      _selectedTerminalId = id;
      _selectedTerminalViewId = id;
      _editorRequest = null;
    });
    widget.controller?._notifyTabsChanged();
  }

  Future<void> _startPendingHostConnection(
    int pendingTabId,
    _HostProtocolSelection selection,
  ) async {
    final pendingTab = _terminalTabs
        .where((tab) => tab.id == pendingTabId)
        .firstOrNull;
    final pending = pendingTab?.primaryView.activeTab.pendingConnection;
    if (pending == null) {
      return;
    }
    switch (selection.protocol) {
      case _HostConnectProtocol.ssh:
        await _connectSshHost(pending.host, port: selection.port);
      case _HostConnectProtocol.mosh:
        await _connectMoshHost(
          pending.host,
          port: selection.port,
          command: selection.moshServerCommand,
        );
      case _HostConnectProtocol.telnet:
        await _connectTelnetHost(pending.host, port: selection.port);
    }
    if (mounted) {
      _closeTerminalTab(pendingTabId);
    }
  }

  Future<void> _connectSshHost(
    HostEntry host, {
    int? port,
    bool useMosh = false,
    String? moshServerCommand,
  }) async {
    final auth = _sshAuthForHost(
      host,
      feature: useMosh ? 'Mosh' : 'SSH',
      portOverride: port,
    );
    if (auth == null) {
      return;
    }

    final theme = await _themeForId(host.themeId);
    if (!mounted) {
      return;
    }
    final environment = _terminalEnvironmentForTheme(
      theme,
      _hostEnvironment(host),
    );

    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      final id = _nextTerminalId++;
      late final TerminalController controller;
      final recorder = _createTerminalRecorder(
        title: host.name,
        target: '${auth.username}@${auth.host}:${auth.port}',
        themeId: host.themeId,
        hostId: host.id,
        host: auth.host,
        port: auth.port,
        username: auth.username,
        shellPath: _emptyToNull(host.shellPath),
        workDir: host.workDir,
        cwdResolver: () => _sshWorkingDirectories[id],
        awaitConnection: true,
      );
      void handleExit() {
        if (controller.shouldCloseOnExit) {
          _closeTerminalViewTab(id, id, id);
        }
      }

      if (useMosh) {
        controller = TerminalController.mosh(
          host: auth.host,
          port: auth.port,
          username: auth.username,
          knownHostsPath: NautermPaths.resolve().knownHostsFile.path,
          hostId: host.id,
          identityId: host.identityId,
          label: host.name,
          password: auth.password,
          privateKey: auth.privateKey,
          certificate: auth.certificate,
          proxy: auth.proxy,
          shellPath: _emptyToNull(host.shellPath),
          startupSnippet: _startupSnippetForHost(host),
          serverCommand:
              _emptyToNull(moshServerCommand) ?? host.moshServerCommand,
          environment: environment,
          theme: theme,
          recorder: recorder,
          config: currentTerminalConfig(),
          onInputSent: (data) => _trackSshPaneInput(id, data),
          onConnected: () => _detectHostOsAfterConnect(host, auth),
          onExit: handleExit,
        );
      } else {
        controller = TerminalController.ssh(
          host: auth.host,
          port: auth.port,
          username: auth.username,
          knownHostsPath: NautermPaths.resolve().knownHostsFile.path,
          hostId: host.id,
          identityId: host.identityId,
          label: host.name,
          password: auth.password,
          privateKey: auth.privateKey,
          certificate: auth.certificate,
          proxy: auth.proxy,
          shellPath: _emptyToNull(host.shellPath),
          startupSnippet: _startupSnippetForHost(host),
          environment: environment,
          encoding: host.encoding,
          theme: theme,
          recorder: recorder,
          config: currentTerminalConfig(),
          onInputSent: (data) => _trackSshPaneInput(id, data),
          onConnected: () => _detectHostOsAfterConnect(host, auth),
          onExit: handleExit,
        );
      }
      _terminalTabs.add(
        _TerminalTab(
          id: id,
          title: host.name,
          theme: theme,
          controller: controller,
        ),
      );
      _sshWorkingDirectories[id] = '~';
      _selectedTerminalId = id;
      _selectedTerminalViewId = id;
      _editorRequest = null;
    });
    widget.controller?._notifyTabsChanged();
  }

  Future<void> _connectMoshHost(
    HostEntry host, {
    int? port,
    String? command,
  }) async {
    if (!host.moshEnabled) {
      _showWorkspaceMessage('Mosh is not enabled for this host.');
      return;
    }
    await _connectSshHost(
      host,
      port: port,
      useMosh: true,
      moshServerCommand: command,
    );
  }

  Future<void> _connectTelnetHost(HostEntry host, {int? port}) async {
    if (!host.telnetEnabled) {
      _showWorkspaceMessage('Telnet is not enabled for this host.');
      return;
    }
    final address = _emptyToNull(host.host);
    if (address == null) {
      _showWorkspaceMessage('Host address is required.');
      return;
    }
    final resolvedPort = port ?? host.telnetPort ?? 23;
    if (resolvedPort < 1 || resolvedPort > 65535) {
      _showWorkspaceMessage('Telnet port must be between 1 and 65535.');
      return;
    }
    final identity = host.telnetIdentityId == null
        ? null
        : _identityEntries
              .where((identity) => identity.id == host.telnetIdentityId)
              .firstOrNull;
    final username = _firstNonEmpty([identity?.username, host.telnetUsername]);
    final password = _firstNonEmpty([identity?.password, host.telnetPassword]);
    final theme = await _themeForId(host.telnetThemeId);
    if (!mounted) {
      return;
    }
    final environment = _terminalEnvironmentForTheme(theme);

    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      final id = _nextTerminalId++;
      late final TerminalController controller;
      controller = TerminalController.telnet(
        host: address,
        port: resolvedPort,
        hostId: host.id,
        identityId: host.telnetIdentityId,
        label: host.name,
        username: username,
        password: password,
        encoding: host.telnetEncoding,
        environment: environment,
        theme: theme,
        recorder: _createTerminalRecorder(
          title: host.name,
          target: 'telnet $address:$resolvedPort',
          themeId: host.telnetThemeId,
          hostId: host.id,
          host: address,
          port: resolvedPort,
          username: username,
          awaitConnection: true,
        ),
        config: currentTerminalConfig(),
        onExit: () {
          if (controller.shouldCloseOnExit) {
            _closeTerminalViewTab(id, id, id);
          }
        },
      );
      _terminalTabs.add(
        _TerminalTab(
          id: id,
          title: host.name,
          theme: theme,
          controller: controller,
        ),
      );
      _selectedTerminalId = id;
      _selectedTerminalViewId = id;
      _editorRequest = null;
    });
    widget.controller?._notifyTabsChanged();
  }

  void _connectSftpHost(_HostItem item) {
    final host = _hostEntries
        .where((host) => host.id != null && host.id == item.id)
        .firstOrNull;
    final auth = _sshAuthForHost(host, feature: 'SFTP');
    if (auth == null) {
      return;
    }

    unawaited(_openSftpTerminalTab(item, host!, auth));
  }

  void _connectSftpHostInSftpPage(_HostItem item) {
    final host = _hostEntries
        .where((host) => host.id != null && host.id == item.id)
        .firstOrNull;
    final auth = _sshAuthForHost(host, feature: 'SFTP');
    if (auth == null) {
      return;
    }

    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sftp;
      _sftpPaneMounted = true;
      _sftpConnectRequest = _SftpConnectRequest(
        id: ++_sftpConnectRequestId,
        host: item,
        auth: _SftpRemoteAuth(
          host: auth.host,
          port: auth.port,
          username: auth.username,
          password: auth.password,
          privateKey: auth.privateKey,
          certificate: auth.certificate,
          proxy: auth.proxy,
          knownHostsPath: NautermPaths.resolve().knownHostsFile.path,
        ),
      );
      _editorRequest = null;
    });
  }

  Future<void> _openSftpTerminalTab(
    _HostItem item,
    HostEntry host,
    _PortForwardSshAuth auth,
  ) async {
    final theme = await _themeForId(host.themeId);
    if (!mounted) {
      return;
    }
    final environment = _terminalEnvironmentForTheme(
      theme,
      _hostEnvironment(host),
    );

    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      final id = _nextTerminalId++;
      late final TerminalController controller;
      final connectRequest = _SftpConnectRequest(
        id: 1,
        host: item,
        auth: _SftpRemoteAuth(
          host: auth.host,
          port: auth.port,
          username: auth.username,
          password: auth.password,
          privateKey: auth.privateKey,
          certificate: auth.certificate,
          proxy: auth.proxy,
          knownHostsPath: NautermPaths.resolve().knownHostsFile.path,
        ),
      );
      controller = TerminalController.ssh(
        host: auth.host,
        port: auth.port,
        username: auth.username,
        knownHostsPath: NautermPaths.resolve().knownHostsFile.path,
        hostId: host.id,
        identityId: host.identityId,
        label: host.name,
        password: auth.password,
        privateKey: auth.privateKey,
        certificate: auth.certificate,
        proxy: auth.proxy,
        shellPath: _emptyToNull(host.shellPath),
        startupSnippet: _startupSnippetForHost(host),
        environment: environment,
        encoding: host.encoding,
        theme: theme,
        recorder: _createTerminalRecorder(
          title: host.name,
          target: '${auth.username}@${auth.host}:${auth.port}',
          themeId: host.themeId,
          hostId: host.id,
          host: auth.host,
          port: auth.port,
          username: auth.username,
          shellPath: _emptyToNull(host.shellPath),
          workDir: host.workDir,
          cwdResolver: () => _sshWorkingDirectories[id],
          awaitConnection: true,
        ),
        config: currentTerminalConfig(),
        onInputSent: (data) => _trackSshPaneInput(id, data),
        onConnected: () => _detectHostOsAfterConnect(host, auth),
        onExit: () {
          if (controller.shouldCloseOnExit) {
            _closeTerminalViewTab(id, id, id);
          }
        },
      );
      _terminalTabs.add(
        _TerminalTab(
          id: id,
          title: host.name,
          theme: theme,
          controller: controller,
          pageMode: _TerminalTabPageMode.sftp,
          sftpPaneMounted: true,
          sftpConnectRequestId: 1,
          sftpConnectRequest: connectRequest,
        ),
      );
      _sshWorkingDirectories[id] = '~';
      _selectedTerminalId = id;
      _selectedTerminalViewId = id;
      _editorRequest = null;
    });
    widget.controller?._notifyTabsChanged();
  }

  void _connectHostQuery(String input) {
    final query = _DirectHostQuery.parse(input);
    if (query == null) {
      _showWorkspaceMessage('Use username@host or username@host:port.');
      return;
    }

    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      final id = _nextTerminalId++;
      late final TerminalController controller;
      controller = TerminalController.ssh(
        host: query.host,
        port: query.port,
        username: query.username,
        knownHostsPath: NautermPaths.resolve().knownHostsFile.path,
        label: query.label,
        environment: _terminalEnvironmentForTheme(defaultTerminalTheme),
        theme: defaultTerminalTheme,
        recorder: _createTerminalRecorder(
          title: query.label,
          target: '${query.username}@${query.host}:${query.port}',
          host: query.host,
          port: query.port,
          username: query.username,
          cwdResolver: () => _sshWorkingDirectories[id],
          awaitConnection: true,
        ),
        config: currentTerminalConfig(),
        onInputSent: (data) => _trackSshPaneInput(id, data),
        onExit: () {
          if (controller.shouldCloseOnExit) {
            _closeTerminalViewTab(id, id, id);
          }
        },
      );
      _terminalTabs.add(
        _TerminalTab(
          id: id,
          title: query.label,
          theme: defaultTerminalTheme,
          controller: controller,
        ),
      );
      _sshWorkingDirectories[id] = '~';
      _selectedTerminalId = id;
      _selectedTerminalViewId = id;
      _editorRequest = null;
    });
    widget.controller?._notifyTabsChanged();
  }

  void _showWorkspaceMessage(
    String message, {
    _WorkspaceNotificationType type = _WorkspaceNotificationType.info,
  }) {
    if (!mounted) {
      return;
    }
    _notificationTimer?.cancel();
    _setWorkspaceState(() {
      _notification = _WorkspaceNotification(message: message, type: type);
    });
    _notificationTimer = Timer(const Duration(seconds: 4), () {
      if (!mounted) {
        return;
      }
      _setWorkspaceState(() {
        _notification = null;
      });
    });
  }

  void _dismissWorkspaceNotification() {
    _notificationTimer?.cancel();
    _setWorkspaceState(() {
      _notification = null;
    });
  }

  void _showUpdateNotice(StartupUpdateNotice? notice) {
    _setWorkspaceState(() {
      _updateNotice = notice;
    });
  }

  Future<TerminalTheme> _themeForId(String? themeId) async {
    final theme = await _terminalThemeCatalog?.loadTheme(themeId);
    return theme ?? defaultTerminalTheme;
  }

  TerminalTheme _defaultThemeForSettings() {
    return terminalThemeId == null ? defaultTerminalTheme : terminalCustomTheme;
  }

  TerminalSessionRecorder? _createTerminalRecorder({
    required String title,
    String? target,
    String? themeId,
    int? hostId,
    String? host,
    int? port,
    String? username,
    String? shellPath,
    String? workDir,
    String? Function()? cwdResolver,
    bool awaitConnection = false,
  }) {
    if (_isClosing) {
      throw StateError('Nauterm is shutting down.');
    }
    final recordingConfig = terminalRecordingConfig;
    if (!recordingConfig.enabled) {
      return null;
    }
    late final TerminalSessionRecorder recorder;
    recorder = TerminalSessionRecorder(
      title: title,
      target: target,
      themeId: themeId,
      onChanged: _handleRecordingChanged,
      onConnectionEstablished: awaitConnection
          ? () => _confirmTerminalRecording(recorder.id)
          : null,
      captureEnabled: recordingConfig.captureEnabled,
      onCaptureBytes: recordingConfig.captureEnabled
          ? (bytes) => _writeTerminalCapture(recorder.id, bytes)
          : null,
      onShellHistoryEntry: _appendShellHistoryEntry,
    );
    _recordingService.register(
      recorder,
      pending: awaitConnection,
      context: TerminalLogContext(
        hostId: hostId,
        host: _emptyToNull(host),
        port: port,
        username: _emptyToNull(username),
        shellPath: _emptyToNull(shellPath),
        workDir: _emptyToNull(workDir),
        cwdResolver: cwdResolver,
      ),
    );
    if (!awaitConnection) {
      _selectedLogId ??= recorder.id;
    }
    return recorder;
  }

  void _confirmTerminalRecording(String logId) {
    if (!_recordingService.confirm(logId)) return;
    if (_recordingService.isIgnored(logId) ||
        !terminalRecordingConfig.enabled) {
      return;
    }
    _selectedLogId ??= logId;
    final captureStore = _terminalLogCaptureStore;
    if (captureStore != null && terminalRecordingConfig.captureEnabled) {
      _ensureTerminalCaptureReference(logId, captureStore);
    }
    _handleRecordingChanged();
  }

  void _appendShellHistoryEntry(TerminalShellHistoryEntry entry) {
    if (!terminalRecordingConfig.enabled ||
        _recordingService.isPending(entry.sessionId) ||
        _recordingService.isIgnored(entry.sessionId)) {
      return;
    }
    final historyEntry = ShellHistoryEntry(
      sourceId: entry.id,
      command: entry.command,
      sessionId: entry.sessionId,
      title: entry.title,
      createdAt: entry.timestamp,
    );
    _shellHistoryPersistence = _shellHistoryPersistence.then((_) async {
      try {
        final updated = await ShellHistoryFileStore(
          NautermPaths.resolve().shellHistoryFile,
        ).append(historyEntry);
        if (mounted) {
          _setWorkspaceState(() => _shellHistory = updated);
        }
      } catch (_) {
        // A command that is already in the active controller remains usable
        // for autocomplete even when the optional aggregate file is unavailable.
      }
    });
  }

  void _writeTerminalCapture(String logId, Uint8List bytes) {
    final captureStore = _terminalLogCaptureStore;
    if (captureStore == null ||
        !terminalRecordingConfig.enabled ||
        !terminalRecordingConfig.captureEnabled ||
        _recordingService.isIgnored(logId) ||
        bytes.isEmpty) {
      return;
    }
    if (!_recordingService.isPending(logId) &&
        !_ensureTerminalCaptureReference(logId, captureStore)) {
      return;
    }
    _recordingService.writeCapture(
      recordingId: logId,
      bytes: bytes,
      captureStore: captureStore,
      onError: (error) {
        if (mounted) {
          _showWorkspaceMessage(
            'Terminal capture stopped: unable to write to disk ($error)',
          );
        }
      },
    );
  }

  bool _ensureTerminalCaptureReference(
    String logId,
    TerminalLogCaptureStore captureStore,
  ) {
    if (_recordingService.hasCaptureReference(logId)) return true;
    final dataStore = _dataStore;
    final recorder = _terminalSessionRecorders
        .where((entry) => entry.id == logId)
        .firstOrNull;
    if (dataStore == null || recorder == null) return false;
    final recording = recorder.snapshot();
    final context = _recordingService.contextFor(logId);
    try {
      final log = _terminalLogEntryFromRecording(
        recording,
        context,
        TerminalLogCaptureInfo(fileName: captureStore.captureFileName(logId)),
        preserveEmptyCaptureFile: true,
      );
      dataStore.saveTerminalLog(
        log,
        events: [
          for (final event in recording.events)
            TerminalLogEvent(
              timestamp: event.timestamp,
              type: event.type.name,
              message: event.message,
              connectionKind: event.connectionKind,
              data: event.data,
            ),
        ],
      );
      _recordingService.markCaptureReferenced(logId);
      return true;
    } on Object catch (error) {
      if (mounted) {
        _showWorkspaceMessage(
          'Terminal capture stopped: unable to save its recovery metadata '
          '($error)',
        );
      }
      return false;
    }
  }

  String? _resolvedLocalShellPath(String? shellPath) {
    return _emptyToNull(shellPath) ??
        _emptyToNull(terminalShellPath) ??
        (io.Platform.isWindows ? null : systemDefaultShellPath());
  }

  String? _localShellUsername() {
    return _emptyToNull(io.Platform.environment['USER']) ??
        _emptyToNull(io.Platform.environment['USERNAME']);
  }

  void _handleRecordingChanged() {
    if (mounted && _section == _SidebarSection.logs) {
      scheduleMicrotask(() {
        if (mounted) {
          _setWorkspaceState(() {});
        }
      });
    }
    _scheduleRecordingSave();
  }

  void _scheduleRecordingSave() {
    _recordingService.scheduleSave(_saveTerminalLogs);
  }

  Future<void> _saveTerminalLogs({bool updateState = true}) {
    return _recordingService.enqueueSave(
      () => _performTerminalLogsSave(updateState: updateState),
    );
  }

  Future<void> _performTerminalLogsSave({required bool updateState}) async {
    _recordingService.cancelScheduledSave();
    final dataStore = _dataStore;
    final captureStore = _terminalLogCaptureStore;
    if (dataStore == null || captureStore == null) {
      return;
    }
    final recorders = _terminalSessionRecorders.toList(growable: false);
    final recordingIds = {for (final recorder in recorders) recorder.id};
    try {
      if (!terminalRecordingConfig.enabled) {
        await _recordingService.closeAllCaptures();
        _recordingService.ignoreAllRecorders();
        return;
      }
      if (!terminalRecordingConfig.captureEnabled) {
        await _recordingService.closeAllCaptures();
      }
      final rejectedRecordingIds = {
        for (final recorder in recorders)
          if (_recordingService.isPending(recorder.id) && !recorder.isActive)
            recorder.id,
      };
      for (final logId in rejectedRecordingIds) {
        await _recordingService.closeCapture(logId);
        await captureStore.deleteCapture(captureStore.captureFileName(logId));
        dataStore.deleteTerminalLog(logId);
        _recordingService.removePending(logId);
        _recordingService.removeCaptureReference(logId);
        _recordingService.ignore(logId);
        if (_selectedLogId == logId) _selectedLogId = null;
      }
      final activeRecordingIds = {
        for (final recorder in recorders)
          if (recorder.isActive) recorder.id,
      };
      await _recordingService.checkpointCaptures(
        recordingIds: recordingIds,
        activeRecordingIds: activeRecordingIds,
      );

      final savedLogs = <TerminalLogEntry>[];
      for (final recorder in recorders) {
        if (_recordingService.isIgnored(recorder.id) ||
            _recordingService.isPending(recorder.id)) {
          continue;
        }
        final recording = recorder.snapshot();
        final context = _recordingService.contextFor(recording.id);
        final info = await captureStore.captureInfo(
          recording.id,
          includeHash: !recording.isActive,
          useFinalizedHash: !recording.isActive,
        );
        final savedLog = _terminalLogEntryFromRecording(
          recording,
          context,
          info,
        );
        dataStore.saveTerminalLog(
          savedLog,
          events: [
            for (final event in recording.events)
              TerminalLogEvent(
                timestamp: event.timestamp,
                type: event.type.name,
                message: event.message,
                connectionKind: event.connectionKind,
                data: event.data,
              ),
          ],
        );
        savedLogs.add(savedLog);
      }
      final retention = await _applyTerminalRetentionPolicy(
        dataStore: dataStore,
        captureStore: captureStore,
        activeLogIds: activeRecordingIds,
      );
      if (updateState) {
        if (mounted) {
          _setWorkspaceState(() {
            final byId = {for (final log in _terminalLogs) log.id: log};
            for (final log in savedLogs) {
              byId[log.id] = log;
            }
            for (final log in retention.updatedLogs) {
              byId[log.id] = log;
            }
            for (final id in retention.deletedLogIds) {
              byId.remove(id);
              if (_selectedLogId == id) _selectedLogId = null;
            }
            _terminalLogs = byId.values.toList(growable: false)
              ..sort((a, b) => b.startedAt.compareTo(a.startedAt));
          });
        }
      }
      final diskUsage = await captureStore.diskUsage();
      if (updateState && mounted) {
        _setWorkspaceState(() => _terminalCaptureDiskUsage = diskUsage);
      }
    } catch (error) {
      if (updateState && mounted) {
        _showWorkspaceMessage('Failed to save terminal logs: $error');
      }
    }
  }

  Future<_TerminalRetentionApplication> _applyTerminalRetentionPolicy({
    required NautermDataStore dataStore,
    required TerminalLogCaptureStore captureStore,
    Set<String> activeLogIds = const {},
  }) async {
    final logs = <TerminalLogEntry>[];
    var offset = 0;
    while (true) {
      final page = dataStore.listTerminalLogs(limit: 500, offset: offset);
      logs.addAll(page);
      if (page.length < 500) break;
      offset += page.length;
    }
    final config = terminalRecordingConfig;
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

    for (final log in logs) {
      if (deletedIds.contains(log.id)) continue;
      if (!plan.oversizedLogIds.contains(log.id) || log.captureFile.isEmpty) {
        continue;
      }
      final capture = await captureStore.retainTail(
        logId: log.id,
        captureFile: log.captureFile,
        maxBytes: config.maxSessionBytes,
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
    }

    for (final id in deletedIds) {
      final captureFile = dataStore.deleteTerminalLog(id);
      if (captureFile != null && captureFile.isNotEmpty) {
        await captureStore.deleteCapture(captureFile);
      }
      if (activeLogIds.contains(id)) continue;
      _recordingService.ignore(id);
      _recordingService.removeRecording(id);
    }
    return _TerminalRetentionApplication(
      updatedLogs: updatedLogs,
      deletedLogIds: deletedIds,
    );
  }

  List<TerminalLogEntry> _visibleTerminalLogs() {
    final logsById = {for (final log in _terminalLogs) log.id: log};
    final captureStore = _terminalLogCaptureStore;
    for (final recorder in _terminalSessionRecorders) {
      if (_recordingService.isIgnored(recorder.id) ||
          _recordingService.isPending(recorder.id)) {
        continue;
      }
      final recording = recorder.snapshot();
      final context = _recordingService.contextFor(recording.id);
      final persisted = logsById[recording.id];
      final persistedCaptureFile = persisted?.captureFile.trim() ?? '';
      logsById[recording.id] = _terminalLogEntryFromRecording(
        recording,
        context,
        TerminalLogCaptureInfo(
          fileName: persistedCaptureFile.isNotEmpty
              ? persistedCaptureFile
              : captureStore?.captureFileName(recording.id) ??
                    '${recording.id}.ntrcap',
          bytes: persisted?.captureBytes ?? recording.captureByteCount,
          sha256: persisted?.captureSha256,
        ),
      );
    }
    final logs = logsById.values.toList(growable: false)
      ..sort((a, b) => b.startedAt.compareTo(a.startedAt));
    return logs;
  }

  TerminalLogEntry _terminalLogEntryFromRecording(
    TerminalSessionRecording recording,
    TerminalLogContext context,
    TerminalLogCaptureInfo capture, {
    bool preserveEmptyCaptureFile = false,
  }) {
    final snapshot = recording.frames.lastOrNull?.snapshot;
    return TerminalLogEntry(
      id: recording.id,
      title: recording.title,
      themeId: recording.themeId,
      hostId: context.hostId,
      host: _emptyToNull(context.host),
      port: context.port,
      username: _emptyToNull(context.username),
      shellPath: _emptyToNull(context.shellPath),
      workDir: _emptyToNull(context.workDir),
      cwd: _emptyToNull(context.cwdResolver?.call()),
      captureFile: capture.bytes > 0 || preserveEmptyCaptureFile
          ? capture.fileName
          : '',
      captureBytes: capture.bytes,
      captureSha256: capture.sha256,
      columns: snapshot?.columns,
      rows: snapshot?.rows,
      startedAt: recording.startedAt,
      endedAt: recording.endedAt,
    );
  }
}

class _TerminalRetentionApplication {
  const _TerminalRetentionApplication({
    required this.updatedLogs,
    required this.deletedLogIds,
  });

  final List<TerminalLogEntry> updatedLogs;
  final Set<String> deletedLogIds;
}

enum _HostConnectProtocol { ssh, mosh, telnet }

class _HostProtocolSelection {
  const _HostProtocolSelection({
    required this.protocol,
    required this.port,
    required this.moshServerCommand,
  });

  final _HostConnectProtocol protocol;
  final int port;
  final String moshServerCommand;
}

Map<String, String> _hostEnvironment(HostEntry host) {
  final environment = <String, String>{};
  for (final entry in host.environmentVariables) {
    final variable = _emptyToNull(entry.variable);
    if (variable != null) {
      environment[variable] = entry.value;
    }
  }
  return environment;
}

Map<String, String> _terminalEnvironmentForTheme(
  TerminalTheme theme, [
  Map<String, String> base = const {},
]) {
  final environment = Map<String, String>.of(base);
  environment.putIfAbsent(
    'COLORFGBG',
    () => theme.type == TerminalThemeType.dark ? '15;0' : '0;15',
  );
  return environment;
}

class _HostProtocolCard extends StatelessWidget {
  const _HostProtocolCard({
    required this.title,
    required this.command,
    required this.portController,
    required this.color,
    required this.selected,
    required this.enabled,
    required this.onTap,
    this.commandController,
  });

  final String title;
  final String command;
  final TextEditingController portController;
  final Color color;
  final bool selected;
  final bool enabled;
  final VoidCallback onTap;
  final TextEditingController? commandController;

  @override
  Widget build(BuildContext context) {
    final portTextStyle = TextStyle(
      fontSize: NautermFontSizes.labelLarge,
      fontWeight: NautermFontWeights.regular,
      letterSpacing: 0,
    );
    final commandPreview = Text(
      command,
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: TextStyle(
        color: _mutedText,
        fontSize: NautermFontSizes.labelMedium,
        fontStyle: FontStyle.italic,
        fontWeight: NautermFontWeights.regular,
        letterSpacing: 0,
      ),
    );
    return Material(
      color: selected
          ? _card
          : _card.withValues(alpha: _workspaceDark ? 0.72 : 0.86),
      borderRadius: BorderRadius.circular(9),
      child: InkWell(
        borderRadius: BorderRadius.circular(9),
        hoverColor: _workspaceDark ? _sidebarHover : null,
        splashColor: _workspaceDark ? _workspaceMenuPressed : null,
        highlightColor: _workspaceDark ? _workspaceMenuPressed : null,
        onTap: enabled ? onTap : null,
        child: Container(
          constraints: const BoxConstraints(minHeight: 62),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(9),
            border: Border.all(
              color: selected ? color : Colors.transparent,
              width: selected ? 2 : 1,
            ),
          ),
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
          child: Row(
            children: [
              Container(
                width: 4,
                height: 40,
                decoration: BoxDecoration(
                  color: color,
                  borderRadius: BorderRadius.circular(999),
                ),
              ),
              SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.baseline,
                      textBaseline: TextBaseline.alphabetic,
                      children: [
                        Expanded(
                          child: Text(
                            title,
                            style: TextStyle(
                              color: _text,
                              fontSize: NautermFontSizes.labelLarge,
                              fontWeight: NautermFontWeights.semibold,
                              letterSpacing: 0,
                            ),
                          ),
                        ),
                        const SizedBox(width: 12),
                        Text(
                          'port:',
                          style: portTextStyle.copyWith(color: _mutedText),
                        ),
                        const SizedBox(width: 5),
                        SizedBox(
                          width: 52,
                          height: 24,
                          child: TextField(
                            key: ValueKey('host-protocol-port:$title'),
                            controller: portController,
                            enabled: enabled,
                            onTap: onTap,
                            onChanged: (_) => onTap(),
                            maxLines: 1,
                            keyboardType: TextInputType.number,
                            inputFormatters: [
                              FilteringTextInputFormatter.digitsOnly,
                            ],
                            textAlign: TextAlign.left,
                            textDirection: TextDirection.ltr,
                            style: portTextStyle.copyWith(color: _text),
                            decoration: InputDecoration(
                              isDense: true,
                              contentPadding: const EdgeInsets.only(bottom: 4),
                              border: UnderlineInputBorder(
                                borderSide: BorderSide(color: _sidebarDivider),
                              ),
                              enabledBorder: UnderlineInputBorder(
                                borderSide: BorderSide(color: _sidebarDivider),
                              ),
                              focusedBorder: UnderlineInputBorder(
                                borderSide: BorderSide(color: color),
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 3),
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.baseline,
                      textBaseline: TextBaseline.alphabetic,
                      children: [
                        if (commandController == null)
                          Flexible(child: commandPreview)
                        else
                          ConstrainedBox(
                            constraints: const BoxConstraints(maxWidth: 150),
                            child: commandPreview,
                          ),
                        if (commandController != null) ...[
                          const SizedBox(width: 10),
                          Expanded(
                            child: SizedBox(
                              height: 24,
                              child: TextField(
                                key: const ValueKey(
                                  'host-protocol-mosh-command',
                                ),
                                controller: commandController,
                                enabled: enabled,
                                onTap: onTap,
                                maxLines: 1,
                                style: TextStyle(
                                  color: _text,
                                  fontSize: NautermFontSizes.labelLarge,
                                  fontWeight: NautermFontWeights.regular,
                                  letterSpacing: 0,
                                ),
                                decoration: InputDecoration(
                                  isDense: true,
                                  hintText: defaultMoshServerCommand,
                                  hintStyle: TextStyle(
                                    color: _mutedText.withValues(alpha: 0.72),
                                    fontSize: NautermFontSizes.labelLarge,
                                  ),
                                  contentPadding: const EdgeInsets.only(
                                    bottom: 4,
                                  ),
                                  border: UnderlineInputBorder(
                                    borderSide: BorderSide(
                                      color: _sidebarDivider,
                                    ),
                                  ),
                                  enabledBorder: UnderlineInputBorder(
                                    borderSide: BorderSide(
                                      color: _sidebarDivider,
                                    ),
                                  ),
                                  focusedBorder: UnderlineInputBorder(
                                    borderSide: BorderSide(color: color),
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ],
                      ],
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
