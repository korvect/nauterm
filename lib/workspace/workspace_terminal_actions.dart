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
          title: Text(context.tr('workspace.dialog.closeTerminal')),
          message: 'workspace.dialog.areYouSureYouWantToCloseThisTerminal',
          confirmLabel: 'common.action.close',
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
        passphrase: auth.passphrase,
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

      final activeController = view.activeTab.controller;
      final sshProfile = activeController.sshProfile;
      final shellPath = _resolvedLocalShellPath(activeController.shellPath);

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
          sshCompatibilityMode: sshProfile.sshCompatibilityMode,
          config: currentTerminalConfig(),
          theme: view.theme,
          recorder: _createTerminalRecorder(
            title: title,
            hostId: sshProfile.hostId,
            host: sshProfile.host,
            port: sshProfile.port,
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
          shellPath: shellPath,
          environment: _terminalEnvironmentForTheme(view.theme),
          theme: view.theme,
          recorder: _createTerminalRecorder(
            title: title,
            username: _localShellUsername(),
            sourceHostUuid: view.activeTab.sourceHostUuid,
            shellPath: shellPath,
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
          sourceHostUuid: sshProfile == null
              ? view.activeTab.sourceHostUuid
              : null,
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

    final activeController = targetView.activeTab.controller;
    final sshProfile = activeController.sshProfile;
    final shellPath = _resolvedLocalShellPath(activeController.shellPath);

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
        sshCompatibilityMode: sshProfile.sshCompatibilityMode,
        config: currentTerminalConfig(),
        theme: targetView.theme,
        recorder: _createTerminalRecorder(
          title: title,
          hostId: sshProfile.hostId,
          host: sshProfile.host,
          port: sshProfile.port,
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
        shellPath: shellPath,
        environment: _terminalEnvironmentForTheme(targetView.theme),
        theme: targetView.theme,
        recorder: _createTerminalRecorder(
          title: title,
          username: _localShellUsername(),
          sourceHostUuid: targetView.activeTab.sourceHostUuid,
          shellPath: shellPath,
        ),
        onExit: () => _closeTerminalViewTab(tab.id, viewId, viewId),
      );
    }
    final newView = _TerminalViewEntry(
      id: viewId,
      title: title,
      controller: controller,
      theme: targetView.theme,
      sourceHostUuid: sshProfile == null
          ? targetView.activeTab.sourceHostUuid
          : null,
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

  void _showTerminalTabSftp(
    int terminalId, {
    int? terminalViewId,
    String? initialPath,
  }) {
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
      if ((!tab.sftpPaneMounted || initialPath != null) && profile != null) {
        tab.sftpConnectRequest = _sftpConnectRequestForProfile(
          tab,
          profile,
          initialPath: initialPath,
        );
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
      auth: _SftpRemoteAuth.fromHostAuth(
        auth,
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
    SshConnectionProfile profile, {
    String? initialPath,
  }) {
    return _SftpConnectRequest(
      id: ++tab.sftpConnectRequestId,
      host: _sftpHostItemForProfile(profile),
      auth: _SftpRemoteAuth.fromSshProfile(profile),
      initialPath: initialPath ?? '~',
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
    return _confirmApplicationExit(closeWindow: false);
  }

  Future<bool> _confirmCloseWindowIfNeeded() async {
    return _confirmApplicationExit(closeWindow: true);
  }

  Future<bool> _confirmApplicationExit({required bool closeWindow}) async {
    final tabCount = _allTerminalTabs.length;
    final forwardCount = _runningPortForwardIds.length;
    final hasSftpWork = _workspaceSftpPanes.any((pane) => pane._hasPendingWork);
    final hasExternalEdits = _workspaceSftpPanes.any(
      (pane) => pane._externalEditSessions.isNotEmpty,
    );
    final exportingKey = _workspaceKeyExportsInProgress.isNotEmpty;
    if (tabCount == 0 &&
        forwardCount == 0 &&
        !hasSftpWork &&
        !hasExternalEdits &&
        !exportingKey) {
      _restoreOnShutdown = false;
      return true;
    }

    final scope = closeWindow ? 'workspace.closeWindow' : 'workspace.quit';
    final messageKey = switch ((tabCount, forwardCount)) {
      (0, 0) => null,
      (0, _) => 'activeForwards',
      (1, 0) => 'openTab',
      (_, 0) => 'openTabs',
      _ => 'openTabsAndForwards',
    };
    final hasRemoteSessions = _allTerminalTabs.any(
      (tab) => tab.controllers.any(
        (terminal) =>
            terminal.connectionStatus.phase ==
                TerminalConnectionPhase.connected &&
            (terminal.sshProfile != null || terminal.telnetProfile != null),
      ),
    );
    final message = [
      if (messageKey != null)
        context.tr(
          '$scope.$messageKey',
          args: {
            'count': tabCount == 0 ? forwardCount : tabCount,
            'tabs': tabCount,
            'forwards': forwardCount,
          },
        ),
      if (hasRemoteSessions)
        context.tr('workspace.closeWindow.remoteSessions.description'),
      if (hasSftpWork) context.tr('workspace.closeWindow.sftpInProgress'),
      if (exportingKey)
        context.tr('workspace.closeWindow.keyExport.description'),
      if (hasExternalEdits)
        context.tr('workspace.closeWindow.externalEdits.description'),
    ].join('\n\n');
    final result = await _showWorkspaceDialog<bool>(
      builder: (context) => _WorkspaceConfirmDialog(
        title: Text(
          context.tr(
            closeWindow
                ? 'workspace.dialog.closeWindow'
                : 'workspace.dialog.quit',
            args: const {'app': 'Nauterm'},
          ),
        ),
        message: message,
        confirmLabel: closeWindow
            ? 'common.action.close'
            : 'common.action.quit',
      ),
    );
    if (result != true) return false;
    _restoreOnShutdown =
        tabCount + forwardCount > 0 &&
        workspaceRestoreBehavior != WorkspaceRestoreBehavior.never;
    return true;
  }

  Future<bool> _confirmDelete({
    required String title,
    required String message,
    String? details,
    Map<String, Object?> messageArgs = const {},
    String confirmLabel = 'common.action.delete',
    _AiAssistantColors? colors,
  }) async {
    if (!mounted) {
      return false;
    }
    final result = await _showWorkspaceDialog<bool>(
      builder: (context) {
        final dialog = _WorkspaceConfirmDialog(
          title: Text(context.tr(title)),
          message: message,
          details: details,
          messageArgs: messageArgs,
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
    Future<void>? dismissWhen,
  }) async {
    _overlayController.dismissTransientOverlays();
    FocusManager.instance.primaryFocus?.unfocus();
    requestMainWindowFocus();
    final result = await showNautermDialog<T>(
      context: context,
      barrierDismissible: barrierDismissible,
      dismissWhen: dismissWhen,
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
