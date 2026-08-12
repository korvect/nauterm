// ignore_for_file: invalid_use_of_internal_member, implementation_imports

part of 'nauterm_workspace.dart';

const _topBarColorTransitionDuration = Duration(milliseconds: 180);
const _topBarColorTransitionCurve = Curves.easeOutCubic;
const _topBarTabSizeTransitionDuration = Duration(milliseconds: 280);
const _topBarTabSizeTransitionCurve = Curves.easeOutExpo;
const double _topBarWorkspaceTabMaxWidth = 160;
const double _topBarWorkspaceTabChromeWidth = 39;
const double _topBarIconButtonWidth = 34;
const double _topBarControlGap = 10;
const double _topBarOverlayFadeWidth = 24;

class _TopBar extends StatelessWidget {
  const _TopBar({
    required this.selectedTab,
    required this.currentWorkspace,
    required this.terminalTabs,
    required this.selectedTerminalId,
    required this.selectedTerminalViewId,
    required this.workspacePageActive,
    required this.workspacePageEnabled,
    required this.sftpTabEnabled,
    required this.terminalTheme,
    required this.terminalChrome,
    required this.connectionPageChrome,
    required this.onTabSelected,
    required this.onTerminalTabSelected,
    required this.onTerminalTabClosed,
    required this.onWorkspaceSelected,
    required this.onQuickConnect,
    required this.aiAssistantAvailable,
    required this.aiAssistantOpen,
    required this.onAiAssistant,
    required this.terminalToolsAvailable,
    required this.terminalToolsOpen,
    required this.onTerminalTools,
    this.onStartWindowDrag,
    this.onToggleWindowMaximized,
    this.isFullscreen = false,
  });

  final _WorkspaceTab selectedTab;
  final _WorkspaceRuntimeState currentWorkspace;
  final List<_TerminalTab> terminalTabs;
  final int? selectedTerminalId;
  final int? selectedTerminalViewId;
  final bool workspacePageActive;
  final bool workspacePageEnabled;
  final bool sftpTabEnabled;
  final TerminalTheme terminalTheme;
  final bool terminalChrome;
  final bool connectionPageChrome;
  final ValueChanged<_WorkspaceTab> onTabSelected;
  final ValueChanged<int> onTerminalTabSelected;
  final ValueChanged<int> onTerminalTabClosed;
  final VoidCallback onWorkspaceSelected;
  final VoidCallback onQuickConnect;
  final bool aiAssistantAvailable;
  final bool aiAssistantOpen;
  final VoidCallback onAiAssistant;
  final bool terminalToolsAvailable;
  final bool terminalToolsOpen;
  final VoidCallback onTerminalTools;
  final VoidCallback? onStartWindowDrag;
  final VoidCallback? onToggleWindowMaximized;
  final bool isFullscreen;

  @override
  Widget build(BuildContext context) {
    final isMacOS = defaultTargetPlatform == TargetPlatform.macOS;
    final isLinux = defaultTargetPlatform == TargetPlatform.linux;
    final isWindows = defaultTargetPlatform == TargetPlatform.windows;
    final usesCustomWindowControls = isLinux || isWindows;
    final background = connectionPageChrome
        ? _surface
        : terminalChrome
        ? terminalTheme.primary.background
        : _topBar;
    final foreground = connectionPageChrome
        ? _text
        : terminalChrome
        ? terminalTheme.primary.foreground
        : _topBarForeground;
    final workspaceTabWidth = math.min(
      _topBarWorkspaceTabMaxWidth,
      _topBarWorkspaceTabNaturalWidth(context, currentWorkspace.name),
    );
    final rightControlAreaWidth =
        (aiAssistantAvailable ? _topBarIconButtonWidth : 0) +
        (terminalToolsAvailable ? _topBarIconButtonWidth : 0) +
        (usesCustomWindowControls
            ? _topBarControlGap + _linuxWindowControlsWidth
            : _topBarControlGap);
    final rightOverlayFadeWidth =
        aiAssistantAvailable || usesCustomWindowControls
        ? _topBarOverlayFadeWidth
        : 0.0;
    final rightOverlayWidth = rightControlAreaWidth + rightOverlayFadeWidth;
    final rightOverlayFadeStop = rightOverlayFadeWidth / rightOverlayWidth;

    return AnimatedContainer(
      duration: _topBarColorTransitionDuration,
      curve: _topBarColorTransitionCurve,
      height: _topBarHeight,
      decoration: BoxDecoration(color: background),
      child: Stack(
        children: [
          Positioned.fill(
            child: _WindowDragHandle(
              onStartWindowDrag: isFullscreen ? null : onStartWindowDrag,
              onDoubleTap: isFullscreen ? null : onToggleWindowMaximized,
            ),
          ),
          Positioned.fill(
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              hitTestBehavior: HitTestBehavior.deferToChild,
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  SizedBox(
                    width: isMacOS
                        ? (isFullscreen ? 14 : _macTrafficLightInset)
                        : 14,
                  ),
                  _WorkspaceTabButton(
                    label: _WorkspaceTab.vaults.label,
                    icon: _WorkspaceTab.vaults.icon,
                    selected: selectedTab == _WorkspaceTab.vaults,
                    terminalChrome: terminalChrome,
                    connectionPageChrome: connectionPageChrome,
                    terminalTheme: terminalTheme,
                    onTap: () => onTabSelected(_WorkspaceTab.vaults),
                    collapseToIconWhenInactive: true,
                    constrainWidth: false,
                  ),
                  if (sftpTabEnabled) ...[
                    SizedBox(width: 6),
                    _WorkspaceTabButton(
                      label: _WorkspaceTab.sftp.label,
                      icon: _WorkspaceTab.sftp.icon,
                      selected: selectedTab == _WorkspaceTab.sftp,
                      terminalChrome: terminalChrome,
                      connectionPageChrome: connectionPageChrome,
                      terminalTheme: terminalTheme,
                      onTap: () => onTabSelected(_WorkspaceTab.sftp),
                      collapseToIconWhenInactive: true,
                      constrainWidth: false,
                    ),
                  ],
                  if (workspacePageEnabled) ...[
                    SizedBox(width: 6),
                    SizedBox(
                      width: workspaceTabWidth,
                      child: _WorkspaceTabButton(
                        label: currentWorkspace.name,
                        icon: currentWorkspace.icon,
                        selected:
                            selectedTab == _WorkspaceTab.sessions &&
                            workspacePageActive,
                        terminalChrome: terminalChrome,
                        connectionPageChrome: connectionPageChrome,
                        terminalTheme: terminalTheme,
                        onTap: onWorkspaceSelected,
                        constrainWidth: false,
                      ),
                    ),
                  ],
                  for (final terminalTab in terminalTabs) ...[
                    SizedBox(width: 6),
                    _WorkspaceTabButton(
                      key: ValueKey('terminal-top-tab:${terminalTab.id}'),
                      label: terminalTab.displayTitle(
                        selectedViewId: selectedTerminalId == terminalTab.id
                            ? selectedTerminalViewId
                            : null,
                      ),
                      icon: LucideIcons.squareTerminal,
                      selected:
                          selectedTab == _WorkspaceTab.sessions &&
                          !workspacePageActive &&
                          selectedTerminalId == terminalTab.id,
                      terminalChrome: terminalChrome,
                      connectionPageChrome: connectionPageChrome,
                      terminalTheme: terminalTheme,
                      onTap: () => onTerminalTabSelected(terminalTab.id),
                      onClose: () => onTerminalTabClosed(terminalTab.id),
                      showBellIndicator:
                          terminalBellConfig.tabIndicator &&
                          terminalTab.bellIndicator &&
                          !(selectedTab == _WorkspaceTab.sessions &&
                              !workspacePageActive &&
                              selectedTerminalId == terminalTab.id),
                    ),
                  ],
                  _TopBarIconButton(
                    tooltip: tr(
                      'common.label.quickConnect',
                      fallback: 'Quick Connect',
                    ),
                    onPressed: onQuickConnect,
                    icon: Icons.add_rounded,
                    color: foreground,
                    background: background,
                  ),
                  SizedBox(width: rightOverlayWidth),
                ],
              ),
            ),
          ),
          Positioned(
            top: 0,
            right: 0,
            bottom: 0,
            width: rightOverlayWidth,
            child: AnimatedContainer(
              duration: _topBarColorTransitionDuration,
              curve: _topBarColorTransitionCurve,
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  colors: rightOverlayFadeWidth > 0
                      ? [
                          background.withValues(alpha: 0),
                          background,
                          background,
                        ]
                      : [background, background, background],
                  stops: [0, rightOverlayFadeStop, 1],
                ),
              ),
              child: Stack(
                children: [
                  Positioned.fill(
                    child: _WindowDragHandle(
                      onStartWindowDrag: isFullscreen
                          ? null
                          : onStartWindowDrag,
                      onDoubleTap: isFullscreen
                          ? null
                          : onToggleWindowMaximized,
                    ),
                  ),
                  Positioned.fill(
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        if (aiAssistantAvailable)
                          _TopBarIconButton(
                            tooltip: tr(
                              'common.label.aiAssistant',
                              fallback: 'AI Assistant',
                            ),
                            onPressed: onAiAssistant,
                            icon: LucideIcons.sparkles,
                            color: aiAssistantOpen
                                ? terminalTheme.primary.accent
                                : foreground,
                            background: background,
                          ),
                        if (terminalToolsAvailable)
                          _TopBarIconButton(
                            tooltip: tr(
                              'workspace.label.terminalTools',
                              fallback: 'Terminal tools',
                            ),
                            onPressed: onTerminalTools,
                            icon: LucideIcons.panelRight,
                            color: terminalToolsOpen
                                ? terminalTheme.primary.accent
                                : foreground,
                            background: background,
                          ),
                        if (usesCustomWindowControls) ...[
                          SizedBox(width: _topBarControlGap),
                          _TopBarInteractiveRegion(
                            child: _LinuxWindowControls(foreground: foreground),
                          ),
                        ] else
                          SizedBox(width: _topBarControlGap),
                      ],
                    ),
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

double _topBarWorkspaceTabNaturalWidth(BuildContext context, String label) {
  final textPainter = TextPainter(
    text: TextSpan(
      text: label,
      style: TextStyle(
        fontSize: NautermFontSizes.labelLarge,
        fontWeight: NautermFontWeights.medium,
        letterSpacing: 0,
      ),
    ),
    maxLines: 1,
    textDirection: Directionality.of(context),
    textScaler: MediaQuery.textScalerOf(context),
  )..layout();
  return _topBarWorkspaceTabChromeWidth + textPainter.width;
}

class _LinuxWindowControls extends StatelessWidget {
  const _LinuxWindowControls({required this.foreground});

  final Color foreground;

  @override
  Widget build(BuildContext context) {
    final isWindows = defaultTargetPlatform == TargetPlatform.windows;
    final regularWindowController = switch (WindowScope.maybeOf(context)) {
      final RegularWindowController controller => controller,
      _ => null,
    };

    Widget buildControls(bool isMaximized) {
      return SizedBox(
        width: _linuxWindowControlsWidth,
        height: _topBarHeight,
        child: Padding(
          padding: const EdgeInsetsDirectional.only(
            end: _linuxWindowControlsEndPadding,
          ),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              _LinuxWindowControlButton(
                tooltip: tr('common.label.minimize', fallback: 'Minimize'),
                icon: Icon(LucideIcons.minus, size: 15),
                foreground: foreground,
                onPressed: regularWindowController == null
                    ? null
                    : () => regularWindowController.setMinimized(true),
              ),
              _LinuxWindowControlButton(
                tooltip: isMaximized ? 'Restore' : 'Maximize',
                icon: isMaximized
                    ? SvgPicture.asset(
                        'assets/icons/ui/window-restore.svg',
                        key: const ValueKey('window-restore-icon'),
                        width: 15,
                        height: 15,
                        colorFilter: ColorFilter.mode(
                          foreground,
                          BlendMode.srcIn,
                        ),
                      )
                    : const Icon(
                        LucideIcons.square,
                        key: ValueKey('window-maximize-icon'),
                        size: 13,
                      ),
                foreground: foreground,
                onPressed: regularWindowController == null
                    ? null
                    : () {
                        regularWindowController.setMaximized(!isMaximized);
                        if (isWindows) {
                          hideMainWindowTitleBar();
                        }
                      },
              ),
              _LinuxWindowControlButton(
                tooltip: tr('common.action.close', fallback: 'Close'),
                icon: Icon(LucideIcons.x, size: 15),
                foreground: foreground,
                hoverColor: _topBarDestructiveHover,
                onPressed: () {
                  if (!requestMainWindowClose()) {
                    regularWindowController?.destroy();
                  }
                },
              ),
            ],
          ),
        ),
      );
    }

    if (regularWindowController == null) {
      return buildControls(false);
    }
    return ListenableBuilder(
      listenable: regularWindowController,
      builder: (context, child) =>
          buildControls(regularWindowController.isMaximized),
    );
  }
}

class _LinuxWindowControlButton extends StatefulWidget {
  const _LinuxWindowControlButton({
    required this.tooltip,
    required this.icon,
    required this.foreground,
    this.hoverColor,
    this.onPressed,
  });

  final String tooltip;
  final Widget icon;
  final Color foreground;
  final Color? hoverColor;
  final VoidCallback? onPressed;

  @override
  State<_LinuxWindowControlButton> createState() =>
      _LinuxWindowControlButtonState();
}

class _LinuxWindowControlButtonState extends State<_LinuxWindowControlButton> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final hoverColor =
        widget.hoverColor ?? widget.foreground.withValues(alpha: 0.10);
    return SizedBox(
      width: _topBarIconButtonWidth,
      height: _topBarHeight,
      child: MouseRegion(
        cursor: widget.onPressed == null
            ? SystemMouseCursors.basic
            : SystemMouseCursors.click,
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onPressed,
          child: Center(
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 120),
              width: 26,
              height: 26,
              decoration: BoxDecoration(
                color: _hovered ? hoverColor : hoverColor.withValues(alpha: 0),
                borderRadius: BorderRadius.circular(7),
              ),
              child: IconTheme(
                data: IconThemeData(color: widget.foreground),
                child: Center(child: widget.icon),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _TopBarInteractiveRegion extends StatefulWidget {
  const _TopBarInteractiveRegion({required this.child});

  final Widget child;

  @override
  State<_TopBarInteractiveRegion> createState() =>
      _TopBarInteractiveRegionState();
}

class _TopBarInteractiveRegionState extends State<_TopBarInteractiveRegion> {
  void _reportRegionAfterLayout() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      final renderObject = context.findRenderObject();
      if (renderObject is! RenderBox || !renderObject.hasSize) {
        return;
      }
      updateMainWindowTitleBarInteractiveRegion(
        this,
        renderObject.localToGlobal(Offset.zero) & renderObject.size,
      );
    });
  }

  @override
  void dispose() {
    removeMainWindowTitleBarInteractiveRegion(this);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    _reportRegionAfterLayout();
    return Listener(behavior: HitTestBehavior.opaque, child: widget.child);
  }
}

class _WindowDragHandle extends StatefulWidget {
  const _WindowDragHandle({this.onStartWindowDrag, this.onDoubleTap});

  final VoidCallback? onStartWindowDrag;
  final VoidCallback? onDoubleTap;

  @override
  State<_WindowDragHandle> createState() => _WindowDragHandleState();
}

class _WindowDragHandleState extends State<_WindowDragHandle> {
  DateTime? _lastPointerDownAt;
  Offset? _lastPointerDownPosition;

  void _handlePointerDown(PointerDownEvent event) {
    if (event.buttons != kPrimaryMouseButton) {
      return;
    }

    final now = DateTime.now();
    final lastPointerDownAt = _lastPointerDownAt;
    final lastPointerDownPosition = _lastPointerDownPosition;
    _lastPointerDownAt = now;
    _lastPointerDownPosition = event.position;

    if (widget.onDoubleTap != null &&
        lastPointerDownAt != null &&
        lastPointerDownPosition != null &&
        now.difference(lastPointerDownAt) <= kDoubleTapTimeout &&
        (event.position - lastPointerDownPosition).distance <= kDoubleTapSlop) {
      _lastPointerDownAt = null;
      _lastPointerDownPosition = null;
      widget.onDoubleTap!();
      return;
    }

    widget.onStartWindowDrag?.call();
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      behavior: HitTestBehavior.translucent,
      onPointerDown: widget.onStartWindowDrag == null
          ? null
          : _handlePointerDown,
      child: const SizedBox(height: _topBarHeight),
    );
  }
}

class _TopBarIconButton extends StatelessWidget {
  const _TopBarIconButton({
    required this.tooltip,
    required this.icon,
    required this.color,
    required this.background,
    required this.onPressed,
  });

  final String tooltip;
  final IconData icon;
  final Color color;
  final Color background;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return _TopBarInteractiveRegion(
      child: SizedBox(
        width: _topBarIconButtonWidth,
        height: _topBarHeight,
        child: Center(
          child: IconButton(
            tooltip: tooltip,
            visualDensity: VisualDensity.compact,
            constraints: const BoxConstraints.tightFor(width: 30, height: 30),
            padding: EdgeInsets.zero,
            onPressed: onPressed,
            style: IconButton.styleFrom(
              hoverColor: _blend(background, color, 0.08),
              highlightColor: _blend(background, color, 0.16),
            ),
            icon: TweenAnimationBuilder<Color?>(
              tween: ColorTween(end: color),
              duration: _topBarColorTransitionDuration,
              curve: _topBarColorTransitionCurve,
              builder: (context, animatedColor, child) {
                return Icon(icon, size: 18, color: animatedColor ?? color);
              },
            ),
          ),
        ),
      ),
    );
  }
}

class _WorkspaceTabButton extends StatefulWidget {
  const _WorkspaceTabButton({
    super.key,
    required this.label,
    required this.icon,
    required this.selected,
    required this.terminalChrome,
    required this.connectionPageChrome,
    required this.terminalTheme,
    required this.onTap,
    this.onClose,
    this.showBellIndicator = false,
    this.collapseToIconWhenInactive = false,
    this.constrainWidth = true,
  });

  final String label;
  final IconData icon;
  final bool selected;
  final bool terminalChrome;
  final bool connectionPageChrome;
  final TerminalTheme terminalTheme;
  final VoidCallback onTap;
  final VoidCallback? onClose;
  final bool showBellIndicator;
  final bool collapseToIconWhenInactive;
  final bool constrainWidth;

  @override
  State<_WorkspaceTabButton> createState() => _WorkspaceTabButtonState();
}

class _WorkspaceTabButtonState extends State<_WorkspaceTabButton> {
  bool _hovered = false;
  bool _pressed = false;

  @override
  Widget build(BuildContext context) {
    final color = widget.connectionPageChrome
        ? (widget.selected ? _text : _mutedText)
        : widget.terminalChrome
        ? widget.terminalTheme.primary.foreground
        : widget.selected
        ? (_workspaceDark ? _text : const Color(0xffe9edf4))
        : (_workspaceDark ? _mutedText : const Color(0xffc6ccda));
    final baseBackground = widget.connectionPageChrome
        ? _blend(_surface, _text, widget.selected ? 0.09 : 0.035)
        : widget.terminalChrome
        ? _blend(
            widget.terminalTheme.primary.background,
            widget.terminalTheme.primary.foreground,
            widget.selected ? 0.13 : 0.06,
          )
        : widget.selected
        ? _topBarTabActive
        : (_workspaceDark
              ? _topBarTabInactive
              : const Color(0xff40465b).withValues(alpha: 0.72));

    final collapsed = widget.collapseToIconWhenInactive && !widget.selected;
    final width = collapsed
        ? 30.0
        : widget.constrainWidth
        ? widget.selected
              ? 220.0
              : 142.0
        : null;
    final contentPadding = EdgeInsets.symmetric(horizontal: collapsed ? 0 : 8);
    final overlayAmount = _pressed ? 0.16 : (_hovered ? 0.08 : 0.0);
    final background = overlayAmount == 0
        ? baseBackground
        : _blend(baseBackground, color, overlayAmount);

    final button = AnimatedContainer(
      duration: _topBarColorTransitionDuration,
      curve: _topBarColorTransitionCurve,
      decoration: BoxDecoration(
        color: background,
        borderRadius: BorderRadius.circular(7),
      ),
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() {
          _hovered = false;
          _pressed = false;
        }),
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTapDown: (_) => setState(() => _pressed = true),
          onTapCancel: () => setState(() => _pressed = false),
          onTapUp: (_) => setState(() => _pressed = false),
          onTap: widget.onTap,
          child: TweenAnimationBuilder<Color?>(
            tween: ColorTween(end: color),
            duration: _topBarColorTransitionDuration,
            curve: _topBarColorTransitionCurve,
            builder: (context, animatedColor, child) {
              final foreground = animatedColor ?? color;
              return Align(
                alignment: collapsed ? Alignment.center : Alignment.centerLeft,
                child: Padding(
                  padding: contentPadding,
                  child: IconTheme(
                    data: IconThemeData(color: foreground),
                    child: DefaultTextStyle(
                      style: TextStyle(
                        color: foreground,
                        fontSize: NautermFontSizes.labelLarge,
                        height: 1,
                        leadingDistribution: TextLeadingDistribution.even,
                        fontWeight: NautermFontWeights.medium,
                        letterSpacing: 0,
                      ),
                      child: Row(
                        mainAxisSize: widget.constrainWidth
                            ? MainAxisSize.max
                            : MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.center,
                        children: [
                          Icon(widget.icon, size: 14),
                          if (!collapsed) ...[
                            SizedBox(width: 7),
                            Flexible(
                              fit: widget.constrainWidth
                                  ? FlexFit.tight
                                  : FlexFit.loose,
                              child: Text(
                                widget.label,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                            if (widget.showBellIndicator) ...[
                              SizedBox(width: 5),
                              Icon(
                                LucideIcons.bell,
                                key: const ValueKey(
                                  'terminal-tab-bell-indicator',
                                ),
                                size: 12,
                              ),
                            ],
                            if (widget.onClose != null) ...[
                              SizedBox(width: 5),
                              _TopBarTabCloseButton(
                                color: foreground,
                                onPressed: widget.onClose!,
                              ),
                            ],
                          ],
                        ],
                      ),
                    ),
                  ),
                ),
              );
            },
          ),
        ),
      ),
    );

    final Widget sizedButton;
    if (widget.collapseToIconWhenInactive && !widget.constrainWidth) {
      sizedButton = AnimatedSize(
        duration: _topBarTabSizeTransitionDuration,
        curve: _topBarTabSizeTransitionCurve,
        alignment: Alignment.centerLeft,
        child: SizedBox(width: width, height: 30, child: button),
      );
    } else {
      sizedButton = AnimatedContainer(
        duration: _topBarTabSizeTransitionDuration,
        curve: _topBarTabSizeTransitionCurve,
        width: width,
        height: 30,
        child: button,
      );
    }

    return _TopBarInteractiveRegion(
      child: SizedBox(
        height: _topBarHeight,
        child: Center(child: sizedButton),
      ),
    );
  }
}

class _TopBarTabCloseButton extends StatefulWidget {
  const _TopBarTabCloseButton({required this.color, required this.onPressed});

  final Color color;
  final VoidCallback onPressed;

  @override
  State<_TopBarTabCloseButton> createState() => _TopBarTabCloseButtonState();
}

class _TopBarTabCloseButtonState extends State<_TopBarTabCloseButton> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.onPressed,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 110),
          curve: Curves.easeOutCubic,
          width: 18,
          height: 18,
          decoration: BoxDecoration(
            color: _hovered
                ? widget.color.withValues(alpha: 0.12)
                : Colors.transparent,
            borderRadius: BorderRadius.circular(5),
          ),
          child: Icon(
            Icons.close_rounded,
            size: 14,
            color: widget.color.withValues(alpha: _hovered ? 0.95 : 0.72),
          ),
        ),
      ),
    );
  }
}
