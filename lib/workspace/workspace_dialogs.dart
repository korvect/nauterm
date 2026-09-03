part of 'nauterm_workspace.dart';

class _WorkspaceDialogThemeScope extends InheritedWidget {
  const _WorkspaceDialogThemeScope({
    required this.colors,
    required super.child,
  });

  final _AiAssistantColors colors;

  static _AiAssistantColors? maybeOf(BuildContext context) {
    return context
        .dependOnInheritedWidgetOfExactType<_WorkspaceDialogThemeScope>()
        ?.colors;
  }

  @override
  bool updateShouldNotify(_WorkspaceDialogThemeScope oldWidget) {
    return colors != oldWidget.colors;
  }
}

class _WorkspaceDialogFrame extends StatelessWidget {
  const _WorkspaceDialogFrame({
    required this.title,
    required this.content,
    required this.actions,
    this.width = 420,
  });

  static const double _contentMinHeight = 56;

  final Widget title;
  final Widget content;
  final List<Widget> actions;
  final double width;

  @override
  Widget build(BuildContext context) {
    final terminalColors = _WorkspaceDialogThemeScope.maybeOf(context);
    final surface = terminalColors?.background ?? _surface;
    final header = terminalColors?.inputBackground ?? _sidebar;
    final border = terminalColors?.border ?? _sidebarDivider;
    final foreground = terminalColors?.foreground ?? _text;
    final dark = terminalColors == null
        ? _workspaceDark
        : terminalColors.canvasBackground.computeLuminance() < 0.45;
    return Dialog(
      backgroundColor: Colors.transparent,
      surfaceTintColor: Colors.transparent,
      insetPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 24),
      child: ConstrainedBox(
        constraints: BoxConstraints(maxWidth: width),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: surface,
            borderRadius: BorderRadius.circular(12),
            border: Border.all(color: border),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: dark ? 0.38 : 0.14),
                blurRadius: 24,
                offset: const Offset(0, 14),
              ),
            ],
          ),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(12),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Container(
                  key: const ValueKey('workspace-dialog-header'),
                  padding: const EdgeInsets.fromLTRB(16, 14, 16, 12),
                  decoration: BoxDecoration(
                    color: header,
                    border: Border(bottom: BorderSide(color: border)),
                  ),
                  child: DefaultTextStyle(
                    style: TextStyle(
                      color: foreground,
                      fontSize: NautermFontSizes.titleSmall,
                      fontWeight: NautermFontWeights.semibold,
                      letterSpacing: 0,
                    ),
                    child: title,
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 16, 16, 14),
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(
                      minHeight: _contentMinHeight,
                    ),
                    child: content,
                  ),
                ),
                Container(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 10),
                  decoration: BoxDecoration(
                    border: Border(top: BorderSide(color: border)),
                  ),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      for (var index = 0; index < actions.length; index++) ...[
                        if (index > 0) SizedBox(width: 8),
                        KeyedSubtree(
                          key: ValueKey('workspace-dialog-action:$index'),
                          child: actions[index],
                        ),
                      ],
                    ],
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

class _WorkspaceConfirmDialog extends StatelessWidget {
  const _WorkspaceConfirmDialog({
    required this.title,
    required this.message,
    required this.confirmLabel,
  });

  final Widget title;
  final String message;
  final String confirmLabel;

  @override
  Widget build(BuildContext context) {
    final terminalColors = _WorkspaceDialogThemeScope.maybeOf(context);
    return _WorkspaceDialogFrame(
      width: 380,
      title: title,
      content: Text(
        tr(message),
        style: TextStyle(
          color: terminalColors?.foreground ?? _text,
          fontSize: NautermFontSizes.labelLarge,
          fontWeight: NautermFontWeights.regular,
          height: 1.35,
          letterSpacing: 0,
        ),
      ),
      actions: [
        _WorkspaceButton(
          label: 'Cancel',
          variant: _WorkspaceButtonVariant.text,
          onPressed: () => Navigator.of(context).pop(false),
        ),
        _WorkspaceButton(
          label: confirmLabel,
          type: _WorkspaceButtonType.error,
          variant: _WorkspaceButtonVariant.solid,
          onPressed: () => Navigator.of(context).pop(true),
        ),
      ],
    );
  }
}

class _WorkspaceExitDecision {
  const _WorkspaceExitDecision({required this.restoreOnNextLaunch});

  final bool restoreOnNextLaunch;
}

class _WorkspaceExitDialog extends StatefulWidget {
  const _WorkspaceExitDialog({
    required this.title,
    required this.message,
    required this.confirmLabel,
    required this.showRestoreOption,
    required this.restoreOnNextLaunch,
  });

  final String title;
  final String message;
  final String confirmLabel;
  final bool showRestoreOption;
  final bool restoreOnNextLaunch;

  @override
  State<_WorkspaceExitDialog> createState() => _WorkspaceExitDialogState();
}

class _WorkspaceExitDialogState extends State<_WorkspaceExitDialog> {
  late bool _restoreOnNextLaunch = widget.restoreOnNextLaunch;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceDialogFrame(
      width: 430,
      title: Text(widget.title),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            widget.message,
            style: TextStyle(
              color: _text,
              fontSize: NautermFontSizes.labelLarge,
              fontWeight: NautermFontWeights.regular,
              height: 1.35,
              letterSpacing: 0,
            ),
          ),
          if (widget.showRestoreOption) ...[
            SizedBox(height: 16),
            InkWell(
              borderRadius: BorderRadius.circular(7),
              onTap: () =>
                  setState(() => _restoreOnNextLaunch = !_restoreOnNextLaunch),
              child: Padding(
                padding: const EdgeInsets.symmetric(vertical: 3),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Checkbox(
                      value: _restoreOnNextLaunch,
                      onChanged: (value) =>
                          setState(() => _restoreOnNextLaunch = value ?? false),
                      activeColor: _blue,
                      materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                      visualDensity: VisualDensity.compact,
                    ),
                    SizedBox(width: 8),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            context.tr(
                              'workspace.restore.nextLaunch.label',
                              fallback: 'Restore workspace on next launch',
                            ),
                            style: TextStyle(
                              color: _text,
                              fontSize: NautermFontSizes.labelLarge,
                              fontWeight: NautermFontWeights.medium,
                              height: 1.3,
                            ),
                          ),
                          SizedBox(height: 2),
                          Text(
                            context.tr(
                              'workspace.restore.nextLaunch.description',
                              fallback: 'Recreates workspaces and split panes, reconnects terminals, and restarts active port forwards.',
                            ),
                            style: TextStyle(
                              color: _mutedText,
                              fontSize: NautermFontSizes.labelMedium,
                              fontWeight: NautermFontWeights.regular,
                              height: 1.35,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
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
          label: widget.confirmLabel,
          type: _WorkspaceButtonType.error,
          variant: _WorkspaceButtonVariant.solid,
          onPressed: () => Navigator.of(context).pop(
            _WorkspaceExitDecision(restoreOnNextLaunch: _restoreOnNextLaunch),
          ),
        ),
      ],
    );
  }
}

class _WorkspaceRecoveryDialog extends StatelessWidget {
  const _WorkspaceRecoveryDialog();

  @override
  Widget build(BuildContext context) {
    return _WorkspaceDialogFrame(
      width: 430,
      title: Text(
        context.tr(
          'workspace.restore.unexpectedExit.title',
          fallback: 'Nauterm closed unexpectedly',
        ),
      ),
      content: Text(
        context.tr(
          'workspace.restore.unexpectedExit.description',
          fallback: 'Restore your previous workspace? Terminals will reconnect and active port forwards will restart.',
        ),
        style: TextStyle(
          color: _text,
          fontSize: NautermFontSizes.labelLarge,
          fontWeight: NautermFontWeights.regular,
          height: 1.35,
          letterSpacing: 0,
        ),
      ),
      actions: [
        _WorkspaceButton(
          label: context.tr(
            'workspace.restore.startFresh',
            fallback: 'Start Fresh',
          ),
          variant: _WorkspaceButtonVariant.text,
          onPressed: () => Navigator.of(context).pop(false),
        ),
        _WorkspaceButton(
          label: context.tr(
            'workspace.restore.restoreWorkspace',
            fallback: 'Restore Workspace',
          ),
          type: _WorkspaceButtonType.primary,
          variant: _WorkspaceButtonVariant.solid,
          onPressed: () => Navigator.of(context).pop(true),
        ),
      ],
    );
  }
}

class _WorkspacePageTransition extends StatelessWidget {
  const _WorkspacePageTransition({required this.pageKey, required this.child});

  final String pageKey;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return ClipRect(
      child: AnimatedSwitcher(
        duration: _workspacePageTransitionDuration,
        reverseDuration: _workspacePageTransitionDuration,
        switchInCurve: Curves.easeOutCubic,
        switchOutCurve: Curves.easeInCubic,
        layoutBuilder: (currentChild, previousChildren) {
          final terminalLayout =
              pageKey == 'sessions' || pageKey.startsWith('terminal:');
          return Stack(
            fit: StackFit.expand,
            // Do not keep an outgoing terminal preview mounted: it can share
            // the controller and overwrite the full terminal's viewport.
            children: [?currentChild, if (!terminalLayout) ...previousChildren],
          );
        },
        transitionBuilder: (child, animation) {
          final offset = Tween<Offset>(
            begin: const Offset(0, 0.018),
            end: Offset.zero,
          ).animate(animation);
          return FadeTransition(
            opacity: animation,
            child: SlideTransition(position: offset, child: child),
          );
        },
        child: KeyedSubtree(
          key: ValueKey(pageKey),
          child: RepaintBoundary(child: child),
        ),
      ),
    );
  }
}

class _TerminalSplitPaneView extends StatefulWidget {
  const _TerminalSplitPaneView({
    super.key,
    required this.axis,
    required this.theme,
    required this.initialFractions,
    required this.onFractionsChanged,
    required this.children,
  });

  final Axis axis;
  final TerminalTheme theme;
  final List<double> initialFractions;
  final ValueChanged<List<double>> onFractionsChanged;
  final List<Widget> children;

  @override
  State<_TerminalSplitPaneView> createState() => _TerminalSplitPaneViewState();
}

class _TerminalSplitPaneViewState extends State<_TerminalSplitPaneView> {
  static const double _dividerExtent = 5;

  late List<double> _fractions = _validSplitFractions(
    widget.initialFractions,
    widget.children.length,
  );

  @override
  void didUpdateWidget(covariant _TerminalSplitPaneView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.children.length != widget.children.length ||
        oldWidget.axis != widget.axis) {
      _fractions = _equalFractions(widget.children.length);
    } else if (!listEquals(
          oldWidget.initialFractions,
          widget.initialFractions,
        ) &&
        !listEquals(_fractions, widget.initialFractions)) {
      _fractions = _validSplitFractions(
        widget.initialFractions,
        widget.children.length,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    if (widget.children.length <= 1) {
      return widget.children.single;
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final mainExtent = widget.axis == Axis.horizontal
            ? constraints.maxWidth
            : constraints.maxHeight;
        final usableExtent = math.max(
          0.0,
          mainExtent - _dividerExtent * (widget.children.length - 1),
        );

        final pieces = <Widget>[];
        for (var index = 0; index < widget.children.length; index++) {
          final paneExtent = usableExtent * _fractions[index];
          pieces.add(
            SizedBox(
              width: widget.axis == Axis.horizontal
                  ? paneExtent
                  : double.infinity,
              height: widget.axis == Axis.vertical
                  ? paneExtent
                  : double.infinity,
              child: widget.children[index],
            ),
          );

          if (index != widget.children.length - 1) {
            pieces.add(
              _TerminalSplitDivider(
                axis: widget.axis,
                theme: widget.theme,
                onDrag: (delta) => _adjustDivider(index, delta, usableExtent),
              ),
            );
          }
        }

        return widget.axis == Axis.vertical
            ? Column(children: pieces)
            : Row(children: pieces);
      },
    );
  }

  void _adjustDivider(int dividerIndex, double deltaPixels, double extent) {
    if (extent <= 0) {
      return;
    }

    final minFraction = math.min(0.18, 1 / (widget.children.length * 2));
    final deltaFraction = deltaPixels / extent;
    final left = _fractions[dividerIndex];
    final right = _fractions[dividerIndex + 1];
    final clampedDelta = deltaFraction.clamp(
      minFraction - left,
      right - minFraction,
    );
    if (clampedDelta == 0) {
      return;
    }

    setState(() {
      _fractions[dividerIndex] = left + clampedDelta;
      _fractions[dividerIndex + 1] = right - clampedDelta;
    });
    widget.onFractionsChanged(List<double>.unmodifiable(_fractions));
  }

  static List<double> _equalFractions(int count) {
    if (count <= 0) {
      return const [];
    }

    return List<double>.filled(count, 1 / count, growable: false);
  }
}

class _TerminalSplitDivider extends StatefulWidget {
  const _TerminalSplitDivider({
    required this.axis,
    required this.theme,
    required this.onDrag,
  });

  final Axis axis;
  final TerminalTheme theme;
  final ValueChanged<double> onDrag;

  @override
  State<_TerminalSplitDivider> createState() => _TerminalSplitDividerState();
}

class _TerminalSplitDividerState extends State<_TerminalSplitDivider> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final horizontal = widget.axis == Axis.horizontal;
    final cursor = horizontal
        ? SystemMouseCursors.resizeColumn
        : SystemMouseCursors.resizeRow;
    final background = widget.theme.primary.background;
    final foreground = widget.theme.primary.foreground;
    final accent = widget.theme.primary.accent;
    final baseLine = _blend(
      background,
      foreground,
      widget.theme.type == TerminalThemeType.dark ? 0.18 : 0.14,
    );
    final lineColor = _hovered ? _blend(baseLine, accent, 0.7) : baseLine;
    final railColor = _hovered
        ? _blend(
            background,
            accent,
            widget.theme.type == TerminalThemeType.dark ? 0.12 : 0.07,
          )
        : background;
    final lineExtent = math.max(
      0.5,
      1 / MediaQuery.devicePixelRatioOf(context),
    );

    return MouseRegion(
      cursor: cursor,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onPanUpdate: (details) {
          widget.onDrag(horizontal ? details.delta.dx : details.delta.dy);
        },
        child: Container(
          width: horizontal
              ? _TerminalSplitPaneViewState._dividerExtent
              : double.infinity,
          height: horizontal
              ? double.infinity
              : _TerminalSplitPaneViewState._dividerExtent,
          color: railColor,
          child: Center(
            child: Container(
              width: horizontal ? lineExtent : double.infinity,
              height: horizontal ? double.infinity : lineExtent,
              color: lineColor,
            ),
          ),
        ),
      ),
    );
  }
}
