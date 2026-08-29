part of 'nauterm_workspace.dart';

extension _NautermWorkspaceTerminalActions on _NautermWorkspaceState {
  void _closeSelectedTerminalTab() {
    final selectedId = _selectedTerminalId;
    if (selectedId == null) {
      return;
    }

    final selectedViewId = _selectedTerminalViewId;
    final selectedTab = _terminalTabs
        .where((tab) => tab.id == selectedId)
        .firstOrNull;
    final selectedView = selectedViewId == null
        ? selectedTab?.primaryView
        : selectedTab?.rootLayout.viewFor(selectedViewId);
    if (selectedView != null) {
      _confirmAndClose(() {
        _closeTerminalViewTab(
          selectedId,
          selectedView.id,
          selectedView.activeTab.id,
        );
      }, controller: selectedView.activeTab.controller);
      return;
    }

    _confirmAndClose(
      () => _closeTerminalTab(selectedId),
      controller: selectedTab?.controller,
    );
  }

  void _confirmAndClose(VoidCallback close, {TerminalController? controller}) {
    final isEstablished =
        controller?.connectionStatus.phase == TerminalConnectionPhase.connected;
    if (!terminalConfirmOnClose || !isEstablished) {
      close();
      return;
    }
    _showWorkspaceDialog<bool>(
      builder: (context) {
        return _WorkspaceConfirmDialog(
          title: Text(
            tr('workspace.label.closeTerminal', fallback: 'Close Terminal'),
          ),
          message: 'Are you sure you want to close this terminal?',
          confirmLabel: 'Close',
        );
      },
    ).then((confirmed) {
      if (confirmed == true) {
        close();
      }
    });
  }

  void _closeTerminalTab(int terminalId) {
    if (!mounted) {
      return;
    }

    final workspace = _workspaces.where((workspace) {
      return workspace.terminalTabs.any((tab) => tab.id == terminalId);
    }).firstOrNull;
    final tabs = workspace?.terminalTabs ?? _terminalTabs;
    final index = tabs.indexWhere((tab) => tab.id == terminalId);
    if (index == -1) {
      if (_selectedTerminalId == terminalId) {
        _setWorkspaceState(() {
          _selectedTerminalId = null;
          _selectedTerminalViewId = null;
        });
      }
      return;
    }

    final closedTab = tabs[index];
    final controllers = closedTab.controllers.toList(growable: false);
    final closedViewIds = [
      for (final view in closedTab.rootLayout.views) view.id,
    ];
    _setWorkspaceState(() {
      for (final viewId in closedViewIds) {
        _clearSshPaneCompletionState(viewId);
      }
      tabs.removeAt(index);
      if (workspace?.selectedTerminalId != terminalId) {
        return;
      }
      if (tabs.isEmpty) {
        workspace?.selectedTerminalId = null;
        workspace?.selectedTerminalViewId = null;
        if (workspace?.id == _selectedWorkspaceId) {
          _tab = _WorkspaceTab.vaults;
          _workspaceOverviewActive = false;
        }
        return;
      }

      final nextIndex = math.min(index, tabs.length - 1);
      final nextTab = tabs[nextIndex];
      workspace?.selectedTerminalId = nextTab.id;
      workspace?.selectedTerminalViewId =
          nextTab.rootLayout.views.firstOrNull?.id;
    });
    widget.controller?._notifyTabsChanged();
    _disposeAiConversation(closedTab.aiConversation);
    for (final controller in controllers) {
      _disposeTerminalController(controller);
    }
  }

  _ReloadedTerminalConnection? _reloadTerminalConnection(
    TerminalController controller,
    TerminalTheme theme,
  ) {
    final current = controller.sshProfile;
    if (current == null) {
      return null;
    }
    final hostId = current.hostId;
    if (hostId == null) {
      return _ReloadedTerminalConnection(profile: current);
    }
    final host = _dataStore?.getHost(hostId);
    final auth = _sshAuthForHost(host, feature: 'SSH reconnect');
    if (host == null || auth == null) {
      return null;
    }
    return _ReloadedTerminalConnection(
      profile: refreshSavedHostSshProfile(
        current: current,
        host: host,
        address: auth.host,
        port: auth.port,
        username: auth.username,
        password: auth.password,
        privateKey: auth.privateKey,
        certificate: auth.certificate,
        proxy: auth.proxy,
        environment: _terminalEnvironmentForTheme(
          theme,
          _hostEnvironment(host),
        ),
      ),
      moshServerCommand: host.moshServerCommand,
    );
  }

  void _openLocalTerminalViewTab(int terminalSessionId, int terminalViewId) {
    _setWorkspaceState(() {
      final tab = _terminalTabs
          .where((tab) => tab.id == terminalSessionId)
          .firstOrNull;
      final view = tab?.rootLayout.viewFor(terminalViewId);
      if (tab == null || view == null) {
        return;
      }

      // Check if the current view has an SSH connection to reuse.
      final sshProfile = view.activeTab.controller.sshProfile;

      final id = _nextTerminalId++;
      late final TerminalController controller;
      late final String title;
      if (sshProfile != null) {
        title = sshProfile.label ?? '${sshProfile.username}@${sshProfile.host}';
        controller = TerminalController.ssh(
          host: sshProfile.host,
          port: sshProfile.port,
          username: sshProfile.username,
          knownHostsPath: sshProfile.knownHostsPath,
          hostId: sshProfile.hostId,
          identityId: sshProfile.identityId,
          label: sshProfile.label,
          password: sshProfile.password,
          privateKey: sshProfile.privateKey,
          certificate: sshProfile.certificate,
          passphrase: sshProfile.passphrase,
          proxy: sshProfile.proxy,
          shellPath: sshProfile.shellPath,
          environment: sshProfile.environment,
          encoding: sshProfile.encoding,
          config: currentTerminalConfig(),
          theme: view.theme,
          recorder: _createTerminalRecorder(
            title: title,
            username: sshProfile.username,
            target: '${sshProfile.host}:${sshProfile.port}',
            awaitConnection: true,
          ),
          onExit: () {
            if (controller.shouldCloseOnExit) {
              _closeTerminalViewTab(terminalSessionId, terminalViewId, id);
            }
          },
        );
      } else {
        title = view.tabs.isEmpty
            ? 'Local Terminal'
            : 'Local Terminal ${view.tabs.length + 1}';
        controller = TerminalController(
          config: currentTerminalConfig(),
          shellPath: _resolvedLocalShellPath(null),
          environment: _terminalEnvironmentForTheme(view.theme),
          theme: view.theme,
          recorder: _createTerminalRecorder(
            title: title,
            username: _localShellUsername(),
            shellPath: _resolvedLocalShellPath(null),
          ),
          onExit: () =>
              _closeTerminalViewTab(terminalSessionId, terminalViewId, id),
        );
      }
      view.tabs.add(
        _TerminalViewTabEntry(
          id: id,
          title: title,
          controller: controller,
          theme: view.theme,
        ),
      );
      view.selectedTabId = id;
      _tab = _WorkspaceTab.sessions;
      _selectedTerminalId = terminalSessionId;
      _selectedTerminalViewId = terminalViewId;
      _editorRequest = null;
    });
  }

  void _closeTerminalViewTab(
    int terminalSessionId,
    int terminalViewId,
    int terminalViewTabId,
  ) {
    final currentWorkspace = _workspaces.where((workspace) {
      return workspace.terminalTabs.any((tab) => tab.id == terminalSessionId);
    }).firstOrNull;
    final currentTabs = currentWorkspace?.terminalTabs ?? _terminalTabs;
    final currentTab = currentTabs
        .where((tab) => tab.id == terminalSessionId)
        .firstOrNull;
    final currentView = currentTab?.rootLayout.viewFor(terminalViewId);
    if (currentView?.tabs.length == 1) {
      _closeTerminalView(terminalSessionId, terminalViewId);
      return;
    }

    var controllersToDispose = <TerminalController>[];
    _setWorkspaceState(() {
      final workspace = _workspaces.where((workspace) {
        return workspace.terminalTabs.any((tab) => tab.id == terminalSessionId);
      }).firstOrNull;
      final tabs = workspace?.terminalTabs ?? _terminalTabs;
      final tabIndex = tabs.indexWhere((tab) => tab.id == terminalSessionId);
      if (tabIndex == -1) {
        return;
      }

      final tab = tabs[tabIndex];
      final view = tab.rootLayout.viewFor(terminalViewId);
      if (view == null) {
        return;
      }
      final tabToCloseIndex = view.tabs.indexWhere(
        (tab) => tab.id == terminalViewTabId,
      );
      if (tabToCloseIndex == -1) {
        return;
      }

      final removed = view.tabs.removeAt(tabToCloseIndex);
      controllersToDispose = [removed.controller];
      if (view.selectedTabId == terminalViewTabId) {
        final nextIndex = math.min(tabToCloseIndex, view.tabs.length - 1);
        view.selectedTabId = view.tabs[nextIndex].id;
      }
      _selectedTerminalId = terminalSessionId;
      _selectedTerminalViewId = terminalViewId;
      _editorRequest = null;
    });
    for (final controller in controllersToDispose) {
      _disposeTerminalController(controller);
    }
  }

  void _closeTerminalView(int terminalSessionId, int terminalViewId) {
    var removedSession = false;
    var controllersToDispose = <TerminalController>[];
    _setWorkspaceState(() {
      final workspace = _workspaces.where((workspace) {
        return workspace.terminalTabs.any((tab) => tab.id == terminalSessionId);
      }).firstOrNull;
      final tabs = workspace?.terminalTabs ?? _terminalTabs;
      final tabIndex = tabs.indexWhere((tab) => tab.id == terminalSessionId);
      if (tabIndex == -1) {
        return;
      }

      final tab = tabs[tabIndex];
      final view = tab.rootLayout.viewFor(terminalViewId);
      if (view == null) {
        return;
      }

      final nextLayout = tab.rootLayout.removeView(terminalViewId);
      controllersToDispose = view.controllers.toList(growable: false);
      _clearSshPaneCompletionState(view.id);

      if (nextLayout == null) {
        removedSession = true;
        tabs.removeAt(tabIndex);
        if (workspace?.selectedTerminalId == terminalSessionId) {
          if (tabs.isEmpty) {
            workspace?.selectedTerminalId = null;
            workspace?.selectedTerminalViewId = null;
            if (workspace?.id == _selectedWorkspaceId) {
              _tab = _WorkspaceTab.vaults;
              _workspaceOverviewActive = false;
            }
          } else {
            final nextIndex = math.min(tabIndex, tabs.length - 1);
            final nextTab = tabs[nextIndex];
            workspace?.selectedTerminalId = nextTab.id;
            workspace?.selectedTerminalViewId =
                nextTab.rootLayout.views.firstOrNull?.id;
          }
        }
        return;
      }

      tabs[tabIndex] = tab.copyWith(rootLayout: nextLayout);
      final wasSelectedSession =
          workspace?.selectedTerminalId == terminalSessionId;
      final previousSelectedViewId = workspace?.selectedTerminalViewId;
      final selectedViewStillExists =
          wasSelectedSession &&
          previousSelectedViewId != null &&
          nextLayout.containsView(previousSelectedViewId);
      if (wasSelectedSession && !selectedViewStillExists) {
        workspace?.selectedTerminalId = terminalSessionId;
        workspace?.selectedTerminalViewId = nextLayout.views.firstOrNull?.id;
      }
      _editorRequest = null;
    });
    if (removedSession) {
      widget.controller?._notifyTabsChanged();
    }
    for (final controller in controllersToDispose) {
      _disposeTerminalController(controller);
    }
  }

  void _clearSshPaneCompletionState(int paneId) {
    _sshDirectoryCompletionDebounceTimers.remove(paneId)?.cancel();
    _sshPathCompletionDebounceTimers.remove(paneId)?.cancel();
    _activeSshDirectoryCompletionKeys.remove(paneId);
    _activeSshPathCompletionKeys.remove(paneId);
    _sshWorkingDirectories.remove(paneId);
    _pendingSshWorkingDirectories.remove(paneId);
    _sshInputBuffers.remove(paneId);
  }

  void _splitSelectedTerminalTab(TerminalSplitDirection direction) {
    final selectedId = _selectedTerminalId;
    if (selectedId == null) {
      return;
    }

    final index = _terminalTabs.indexWhere((tab) => tab.id == selectedId);
    if (index == -1) {
      return;
    }

    final tab = _terminalTabs[index];
    final currentLayout = tab.rootLayout;
    final selectedViewId = _selectedTerminalViewId;
    final targetViewId =
        selectedViewId != null && currentLayout.containsView(selectedViewId)
        ? selectedViewId
        : tab.id;
    final targetView = currentLayout.viewFor(targetViewId) ?? tab.primaryView;
    final viewId = _nextTerminalId++;

    // Check if the target view has an SSH connection to reuse.
    final sshProfile = targetView.activeTab.controller.sshProfile;

    late final TerminalController controller;
    late final String title;
    if (sshProfile != null) {
      title = sshProfile.label ?? '${sshProfile.username}@${sshProfile.host}';
      controller = TerminalController.ssh(
        host: sshProfile.host,
        port: sshProfile.port,
        username: sshProfile.username,
        knownHostsPath: sshProfile.knownHostsPath,
        hostId: sshProfile.hostId,
        identityId: sshProfile.identityId,
        label: sshProfile.label,
        password: sshProfile.password,
        privateKey: sshProfile.privateKey,
        certificate: sshProfile.certificate,
        passphrase: sshProfile.passphrase,
        proxy: sshProfile.proxy,
        shellPath: sshProfile.shellPath,
        environment: sshProfile.environment,
        encoding: sshProfile.encoding,
        config: currentTerminalConfig(),
        theme: targetView.theme,
        recorder: _createTerminalRecorder(
          title: title,
          username: sshProfile.username,
          target: '${sshProfile.host}:${sshProfile.port}',
          awaitConnection: true,
        ),
        onExit: () {
          if (controller.shouldCloseOnExit) {
            _closeTerminalViewTab(tab.id, viewId, viewId);
          }
        },
      );
    } else {
      title = 'Local Terminal';
      controller = TerminalController(
        config: currentTerminalConfig(),
        shellPath: _resolvedLocalShellPath(null),
        environment: _terminalEnvironmentForTheme(targetView.theme),
        theme: targetView.theme,
        recorder: _createTerminalRecorder(
          title: title,
          username: _localShellUsername(),
          shellPath: _resolvedLocalShellPath(null),
        ),
        onExit: () => _closeTerminalViewTab(tab.id, viewId, viewId),
      );
    }
    final newView = _TerminalViewEntry(
      id: viewId,
      title: title,
      controller: controller,
      theme: targetView.theme,
      composerVisible: targetView.composerVisible,
    );
    final nextLayout = currentLayout.splitView(
      targetViewId: targetViewId,
      newView: newView,
      axis: direction == TerminalSplitDirection.down
          ? Axis.vertical
          : Axis.horizontal,
      splitId: _nextTerminalSplitId++,
    );

    _setWorkspaceState(() {
      _terminalTabs[index] = tab.copyWith(rootLayout: nextLayout);
      _selectedTerminalViewId = newView.id;
    });
  }

  void _showSelectedTerminalSsh() {
    final tab = _selectedTerminalTab;
    if (tab == null) {
      return;
    }
    _showTerminalTabSsh(tab.id);
  }

  void _showSelectedTerminalSftp() {
    final tab = _selectedTerminalTab;
    if (tab == null) {
      return;
    }
    _showTerminalTabSftp(tab.id);
  }

  void _showTerminalTabSsh(int terminalId) {
    _setWorkspaceState(() {
      final tab = _terminalTabs
          .where((tab) => tab.id == terminalId)
          .firstOrNull;
      if (tab == null) {
        return;
      }
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      _selectedTerminalId = terminalId;
      final selectedViewId = _selectedTerminalViewId;
      _selectedTerminalViewId =
          selectedViewId != null && tab.rootLayout.containsView(selectedViewId)
          ? selectedViewId
          : tab.rootLayout.views.firstOrNull?.id;
      tab.pageMode = _TerminalTabPageMode.ssh;
      _editorRequest = null;
    });
  }

  void _showTerminalTabSftp(int terminalId, {int? terminalViewId}) {
    final tab = _terminalTabs.where((tab) => tab.id == terminalId).firstOrNull;
    if (tab == null) {
      return;
    }

    final view = _terminalViewForSftp(tab, terminalViewId);
    final profile = view?.controller.sshProfile;
    if (!tab.sftpPaneMounted && profile == null) {
      _showWorkspaceMessage('SFTP is available for SSH sessions.');
      return;
    }

    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      _selectedTerminalId = terminalId;
      _selectedTerminalViewId =
          view?.id ?? tab.rootLayout.views.firstOrNull?.id;
      if (!tab.sftpPaneMounted && profile != null) {
        tab.sftpConnectRequest = _sftpConnectRequestForProfile(tab, profile);
      }
      tab.sftpPaneMounted = true;
      tab.pageMode = _TerminalTabPageMode.sftp;
      _editorRequest = null;
    });
  }

  void _connectTerminalTabSftpHost(int terminalId, _HostItem item) {
    final tab = _terminalTabs.where((tab) => tab.id == terminalId).firstOrNull;
    if (tab == null) {
      return;
    }
    final request = _sftpConnectRequestForHostItem(
      ++tab.sftpConnectRequestId,
      item,
    );
    if (request == null) {
      return;
    }

    _setWorkspaceState(() {
      _tab = _WorkspaceTab.sessions;
      _workspaceOverviewActive = false;
      _selectedTerminalId = terminalId;
      final selectedViewId = _selectedTerminalViewId;
      _selectedTerminalViewId =
          selectedViewId != null && tab.rootLayout.containsView(selectedViewId)
          ? selectedViewId
          : tab.rootLayout.views.firstOrNull?.id;
      tab.sftpPaneMounted = true;
      tab.pageMode = _TerminalTabPageMode.sftp;
      tab.sftpConnectRequest = request;
      _editorRequest = null;
    });
  }

  _SftpConnectRequest? _sftpConnectRequestForHostItem(
    int requestId,
    _HostItem item,
  ) {
    final host = _hostEntries
        .where((host) => host.id != null && host.id == item.id)
        .firstOrNull;
    final auth = _sshAuthForHost(host, feature: 'SFTP');
    if (auth == null) {
      return null;
    }
    return _SftpConnectRequest(
      id: requestId,
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
  }

  _TerminalViewEntry? _terminalViewForSftp(
    _TerminalTab tab,
    int? terminalViewId,
  ) {
    final selectedViewId =
        terminalViewId ??
        (_selectedTerminalId == tab.id ? _selectedTerminalViewId : null);
    if (selectedViewId != null) {
      return tab.rootLayout.viewFor(selectedViewId) ?? tab.primaryView;
    }
    return tab.primaryView;
  }

  _SftpConnectRequest _sftpConnectRequestForProfile(
    _TerminalTab tab,
    SshConnectionProfile profile,
  ) {
    return _SftpConnectRequest(
      id: ++tab.sftpConnectRequestId,
      host: _sftpHostItemForProfile(profile),
      auth: _SftpRemoteAuth(
        host: profile.host,
        port: profile.port,
        username: profile.username,
        password: profile.password,
        privateKey: profile.privateKey,
        certificate: profile.certificate,
        passphrase: profile.passphrase,
        proxy: profile.proxy,
        knownHostsPath: profile.knownHostsPath,
      ),
    );
  }

  _HostItem _sftpHostItemForProfile(SshConnectionProfile profile) {
    final existingHost = profile.hostId == null
        ? null
        : _hosts.where((host) => host.id == profile.hostId).firstOrNull;
    if (existingHost != null) {
      return existingHost;
    }

    final label =
        _emptyToNull(profile.label) ?? '${profile.username}@${profile.host}';
    return _HostItem(
      id: profile.hostId ?? 0,
      name: label,
      subtitle: 'ssh, ${profile.username}',
      icon: Icons.public_rounded,
      color: _orange,
      type: NautermHostType.remote.storageValue,
      host: profile.host,
      port: profile.port,
      username: profile.username,
    );
  }

  Future<bool> _confirmQuitIfNeeded() async {
    return _confirmOpenTerminalTabsAction(
      title: 'Quit Nauterm?',
      singularMessage: 'There is still 1 terminal tab open. Quit anyway?',
      pluralMessage: (count) =>
          'There are still $count terminal tabs open. Quit anyway?',
      confirmLabel: 'Quit',
    );
  }

  Future<bool> _confirmCloseWindowIfNeeded() async {
    return _confirmOpenTerminalTabsAction(
      title: 'Close window?',
      singularMessage:
          'There is still 1 terminal tab open. Close this window anyway?',
      pluralMessage: (count) =>
          'There are still $count terminal tabs open. Close this window anyway?',
      confirmLabel: 'Close',
    );
  }

  Future<bool> _confirmOpenTerminalTabsAction({
    required String title,
    required String singularMessage,
    required String Function(int count) pluralMessage,
    required String confirmLabel,
  }) async {
    final tabCount = _allTerminalTabs.length;
    if (tabCount == 0) {
      return true;
    }

    final result = await _showWorkspaceDialog<bool>(
      builder: (context) {
        return _WorkspaceConfirmDialog(
          title: Text(tr(title)),
          message: tabCount == 1 ? singularMessage : pluralMessage(tabCount),
          confirmLabel: confirmLabel,
        );
      },
    );

    return result ?? false;
  }

  Future<bool> _confirmDelete({
    required String title,
    required String message,
    String confirmLabel = 'Delete',
    _AiAssistantColors? colors,
  }) async {
    if (!mounted) {
      return false;
    }
    final result = await _showWorkspaceDialog<bool>(
      builder: (context) {
        final dialog = _WorkspaceConfirmDialog(
          title: Text(tr(title)),
          message: message,
          confirmLabel: confirmLabel,
        );
        return colors == null
            ? dialog
            : _WorkspaceDialogThemeScope(colors: colors, child: dialog);
      },
    );

    return result ?? false;
  }

  Future<T?> _showWorkspaceDialog<T>({
    required WidgetBuilder builder,
    bool barrierDismissible = true,
  }) async {
    _overlayController.dismissTransientOverlays();
    FocusManager.instance.primaryFocus?.unfocus();
    requestMainWindowFocus();
    final result = await showNautermDialog<T>(
      context: context,
      barrierDismissible: barrierDismissible,
      builder: builder,
    );
    if (mounted) {
      _workspaceFocusNode.requestFocus();
    }
    return result;
  }

  _TerminalTab? get _selectedTerminalTab {
    final selectedId = _selectedTerminalId;
    if (selectedId == null) {
      return null;
    }

    for (final tab in _terminalTabs) {
      if (tab.id == selectedId) {
        return tab;
      }
    }

    return null;
  }
}
