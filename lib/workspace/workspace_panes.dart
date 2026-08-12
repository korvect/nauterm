part of 'nauterm_workspace.dart';

@visibleForTesting
bool shouldBeginConnectionCompletionHold({
  required TerminalConnectionPhase phase,
  required TerminalConnectionPhase? previousPhase,
  required bool connectionPageWasShown,
}) {
  return connectionPageWasShown &&
      phase == TerminalConnectionPhase.connected &&
      previousPhase != TerminalConnectionPhase.connected;
}

class _TerminalSessionView extends StatefulWidget {
  const _TerminalSessionView({
    super.key,
    required this.controller,
    required this.config,
    required this.theme,
    required this.connectionKeys,
    required this.connectionIdentities,
    required this.composerHistory,
    required this.composerSuggestions,
    required this.autofocusTerminal,
    this.readOnly = false,
    this.composerSuggestionResolver,
    this.composerVisible,
    this.onComposerVisibilityChanged,
    this.tabToolbar,
    this.onConnectionAuthSaved,
    this.onAddKeyRequested,
    this.onEditHostRequested,
    this.onReloadConnection,
    this.onConnectionPageVisibilityChanged,
    this.onSplitRequested,
    this.onNewTabRequested,
    this.onSettingsRequested,
    this.onCloseRequested,
    this.dataStore,
  });

  final TerminalController controller;
  final TerminalConfig config;
  final TerminalTheme theme;
  final NautermDataStore? dataStore;
  final List<TerminalConnectionKeyOption> connectionKeys;
  final List<TerminalConnectionIdentityOption> connectionIdentities;
  final List<String> composerHistory;
  final List<String> composerSuggestions;
  final bool autofocusTerminal;
  final bool readOnly;
  final TerminalComposerSuggestionResolver? composerSuggestionResolver;
  final bool? composerVisible;
  final ValueChanged<bool>? onComposerVisibilityChanged;
  final TerminalViewTabToolbar? tabToolbar;
  final TerminalConnectionAuthSaver? onConnectionAuthSaved;
  final VoidCallback? onAddKeyRequested;
  final VoidCallback? onEditHostRequested;
  final _ReloadedTerminalConnection? Function()? onReloadConnection;
  final ValueChanged<bool>? onConnectionPageVisibilityChanged;
  final ValueChanged<TerminalSplitDirection>? onSplitRequested;
  final VoidCallback? onNewTabRequested;
  final VoidCallback? onSettingsRequested;
  final VoidCallback? onCloseRequested;

  @override
  State<_TerminalSessionView> createState() => _TerminalSessionViewState();
}

class _TerminalSessionViewState extends State<_TerminalSessionView> {
  static const _minimumConnectionPageDuration = Duration(milliseconds: 333);
  DateTime? _connectionPageShownAt;
  Timer? _connectionPageTimer;
  bool _holdingConnectedPage = false;
  TerminalConnectionPhase? _lastConnectionPhase;
  bool? _reportedConnectionPageVisibility;

  void _reportConnectionPageVisibility(bool visible) {
    if (_reportedConnectionPageVisibility == visible) return;
    _reportedConnectionPageVisibility = visible;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) widget.onConnectionPageVisibilityChanged?.call(visible);
    });
  }

  @override
  void didUpdateWidget(covariant _TerminalSessionView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (identical(oldWidget.controller, widget.controller)) return;
    _connectionPageTimer?.cancel();
    _connectionPageTimer = null;
    _connectionPageShownAt = null;
    _holdingConnectedPage = false;
    _lastConnectionPhase = null;
    _reportedConnectionPageVisibility = null;
  }

  @override
  void dispose() {
    _connectionPageTimer?.cancel();
    super.dispose();
  }

  bool _showConnectionPage(
    TerminalConnectionStatus status,
    SshConnectionProfile? profile,
    SerialConnectionProfile? serialProfile,
    TelnetConnectionProfile? telnetProfile,
  ) {
    if (widget.controller.isLocalTerminal) {
      _connectionPageShownAt = null;
      _holdingConnectedPage = false;
      _connectionPageTimer?.cancel();
      _connectionPageTimer = null;
      return false;
    }
    final shouldShow = _shouldShowConnectionPage(
      widget.controller,
      status,
      profile,
      serialProfile,
      telnetProfile,
    );
    if (shouldShow) {
      if (status.phase == TerminalConnectionPhase.connecting &&
          (_lastConnectionPhase == TerminalConnectionPhase.hostKey ||
              _lastConnectionPhase == TerminalConnectionPhase.authentication)) {
        _connectionPageShownAt = DateTime.now();
      } else {
        _connectionPageShownAt ??= DateTime.now();
      }
      _lastConnectionPhase = status.phase;
      _holdingConnectedPage = false;
      _connectionPageTimer?.cancel();
      return true;
    }
    if (shouldBeginConnectionCompletionHold(
      phase: status.phase,
      previousPhase: _lastConnectionPhase,
      connectionPageWasShown: _connectionPageShownAt != null,
    )) {
      _connectionPageShownAt = DateTime.now();
    }
    if (status.phase != TerminalConnectionPhase.connected ||
        _connectionPageShownAt == null) {
      _lastConnectionPhase = status.phase;
      return false;
    }
    _lastConnectionPhase = status.phase;
    final elapsed = DateTime.now().difference(_connectionPageShownAt!);
    final remaining = _minimumConnectionPageDuration - elapsed;
    if (remaining <= Duration.zero) {
      return false;
    }
    if (!_holdingConnectedPage) {
      _holdingConnectedPage = true;
      _connectionPageTimer?.cancel();
      _connectionPageTimer = Timer(remaining, () {
        if (!mounted) return;
        setState(() => _holdingConnectedPage = false);
      });
    }
    return _holdingConnectedPage;
  }

  @override
  Widget build(BuildContext context) {
    return ColoredBox(
      color: widget.theme.primary.background,
      child: AnimatedBuilder(
        animation: widget.controller,
        builder: (context, _) {
          final status = widget.controller.connectionStatus;
          final profile = widget.controller.sshProfile;
          final serialProfile = widget.controller.serialProfile;
          final telnetProfile = widget.controller.telnetProfile;
          final showConnectionPage = _showConnectionPage(
            status,
            profile,
            serialProfile,
            telnetProfile,
          );
          _reportConnectionPageVisibility(showConnectionPage);
          if (showConnectionPage) {
            return _TerminalConnectionPage(
              controller: widget.controller,
              keys: widget.connectionKeys,
              identities: widget.connectionIdentities,
              onSaveAuth: widget.onConnectionAuthSaved,
              onAddKeyRequested: widget.onAddKeyRequested,
              onEditHostRequested: widget.onEditHostRequested,
              onReloadConnection: widget.onReloadConnection,
              onCloseRequested: widget.onCloseRequested,
              dataStore: widget.dataStore,
            );
          }

          return Stack(
            fit: StackFit.expand,
            children: [
              TerminalView(
                controller: widget.controller,
                config: widget.config,
                theme: widget.theme,
                autofocusTerminal: widget.autofocusTerminal,
                readOnly: widget.readOnly,
                composerHistory: widget.composerHistory,
                composerSuggestions: widget.composerSuggestions,
                composerSuggestionResolver: widget.composerSuggestionResolver,
                composerVisible: widget.composerVisible,
                onComposerVisibilityChanged: widget.onComposerVisibilityChanged,
                tabToolbar: widget.tabToolbar,
                onSplitRequested: widget.onSplitRequested,
                onNewTabRequested: widget.onNewTabRequested,
                onSettingsRequested: widget.onSettingsRequested,
                onCloseRequested: widget.onCloseRequested,
              ),
              if (widget.controller.showsReconnectStatus)
                _TerminalReconnectOverlay(
                  key: ObjectKey(widget.controller),
                  controller: widget.controller,
                  theme: widget.theme,
                  onCloseRequested: widget.onCloseRequested,
                ),
            ],
          );
        },
      ),
    );
  }
}

class _TerminalReplayLoadingOverlay extends StatelessWidget {
  const _TerminalReplayLoadingOverlay({required this.theme});

  final TerminalTheme theme;

  @override
  Widget build(BuildContext context) {
    final foreground = theme.primary.foreground;
    return ColoredBox(
      key: const ValueKey('terminal-replay-loading'),
      color: theme.primary.background,
      child: Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            SizedBox.square(
              dimension: 20,
              child: CircularProgressIndicator(
                strokeWidth: 2,
                color: foreground.withValues(alpha: 0.72),
              ),
            ),
            const SizedBox(height: 12),
            Text(
              tr(
                'workspace.label.processingTerminalReplay',
                fallback: 'Processing terminal replay…',
              ),
              style: TextStyle(
                color: foreground.withValues(alpha: 0.72),
                fontSize: 13,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _KnownHostsPane extends StatefulWidget {
  const _KnownHostsPane({
    required this.text,
    required this.onImport,
    required this.onContextAction,
    required this.onContextActions,
  });

  final String text;
  final VoidCallback onImport;
  final _WorkspaceContextAction<_KnownHostItem> onContextAction;
  final _WorkspaceContextActions<_KnownHostItem> onContextActions;

  @override
  State<_KnownHostsPane> createState() => _KnownHostsPaneState();
}

class _KnownHostsPaneState extends State<_KnownHostsPane> {
  _WorkspaceSortOrder _sortOrder = _WorkspaceSortOrder.nameAscending;
  _WorkspaceViewMode _viewMode = _WorkspaceViewMode.grid;
  String _searchQuery = '';

  @override
  Widget build(BuildContext context) {
    final knownHosts = _sortWorkspaceItems(
      _filterWorkspaceItems(
        _parseKnownHosts(widget.text),
        _searchQuery,
        extraText: (host) => [
          host.line,
          host.hostPattern,
          host.host ?? '',
          if (host.port != null) '${host.port}',
        ],
      ),
      _sortOrder,
      ordinal: (host) => host.lineIndex,
    );

    return ColoredBox(
      color: _surface,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _KnownHostsToolbar(
            onImport: widget.onImport,
            sortOrder: _sortOrder,
            onSortOrderChanged: (value) => setState(() => _sortOrder = value),
            searchQuery: _searchQuery,
            onSearchQueryChanged: (value) =>
                setState(() => _searchQuery = value),
            viewMode: _viewMode,
            onViewModeChanged: (value) => setState(() => _viewMode = value),
          ),
          Expanded(
            child: SingleChildScrollView(
              padding: _workspacePanePadding,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const _SectionTitle('Known Hosts'),
                  SizedBox(height: 12),
                  if (knownHosts.isEmpty)
                    _WorkspaceInlineMessage(
                      _searchQuery.trim().isEmpty
                          ? 'No known hosts yet.'
                          : 'No known hosts found.',
                    )
                  else
                    _WorkspaceItemCollection(
                      items: knownHosts,
                      viewMode: _viewMode,
                      onContextAction: widget.onContextAction,
                      onContextActions: widget.onContextActions,
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _KnownHostsToolbar extends StatelessWidget {
  const _KnownHostsToolbar({
    required this.onImport,
    required this.sortOrder,
    required this.onSortOrderChanged,
    required this.searchQuery,
    required this.onSearchQueryChanged,
    required this.viewMode,
    required this.onViewModeChanged,
  });

  final VoidCallback onImport;
  final _WorkspaceSortOrder sortOrder;
  final ValueChanged<_WorkspaceSortOrder> onSortOrderChanged;
  final String searchQuery;
  final ValueChanged<String> onSearchQueryChanged;
  final _WorkspaceViewMode viewMode;
  final ValueChanged<_WorkspaceViewMode> onViewModeChanged;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceToolbarFrame(
      children: [
        _WorkspaceButton(
          label: 'Import',
          size: _WorkspaceControlSize.tiny,
          variant: _WorkspaceButtonVariant.filled,
          height: _workspaceToolbarControlExtent,
          horizontalPadding: 12,
          onPressed: onImport,
        ),
        Expanded(
          child: _ToolbarTrailingActions(
            children: _defaultToolbarTrailingActions(
              sortOrder: sortOrder,
              onSortOrderChanged: onSortOrderChanged,
              searchQuery: searchQuery,
              onSearchQueryChanged: onSearchQueryChanged,
              viewMode: viewMode,
              onViewModeChanged: onViewModeChanged,
            ),
          ),
        ),
      ],
    );
  }
}

class _SnippetsPane extends StatefulWidget {
  const _SnippetsPane({
    required this.packages,
    required this.snippets,
    required this.onCreateSnippet,
    required this.onCreatePackage,
    required this.onShowShellHistory,
    required this.onPackageContextAction,
    required this.onPackageContextActions,
    required this.onSnippetContextAction,
    required this.onSnippetContextActions,
    required this.onSnippetRun,
  });

  final List<_SnippetPackageItem> packages;
  final List<_SnippetItem> snippets;
  final ValueChanged<int?> onCreateSnippet;
  final VoidCallback onCreatePackage;
  final VoidCallback onShowShellHistory;
  final _WorkspaceContextAction<_SnippetPackageItem> onPackageContextAction;
  final _WorkspaceContextActions<_SnippetPackageItem> onPackageContextActions;
  final _WorkspaceContextAction<_SnippetItem> onSnippetContextAction;
  final _WorkspaceContextActions<_SnippetItem> onSnippetContextActions;
  final ValueChanged<_SnippetItem> onSnippetRun;

  @override
  State<_SnippetsPane> createState() => _SnippetsPaneState();
}

class _SnippetsPaneState extends State<_SnippetsPane> {
  _WorkspaceSortOrder _sortOrder = _WorkspaceSortOrder.nameAscending;
  _WorkspaceViewMode _viewMode = _WorkspaceViewMode.grid;
  String _searchQuery = '';
  int? _currentPackageId;

  @override
  Widget build(BuildContext context) {
    final selectedPackage = _currentPackageId == null
        ? null
        : widget.packages
              .where((package) => package.id == _currentPackageId)
              .firstOrNull;
    final currentPackageId = selectedPackage?.id;

    final packages = _sortWorkspaceItems(
      _filterWorkspaceItems(widget.packages, _searchQuery),
      _sortOrder,
      ordinal: (package) => package.id,
    );
    final scopedSnippets = currentPackageId == null
        ? widget.snippets
        : widget.snippets.where(
            (snippet) => snippet.packageId == currentPackageId,
          );
    final snippets = _sortWorkspaceItems(
      _filterWorkspaceItems(
        scopedSnippets,
        _searchQuery,
        extraText: (snippet) => [
          snippet.script,
          if (snippet.scope == SnippetScope.global) 'global',
          if (snippet.scope == SnippetScope.targeted) 'targeted',
          ...snippet.targetGroupIds.map((id) => 'group:$id'),
          ...snippet.targetHostIds.map((id) => 'host:$id'),
        ],
      ),
      _sortOrder,
      ordinal: (snippet) => snippet.id,
    );
    final showingPackage = selectedPackage != null;
    final sharedGridItemCount = showingPackage
        ? snippets.length
        : math.max(packages.length, snippets.length);
    final hasStoredItems =
        widget.packages.isNotEmpty || widget.snippets.isNotEmpty;
    final showEmptyState = !hasStoredItems && _searchQuery.trim().isEmpty;

    return ColoredBox(
      color: _surface,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _SnippetsToolbar(
            onCreateSnippet: () => widget.onCreateSnippet(currentPackageId),
            onCreatePackage: widget.onCreatePackage,
            onShowShellHistory: widget.onShowShellHistory,
            sortOrder: _sortOrder,
            onSortOrderChanged: (value) => setState(() => _sortOrder = value),
            searchQuery: _searchQuery,
            onSearchQueryChanged: (value) =>
                setState(() => _searchQuery = value),
            viewMode: _viewMode,
            onViewModeChanged: (value) => setState(() => _viewMode = value),
          ),
          Expanded(
            child: showEmptyState
                ? _WorkspaceEmptyState(
                    icon: LucideIcons.code,
                    title: 'No snippets yet',
                    description:
                        'Save frequently used commands as snippets so they are ready in every terminal.',
                    actionLabel: 'Add snippet',
                    onAction: () => widget.onCreateSnippet(currentPackageId),
                  )
                : SingleChildScrollView(
                    padding: _workspacePanePadding,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        if (showingPackage) ...[
                          _SnippetPackageBreadcrumb(
                            package: selectedPackage,
                            onRootSelected: _openRoot,
                          ),
                          SizedBox(height: 18),
                        ],
                        if (!showingPackage && packages.isNotEmpty) ...[
                          const _SectionTitle('Packages'),
                          SizedBox(height: 14),
                          _WorkspaceItemCollection(
                            items: packages,
                            viewMode: _viewMode,
                            maxColumns: 2,
                            layoutItemCount: sharedGridItemCount,
                            onItemTap: _openPackage,
                            onContextAction: _handlePackageContextAction,
                            onContextActions: widget.onPackageContextActions,
                          ),
                        ],
                        if (!showingPackage &&
                            packages.isNotEmpty &&
                            snippets.isNotEmpty)
                          SizedBox(height: 30),
                        if (snippets.isNotEmpty) ...[
                          const _SectionTitle('Snippets'),
                          SizedBox(height: 14),
                          _WorkspaceItemCollection(
                            items: snippets,
                            viewMode: _viewMode,
                            maxColumns: 2,
                            layoutItemCount: sharedGridItemCount,
                            onItemDoubleTap: widget.onSnippetRun,
                            onContextAction: widget.onSnippetContextAction,
                            onContextActions: widget.onSnippetContextActions,
                          ),
                        ],
                        if (packages.isEmpty && snippets.isEmpty)
                          const _WorkspaceInlineMessage('No snippets found.'),
                      ],
                    ),
                  ),
          ),
        ],
      ),
    );
  }

  void _openPackage(_SnippetPackageItem package) {
    setState(() => _currentPackageId = package.id);
  }

  void _handlePackageContextAction(
    _SnippetPackageItem package,
    _ContextMenuActionId action,
  ) {
    if (action == _ContextMenuActionId.open) {
      _openPackage(package);
      return;
    }
    widget.onPackageContextAction(package, action);
  }

  void _openRoot() {
    setState(() => _currentPackageId = null);
  }
}

class _SnippetPackageBreadcrumb extends StatelessWidget {
  const _SnippetPackageBreadcrumb({
    required this.package,
    required this.onRootSelected,
  });

  final _SnippetPackageItem package;
  final VoidCallback onRootSelected;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 28,
      child: Row(
        children: [
          _GroupBreadcrumbButton(
            label: 'Snippets',
            current: false,
            onPressed: onRootSelected,
          ),
          Padding(
            padding: EdgeInsets.symmetric(horizontal: 3),
            child: Icon(
              Icons.chevron_right_rounded,
              size: 17,
              color: Color(0xff8da1a7),
            ),
          ),
          _GroupBreadcrumbButton(
            label: package.name,
            current: true,
            onPressed: null,
          ),
        ],
      ),
    );
  }
}

class _PortForwardingPane extends StatefulWidget {
  const _PortForwardingPane({
    required this.forwards,
    required this.onCreateForward,
    required this.onEditForward,
    required this.onToggleForward,
  });

  final List<_PortForwardItem> forwards;
  final ValueChanged<String> onCreateForward;
  final ValueChanged<_PortForwardItem> onEditForward;
  final void Function(_PortForwardItem item, bool enabled) onToggleForward;

  @override
  State<_PortForwardingPane> createState() => _PortForwardingPaneState();
}

class _PortForwardingPaneState extends State<_PortForwardingPane> {
  _WorkspaceSortOrder _sortOrder = _WorkspaceSortOrder.nameAscending;
  String _searchQuery = '';

  @override
  Widget build(BuildContext context) {
    final forwards = _sortWorkspaceItems(
      _filterWorkspaceItems(
        widget.forwards,
        _searchQuery,
        extraText: (forward) => [
          forward.type,
          forward.bindAddress,
          '${forward.bindPort}',
          forward.destinationHost,
          '${forward.destinationPort}',
          forward.intermediateHostName,
          forward.enabled ? 'enabled active' : 'disabled',
          forward.statusError ?? '',
        ],
      ),
      _sortOrder,
      ordinal: (forward) => forward.id,
    );
    final showEmptyState =
        widget.forwards.isEmpty && _searchQuery.trim().isEmpty;

    return ColoredBox(
      color: _surface,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _PortForwardingToolbar(
            onCreateForward: widget.onCreateForward,
            sortOrder: _sortOrder,
            onSortOrderChanged: (value) => setState(() => _sortOrder = value),
            searchQuery: _searchQuery,
            onSearchQueryChanged: (value) =>
                setState(() => _searchQuery = value),
          ),
          Expanded(
            child: showEmptyState
                ? _WorkspaceEmptyState(
                    icon: LucideIcons.arrowRightLeft,
                    title: 'No forwarding rules yet',
                    description:
                        'Add a forwarding rule to route local, remote, or dynamic traffic through SSH.',
                    actionLabel: 'Add forwarding',
                    onAction: () => widget.onCreateForward('local'),
                  )
                : SingleChildScrollView(
                    padding: _workspacePanePadding,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        if (forwards.isEmpty)
                          const _WorkspaceInlineMessage(
                            'No forwarding rules found.',
                          )
                        else ...[
                          const _SectionTitle('Port Forwarding'),
                          SizedBox(height: 14),
                          Column(
                            children: [
                              for (final forward in forwards) ...[
                                _PortForwardCard(
                                  forward: forward,
                                  onEdit: () => widget.onEditForward(forward),
                                  onToggle: () => widget.onToggleForward(
                                    forward,
                                    !forward.enabled,
                                  ),
                                ),
                                SizedBox(height: 12),
                              ],
                            ],
                          ),
                        ],
                      ],
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}

class _ProxiesPane extends StatefulWidget {
  const _ProxiesPane({
    required this.proxies,
    required this.onCreateProxy,
    required this.onEditProxy,
    required this.onContextAction,
    required this.onContextActions,
  });

  final List<_ProxyItem> proxies;
  final VoidCallback onCreateProxy;
  final ValueChanged<_ProxyItem> onEditProxy;
  final _WorkspaceContextAction<_ProxyItem> onContextAction;
  final _WorkspaceContextActions<_ProxyItem> onContextActions;

  @override
  State<_ProxiesPane> createState() => _ProxiesPaneState();
}

class _ProxiesPaneState extends State<_ProxiesPane> {
  _WorkspaceSortOrder _sortOrder = _WorkspaceSortOrder.nameAscending;
  _WorkspaceViewMode _viewMode = _WorkspaceViewMode.grid;
  String _searchQuery = '';

  @override
  Widget build(BuildContext context) {
    final proxies = _sortWorkspaceItems(
      _filterWorkspaceItems(
        widget.proxies,
        _searchQuery,
        extraText: (proxy) => [
          proxy.type,
          proxy.host,
          '${proxy.port}',
          proxy.identityName ?? '',
          proxy.username ?? '',
        ],
      ),
      _sortOrder,
      ordinal: (proxy) => proxy.id,
    );
    final showEmptyState =
        widget.proxies.isEmpty && _searchQuery.trim().isEmpty;

    return ColoredBox(
      color: _surface,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _ProxiesToolbar(
            onCreateProxy: widget.onCreateProxy,
            sortOrder: _sortOrder,
            onSortOrderChanged: (value) => setState(() => _sortOrder = value),
            searchQuery: _searchQuery,
            onSearchQueryChanged: (value) =>
                setState(() => _searchQuery = value),
            viewMode: _viewMode,
            onViewModeChanged: (value) => setState(() => _viewMode = value),
          ),
          Expanded(
            child: showEmptyState
                ? _WorkspaceEmptyState(
                    icon: LucideIcons.network,
                    title: 'No proxies yet',
                    description:
                        'Add a proxy once and reuse it across hosts that share the same route.',
                    actionLabel: 'Add proxy',
                    onAction: widget.onCreateProxy,
                  )
                : SingleChildScrollView(
                    padding: _workspacePanePadding,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const _SectionTitle('Proxies'),
                        SizedBox(height: 12),
                        if (proxies.isEmpty)
                          const _WorkspaceInlineMessage('No proxies found.')
                        else
                          _WorkspaceItemCollection<_ProxyItem>(
                            items: proxies,
                            viewMode: _viewMode,
                            onItemDoubleTap: widget.onEditProxy,
                            onContextAction: widget.onContextAction,
                            onContextActions: widget.onContextActions,
                          ),
                      ],
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}

class _ProxiesToolbar extends StatelessWidget {
  const _ProxiesToolbar({
    required this.onCreateProxy,
    required this.sortOrder,
    required this.onSortOrderChanged,
    required this.searchQuery,
    required this.onSearchQueryChanged,
    required this.viewMode,
    required this.onViewModeChanged,
  });

  final VoidCallback onCreateProxy;
  final _WorkspaceSortOrder sortOrder;
  final ValueChanged<_WorkspaceSortOrder> onSortOrderChanged;
  final String searchQuery;
  final ValueChanged<String> onSearchQueryChanged;
  final _WorkspaceViewMode viewMode;
  final ValueChanged<_WorkspaceViewMode> onViewModeChanged;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceToolbarFrame(
      children: [
        _ToolbarPrimaryButton(
          icon: Icons.add_rounded,
          label: tr('workspace.label.newProxy', fallback: 'New proxy'),
          onPressed: onCreateProxy,
        ),
        Expanded(
          child: _ToolbarTrailingActions(
            children: _defaultToolbarTrailingActions(
              sortOrder: sortOrder,
              onSortOrderChanged: onSortOrderChanged,
              searchQuery: searchQuery,
              onSearchQueryChanged: onSearchQueryChanged,
              viewMode: viewMode,
              onViewModeChanged: onViewModeChanged,
            ),
          ),
        ),
      ],
    );
  }
}

class _PortForwardingToolbar extends StatelessWidget {
  const _PortForwardingToolbar({
    required this.onCreateForward,
    required this.sortOrder,
    required this.onSortOrderChanged,
    required this.searchQuery,
    required this.onSearchQueryChanged,
  });

  final ValueChanged<String> onCreateForward;
  final _WorkspaceSortOrder sortOrder;
  final ValueChanged<_WorkspaceSortOrder> onSortOrderChanged;
  final String searchQuery;
  final ValueChanged<String> onSearchQueryChanged;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceToolbarFrame(
      children: [
        _WorkspaceDropdown<String>(
          width: 170,
          entries: [
            NautermContextMenuAction<String>(
              value: 'local',
              icon: Icons.call_split_rounded,
              label: tr('common.label.local', fallback: 'Local'),
            ),
            NautermContextMenuAction<String>(
              value: 'remote',
              icon: Icons.swap_horiz_rounded,
              label: tr('common.label.remote', fallback: 'Remote'),
            ),
            NautermContextMenuAction<String>(
              value: 'dynamic',
              icon: Icons.hub_rounded,
              label: tr('common.label.dynamic', fallback: 'Dynamic'),
            ),
          ],
          onSelected: onCreateForward,
          triggerBuilder: (openMenu) => _ToolbarSplitButton(
            icon: Icons.add_rounded,
            label: tr(
              'workspace.label.newForwarding',
              fallback: 'New forwarding',
            ),
            onPrimaryPressed: () => onCreateForward('local'),
            onSecondaryPressed: (_) => openMenu(),
          ),
        ),
        Expanded(
          child: _ToolbarTrailingActions(
            children: _defaultToolbarTrailingActions(
              sortOrder: sortOrder,
              onSortOrderChanged: onSortOrderChanged,
              searchQuery: searchQuery,
              onSearchQueryChanged: onSearchQueryChanged,
            ),
          ),
        ),
      ],
    );
  }
}

class _PortForwardCard extends StatelessWidget {
  const _PortForwardCard({
    required this.forward,
    required this.onEdit,
    required this.onToggle,
  });

  final _PortForwardItem forward;
  final VoidCallback onEdit;
  final VoidCallback onToggle;

  @override
  Widget build(BuildContext context) {
    final typeLabel = switch (forward.type.toLowerCase()) {
      'remote' => tr('common.label.remote', fallback: 'Remote'),
      'dynamic' => tr('common.label.dynamic', fallback: 'Dynamic'),
      _ => tr('common.label.local', fallback: 'Local'),
    };
    final hasError = forward.statusError != null;
    final statusColor = hasError
        ? const Color(0xffd24135)
        : forward.enabled
        ? _green
        : _mutedText;
    final statusLabel = hasError
        ? tr('common.label.error', fallback: 'Error')
        : forward.enabled
        ? tr('common.label.enabled', fallback: 'Enabled')
        : tr('common.label.stopped', fallback: 'Stopped');
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: _card,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: forward.enabled ? _blue : _sidebarDivider),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Container(
                height: 24,
                padding: const EdgeInsets.symmetric(horizontal: 9),
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: const Color(0x1a075e92),
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: const Color(0x33075e92)),
                ),
                child: Text(
                  typeLabel,
                  style: TextStyle(
                    color: _text,
                    fontSize: NautermFontSizes.labelSmall,
                    fontWeight: NautermFontWeights.semibold,
                    letterSpacing: 0,
                  ),
                ),
              ),
              SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      forward.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: _text,
                        fontSize: NautermFontSizes.labelLarge,
                        fontWeight: NautermFontWeights.semibold,
                        letterSpacing: 0,
                      ),
                    ),
                    SizedBox(height: 2),
                    Text(
                      forward.type == 'remote'
                          ? tr(
                              'workspace.portForward.remoteHost',
                              fallback: 'Remote host: {host}',
                              args: {'host': forward.intermediateHostName},
                            )
                          : tr(
                              'workspace.portForward.via',
                              fallback: 'Via: {host}',
                              args: {'host': forward.intermediateHostName},
                            ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: _mutedText,
                        fontSize: NautermFontSizes.labelSmall,
                        fontWeight: NautermFontWeights.medium,
                        letterSpacing: 0,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
          SizedBox(height: 12),
          Container(
            padding: const EdgeInsets.all(10),
            decoration: BoxDecoration(
              color: _surface,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: _sidebarDivider),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  tr('common.label.path', fallback: 'Path'),
                  style: TextStyle(
                    color: _mutedText,
                    fontSize: NautermFontSizes.labelSmall,
                    fontWeight: NautermFontWeights.semibold,
                    letterSpacing: 0,
                  ),
                ),
                SizedBox(height: 4),
                Text(
                  _portForwardPath(forward),
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: _text,
                    fontSize: NautermFontSizes.labelMedium,
                    fontWeight: NautermFontWeights.medium,
                    letterSpacing: 0,
                    fontFamily: 'monospace',
                  ),
                ),
              ],
            ),
          ),
          SizedBox(height: 12),
          Row(
            children: [
              Container(
                height: 24,
                padding: const EdgeInsets.symmetric(horizontal: 9),
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: statusColor.withValues(
                    alpha: forward.enabled ? 0.14 : 0.06,
                  ),
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(
                    color: forward.enabled ? statusColor : _sidebarDivider,
                  ),
                ),
                child: Text(
                  statusLabel,
                  style: TextStyle(
                    color: statusColor,
                    fontSize: NautermFontSizes.labelSmall,
                    fontWeight: NautermFontWeights.semibold,
                    letterSpacing: 0,
                  ),
                ),
              ),
              if (forward.enabled) ...[
                SizedBox(width: 8),
                Text(
                  tr(
                    'workspace.portForward.activeConnections',
                    fallback: '{count} active',
                    args: {'count': forward.activeConnections},
                  ),
                  style: TextStyle(
                    color: _mutedText,
                    fontSize: NautermFontSizes.labelSmall,
                    fontWeight: NautermFontWeights.medium,
                    letterSpacing: 0,
                  ),
                ),
              ],
              const Spacer(),
              _WorkspaceButton(
                label: forward.enabled
                    ? tr('common.action.stop', fallback: 'Stop')
                    : tr('common.action.start', fallback: 'Start'),
                size: _WorkspaceControlSize.tiny,
                variant: _WorkspaceButtonVariant.filled,
                type: forward.enabled
                    ? _WorkspaceButtonType.defaultType
                    : _WorkspaceButtonType.primary,
                onPressed: onToggle,
              ),
              SizedBox(width: 8),
              _WorkspaceButton(
                label: 'Edit',
                size: _WorkspaceControlSize.tiny,
                variant: _WorkspaceButtonVariant.text,
                onPressed: onEdit,
              ),
            ],
          ),
          if (forward.statusError != null) ...[
            SizedBox(height: 10),
            Text(
              forward.statusError!,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: Color(0xffd24135),
                fontSize: NautermFontSizes.labelSmall,
                fontWeight: NautermFontWeights.medium,
                letterSpacing: 0,
              ),
            ),
          ],
        ],
      ),
    );
  }
}

String _normalizeProxyType(String? value) {
  final normalized = value?.trim().toLowerCase();
  return switch (normalized) {
    'socks5' => 'socks5',
    _ => 'http',
  };
}

String _proxyTypeLabel(String type) {
  return switch (_normalizeProxyType(type)) {
    'socks5' => 'SOCKS5',
    _ => 'HTTP',
  };
}

String _portForwardPath(_PortForwardItem forward) {
  final bind = '${forward.bindAddress}:${forward.bindPort}';
  final type = forward.type.toLowerCase();
  if (type == 'dynamic') {
    return '$bind  ->  SOCKS via ${forward.intermediateHostName}';
  }
  final destination = '${forward.destinationHost}:${forward.destinationPort}';
  if (type == 'remote') {
    return '$bind  ->  $destination';
  }
  return '$bind  ->  ${forward.intermediateHostName}  ->  $destination';
}

enum _SnippetCreateAction { package }

class _SnippetsToolbar extends StatelessWidget {
  const _SnippetsToolbar({
    required this.onCreateSnippet,
    required this.onCreatePackage,
    required this.onShowShellHistory,
    required this.sortOrder,
    required this.onSortOrderChanged,
    required this.searchQuery,
    required this.onSearchQueryChanged,
    required this.viewMode,
    required this.onViewModeChanged,
  });

  final VoidCallback onCreateSnippet;
  final VoidCallback onCreatePackage;
  final VoidCallback onShowShellHistory;
  final _WorkspaceSortOrder sortOrder;
  final ValueChanged<_WorkspaceSortOrder> onSortOrderChanged;
  final String searchQuery;
  final ValueChanged<String> onSearchQueryChanged;
  final _WorkspaceViewMode viewMode;
  final ValueChanged<_WorkspaceViewMode> onViewModeChanged;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceToolbarFrame(
      children: [
        _WorkspaceDropdown<_SnippetCreateAction>(
          width: 205,
          entries: [
            NautermContextMenuAction<_SnippetCreateAction>(
              value: _SnippetCreateAction.package,
              icon: Icons.inventory_2_rounded,
              label: tr(
                'workspace.label.newSnippetPackage',
                fallback: 'New snippet package',
              ),
            ),
          ],
          onSelected: (_) => onCreatePackage(),
          triggerBuilder: (openMenu) => _ToolbarSplitButton(
            icon: Icons.add_rounded,
            label: tr('workspace.label.newSnippet', fallback: 'New snippet'),
            onPrimaryPressed: onCreateSnippet,
            onSecondaryPressed: (_) => openMenu(),
          ),
        ),
        SizedBox(width: 10),
        _ModeButton(
          icon: Icons.history_rounded,
          label: 'Shell History',
          onTap: onShowShellHistory,
        ),
        Expanded(
          child: _ToolbarTrailingActions(
            children: _defaultToolbarTrailingActions(
              sortOrder: sortOrder,
              onSortOrderChanged: onSortOrderChanged,
              searchQuery: searchQuery,
              onSearchQueryChanged: onSearchQueryChanged,
              viewMode: viewMode,
              onViewModeChanged: onViewModeChanged,
            ),
          ),
        ),
      ],
    );
  }
}

enum _TerminalLogExportFormat { readableText, rawAnsi }

typedef _TerminalLogExportCallback =
    void Function(TerminalLogEntry log, _TerminalLogExportFormat format);

class _LogsPane extends StatefulWidget {
  const _LogsPane({
    required this.terminalLogs,
    required this.hasMore,
    required this.loadingMore,
    required this.captureDiskUsage,
    required this.shellHistory,
    required this.selectedLogId,
    required this.onLogReplay,
    required this.onLogSelected,
    required this.onLogDelete,
    required this.onLogExport,
    required this.onClearLogs,
    required this.onLoadMore,
    this.dataStore,
  });

  final List<TerminalLogEntry> terminalLogs;
  final bool hasMore;
  final bool loadingMore;
  final int captureDiskUsage;
  final List<ShellHistoryEntry> shellHistory;
  final String? selectedLogId;
  final ValueChanged<TerminalLogEntry> onLogReplay;
  final ValueChanged<TerminalLogEntry> onLogSelected;
  final ValueChanged<TerminalLogEntry> onLogDelete;
  final _TerminalLogExportCallback onLogExport;
  final VoidCallback onClearLogs;
  final VoidCallback onLoadMore;
  final NautermDataStore? dataStore;

  @override
  State<_LogsPane> createState() => _LogsPaneState();
}

class _LogsPaneState extends State<_LogsPane> {
  static const EdgeInsets _logsPanePadding = EdgeInsets.fromLTRB(10, 12, 10, 0);

  bool _newestFirst = true;

  @override
  Widget build(BuildContext context) {
    final sortedLogs = widget.terminalLogs.toList(growable: false)
      ..sort((a, b) {
        final result = a.startedAt.compareTo(b.startedAt);
        return _newestFirst ? -result : result;
      });

    return ColoredBox(
      color: _surface,
      child: Semantics(
        label:
            '${widget.terminalLogs.length} terminal logs, '
            '${widget.shellHistory.length} shell commands',
        child: Padding(
          padding: _logsPanePadding,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _LogsTableHeader(
                newestFirst: _newestFirst,
                canClear: widget.terminalLogs.isNotEmpty,
                onClearLogs: widget.onClearLogs,
                onDateSortToggle: () {
                  setState(() {
                    _newestFirst = !_newestFirst;
                  });
                },
              ),
              SizedBox(height: 12),
              Expanded(
                child: sortedLogs.isEmpty
                    ? Padding(
                        padding: EdgeInsets.symmetric(horizontal: 26),
                        child: Align(
                          alignment: Alignment.topLeft,
                          child: _WorkspaceInlineMessage(
                            'No terminal sessions recorded yet.',
                          ),
                        ),
                      )
                    : _LogsTableCard(
                        terminalLogs: sortedLogs,
                        selectedLogId: widget.selectedLogId,
                        onLogReplay: widget.onLogReplay,
                        onLogSelected: widget.onLogSelected,
                        onLogDelete: widget.onLogDelete,
                        onLogExport: widget.onLogExport,
                        hasMore: widget.hasMore,
                        loadingMore: widget.loadingMore,
                        onLoadMore: widget.onLoadMore,
                        dataStore: widget.dataStore,
                      ),
              ),
              SizedBox(height: 8),
              _LogsFooter(captureDiskUsage: widget.captureDiskUsage),
            ],
          ),
        ),
      ),
    );
  }
}

class _LogsFooter extends StatelessWidget {
  const _LogsFooter({required this.captureDiskUsage});

  final int captureDiskUsage;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(10, 2, 2, 4),
      child: Row(
        children: [
          Icon(LucideIcons.hardDrive, size: 14, color: _mutedText),
          SizedBox(width: 7),
          Text(
            tr('Capture storage  ${_formatStorageBytes(captureDiskUsage)}'),
            style: TextStyle(
              color: _mutedText,
              fontSize: NautermFontSizes.labelSmall,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
        ],
      ),
    );
  }
}

class _LogsTableHeader extends StatelessWidget {
  const _LogsTableHeader({
    required this.newestFirst,
    required this.canClear,
    required this.onClearLogs,
    required this.onDateSortToggle,
  });

  final bool newestFirst;
  final bool canClear;
  final VoidCallback onClearLogs;
  final VoidCallback onDateSortToggle;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 26),
      child: Row(
        children: [
          SizedBox(
            width: 112,
            child: _LogsHeaderCell(
              label: tr('common.label.date', fallback: 'Date'),
              icon: newestFirst
                  ? LucideIcons.arrowDownWideNarrow
                  : LucideIcons.arrowUpNarrowWide,
              onTap: onDateSortToggle,
            ),
          ),
          Expanded(
            flex: 3,
            child: _LogsHeaderCell(
              label: tr('common.label.user', fallback: 'User'),
            ),
          ),
          Expanded(
            flex: 4,
            child: _LogsHeaderCell(
              label: tr('common.label.host', fallback: 'Host'),
            ),
          ),
          SizedBox(
            width: 96,
            child: Row(
              children: [
                Expanded(
                  child: _LogsHeaderCell(
                    label: tr('common.label.actions', fallback: 'Actions'),
                  ),
                ),
                _LogsActionButton(
                  onPressed: canClear ? onClearLogs : null,
                  icon: LucideIcons.trash2,
                  tooltip: tr(
                    'workspace.label.deleteAllTerminalHistoryAndCaptures',
                    fallback: 'Delete all terminal history and captures',
                  ),
                  dimension: 24,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _LogsHeaderCell extends StatelessWidget {
  const _LogsHeaderCell({required this.label, this.icon, this.onTap});

  final String label;
  final IconData? icon;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final content = Row(
      children: [
        Flexible(
          child: Text(
            label,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: _text,
              fontSize: NautermFontSizes.labelMedium,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
        ),
        if (icon != null) ...[
          SizedBox(width: 8),
          if (onTap == null)
            Icon(icon, size: 15, color: const Color(0xff90a4ab))
          else
            _WorkspaceButton(
              tooltip: tr(
                'workspace.label.sortByDate',
                fallback: 'Sort by date',
              ),
              size: _WorkspaceControlSize.tiny,
              variant: _WorkspaceButtonVariant.text,
              height: 26,
              minWidth: 26,
              horizontalPadding: 0,
              onPressed: onTap,
              child: Icon(icon),
            ),
        ],
      ],
    );
    return content;
  }
}

class _LogsTableCard extends StatelessWidget {
  const _LogsTableCard({
    required this.terminalLogs,
    required this.selectedLogId,
    required this.onLogReplay,
    required this.onLogSelected,
    required this.onLogDelete,
    required this.onLogExport,
    required this.hasMore,
    required this.loadingMore,
    required this.onLoadMore,
    this.dataStore,
  });

  final List<TerminalLogEntry> terminalLogs;
  final String? selectedLogId;
  final ValueChanged<TerminalLogEntry> onLogReplay;
  final ValueChanged<TerminalLogEntry> onLogSelected;
  final ValueChanged<TerminalLogEntry> onLogDelete;
  final _TerminalLogExportCallback onLogExport;
  final bool hasMore;
  final bool loadingMore;
  final VoidCallback onLoadMore;
  final NautermDataStore? dataStore;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(7),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: _card,
          border: Border.all(color: _sidebarDivider),
          borderRadius: BorderRadius.circular(7),
        ),
        child: Material(
          type: MaterialType.transparency,
          child: NotificationListener<ScrollNotification>(
            onNotification: (notification) {
              if (hasMore &&
                  !loadingMore &&
                  notification.metrics.extentAfter < 240) {
                onLoadMore();
              }
              return false;
            },
            child: ListView.separated(
              padding: EdgeInsets.zero,
              itemCount: terminalLogs.length + (loadingMore ? 1 : 0),
              separatorBuilder: (context, index) =>
                  Divider(height: 1, color: _sidebarDivider),
              itemBuilder: (context, index) {
                if (index == terminalLogs.length) {
                  return const _WorkspaceLoadingLine();
                }
                final log = terminalLogs[index];
                return _LogsTableRow(
                  key: ValueKey(log.id),
                  log: log,
                  selected: log.id == selectedLogId,
                  dataStore: dataStore,
                  onTap: () => onLogSelected(log),
                  onReplay: () {
                    onLogSelected(log);
                    onLogReplay(log);
                  },
                  onDelete: () => onLogDelete(log),
                  onExport: (format) => onLogExport(log, format),
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}

class _LogsTableRow extends StatelessWidget {
  const _LogsTableRow({
    super.key,
    required this.log,
    required this.selected,
    required this.onTap,
    required this.onReplay,
    required this.onDelete,
    required this.onExport,
    this.dataStore,
  });

  final TerminalLogEntry log;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onReplay;
  final VoidCallback onDelete;
  final ValueChanged<_TerminalLogExportFormat> onExport;
  final NautermDataStore? dataStore;

  @override
  Widget build(BuildContext context) {
    final account = _logAccountInfo(log);
    final linkedHost = log.hostId == null
        ? null
        : dataStore?.getHost(log.hostId!);
    final host = _logHostInfo(log, linkedHost);
    return Stack(
      children: [
        Material(
          color: selected
              ? _blend(_card, _blue, _workspaceDark ? 0.1 : 0.055)
              : _card,
          child: ConstrainedBox(
            constraints: const BoxConstraints(minHeight: 60),
            child: _LogsRowInteraction(
              onSelect: onTap,
              onDoubleTap: onReplay,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 26),
                child: Row(
                  children: [
                    SizedBox(width: 112, child: _LogsDateCell(log: log)),
                    Expanded(
                      flex: 3,
                      child: _LogsIdentityCell(
                        leading: Icon(
                          LucideIcons.userRound,
                          size: 16,
                          color: _mutedText,
                        ),
                        foregroundColor: _text,
                        title: account.title,
                        subtitle: account.subtitle,
                      ),
                    ),
                    Expanded(
                      flex: 4,
                      child: _LogsIdentityCell(
                        leading: _LogsHostIcon(info: host),
                        foregroundColor: _text,
                        title: host.title,
                        subtitle: host.subtitle,
                      ),
                    ),
                    const SizedBox(width: 96),
                  ],
                ),
              ),
            ),
          ),
        ),
        Positioned(
          right: 26,
          top: 0,
          bottom: 0,
          width: 96,
          child: Align(
            alignment: Alignment.centerLeft,
            child: Row(
              children: [
                _LogsActionButton(
                  tooltip: tr('common.action.replay', fallback: 'Replay'),
                  icon: LucideIcons.play,
                  onPressed: onReplay,
                ),
                SizedBox(width: 2),
                _WorkspaceDropdown<_TerminalLogExportFormat>(
                  width: 232,
                  entries: const [
                    NautermContextMenuAction(
                      value: _TerminalLogExportFormat.readableText,
                      label: 'Readable text (.txt)',
                      icon: LucideIcons.fileText,
                    ),
                    NautermContextMenuAction(
                      value: _TerminalLogExportFormat.rawAnsi,
                      label: 'Raw terminal stream (.ansi)',
                      icon: LucideIcons.fileCode2,
                    ),
                  ],
                  onSelected: onExport,
                  triggerBuilder: (openMenu) => _LogsActionButton(
                    tooltip: tr(
                      'workspace.label.exportTerminalCapture',
                      fallback: 'Export terminal capture',
                    ),
                    icon: LucideIcons.download,
                    onPressed: openMenu,
                  ),
                ),
                SizedBox(width: 2),
                _LogsActionButton(
                  tooltip: tr(
                    'workspace.label.deleteLogAndCapture',
                    fallback: 'Delete log and capture',
                  ),
                  icon: LucideIcons.trash2,
                  onPressed: onDelete,
                ),
              ],
            ),
          ),
        ),
        if (selected)
          Positioned(
            left: 0,
            top: 12,
            bottom: 12,
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: _blue,
                borderRadius: BorderRadius.circular(1),
              ),
              child: const SizedBox(width: 2),
            ),
          ),
      ],
    );
  }
}

class _LogsRowInteraction extends StatefulWidget {
  const _LogsRowInteraction({
    required this.onSelect,
    required this.onDoubleTap,
    required this.child,
  });

  final VoidCallback onSelect;
  final VoidCallback onDoubleTap;
  final Widget child;

  @override
  State<_LogsRowInteraction> createState() => _LogsRowInteractionState();
}

class _LogsRowInteractionState extends State<_LogsRowInteraction> {
  Duration? _lastPrimaryDownTime;
  Offset? _lastPrimaryDownPosition;

  void _handlePointerDown(PointerDownEvent event) {
    if (event.buttons != kPrimaryButton) return;
    widget.onSelect();

    final lastTime = _lastPrimaryDownTime;
    final lastPosition = _lastPrimaryDownPosition;
    final isDoubleTap =
        lastTime != null &&
        lastPosition != null &&
        event.timeStamp - lastTime <= kDoubleTapTimeout &&
        (event.localPosition - lastPosition).distance <= kDoubleTapSlop;
    if (isDoubleTap) {
      _lastPrimaryDownTime = null;
      _lastPrimaryDownPosition = null;
      widget.onDoubleTap();
      return;
    }
    _lastPrimaryDownTime = event.timeStamp;
    _lastPrimaryDownPosition = event.localPosition;
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerDown: _handlePointerDown,
      child: InkWell(
        splashFactory: InkRipple.splashFactory,
        onTap: widget.onSelect,
        hoverColor: _cardHover,
        highlightColor: _blue.withValues(alpha: _workspaceDark ? 0.10 : 0.06),
        splashColor: _blue.withValues(alpha: _workspaceDark ? 0.18 : 0.12),
        child: widget.child,
      ),
    );
  }
}

class _LogsActionButton extends StatelessWidget {
  const _LogsActionButton({
    required this.tooltip,
    required this.icon,
    required this.onPressed,
    this.dimension = 28,
  });

  final String tooltip;
  final IconData icon;
  final VoidCallback? onPressed;
  final double dimension;

  @override
  Widget build(BuildContext context) {
    final enabled = onPressed != null;
    final shape = RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(5),
    );
    return Tooltip(
      message: tr(tooltip),
      waitDuration: const Duration(milliseconds: 250),
      child: Material(
        type: MaterialType.transparency,
        shape: shape,
        clipBehavior: Clip.antiAlias,
        child: InkResponse(
          onTap: onPressed,
          containedInkWell: true,
          customBorder: shape,
          splashFactory: InkRipple.splashFactory,
          hoverColor: enabled ? _blue.withValues(alpha: 0.08) : null,
          highlightColor: enabled ? _blue.withValues(alpha: 0.12) : null,
          splashColor: enabled ? _blue.withValues(alpha: 0.28) : null,
          child: SizedBox.square(
            dimension: dimension,
            child: Icon(
              icon,
              size: 16,
              color: enabled ? _mutedText : _mutedText.withValues(alpha: 0.45),
            ),
          ),
        ),
      ),
    );
  }
}

String _formatStorageBytes(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  if (bytes < 1024 * 1024 * 1024) {
    return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }
  return '${(bytes / (1024 * 1024 * 1024)).toStringAsFixed(1)} GB';
}

class _LogsDateCell extends StatelessWidget {
  const _LogsDateCell({required this.log});

  final TerminalLogEntry log;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 10),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            _formatLogDate(log.startedAt),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: _text,
              fontSize: NautermFontSizes.labelMedium,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
          SizedBox(height: 4),
          Text(
            _formatLogTimeRange(log),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: _mutedText,
              fontSize: NautermFontSizes.labelSmall,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
        ],
      ),
    );
  }
}

class _LogsIdentityCell extends StatelessWidget {
  const _LogsIdentityCell({
    required this.leading,
    required this.foregroundColor,
    required this.title,
    required this.subtitle,
  });

  final Widget leading;
  final Color foregroundColor;
  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        SizedBox(width: 24, height: 24, child: Center(child: leading)),
        SizedBox(width: 10),
        Expanded(
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 9),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: foregroundColor,
                    fontSize: NautermFontSizes.labelMedium,
                    fontWeight: NautermFontWeights.medium,
                    letterSpacing: 0,
                  ),
                ),
                SizedBox(height: 3),
                Text(
                  subtitle,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: _mutedText,
                    fontSize: NautermFontSizes.labelSmall,
                    fontWeight: NautermFontWeights.medium,
                    letterSpacing: 0,
                  ),
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

class _LogsHostIcon extends StatelessWidget {
  const _LogsHostIcon({required this.info});

  final _LogHostInfo info;

  @override
  Widget build(BuildContext context) {
    if (info.kind == _LogHostKind.serial) {
      return _compactIcon(
        color: const Color(0xff7456c8),
        child: const Icon(LucideIcons.usb, size: 14, color: Colors.white),
      );
    }
    if (info.kind == _LogHostKind.local) {
      return _compactIcon(
        color: const Color(0xff075e92),
        child: const Icon(
          LucideIcons.squareTerminal,
          size: 14,
          color: Colors.white,
        ),
      );
    }

    final osSlug = _BrandIcon._resolveOsSlug(info.os, info.distro);
    if (hostIconMode == HostIconMode.osIcon && osSlug != null) {
      return _compactOsIcon(osSlug);
    }

    final initial = _compactInitial(info.title);
    if (hostIconMode != HostIconMode.osBadge || osSlug == null) {
      return initial;
    }
    return Stack(
      clipBehavior: Clip.none,
      children: [
        initial,
        Positioned(
          right: -2,
          bottom: -2,
          child: Container(
            width: 10,
            height: 10,
            padding: const EdgeInsets.all(1.5),
            decoration: BoxDecoration(
              color: _BrandIcon._osBrandColor(osSlug),
              borderRadius: BorderRadius.circular(3),
              border: Border.all(color: _card, width: 1),
            ),
            child: SvgPicture.asset(
              'assets/icons/os/system-$osSlug.svg',
              colorFilter: const ColorFilter.mode(
                Colors.white,
                BlendMode.srcIn,
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _compactInitial(String name) {
    final initial = name.trim().isEmpty ? '?' : name.trim()[0].toUpperCase();
    return _compactIcon(
      color: _InitialIcon._deterministicColor(name),
      child: Text(
        initial,
        style: const TextStyle(
          color: Colors.white,
          fontSize: 12,
          fontWeight: FontWeight.w600,
        ),
      ),
    );
  }

  Widget _compactOsIcon(String osSlug) {
    return _compactIcon(
      color: _BrandIcon._osBrandColor(osSlug),
      child: Padding(
        padding: const EdgeInsets.all(4.5),
        child: SvgPicture.asset(
          'assets/icons/os/system-$osSlug.svg',
          colorFilter: const ColorFilter.mode(Colors.white, BlendMode.srcIn),
        ),
      ),
    );
  }

  Widget _compactIcon({required Color color, required Widget child}) {
    return Container(
      width: 24,
      height: 24,
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(6),
      ),
      alignment: Alignment.center,
      child: child,
    );
  }
}

List<_KnownHostItem> _parseKnownHosts(String text) {
  final lines = text.split('\n');
  final items = <_KnownHostItem>[];

  for (var index = 0; index < lines.length; index++) {
    final rawLine = lines[index].replaceFirst(RegExp(r'\r$'), '');
    final line = rawLine.trim();
    if (line.isEmpty || line.startsWith('#')) {
      continue;
    }

    final fields = line.split(RegExp(r'\s+'));
    final hostFieldIndex = fields.first.startsWith('@') ? 1 : 0;
    if (fields.length <= hostFieldIndex + 1) {
      continue;
    }

    final hostPattern = fields[hostFieldIndex];
    final keyType = fields[hostFieldIndex + 1];
    final displayPattern = hostPattern.split(',').first.trim();
    if (displayPattern.isEmpty) {
      continue;
    }
    final target = _parseKnownHostTarget(displayPattern);

    items.add(
      _KnownHostItem(
        lineIndex: index,
        line: rawLine,
        hostPattern: displayPattern,
        host: target?.host,
        port: target?.port,
        name: displayPattern,
        subtitle: keyType,
        icon: Icons.fingerprint_rounded,
        color: const Color(0xff075e92),
      ),
    );
  }

  return items;
}

_KnownHostTarget? _parseKnownHostTarget(String pattern) {
  if (pattern.startsWith('|') ||
      pattern.contains('*') ||
      pattern.contains('?')) {
    return null;
  }

  if (pattern.startsWith('[')) {
    final closeBracket = pattern.indexOf(']');
    if (closeBracket <= 1 ||
        closeBracket + 2 >= pattern.length ||
        pattern[closeBracket + 1] != ':') {
      return null;
    }
    final port = int.tryParse(pattern.substring(closeBracket + 2));
    if (port == null || port < 1 || port > 65535) {
      return null;
    }
    return _KnownHostTarget(pattern.substring(1, closeBracket), port);
  }

  if (pattern.contains(':')) {
    return null;
  }

  return _KnownHostTarget(pattern, 22);
}

_LogAccountInfo _logAccountInfo(TerminalLogEntry log) {
  final username = log.username?.trim();
  return _LogAccountInfo(
    title: username == null || username.isEmpty ? 'Local' : username,
    subtitle: log.host?.trim().isNotEmpty == true
        ? log.host!.trim()
        : io.Platform.localHostname,
  );
}

_LogHostInfo _logHostInfo(TerminalLogEntry log, HostEntry? linkedHost) {
  if (_isSerialTerminalLog(log)) {
    return _LogHostInfo(
      title: log.title,
      subtitle: 'Serial device',
      kind: _LogHostKind.serial,
    );
  }

  final host = log.host?.trim();
  if (host != null && host.isNotEmpty) {
    final username = log.username?.trim();
    return _LogHostInfo(
      title: log.title,
      subtitle: username == null || username.isEmpty ? 'ssh' : 'ssh, $username',
      kind: _LogHostKind.remote,
      os: linkedHost?.os,
      distro: linkedHost?.distro,
    );
  }

  final shell = log.shellPath?.trim();
  final cwd = log.cwd?.trim() ?? log.workDir?.trim();
  return _LogHostInfo(
    title: log.title,
    subtitle: cwd?.isNotEmpty == true
        ? cwd!
        : shell?.isNotEmpty == true
        ? shell!
        : 'local',
    kind: _LogHostKind.local,
  );
}

bool _isSerialTerminalLog(TerminalLogEntry log) {
  final title = log.title.trim();
  if (title.startsWith('/dev/')) return true;
  if (RegExp(r'^COM\d+$', caseSensitive: false).hasMatch(title)) return true;
  return title.startsWith(r'\\.\COM');
}

String _formatLogDate(DateTime timestamp) {
  final local = timestamp.toLocal();
  const months = [
    'Jan',
    'Feb',
    'Mar',
    'Apr',
    'May',
    'Jun',
    'Jul',
    'Aug',
    'Sep',
    'Oct',
    'Nov',
    'Dec',
  ];
  return '${months[local.month - 1]} ${local.day}, ${local.year}';
}

String _formatLogTimeRange(TerminalLogEntry log) {
  final start = log.startedAt.toLocal();
  final end = (log.endedAt ?? DateTime.now()).toLocal();
  final suffixDays = DateTime(
    end.year,
    end.month,
    end.day,
  ).difference(DateTime(start.year, start.month, start.day)).inDays;
  final suffix = suffixDays > 0 ? ' (+${suffixDays}d)' : '';
  return '${_formatLogTime(start)} - ${_formatLogTime(end)}$suffix';
}

String _formatLogTime(DateTime timestamp) {
  return '${timestamp.hour.toString().padLeft(2, '0')}:'
      '${timestamp.minute.toString().padLeft(2, '0')}';
}

class _LogAccountInfo {
  const _LogAccountInfo({required this.title, required this.subtitle});

  final String title;
  final String subtitle;
}

class _LogHostInfo {
  const _LogHostInfo({
    required this.title,
    required this.subtitle,
    required this.kind,
    this.os,
    this.distro,
  });

  final String title;
  final String subtitle;
  final _LogHostKind kind;
  final String? os;
  final String? distro;
}

enum _LogHostKind { remote, local, serial }

class _KnownHostTarget {
  const _KnownHostTarget(this.host, this.port);

  final String host;
  final int port;
}
