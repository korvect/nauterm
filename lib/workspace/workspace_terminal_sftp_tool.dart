part of 'nauterm_workspace.dart';

class _TerminalSftpPanel extends ConsumerStatefulWidget {
  const _TerminalSftpPanel({
    required this.sessionId,
    required this.colors,
    required this.groups,
    required this.hosts,
    required this.tags,
    required this.dataStore,
    required this.profile,
    required this.profileHost,
    required this.terminalController,
    required this.createHostRequest,
    this.onRemoteConnected,
  });

  final String sessionId;
  final _AiAssistantColors colors;
  final List<_GroupItem> groups;
  final List<_HostItem> hosts;
  final List<TagEntry> tags;
  final NautermDataStore? dataStore;
  final SshConnectionProfile? profile;
  final _HostItem? profileHost;
  final TerminalController? terminalController;
  final _SftpConnectRequest? Function(int requestId, _HostItem host)
  createHostRequest;
  final void Function(_HostItem host, _SftpRemoteAuth auth)? onRemoteConnected;

  @override
  ConsumerState<_TerminalSftpPanel> createState() => _TerminalSftpPanelState();
}

class _TerminalSftpPanelState extends ConsumerState<_TerminalSftpPanel> {
  int _nextRequestId = 0;
  _SftpConnectRequest? _connectRequest;
  late final _SftpPaneController _paneController;
  SshConnectionProfile? _boundProfile;
  int? _boundHostId;
  String? _boundHostName;

  @override
  void initState() {
    super.initState();
    _paneController = ref.read(
      _sftpPaneControllerProvider('${widget.sessionId}:sftp-tool'),
    );
    _nextRequestId = _paneController.retainedConnectRequest?.id ?? 0;
    final sameBinding =
        identical(_paneController.boundToolProfile, widget.profile) &&
        _paneController.boundToolHostId == widget.profileHost?.id &&
        _paneController.boundToolHostName == widget.profileHost?.name;
    _rememberConnection();
    _connectRequest = sameBinding
        ? _paneController.retainedConnectRequest ?? _createProfileRequest()
        : _createProfileRequest();
  }

  @override
  void didUpdateWidget(covariant _TerminalSftpPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    final host = widget.profileHost;
    if (!identical(_boundProfile, widget.profile) ||
        _boundHostId != host?.id ||
        _boundHostName != host?.name) {
      _rememberConnection();
      _connectRequest = _createProfileRequest();
    }
  }

  void _rememberConnection() {
    final host = widget.profileHost;
    _boundProfile = widget.profile;
    _boundHostId = host?.id;
    _boundHostName = host?.name;
    _paneController.boundToolProfile = widget.profile;
    _paneController.boundToolHostId = host?.id;
    _paneController.boundToolHostName = host?.name;
  }

  _SftpConnectRequest? _createProfileRequest() {
    final profile = widget.profile;
    final host = widget.profileHost;
    if (profile == null || host == null) {
      return null;
    }
    return _SftpConnectRequest(
      id: ++_nextRequestId,
      host: host,
      auth: _SftpRemoteAuth(
        host: profile.host,
        port: profile.port,
        username: profile.username,
        password: profile.password,
        privateKey: profile.privateKey,
        passphrase: profile.passphrase,
        proxy: profile.proxy,
        knownHostsPath: profile.knownHostsPath,
      ),
    );
  }

  void _selectHost(_HostItem host) {
    final request = widget.createHostRequest(++_nextRequestId, host);
    if (request == null) {
      return;
    }
    setState(() => _connectRequest = request);
  }

  @override
  Widget build(BuildContext context) {
    final available = widget.profile != null && widget.profileHost != null;
    return Expanded(
      key: const ValueKey('terminal-sftp-tool-panel'),
      child: available
          ? _SftpPane(
              key: ValueKey(
                'terminal-sftp-tool:${widget.terminalController.hashCode}',
              ),
              sessionId: '${widget.sessionId}:sftp-tool',
              active: true,
              groups: widget.groups,
              hosts: widget.hosts,
              tags: widget.tags,
              dataStore: widget.dataStore,
              connectRequest: _connectRequest,
              onHostSelected: _selectHost,
              onRemoteConnected: widget.onRemoteConnected,
              manageFileDrop: false,
              remoteOnly: true,
              sshEditorController: widget.terminalController,
              compact: true,
              panelColors: widget.colors,
            )
          : _TerminalToolEmptyState(
              icon: LucideIcons.folderOpen,
              title: 'No SSH session',
              description: 'SFTP is available after connecting with SSH.',
              colors: widget.colors,
            ),
    );
  }
}

class _TerminalSftpBrowser extends StatefulWidget {
  const _TerminalSftpBrowser({
    required this.colors,
    required this.title,
    required this.path,
    required this.pathController,
    required this.pathFocusNode,
    required this.filterController,
    required this.entries,
    required this.selectedPath,
    required this.selectedPaths,
    required this.selectionAnchorPath,
    required this.sortColumn,
    required this.sortAscending,
    required this.loading,
    required this.loadError,
    required this.editingPath,
    required this.canGoBack,
    required this.canGoForward,
    required this.showHiddenFiles,
    required this.sshEditorAvailable,
    required this.tasks,
    required this.taskListOpen,
    required this.favoriteListOpen,
    required this.favoritePaths,
    required this.onHome,
    required this.onBack,
    required this.onForward,
    required this.onPathEditRequested,
    required this.onPathSubmitted,
    required this.onPathEditCancelled,
    required this.onSelectionChanged,
    required this.onEntryDoubleTap,
    required this.onEntrySecondaryTapDown,
    required this.onRefresh,
    required this.onTaskListToggle,
    required this.onTaskClearCompleted,
    required this.onTaskDismiss,
    required this.onTaskCancel,
    required this.onTaskPauseToggle,
    required this.onPanelsDismiss,
    required this.onPathFavoriteToggle,
    required this.onFavoriteListToggle,
    required this.onFavoritePathSelected,
    required this.onSortChanged,
    required this.onAction,
  });

  final _AiAssistantColors colors;
  final String title;
  final String path;
  final TextEditingController pathController;
  final FocusNode pathFocusNode;
  final TextEditingController filterController;
  final List<_SftpFileEntry> entries;
  final String? selectedPath;
  final Set<String> selectedPaths;
  final String? selectionAnchorPath;
  final _SftpSortColumn sortColumn;
  final bool sortAscending;
  final bool loading;
  final Object? loadError;
  final bool editingPath;
  final bool canGoBack;
  final bool canGoForward;
  final bool showHiddenFiles;
  final bool sshEditorAvailable;
  final List<_SftpTask> tasks;
  final bool taskListOpen;
  final bool favoriteListOpen;
  final List<String> favoritePaths;
  final VoidCallback onHome;
  final VoidCallback onBack;
  final VoidCallback onForward;
  final VoidCallback onPathEditRequested;
  final ValueChanged<String> onPathSubmitted;
  final VoidCallback onPathEditCancelled;
  final ValueChanged<_SftpSelectionChange> onSelectionChanged;
  final ValueChanged<_SftpFileEntry> onEntryDoubleTap;
  final void Function(TapDownDetails details, _SftpFileEntry entry)
  onEntrySecondaryTapDown;
  final VoidCallback onRefresh;
  final VoidCallback onTaskListToggle;
  final VoidCallback onTaskClearCompleted;
  final ValueChanged<int> onTaskDismiss;
  final ValueChanged<int> onTaskCancel;
  final ValueChanged<int> onTaskPauseToggle;
  final VoidCallback onPanelsDismiss;
  final VoidCallback onPathFavoriteToggle;
  final VoidCallback onFavoriteListToggle;
  final ValueChanged<String> onFavoritePathSelected;
  final ValueChanged<_SftpSortColumn> onSortChanged;
  final ValueChanged<_SftpAction> onAction;

  @override
  State<_TerminalSftpBrowser> createState() => _TerminalSftpBrowserState();
}

class _TerminalSftpBrowserState extends State<_TerminalSftpBrowser> {
  final GlobalKey _actionsKey = GlobalKey();
  final GlobalKey _sortKey = GlobalKey();
  final FocusNode _searchFocusNode = FocusNode();
  bool _searching = false;

  @override
  void dispose() {
    _searchFocusNode.dispose();
    super.dispose();
  }

  void _beginSearch() {
    setState(() => _searching = true);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        _searchFocusNode.requestFocus();
      }
    });
  }

  void _endSearch() {
    widget.filterController.clear();
    _searchFocusNode.unfocus();
    setState(() => _searching = false);
  }

  Rect? _anchorFor(GlobalKey key) {
    final buttonBox = key.currentContext?.findRenderObject();
    final overlayBox = Overlay.of(context).context.findRenderObject();
    if (buttonBox is! RenderBox || overlayBox is! RenderBox) {
      return null;
    }
    return buttonBox.localToGlobal(Offset.zero, ancestor: overlayBox) &
        buttonBox.size;
  }

  Future<void> _showActions() async {
    final anchor = _anchorFor(_actionsKey);
    if (anchor == null) {
      return;
    }
    final selectedEntry = widget.entries
        .where((entry) => entry.path == widget.selectedPath)
        .firstOrNull;
    final selectionIsFile =
        selectedEntry != null &&
        !selectedEntry.isDirectory &&
        !selectedEntry.isParent;
    final action = await showNautermDropdownMenu<_SftpAction>(
      context: context,
      anchor: anchor,
      width: 210,
      style: _terminalSftpMenuStyle(widget.colors),
      entries: _sftpContextMenuEntries(
        _sftpActionMenuEntries(
          hasSelection:
              widget.selectedPath != null || widget.selectedPaths.isNotEmpty,
          selectionIsFile: selectionIsFile,
          selectionCanOpenWithSystemDefault:
              selectedEntry == null ||
              selectedEntry.isDirectory ||
              selectedEntry.isParent ||
              canOpenFileWithSystemDefaultApplication(
                selectedEntry.name,
                permissions: selectedEntry.permissions,
              ),
          showHiddenFiles: widget.showHiddenFiles,
          remote: true,
          showCloseAction: false,
          sshEditorAvailable: widget.sshEditorAvailable,
        ),
      ),
      maxHeight: double.infinity,
    );
    if (action != null && mounted) {
      widget.onAction(action);
    }
  }

  Future<void> _showSortMenu() async {
    final anchor = _anchorFor(_sortKey);
    if (anchor == null) {
      return;
    }
    final column = await showNautermDropdownMenu<_SftpSortColumn>(
      context: context,
      anchor: anchor,
      width: 150,
      style: _terminalSftpMenuStyle(widget.colors),
      entries: [
        for (final value in _SftpSortColumn.values)
          NautermContextMenuAction<_SftpSortColumn>(
            value: value,
            label: _terminalSftpSortLabel(value),
            selected: widget.sortColumn == value,
          ),
      ],
    );
    if (column != null && mounted) {
      widget.onSortChanged(column);
    }
  }

  void _selectEntry(_SftpFileEntry entry) {
    final additive =
        HardwareKeyboard.instance.isMetaPressed ||
        HardwareKeyboard.instance.isControlPressed;
    final next = additive ? Set<String>.from(widget.selectedPaths) : <String>{};
    if (additive && next.contains(entry.path)) {
      next.remove(entry.path);
    } else {
      next.add(entry.path);
    }
    widget.onSelectionChanged(
      _SftpSelectionChange(
        selectedPaths: next,
        primaryPath: next.contains(entry.path) ? entry.path : next.firstOrNull,
        anchorPath: entry.path,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final colors = widget.colors;
    final visibleEntries = _sortSftpEntries(
      _filterSftpEntries(widget.entries, widget.filterController.text),
      widget.sortColumn,
      ascending: widget.sortAscending,
    );
    final panelOpen = widget.taskListOpen || widget.favoriteListOpen;
    return ColoredBox(
      color: colors.background,
      child: Stack(
        children: [
          Column(
            children: [
              Container(
                height: 40,
                padding: const EdgeInsets.fromLTRB(9, 0, 6, 0),
                decoration: BoxDecoration(
                  color: colors.background,
                  border: Border(bottom: BorderSide(color: colors.border)),
                ),
                child: Row(
                  children: [
                    Icon(LucideIcons.folder, size: 15, color: colors.accent),
                    const SizedBox(width: 7),
                    Expanded(
                      child: _searching
                          ? TextField(
                              controller: widget.filterController,
                              focusNode: _searchFocusNode,
                              style: TextStyle(
                                color: colors.foreground,
                                fontSize: 11.5,
                                letterSpacing: 0,
                              ),
                              cursorColor: colors.accent,
                              decoration: InputDecoration.collapsed(
                                hintText: tr(
                                  'workspace.label.filterFiles',
                                  fallback: 'Filter files',
                                ),
                                hintStyle: TextStyle(
                                  color: colors.muted,
                                  fontSize: 11.5,
                                  letterSpacing: 0,
                                ),
                              ),
                            )
                          : Text(
                              widget.title,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                color: colors.foreground,
                                fontSize: 12,
                                fontWeight: NautermFontWeights.semibold,
                                letterSpacing: 0,
                              ),
                            ),
                    ),
                    _TerminalSftpIconButton(
                      tooltip: _searching ? 'Close filter' : 'Filter files',
                      icon: _searching ? LucideIcons.x : LucideIcons.search,
                      colors: colors,
                      onPressed: _searching ? _endSearch : _beginSearch,
                    ),
                    const SizedBox(width: 2),
                    _TerminalSftpIconButton(
                      tooltip: tr('common.action.refresh', fallback: 'Refresh'),
                      icon: LucideIcons.refreshCw,
                      colors: colors,
                      onPressed: widget.loading ? null : widget.onRefresh,
                    ),
                    const SizedBox(width: 2),
                    _TerminalSftpIconButton(
                      key: _sortKey,
                      tooltip: tr(
                        'Sort by ${_terminalSftpSortLabel(widget.sortColumn)}',
                      ),
                      icon: widget.sortAscending
                          ? LucideIcons.arrowUpNarrowWide
                          : LucideIcons.arrowDownWideNarrow,
                      colors: colors,
                      onPressed: _showSortMenu,
                    ),
                    const SizedBox(width: 2),
                    _TerminalSftpTaskButton(
                      colors: colors,
                      tasks: widget.tasks,
                      selected: widget.taskListOpen,
                      onPressed: widget.onTaskListToggle,
                    ),
                    const SizedBox(width: 2),
                    _TerminalSftpIconButton(
                      key: _actionsKey,
                      tooltip: tr('common.label.actions', fallback: 'Actions'),
                      icon: LucideIcons.ellipsis,
                      colors: colors,
                      onPressed: _showActions,
                    ),
                  ],
                ),
              ),
              Container(
                height: 36,
                padding: const EdgeInsets.fromLTRB(5, 0, 6, 0),
                decoration: BoxDecoration(
                  color: colors.inputBackground.withValues(alpha: 0.56),
                  border: Border(bottom: BorderSide(color: colors.border)),
                ),
                child: Row(
                  children: [
                    _TerminalSftpIconButton(
                      tooltip: tr('common.label.home', fallback: 'Home'),
                      icon: LucideIcons.house,
                      colors: colors,
                      onPressed: widget.onHome,
                    ),
                    _TerminalSftpIconButton(
                      tooltip: tr('common.action.back', fallback: 'Back'),
                      icon: LucideIcons.chevronLeft,
                      colors: colors,
                      onPressed: widget.canGoBack ? widget.onBack : null,
                    ),
                    _TerminalSftpIconButton(
                      tooltip: tr('common.label.forward', fallback: 'Forward'),
                      icon: LucideIcons.chevronRight,
                      colors: colors,
                      onPressed: widget.canGoForward ? widget.onForward : null,
                    ),
                    const SizedBox(width: 2),
                    Expanded(
                      child: widget.editingPath
                          ? SizedBox.expand(
                              child: TextField(
                                controller: widget.pathController,
                                focusNode: widget.pathFocusNode,
                                autofocus: true,
                                expands: true,
                                minLines: null,
                                maxLines: null,
                                textAlignVertical: TextAlignVertical.center,
                                onSubmitted: widget.onPathSubmitted,
                                onTapOutside: (_) =>
                                    widget.onPathEditCancelled(),
                                style: TextStyle(
                                  color: colors.foreground,
                                  fontSize: 11.5,
                                  height: 1,
                                  fontFamily: 'monospace',
                                  letterSpacing: 0,
                                ),
                                cursorColor: colors.accent,
                                decoration: const InputDecoration(
                                  isCollapsed: true,
                                  border: InputBorder.none,
                                  contentPadding: EdgeInsets.zero,
                                ),
                              ),
                            )
                          : GestureDetector(
                              behavior: HitTestBehavior.opaque,
                              onTap: widget.onPathEditRequested,
                              child: Align(
                                alignment: Alignment.centerLeft,
                                child: Text(
                                  widget.path,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                    color: colors.foreground,
                                    fontSize: 11.5,
                                    height: 1,
                                    fontFamily: 'monospace',
                                    letterSpacing: 0,
                                  ),
                                ),
                              ),
                            ),
                    ),
                    _TerminalSftpIconButton(
                      tooltip: widget.favoritePaths.contains(widget.path)
                          ? 'Remove favorite'
                          : 'Favorite path',
                      icon: widget.favoritePaths.contains(widget.path)
                          ? LucideIcons.star
                          : LucideIcons.star,
                      colors: colors,
                      selected: widget.favoritePaths.contains(widget.path),
                      onPressed: widget.onPathFavoriteToggle,
                    ),
                    _TerminalSftpIconButton(
                      tooltip: tr(
                        'workspace.label.favoritePaths',
                        fallback: 'Favorite paths',
                      ),
                      icon: LucideIcons.bookmark,
                      colors: colors,
                      selected: widget.favoriteListOpen,
                      onPressed: widget.onFavoriteListToggle,
                    ),
                  ],
                ),
              ),
              Expanded(
                child: widget.loadError != null && widget.entries.isEmpty
                    ? _TerminalToolEmptyState(
                        icon: LucideIcons.circleAlert,
                        title: 'Unable to load folder',
                        description: '${widget.loadError}',
                        colors: colors,
                        actionLabel: 'Retry',
                        onAction: widget.onRefresh,
                      )
                    : visibleEntries.isEmpty && !widget.loading
                    ? Center(
                        child: Text(
                          widget.filterController.text.trim().isEmpty
                              ? 'This folder is empty.'
                              : 'No matching files.',
                          style: TextStyle(
                            color: colors.muted,
                            fontSize: 11,
                            letterSpacing: 0,
                          ),
                        ),
                      )
                    : Stack(
                        children: [
                          ListView.builder(
                            padding: const EdgeInsets.fromLTRB(5, 5, 5, 10),
                            itemCount: visibleEntries.length,
                            itemBuilder: (context, index) {
                              final entry = visibleEntries[index];
                              return _TerminalSftpFileRow(
                                key: ValueKey(entry.path),
                                entry: entry,
                                selected: widget.selectedPaths.contains(
                                  entry.path,
                                ),
                                colors: colors,
                                onTap: () => _selectEntry(entry),
                                onDoubleTap: () =>
                                    widget.onEntryDoubleTap(entry),
                                onSecondaryTapDown: (details) => widget
                                    .onEntrySecondaryTapDown(details, entry),
                              );
                            },
                          ),
                          if (widget.loading)
                            Positioned(
                              left: 0,
                              right: 0,
                              top: 0,
                              child: LinearProgressIndicator(
                                minHeight: 1.5,
                                color: colors.accent,
                                backgroundColor: Colors.transparent,
                              ),
                            ),
                        ],
                      ),
              ),
            ],
          ),
          if (panelOpen)
            Positioned.fill(
              child: GestureDetector(
                behavior: HitTestBehavior.translucent,
                onTap: widget.onPanelsDismiss,
                child: const SizedBox.expand(),
              ),
            ),
          if (widget.taskListOpen)
            Positioned(
              left: 8,
              right: 8,
              top: 46,
              height: 190,
              child: _TerminalSftpTasksPanel(
                colors: colors,
                tasks: widget.tasks,
                onClearCompleted: widget.onTaskClearCompleted,
                onDismiss: widget.onTaskDismiss,
                onCancel: widget.onTaskCancel,
                onPauseToggle: widget.onTaskPauseToggle,
              ),
            ),
          if (widget.favoriteListOpen)
            Positioned(
              left: 8,
              right: 8,
              top: 46,
              child: _TerminalSftpFavoritesPanel(
                colors: colors,
                paths: widget.favoritePaths,
                onSelected: widget.onFavoritePathSelected,
              ),
            ),
        ],
      ),
    );
  }
}

class _TerminalSftpIconButton extends StatelessWidget {
  const _TerminalSftpIconButton({
    super.key,
    required this.tooltip,
    required this.icon,
    required this.colors,
    required this.onPressed,
    this.selected = false,
  });

  final String tooltip;
  final IconData icon;
  final _AiAssistantColors colors;
  final VoidCallback? onPressed;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final enabled = onPressed != null;
    return IconButton(
      tooltip: tooltip,
      onPressed: onPressed,
      icon: Icon(
        icon,
        size: 15,
        color: enabled
            ? selected
                  ? colors.accent
                  : colors.muted
            : colors.muted.withValues(alpha: 0.34),
      ),
      padding: EdgeInsets.zero,
      constraints: const BoxConstraints.tightFor(width: 29, height: 29),
      visualDensity: VisualDensity.compact,
      style: IconButton.styleFrom(
        backgroundColor: selected ? colors.inputBackground : null,
        hoverColor: colors.inputBackground,
        highlightColor: colors.accent.withValues(alpha: 0.12),
      ),
    );
  }
}

class _TerminalSftpTaskButton extends StatelessWidget {
  const _TerminalSftpTaskButton({
    required this.colors,
    required this.tasks,
    required this.selected,
    required this.onPressed,
  });

  final _AiAssistantColors colors;
  final List<_SftpTask> tasks;
  final bool selected;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final active = tasks.where((task) {
      return task.status == _SftpTaskStatus.running ||
          task.status == _SftpTaskStatus.queued ||
          task.status == _SftpTaskStatus.paused;
    }).length;
    return Stack(
      clipBehavior: Clip.none,
      children: [
        _TerminalSftpIconButton(
          tooltip: tr('common.label.tasks', fallback: 'Tasks'),
          icon: LucideIcons.listTodo,
          colors: colors,
          selected: selected,
          onPressed: onPressed,
        ),
        if (active > 0)
          Positioned(
            right: 0,
            top: 0,
            child: Container(
              constraints: const BoxConstraints(minWidth: 12, minHeight: 12),
              padding: const EdgeInsets.symmetric(horizontal: 2),
              decoration: BoxDecoration(
                color: colors.accent,
                borderRadius: BorderRadius.circular(999),
              ),
              child: Text(
                active > 9 ? '9+' : '$active',
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: colors.canvasBackground,
                  fontSize: 7.5,
                  fontWeight: FontWeight.w700,
                  height: 1.45,
                  letterSpacing: 0,
                ),
              ),
            ),
          ),
      ],
    );
  }
}

class _TerminalSftpFileRow extends StatefulWidget {
  const _TerminalSftpFileRow({
    super.key,
    required this.entry,
    required this.selected,
    required this.colors,
    required this.onTap,
    required this.onDoubleTap,
    required this.onSecondaryTapDown,
  });

  final _SftpFileEntry entry;
  final bool selected;
  final _AiAssistantColors colors;
  final VoidCallback onTap;
  final VoidCallback onDoubleTap;
  final GestureTapDownCallback onSecondaryTapDown;

  @override
  State<_TerminalSftpFileRow> createState() => _TerminalSftpFileRowState();
}

class _TerminalSftpFileRowState extends State<_TerminalSftpFileRow> {
  Duration? _lastPrimaryDownTime;
  Offset? _lastPrimaryDownPosition;

  void _handlePointerDown(PointerDownEvent event) {
    if (event.buttons != kPrimaryMouseButton) {
      return;
    }
    final previousTime = _lastPrimaryDownTime;
    final previousPosition = _lastPrimaryDownPosition;
    final doubleTap =
        previousTime != null &&
        previousPosition != null &&
        event.timeStamp - previousTime <= kDoubleTapTimeout &&
        (event.position - previousPosition).distance <= kDoubleTapSlop;
    if (doubleTap) {
      _lastPrimaryDownTime = null;
      _lastPrimaryDownPosition = null;
      widget.onDoubleTap();
      return;
    }
    _lastPrimaryDownTime = event.timeStamp;
    _lastPrimaryDownPosition = event.position;
  }

  @override
  Widget build(BuildContext context) {
    final metadata = [
      if (!widget.entry.isParent && widget.entry.permissions.trim().isNotEmpty)
        widget.entry.permissions.trim(),
      if (!widget.entry.isDirectory) _formatBytes(widget.entry.size),
      if (!widget.entry.isParent)
        _compactTerminalDateTime(widget.entry.modified),
    ].join(' · ');
    return Listener(
      onPointerDown: _handlePointerDown,
      child: Material(
        color: widget.selected
            ? widget.colors.inputBackground
            : Colors.transparent,
        borderRadius: BorderRadius.circular(6),
        child: InkWell(
          borderRadius: BorderRadius.circular(6),
          onTap: widget.onTap,
          onSecondaryTapDown: widget.onSecondaryTapDown,
          hoverColor: widget.colors.inputBackground,
          highlightColor: widget.colors.accent.withValues(alpha: 0.10),
          child: SizedBox(
            height: 39,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 7),
              child: Row(
                children: [
                  Icon(
                    widget.entry.isDirectory
                        ? LucideIcons.folder
                        : LucideIcons.file,
                    size: 15,
                    color: widget.entry.isDirectory
                        ? widget.colors.accent
                        : widget.colors.muted,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          widget.entry.name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            color: widget.colors.foreground,
                            fontSize: 11.5,
                            fontWeight: NautermFontWeights.medium,
                            letterSpacing: 0,
                          ),
                        ),
                        if (metadata.isNotEmpty)
                          Text(
                            metadata,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: widget.colors.muted,
                              fontSize: 9,
                              height: 1.25,
                              letterSpacing: 0,
                            ),
                          ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _TerminalSftpTasksPanel extends StatelessWidget {
  const _TerminalSftpTasksPanel({
    required this.colors,
    required this.tasks,
    required this.onClearCompleted,
    required this.onDismiss,
    required this.onCancel,
    required this.onPauseToggle,
  });

  final _AiAssistantColors colors;
  final List<_SftpTask> tasks;
  final VoidCallback onClearCompleted;
  final ValueChanged<int> onDismiss;
  final ValueChanged<int> onCancel;
  final ValueChanged<int> onPauseToggle;

  @override
  Widget build(BuildContext context) {
    final ordered = _orderedSftpTasks(tasks);
    final hasFinished = tasks.any(_isFinishedSftpTask);
    return _TerminalSftpPanelSurface(
      colors: colors,
      child: Column(
        children: [
          SizedBox(
            height: 38,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      tr('common.label.tasks', fallback: 'Tasks'),
                      style: TextStyle(
                        color: colors.foreground,
                        fontSize: 11.5,
                        fontWeight: NautermFontWeights.semibold,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                  TextButton(
                    onPressed: hasFinished ? onClearCompleted : null,
                    style: TextButton.styleFrom(
                      foregroundColor: colors.muted,
                      disabledForegroundColor: colors.muted.withValues(
                        alpha: 0.35,
                      ),
                      padding: const EdgeInsets.symmetric(horizontal: 6),
                      minimumSize: const Size(0, 26),
                    ),
                    child: Text(
                      tr('common.action.clear', fallback: 'Clear'),
                      style: TextStyle(fontSize: 10.5),
                    ),
                  ),
                ],
              ),
            ),
          ),
          Divider(height: 1, color: colors.border),
          Expanded(
            child: ordered.isEmpty
                ? Center(
                    child: Text(
                      tr('common.label.noTasks', fallback: 'No tasks'),
                      style: TextStyle(
                        color: colors.muted,
                        fontSize: 11,
                        letterSpacing: 0,
                      ),
                    ),
                  )
                : ListView.builder(
                    padding: const EdgeInsets.symmetric(vertical: 4),
                    itemCount: ordered.length,
                    itemBuilder: (context, index) {
                      final task = ordered[index];
                      final cancellable =
                          task.status == _SftpTaskStatus.queued ||
                          task.status == _SftpTaskStatus.running ||
                          task.status == _SftpTaskStatus.paused;
                      final progress = task.totalBytes > 0
                          ? (task.bytes / task.totalBytes)
                                .clamp(0.0, 1.0)
                                .toDouble()
                          : null;
                      return SizedBox(
                        height: 44,
                        child: Stack(
                          children: [
                            Padding(
                              padding: const EdgeInsets.symmetric(vertical: 5),
                              child: Column(
                                mainAxisAlignment: MainAxisAlignment.center,
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Padding(
                                    padding: const EdgeInsets.only(
                                      left: 10,
                                      right: 58,
                                    ),
                                    child: Text.rich(
                                      TextSpan(
                                        children: [
                                          TextSpan(
                                            text: _sftpTaskActionLabel(
                                              task.type,
                                            ),
                                          ),
                                          TextSpan(
                                            text:
                                                ' ${_sftpTaskPrimaryPath(task)}',
                                            style: TextStyle(
                                              decoration:
                                                  task.status ==
                                                          _SftpTaskStatus
                                                              .cancelled ||
                                                      _sftpTaskLocalPathUnavailable(
                                                        task,
                                                      )
                                                  ? TextDecoration.lineThrough
                                                  : TextDecoration.none,
                                            ),
                                          ),
                                        ],
                                      ),
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: TextStyle(
                                        color: colors.foreground,
                                        fontSize: 10.5,
                                        fontWeight: NautermFontWeights.medium,
                                        letterSpacing: 0,
                                      ),
                                    ),
                                  ),
                                  const SizedBox(height: 2),
                                  Padding(
                                    padding: const EdgeInsets.symmetric(
                                      horizontal: 10,
                                    ),
                                    child: Text(
                                      _sftpTaskSubtitle(task),
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: TextStyle(
                                        color:
                                            task.status ==
                                                _SftpTaskStatus.failed
                                            ? const Color(0xffd54b3f)
                                            : colors.muted,
                                        fontSize: 9,
                                        letterSpacing: 0,
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                            ),
                            if (task.status == _SftpTaskStatus.running ||
                                task.status == _SftpTaskStatus.paused)
                              Positioned(
                                left: 10,
                                right: 10,
                                bottom: 2,
                                child: LinearProgressIndicator(
                                  minHeight: 1.5,
                                  value: progress,
                                  color: task.status == _SftpTaskStatus.paused
                                      ? const Color(0xffd18b22)
                                      : colors.accent,
                                  backgroundColor: colors.inputBackground,
                                ),
                              ),
                            if (_isPausableSftpTask(task) &&
                                (task.status == _SftpTaskStatus.queued ||
                                    task.status == _SftpTaskStatus.running ||
                                    task.status == _SftpTaskStatus.paused))
                              Positioned(
                                right: 28,
                                top: 8,
                                child: _TerminalSftpIconButton(
                                  tooltip: task.status == _SftpTaskStatus.paused
                                      ? tr(
                                          'common.action.continue',
                                          fallback: 'Continue',
                                        )
                                      : tr(
                                          'common.action.pause',
                                          fallback: 'Pause',
                                        ),
                                  icon: task.status == _SftpTaskStatus.paused
                                      ? LucideIcons.play
                                      : LucideIcons.circlePause,
                                  colors: colors,
                                  onPressed:
                                      task.pauseRequested ||
                                          task.cancelRequested ||
                                          (task.status !=
                                                  _SftpTaskStatus.queued &&
                                              task.status !=
                                                  _SftpTaskStatus.running &&
                                              task.status !=
                                                  _SftpTaskStatus.paused)
                                      ? null
                                      : () => onPauseToggle(task.id),
                                ),
                              ),
                            Positioned(
                              right: 4,
                              top: 8,
                              child: _TerminalSftpIconButton(
                                tooltip: cancellable ? 'Cancel' : 'Dismiss',
                                icon: LucideIcons.x,
                                colors: colors,
                                onPressed: cancellable && task.cancelRequested
                                    ? null
                                    : () => cancellable
                                          ? onCancel(task.id)
                                          : onDismiss(task.id),
                              ),
                            ),
                          ],
                        ),
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }
}

class _TerminalSftpFavoritesPanel extends StatelessWidget {
  const _TerminalSftpFavoritesPanel({
    required this.colors,
    required this.paths,
    required this.onSelected,
  });

  final _AiAssistantColors colors;
  final List<String> paths;
  final ValueChanged<String> onSelected;

  @override
  Widget build(BuildContext context) {
    return _TerminalSftpPanelSurface(
      colors: colors,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxHeight: 250),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            SizedBox(
              height: 38,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 10),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: Text(
                    tr(
                      'workspace.label.favoritePaths',
                      fallback: 'Favorite paths',
                    ),
                    style: TextStyle(
                      color: colors.foreground,
                      fontSize: 11.5,
                      fontWeight: NautermFontWeights.semibold,
                      letterSpacing: 0,
                    ),
                  ),
                ),
              ),
            ),
            Divider(height: 1, color: colors.border),
            if (paths.isEmpty)
              Padding(
                padding: const EdgeInsets.all(18),
                child: Text(
                  tr(
                    'common.label.noFavoritePaths',
                    fallback: 'No favorite paths',
                  ),
                  style: TextStyle(
                    color: colors.muted,
                    fontSize: 11,
                    letterSpacing: 0,
                  ),
                ),
              )
            else
              Flexible(
                child: ListView.builder(
                  shrinkWrap: true,
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  itemCount: paths.length,
                  itemBuilder: (context, index) {
                    final path = paths[index];
                    return Material(
                      color: Colors.transparent,
                      child: InkWell(
                        onTap: () => onSelected(path),
                        hoverColor: colors.inputBackground,
                        child: SizedBox(
                          height: 34,
                          child: Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 10),
                            child: Row(
                              children: [
                                Icon(
                                  LucideIcons.folder,
                                  size: 14,
                                  color: colors.accent,
                                ),
                                const SizedBox(width: 8),
                                Expanded(
                                  child: Text(
                                    path,
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: TextStyle(
                                      color: colors.foreground,
                                      fontSize: 10.5,
                                      fontFamily: 'monospace',
                                      letterSpacing: 0,
                                    ),
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                      ),
                    );
                  },
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _TerminalSftpPanelSurface extends StatelessWidget {
  const _TerminalSftpPanelSurface({required this.colors, required this.child});

  final _AiAssistantColors colors;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: colors.background,
        borderRadius: BorderRadius.circular(9),
        border: Border.all(color: colors.border),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.12),
            blurRadius: 12,
            spreadRadius: -5,
            offset: const Offset(0, 4),
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(9),
        child: Material(type: MaterialType.transparency, child: child),
      ),
    );
  }
}

class _TerminalSftpConnectionView extends StatelessWidget {
  const _TerminalSftpConnectionView({
    required this.colors,
    required this.state,
    required this.onRetry,
    required this.onTrustOnceAndRetry,
    required this.onTrustAndRetry,
    required this.onChangeHost,
  });

  final _AiAssistantColors colors;
  final _SftpConnectionState state;
  final VoidCallback onRetry;
  final VoidCallback onTrustOnceAndRetry;
  final VoidCallback onTrustAndRetry;
  final VoidCallback onChangeHost;

  @override
  Widget build(BuildContext context) {
    final failed = state.phase == _SftpConnectionPhase.failed;
    final hostKey = state.phase == _SftpConnectionPhase.hostKey;
    return ColoredBox(
      color: colors.background,
      child: Column(
        children: [
          _TerminalToolSectionHeader(
            title: state.host.name,
            subtitle: _sftpConnectionTarget(state.host),
            colors: colors,
          ),
          Divider(height: 1, color: colors.border),
          Expanded(
            child: Center(
              child: SingleChildScrollView(
                padding: const EdgeInsets.all(18),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (!failed && !hostKey)
                      SizedBox(
                        width: 20,
                        height: 20,
                        child: CircularProgressIndicator(
                          strokeWidth: 1.8,
                          color: colors.accent,
                        ),
                      )
                    else
                      Icon(
                        hostKey
                            ? LucideIcons.fingerprint
                            : LucideIcons.circleAlert,
                        size: 22,
                        color: hostKey
                            ? colors.accent
                            : const Color(0xffd54b3f),
                      ),
                    const SizedBox(height: 12),
                    Text(
                      hostKey
                          ? 'Host verification required'
                          : failed
                          ? 'Connection failed'
                          : 'Opening SFTP…',
                      textAlign: TextAlign.center,
                      style: TextStyle(
                        color: colors.foreground,
                        fontSize: 12,
                        fontWeight: NautermFontWeights.semibold,
                        letterSpacing: 0,
                      ),
                    ),
                    if (failed || hostKey) ...[
                      const SizedBox(height: 7),
                      Text(
                        state.message ?? 'Unable to connect.',
                        textAlign: TextAlign.center,
                        style: TextStyle(
                          color: colors.muted,
                          fontSize: 10.5,
                          height: 1.4,
                          letterSpacing: 0,
                        ),
                      ),
                    ],
                    const SizedBox(height: 14),
                    Wrap(
                      alignment: WrapAlignment.center,
                      spacing: 8,
                      runSpacing: 8,
                      children: [
                        if (failed)
                          _TerminalSftpTextButton(
                            label: 'Change host',
                            colors: colors,
                            onPressed: onChangeHost,
                          ),
                        if (failed)
                          _TerminalSftpTextButton(
                            label: 'Retry',
                            colors: colors,
                            primary: true,
                            onPressed: onRetry,
                          ),
                        if (hostKey)
                          _TerminalSftpTextButton(
                            label: 'Once',
                            colors: colors,
                            onPressed: onTrustOnceAndRetry,
                          ),
                        if (hostKey)
                          _TerminalSftpTextButton(
                            label: 'Trust & save',
                            colors: colors,
                            primary: true,
                            onPressed: onTrustAndRetry,
                          ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _TerminalSftpHostSelector extends StatelessWidget {
  const _TerminalSftpHostSelector({
    required this.colors,
    required this.groups,
    required this.hosts,
    required this.tags,
    required this.searchController,
    required this.onBack,
    required this.onHostSelected,
  });

  final _AiAssistantColors colors;
  final List<_GroupItem> groups;
  final List<_HostItem> hosts;
  final List<TagEntry> tags;
  final TextEditingController searchController;
  final VoidCallback onBack;
  final ValueChanged<_HostItem> onHostSelected;

  @override
  Widget build(BuildContext context) {
    final groupNames = {for (final group in groups) group.id: group.name};
    final tagNamesByUuid = _tagNamesByUuid(tags);
    final query = _HostSearchQuery.parse(searchController.text);
    final visibleHosts =
        hosts
            .where(
              (host) =>
                  host.type == NautermHostType.remote.storageValue &&
                  query.matchesHost(
                    host,
                    groupNames[host.groupId],
                    tagNames: _tagNamesForHost(host, tagNamesByUuid),
                  ),
            )
            .toList()
          ..sort(
            (left, right) =>
                left.name.toLowerCase().compareTo(right.name.toLowerCase()),
          );
    return ColoredBox(
      color: colors.background,
      child: Column(
        children: [
          SizedBox(
            height: 42,
            child: Padding(
              padding: const EdgeInsets.fromLTRB(6, 0, 10, 0),
              child: Row(
                children: [
                  _TerminalSftpIconButton(
                    tooltip: tr('common.action.back', fallback: 'Back'),
                    icon: LucideIcons.arrowLeft,
                    colors: colors,
                    onPressed: onBack,
                  ),
                  const SizedBox(width: 4),
                  Text(
                    tr('common.label.selectHost', fallback: 'Select host'),
                    style: TextStyle(
                      color: colors.foreground,
                      fontSize: 12,
                      fontWeight: NautermFontWeights.semibold,
                      letterSpacing: 0,
                    ),
                  ),
                ],
              ),
            ),
          ),
          Divider(height: 1, color: colors.border),
          Padding(
            padding: const EdgeInsets.fromLTRB(9, 8, 9, 7),
            child: Container(
              height: 32,
              decoration: BoxDecoration(
                color: colors.inputBackground,
                borderRadius: BorderRadius.circular(7),
                border: Border.all(color: colors.border),
              ),
              child: Row(
                children: [
                  const SizedBox(width: 8),
                  Icon(LucideIcons.search, size: 14, color: colors.muted),
                  const SizedBox(width: 7),
                  Expanded(
                    child: TextField(
                      controller: searchController,
                      style: TextStyle(
                        color: colors.foreground,
                        fontSize: 11,
                        letterSpacing: 0,
                      ),
                      cursorColor: colors.accent,
                      decoration: InputDecoration.collapsed(
                        hintText: tr(
                          'common.label.searchHosts',
                          fallback: 'Search hosts',
                        ),
                        hintStyle: TextStyle(
                          color: colors.muted,
                          fontSize: 11,
                          letterSpacing: 0,
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
          Expanded(
            child: visibleHosts.isEmpty
                ? Center(
                    child: Text(
                      tr(
                        'workspace.label.noMatchingSshHosts',
                        fallback: 'No matching SSH hosts',
                      ),
                      style: TextStyle(
                        color: colors.muted,
                        fontSize: 11,
                        letterSpacing: 0,
                      ),
                    ),
                  )
                : ListView.builder(
                    padding: const EdgeInsets.fromLTRB(5, 2, 5, 10),
                    itemCount: visibleHosts.length,
                    itemBuilder: (context, index) {
                      final host = visibleHosts[index];
                      return Material(
                        color: Colors.transparent,
                        borderRadius: BorderRadius.circular(6),
                        child: InkWell(
                          borderRadius: BorderRadius.circular(6),
                          hoverColor: colors.inputBackground,
                          onTap: () => onHostSelected(host),
                          child: SizedBox(
                            height: 42,
                            child: Padding(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 8,
                              ),
                              child: Row(
                                children: [
                                  Icon(
                                    LucideIcons.server,
                                    size: 15,
                                    color: host.color,
                                  ),
                                  const SizedBox(width: 8),
                                  Expanded(
                                    child: Column(
                                      mainAxisAlignment:
                                          MainAxisAlignment.center,
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        Text(
                                          host.name,
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis,
                                          style: TextStyle(
                                            color: colors.foreground,
                                            fontSize: 11.5,
                                            fontWeight:
                                                NautermFontWeights.medium,
                                            letterSpacing: 0,
                                          ),
                                        ),
                                        Text(
                                          [
                                            ?groupNames[host.groupId],
                                            ?host.host,
                                          ].join(' · '),
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis,
                                          style: TextStyle(
                                            color: colors.muted,
                                            fontSize: 9,
                                            letterSpacing: 0,
                                          ),
                                        ),
                                      ],
                                    ),
                                  ),
                                ],
                              ),
                            ),
                          ),
                        ),
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }
}

class _TerminalSftpTextButton extends StatelessWidget {
  const _TerminalSftpTextButton({
    required this.label,
    required this.colors,
    required this.onPressed,
    this.primary = false,
  });

  final String label;
  final _AiAssistantColors colors;
  final VoidCallback onPressed;
  final bool primary;

  @override
  Widget build(BuildContext context) {
    return TextButton(
      onPressed: onPressed,
      style: TextButton.styleFrom(
        foregroundColor: primary ? colors.canvasBackground : colors.foreground,
        backgroundColor: primary ? colors.accent : colors.inputBackground,
        overlayColor: primary
            ? colors.canvasBackground.withValues(alpha: 0.10)
            : colors.accent.withValues(alpha: 0.10),
        minimumSize: const Size(0, 30),
        padding: const EdgeInsets.symmetric(horizontal: 11),
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(7),
          side: BorderSide(color: primary ? colors.accent : colors.border),
        ),
      ),
      child: Text(
        label,
        style: const TextStyle(fontSize: 10.5, letterSpacing: 0),
      ),
    );
  }
}

String _terminalSftpSortLabel(_SftpSortColumn column) {
  return switch (column) {
    _SftpSortColumn.name => 'Name',
    _SftpSortColumn.modified => 'Modified',
    _SftpSortColumn.size => 'Size',
    _SftpSortColumn.kind => 'Kind',
  };
}

NautermContextMenuStyle _terminalSftpMenuStyle(_AiAssistantColors colors) {
  return NautermContextMenuStyle(
    background: colors.background,
    foreground: colors.foreground,
    mutedForeground: colors.muted,
    disabledForeground: colors.muted.withValues(alpha: 0.34),
    border: colors.border,
    hoverBackground: colors.inputBackground,
    accent: colors.accent,
    shadows: [
      BoxShadow(
        color: Colors.black.withValues(alpha: 0.16),
        blurRadius: 16,
        spreadRadius: -4,
        offset: const Offset(0, 6),
      ),
    ],
  );
}
