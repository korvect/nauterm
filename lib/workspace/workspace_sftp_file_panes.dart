part of 'nauterm_workspace.dart';

class _SftpPaneDragPayload {
  const _SftpPaneDragPayload({
    required this.sourceSlot,
    required this.sourceRemote,
    required this.entries,
  });

  final _SftpPaneSlot sourceSlot;
  final bool sourceRemote;
  final List<_SftpFileEntry> entries;
}

class _SftpLocalPane extends StatelessWidget {
  const _SftpLocalPane({
    required this.title,
    required this.icon,
    required this.iconColor,
    required this.remote,
    this.onEndpointTap,
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
    required this.hasSelection,
    required this.showCloseAction,
    this.sshEditorAvailable = false,
    this.onSshSelected,
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
    required this.onTaskListToggle,
    required this.onTaskClearCompleted,
    required this.onTaskDismiss,
    required this.onTaskCancel,
    required this.onTaskPauseToggle,
    required this.onPathPanelDismiss,
    required this.onPathFavoriteToggle,
    required this.onFavoriteListToggle,
    required this.onFavoritePathSelected,
    required this.onSelectionChanged,
    required this.onEntryDoubleTap,
    required this.onEntrySecondaryTapDown,
    required this.onBlankSecondaryTapDown,
    required this.onSortChanged,
    required this.onAction,
    required this.createDragPayload,
  });

  final String title;
  final IconData icon;
  final Color iconColor;
  final bool remote;
  final VoidCallback? onEndpointTap;
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
  final bool hasSelection;
  final bool showCloseAction;
  final bool sshEditorAvailable;
  final VoidCallback? onSshSelected;
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
  final VoidCallback onTaskListToggle;
  final VoidCallback onTaskClearCompleted;
  final ValueChanged<int> onTaskDismiss;
  final ValueChanged<int> onTaskCancel;
  final ValueChanged<int> onTaskPauseToggle;
  final VoidCallback onPathPanelDismiss;
  final VoidCallback onPathFavoriteToggle;
  final VoidCallback onFavoriteListToggle;
  final ValueChanged<String> onFavoritePathSelected;
  final ValueChanged<_SftpSelectionChange> onSelectionChanged;
  final ValueChanged<_SftpFileEntry> onEntryDoubleTap;
  final void Function(TapDownDetails details, _SftpFileEntry entry)
  onEntrySecondaryTapDown;
  final GestureTapDownCallback onBlankSecondaryTapDown;
  final ValueChanged<_SftpSortColumn> onSortChanged;
  final ValueChanged<_SftpAction> onAction;
  final _SftpPaneDragPayload Function(List<_SftpFileEntry> entries)
  createDragPayload;

  @override
  Widget build(BuildContext context) {
    final visibleEntries = _sortSftpEntries(
      _filterSftpEntries(entries, filterController.text),
      sortColumn,
      ascending: sortAscending,
    );
    final selectedEntry = entries
        .where((entry) => entry.path == selectedPath)
        .firstOrNull;
    final selectionIsFile =
        selectedEntry != null &&
        !selectedEntry.isDirectory &&
        !selectedEntry.isParent;
    return ColoredBox(
      color: _surface,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final showPathPanels = constraints.maxWidth >= 520;
          final panelWidth = math.min(360.0, constraints.maxWidth - 28);
          final pathPanelOpen =
              remote &&
              showPathPanels &&
              panelWidth > 0 &&
              (taskListOpen || favoriteListOpen);
          return Stack(
            children: [
              Column(
                children: [
                  _SftpLocalToolbar(
                    title: title,
                    icon: icon,
                    iconColor: iconColor,
                    remote: remote,
                    onEndpointTap: onEndpointTap,
                    filterController: filterController,
                    showHiddenFiles: showHiddenFiles,
                    hasSelection: hasSelection,
                    selectionIsFile: selectionIsFile,
                    selectionCanOpenWithSystemDefault:
                        selectedEntry == null ||
                        selectedEntry.isDirectory ||
                        selectedEntry.isParent ||
                        canOpenFileWithSystemDefaultApplication(
                          selectedEntry.name,
                          permissions: selectedEntry.permissions,
                        ),
                    showCloseAction: showCloseAction,
                    sshEditorAvailable: sshEditorAvailable,
                    onSshSelected: onSshSelected,
                    onAction: onAction,
                  ),
                  _SftpPathBar(
                    remote: remote,
                    path: path,
                    controller: pathController,
                    focusNode: pathFocusNode,
                    editing: editingPath,
                    canGoBack: canGoBack,
                    canGoForward: canGoForward,
                    onHome: onHome,
                    onBack: onBack,
                    onForward: onForward,
                    onEditRequested: onPathEditRequested,
                    onSubmitted: onPathSubmitted,
                    onCancelled: onPathEditCancelled,
                    onPathSelected: onPathSubmitted,
                    tasks: tasks,
                    taskListOpen: taskListOpen,
                    favoriteListOpen: favoriteListOpen,
                    favoritePaths: favoritePaths,
                    pathFavorited: favoritePaths.contains(path),
                    onTaskListToggle: onTaskListToggle,
                    onPathFavoriteToggle: onPathFavoriteToggle,
                    onFavoriteListToggle: onFavoriteListToggle,
                  ),
                  _SftpTableHeader(
                    sortColumn: sortColumn,
                    sortAscending: sortAscending,
                    onSortChanged: onSortChanged,
                  ),
                  Expanded(
                    child: loading
                        ? Center(
                            child: SizedBox(
                              width: 18,
                              height: 18,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            ),
                          )
                        : loadError != null
                        ? _SftpLoadError(message: '$loadError')
                        : _SftpFileTable(
                            entries: visibleEntries,
                            selectedPath: selectedPath,
                            selectedPaths: selectedPaths,
                            selectionAnchorPath: selectionAnchorPath,
                            onSelectionChanged: onSelectionChanged,
                            onEntryDoubleTap: onEntryDoubleTap,
                            onEntrySecondaryTapDown: onEntrySecondaryTapDown,
                            onBlankSecondaryTapDown: onBlankSecondaryTapDown,
                            createDragPayload: createDragPayload,
                          ),
                  ),
                ],
              ),
              if (pathPanelOpen)
                Positioned.fill(
                  child: GestureDetector(
                    behavior: HitTestBehavior.translucent,
                    onTap: onPathPanelDismiss,
                    child: const SizedBox.expand(),
                  ),
                ),
              if (remote && showPathPanels && taskListOpen && panelWidth > 0)
                Positioned(
                  top: 100,
                  right: 14,
                  width: panelWidth,
                  child: _SftpTaskBar(
                    tasks: tasks,
                    onClearCompleted: onTaskClearCompleted,
                    onDismissTask: onTaskDismiss,
                    onCancelTask: onTaskCancel,
                    onPauseToggle: onTaskPauseToggle,
                  ),
                ),
              if (remote &&
                  showPathPanels &&
                  favoriteListOpen &&
                  panelWidth > 0)
                Positioned(
                  top: 100,
                  right: 14,
                  width: panelWidth,
                  child: _SftpFavoritePathsPanel(
                    favoritePaths: favoritePaths,
                    onSelected: onFavoritePathSelected,
                  ),
                ),
            ],
          );
        },
      ),
    );
  }
}

class _SftpLocalToolbar extends StatelessWidget {
  const _SftpLocalToolbar({
    required this.title,
    required this.icon,
    required this.iconColor,
    required this.remote,
    this.onEndpointTap,
    required this.filterController,
    required this.showHiddenFiles,
    required this.hasSelection,
    required this.selectionIsFile,
    required this.selectionCanOpenWithSystemDefault,
    required this.showCloseAction,
    required this.sshEditorAvailable,
    this.onSshSelected,
    required this.onAction,
  });

  final String title;
  final IconData icon;
  final Color iconColor;
  final bool remote;
  final VoidCallback? onEndpointTap;
  final TextEditingController filterController;
  final bool showHiddenFiles;
  final bool hasSelection;
  final bool selectionIsFile;
  final bool selectionCanOpenWithSystemDefault;
  final bool showCloseAction;
  final bool sshEditorAvailable;
  final VoidCallback? onSshSelected;
  final ValueChanged<_SftpAction> onAction;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 54,
      color: _card,
      padding: const EdgeInsets.symmetric(horizontal: 14),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Expanded(
            child: _SftpPaneTitleButton(
              title: title,
              icon: icon,
              iconColor: iconColor,
              onTap: onEndpointTap,
            ),
          ),
          _SftpToolbarFilter(controller: filterController),
          if (onSshSelected != null) ...[
            SizedBox(width: 10),
            _SftpToolbarTextButton(
              icon: LucideIcons.squareTerminal,
              label: 'SSH',
              onTap: onSshSelected,
            ),
          ],
          SizedBox(width: 14),
          _SftpActionsMenuButton(
            hasSelection: hasSelection,
            selectionIsFile: selectionIsFile,
            selectionCanOpenWithSystemDefault:
                selectionCanOpenWithSystemDefault,
            showHiddenFiles: showHiddenFiles,
            remote: remote,
            showCloseAction: showCloseAction,
            sshEditorAvailable: sshEditorAvailable,
            onAction: onAction,
          ),
        ],
      ),
    );
  }
}

class _SftpActionsMenuButton extends StatefulWidget {
  const _SftpActionsMenuButton({
    required this.hasSelection,
    required this.selectionIsFile,
    required this.selectionCanOpenWithSystemDefault,
    required this.showHiddenFiles,
    required this.remote,
    required this.showCloseAction,
    required this.sshEditorAvailable,
    required this.onAction,
  });

  final bool hasSelection;
  final bool selectionIsFile;
  final bool selectionCanOpenWithSystemDefault;
  final bool showHiddenFiles;
  final bool remote;
  final bool showCloseAction;
  final bool sshEditorAvailable;
  final ValueChanged<_SftpAction> onAction;

  @override
  State<_SftpActionsMenuButton> createState() => _SftpActionsMenuButtonState();
}

class _SftpActionsMenuButtonState extends State<_SftpActionsMenuButton> {
  final GlobalKey _buttonKey = GlobalKey();

  @override
  Widget build(BuildContext context) {
    return _SftpToolbarTextButton(
      key: _buttonKey,
      icon: null,
      label: tr('common.label.actions', fallback: 'Actions'),
      trailing: Icons.keyboard_arrow_down_rounded,
      onTap: _showActionsMenu,
    );
  }

  Future<void> _showActionsMenu() async {
    final buttonContext = _buttonKey.currentContext;
    if (buttonContext == null) {
      return;
    }
    final buttonBox = buttonContext.findRenderObject();
    final overlayBox = Overlay.of(context).context.findRenderObject();
    if (buttonBox is! RenderBox || overlayBox is! RenderBox) {
      return;
    }

    final buttonRect =
        buttonBox.localToGlobal(Offset.zero, ancestor: overlayBox) &
        buttonBox.size;
    final action = await showNautermDropdownMenu<_SftpAction>(
      context: context,
      anchor: buttonRect,
      width: math.max(220, buttonRect.width),
      entries: _sftpContextMenuEntries(
        _sftpActionMenuEntries(
          hasSelection: widget.hasSelection,
          selectionIsFile: widget.selectionIsFile,
          selectionCanOpenWithSystemDefault:
              widget.selectionCanOpenWithSystemDefault,
          showHiddenFiles: widget.showHiddenFiles,
          remote: widget.remote,
          showCloseAction: widget.showCloseAction,
          sshEditorAvailable: widget.sshEditorAvailable,
        ),
      ),
      maxHeight: double.infinity,
    );
    if (action != null) {
      await WidgetsBinding.instance.endOfFrame;
    }
    if (action != null && mounted) {
      widget.onAction(action);
    }
  }
}

class _SftpPaneTitleButton extends StatelessWidget {
  const _SftpPaneTitleButton({
    required this.title,
    required this.icon,
    required this.iconColor,
    required this.onTap,
  });

  final String title;
  final IconData icon;
  final Color iconColor;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final content = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 24,
          height: 24,
          decoration: BoxDecoration(
            color: iconColor,
            borderRadius: BorderRadius.circular(6),
          ),
          child: Icon(icon, color: Colors.white, size: 14),
        ),
        SizedBox(width: 8),
        Flexible(
          child: Text(
            title,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: _text,
              fontSize: NautermFontSizes.labelLarge,
              fontWeight: NautermFontWeights.semibold,
              letterSpacing: 0,
            ),
          ),
        ),
        if (onTap != null) ...[
          SizedBox(width: 3),
          Icon(Icons.keyboard_arrow_down_rounded, size: 18, color: _text),
        ],
      ],
    );

    if (onTap == null) {
      return Align(alignment: Alignment.centerLeft, child: content);
    }

    return Align(
      alignment: Alignment.centerLeft,
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(7),
        child: InkWell(
          borderRadius: BorderRadius.circular(7),
          hoverColor: _workspaceDark ? _sidebarHover : null,
          splashColor: _workspaceDark ? _workspaceMenuPressed : null,
          highlightColor: _workspaceDark ? _workspaceMenuPressed : null,
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 5),
            child: content,
          ),
        ),
      ),
    );
  }
}

class _SftpToolbarTextButton extends StatelessWidget {
  const _SftpToolbarTextButton({
    super.key,
    required this.icon,
    required this.label,
    required this.onTap,
    this.trailing,
  });

  final IconData? icon;
  final String label;
  final VoidCallback? onTap;
  final IconData? trailing;

  @override
  Widget build(BuildContext context) {
    final content = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (icon != null) ...[
          Icon(icon, size: 17, color: _text),
          if (label.isNotEmpty) SizedBox(width: 6),
        ],
        if (label.isNotEmpty)
          Text(
            label,
            style: TextStyle(
              color: _text,
              fontSize: NautermFontSizes.labelLarge,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
        if (trailing != null) ...[
          SizedBox(width: 3),
          Icon(trailing, size: 18, color: _text),
        ],
      ],
    );

    if (onTap == null) {
      return content;
    }

    return Material(
      color: Colors.transparent,
      borderRadius: BorderRadius.circular(6),
      child: InkWell(
        borderRadius: BorderRadius.circular(6),
        hoverColor: _workspaceDark ? _sidebarHover : null,
        splashColor: _workspaceDark ? _workspaceMenuPressed : null,
        highlightColor: _workspaceDark ? _workspaceMenuPressed : null,
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 5),
          child: content,
        ),
      ),
    );
  }
}

class _SftpToolbarFilter extends StatefulWidget {
  const _SftpToolbarFilter({required this.controller});

  final TextEditingController controller;

  @override
  State<_SftpToolbarFilter> createState() => _SftpToolbarFilterState();
}

class _SftpToolbarFilterState extends State<_SftpToolbarFilter> {
  final FocusNode _focusNode = FocusNode();
  bool _editing = false;

  bool get _showInput => _editing || widget.controller.text.trim().isNotEmpty;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_handleControllerChanged);
    _focusNode.addListener(_handleFocusChanged);
  }

  @override
  void didUpdateWidget(covariant _SftpToolbarFilter oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller == widget.controller) {
      return;
    }
    oldWidget.controller.removeListener(_handleControllerChanged);
    widget.controller.addListener(_handleControllerChanged);
  }

  @override
  void dispose() {
    widget.controller.removeListener(_handleControllerChanged);
    _focusNode.removeListener(_handleFocusChanged);
    _focusNode.dispose();
    super.dispose();
  }

  void _handleControllerChanged() {
    if (mounted) {
      setState(() {});
    }
  }

  void _handleFocusChanged() {
    if (_focusNode.hasFocus || widget.controller.text.trim().isNotEmpty) {
      return;
    }
    setState(() {
      _editing = false;
    });
  }

  void _beginEditing() {
    setState(() {
      _editing = true;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        _focusNode.requestFocus();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    if (!_showInput) {
      return _SftpToolbarTextButton(
        icon: Icons.search_rounded,
        label: tr('common.label.filter', fallback: 'Filter'),
        onTap: _beginEditing,
      );
    }

    return SizedBox(
      width: 150,
      height: 31,
      child: TextField(
        controller: widget.controller,
        focusNode: _focusNode,
        autofocus: true,
        textInputAction: TextInputAction.search,
        style: TextStyle(
          color: _text,
          fontSize: NautermFontSizes.labelMedium,
          fontWeight: NautermFontWeights.medium,
          letterSpacing: 0,
        ),
        decoration: InputDecoration(
          isDense: true,
          hintText: tr('common.label.filter', fallback: 'Filter'),
          hintStyle: TextStyle(
            color: _mutedText,
            fontSize: NautermFontSizes.labelMedium,
            fontWeight: NautermFontWeights.medium,
            letterSpacing: 0,
          ),
          prefixIcon: Icon(Icons.search_rounded, size: 17, color: _text),
          prefixIconConstraints: const BoxConstraints(
            minWidth: 30,
            minHeight: 30,
          ),
          suffixIcon: widget.controller.text.isEmpty
              ? null
              : IconButton(
                  icon: Icon(Icons.close_rounded, size: 16),
                  color: _mutedText,
                  padding: EdgeInsets.zero,
                  style: _workspaceIconButtonInteractionStyle,
                  constraints: const BoxConstraints(
                    minWidth: 28,
                    minHeight: 28,
                  ),
                  onPressed: widget.controller.clear,
                ),
          filled: true,
          fillColor: _sidebarHover,
          contentPadding: const EdgeInsets.symmetric(
            horizontal: 8,
            vertical: 8,
          ),
          border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(7),
            borderSide: BorderSide(color: _sidebarDivider),
          ),
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(7),
            borderSide: BorderSide(color: _sidebarDivider),
          ),
          focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(7),
            borderSide: BorderSide(color: _blue.withValues(alpha: 0.52)),
          ),
        ),
      ),
    );
  }
}

class _SftpPathBar extends StatelessWidget {
  const _SftpPathBar({
    required this.remote,
    required this.path,
    required this.controller,
    required this.focusNode,
    required this.editing,
    required this.canGoBack,
    required this.canGoForward,
    required this.onHome,
    required this.onBack,
    required this.onForward,
    required this.onEditRequested,
    required this.onSubmitted,
    required this.onCancelled,
    required this.onPathSelected,
    required this.tasks,
    required this.taskListOpen,
    required this.favoriteListOpen,
    required this.favoritePaths,
    required this.pathFavorited,
    required this.onTaskListToggle,
    required this.onPathFavoriteToggle,
    required this.onFavoriteListToggle,
  });

  final bool remote;
  final String path;
  final TextEditingController controller;
  final FocusNode focusNode;
  final bool editing;
  final bool canGoBack;
  final bool canGoForward;
  final VoidCallback onHome;
  final VoidCallback onBack;
  final VoidCallback onForward;
  final VoidCallback onEditRequested;
  final ValueChanged<String> onSubmitted;
  final VoidCallback onCancelled;
  final ValueChanged<String> onPathSelected;
  final List<_SftpTask> tasks;
  final bool taskListOpen;
  final bool favoriteListOpen;
  final List<String> favoritePaths;
  final bool pathFavorited;
  final VoidCallback onTaskListToggle;
  final VoidCallback onPathFavoriteToggle;
  final VoidCallback onFavoriteListToggle;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final showPathActions = !editing && constraints.maxWidth >= 520;
        return Container(
          height: 46,
          color: _card,
          padding: const EdgeInsets.fromLTRB(14, 0, 14, 8),
          child: Row(
            children: [
              _SftpPathNavButton(
                tooltip: tr('common.label.home', fallback: 'Home'),
                icon: LucideIcons.house,
                enabled: true,
                onTap: onHome,
              ),
              SizedBox(width: 10),
              _SftpPathNavButton(
                tooltip: tr('common.action.back', fallback: 'Back'),
                icon: LucideIcons.chevronLeft,
                enabled: canGoBack,
                onTap: onBack,
              ),
              SizedBox(width: 10),
              _SftpPathNavButton(
                tooltip: tr('common.label.forward', fallback: 'Forward'),
                icon: LucideIcons.chevronRight,
                enabled: canGoForward,
                onTap: onForward,
              ),
              SizedBox(width: 14),
              Expanded(
                child: editing
                    ? _SftpPathInput(
                        controller: controller,
                        focusNode: focusNode,
                        onSubmitted: onSubmitted,
                        onCancelled: onCancelled,
                      )
                    : _SftpBreadcrumbs(
                        path: path,
                        remote: remote,
                        onEditRequested: onEditRequested,
                        onPathSelected: onPathSelected,
                      ),
              ),
              if (showPathActions && remote) ...[
                SizedBox(width: 8),
                _SftpTaskListButton(
                  tasks: tasks,
                  open: taskListOpen,
                  onTap: onTaskListToggle,
                ),
                SizedBox(width: 6),
                _SftpPathIconButton(
                  icon: pathFavorited
                      ? Icons.star_rounded
                      : Icons.star_border_rounded,
                  tooltip: pathFavorited
                      ? tr(
                          'sftp.tooltip.removePathFavorite',
                          fallback: 'Remove path favorite',
                        )
                      : tr(
                          'sftp.tooltip.favoriteCurrentPath',
                          fallback: 'Favorite current path',
                        ),
                  selected: pathFavorited,
                  onTap: onPathFavoriteToggle,
                ),
                SizedBox(width: 6),
                _SftpFavoritePathsButton(
                  favoritePaths: favoritePaths,
                  open: favoriteListOpen,
                  onTap: onFavoriteListToggle,
                ),
              ],
            ],
          ),
        );
      },
    );
  }
}

class _SftpPathIconButton extends StatelessWidget {
  const _SftpPathIconButton({
    required this.icon,
    required this.tooltip,
    required this.onTap,
    this.selected = false,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback? onTap;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tr(tooltip),
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(7),
        child: InkWell(
          borderRadius: BorderRadius.circular(7),
          onTap: onTap,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: onTap == null
                  ? _surface
                  : selected
                  ? const Color(0xffffd56b).withValues(alpha: 0.16)
                  : _sidebarHover,
              borderRadius: BorderRadius.circular(7),
              border: Border.all(
                color: onTap == null
                    ? _sidebarDivider
                    : selected
                    ? const Color(0xffffd56b)
                    : _sidebarDivider,
              ),
            ),
            child: SizedBox(
              width: 30,
              height: 30,
              child: Icon(
                icon,
                size: 17,
                color: onTap == null
                    ? const Color(0xffb7c6cc)
                    : selected
                    ? const Color(0xffb7791f)
                    : _text,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _SftpFavoritePathsButton extends StatelessWidget {
  const _SftpFavoritePathsButton({
    required this.favoritePaths,
    required this.open,
    required this.onTap,
  });

  final List<String> favoritePaths;
  final bool open;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tr(
        open ? 'sftp.label.hideFavoritePaths' : 'sftp.label.showFavoritePaths',
        fallback: open ? 'Hide favorite paths' : 'Show favorite paths',
      ),
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(7),
        child: InkWell(
          borderRadius: BorderRadius.circular(7),
          onTap: onTap,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: open
                  ? _blue.withValues(alpha: _workspaceDark ? 0.18 : 0.10)
                  : _sidebarHover,
              borderRadius: BorderRadius.circular(7),
              border: Border.all(
                color: open ? _blue.withValues(alpha: 0.42) : _sidebarDivider,
              ),
            ),
            child: SizedBox(
              width: 30,
              height: 30,
              child: Stack(
                clipBehavior: Clip.none,
                children: [
                  Center(
                    child: Icon(
                      open ? Icons.bookmarks_rounded : Icons.bookmarks_outlined,
                      size: 17,
                      color: open ? _blue : _text,
                    ),
                  ),
                  if (favoritePaths.isNotEmpty)
                    Positioned(
                      right: -4,
                      top: -4,
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          color: const Color(0xff7a8f98),
                          borderRadius: BorderRadius.circular(999),
                          border: Border.all(color: _card, width: 1.5),
                        ),
                        child: ConstrainedBox(
                          constraints: const BoxConstraints(
                            minWidth: 17,
                            minHeight: 17,
                          ),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 4),
                            child: Center(
                              child: Text(
                                favoritePaths.length > 99
                                    ? '99+'
                                    : '${favoritePaths.length}',
                                style: TextStyle(
                                  color: Colors.white,
                                  fontSize: 9,
                                  fontWeight: NautermFontWeights.semibold,
                                  letterSpacing: 0,
                                ),
                              ),
                            ),
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
    );
  }
}

class _SftpFavoritePathsPanel extends StatelessWidget {
  const _SftpFavoritePathsPanel({
    required this.favoritePaths,
    required this.onSelected,
  });

  final List<String> favoritePaths;
  final ValueChanged<String> onSelected;

  @override
  Widget build(BuildContext context) {
    return _SftpPanelSurface(
      height: 246,
      child: Column(
        children: [
          Padding(
            padding: EdgeInsets.fromLTRB(13, 13, 12, 7),
            child: Align(
              alignment: Alignment.centerLeft,
              child: Text(
                tr('common.label.favorites', fallback: 'Favorites'),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: _text,
                  fontSize: NautermFontSizes.labelLarge,
                  fontWeight: NautermFontWeights.semibold,
                  letterSpacing: 0,
                ),
              ),
            ),
          ),
          Expanded(
            child: favoritePaths.isEmpty
                ? const _SftpFavoritePathsEmptyState()
                : ClipRRect(
                    borderRadius: const BorderRadius.vertical(
                      bottom: Radius.circular(8),
                    ),
                    child: ListView.separated(
                      padding: EdgeInsets.zero,
                      itemCount: favoritePaths.length,
                      separatorBuilder: (_, _) => Divider(
                        height: 1,
                        thickness: 1,
                        indent: 13,
                        endIndent: 13,
                        color: _workspaceMenuBorder,
                      ),
                      itemBuilder: (context, index) {
                        final path = favoritePaths[index];
                        return _SftpFavoritePathRow(
                          path: path,
                          onTap: () => onSelected(path),
                        );
                      },
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}

class _SftpFavoritePathsEmptyState extends StatelessWidget {
  const _SftpFavoritePathsEmptyState();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Text(
        tr('common.label.noFavoritePaths', fallback: 'No favorite paths'),
        style: TextStyle(
          color: _mutedText,
          fontSize: NautermFontSizes.labelMedium,
          fontWeight: NautermFontWeights.medium,
          letterSpacing: 0,
        ),
      ),
    );
  }
}

class _SftpFavoritePathRow extends StatelessWidget {
  const _SftpFavoritePathRow({required this.path, required this.onTap});

  final String path;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: onTap,
        child: SizedBox(
          height: 45,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(19, 0, 18, 0),
            child: Row(
              children: [
                Icon(Icons.folder_rounded, size: 17, color: Color(0xff5ec4f2)),
                SizedBox(width: 8),
                Expanded(
                  child: Text(
                    path,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: _text,
                      fontSize: NautermFontSizes.labelSmall,
                      fontWeight: NautermFontWeights.semibold,
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
  }
}

class _SftpPathNavButton extends StatelessWidget {
  const _SftpPathNavButton({
    required this.tooltip,
    required this.icon,
    required this.enabled,
    required this.onTap,
  });

  final String tooltip;
  final IconData icon;
  final bool enabled;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: InkResponse(
        radius: 15,
        onTap: enabled ? onTap : null,
        child: Icon(
          icon,
          size: 18,
          color: enabled ? _text : const Color(0xffb9c9ce),
        ),
      ),
    );
  }
}

class _SftpPathInput extends StatelessWidget {
  const _SftpPathInput({
    required this.controller,
    required this.focusNode,
    required this.onSubmitted,
    required this.onCancelled,
  });

  final TextEditingController controller;
  final FocusNode focusNode;
  final ValueChanged<String> onSubmitted;
  final VoidCallback onCancelled;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 32,
      child: TextField(
        controller: controller,
        focusNode: focusNode,
        autofocus: true,
        onSubmitted: onSubmitted,
        onTapOutside: (_) => onCancelled(),
        style: TextStyle(
          color: _text,
          fontSize: NautermFontSizes.labelLarge,
          fontWeight: NautermFontWeights.medium,
          letterSpacing: 0,
        ),
        decoration: InputDecoration(
          isDense: true,
          filled: true,
          fillColor: _sidebarHover,
          contentPadding: const EdgeInsets.symmetric(
            horizontal: 10,
            vertical: 8,
          ),
          border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(5),
            borderSide: BorderSide(color: _sidebarDivider),
          ),
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(5),
            borderSide: BorderSide(color: _sidebarDivider),
          ),
          focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(5),
            borderSide: BorderSide(color: _blue.withValues(alpha: 0.52)),
          ),
        ),
      ),
    );
  }
}

class _SftpBreadcrumbs extends StatelessWidget {
  const _SftpBreadcrumbs({
    required this.path,
    required this.remote,
    required this.onEditRequested,
    required this.onPathSelected,
  });

  final String path;
  final bool remote;
  final VoidCallback onEditRequested;
  final ValueChanged<String> onPathSelected;

  @override
  Widget build(BuildContext context) {
    final parts = _sftpBreadcrumbParts(path, remote: remote);
    final labelStyle = TextStyle(
      color: _text,
      fontSize: NautermFontSizes.labelMedium,
      fontWeight: NautermFontWeights.medium,
      letterSpacing: 0,
    );
    return Material(
      color: Colors.transparent,
      child: LayoutBuilder(
        builder: (context, constraints) {
          const editReserveWidth = 72.0;
          final visibleParts = _visibleSftpBreadcrumbParts(
            parts,
            math.max(0, constraints.maxWidth - editReserveWidth),
            labelStyle,
            Directionality.of(context),
          );
          final hiddenParts = visibleParts.firstOrNull?.isOverflow == true
              ? parts
                    .take(parts.length - visibleParts.length + 1)
                    .toList(growable: false)
              : const <_SftpBreadcrumbPart>[];
          final overflowTarget = hiddenParts.lastOrNull?.path;
          return SizedBox(
            height: 32,
            child: InkWell(
              borderRadius: BorderRadius.circular(6),
              hoverColor: Colors.transparent,
              highlightColor: Colors.transparent,
              splashColor: Colors.transparent,
              mouseCursor: SystemMouseCursors.text,
              onTap: onEditRequested,
              child: Row(
                children: [
                  for (var i = 0; i < visibleParts.length; i++) ...[
                    if (i > 0)
                      Icon(Icons.chevron_right_rounded, size: 15, color: _text),
                    if (visibleParts[i].isOverflow)
                      _SftpBreadcrumbOverflow(
                        onTap: overflowTarget == null
                            ? null
                            : () => onPathSelected(overflowTarget),
                      )
                    else
                      Flexible(
                        fit: FlexFit.loose,
                        child: _SftpBreadcrumbSegment(
                          part: visibleParts[i],
                          labelStyle: labelStyle,
                          onPathSelected: onPathSelected,
                        ),
                      ),
                  ],
                  Expanded(child: SizedBox.expand()),
                ],
              ),
            ),
          );
        },
      ),
    );
  }
}

class _SftpBreadcrumbSegment extends StatelessWidget {
  const _SftpBreadcrumbSegment({
    required this.part,
    required this.labelStyle,
    required this.onPathSelected,
  });

  final _SftpBreadcrumbPart part;
  final TextStyle labelStyle;
  final ValueChanged<String> onPathSelected;

  @override
  Widget build(BuildContext context) {
    return Builder(
      builder: (anchorContext) => InkWell(
        key: part.isDrive
            ? ValueKey('sftp-local-drive-selector:${part.path}')
            : null,
        borderRadius: BorderRadius.circular(6),
        hoverColor: _workspaceDark ? _sidebarHover : const Color(0xffe7eef1),
        highlightColor: Colors.transparent,
        splashColor: Colors.transparent,
        mouseCursor: SystemMouseCursors.click,
        onTap: part.isDrive
            ? () => _showDriveMenu(anchorContext)
            : () => onPathSelected(part.path),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 2, vertical: 5),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.folder_rounded, size: 16, color: Color(0xff5ec4f2)),
              SizedBox(width: 3),
              Flexible(
                child: Text(
                  part.label,
                  maxLines: 1,
                  softWrap: false,
                  overflow: TextOverflow.ellipsis,
                  style: labelStyle,
                ),
              ),
              if (part.isDrive) ...[
                SizedBox(width: 2),
                Icon(
                  Icons.keyboard_arrow_down_rounded,
                  size: 14,
                  color: _mutedText,
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _showDriveMenu(BuildContext context) async {
    final renderObject = context.findRenderObject();
    final overlay = Overlay.of(context).context.findRenderObject();
    if (renderObject is! RenderBox || overlay is! RenderBox) {
      return;
    }
    final anchor =
        renderObject.localToGlobal(Offset.zero, ancestor: overlay) &
        renderObject.size;
    final drives = _availableLocalSftpDriveRoots();
    if (drives.isEmpty) {
      return;
    }
    final selected = await showNautermDropdownMenu<String>(
      context: context,
      anchor: anchor,
      width: math.max(180, anchor.width),
      entries: [
        for (final drive in drives)
          NautermContextMenuAction<String>(
            value: drive,
            label: drive,
            icon: Icons.storage_rounded,
            selected: drive.toLowerCase() == part.path.toLowerCase(),
          ),
      ],
    );
    if (selected != null) {
      onPathSelected(selected);
    }
  }
}

class _SftpBreadcrumbOverflow extends StatelessWidget {
  const _SftpBreadcrumbOverflow({required this.onTap});

  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      borderRadius: BorderRadius.circular(6),
      hoverColor: _workspaceDark ? _sidebarHover : const Color(0xffe7eef1),
      highlightColor: Colors.transparent,
      splashColor: Colors.transparent,
      mouseCursor: onTap == null
          ? SystemMouseCursors.basic
          : SystemMouseCursors.click,
      onTap: onTap,
      child: Padding(
        padding: EdgeInsets.symmetric(horizontal: 2, vertical: 5),
        child: Text(
          tr('...'),
          maxLines: 1,
          softWrap: false,
          overflow: TextOverflow.clip,
          style: TextStyle(
            color: _mutedText,
            fontSize: NautermFontSizes.labelMedium,
            fontWeight: NautermFontWeights.medium,
            letterSpacing: 0,
          ),
        ),
      ),
    );
  }
}

class _SftpTableHeader extends StatelessWidget {
  const _SftpTableHeader({
    required this.sortColumn,
    required this.sortAscending,
    required this.onSortChanged,
  });

  final _SftpSortColumn sortColumn;
  final bool sortAscending;
  final ValueChanged<_SftpSortColumn> onSortChanged;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 45,
      decoration: BoxDecoration(
        color: _surface,
        border: Border(
          top: BorderSide(color: _sidebarDivider),
          bottom: BorderSide(color: _sidebarDivider),
        ),
      ),
      child: Row(
        children: [
          _SftpHeaderCell(
            label: tr('common.label.name', fallback: 'Name'),
            flex: 8,
            column: _SftpSortColumn.name,
            active: sortColumn == _SftpSortColumn.name,
            ascending: sortAscending,
            onTap: onSortChanged,
          ),
          _SftpHeaderCell(
            label: tr('sftp.label.dateModified', fallback: 'Date Modified'),
            flex: 5,
            column: _SftpSortColumn.modified,
            active: sortColumn == _SftpSortColumn.modified,
            ascending: sortAscending,
            onTap: onSortChanged,
          ),
          _SftpHeaderCell(
            label: tr('common.label.size', fallback: 'Size'),
            flex: 3,
            column: _SftpSortColumn.size,
            active: sortColumn == _SftpSortColumn.size,
            ascending: sortAscending,
            onTap: onSortChanged,
          ),
          _SftpHeaderCell(
            label: tr('common.label.kind', fallback: 'Kind'),
            flex: 4,
            column: _SftpSortColumn.kind,
            active: sortColumn == _SftpSortColumn.kind,
            ascending: sortAscending,
            onTap: onSortChanged,
            borderRight: false,
          ),
        ],
      ),
    );
  }
}

class _SftpHeaderCell extends StatelessWidget {
  const _SftpHeaderCell({
    required this.label,
    required this.flex,
    required this.column,
    required this.active,
    required this.ascending,
    required this.onTap,
    this.borderRight = true,
  });

  final String label;
  final int flex;
  final _SftpSortColumn column;
  final bool active;
  final bool ascending;
  final ValueChanged<_SftpSortColumn> onTap;
  final bool borderRight;

  @override
  Widget build(BuildContext context) {
    return Expanded(
      flex: flex,
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          onTap: () => onTap(column),
          child: Container(
            height: double.infinity,
            padding: const EdgeInsets.symmetric(horizontal: 12),
            decoration: BoxDecoration(
              border: borderRight
                  ? Border(right: BorderSide(color: _sidebarDivider))
                  : null,
            ),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    label,
                    maxLines: 1,
                    softWrap: false,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: _text,
                      fontSize: NautermFontSizes.labelMedium,
                      fontWeight: NautermFontWeights.semibold,
                      letterSpacing: 0,
                    ),
                  ),
                ),
                if (active)
                  Icon(
                    ascending
                        ? Icons.keyboard_arrow_up_rounded
                        : Icons.keyboard_arrow_down_rounded,
                    color: _text,
                    size: 18,
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _SftpSelectionChange {
  const _SftpSelectionChange({
    required this.selectedPaths,
    required this.primaryPath,
    required this.anchorPath,
  });

  final Set<String> selectedPaths;
  final String? primaryPath;
  final String? anchorPath;
}

class _SftpFileTable extends StatefulWidget {
  const _SftpFileTable({
    required this.entries,
    required this.selectedPath,
    required this.selectedPaths,
    required this.selectionAnchorPath,
    required this.onSelectionChanged,
    required this.onEntryDoubleTap,
    required this.onEntrySecondaryTapDown,
    required this.onBlankSecondaryTapDown,
    required this.createDragPayload,
  });

  final List<_SftpFileEntry> entries;
  final String? selectedPath;
  final Set<String> selectedPaths;
  final String? selectionAnchorPath;
  final ValueChanged<_SftpSelectionChange> onSelectionChanged;
  final ValueChanged<_SftpFileEntry> onEntryDoubleTap;
  final void Function(TapDownDetails details, _SftpFileEntry entry)
  onEntrySecondaryTapDown;
  final GestureTapDownCallback onBlankSecondaryTapDown;
  final _SftpPaneDragPayload Function(List<_SftpFileEntry> entries)
  createDragPayload;

  @override
  State<_SftpFileTable> createState() => _SftpFileTableState();
}

class _SftpFileTableState extends State<_SftpFileTable> {
  late Set<String> _selectedPaths = _selectionSetFromWidget(widget);
  late String? _primaryPath = widget.selectedPath;
  late String? _anchorPath = widget.selectionAnchorPath;

  @override
  void didUpdateWidget(covariant _SftpFileTable oldWidget) {
    super.didUpdateWidget(oldWidget);
    final nextSelectedPaths = _selectionSetFromWidget(widget);
    if (!_setEquals(nextSelectedPaths, _selectedPaths) ||
        widget.selectedPath != _primaryPath ||
        widget.selectionAnchorPath != _anchorPath) {
      _selectedPaths = nextSelectedPaths;
      _primaryPath = widget.selectedPath;
      _anchorPath = widget.selectionAnchorPath;
    }
  }

  static Set<String> _selectionSetFromWidget(_SftpFileTable widget) {
    if (widget.selectedPaths.isNotEmpty) {
      return Set<String>.of(widget.selectedPaths);
    }
    final selectedPath = widget.selectedPath;
    return selectedPath == null ? <String>{} : <String>{selectedPath};
  }

  bool _setEquals(Set<String> left, Set<String> right) {
    if (left.length != right.length) {
      return false;
    }
    for (final value in left) {
      if (!right.contains(value)) {
        return false;
      }
    }
    return true;
  }

  void _selectEntry(_SftpFileEntry entry) {
    if (entry.isParent) {
      _applySelection({entry.path}, primaryPath: entry.path, anchorPath: null);
      return;
    }
    final keyboard = HardwareKeyboard.instance;
    final additive = keyboard.isMetaPressed || keyboard.isControlPressed;
    final range = keyboard.isShiftPressed;
    final selectableEntries = [
      for (final visibleEntry in widget.entries)
        if (!visibleEntry.isParent) visibleEntry,
    ];
    var nextSelectedPaths = Set<String>.of(_selectedPaths);
    var nextAnchorPath = _anchorPath;
    if (range) {
      final anchorPath = nextAnchorPath ?? _primaryPath ?? entry.path;
      final anchorIndex = selectableEntries.indexWhere(
        (candidate) => candidate.path == anchorPath,
      );
      final entryIndex = selectableEntries.indexWhere(
        (candidate) => candidate.path == entry.path,
      );
      final rangePaths = <String>{};
      if (anchorIndex >= 0 && entryIndex >= 0) {
        final start = math.min(anchorIndex, entryIndex);
        final end = math.max(anchorIndex, entryIndex);
        for (var index = start; index <= end; index++) {
          rangePaths.add(selectableEntries[index].path);
        }
      } else {
        rangePaths.add(entry.path);
      }
      nextSelectedPaths = additive
          ? (nextSelectedPaths..addAll(rangePaths))
          : rangePaths;
      nextAnchorPath = anchorPath;
    } else if (additive) {
      if (nextSelectedPaths.contains(entry.path)) {
        nextSelectedPaths.remove(entry.path);
      } else {
        nextSelectedPaths.add(entry.path);
      }
      nextAnchorPath = entry.path;
    } else {
      nextSelectedPaths = {entry.path};
      nextAnchorPath = entry.path;
    }
    final primaryPath = nextSelectedPaths.contains(entry.path)
        ? entry.path
        : _primaryPathForSelection(nextSelectedPaths);
    _applySelection(
      nextSelectedPaths,
      primaryPath: primaryPath,
      anchorPath: nextSelectedPaths.isEmpty ? null : nextAnchorPath,
    );
  }

  String? _primaryPathForSelection(Set<String> selectedPaths) {
    for (final entry in widget.entries) {
      if (selectedPaths.contains(entry.path)) {
        return entry.path;
      }
    }
    return selectedPaths.isEmpty ? null : selectedPaths.first;
  }

  void _applySelection(
    Set<String> selectedPaths, {
    required String? primaryPath,
    required String? anchorPath,
  }) {
    setState(() {
      _selectedPaths = selectedPaths;
      _primaryPath = primaryPath;
      _anchorPath = anchorPath;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        widget.onSelectionChanged(
          _SftpSelectionChange(
            selectedPaths: Set<String>.of(selectedPaths),
            primaryPath: primaryPath,
            anchorPath: anchorPath,
          ),
        );
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    if (widget.entries.isEmpty) {
      return GestureDetector(
        behavior: HitTestBehavior.opaque,
        onSecondaryTapDown: widget.onBlankSecondaryTapDown,
        child: Center(
          child: Text(
            tr('sftp.label.noFiles', fallback: 'No files'),
            style: TextStyle(
              color: _mutedText,
              fontSize: NautermFontSizes.labelLarge,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
        ),
      );
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        const topPadding = 4.0;
        const rowHeight = 40.0;
        final filledHeight = topPadding + widget.entries.length * rowHeight;
        final blankHeight = math.max(0.0, constraints.maxHeight - filledHeight);
        final hasBlankTail = blankHeight > 0;
        return ListView.builder(
          itemCount: widget.entries.length + (hasBlankTail ? 2 : 1),
          itemBuilder: (context, index) {
            if (index == 0) {
              return GestureDetector(
                behavior: HitTestBehavior.opaque,
                onSecondaryTapDown: widget.onBlankSecondaryTapDown,
                child: SizedBox(height: topPadding),
              );
            }
            final entryIndex = index - 1;
            if (entryIndex >= widget.entries.length) {
              return GestureDetector(
                behavior: HitTestBehavior.opaque,
                onSecondaryTapDown: widget.onBlankSecondaryTapDown,
                child: SizedBox(height: blankHeight),
              );
            }
            final entry = widget.entries[entryIndex];
            final dragEntries = _selectedPaths.contains(entry.path)
                ? [
                    for (final candidate in widget.entries)
                      if (!candidate.isParent &&
                          _selectedPaths.contains(candidate.path))
                        candidate,
                  ]
                : <_SftpFileEntry>[entry];
            return _SftpFileRow(
              entry: entry,
              selected: _selectedPaths.contains(entry.path),
              onPrimaryPointerDown: () => _selectEntry(entry),
              onDoubleTap: () => widget.onEntryDoubleTap(entry),
              onSecondaryTapDown: (details) =>
                  widget.onEntrySecondaryTapDown(details, entry),
              dragPayload: entry.isParent
                  ? null
                  : widget.createDragPayload(
                      List<_SftpFileEntry>.unmodifiable(dragEntries),
                    ),
            );
          },
        );
      },
    );
  }
}

class _SftpFileRow extends StatefulWidget {
  const _SftpFileRow({
    required this.entry,
    required this.selected,
    required this.onPrimaryPointerDown,
    required this.onDoubleTap,
    required this.onSecondaryTapDown,
    required this.dragPayload,
  });

  final _SftpFileEntry entry;
  final bool selected;
  final VoidCallback onPrimaryPointerDown;
  final VoidCallback onDoubleTap;
  final GestureTapDownCallback onSecondaryTapDown;
  final _SftpPaneDragPayload? dragPayload;

  @override
  State<_SftpFileRow> createState() => _SftpFileRowState();
}

class _SftpFileRowState extends State<_SftpFileRow> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final selected = widget.selected;
    final rowColor = selected
        ? _workspaceDark
              ? _blend(_surface, _blue, 0.48)
              : _blue
        : _hovered
        ? _sidebarHover
        : _surface;
    final primaryColor = selected ? Colors.white : _text;
    final secondaryColor = selected ? Colors.white : _mutedText;

    final row = MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: Listener(
        behavior: HitTestBehavior.opaque,
        onPointerDown: (event) {
          if (event.buttons == kPrimaryButton) {
            widget.onPrimaryPointerDown();
          }
        },
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onDoubleTap: widget.onDoubleTap,
          onSecondaryTapDown: widget.onSecondaryTapDown,
          child: Container(
            height: 40,
            color: rowColor,
            child: Row(
              children: [
                Expanded(
                  flex: 8,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    child: Row(
                      children: [
                        Icon(
                          widget.entry.isDirectory || widget.entry.isParent
                              ? Icons.folder_rounded
                              : Icons.insert_drive_file_rounded,
                          size: 22,
                          color:
                              widget.entry.isDirectory || widget.entry.isParent
                              ? const Color(0xff5ec4f2)
                              : const Color(0xff8ca0a6),
                        ),
                        SizedBox(width: 8),
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
                                  color: primaryColor,
                                  fontSize: NautermFontSizes.labelMedium,
                                  fontWeight: NautermFontWeights.medium,
                                  letterSpacing: 0,
                                ),
                              ),
                              if (!widget.entry.isParent)
                                Text(
                                  widget.entry.permissions,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                    color: secondaryColor,
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
                  ),
                ),
                Expanded(
                  flex: 5,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    child: Text(
                      widget.entry.isParent
                          ? ''
                          : _formatSftpModified(widget.entry.modified),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: secondaryColor,
                        fontSize: NautermFontSizes.labelMedium,
                        fontWeight: NautermFontWeights.medium,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                ),
                Expanded(
                  flex: 3,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    child: Text(
                      widget.entry.isParent
                          ? ''
                          : widget.entry.isDirectory
                          ? '- -'
                          : _formatSftpSize(widget.entry.size),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: secondaryColor,
                        fontSize: NautermFontSizes.labelMedium,
                        fontWeight: NautermFontWeights.medium,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                ),
                Expanded(
                  flex: 4,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    child: Text(
                      widget.entry.isParent ? '' : widget.entry.kind,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: secondaryColor,
                        fontSize: NautermFontSizes.labelMedium,
                        fontWeight: NautermFontWeights.medium,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
    final payload = widget.dragPayload;
    if (payload == null) return row;
    return Draggable<_SftpPaneDragPayload>(
      key: ValueKey('sftp-transfer-source:${widget.entry.path}'),
      data: payload,
      dragAnchorStrategy: pointerDragAnchorStrategy,
      rootOverlay: true,
      maxSimultaneousDrags: 1,
      feedback: _SftpDragFeedback(
        entry: widget.entry,
        itemCount: payload.entries.length,
      ),
      childWhenDragging: Opacity(opacity: 0.48, child: row),
      child: row,
    );
  }
}

class _SftpDragFeedback extends StatelessWidget {
  const _SftpDragFeedback({required this.entry, required this.itemCount});

  final _SftpFileEntry entry;
  final int itemCount;

  @override
  Widget build(BuildContext context) {
    final label = itemCount == 1 ? entry.name : '$itemCount items';
    return Material(
      color: Colors.transparent,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: _workspaceMenuBackground.withValues(alpha: 0.96),
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: _blue.withValues(alpha: 0.62)),
          boxShadow: _workspaceMenuShadows,
        ),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 240),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(
                  entry.isDirectory
                      ? Icons.folder_rounded
                      : Icons.insert_drive_file_rounded,
                  size: 18,
                  color: entry.isDirectory
                      ? const Color(0xff5ec4f2)
                      : const Color(0xff8ca0a6),
                ),
                SizedBox(width: 7),
                Flexible(
                  child: Text(
                    label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: _text,
                      fontSize: NautermFontSizes.labelMedium,
                      fontWeight: NautermFontWeights.semibold,
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
  }
}

class _SftpLoadError extends StatelessWidget {
  const _SftpLoadError({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Text(
          tr(message),
          textAlign: TextAlign.center,
          style: TextStyle(
            color: _mutedText,
            fontSize: NautermFontSizes.labelLarge,
            fontWeight: NautermFontWeights.medium,
            letterSpacing: 0,
          ),
        ),
      ),
    );
  }
}

class _SftpRemoteEmptyState extends StatelessWidget {
  const _SftpRemoteEmptyState({required this.onSelectHost, this.onUseLocal});

  final VoidCallback onSelectHost;
  final VoidCallback? onUseLocal;

  @override
  Widget build(BuildContext context) {
    return ColoredBox(
      color: _surface,
      child: Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: 60,
              height: 60,
              decoration: BoxDecoration(
                color: _sidebarHover,
                borderRadius: BorderRadius.circular(17),
              ),
              child: Icon(Icons.folder_rounded, color: _text, size: 28),
            ),
            SizedBox(height: 28),
            Text(
              tr('sftp.label.connectToHost', fallback: 'Connect to host'),
              style: TextStyle(
                color: _text,
                fontSize: NautermFontSizes.titleMedium,
                fontWeight: NautermFontWeights.semibold,
                letterSpacing: 0,
              ),
            ),
            SizedBox(height: 10),
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 290),
              child: Text(
                tr(
                  'Start by connecting to a saved host\nto manage your files with SFTP.',
                ),
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: _mutedText,
                  fontSize: NautermFontSizes.labelLarge,
                  fontWeight: NautermFontWeights.medium,
                  height: 1.45,
                  letterSpacing: 0,
                ),
              ),
            ),
            SizedBox(height: 22),
            _WorkspaceButton(
              label: 'Select host',
              variant: _WorkspaceButtonVariant.filled,
              horizontalPadding: 16,
              onPressed: onSelectHost,
            ),
            if (onUseLocal != null) ...[
              SizedBox(height: 8),
              KeyedSubtree(
                key: const ValueKey('sftp-empty-use-local-button'),
                child: _WorkspaceButton(
                  icon: Icons.drive_folder_upload_rounded,
                  label: tr('common.label.local', fallback: 'Local'),
                  variant: _WorkspaceButtonVariant.text,
                  horizontalPadding: 12,
                  onPressed: onUseLocal,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
