part of 'nauterm_workspace.dart';

const _workspacePanelTransitionDuration = Duration(milliseconds: 180);
const _workspacePanelTransitionCurve = Curves.easeInOutCubic;

@visibleForTesting
bool shouldUseTerminalChrome({
  required bool terminalPageVisible,
  required TerminalConnectionPhase phase,
  required bool hasConnectedOnce,
}) {
  return terminalPageVisible &&
      (phase == TerminalConnectionPhase.connected || hasConnectedOnce);
}

extension _NautermWorkspaceRendering on _NautermWorkspaceState {
  void _handleTerminalBell(_TerminalTab tab) {
    if (!terminalBellConfig.tabIndicator || tab.bellIndicator) return;
    _setWorkspaceState(() => tab.bellIndicator = true);
  }

  Widget _buildTopBar(_TerminalTab? selectedTerminalTab) {
    Widget buildTopBar() {
      final aiAssistantAvailable = _aiAssistantAvailable(selectedTerminalTab);
      final terminalToolsAvailable =
          aiAssistantAvailable && !_workspaceOverviewActive;
      final assistantOpen = _aiAssistantOpen(selectedTerminalTab);
      return _TopBar(
        selectedTab: _tab,
        currentWorkspace: _selectedWorkspace,
        terminalTabs: _terminalTabs,
        selectedTerminalId: _selectedTerminalId,
        selectedTerminalViewId: _selectedTerminalViewId,
        workspacePageActive:
            _tab == _WorkspaceTab.sessions && _workspaceOverviewActive,
        workspacePageEnabled: workspacePageEnabled,
        sftpTabEnabled: sftpTabEnabled,
        terminalTheme: selectedTerminalTab?.theme ?? defaultTerminalTheme,
        terminalChrome: _usesTerminalChrome(selectedTerminalTab),
        connectionPageChrome: _usesConnectionPageChrome(selectedTerminalTab),
        onTabSelected: _selectWorkspaceTab,
        onTerminalTabSelected: _selectTerminalTab,
        onTerminalTabClosed: _closeTerminalTab,
        onWorkspaceSelected: _showSelectedWorkspaceSessions,
        onQuickConnect: _openQuickConnectDialog,
        aiAssistantAvailable: aiAssistantAvailable,
        aiAssistantOpen:
            assistantOpen &&
            (!terminalToolsAvailable ||
                selectedTerminalTab?.toolPanelMode ==
                    _TerminalToolPanelMode.ai),
        onAiAssistant: () => _toggleAiAssistant(selectedTerminalTab),
        terminalToolsAvailable: terminalToolsAvailable,
        terminalToolsOpen: terminalToolsAvailable && assistantOpen,
        onTerminalTools: () => _toggleTerminalTools(selectedTerminalTab),
        onStartWindowDrag: widget.onStartWindowDrag,
        onToggleWindowMaximized: widget.onToggleWindowMaximized,
        isFullscreen: isMainWindowFullscreen(),
      );
    }

    final titleListenable = _terminalTitleListenable();
    if (titleListenable == null) {
      return AnimatedBuilder(
        animation: Listenable.merge([
          sftpTabEnabledListenable,
          workspacePageEnabledListenable,
        ]),
        builder: (context, _) => buildTopBar(),
      );
    }

    return AnimatedBuilder(
      animation: Listenable.merge([
        titleListenable,
        sftpTabEnabledListenable,
        workspacePageEnabledListenable,
      ]),
      builder: (context, _) => buildTopBar(),
    );
  }

  Listenable? _terminalTitleListenable() {
    final controllers = [for (final tab in _terminalTabs) ...tab.controllers];
    if (controllers.isEmpty) {
      return null;
    }
    return Listenable.merge(controllers);
  }

  bool _usesTerminalChrome(_TerminalTab? selectedTerminalTab) {
    final controller = selectedTerminalTab?.controller;
    if (controller == null) {
      return false;
    }
    return shouldUseTerminalChrome(
      terminalPageVisible:
          selectedTerminalTab!.pageMode == _TerminalTabPageMode.ssh,
      phase: controller.connectionStatus.phase,
      hasConnectedOnce: controller.hasConnectedOnce,
    );
  }

  bool _usesConnectionPageChrome(_TerminalTab? selectedTerminalTab) {
    if (selectedTerminalTab?.pageMode == _TerminalTabPageMode.sftp) {
      return false;
    }
    final view = selectedTerminalTab?.primaryView.activeTab;
    if (view?.pendingConnection != null) return true;
    if (view?.connectionPageVisible == true) return true;
    final controller = view?.controller;
    if (controller == null) return false;
    return _shouldShowConnectionPage(
      controller,
      controller.connectionStatus,
      controller.sshProfile,
      controller.serialProfile,
      controller.telnetProfile,
    );
  }

  Widget _buildWorkspaceBody(_TerminalTab? selectedTerminalTab) {
    final vaultWorkspace = _WorkspaceItemSelectionScope(
      key: ValueKey('workspace-item-selection:${_section.name}'),
      controller: _itemSelectionControllers[_section]!,
      child: _VaultWorkspace(
        selectedSection: _section,
        groups: _groups,
        hosts: _hosts,
        tags: _tagEntries,
        keys: _keys,
        identities: _identities,
        portForwards: _portForwards,
        proxies: _proxies,
        snippetPackages: _snippetPackages,
        snippets: _snippets,
        terminalLogs: _visibleTerminalLogs(),
        terminalLogsHasMore: _terminalLogsHasMore,
        terminalLogsLoading: _terminalLogsLoading,
        terminalCaptureDiskUsage: _terminalCaptureDiskUsage,
        shellHistory: _shellHistory,
        selectedLogId: _selectedLogId,
        knownHostsText: _knownHostsText,
        loadingData: _loadingData,
        onSectionSelected: (section) {
          _setWorkspaceState(() {
            _section = section;
          });
        },
        onCreateGroup: (parentId) => _createGroup(parentId),
        onCreateHost: (groupId) => _createHost(groupId),
        onImportHosts: _importHosts,
        onExportHosts: _exportHosts,
        onCreateKey: _createKey,
        onCreateCertificate: _createCertificate,
        onGenerateKey: _generateKey,
        onCreateIdentity: _createIdentity,
        onCreateSnippet: (packageId) => _createSnippet(packageId),
        onCreateSnippetPackage: _createSnippetPackage,
        onCreatePortForward: _createPortForward,
        onPortForwardEdit: _editPortForward,
        onPortForwardToggle: _togglePortForward,
        onCreateProxy: _createProxy,
        onProxyEdit: _editProxy,
        onProxyContextAction: _handleProxyContextAction,
        onProxyContextActions: _handleProxyContextActions,
        onGroupContextAction: _handleGroupContextAction,
        onGroupContextActions: _handleGroupContextActions,
        onHostContextAction: _handleHostContextAction,
        onHostContextActions: _handleHostContextActions,
        onHostConnected: _connectHost,
        onHostQueryConnected: _connectHostQuery,
        onSaveTag: _saveTag,
        onDeleteTag: _deleteTag,
        onSnippetPackageContextAction: _handleSnippetPackageContextAction,
        onSnippetPackageContextActions: _handleSnippetPackageContextActions,
        onSnippetContextAction: _handleSnippetContextAction,
        onSnippetContextActions: _handleSnippetContextActions,
        onSnippetRun: _runSnippet,
        onShowShellHistory: _showShellHistory,
        onLogReplay: _replayTerminalLog,
        onLogSelected: _selectTerminalLog,
        onLogDelete: _deleteTerminalLog,
        onLogExport: _exportTerminalLog,
        onClearLogs: _clearTerminalLogs,
        onLoadMoreLogs: _loadMoreTerminalLogs,
        onImportKnownHosts: _importKnownHosts,
        onKnownHostContextAction: _handleKnownHostContextAction,
        onKnownHostContextActions: _handleKnownHostContextActions,
        onKeyContextAction: _handleKeyContextAction,
        onKeyContextActions: _handleKeyContextActions,
        onIdentityContextAction: _handleIdentityContextAction,
        onIdentityContextActions: _handleIdentityContextActions,
        onOpenSettings: widget.onOpenSettings,
        onOpenLocalTerminal: _openLocalTerminalTab,
        onOpenSerialTerminal: _openSerialTerminalDialog,
        currentWorkspaceName: _activeSessionWorkspace.name,
        dataStore: _dataStore,
      ),
    );
    final selectedTerminalPageActive =
        _tab == _WorkspaceTab.sessions &&
        !_workspaceOverviewActive &&
        selectedTerminalTab != null;
    final sftpActive =
        sftpTabEnabled &&
        _tab == _WorkspaceTab.sftp &&
        !selectedTerminalPageActive;
    final terminalToolSftpActive =
        selectedTerminalPageActive &&
        selectedTerminalTab.pageMode == _TerminalTabPageMode.ssh &&
        selectedTerminalTab.aiAssistantOpen &&
        selectedTerminalTab.toolPanelMode == _TerminalToolPanelMode.sftp &&
        _aiTerminalController(selectedTerminalTab)?.sshProfile != null;
    final sftpPane = (_sftpPaneMounted || sftpActive)
        ? _SftpPane(
            sessionId: 'workspace:${_activeSessionWorkspace.id}:sftp',
            active: sftpActive,
            groups: _groups,
            hosts: _hosts,
            tags: _tagEntries,
            dataStore: _dataStore,
            connectRequest: _sftpConnectRequest,
            onHostSelected: _connectSftpHostInSftpPage,
            onRemoteConnected: _handleSftpRemoteConnected,
            manageFileDrop: false,
          )
        : null;
    _syncSftpFileDropEnabled(
      sftpActive ||
          terminalToolSftpActive ||
          (_tab == _WorkspaceTab.sessions &&
              !_workspaceOverviewActive &&
              selectedTerminalTab?.pageMode == _TerminalTabPageMode.sftp &&
              selectedTerminalTab?.id == _selectedTerminalId),
    );
    final content = selectedTerminalPageActive
        ? _buildTerminalTabContent(selectedTerminalTab)
        : switch (_tab == _WorkspaceTab.sftp && !sftpTabEnabled
              ? _WorkspaceTab.sessions
              : _tab) {
            _WorkspaceTab.vaults => vaultWorkspace,
            _WorkspaceTab.sftp => const SizedBox.shrink(),
            _WorkspaceTab.sessions => _WorkspaceSessionsScaffold(
              workspaces: _workspaces,
              selectedWorkspaceId: _selectedWorkspaceId,
              onWorkspaceSelected: _selectRuntimeWorkspace,
              onWorkspaceClosed: (workspaceId) =>
                  unawaited(_confirmAndCloseRuntimeWorkspace(workspaceId)),
              onWorkspaceRenamed: (workspaceId) =>
                  unawaited(_renameRuntimeWorkspace(workspaceId)),
              onCreateWorkspace: _createRuntimeWorkspace,
              child: _WorkspaceSessionsPane(
                workspace: _selectedWorkspace,
                selectedTerminalId: _selectedTerminalId,
                onNewTerminal: _openWorkspaceQuickConnectDialog,
                onSessionSelected: _focusWorkspaceSession,
                onSessionOpened: _selectTerminalTab,
                sessionBuilder: (session) => _buildTerminalViewLayout(
                  session,
                  session.rootLayout,
                  activateOnPointerDown: false,
                  sftpPageAvailable: false,
                ),
              ),
            ),
          };
    final preserveSelectedTerminalSftp =
        sftpActive &&
        selectedTerminalTab != null &&
        selectedTerminalTab.pageMode == _TerminalTabPageMode.sftp &&
        selectedTerminalTab.sftpPaneMounted;
    final pageKey = preserveSelectedTerminalSftp
        ? 'terminal:${selectedTerminalTab.id}'
        : _workspacePageKey(selectedTerminalTab);
    final pageContent = preserveSelectedTerminalSftp
        ? _buildTerminalTabContent(selectedTerminalTab)
        : content;
    final keepPageContent = !sftpActive || preserveSelectedTerminalSftp;

    return LayoutBuilder(
      builder: (context, _) {
        final editorOpen = _editorStack.isNotEmpty;

        return Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(
              child: ExcludeFocus(
                excluding: editorOpen,
                child: Stack(
                  fit: StackFit.expand,
                  children: [
                    if (sftpPane != null)
                      Offstage(
                        offstage: !sftpActive,
                        child: TickerMode(enabled: sftpActive, child: sftpPane),
                      ),
                    if (keepPageContent)
                      Offstage(
                        offstage: sftpActive,
                        child: TickerMode(
                          enabled: !sftpActive,
                          child: KeyedSubtree(
                            key: ValueKey('workspace-top-page:$pageKey'),
                            child: RepaintBoundary(child: pageContent),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
            AnimatedContainer(
              duration: const Duration(milliseconds: 170),
              curve: Curves.easeOutCubic,
              width: editorOpen ? _workspaceEditorDrawerWidth : 0,
              child: ClipRect(
                child: Align(
                  alignment: Alignment.centerRight,
                  widthFactor: editorOpen ? 1 : 0,
                  child: SizedBox(
                    width: _workspaceEditorDrawerWidth,
                    child: Stack(
                      fit: StackFit.expand,
                      children: [
                        for (final section in _SidebarSection.values)
                          for (
                            var index = 0;
                            index < _editorStackFor(section).length;
                            index++
                          )
                            Offstage(
                              key: ObjectKey(_editorStackFor(section)[index]),
                              offstage:
                                  section != _section ||
                                  index != _editorStackFor(section).length - 1,
                              child: TickerMode(
                                enabled:
                                    section == _section &&
                                    index ==
                                        _editorStackFor(section).length - 1,
                                child: FocusScope(
                                  key: ValueKey(
                                    'workspace-editor-focus-region:${section.name}:$index',
                                  ),
                                  child: _WorkspaceEditorDrawer(
                                    request: _editorStackFor(
                                      section,
                                    )[index].request,
                                    groups: _groupEntries,
                                    hosts: _hostEntries,
                                    keys: _keyEntries,
                                    identities: _identityEntries,
                                    tags: _tagEntries,
                                    proxies: _proxyEntries,
                                    snippetPackages: _snippetPackages,
                                    snippets: _snippets,
                                    shellHistory: _shellHistory,
                                    terminalThemeCatalog: _terminalThemeCatalog,
                                    onClose: _closeEditor,
                                    onCreateGroup: _createGroupForEditor,
                                    onCreateGroupFromProtocol:
                                        _createGroupFromProtocol,
                                    onCreateCredential:
                                        _createCredentialForEditor,
                                    onCreateIdentity: _createIdentityForEditor,
                                    onCreateProxy: _createProxyForEditor,
                                    onCreateTag: _createTag,
                                    onCreateSnippet: _createSnippetForEditor,
                                    onCreateSnippetPackage:
                                        _createSnippetPackageForEditor,
                                    onEditHostEnvironment:
                                        _editHostEnvironmentForEditor,
                                    onSaveGeneratedKey: _saveGeneratedKey,
                                    onExportKey: _exportKeyToHost,
                                    onShowNotification: _showWorkspaceMessage,
                                    onSaveGroup: _saveGroup,
                                    onDuplicateGroup: (group) {
                                      _handleGroupContextAction(
                                        _mapGroups([group], _hostEntries).first,
                                        _ContextMenuActionId.duplicate,
                                      );
                                    },
                                    onDeleteGroup: (group) {
                                      () async {
                                        final item = _mapGroups([
                                          group,
                                        ], _hostEntries).first;
                                        await _deleteGroup(item);
                                        if (mounted &&
                                            !_groupEntries.any(
                                              (entry) => entry.id == group.id,
                                            )) {
                                          _closeEditor();
                                        }
                                      }();
                                    },
                                    onSaveHost: _saveHost,
                                    onConnectHost: (host) {
                                      _connectHost(
                                        _mapHost(
                                          host,
                                          _identityEntries,
                                          _tagNamesByUuid(_tagEntries),
                                        ),
                                      );
                                    },
                                    onDuplicateHost: (host) {
                                      _handleHostContextAction(
                                        _mapHost(
                                          host,
                                          _identityEntries,
                                          _tagNamesByUuid(_tagEntries),
                                        ),
                                        _ContextMenuActionId.duplicate,
                                      );
                                    },
                                    onDeleteHost: (host) {
                                      () async {
                                        final item = _mapHost(
                                          host,
                                          _identityEntries,
                                          _tagNamesByUuid(_tagEntries),
                                        );
                                        await _deleteHost(item);
                                        if (mounted &&
                                            !_hostEntries.any(
                                              (entry) => entry.id == host.id,
                                            )) {
                                          _closeEditor();
                                        }
                                      }();
                                    },
                                    onSaveHostEnvironment:
                                        _saveHostEnvironmentForEditor,
                                    onSaveKey: _saveKey,
                                    onDuplicateKey: (key) {
                                      _handleKeyContextAction(
                                        _mapKey(key),
                                        _ContextMenuActionId.duplicate,
                                      );
                                    },
                                    onExportKeyToHost: (key) {
                                      _exportKey(_mapKey(key));
                                    },
                                    onExportKeyToFile: (key) {
                                      unawaited(_exportKeyToFile(key));
                                    },
                                    onDeleteKey: (key) {
                                      () async {
                                        await _deleteKey(_mapKey(key));
                                        if (mounted &&
                                            !_keyEntries.any(
                                              (entry) => entry.id == key.id,
                                            )) {
                                          _closeEditor();
                                        }
                                      }();
                                    },
                                    onSaveIdentity: _saveIdentity,
                                    onDuplicateIdentity: (identity) {
                                      _handleIdentityContextAction(
                                        _mapIdentity(identity),
                                        _ContextMenuActionId.duplicate,
                                      );
                                    },
                                    onDeleteIdentity: (identity) {
                                      () async {
                                        await _deleteIdentity(
                                          _mapIdentity(identity),
                                        );
                                        if (mounted &&
                                            !_identityEntries.any(
                                              (entry) =>
                                                  entry.id == identity.id,
                                            )) {
                                          _closeEditor();
                                        }
                                      }();
                                    },
                                    onSavePortForward: _savePortForward,
                                    onDeletePortForward: _deletePortForward,
                                    onSaveProxy: _saveProxy,
                                    onDuplicateProxy: (proxy) {
                                      final item = _mapProxy(
                                        proxy,
                                        _identityEntries,
                                      );
                                      _handleProxyContextAction(
                                        item,
                                        _ContextMenuActionId.duplicate,
                                      );
                                    },
                                    onDeleteProxy: (proxy) {
                                      () async {
                                        await _confirmAndDeleteProxy(
                                          proxy.id,
                                          proxy.name,
                                        );
                                        if (mounted &&
                                            !_proxyEntries.any(
                                              (entry) => entry.id == proxy.id,
                                            )) {
                                          _closeEditor();
                                        }
                                      }();
                                    },
                                    onSaveSnippetPackage: _saveSnippetPackage,
                                    onDeleteSnippetPackage: (package) {
                                      () async {
                                        await _confirmAndDeleteSnippetPackage(
                                          package,
                                        );
                                        if (mounted &&
                                            !_snippetPackageEntries.any(
                                              (entry) => entry.id == package.id,
                                            )) {
                                          _closeEditor();
                                        }
                                      }();
                                    },
                                    onSaveSnippet: _saveSnippet,
                                    onDuplicateSnippet: _duplicateSnippet,
                                    onDeleteSnippet: _deleteSnippet,
                                    onCreateSnippetFromShellHistory:
                                        _createSnippetFromShellHistory,
                                    onClearShellHistory: _clearShellHistory,
                                  ),
                                ),
                              ),
                            ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ],
        );
      },
    );
  }

  String _workspacePageKey(_TerminalTab? selectedTerminalTab) {
    final selectedTerminalPageActive =
        _tab == _WorkspaceTab.sessions &&
        !_workspaceOverviewActive &&
        selectedTerminalTab != null;
    if (selectedTerminalPageActive) {
      return 'terminal:${selectedTerminalTab.id}';
    }

    return switch (_tab) {
      _WorkspaceTab.vaults => 'vaults',
      _WorkspaceTab.sftp => 'sftp',
      _WorkspaceTab.sessions => 'sessions',
    };
  }

  Widget _buildTerminalTabContent(
    _TerminalTab tab, {
    bool activateTerminalOnPointerDown = true,
  }) {
    final terminalVisible = tab.pageMode == _TerminalTabPageMode.ssh;
    final sftpVisible = tab.pageMode == _TerminalTabPageMode.sftp;
    final sftpPage = (tab.sftpPaneMounted || sftpVisible)
        ? _buildTerminalTabSftpPage(tab, active: _terminalTabSftpActive(tab))
        : null;

    return Stack(
      fit: StackFit.expand,
      children: [
        Offstage(
          offstage: !terminalVisible,
          child: TickerMode(
            enabled: terminalVisible,
            child: _buildTerminalViewLayout(
              tab,
              tab.rootLayout,
              activateOnPointerDown: activateTerminalOnPointerDown,
            ),
          ),
        ),
        if (sftpPage != null)
          Offstage(
            offstage: !sftpVisible,
            child: TickerMode(enabled: sftpVisible, child: sftpPage),
          ),
      ],
    );
  }

  bool _terminalTabSftpActive(_TerminalTab tab) {
    return _tab == _WorkspaceTab.sessions &&
        _selectedTerminalId == tab.id &&
        tab.pageMode == _TerminalTabPageMode.sftp;
  }

  Widget _buildTerminalTabSftpPage(_TerminalTab tab, {required bool active}) {
    return _SftpPane(
      key: ValueKey('terminal-sftp:${tab.id}'),
      sessionId: 'terminal:${tab.id}:sftp-page',
      active: active,
      groups: _groups,
      hosts: _hosts,
      tags: _tagEntries,
      dataStore: _dataStore,
      connectRequest: tab.sftpConnectRequest,
      onHostSelected: (host) => _connectTerminalTabSftpHost(tab.id, host),
      onRemoteConnected: _handleSftpRemoteConnected,
      manageFileDrop: false,
      remoteOnly: true,
      sshEditorController: _terminalViewForSftp(
        tab,
        _selectedTerminalViewId,
      )?.controller,
      onSshEditorOpened: () => _showTerminalTabSsh(tab.id),
      onSshSelected: () => _showTerminalTabSsh(tab.id),
    );
  }

  Widget _buildTerminalViewLayout(
    _TerminalTab tab,
    _TerminalViewLayout layout, {
    required bool activateOnPointerDown,
    bool sftpPageAvailable = true,
  }) {
    return switch (layout) {
      _TerminalViewLeaf(:final view) => _buildTerminalView(
        tab,
        view,
        activateOnPointerDown: activateOnPointerDown,
        sftpPageAvailable: sftpPageAvailable,
      ),
      _TerminalSplitLayout(
        :final id,
        :final axis,
        :final children,
        :final theme,
      ) =>
        _TerminalSplitPaneView(
          key: ValueKey('split:${tab.id}:$id'),
          axis: axis,
          theme: theme,
          children: [
            for (final child in children)
              _buildTerminalViewLayout(
                tab,
                child,
                activateOnPointerDown: activateOnPointerDown,
                sftpPageAvailable: sftpPageAvailable,
              ),
          ],
        ),
    };
  }

  TerminalViewTabToolbar? _terminalViewTabToolbar(
    _TerminalTab tab,
    _TerminalViewEntry view, {
    bool sftpPageAvailable = true,
  }) {
    final selectedViewId = _selectedTerminalViewId;
    final selected =
        _selectedTerminalId == tab.id &&
        (selectedViewId == view.id ||
            (selectedViewId == null && tab.primaryView.id == view.id));

    return TerminalViewTabToolbar(
      sessionTitle: () => _terminalViewSessionTitle(view.activeTab),
      sessionTitleListenable: view.activeTab.controller,
      sftpAvailable:
          sftpPageAvailable &&
          (view.controller.sshProfile != null || tab.sftpPaneMounted),
      onSftpRequested: sftpPageAvailable
          ? () => _showTerminalTabSftp(tab.id, terminalViewId: view.id)
          : null,
      tabs: [
        for (final terminalTab in view.tabs)
          TerminalViewTabToolbarTab(
            title: _terminalViewTabDisplayTitle(terminalTab),
            selected: selected && terminalTab.id == view.activeTab.id,
            onSelected: () {
              _setWorkspaceState(() {
                _tab = _WorkspaceTab.sessions;
                _selectedTerminalId = tab.id;
                _selectedTerminalViewId = view.id;
                view.selectedTabId = terminalTab.id;
                _editorRequest = null;
              });
            },
            onClose: () => _confirmAndClose(
              () => _closeTerminalViewTab(tab.id, view.id, terminalTab.id),
              controller: terminalTab.controller,
            ),
          ),
      ],
    );
  }

  Widget _buildTerminalView(
    _TerminalTab tab,
    _TerminalViewEntry view, {
    required bool activateOnPointerDown,
    bool sftpPageAvailable = true,
  }) {
    final pendingConnection = view.activeTab.pendingConnection;
    if (pendingConnection != null) {
      return _PendingHostConnectionPage(
        key: ValueKey('pending-connection-${tab.id}'),
        pending: pendingConnection,
        onCloseRequested: () => _closeTerminalTab(tab.id),
        onConnect: (selection) =>
            _startPendingHostConnection(tab.id, selection),
      );
    }
    final connectionViewTab = view.activeTab;
    final selectedViewId = _selectedTerminalViewId;
    final selected =
        _selectedTerminalId == tab.id &&
        (selectedViewId == view.id ||
            (selectedViewId == null && tab.primaryView.id == view.id));
    final composerHistory = _composerHistoryForController(view.controller);
    final composerSuggestions = [
      ...composerHistory,
      ..._snippetComposerSuggestionsForController(view.controller),
    ];
    final terminalConfig =
        (tab.replay
                ? currentTerminalConfig().copyWith(
                    cursor: const TerminalCursorConfig(visible: false),
                    composer: const TerminalComposerConfig(enabled: false),
                  )
                : currentTerminalConfig())
            .copyWith(font: tab.font);
    Widget buildView() {
      return Listener(
        onPointerDown: (_) {
          if (!activateOnPointerDown) {
            return;
          }
          if (_selectedTerminalId == tab.id &&
              _selectedTerminalViewId == view.id) {
            return;
          }
          _setWorkspaceState(() {
            _selectedTerminalId = tab.id;
            _selectedTerminalViewId = view.id;
          });
        },
        child: Stack(
          fit: StackFit.expand,
          children: [
            _TerminalSessionView(
              key: ValueKey('${tab.id}:${view.id}'),
              controller: view.controller,
              theme: view.theme,
              config: terminalConfig,
              connectionKeys: _terminalConnectionKeys,
              connectionIdentities: _terminalConnectionIdentities,
              composerHistory: tab.replay ? const [] : composerHistory,
              composerSuggestions: tab.replay ? const [] : composerSuggestions,
              autofocusTerminal: selected,
              readOnly: tab.replay,
              bellEnabled: !tab.replay,
              onBell: tab.replay ? null : () => _handleTerminalBell(tab),
              composerSuggestionResolver: (input, limit) =>
                  _composerDirectorySuggestions(view, input, limit),
              composerVisible: tab.replay ? false : view.composerVisible,
              onComposerVisibilityChanged: tab.replay
                  ? null
                  : (visible) {
                      if (view.composerVisible == visible) {
                        return;
                      }
                      _setWorkspaceState(() => view.composerVisible = visible);
                    },
              tabToolbar: tab.replay
                  ? null
                  : _terminalViewTabToolbar(
                      tab,
                      view,
                      sftpPageAvailable: sftpPageAvailable,
                    ),
              onConnectionAuthSaved: tab.replay ? null : _saveTerminalAuth,
              onAddKeyRequested: tab.replay ? null : _createKey,
              onConnectionPageVisibilityChanged: tab.replay
                  ? null
                  : (visible) {
                      if (connectionViewTab.connectionPageVisible == visible) {
                        return;
                      }
                      _setWorkspaceState(
                        () => connectionViewTab.connectionPageVisible = visible,
                      );
                    },
              onEditHostRequested: tab.replay
                  ? null
                  : () {
                      final hostId =
                          view.controller.sshProfile?.hostId ??
                          view.controller.telnetProfile?.hostId;
                      final host = hostId == null
                          ? null
                          : _hosts
                                .where((entry) => entry.id == hostId)
                                .firstOrNull;
                      if (host != null) _editHost(host);
                    },
              onReloadConnection: tab.replay
                  ? null
                  : () =>
                        _reloadTerminalConnection(view.controller, view.theme),
              onSplitRequested: tab.replay
                  ? null
                  : (direction) {
                      _selectedTerminalId = tab.id;
                      _selectedTerminalViewId = view.id;
                      _splitSelectedTerminalTab(direction);
                    },
              onNewTabRequested: tab.replay
                  ? null
                  : () => _openLocalTerminalViewTab(tab.id, view.id),
              onSettingsRequested: tab.replay
                  ? null
                  : widget.onOpenTerminalSettings,
              onCloseRequested: tab.replay
                  ? null
                  : () => _confirmAndClose(
                      () => _closeTerminalViewTab(
                        tab.id,
                        view.id,
                        view.activeTab.id,
                      ),
                      controller: view.activeTab.controller,
                    ),
              dataStore: _dataStore,
            ),
            if (tab.replayLoading)
              _TerminalReplayLoadingOverlay(theme: view.theme),
          ],
        ),
      );
    }

    return AnimatedBuilder(
      animation: Listenable.merge(view.controllers.toList(growable: false)),
      builder: (context, _) => buildView(),
    );
  }

  List<String> _composerHistoryForController(TerminalController controller) {
    return [
      for (final entry in ShellHistoryReader.newestFirst(
        _shellHistoryForController(controller),
      ))
        if (entry.command.trim().isNotEmpty) entry.command.trim(),
    ];
  }

  List<ShellHistoryEntry> _shellHistoryForController(
    TerminalController controller,
  ) {
    _refreshShellHistoryIfNeeded(controller);
    final recording = controller.sessionRecording;
    final live = recording?.shellHistory ?? const [];
    final profile = controller.sshProfile;
    return [
      ...?_shellHistoryByController[controller],
      for (final entry in live)
        ShellHistoryEntry(
          sourceId: entry.id,
          command: entry.command,
          sessionId: entry.sessionId,
          title: entry.title,
          host: profile?.host,
          port: profile?.port,
          username: profile?.username,
          shellPath: profile == null
              ? _resolvedLocalShellPath(controller.shellPath)
              : null,
          createdAt: entry.timestamp,
        ),
    ];
  }

  void _refreshShellHistoryIfNeeded(
    TerminalController controller, {
    bool force = false,
  }) {
    if (controller.isDisposed) return;
    final profile = controller.sshProfile;
    if (profile != null) {
      _refreshRemoteShellHistoryIfNeeded(controller, profile, force: force);
      return;
    }
    final shellPath = _resolvedLocalShellPath(controller.shellPath);
    if (shellPath == null ||
        ShellHistoryReader.formatForShell(shellPath) == null) {
      return;
    }
    final now = DateTime.now();
    final lastRead = _shellHistoryLastRead[controller];
    if (_shellHistoryLoading.contains(controller) ||
        (!force &&
            lastRead != null &&
            now.difference(lastRead) < const Duration(seconds: 2))) {
      return;
    }
    _shellHistoryLoading.add(controller);
    _shellHistoryLastRead[controller] = now;
    unawaited(() async {
      try {
        final entries = await ShellHistoryReader.readLocal(
          shellPath: shellPath,
        );
        if (mounted && !controller.isDisposed) {
          _applyControllerShellHistory(controller, entries);
        }
      } finally {
        _shellHistoryLoading.remove(controller);
      }
    }());
  }

  void _refreshRemoteShellHistoryIfNeeded(
    TerminalController controller,
    SshConnectionProfile profile, {
    bool force = false,
  }) {
    if (controller.connectionStatus.phase !=
        TerminalConnectionPhase.connected) {
      return;
    }
    final now = DateTime.now();
    final lastRead = _shellHistoryLastRead[controller];
    if (_shellHistoryLoading.contains(controller) ||
        (!force &&
            lastRead != null &&
            now.difference(lastRead) < const Duration(seconds: 5))) {
      return;
    }
    _shellHistoryLoading.add(controller);
    _shellHistoryLastRead[controller] = now;
    unawaited(() async {
      try {
        final sessionId = controller.nativeSessionId;
        if (sessionId == null || sessionId == 0) return;
        final result =
            await FfiRemoteShellHistoryReader.readSessionInBackground(
              sessionId,
            );
        if (result.error != null) {
          NautermLog.warning(
            'shell_history',
            'Unable to read remote shell history.',
          );
          return;
        }
        final shellPath = (profile.shellPath ?? result.shell)?.trim();
        final format = ShellHistoryReader.formatForShell(shellPath);
        if (format == null) {
          NautermLog.warning(
            'shell_history',
            'Remote shell history format is unsupported.',
          );
          return;
        }
        final rawEntries = ShellHistoryReader.parse(
          result.content,
          format: format,
          readAt: DateTime.now(),
          shellPath: shellPath,
        );
        final entries = [
          for (final entry in rawEntries)
            ShellHistoryEntry(
              command: entry.command,
              host: profile.host,
              port: profile.port,
              username: profile.username,
              shellPath: shellPath,
              createdAt: entry.createdAt,
            ),
        ];
        if (mounted && !controller.isDisposed) {
          _applyControllerShellHistory(controller, entries);
        }
      } finally {
        _shellHistoryLoading.remove(controller);
      }
    }());
  }

  void _applyControllerShellHistory(
    TerminalController controller,
    List<ShellHistoryEntry> entries,
  ) {
    final current = _shellHistoryByController[controller];
    if (_sameShellHistoryEntries(current, entries)) return;
    _setWorkspaceState(() => _shellHistoryByController[controller] = entries);
    _shellHistoryPersistence = _shellHistoryPersistence.then((_) async {
      try {
        final merged = await ShellHistoryFileStore(
          NautermPaths.resolve().shellHistoryFile,
        ).merge(entries);
        if (mounted) {
          _setWorkspaceState(() => _shellHistory = merged);
        }
      } on Object catch (error, stackTrace) {
        NautermLog.warning(
          'shell_history',
          'Unable to merge shell history.',
          error: error,
          stackTrace: stackTrace,
        );
      }
    });
  }

  bool _sameShellHistoryEntries(
    List<ShellHistoryEntry>? left,
    List<ShellHistoryEntry> right,
  ) {
    if (left == null || left.length != right.length) return false;
    for (var index = 0; index < left.length; index++) {
      if (left[index].command != right[index].command) {
        return false;
      }
    }
    return true;
  }

  List<ShellHistoryEntry> _shellHistoryForDisplay(
    TerminalController controller,
  ) {
    return ShellHistoryReader.newestFirst(
      _shellHistoryForController(controller),
    );
  }

  void _syncSftpFileDropEnabled(bool enabled) {
    if (_sftpFileDropEnabled == enabled) {
      return;
    }
    _sftpFileDropEnabled = enabled;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || _sftpFileDropEnabled != enabled) {
        return;
      }
      unawaited(NautermFileDropChannel.instance.setEnabled(enabled));
    });
  }

  Widget _buildWorkspaceContent(_TerminalTab? selectedTerminalTab) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final aiAssistantOpen = _aiAssistantOpen(selectedTerminalTab);
        final aiAssistantColors = _aiAssistantColors(selectedTerminalTab);
        final aiConversation = _aiConversation(selectedTerminalTab);
        _watchAiConversationPersistence(aiConversation, selectedTerminalTab);
        final terminalPageActive =
            _tab == _WorkspaceTab.sessions &&
            !_workspaceOverviewActive &&
            selectedTerminalTab != null;
        final narrow = constraints.maxWidth < 900;
        final maximumPanelWidth = terminalPageActive
            ? math.min(640.0, math.max(240.0, constraints.maxWidth - 480))
            : narrow
            ? math.min(640.0, math.max(240.0, constraints.maxWidth - 24))
            : math.min(640.0, math.max(280.0, constraints.maxWidth - 360));
        final minimumPanelWidth = math.min(280.0, maximumPanelWidth);
        final panelWidth = _aiAssistantWidth(selectedTerminalTab)
            .clamp(minimumPanelWidth, maximumPanelWidth)
            .toDouble();
        void resizePanel(double delta) {
          _setAiAssistantWidth(
            selectedTerminalTab,
            (_aiAssistantWidth(selectedTerminalTab) - delta)
                .clamp(minimumPanelWidth, maximumPanelWidth)
                .toDouble(),
          );
        }

        void setPanelResizing(bool resizing) {
          if (_aiAssistantResizing == resizing) {
            return;
          }
          if (resizing) {
            _aiAssistantResizing = true;
          } else {
            // Paint the final pointer update without interpolation before
            // restoring the open/close transition for subsequent changes.
            _setWorkspaceState(() {});
            WidgetsBinding.instance.addPostFrameCallback((_) {
              if (mounted && _aiAssistantResizing) {
                _setWorkspaceState(() => _aiAssistantResizing = false);
              }
            });
          }
        }

        Widget assistantPanel({required bool terminalTools}) {
          final terminalController = terminalTools
              ? _aiTerminalController(selectedTerminalTab)
              : null;
          final sshProfile = terminalController?.sshProfile;
          final panel = _AiAssistantResizablePanel(
            colors: aiAssistantColors,
            onResize: resizePanel,
            onResizeStart: () => setPanelResizing(true),
            onResizeEnd: () => setPanelResizing(false),
            persistentBorder: _workspaceOverviewActive,
            child: _AiAssistantPanel(
              colors: aiAssistantColors,
              conversation: aiConversation,
              workspaceScope: _workspaceOverviewActive,
              terminalTools: terminalTools,
              terminalToolMode:
                  selectedTerminalTab?.toolPanelMode ??
                  _TerminalToolPanelMode.ai,
              onTerminalToolModeChanged:
                  terminalTools && selectedTerminalTab != null
                  ? (mode) => _selectTerminalTool(selectedTerminalTab, mode)
                  : null,
              terminalControllerResolver: () =>
                  _aiTerminalController(_selectedTerminalTab),
              snippets: terminalTools ? _snippets : const [],
              snippetPackages: terminalTools ? _snippetPackages : const [],
              shellHistory: terminalTools && terminalController != null
                  ? _shellHistoryForDisplay(terminalController)
                  : const [],
              sftpPanel: terminalTools
                  ? _TerminalSftpPanel(
                      sessionId: 'terminal:${selectedTerminalTab?.id}',
                      colors: aiAssistantColors,
                      groups: _groups,
                      hosts: _hosts,
                      tags: _tagEntries,
                      dataStore: _dataStore,
                      profile: sshProfile,
                      profileHost: sshProfile == null
                          ? null
                          : _sftpHostItemForProfile(sshProfile),
                      terminalController: terminalController,
                      createHostRequest: _sftpConnectRequestForHostItem,
                      onRemoteConnected: _handleSftpRemoteConnected,
                    )
                  : null,
              systemTarget: sshProfile?.host,
              loadSystemInfo: terminalTools && sshProfile != null
                  ? () async =>
                        (await _loadTerminalSystemInfo(sshProfile))
                            .withLatency(terminalController?.sshLatencyMs)
                  : null,
              onRunSnippet: terminalTools && selectedTerminalTab != null
                  ? (snippet) =>
                        _runTextInTerminal(selectedTerminalTab, snippet.script)
                  : null,
              onCopySnippet: terminalTools && selectedTerminalTab != null
                  ? (snippet) => _copyTextToTerminalPrompt(
                      selectedTerminalTab,
                      snippet.script,
                    )
                  : null,
              snippetTargetHostId: sshProfile?.hostId,
              snippetTargetLabel: sshProfile == null
                  ? null
                  : _emptyToNull(sshProfile.label) ??
                        '${sshProfile.username}@${sshProfile.host}',
              onSaveSnippet: terminalTools && _dataStore != null
                  ? _saveTerminalSnippet
                  : null,
              onCreateSnippetPackage: terminalTools && _dataStore != null
                  ? _createSnippetPackageForEditor
                  : null,
              onCreateSnippetUnavailable: terminalTools
                  ? () => _showWorkspaceMessage('Database is not ready.')
                  : null,
              onRunHistory: terminalTools && selectedTerminalTab != null
                  ? (entry) =>
                        _runTextInTerminal(selectedTerminalTab, entry.command)
                  : null,
              onCopyHistory: terminalTools && selectedTerminalTab != null
                  ? (entry) => _copyTextToTerminalPrompt(
                      selectedTerminalTab,
                      entry.command,
                    )
                  : null,
              loadTerminalThemes: terminalTools
                  ? () =>
                        _terminalThemeCatalog?.loadThemes() ??
                        Future.value(builtInTerminalThemes)
                  : null,
              currentTerminalTheme: terminalTools && selectedTerminalTab != null
                  ? () => _activeTerminalThemeFor(selectedTerminalTab)
                  : null,
              currentTerminalFont: terminalTools && selectedTerminalTab != null
                  ? () => selectedTerminalTab.font
                  : null,
              onTerminalThemeSelected:
                  terminalTools && selectedTerminalTab != null
                  ? (entry) => unawaited(
                      _setCurrentTerminalTheme(selectedTerminalTab, entry),
                    )
                  : null,
              onTerminalFontChanged:
                  terminalTools && selectedTerminalTab != null
                  ? (font) => unawaited(
                      _setCurrentTerminalFont(selectedTerminalTab, font),
                    )
                  : null,
              attachmentPicker: () => _pickAiAttachments(
                AiAttachment.maximumCount -
                    aiConversation.pendingAttachments.length,
              ),
              onClear: () {
                if (_saveAiConversationNow(aiConversation)) {
                  aiConversation.clear();
                } else {
                  _showWorkspaceMessage(
                    'Unable to save the current AI conversation.',
                  );
                }
              },
              loadHistory: () => _loadAiConversationHistory(aiConversation),
              openHistory: (entry) =>
                  _openAiConversationHistory(aiConversation, entry),
              deleteHistory: (entry) =>
                  _deleteAiConversationHistory(aiConversation, entry),
            ),
          );
          return panel;
        }

        final workspaceBody = KeyedSubtree(
          key: const ValueKey('nauterm-workspace-body'),
          child: _buildWorkspaceBody(selectedTerminalTab),
        );
        final separatePanel = terminalPageActive || !narrow;
        final bodyRightInset = aiAssistantOpen && separatePanel
            ? panelWidth
            : 0.0;
        return ColoredBox(
          color: terminalPageActive
              ? aiAssistantColors.canvasBackground
              : context.nautermPalette.background,
          child: Stack(
            fit: StackFit.expand,
            children: [
              AnimatedPositioned(
                duration: _aiAssistantResizing
                    ? Duration.zero
                    : _workspacePanelTransitionDuration,
                curve: _workspacePanelTransitionCurve,
                top: 0,
                left: 0,
                right: bodyRightInset,
                bottom: 0,
                child: workspaceBody,
              ),
              AnimatedPositioned(
                duration: _aiAssistantResizing
                    ? Duration.zero
                    : _workspacePanelTransitionDuration,
                curve: _workspacePanelTransitionCurve,
                top: 0,
                right: aiAssistantOpen ? 0 : -panelWidth,
                bottom: 0,
                width: panelWidth,
                child: ClipRect(
                  child: AnimatedSwitcher(
                    duration: _workspacePanelTransitionDuration,
                    reverseDuration: _workspacePanelTransitionDuration,
                    switchInCurve: Curves.linear,
                    switchOutCurve: Curves.linear,
                    layoutBuilder: (currentChild, previousChildren) => Stack(
                      fit: StackFit.expand,
                      children: [...previousChildren, ?currentChild],
                    ),
                    transitionBuilder: (child, animation) => child,
                    child: aiAssistantOpen
                        ? KeyedSubtree(
                            key: terminalPageActive
                                ? const ValueKey('terminal-tools-region')
                                : const ValueKey('ai-assistant-region'),
                            child: terminalPageActive
                                ? Padding(
                                    padding: const EdgeInsets.fromLTRB(
                                      4,
                                      8,
                                      8,
                                      8,
                                    ),
                                    child: assistantPanel(terminalTools: true),
                                  )
                                : narrow
                                ? assistantPanel(terminalTools: false)
                                : ColoredBox(
                                    color: aiAssistantColors.background,
                                    child: assistantPanel(terminalTools: false),
                                  ),
                          )
                        : const SizedBox.expand(
                            key: ValueKey('assistant-panel-closed'),
                          ),
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  double _aiAssistantWidth(_TerminalTab? tab) {
    return _workspaceOverviewActive
        ? _selectedWorkspace.aiAssistantWidth
        : tab?.aiAssistantWidth ?? 360;
  }

  Future<void> _setCurrentTerminalTheme(
    _TerminalTab tab,
    StoredTerminalTheme entry,
  ) async {
    final theme = entry.theme;
    _setWorkspaceState(() {
      for (final view in tab.rootLayout.views) {
        for (final terminalTab in view.tabs) {
          terminalTab.theme = theme;
        }
      }
      terminalThemeId =
          entry.id == 'default' || entry.id == nysaLightTerminalThemeId
          ? null
          : entry.id;
      terminalCustomTheme = theme;
    });
    try {
      await NautermConfigStore(NautermPaths.resolve()).saveTerminalTheme(
        themeId: terminalThemeId,
        customThemeJson: terminalCustomTheme.toJson(),
      );
    } on Object {
      _showWorkspaceMessage(
        'Theme applied, but the configuration file could not be updated.',
        type: _WorkspaceNotificationType.error,
      );
    }
  }

  Future<void> _setCurrentTerminalFont(
    _TerminalTab tab,
    TerminalFontConfig font,
  ) async {
    _setWorkspaceState(() {
      tab.font = font;
      terminalFontConfig = font;
    });
    terminalConfigNotifier.value++;
    try {
      await NautermConfigStore(NautermPaths.resolve())
          .saveRuntimeSettings(currentNautermRuntimeSettings());
    } on Object {
      _showWorkspaceMessage(
        'Font applied, but the configuration file could not be updated.',
        type: _WorkspaceNotificationType.error,
      );
    }
  }

  void _setAiAssistantWidth(_TerminalTab? tab, double width) {
    _setWorkspaceState(() {
      if (_workspaceOverviewActive) {
        _selectedWorkspace.aiAssistantWidth = width;
      } else if (tab != null) {
        tab.aiAssistantWidth = width;
      }
    });
  }

  Future<_AiAttachmentPickerResult> _pickAiAttachments(
    int availableSlots,
  ) async {
    if (availableSlots <= 0) {
      return const _AiAttachmentPickerResult(
        errors: ['Attach up to 5 files per message.'],
      );
    }
    final files = await _runExclusiveFilePicker(openFiles);
    if (files == null || files.isEmpty) {
      return const _AiAttachmentPickerResult();
    }
    final attachments = <AiAttachment>[];
    final errors = <String>[];
    if (files.length > availableSlots) {
      errors.add('Only the first $availableSlots selected files were added.');
    }
    for (final file in files.take(availableSlots)) {
      try {
        attachments.add(await AiAttachment.fromFile(file));
      } on AiAttachmentException catch (error) {
        errors.add(error.message);
      } on Object {
        errors.add('Unable to read ${file.name}.');
      }
    }
    return _AiAttachmentPickerResult(attachments: attachments, errors: errors);
  }

  _AiAssistantColors _aiAssistantColors(_TerminalTab? tab) {
    if (_workspaceOverviewActive) {
      return _AiAssistantColors(
        canvasBackground: _surface,
        background: _card,
        foreground: _text,
        accent: _blue,
        border: _sidebarDivider,
        muted: _mutedText,
        inputBackground: _surface,
      );
    }
    if (tab == null) {
      return _AiAssistantColors.fromTerminalTheme(defaultTerminalTheme);
    }
    return _AiAssistantColors.fromTerminalTheme(_activeTerminalThemeFor(tab));
  }

  TerminalTheme _activeTerminalThemeFor(_TerminalTab tab) {
    if (_selectedTerminalId == tab.id) {
      final selectedViewId = _selectedTerminalViewId;
      if (selectedViewId != null) {
        final selectedView = tab.rootLayout.viewFor(selectedViewId);
        if (selectedView != null) {
          return selectedView.theme;
        }
      }
    }
    return tab.primaryView.theme;
  }

  bool _aiAssistantAvailable(_TerminalTab? tab) {
    if (_tab != _WorkspaceTab.sessions) {
      return false;
    }
    if (_workspaceOverviewActive) {
      return true;
    }
    return tab?.pageMode == _TerminalTabPageMode.ssh;
  }

  bool _aiAssistantOpen(_TerminalTab? tab) {
    if (!_aiAssistantAvailable(tab)) {
      return false;
    }
    return _workspaceOverviewActive
        ? _selectedWorkspace.aiAssistantOpen
        : tab!.aiAssistantOpen;
  }

  AiConversationController _aiConversation(_TerminalTab? tab) {
    return _workspaceOverviewActive
        ? _selectedWorkspace.aiConversation
        : tab?.aiConversation ?? _selectedWorkspace.aiConversation;
  }

  void _watchAiConversationPersistence(
    AiConversationController conversation,
    _TerminalTab? tab,
  ) {
    final terminalScope = !_workspaceOverviewActive && tab != null;
    final sshProfile = terminalScope ? tab.controller.sshProfile : null;
    String? hostUuid;
    if (terminalScope) {
      final hostId = sshProfile?.hostId;
      hostUuid = _hostEntries
          .where((host) => host.id == hostId)
          .firstOrNull
          ?.uuid;
    }
    _aiConversationRepository.watch(
      conversation,
      AiConversationPersistenceTarget(
        scope: terminalScope ? 'terminal' : 'workspace',
        hostUuid: hostUuid,
      ),
    );
  }

  bool _saveAiConversationNow(AiConversationController conversation) {
    return _aiConversationRepository.saveNow(conversation);
  }

  Future<List<AiConversationEntry>> _loadAiConversationHistory(
    AiConversationController conversation,
  ) async {
    return _aiConversationRepository.loadHistory(conversation);
  }

  Future<bool> _openAiConversationHistory(
    AiConversationController conversation,
    AiConversationEntry entry,
  ) async {
    return _aiConversationRepository.openHistory(conversation, entry);
  }

  Future<bool?> _deleteAiConversationHistory(
    AiConversationController conversation,
    AiConversationEntry entry,
  ) async {
    final uuid = entry.uuid;
    if (uuid == null) {
      return false;
    }
    final confirmed = await _confirmDelete(
      title: 'Delete Conversation?',
      message: 'Delete "${entry.title}" from conversation history?',
      confirmLabel: 'Delete',
      colors: _workspaceOverviewActive
          ? null
          : _aiAssistantColors(_selectedTerminalTab),
    );
    if (!confirmed || !mounted) {
      return null;
    }
    return _aiConversationRepository.deleteHistory(conversation, entry);
  }

  TerminalController? _aiTerminalController(_TerminalTab? tab) {
    if (tab == null || tab.replay) {
      return null;
    }
    final selectedViewId = _selectedTerminalViewId;
    final controller = (selectedViewId == null
        ? tab.controller
        : tab.rootLayout.viewFor(selectedViewId)?.controller ?? tab.controller);
    return controller;
  }

  void _toggleAiAssistant(_TerminalTab? tab) {
    if (!_aiAssistantAvailable(tab)) {
      return;
    }
    _setWorkspaceState(() {
      if (_workspaceOverviewActive) {
        _selectedWorkspace.aiAssistantOpen =
            !_selectedWorkspace.aiAssistantOpen;
      } else {
        final terminalTab = tab!;
        if (terminalTab.aiAssistantOpen &&
            terminalTab.toolPanelMode == _TerminalToolPanelMode.ai) {
          terminalTab.aiAssistantOpen = false;
        } else {
          terminalTab.toolPanelMode = _TerminalToolPanelMode.ai;
          terminalTab.aiAssistantOpen = true;
        }
      }
    });
  }

  void _toggleTerminalTools(_TerminalTab? tab) {
    if (!_aiAssistantAvailable(tab) ||
        _workspaceOverviewActive ||
        tab == null) {
      return;
    }
    final opening = !tab.aiAssistantOpen;
    _setWorkspaceState(() => tab.aiAssistantOpen = opening);
    if (opening && tab.toolPanelMode == _TerminalToolPanelMode.shellHistory) {
      final controller = _aiTerminalController(tab);
      if (controller != null) {
        _refreshShellHistoryIfNeeded(controller, force: true);
      }
    }
  }

  void _selectTerminalTool(_TerminalTab tab, _TerminalToolPanelMode mode) {
    if (tab.toolPanelMode == mode) {
      return;
    }
    _setWorkspaceState(() => tab.toolPanelMode = mode);
    if (mode == _TerminalToolPanelMode.shellHistory) {
      final controller = _aiTerminalController(tab);
      if (controller != null) {
        _refreshShellHistoryIfNeeded(controller, force: true);
      }
    }
  }

  void _runTextInTerminal(_TerminalTab tab, String text) {
    final controller = _aiTerminalController(tab);
    final command = text.trim();
    if (controller == null || command.isEmpty) {
      return;
    }
    final normalized = command.replaceAll('\r\n', '\n').replaceAll('\n', '\r');
    controller.sendInput(
      normalized.endsWith('\r') ? normalized : '$normalized\r',
    );
  }

  void _copyTextToTerminalPrompt(_TerminalTab tab, String text) {
    final controller = _aiTerminalController(tab);
    final command = text.trim();
    if (controller == null || command.isEmpty) {
      return;
    }
    controller.sendInput(
      terminalPromptInsertionSequence(
        command,
        controller.snapshot.keyboardMode,
      ),
    );
  }

  Future<FfiHostSystemInfoResult> _loadTerminalSystemInfo(
    SshConnectionProfile profile,
  ) {
    return _spawnHostSystemInfo({
      'host': profile.host,
      'port': profile.port,
      'username': profile.username,
      'knownHostsPath': profile.knownHostsPath,
      'password': profile.password,
      'privateKey': profile.privateKey,
      'certificate': profile.certificate,
      'passphrase': profile.passphrase,
      'proxy': profile.proxy?.toJson(),
      'hostKeyTrustMode': SshHostKeyTrustMode.strict.wireValue,
    });
  }
}
