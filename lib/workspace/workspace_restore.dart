part of 'nauterm_workspace.dart';

const Duration _workspaceStateSaveDebounce = Duration(milliseconds: 400);

extension _NautermWorkspaceRestore on _NautermWorkspaceState {
  void _scheduleWorkspaceStateSave() {
    if (!_workspaceStateReady || _isClosing || _workspaceStateStore == null) {
      return;
    }
    _workspaceStateSaveTimer?.cancel();
    _workspaceStateSaveTimer = Timer(_workspaceStateSaveDebounce, () {
      _workspaceStateSaveTimer = null;
      unawaited(
        _saveWorkspaceState(cleanShutdown: false, restoreOnNextLaunch: false),
      );
    });
  }

  Future<void> _finishWorkspaceRestoreInitialization() {
    return _workspaceRestoreInitializationFuture ??=
        _performWorkspaceRestoreInitialization();
  }

  Future<void> _performWorkspaceRestoreInitialization() async {
    final store = _workspaceStateStore;
    if (store == null) return;

    final previous = await _previousWorkspaceState;
    if (!mounted || _isClosing) return;
    final restore = switch (previous?.launchAction) {
      WorkspaceRestoreLaunchAction.ask => await _askToRestoreUnexpectedExit(),
      WorkspaceRestoreLaunchAction.restore => true,
      WorkspaceRestoreLaunchAction.none || null => false,
    };

    if (!mounted || _isClosing) return;
    if (restore && previous != null) {
      await _restoreWorkspaceSnapshot(previous);
    }
    if (!mounted || _isClosing) return;

    // The launch-time recovery candidate has been consumed, whether the user
    // restored it or started fresh. Do not let a later window close resurrect
    // the stale unexpected-exit snapshot.
    _previousWorkspaceState = Future.value(null);
    _workspaceStateReady = true;
    await _saveWorkspaceState(cleanShutdown: false, restoreOnNextLaunch: false);
  }

  Future<bool> _askToRestoreUnexpectedExit() async {
    final result = await _showWorkspaceDialog<bool>(
      builder: (context) => const _WorkspaceRecoveryDialog(),
    );
    return result ?? false;
  }

  Future<void> _saveRestorationStateForClose() async {
    _workspaceStateReady = false;
    _workspaceStateSaveTimer?.cancel();
    _workspaceStateSaveTimer = null;
    _workspaceStateCheckpointTimer?.cancel();
    _workspaceStateCheckpointTimer = null;
    await _saveWorkspaceState(
      cleanShutdown: true,
      restoreOnNextLaunch: _restoreOnShutdown,
    );
  }

  Future<void> _saveWorkspaceState({
    required bool cleanShutdown,
    required bool restoreOnNextLaunch,
  }) async {
    final store = _workspaceStateStore;
    if (store == null) return;
    _workspaceStateSaveTimer?.cancel();
    _workspaceStateSaveTimer = null;
    try {
      final current = _createWorkspaceSnapshot(
        cleanShutdown: cleanShutdown,
        restoreOnNextLaunch: restoreOnNextLaunch,
      );
      if (cleanShutdown &&
          !_workspaceStateReady &&
          !current.hasRestorableContent) {
        final previous = await _previousWorkspaceState;
        final hasPendingRecovery =
            previous != null &&
            previous.launchAction != WorkspaceRestoreLaunchAction.none;
        if (hasPendingRecovery) {
          await store.save(previous);
          return;
        }
      }
      await store.save(current);
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'workspace-restore',
        'Unable to save workspace recovery state.',
        error: error,
        stackTrace: stackTrace,
      );
    }
  }

  NautermWorkspaceStateSnapshot _createWorkspaceSnapshot({
    required bool cleanShutdown,
    required bool restoreOnNextLaunch,
  }) {
    final workspaces = <WorkspaceSnapshot>[];
    for (final workspace in _workspaces) {
      final tabs = <WorkspaceTabSnapshot>[];
      for (final tab in workspace.terminalTabs) {
        final snapshot = _snapshotTerminalTab(tab);
        if (snapshot != null) tabs.add(snapshot);
      }
      workspaces.add(
        WorkspaceSnapshot(
          id: workspace.id,
          name: workspace.name,
          colorValue: workspace.color.toARGB32(),
          selectedTabId: workspace.selectedTerminalId,
          selectedPaneId: workspace.selectedTerminalViewId,
          tabs: tabs,
        ),
      );
    }

    final activePortForwardUuids = <String>[];
    for (final entry in _portForwardEntries) {
      final id = entry.id;
      final uuid = _emptyToNull(entry.uuid);
      if (id != null && uuid != null && _runningPortForwardIds.contains(id)) {
        activePortForwardUuids.add(uuid);
      }
    }

    final hasRestorableContent =
        activePortForwardUuids.isNotEmpty ||
        workspaces.any((workspace) => workspace.tabs.isNotEmpty);
    return NautermWorkspaceStateSnapshot(
      cleanShutdown: cleanShutdown,
      restoreOnNextLaunch: restoreOnNextLaunch && hasRestorableContent,
      savedAt: DateTime.now().toUtc(),
      selectedWorkspaceId: _selectedWorkspaceId,
      workspaces: workspaces,
      activePortForwardUuids: activePortForwardUuids,
    );
  }

  WorkspaceTabSnapshot? _snapshotTerminalTab(_TerminalTab tab) {
    if (tab.replay) return null;
    final layout = _snapshotTerminalLayout(tab.rootLayout);
    if (layout == null) return null;
    return WorkspaceTabSnapshot(
      id: tab.id,
      title: tab.title,
      layout: layout,
      showSftp: tab.pageMode == _TerminalTabPageMode.sftp,
    );
  }

  WorkspaceLayoutSnapshot? _snapshotTerminalLayout(_TerminalViewLayout layout) {
    return switch (layout) {
      _TerminalViewLeaf(:final view) => _snapshotTerminalPane(view),
      _TerminalSplitLayout(
        :final id,
        :final axis,
        :final children,
        :final fractions,
      ) =>
        _snapshotTerminalSplit(id, axis, children, fractions),
    };
  }

  WorkspaceLayoutSnapshot? _snapshotTerminalPane(_TerminalViewEntry view) {
    final sessions = <WorkspaceTerminalSessionSnapshot>[];
    for (final entry in view.tabs) {
      final target = _snapshotTerminalTarget(entry);
      if (target == null) continue;
      sessions.add(
        WorkspaceTerminalSessionSnapshot(
          id: entry.id,
          title: entry.title,
          target: target,
        ),
      );
    }
    if (sessions.isEmpty) return null;

    return WorkspaceLayoutSnapshot.leaf(
      WorkspacePaneSnapshot(
        id: view.id,
        selectedSessionId:
            sessions.any((session) => session.id == view.selectedTabId)
            ? view.selectedTabId
            : sessions.first.id,
        composerVisible: view.composerVisible,
        workingDirectory: _workingDirectoryForSnapshot(view),
        sessions: sessions,
      ),
    );
  }

  WorkspaceLayoutSnapshot? _snapshotTerminalSplit(
    int id,
    Axis axis,
    List<_TerminalViewLayout> children,
    List<double> fractions,
  ) {
    final retainedChildren = <WorkspaceLayoutSnapshot>[];
    final retainedFractions = <double>[];
    for (var index = 0; index < children.length; index++) {
      final child = _snapshotTerminalLayout(children[index]);
      if (child == null) continue;
      retainedChildren.add(child);
      retainedFractions.add(
        index < fractions.length ? fractions[index] : 1 / children.length,
      );
    }
    if (retainedChildren.isEmpty) return null;
    if (retainedChildren.length == 1) return retainedChildren.single;
    return WorkspaceLayoutSnapshot.split(
      splitId: id,
      axis: axis == Axis.horizontal
          ? WorkspaceSplitAxis.horizontal
          : WorkspaceSplitAxis.vertical,
      fractions: _validSplitFractions(
        retainedFractions,
        retainedChildren.length,
      ),
      children: retainedChildren,
    );
  }

  WorkspaceTerminalTargetSnapshot? _snapshotTerminalTarget(
    _TerminalViewTabEntry entry,
  ) {
    if (entry.pendingConnection != null) return null;
    final controller = entry.controller;
    if (controller.serialProfile != null) return null;

    final ssh = controller.sshProfile;
    if (ssh != null) {
      final hostUuid = _hostUuidForId(ssh.hostId);
      if (hostUuid == null) return null;
      return WorkspaceTerminalTargetSnapshot(
        protocol: controller.isMoshSession
            ? WorkspaceTerminalProtocol.mosh
            : WorkspaceTerminalProtocol.ssh,
        hostUuid: hostUuid,
      );
    }

    final telnet = controller.telnetProfile;
    if (telnet != null) {
      final hostUuid = _hostUuidForId(telnet.hostId);
      if (hostUuid == null) return null;
      return WorkspaceTerminalTargetSnapshot(
        protocol: WorkspaceTerminalProtocol.telnet,
        hostUuid: hostUuid,
      );
    }

    if (!controller.isLocalTerminal) return null;
    return WorkspaceTerminalTargetSnapshot(
      protocol: WorkspaceTerminalProtocol.local,
      hostUuid: _emptyToNull(entry.sourceHostUuid),
      shellPath: _emptyToNull(controller.shellPath),
    );
  }

  String? _hostUuidForId(int? hostId) {
    if (hostId == null) return null;
    return _hostEntries.where((host) => host.id == hostId).firstOrNull?.uuid;
  }

  String? _workingDirectoryForSnapshot(_TerminalViewEntry view) {
    final controller = view.activeTab.controller;
    if (controller.sshProfile != null) {
      return _pendingSshWorkingDirectories[view.id] ??
          _sshWorkingDirectoryForPane(view.id, controller.snapshot);
    }
    if (!controller.isLocalTerminal ||
        view.activeTab.pendingConnection != null) {
      return null;
    }
    return _promptWorkingDirectoryFromSnapshot(
      controller.snapshot,
      requireLocalDirectory: true,
      expandHome: true,
    );
  }

  Future<void> _restoreWorkspaceSnapshot(
    NautermWorkspaceStateSnapshot snapshot,
  ) async {
    final restoredWorkspaces = <_WorkspaceRuntimeState>[];
    for (final workspaceSnapshot in snapshot.workspaces) {
      final tabs = <_TerminalTab>[];
      for (final tabSnapshot in workspaceSnapshot.tabs) {
        final tab = await _restoreTerminalTab(tabSnapshot);
        if (tab != null) tabs.add(tab);
      }
      restoredWorkspaces.add(
        _WorkspaceRuntimeState(
            id: workspaceSnapshot.id,
            name: workspaceSnapshot.name,
            icon: Icons.dashboard_rounded,
            color: Color(workspaceSnapshot.colorValue),
            terminalTabs: tabs,
          )
          ..selectedTerminalId =
              tabs.any((tab) => tab.id == workspaceSnapshot.selectedTabId)
              ? workspaceSnapshot.selectedTabId
              : tabs.firstOrNull?.id
          ..selectedTerminalViewId = _restoredSelectedPaneId(
            tabs,
            workspaceSnapshot.selectedTabId,
            workspaceSnapshot.selectedPaneId,
          ),
      );
    }

    if (!mounted) return;
    final oldWorkspaces = _workspaces.toList(growable: false);
    _setWorkspaceState(() {
      _workspaces
        ..clear()
        ..addAll(restoredWorkspaces);
      if (_workspaces.isEmpty) {
        _workspaces.add(
          _WorkspaceRuntimeState(
            id: 1,
            name: 'Default',
            icon: Icons.dashboard_rounded,
            color: const Color(0xff075e92),
          ),
        );
      }
      _selectedWorkspaceId =
          _workspaces.any(
            (workspace) => workspace.id == snapshot.selectedWorkspaceId,
          )
          ? snapshot.selectedWorkspaceId!
          : _workspaces.first.id;
      _nextWorkspaceId =
          _workspaces.map((workspace) => workspace.id).fold(0, math.max) + 1;
      _nextTerminalId = _maximumRestoredTerminalId(_workspaces) + 1;
      _nextTerminalSplitId = _maximumRestoredSplitId(_workspaces) + 1;
      if (_allTerminalTabs.isNotEmpty) {
        _tab = _WorkspaceTab.sessions;
        _workspaceOverviewActive = false;
      }
      _editorRequest = null;
    });
    for (final workspace in oldWorkspaces) {
      _disposeAiConversation(workspace.aiConversation);
    }
    widget.controller?._notifyTabsChanged();
    await _restorePortForwards(snapshot.activePortForwardUuids);
  }

  int? _restoredSelectedPaneId(
    List<_TerminalTab> tabs,
    int? selectedTabId,
    int? selectedPaneId,
  ) {
    final selectedTab = tabs
        .where((tab) => tab.id == selectedTabId)
        .firstOrNull;
    if (selectedTab == null) return tabs.firstOrNull?.primaryView.id;
    if (selectedPaneId != null &&
        selectedTab.rootLayout.containsView(selectedPaneId)) {
      return selectedPaneId;
    }
    return selectedTab.primaryView.id;
  }

  Future<_TerminalTab?> _restoreTerminalTab(
    WorkspaceTabSnapshot snapshot,
  ) async {
    final layout = await _restoreTerminalLayout(snapshot.id, snapshot.layout);
    if (layout == null) return null;
    final primary = layout.views.first;
    final tab = _TerminalTab(
      id: snapshot.id,
      title: snapshot.title,
      controller: primary.controller,
      theme: primary.theme,
      rootLayout: layout,
    );
    if (snapshot.showSftp) {
      final profile = primary.activeTab.controller.sshProfile;
      if (profile != null) {
        tab
          ..pageMode = _TerminalTabPageMode.sftp
          ..sftpPaneMounted = true
          ..sftpConnectRequest = _sftpConnectRequestForProfile(tab, profile);
      }
    }
    return tab;
  }

  Future<_TerminalViewLayout?> _restoreTerminalLayout(
    int terminalTabId,
    WorkspaceLayoutSnapshot snapshot,
  ) async {
    final pane = snapshot.pane;
    if (pane != null) {
      final sessions = <_TerminalViewTabEntry>[];
      for (final session in pane.sessions) {
        _TerminalViewTabEntry? restored;
        try {
          restored = await _restoreTerminalSession(
            terminalTabId: terminalTabId,
            pane: pane,
            session: session,
          );
        } on Object catch (error, stackTrace) {
          NautermLog.warning(
            'workspace-restore',
            'Unable to restore a terminal session.',
            error: error,
            stackTrace: stackTrace,
          );
        }
        if (restored != null) sessions.add(restored);
      }
      if (sessions.isEmpty) return null;
      final first = sessions.first;
      final view = _TerminalViewEntry(
        id: pane.id,
        title: first.title,
        controller: first.controller,
        theme: first.theme,
        sourceHostUuid: first.sourceHostUuid,
        composerVisible: pane.composerVisible,
      );
      view.tabs
        ..clear()
        ..addAll(sessions);
      view.selectedTabId =
          sessions.any((session) => session.id == pane.selectedSessionId)
          ? pane.selectedSessionId
          : sessions.first.id;
      return _TerminalViewLeaf(view);
    }

    final children = <_TerminalViewLayout>[];
    final fractions = <double>[];
    for (var index = 0; index < snapshot.children.length; index++) {
      final child = await _restoreTerminalLayout(
        terminalTabId,
        snapshot.children[index],
      );
      if (child == null) continue;
      children.add(child);
      fractions.add(
        index < snapshot.fractions.length ? snapshot.fractions[index] : 1,
      );
    }
    if (children.isEmpty) return null;
    if (children.length == 1) return children.single;
    return _TerminalSplitLayout(
      id: snapshot.splitId!,
      axis: snapshot.axis == WorkspaceSplitAxis.horizontal
          ? Axis.horizontal
          : Axis.vertical,
      children: children,
      fractions: fractions,
    );
  }

  Future<_TerminalViewTabEntry?> _restoreTerminalSession({
    required int terminalTabId,
    required WorkspacePaneSnapshot pane,
    required WorkspaceTerminalSessionSnapshot session,
  }) async {
    final target = session.target;
    final host = target.hostUuid == null
        ? null
        : _hostEntries
              .where((host) => host.uuid == target.hostUuid)
              .firstOrNull;
    if (target.hostUuid != null && host == null) return null;

    if (target.protocol == WorkspaceTerminalProtocol.local) {
      if (host != null && host.type != NautermHostType.local) return null;
      final theme = await _themeForId(host?.themeId);
      if (!mounted) return null;
      final shellPath = _resolvedLocalShellPath(
        _emptyToNull(host?.shellPath) ?? target.shellPath,
      );
      final workingDirectory =
          _emptyToNull(pane.workingDirectory) ?? _emptyToNull(host?.workDir);
      late final TerminalController controller;
      controller = TerminalController(
        config: currentTerminalConfig(),
        shellPath: shellPath,
        workingDirectory: workingDirectory,
        environment: _terminalEnvironmentForTheme(theme),
        theme: theme,
        recorder: _createTerminalRecorder(
          title: session.title,
          target: workingDirectory,
          themeId: host?.themeId,
          hostId: host?.id,
          username: _localShellUsername(),
          shellPath: shellPath,
          workDir: workingDirectory,
          cwdResolver: () => workingDirectory,
        ),
        onExit: () => _closeTerminalViewTab(terminalTabId, pane.id, session.id),
      );
      return _TerminalViewTabEntry(
        id: session.id,
        title: session.title,
        controller: controller,
        theme: theme,
        sourceHostUuid: host?.uuid,
      );
    }

    if (host == null || host.type != NautermHostType.remote) return null;
    if (!_hostSupportsRestoreProtocol(host, target.protocol)) return null;
    final themeId = target.protocol == WorkspaceTerminalProtocol.telnet
        ? host.telnetThemeId
        : host.themeId;
    final theme = await _themeForId(themeId);
    if (!mounted) return null;

    late final TerminalController controller;
    switch (target.protocol) {
      case WorkspaceTerminalProtocol.ssh || WorkspaceTerminalProtocol.mosh:
        final auth = _sshAuthForHost(host, feature: 'Workspace restore');
        if (auth == null) return null;
        final startupSnippet = _restoredStartupSnippet(
          _startupSnippetForHost(host),
          pane.workingDirectory,
        );
        final environment = _terminalEnvironmentForTheme(
          theme,
          _hostEnvironment(host),
        );
        final recorder = _createTerminalRecorder(
          title: session.title,
          target: '${auth.username}@${auth.host}:${auth.port}',
          themeId: host.themeId,
          hostId: host.id,
          host: auth.host,
          port: auth.port,
          username: auth.username,
          shellPath: _emptyToNull(host.shellPath),
          workDir: host.workDir,
          cwdResolver: () => _sshWorkingDirectories[pane.id],
          awaitConnection: true,
        );
        void handleExit() {
          if (controller.shouldCloseOnExit) {
            _closeTerminalViewTab(terminalTabId, pane.id, session.id);
          }
        }
        void handleConnected() => _detectHostOsAfterConnect(host, auth);
        if (target.protocol == WorkspaceTerminalProtocol.mosh) {
          controller = TerminalController.mosh(
            host: auth.host,
            port: auth.port,
            username: auth.username,
            knownHostsPath: _workspaceRestoreKnownHostsPath,
            hostId: host.id,
            identityId: host.identityId,
            label: host.name,
            password: auth.password,
            privateKey: auth.privateKey,
            certificate: auth.certificate,
            passphrase: auth.passphrase,
            proxy: auth.proxy,
            shellPath: _emptyToNull(host.shellPath),
            startupSnippet: startupSnippet,
            serverCommand: host.moshServerCommand,
            environment: environment,
            theme: theme,
            recorder: recorder,
            config: currentTerminalConfig(),
            onInputSent: (data) => _trackSshPaneInput(pane.id, data),
            onConnected: handleConnected,
            onExit: handleExit,
          );
        } else {
          controller = TerminalController.ssh(
            host: auth.host,
            port: auth.port,
            username: auth.username,
            knownHostsPath: _workspaceRestoreKnownHostsPath,
            hostId: host.id,
            identityId: host.identityId,
            label: host.name,
            password: auth.password,
            privateKey: auth.privateKey,
            certificate: auth.certificate,
            passphrase: auth.passphrase,
            proxy: auth.proxy,
            shellPath: _emptyToNull(host.shellPath),
            startupSnippet: startupSnippet,
            environment: environment,
            encoding: host.encoding,
            theme: theme,
            recorder: recorder,
            config: currentTerminalConfig(),
            onInputSent: (data) => _trackSshPaneInput(pane.id, data),
            onConnected: handleConnected,
            onExit: handleExit,
          );
        }
        _sshWorkingDirectories[pane.id] =
            _emptyToNull(pane.workingDirectory) ?? '~';
      case WorkspaceTerminalProtocol.telnet:
        final address = _emptyToNull(host.host);
        if (address == null) return null;
        final identity = host.telnetIdentityId == null
            ? null
            : _identityEntries
                  .where((entry) => entry.id == host.telnetIdentityId)
                  .firstOrNull;
        final username = _firstNonEmpty([
          identity?.username,
          host.telnetUsername,
        ]);
        final password = _firstNonEmpty([
          identity?.password,
          host.telnetPassword,
        ]);
        controller = TerminalController.telnet(
          host: address,
          port: host.telnetPort ?? 23,
          hostId: host.id,
          identityId: host.telnetIdentityId,
          label: host.name,
          username: username,
          password: password,
          encoding: host.telnetEncoding,
          startupSnippet: _restoredStartupSnippet(null, pane.workingDirectory),
          environment: _terminalEnvironmentForTheme(theme),
          theme: theme,
          recorder: _createTerminalRecorder(
            title: session.title,
            target: 'telnet $address:${host.telnetPort ?? 23}',
            themeId: host.telnetThemeId,
            hostId: host.id,
            host: address,
            port: host.telnetPort ?? 23,
            username: username,
            awaitConnection: true,
          ),
          config: currentTerminalConfig(),
          onExit: () {
            if (controller.shouldCloseOnExit) {
              _closeTerminalViewTab(terminalTabId, pane.id, session.id);
            }
          },
        );
      case WorkspaceTerminalProtocol.local:
        throw StateError('Local restore was handled before this switch.');
    }

    return _TerminalViewTabEntry(
      id: session.id,
      title: session.title,
      controller: controller,
      theme: theme,
      sourceHostUuid: host.uuid,
    );
  }

  bool _hostSupportsRestoreProtocol(
    HostEntry host,
    WorkspaceTerminalProtocol protocol,
  ) => switch (protocol) {
    WorkspaceTerminalProtocol.ssh => host.sshEnabled,
    WorkspaceTerminalProtocol.mosh => host.moshEnabled,
    WorkspaceTerminalProtocol.telnet => host.telnetEnabled,
    WorkspaceTerminalProtocol.local => host.type == NautermHostType.local,
  };

  String? _restoredStartupSnippet(String? configured, String? directory) {
    final snippet = _emptyToNull(configured);
    final changeDirectory = _restoreChangeDirectoryCommand(directory);
    if (snippet == null) return changeDirectory;
    if (changeDirectory == null) return snippet;
    return '$snippet\n$changeDirectory';
  }

  String? _restoreChangeDirectoryCommand(String? value) {
    final directory = _emptyToNull(value);
    if (directory == null || directory == '~') return null;
    if (directory.startsWith('~/')) {
      return "cd -- ~/${_singleQuoteShell(directory.substring(2))}";
    }
    return 'cd -- ${_singleQuoteShell(directory)}';
  }

  String _singleQuoteShell(String value) =>
      "'${value.replaceAll("'", "'\\''")}'";

  String get _workspaceRestoreKnownHostsPath =>
      (_workspaceStateStore?.paths ?? NautermPaths.resolve())
          .knownHostsFile
          .path;

  Future<void> _restorePortForwards(List<String> uuids) async {
    if (uuids.isEmpty || !mounted) return;
    final requested = uuids.toSet();
    for (final entry in _portForwardEntries) {
      final id = entry.id;
      if (id == null || !requested.contains(entry.uuid)) continue;
      final item = _portForwards.where((item) => item.id == id).firstOrNull;
      if (item == null) continue;
      final status = _startPortForward(item);
      if (status == null || status.isError) continue;
      _runningPortForwardIds.add(id);
      _portForwardStatuses[id] = status;
    }
    _setWorkspaceState(() {
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

  int _maximumRestoredTerminalId(List<_WorkspaceRuntimeState> workspaces) {
    var maximum = 0;
    for (final workspace in workspaces) {
      for (final tab in workspace.terminalTabs) {
        maximum = math.max(maximum, tab.id);
        for (final view in tab.rootLayout.views) {
          maximum = math.max(maximum, view.id);
          for (final session in view.tabs) {
            maximum = math.max(maximum, session.id);
          }
        }
      }
    }
    return maximum;
  }

  int _maximumRestoredSplitId(List<_WorkspaceRuntimeState> workspaces) {
    var maximum = 0;
    void visit(_TerminalViewLayout layout) {
      if (layout case _TerminalSplitLayout(:final id, :final children)) {
        maximum = math.max(maximum, id);
        for (final child in children) {
          visit(child);
        }
      }
    }

    for (final workspace in workspaces) {
      for (final tab in workspace.terminalTabs) {
        visit(tab.rootLayout);
      }
    }
    return maximum;
  }
}
