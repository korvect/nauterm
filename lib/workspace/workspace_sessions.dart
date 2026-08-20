part of 'nauterm_workspace.dart';

const double _workspaceSidebarWidth = 258;
const double _workspaceSidebarHorizontalInset = 12;
const double _workspaceMonitorGutter = 10;
const double _workspaceMonitorPadding = 12;
const double _workspaceMonitorHeaderHeight = 34;
const double _workspaceMinTerminalPaneWidth = 420;
const double _workspaceMinTerminalViewHeight = 280;
const double _workspaceTerminalSplitDividerExtent = 5;

class _WorkspaceSessionsScaffold extends StatefulWidget {
  const _WorkspaceSessionsScaffold({
    required this.workspaces,
    required this.selectedWorkspaceId,
    required this.onWorkspaceSelected,
    required this.onWorkspaceClosed,
    required this.onWorkspaceRenamed,
    required this.onCreateWorkspace,
    required this.child,
  });

  final List<_WorkspaceRuntimeState> workspaces;
  final int selectedWorkspaceId;
  final ValueChanged<int> onWorkspaceSelected;
  final ValueChanged<int> onWorkspaceClosed;
  final ValueChanged<int> onWorkspaceRenamed;
  final VoidCallback onCreateWorkspace;
  final Widget child;

  @override
  State<_WorkspaceSessionsScaffold> createState() =>
      _WorkspaceSessionsScaffoldState();
}

class _WorkspaceSessionsScaffoldState
    extends State<_WorkspaceSessionsScaffold> {
  final ScrollController _workspaceListController = ScrollController();

  @override
  void dispose() {
    _workspaceListController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Container(
          width: _workspaceSidebarWidth,
          decoration: BoxDecoration(
            color: _sidebar,
            border: Border(right: BorderSide(color: _sidebarDivider)),
          ),
          padding: const EdgeInsets.only(top: 14, bottom: 12),
          child: Column(
            children: [
              Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: _workspaceSidebarHorizontalInset,
                ),
                child: _WorkspaceButton(
                  label: 'New workspace',
                  icon: Icons.add_rounded,
                  onPressed: widget.onCreateWorkspace,
                  variant: _WorkspaceButtonVariant.filled,
                  size: _WorkspaceControlSize.tiny,
                  fullWidth: true,
                  height: 32,
                ),
              ),
              SizedBox(height: 14),
              Expanded(
                child: ListView.separated(
                  padding: const EdgeInsets.symmetric(
                    horizontal: _workspaceSidebarHorizontalInset,
                  ),
                  controller: _workspaceListController,
                  itemCount: widget.workspaces.length,
                  separatorBuilder: (context, index) => SizedBox(height: 6),
                  itemBuilder: (context, index) {
                    final workspace = widget.workspaces[index];
                    return _WorkspaceRuntimeButton(
                      workspace: workspace,
                      selected: workspace.id == widget.selectedWorkspaceId,
                      onTap: () => widget.onWorkspaceSelected(workspace.id),
                      onRename: () => widget.onWorkspaceRenamed(workspace.id),
                      onClose: () => widget.onWorkspaceClosed(workspace.id),
                    );
                  },
                ),
              ),
            ],
          ),
        ),
        Expanded(
          child: _WorkspacePageTransition(
            pageKey: 'workspace-sessions:${widget.selectedWorkspaceId}',
            child: widget.child,
          ),
        ),
      ],
    );
  }
}

class _WorkspaceRuntimeButton extends StatefulWidget {
  const _WorkspaceRuntimeButton({
    required this.workspace,
    required this.selected,
    required this.onTap,
    required this.onRename,
    required this.onClose,
  });

  final _WorkspaceRuntimeState workspace;
  final bool selected;
  final VoidCallback onTap;
  final VoidCallback onRename;
  final VoidCallback onClose;

  @override
  State<_WorkspaceRuntimeButton> createState() =>
      _WorkspaceRuntimeButtonState();
}

class _WorkspaceRuntimeButtonState extends State<_WorkspaceRuntimeButton> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: widget.selected
          ? (_workspaceDark
                ? _blue.withValues(alpha: 0.16)
                : const Color(0xffedf7ff))
          : Colors.transparent,
      borderRadius: BorderRadius.circular(8),
      clipBehavior: Clip.antiAlias,
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: InkWell(
          onTap: widget.onTap,
          hoverColor: widget.selected
              ? (_workspaceDark
                    ? _blue.withValues(alpha: 0.22)
                    : const Color(0xffe3f1ff))
              : _sidebarHover,
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 140),
            height: 56,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(8),
              border: Border.all(
                color: widget.selected
                    ? (_workspaceDark
                          ? _blue.withValues(alpha: 0.70)
                          : const Color(0xff3d7eff))
                    : Colors.transparent,
              ),
            ),
            child: Row(
              children: [
                Container(
                  width: 36,
                  height: 36,
                  decoration: BoxDecoration(
                    color: widget.selected
                        ? widget.workspace.color
                        : (_workspaceDark
                              ? _sidebarHover
                              : const Color(0xffe5ecef)),
                    borderRadius: BorderRadius.circular(7),
                  ),
                  child: Icon(
                    widget.workspace.icon,
                    size: 21,
                    color: widget.selected ? Colors.white : _text,
                  ),
                ),
                SizedBox(width: 11),
                Expanded(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        widget.workspace.name,
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
                        '${widget.workspace.sessionCount} ${widget.workspace.sessionCount == 1 ? 'session' : 'sessions'}',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: _mutedText,
                          fontSize: NautermFontSizes.labelMedium,
                          fontWeight: NautermFontWeights.medium,
                          letterSpacing: 0,
                        ),
                      ),
                    ],
                  ),
                ),
                SizedBox(width: 6),
                AnimatedSize(
                  duration: const Duration(milliseconds: 140),
                  curve: Curves.easeOutCubic,
                  alignment: Alignment.centerRight,
                  child: _hovered
                      ? Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            _WorkspaceRuntimeActionButton(
                              tooltip: tr(
                                'workspace.label.renameWorkspace',
                                fallback: 'Rename workspace',
                              ),
                              icon: Icons.edit_rounded,
                              onPressed: widget.onRename,
                            ),
                            _WorkspaceRuntimeActionButton(
                              tooltip: tr(
                                'workspace.label.closeWorkspace',
                                fallback: 'Close workspace',
                              ),
                              icon: Icons.close_rounded,
                              color: const Color(0xffef4444),
                              onPressed: widget.onClose,
                            ),
                          ],
                        )
                      : SizedBox(width: 24, height: 24),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _WorkspaceRuntimeActionButton extends StatelessWidget {
  const _WorkspaceRuntimeActionButton({
    required this.tooltip,
    required this.icon,
    required this.onPressed,
    this.color = const Color(0xff6f8188),
  });

  final String tooltip;
  final IconData icon;
  final VoidCallback onPressed;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 24,
      height: 24,
      child: IconButton(
        tooltip: tooltip,
        visualDensity: VisualDensity.compact,
        constraints: const BoxConstraints.tightFor(width: 24, height: 24),
        padding: EdgeInsets.zero,
        onPressed: onPressed,
        icon: Icon(icon, size: 15, color: color),
        style: _workspaceIconButtonInteractionStyle,
      ),
    );
  }
}

class _WorkspaceRenameDialog extends StatefulWidget {
  const _WorkspaceRenameDialog({required this.initialName});

  final String initialName;

  @override
  State<_WorkspaceRenameDialog> createState() => _WorkspaceRenameDialogState();
}

class _WorkspaceRenameDialogState extends State<_WorkspaceRenameDialog> {
  late final TextEditingController _nameController;
  String? _error;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.initialName);
    _nameController.selection = TextSelection(
      baseOffset: 0,
      extentOffset: widget.initialName.length,
    );
  }

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
  }

  void _submit() {
    final name = _nameController.text.trim();
    if (name.isEmpty) {
      setState(() => _error = 'Workspace name is required.');
      return;
    }
    Navigator.of(context).pop(name);
  }

  @override
  Widget build(BuildContext context) {
    return _WorkspaceDialogFrame(
      width: 360,
      title: Text(
        tr('workspace.label.renameWorkspace', fallback: 'Rename workspace'),
      ),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _WorkspaceInput(
            controller: _nameController,
            label: 'Name',
            autofocus: true,
            textInputAction: TextInputAction.done,
            onSubmitted: (_) => _submit(),
          ),
          if (_error != null) ...[
            SizedBox(height: 12),
            Text(
              _error!,
              style: TextStyle(
                color: Color(0xffef4444),
                fontSize: NautermFontSizes.labelMedium,
                fontWeight: NautermFontWeights.medium,
                letterSpacing: 0,
              ),
            ),
          ],
        ],
      ),
      actions: [
        _WorkspaceButton(
          label: 'Cancel',
          variant: _WorkspaceButtonVariant.text,
          onPressed: () => Navigator.of(context).pop(),
        ),
        _WorkspaceButton(
          label: 'Rename',
          type: _WorkspaceButtonType.primary,
          variant: _WorkspaceButtonVariant.solid,
          onPressed: _submit,
        ),
      ],
    );
  }
}

class _WorkspaceSessionsPane extends StatelessWidget {
  const _WorkspaceSessionsPane({
    required this.workspace,
    required this.selectedTerminalId,
    required this.onNewTerminal,
    required this.onSessionSelected,
    required this.onSessionOpened,
    required this.sessionBuilder,
  });

  final _WorkspaceRuntimeState workspace;
  final int? selectedTerminalId;
  final VoidCallback onNewTerminal;
  final ValueChanged<int> onSessionSelected;
  final ValueChanged<int> onSessionOpened;
  final Widget Function(_TerminalTab session) sessionBuilder;

  @override
  Widget build(BuildContext context) {
    final sessions = workspace.terminalTabs;
    final selectedSessionId =
        sessions
            .where((session) => session.id == selectedTerminalId)
            .firstOrNull
            ?.id ??
        sessions.firstOrNull?.id;

    return DecoratedBox(
      decoration: BoxDecoration(color: _card),
      child: sessions.isEmpty
          ? _WorkspaceTerminalEmptyState(onNewTerminal: onNewTerminal)
          : _WorkspaceTerminalLayout(
              sessions: sessions,
              selectedTerminalId: selectedSessionId,
              onNewTerminal: onNewTerminal,
              onSessionSelected: onSessionSelected,
              onSessionOpened: onSessionOpened,
              sessionBuilder: sessionBuilder,
            ),
    );
  }
}

class _WorkspaceTerminalLayout extends StatelessWidget {
  const _WorkspaceTerminalLayout({
    required this.sessions,
    required this.selectedTerminalId,
    required this.onNewTerminal,
    required this.onSessionSelected,
    required this.onSessionOpened,
    required this.sessionBuilder,
  });

  final List<_TerminalTab> sessions;
  final int? selectedTerminalId;
  final VoidCallback onNewTerminal;
  final ValueChanged<int> onSessionSelected;
  final ValueChanged<int> onSessionOpened;
  final Widget Function(_TerminalTab session) sessionBuilder;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final monitorItemCount = sessions.length + 1;
        final width = math.max(
          1.0,
          constraints.maxWidth - _workspaceMonitorPadding * 2,
        );
        final availableHeight = math.max(
          1.0,
          (constraints.hasBoundedHeight ? constraints.maxHeight : 0) -
              _workspaceMonitorPadding * 2,
        );
        final columns = _workspaceMonitorColumnCount(
          itemCount: monitorItemCount,
          width: width,
          height: availableHeight,
        );
        final rows = (monitorItemCount / columns).ceil();
        final minTileBodyHeight = math.max(
          _workspaceMinTerminalViewHeight + _workspaceMonitorHeaderHeight,
          sessions
              .map(
                (session) =>
                    _workspaceTerminalSessionMinHeight(session) +
                    _workspaceMonitorHeaderHeight,
              )
              .fold<double>(0, math.max),
        );
        final minGridHeight =
            minTileBodyHeight * rows + _workspaceMonitorGutter * (rows - 1);
        final contentHeight = constraints.hasBoundedHeight
            ? math.max(availableHeight, minGridHeight)
            : minGridHeight;
        final gridGaps = _workspaceMonitorGutter * (rows - 1);
        final rowHeight = rows == 0
            ? contentHeight
            : (contentHeight - gridGaps) / rows;

        return SingleChildScrollView(
          padding: const EdgeInsets.all(_workspaceMonitorPadding),
          child: SizedBox(
            height: contentHeight,
            width: width,
            child: ClipRect(
              child: GridView.builder(
                padding: EdgeInsets.zero,
                physics: const NeverScrollableScrollPhysics(),
                gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                  crossAxisCount: columns,
                  crossAxisSpacing: _workspaceMonitorGutter,
                  mainAxisSpacing: _workspaceMonitorGutter,
                  mainAxisExtent: rowHeight,
                ),
                itemCount: monitorItemCount,
                itemBuilder: (context, index) {
                  if (index == sessions.length) {
                    return _WorkspaceTerminalNewSessionTile(
                      onPressed: onNewTerminal,
                    );
                  }

                  final session = sessions[index];
                  return _WorkspaceTerminalPaneFrame(
                    session: session,
                    selected: session.id == selectedTerminalId,
                    onSelected: onSessionSelected,
                    onOpened: onSessionOpened,
                    child: sessionBuilder(session),
                  );
                },
              ),
            ),
          ),
        );
      },
    );
  }
}

int _workspaceMonitorColumnCount({
  required int itemCount,
  required double width,
  required double height,
}) {
  if (itemCount <= 1) {
    return 1;
  }

  const targetAspect = 16 / 10;
  final maxColumns = math.max(
    1,
    ((width + _workspaceMonitorGutter) /
            (_workspaceMinTerminalPaneWidth + _workspaceMonitorGutter))
        .floor(),
  );
  final columnLimit = math.min(itemCount, maxColumns);
  var bestColumns = 1;
  var bestScore = double.infinity;

  for (var columns = 1; columns <= columnLimit; columns++) {
    final rows = (itemCount / columns).ceil();
    final tileWidth =
        (width - _workspaceMonitorGutter * (columns - 1)) / columns;
    final tileHeight = (height - _workspaceMonitorGutter * (rows - 1)) / rows;
    final aspect = tileWidth / math.max(1.0, tileHeight);
    final score = (aspect - targetAspect).abs() + rows * 0.03;
    if (score < bestScore) {
      bestScore = score;
      bestColumns = columns;
    }
  }

  return bestColumns;
}

double _workspaceTerminalSessionMinHeight(_TerminalTab session) {
  return _workspaceTerminalLayoutMinHeight(session.rootLayout);
}

double _workspaceTerminalLayoutMinHeight(_TerminalViewLayout layout) {
  return switch (layout) {
    _TerminalViewLeaf() => _workspaceMinTerminalViewHeight,
    _TerminalSplitLayout(:final axis, :final children) =>
      children.isEmpty
          ? _workspaceMinTerminalViewHeight
          : axis == Axis.vertical
          ? children
                    .map(_workspaceTerminalLayoutMinHeight)
                    .fold<double>(0, (sum, height) => sum + height) +
                _workspaceTerminalSplitDividerExtent * (children.length - 1)
          : children
                .map(_workspaceTerminalLayoutMinHeight)
                .fold<double>(0, math.max),
  };
}

class _WorkspaceTerminalPaneFrame extends StatelessWidget {
  const _WorkspaceTerminalPaneFrame({
    required this.session,
    required this.selected,
    required this.onSelected,
    required this.onOpened,
    required this.child,
  });

  final _TerminalTab session;
  final bool selected;
  final ValueChanged<int> onSelected;
  final ValueChanged<int> onOpened;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Listener(
      behavior: HitTestBehavior.translucent,
      onPointerUp: (_) => onSelected(session.id),
      child: AnimatedContainer(
        key: ValueKey('workspace-terminal-pane-frame:${session.id}'),
        duration: const Duration(milliseconds: 130),
        curve: Curves.easeOutCubic,
        decoration: BoxDecoration(
          color: _card,
          borderRadius: BorderRadius.circular(8),
          boxShadow: [
            BoxShadow(
              color: Colors.black.withValues(
                alpha: _workspaceDark ? 0.22 : 0.06,
              ),
              blurRadius: 10,
              offset: const Offset(0, 3),
            ),
          ],
        ),
        child: Stack(
          fit: StackFit.expand,
          children: [
            ClipRRect(
              borderRadius: BorderRadius.circular(7),
              child: Column(
                children: [
                  _WorkspaceTerminalPaneHeader(
                    session: session,
                    selected: selected,
                    onOpen: () => onOpened(session.id),
                  ),
                  Expanded(child: ClipRect(child: child)),
                ],
              ),
            ),
            IgnorePointer(
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 130),
                curve: Curves.easeOutCubic,
                decoration: BoxDecoration(
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(
                    color: selected
                        ? _blue.withValues(alpha: 0.72)
                        : _sidebarDivider,
                    width: selected ? 1.5 : 1,
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _WorkspaceTerminalNewSessionTile extends StatefulWidget {
  const _WorkspaceTerminalNewSessionTile({required this.onPressed});

  final VoidCallback onPressed;

  @override
  State<_WorkspaceTerminalNewSessionTile> createState() =>
      _WorkspaceTerminalNewSessionTileState();
}

class _WorkspaceTerminalNewSessionTileState
    extends State<_WorkspaceTerminalNewSessionTile> {
  bool _hovered = false;
  bool _pressed = false;

  @override
  Widget build(BuildContext context) {
    final borderColor = _pressed
        ? _sidebarPressed
        : _hovered
        ? _blue.withValues(alpha: 0.42)
        : _sidebarDivider;
    final iconBackground = _pressed
        ? _workspaceMenuPressed
        : _hovered
        ? _blue.withValues(alpha: _workspaceDark ? 0.16 : 0.10)
        : _sidebarHover;

    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() {
        _hovered = false;
        _pressed = false;
      }),
      child: Listener(
        onPointerDown: (_) => setState(() => _pressed = true),
        onPointerUp: (_) => setState(() => _pressed = false),
        onPointerCancel: (_) => setState(() => _pressed = false),
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onPressed,
          child: Container(
            decoration: BoxDecoration(
              color: _card,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: borderColor),
            ),
            child: Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Container(
                    width: 42,
                    height: 42,
                    decoration: BoxDecoration(
                      color: iconBackground,
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Icon(Icons.add_rounded, size: 28, color: _blue),
                  ),
                  SizedBox(height: 12),
                  Text(
                    tr('workspace.label.newConnect', fallback: 'New Connect'),
                    style: TextStyle(
                      color: _text,
                      fontSize: NautermFontSizes.labelLarge,
                      fontWeight: NautermFontWeights.semibold,
                      letterSpacing: 0,
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

class _WorkspaceTerminalPaneHeader extends StatelessWidget {
  const _WorkspaceTerminalPaneHeader({
    required this.session,
    required this.selected,
    required this.onOpen,
  });

  final _TerminalTab session;
  final bool selected;
  final VoidCallback onOpen;

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: Listenable.merge(session.controllers.toList(growable: false)),
      builder: (context, _) {
        final status = _workspaceTerminalMonitorStatus(session);
        return Container(
          height: _workspaceMonitorHeaderHeight,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          decoration: BoxDecoration(
            color: selected
                ? _blue.withValues(alpha: _workspaceDark ? 0.16 : 0.08)
                : _card,
            border: Border(bottom: BorderSide(color: _sidebarDivider)),
          ),
          child: Row(
            children: [
              Icon(
                LucideIcons.squareTerminal,
                size: 16,
                color: selected ? _blue : _mutedText,
              ),
              SizedBox(width: 7),
              Expanded(
                child: Text(
                  session.displayTitle(),
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
              SizedBox(width: 8),
              _WorkspaceTerminalStatusPill(status: status),
              SizedBox(width: 4),
              _WorkspaceButton(
                icon: Icons.open_in_new_rounded,
                tooltip: tr(
                  'workspace.label.openTerminalTab',
                  fallback: 'Open terminal tab',
                ),
                variant: _WorkspaceButtonVariant.text,
                width: 22,
                height: 22,
                horizontalPadding: 0,
                color: selected ? _blue : _mutedText,
                onPressed: onOpen,
              ),
            ],
          ),
        );
      },
    );
  }
}

class _WorkspaceTerminalMonitorStatus {
  const _WorkspaceTerminalMonitorStatus({
    required this.label,
    required this.color,
    required this.background,
  });

  final String label;
  final Color color;
  final Color background;
}

class _WorkspaceTerminalStatusPill extends StatelessWidget {
  const _WorkspaceTerminalStatusPill({required this.status});

  final _WorkspaceTerminalMonitorStatus status;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 20,
      padding: const EdgeInsets.symmetric(horizontal: 7),
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: status.background,
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        tr(status.label),
        style: TextStyle(
          color: status.color,
          fontSize: NautermFontSizes.labelSmall,
          fontWeight: NautermFontWeights.semibold,
          letterSpacing: 0,
        ),
      ),
    );
  }
}

_WorkspaceTerminalMonitorStatus _workspaceTerminalMonitorStatus(
  _TerminalTab session,
) {
  final phases = [
    for (final controller in session.controllers)
      controller.connectionStatus.phase,
  ];
  if (phases.any((phase) => phase == TerminalConnectionPhase.failed)) {
    return _WorkspaceTerminalMonitorStatus(
      label: 'Failed',
      color: const Color(0xffca3f37),
      background: const Color(0xffca3f37)
          .withValues(alpha: _workspaceDark ? 0.18 : 0.12),
    );
  }
  if (phases.any((phase) => phase == TerminalConnectionPhase.exited)) {
    return _WorkspaceTerminalMonitorStatus(
      label: 'Exited',
      color: _mutedText,
      background: _sidebarHover,
    );
  }
  if (phases.any(
    (phase) =>
        phase == TerminalConnectionPhase.connecting ||
        phase == TerminalConnectionPhase.hostKey ||
        phase == TerminalConnectionPhase.authentication,
  )) {
    return _WorkspaceTerminalMonitorStatus(
      label: 'Connecting',
      color: _blue,
      background: _blue.withValues(alpha: _workspaceDark ? 0.18 : 0.10),
    );
  }
  if (phases.any((phase) => phase == TerminalConnectionPhase.connected)) {
    return _WorkspaceTerminalMonitorStatus(
      label: 'Live',
      color: _green,
      background: _green.withValues(alpha: _workspaceDark ? 0.18 : 0.10),
    );
  }
  return _WorkspaceTerminalMonitorStatus(
    label: 'Idle',
    color: _mutedText,
    background: _sidebarHover,
  );
}

class _WorkspaceTerminalEmptyState extends StatelessWidget {
  const _WorkspaceTerminalEmptyState({required this.onNewTerminal});

  final VoidCallback onNewTerminal;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 56,
            height: 56,
            decoration: BoxDecoration(
              color: _sidebarHover,
              borderRadius: BorderRadius.circular(12),
            ),
            child: Icon(LucideIcons.squareTerminal, size: 28, color: _text),
          ),
          SizedBox(height: 14),
          Text(
            tr(
              'workspace.label.noTerminalSessions',
              fallback: 'No terminal sessions',
            ),
            style: TextStyle(
              color: _text,
              fontSize: NautermFontSizes.titleMedium,
              fontWeight: NautermFontWeights.semibold,
              letterSpacing: 0,
            ),
          ),
          SizedBox(height: 8),
          Text(
            tr(
              'workspace.description.createAConnectFromAHost',
              fallback: 'Create a connect from a host.',
            ),
            style: TextStyle(
              color: _mutedText,
              fontSize: NautermFontSizes.labelLarge,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
          SizedBox(height: 18),
          _WorkspaceButton(
            label: 'New Connect',
            icon: Icons.add_rounded,
            onPressed: onNewTerminal,
            variant: _WorkspaceButtonVariant.solid,
            type: _WorkspaceButtonType.primary,
            size: _WorkspaceControlSize.medium,
          ),
        ],
      ),
    );
  }
}
