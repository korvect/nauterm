part of 'terminal_widget.dart';

class _TerminalViewTabToolbar extends StatelessWidget {
  const _TerminalViewTabToolbar({
    required this.toolbar,
    required this.theme,
    required this.composerEnabled,
    required this.onToggleComposer,
    required this.onNewTab,
    required this.onSplitRight,
    required this.onSplitDown,
  });

  static const double height = 34;

  final TerminalViewTabToolbar toolbar;
  final TerminalTheme theme;
  final bool composerEnabled;
  final VoidCallback? onToggleComposer;
  final VoidCallback? onNewTab;
  final VoidCallback? onSplitRight;
  final VoidCallback? onSplitDown;

  @override
  Widget build(BuildContext context) {
    final showTabStrip = onNewTab != null || toolbar.tabs.length > 1;
    final activeTab = toolbar.tabs.isEmpty
        ? null
        : toolbar.tabs.firstWhere(
            (tab) => tab.selected,
            orElse: () => toolbar.tabs.first,
          );
    return Material(
      key: const ValueKey('terminal-view-toolbar'),
      color: theme.primary.background,
      child: SizedBox(
        height: height,
        child: LayoutBuilder(
          builder: (context, constraints) {
            final compact = constraints.maxWidth < 260;
            return Row(
              children: [
                Expanded(
                  child: showTabStrip
                      ? ListView.separated(
                          scrollDirection: Axis.horizontal,
                          padding: const EdgeInsets.only(left: 6, right: 10),
                          itemCount: toolbar.tabs.length,
                          separatorBuilder: (context, index) =>
                              const SizedBox(width: 4),
                          itemBuilder: (context, index) {
                            return _TerminalViewTab(
                              tab: toolbar.tabs[index],
                              theme: theme,
                            );
                          },
                        )
                      : _TerminalViewSessionTitle(
                          fallbackTitle: activeTab?.title ?? '',
                          title: toolbar.sessionTitle,
                          titleListenable: toolbar.sessionTitleListenable,
                          theme: theme,
                        ),
                ),
                Row(
                  mainAxisSize: MainAxisSize.min,
                  spacing: 2,
                  children: [
                    _TerminalViewToolbarButton(
                      tooltip: composerEnabled
                          ? tr(
                              'terminal.action.hideComposer',
                              fallback: 'Hide Composer',
                            )
                          : tr(
                              'terminal.action.showComposer',
                              fallback: 'Show Composer',
                            ),
                      icon: LucideIcons.keyboard,
                      theme: theme,
                      selected: composerEnabled,
                      onPressed: onToggleComposer,
                    ),
                    if (!compact)
                      _TerminalViewToolbarButton(
                        tooltip: tr(
                          'terminal.label.openSftp',
                          fallback: 'Open SFTP',
                        ),
                        icon: LucideIcons.folder,
                        theme: theme,
                        onPressed: toolbar.sftpAvailable
                            ? toolbar.onSftpRequested
                            : null,
                      ),
                    if (!compact && onNewTab != null)
                      _TerminalViewToolbarButton(
                        tooltip: tr(
                          'terminal.label.newTab',
                          fallback: 'New Tab',
                        ),
                        icon: LucideIcons.squarePlus,
                        theme: theme,
                        onPressed: onNewTab,
                      ),
                    _TerminalViewToolbarButton(
                      tooltip: tr(
                        'terminal.label.splitRight',
                        fallback: 'Split right',
                      ),
                      icon: LucideIcons.squareSplitHorizontal,
                      theme: theme,
                      onPressed: onSplitRight,
                    ),
                    _TerminalViewToolbarButton(
                      tooltip: tr(
                        'terminal.label.splitDown',
                        fallback: 'Split down',
                      ),
                      icon: LucideIcons.squareSplitVertical,
                      theme: theme,
                      onPressed: onSplitDown,
                    ),
                  ],
                ),
                const SizedBox(width: 4),
              ],
            );
          },
        ),
      ),
    );
  }
}

class _TerminalViewSessionTitle extends StatelessWidget {
  const _TerminalViewSessionTitle({
    required this.fallbackTitle,
    required this.title,
    required this.titleListenable,
    required this.theme,
  });

  final String fallbackTitle;
  final ValueGetter<String>? title;
  final Listenable? titleListenable;
  final TerminalTheme theme;

  @override
  Widget build(BuildContext context) {
    final foreground = theme.primary.foreground.withValues(alpha: 0.92);
    Widget buildTitle() {
      return Padding(
        padding: const EdgeInsets.only(left: 8, right: 10),
        child: Align(
          alignment: Alignment.centerLeft,
          child: Text(
            title?.call() ?? fallbackTitle,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: foreground,
              fontSize: 11.5,
              fontWeight: FontWeight.w600,
              height: 1.18,
              letterSpacing: 0,
            ),
          ),
        ),
      );
    }

    final listenable = titleListenable;
    if (listenable == null) return buildTitle();
    return ListenableBuilder(
      listenable: listenable,
      builder: (context, _) => buildTitle(),
    );
  }
}

class _TerminalViewTab extends StatelessWidget {
  const _TerminalViewTab({required this.tab, required this.theme});

  final TerminalViewTabToolbarTab tab;
  final TerminalTheme theme;

  @override
  Widget build(BuildContext context) {
    final foreground = theme.primary.foreground;
    final textColor = tab.selected
        ? foreground
        : foreground.withValues(alpha: 0.58);
    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 260),
      child: Padding(
        padding: const EdgeInsets.only(left: 2, top: 2),
        child: Material(
          color: Colors.transparent,
          borderRadius: BorderRadius.circular(6),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            onTap: tab.onSelected,
            hoverColor: foreground.withValues(alpha: 0.055),
            child: Container(
              height: 28,
              padding: const EdgeInsets.only(left: 8, right: 7),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  Flexible(
                    child: Text(
                      tab.title,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: textColor,
                        fontSize: 11.5,
                        fontWeight: FontWeight.w600,
                        height: 1.18,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                  const SizedBox(width: 6),
                  SizedBox(
                    width: 22,
                    height: 22,
                    child: IconButton(
                      tooltip: tr(
                        'terminal.label.closeTab',
                        fallback: 'Close tab',
                      ),
                      constraints: const BoxConstraints.tightFor(
                        width: 22,
                        height: 22,
                      ),
                      padding: EdgeInsets.zero,
                      visualDensity: VisualDensity.compact,
                      onPressed: tab.onClose,
                      icon: Icon(
                        Icons.close_rounded,
                        size: 14,
                        color: textColor.withValues(alpha: 0.82),
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

class _TerminalViewToolbarButton extends StatelessWidget {
  const _TerminalViewToolbarButton({
    required this.tooltip,
    required this.icon,
    required this.theme,
    required this.onPressed,
    this.selected = false,
  });

  final String tooltip;
  final IconData icon;
  final TerminalTheme theme;
  final VoidCallback? onPressed;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final foreground = theme.primary.foreground;
    final enabled = onPressed != null;
    final color = enabled
        ? foreground.withValues(alpha: selected ? 0.95 : 0.80)
        : foreground.withValues(alpha: 0.28);
    return SizedBox(
      width: 26,
      height: 30,
      child: Center(
        child: Tooltip(
          message: tr(tooltip),
          child: SizedBox(
            width: 26,
            height: 26,
            child: Material(
              color: selected && enabled
                  ? foreground.withValues(alpha: 0.11)
                  : Colors.transparent,
              shape: const CircleBorder(),
              clipBehavior: Clip.antiAlias,
              child: InkWell(
                onTap: onPressed,
                customBorder: const CircleBorder(),
                hoverColor: foreground.withValues(alpha: 0.12),
                splashColor: foreground.withValues(alpha: 0.18),
                highlightColor: foreground.withValues(alpha: 0.15),
                child: Center(child: Icon(icon, size: 16, color: color)),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _TerminalComposer extends StatelessWidget {
  const _TerminalComposer({
    required this.controller,
    required this.composerKey,
    required this.layerLink,
    required this.focusNode,
    required this.suggestion,
    required this.candidates,
    required this.selectedCandidateIndex,
    required this.completionActive,
    required this.sensitive,
    required this.expanded,
    required this.controlsExpanded,
    required this.expansionAnimation,
    required this.config,
    required this.theme,
    required this.textStyle,
    required this.onToggleExpanded,
    required this.onSubmitted,
    required this.onAcceptSuggestion,
    required this.onCancelCompletion,
    required this.onHistoryPrevious,
    required this.onHistoryNext,
    required this.onSuggestionHighlighted,
    required this.onSuggestionSelected,
  });

  final _TerminalComposerTextController controller;
  final GlobalKey composerKey;
  final LayerLink layerLink;
  final FocusNode focusNode;
  final String? suggestion;
  final List<String> candidates;
  final int? selectedCandidateIndex;
  final bool completionActive;
  final bool sensitive;
  final bool expanded;
  final bool controlsExpanded;
  final Animation<double> expansionAnimation;
  final TerminalComposerConfig config;
  final TerminalTheme theme;
  final TextStyle textStyle;
  final VoidCallback onToggleExpanded;
  final VoidCallback onSubmitted;
  final VoidCallback onAcceptSuggestion;
  final bool Function() onCancelCompletion;
  final bool Function() onHistoryPrevious;
  final bool Function() onHistoryNext;
  final ValueChanged<int> onSuggestionHighlighted;
  final ValueChanged<String> onSuggestionSelected;

  @override
  Widget build(BuildContext context) {
    final foreground = theme.primary.foreground;
    final muted = foreground.withValues(alpha: 0.48);
    final minLines = math.max(1, config.minLines);
    final maxLines = math.max(minLines, config.maxLines);
    final suggestion = sensitive ? null : this.suggestion;
    final inputTextStyle = textStyle.copyWith(color: foreground);
    final inputStrutStyle = StrutStyle.fromTextStyle(
      textStyle,
      forceStrutHeight: true,
    );
    controller.configureSuggestion(suggestion, muted);
    final surface = Material(
      key: const ValueKey('terminal-composer-surface'),
      color: Colors.transparent,
      child: CompositedTransformTarget(
        link: layerLink,
        child: _TerminalComposerSurface(
          composerKey: composerKey,
          focusNode: focusNode,
          expanded: expanded,
          theme: theme,
          child: LayoutBuilder(
            builder: (context, constraints) {
              const buttonSize = 28.0;
              const buttonGap = 2.0;
              final bottomAlignedButtonTop = math.max(
                0.0,
                constraints.maxHeight - buttonSize,
              );
              return AnimatedBuilder(
                animation: expansionAnimation,
                builder: (context, _) {
                  final progress = Curves.easeInOutCubic.transform(
                    expansionAnimation.value,
                  );
                  final collapsedInputRight = buttonSize * 2 + buttonGap + 6;
                  final expandedInputInset = buttonSize + 6;
                  return Stack(
                    key: const ValueKey('terminal-composer-controls-region'),
                    children: [
                      Positioned(
                        left: 0,
                        top: 0,
                        right:
                            collapsedInputRight +
                            (expandedInputInset - collapsedInputRight) *
                                progress,
                        bottom: expandedInputInset * progress,
                        child: Row(
                          crossAxisAlignment: expanded
                              ? CrossAxisAlignment.start
                              : CrossAxisAlignment.center,
                          children: [
                            sensitive
                                ? Icon(
                                    LucideIcons.lock,
                                    size: textStyle.fontSize,
                                    color: theme.primary.accent,
                                  )
                                : Text(
                                    r'$',
                                    style: textStyle.copyWith(
                                      color: theme.primary.accent,
                                      fontWeight: FontWeight.w700,
                                    ),
                                  ),
                            const SizedBox(width: 8),
                            Expanded(
                              child: Focus(
                                onKeyEvent: _handleSuggestionKey,
                                child: TextField(
                                  controller: controller,
                                  focusNode: focusNode,
                                  onTapOutside: (_) {},
                                  onSubmitted: (_) => onSubmitted(),
                                  keyboardType: sensitive
                                      ? TextInputType.visiblePassword
                                      : TextInputType.multiline,
                                  expands: expanded && !sensitive,
                                  minLines: sensitive
                                      ? 1
                                      : expanded
                                      ? null
                                      : minLines,
                                  maxLines: sensitive
                                      ? 1
                                      : expanded
                                      ? null
                                      : maxLines,
                                  textAlignVertical: expanded && !sensitive
                                      ? TextAlignVertical.top
                                      : TextAlignVertical.center,
                                  textInputAction: TextInputAction.send,
                                  obscureText: sensitive,
                                  obscuringCharacter: '*',
                                  enableSuggestions: !sensitive,
                                  autocorrect: false,
                                  cursorColor: theme.cursor.cursor,
                                  style: inputTextStyle,
                                  strutStyle: inputStrutStyle,
                                  decoration: InputDecoration(
                                    isDense: true,
                                    border: InputBorder.none,
                                    contentPadding: const EdgeInsets.symmetric(
                                      vertical: 3,
                                    ),
                                    hintText: sensitive
                                        ? 'Password'
                                        : config.placeholder,
                                    hintStyle: inputTextStyle.copyWith(
                                      color: muted,
                                    ),
                                  ),
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                      Positioned(
                        top: bottomAlignedButtonTop * (1 - progress),
                        right: (buttonSize + buttonGap) * (1 - progress),
                        width: buttonSize,
                        height: buttonSize,
                        child: IconButton(
                          key: const ValueKey('terminal-composer-expand'),
                          tooltip: controlsExpanded
                              ? 'Collapse Composer'
                              : 'Expand Composer',
                          padding: EdgeInsets.zero,
                          icon: Icon(
                            controlsExpanded
                                ? Icons.close_fullscreen_rounded
                                : Icons.open_in_full_rounded,
                            size: 16,
                            color: muted,
                          ),
                          onPressed: onToggleExpanded,
                        ),
                      ),
                      Positioned(
                        bottom: 0,
                        right: 0,
                        width: buttonSize,
                        height: buttonSize,
                        child: IconButton(
                          key: const ValueKey('terminal-composer-send'),
                          tooltip: tr('common.action.send', fallback: 'Send'),
                          padding: EdgeInsets.zero,
                          icon: Icon(
                            Icons.keyboard_return_rounded,
                            size: 17,
                            color: muted,
                          ),
                          onPressed: onSubmitted,
                        ),
                      ),
                    ],
                  );
                },
              );
            },
          ),
        ),
      ),
    );
    return surface;
  }

  KeyEventResult _handleSuggestionKey(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.escape) {
      return onCancelCompletion()
          ? KeyEventResult.handled
          : KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.arrowDown) {
      if (completionActive && candidates.isNotEmpty) {
        final selectedIndex = selectedCandidateIndex ?? 0;
        onSuggestionHighlighted((selectedIndex + 1) % candidates.length);
        return KeyEventResult.handled;
      }
      return onHistoryNext() ? KeyEventResult.handled : KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.arrowUp) {
      if (completionActive && candidates.isNotEmpty) {
        final selectedIndex = selectedCandidateIndex ?? 0;
        onSuggestionHighlighted(
          (selectedIndex - 1 + candidates.length) % candidates.length,
        );
        return KeyEventResult.handled;
      }
      return onHistoryPrevious()
          ? KeyEventResult.handled
          : KeyEventResult.ignored;
    }
    if ((event.logicalKey == LogicalKeyboardKey.tab ||
            event.logicalKey == LogicalKeyboardKey.arrowRight) &&
        _selectionAtInputEnd()) {
      onAcceptSuggestion();
      return KeyEventResult.handled;
    }
    if (candidates.isEmpty) {
      return KeyEventResult.ignored;
    }
    if (suggestion == null || !_selectionAtInputEnd()) {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.ignored;
  }

  bool _selectionAtInputEnd() {
    final selection = controller.selection;
    return selection.isCollapsed &&
        selection.extentOffset == controller.text.length;
  }
}

class _TerminalComposerSurface extends StatefulWidget {
  const _TerminalComposerSurface({
    required this.composerKey,
    required this.focusNode,
    required this.expanded,
    required this.theme,
    required this.child,
  });

  final GlobalKey composerKey;
  final FocusNode focusNode;
  final bool expanded;
  final TerminalTheme theme;
  final Widget child;

  @override
  State<_TerminalComposerSurface> createState() =>
      _TerminalComposerSurfaceState();
}

class _TerminalComposerSurfaceState extends State<_TerminalComposerSurface> {
  @override
  void initState() {
    super.initState();
    widget.focusNode.addListener(_handleFocusChanged);
  }

  @override
  void didUpdateWidget(_TerminalComposerSurface oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.focusNode != widget.focusNode) {
      oldWidget.focusNode.removeListener(_handleFocusChanged);
      widget.focusNode.addListener(_handleFocusChanged);
    }
  }

  @override
  void dispose() {
    widget.focusNode.removeListener(_handleFocusChanged);
    super.dispose();
  }

  void _handleFocusChanged() {
    if (mounted) {
      setState(() {});
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = widget.theme;
    final foreground = theme.primary.foreground;
    return Container(
      key: widget.composerKey,
      decoration: BoxDecoration(
        color: theme.primary.background,
        border: Border.all(
          color: widget.focusNode.hasFocus
              ? theme.primary.accent.withValues(alpha: 0.78)
              : foreground.withValues(
                  alpha: theme.type == TerminalThemeType.dark ? 0.14 : 0.10,
                ),
        ),
        borderRadius: BorderRadius.circular(10),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(
              alpha: widget.expanded
                  ? theme.type == TerminalThemeType.dark
                        ? 0.36
                        : 0.22
                  : theme.type == TerminalThemeType.dark
                  ? 0.26
                  : 0.13,
            ),
            blurRadius: widget.expanded ? 24 : 10,
            spreadRadius: widget.expanded ? -4 : -2,
            offset: Offset(0, widget.expanded ? 8 : 3),
          ),
        ],
      ),
      padding: const EdgeInsets.fromLTRB(11, 6, 7, 6),
      child: widget.child,
    );
  }
}

class _TerminalComposerSuggestionList extends StatefulWidget {
  const _TerminalComposerSuggestionList({
    required this.candidates,
    required this.selectedIndex,
    required this.theme,
    required this.textStyle,
    required this.onHighlighted,
    required this.onSelected,
  });

  final List<String> candidates;
  final int? selectedIndex;
  final TerminalTheme theme;
  final TextStyle textStyle;
  final ValueChanged<int> onHighlighted;
  final ValueChanged<String> onSelected;

  @override
  State<_TerminalComposerSuggestionList> createState() =>
      _TerminalComposerSuggestionListState();
}

class _TerminalComposerSuggestionListState
    extends State<_TerminalComposerSuggestionList> {
  static const double rowHeight = 28;
  static const double verticalPadding = 4;

  final ScrollController _scrollController = ScrollController();

  @override
  void initState() {
    super.initState();
    _scheduleSelectedRowScroll();
  }

  @override
  void didUpdateWidget(covariant _TerminalComposerSuggestionList oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.selectedIndex != widget.selectedIndex ||
        oldWidget.candidates.length != widget.candidates.length) {
      _scheduleSelectedRowScroll();
    }
  }

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  void _scheduleSelectedRowScroll() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted ||
          !_scrollController.hasClients ||
          widget.candidates.isEmpty ||
          widget.selectedIndex == null) {
        return;
      }

      final selectedTop = verticalPadding + widget.selectedIndex! * rowHeight;
      final selectedBottom = selectedTop + rowHeight;
      final viewportTop = _scrollController.offset;
      final viewportBottom =
          viewportTop + _scrollController.position.viewportDimension;

      if (selectedTop < viewportTop) {
        _scrollController.jumpTo(
          selectedTop.clamp(0, _scrollController.position.maxScrollExtent),
        );
      } else if (selectedBottom > viewportBottom) {
        _scrollController.jumpTo(
          (selectedBottom - _scrollController.position.viewportDimension).clamp(
            0,
            _scrollController.position.maxScrollExtent,
          ),
        );
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final foreground = widget.theme.primary.foreground;
    final borderColor = foreground.withValues(alpha: 0.14);
    final selectedColor = widget.theme.selection.background.withValues(
      alpha: 0.34,
    );
    final background = widget.theme.primary.background;
    return Container(
      constraints: const BoxConstraints(maxHeight: 176),
      decoration: BoxDecoration(
        color: background,
        border: Border(top: BorderSide(color: borderColor)),
      ),
      child: ListView.builder(
        controller: _scrollController,
        padding: const EdgeInsets.symmetric(vertical: 4),
        itemExtent: rowHeight,
        itemCount: widget.candidates.length,
        itemBuilder: (context, index) {
          final selected = index == widget.selectedIndex;
          final candidate = widget.candidates[index];
          return MouseRegion(
            onEnter: (_) => widget.onHighlighted(index),
            child: InkWell(
              onTap: () => widget.onSelected(candidate),
              child: Container(
                alignment: Alignment.centerLeft,
                padding: const EdgeInsets.symmetric(horizontal: 28),
                color: selected ? selectedColor : Colors.transparent,
                child: Text(
                  candidate,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: widget.textStyle.copyWith(color: foreground),
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}

class _TerminalComposerTextController extends TextEditingController {
  String? _suggestion;
  Color _suggestionColor = Colors.transparent;

  void configureSuggestion(String? suggestion, Color color) {
    _suggestion = suggestion;
    _suggestionColor = color;
  }

  @override
  TextSpan buildTextSpan({
    required BuildContext context,
    TextStyle? style,
    required bool withComposing,
  }) {
    final text = value.text;
    final suggestion = _suggestion;
    final base = super.buildTextSpan(
      context: context,
      style: style,
      withComposing: withComposing,
    );
    if (suggestion == null ||
        text.isEmpty ||
        text.contains('\n') ||
        suggestion.length <= text.length ||
        !suggestion.toLowerCase().startsWith(text.toLowerCase())) {
      return base;
    }

    return TextSpan(
      style: style,
      children: [
        base,
        TextSpan(
          text: suggestion.substring(text.length),
          style: style?.copyWith(color: _suggestionColor),
        ),
      ],
    );
  }
}

bool _isWorkspaceShortcut(KeyEvent event) {
  final keyboard = HardwareKeyboard.instance;
  final key = event.logicalKey;
  final isMacOS = defaultTargetPlatform == TargetPlatform.macOS;

  // Must have the platform modifier held
  if (isMacOS ? !keyboard.isMetaPressed : !keyboard.isControlPressed) {
    return false;
  }

  bool matches(String shortcut) {
    final parts = shortcut.toLowerCase().split('+');
    final keyName = parts.last.trim();
    final needsShift = parts.contains('shift');
    if (!shortcutKeyMatches(key, keyName, shift: needsShift)) return false;
    if (needsShift != keyboard.isShiftPressed) return false;
    final needsAlt = parts.contains('alt');
    if (needsAlt != keyboard.isAltPressed) return false;
    return true;
  }

  final config = terminalShortcutConfig;
  return matches(config.quickConnect) ||
      matches(config.commandPalette) ||
      matches(config.switchToSsh) ||
      matches(config.switchToSftp) ||
      matches(config.previousTab) ||
      matches(config.nextTab) ||
      matches(config.closeTab) ||
      matches(config.splitRight) ||
      matches(config.splitDown) ||
      matches(config.newLocalTerminal) ||
      matches(config.openSettings) ||
      config.tabSwitches.any(matches);
}
