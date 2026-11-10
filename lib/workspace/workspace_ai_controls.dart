part of 'nauterm_workspace.dart';

typedef _AiProviderModelValue = ({String providerUuid, String model});

class _AiProviderModelOption {
  const _AiProviderModelOption({
    required this.value,
    required this.providerName,
    required this.model,
  });

  final _AiProviderModelValue value;
  final String providerName;
  final String model;
}

class _AiProviderModelSelector extends StatefulWidget {
  const _AiProviderModelSelector({
    required this.value,
    required this.providerName,
    required this.model,
    required this.options,
    required this.colors,
    required this.onChanged,
  });

  final _AiProviderModelValue value;
  final String providerName;
  final String model;
  final List<_AiProviderModelOption> options;
  final _AiAssistantColors colors;
  final ValueChanged<_AiProviderModelValue> onChanged;

  @override
  State<_AiProviderModelSelector> createState() =>
      _AiProviderModelSelectorState();
}

class _AiProviderModelSelectorState extends State<_AiProviderModelSelector> {
  final GlobalKey _anchorKey = GlobalKey();
  bool _open = false;

  Future<void> _showMenu() async {
    if (_open) {
      return;
    }
    final anchorBox =
        _anchorKey.currentContext?.findRenderObject() as RenderBox?;
    final overlayBox =
        Overlay.of(context).context.findRenderObject() as RenderBox?;
    if (anchorBox == null || overlayBox == null) {
      return;
    }
    final anchor =
        anchorBox.localToGlobal(Offset.zero, ancestor: overlayBox) &
        anchorBox.size;
    setState(() => _open = true);
    final selected = await _showAiProviderModelMenu(
      context: context,
      anchor: anchor,
      value: widget.value,
      options: widget.options,
      colors: widget.colors,
    );
    if (!mounted) {
      return;
    }
    setState(() => _open = false);
    if (selected != null && selected != widget.value) {
      widget.onChanged(selected);
    }
  }

  @override
  Widget build(BuildContext context) {
    const maxWidth = 220.0;
    const arrowSize = 14.0;
    const arrowGap = 3.0;
    final textStyle = TextStyle(
      color: widget.colors.muted,
      fontSize: 11.5,
      fontWeight: NautermFontWeights.medium,
      height: 1.2,
      letterSpacing: 0,
      decoration: TextDecoration.none,
    );
    final fullLabel = widget.model.isEmpty
        ? widget.providerName
        : '${widget.providerName} / ${widget.model}';

    return MouseRegion(
      cursor: SystemMouseCursors.click,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: _showMenu,
        child: Semantics(
          button: true,
          label: 'AI provider and model',
          value: fullLabel,
          child: ConstrainedBox(
            key: _anchorKey,
            constraints: const BoxConstraints(
              maxWidth: maxWidth,
              minHeight: 26,
            ),
            child: LayoutBuilder(
              builder: (context, constraints) {
                final fullLabelWidth = _measureWorkspaceSelectorText(
                  context,
                  fullLabel,
                  textStyle,
                );
                final label =
                    fullLabelWidth + arrowGap + arrowSize <=
                            constraints.maxWidth &&
                        constraints.maxWidth.isFinite
                    ? fullLabel
                    : widget.model.isEmpty
                    ? widget.providerName
                    : widget.model;
                return Align(
                  alignment: Alignment.centerRight,
                  widthFactor: 1,
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Flexible(
                        child: Text(
                          label,
                          key: const ValueKey('ai-provider-model-label'),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          textAlign: TextAlign.right,
                          style: textStyle,
                        ),
                      ),
                      const SizedBox(width: arrowGap),
                      Icon(
                        _open ? LucideIcons.chevronUp : LucideIcons.chevronDown,
                        size: arrowSize,
                        color: widget.colors.muted,
                      ),
                    ],
                  ),
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}

Future<_AiProviderModelValue?> _showAiProviderModelMenu({
  required BuildContext context,
  required Rect anchor,
  required _AiProviderModelValue value,
  required List<_AiProviderModelOption> options,
  required _AiAssistantColors colors,
}) {
  final completer = Completer<_AiProviderModelValue?>();
  late final NautermTransientOverlayHandle handle;
  var completed = false;

  void complete(_AiProviderModelValue? result, {bool dismiss = true}) {
    if (completed) {
      return;
    }
    completed = true;
    completer.complete(result);
    if (dismiss) {
      handle.dismiss(notify: false);
    }
  }

  handle = showNautermTransientOverlay(
    context: context,
    token: Object(),
    dismissExisting: true,
    onDismissed: () => complete(null, dismiss: false),
    builder: (context) => _AiProviderModelMenuOverlay(
      anchor: anchor,
      value: value,
      options: options,
      colors: colors,
      onSelected: complete,
      onDismissed: () => complete(null),
    ),
  );
  return completer.future;
}

class _AiProviderModelMenuOverlay extends StatelessWidget {
  const _AiProviderModelMenuOverlay({
    required this.anchor,
    required this.value,
    required this.options,
    required this.colors,
    required this.onSelected,
    required this.onDismissed,
  });

  final Rect anchor;
  final _AiProviderModelValue value;
  final List<_AiProviderModelOption> options;
  final _AiAssistantColors colors;
  final ValueChanged<_AiProviderModelValue> onSelected;
  final VoidCallback onDismissed;

  @override
  Widget build(BuildContext context) {
    final overlaySize = MediaQuery.sizeOf(context);
    final safePadding = nautermTransientOverlaySafePadding(
      context,
      base: MediaQuery.paddingOf(context),
    );
    const margin = 8.0;
    const gap = 5.0;
    const menuChromeHeight = 14.0;
    const menuChromeWidth = 54.0;
    final providerTextStyle = TextStyle(
      fontSize: 10.5,
      fontWeight: NautermFontWeights.medium,
      height: 1.1,
      letterSpacing: 0,
    );
    final modelTextStyle = TextStyle(
      fontSize: 11.5,
      fontWeight: NautermFontWeights.medium,
      height: 1.1,
      letterSpacing: 0,
    );
    final longestOptionWidth = options.fold<double>(
      0,
      (width, option) => math.max(
        width,
        math.max(
          _measureWorkspaceSelectorText(
            context,
            option.providerName,
            providerTextStyle,
          ),
          _measureWorkspaceSelectorText(context, option.model, modelTextStyle),
        ),
      ),
    );
    final maximumMenuWidth = math.max(
      1.0,
      math.min(320.0, overlaySize.width - margin * 2),
    );
    final minimumMenuWidth = math.min(180.0, maximumMenuWidth);
    final menuWidth = (longestOptionWidth + menuChromeWidth)
        .clamp(minimumMenuWidth, maximumMenuWidth)
        .toDouble();
    const footerActionHeight = 40.0;
    const footerDividerHeight = 1.0;
    final contentHeight = options.isEmpty
        ? footerActionHeight + 2
        : menuChromeHeight +
              options.length * 44.0 +
              footerDividerHeight +
              footerActionHeight;
    final desiredHeight = math.min(280.0, contentHeight);
    final availableAbove = anchor.top - safePadding.top - margin - gap;
    final availableBelow =
        overlaySize.height - safePadding.bottom - anchor.bottom - margin - gap;
    final openAbove =
        availableAbove >= desiredHeight || availableAbove > availableBelow;
    final availableHeight = math.max(
      44.0,
      openAbove ? availableAbove : availableBelow,
    );
    final menuHeight = math.min(desiredHeight, availableHeight);
    final minLeft = safePadding.left + margin;
    final maxLeft = math.max(
      minLeft,
      overlaySize.width - safePadding.right - menuWidth - margin,
    );
    final left = (anchor.right - menuWidth).clamp(minLeft, maxLeft).toDouble();
    final top = openAbove ? anchor.top - gap - menuHeight : anchor.bottom + gap;
    final rect = positionNautermTransientOverlay(
      context: context,
      preferredRect: Rect.fromLTWH(left, top, menuWidth, menuHeight),
      overlaySize: overlaySize,
      safePadding: safePadding,
    );

    return Focus(
      autofocus: true,
      onKeyEvent: (_, event) {
        if (event is KeyDownEvent &&
            event.logicalKey == LogicalKeyboardKey.escape) {
          onDismissed();
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: Stack(
        children: [
          Positioned.fill(
            child: Listener(
              behavior: HitTestBehavior.opaque,
              onPointerDown: (_) => onDismissed(),
            ),
          ),
          Positioned(
            left: rect.left,
            top: rect.top,
            width: menuWidth,
            height: menuHeight,
            child: TweenAnimationBuilder<double>(
              tween: Tween(begin: 0, end: 1),
              duration: const Duration(milliseconds: 90),
              curve: Curves.easeOutCubic,
              builder: (context, animation, child) {
                return Opacity(
                  opacity: animation,
                  child: Transform.translate(
                    offset: Offset(0, (1 - animation) * 4),
                    child: child,
                  ),
                );
              },
              child: Container(
                key: const ValueKey('ai-provider-model-menu'),
                decoration: BoxDecoration(
                  color: colors.background,
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(color: colors.border),
                  boxShadow: [
                    BoxShadow(
                      color: Colors.black.withValues(
                        alpha:
                            ThemeData.estimateBrightnessForColor(
                                  colors.background,
                                ) ==
                                Brightness.dark
                            ? 0.34
                            : 0.14,
                      ),
                      blurRadius: 22,
                      offset: const Offset(0, 10),
                    ),
                  ],
                ),
                clipBehavior: Clip.antiAlias,
                child: Material(
                  type: MaterialType.transparency,
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      if (options.isNotEmpty)
                        Flexible(
                          child: ListView(
                            key: const ValueKey(
                              'ai-provider-model-menu-scroll',
                            ),
                            shrinkWrap: true,
                            padding: const EdgeInsets.all(6),
                            children: [
                              for (final option in options)
                                _AiProviderModelMenuItem(
                                  option: option,
                                  selected: option.value == value,
                                  colors: colors,
                                  onPressed: () => onSelected(option.value),
                                ),
                            ],
                          ),
                        ),
                      if (options.isNotEmpty)
                        Divider(height: 1, color: colors.border),
                      Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 6,
                          vertical: 4,
                        ),
                        child: TextButton.icon(
                          key: const ValueKey('ai-configure-providers'),
                          onPressed: () {
                            onDismissed();
                            showSettingsWindow(page: NautermSettingsPage.ai);
                          },
                          icon: const Icon(LucideIcons.settings, size: 14),
                          label: Text(
                            tr(
                              'ai.action.configureProviders',
                              fallback: 'Configure Providers…',
                            ),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          style: TextButton.styleFrom(
                            alignment: Alignment.centerLeft,
                            foregroundColor: colors.foreground,
                            minimumSize: const Size(0, 40),
                            padding: const EdgeInsets.symmetric(horizontal: 9),
                            textStyle: const TextStyle(fontSize: 11.5),
                            tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                            visualDensity: VisualDensity.compact,
                            shape: RoundedRectangleBorder(
                              borderRadius: BorderRadius.circular(6),
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
      ),
    );
  }
}

class _AiProviderModelMenuItem extends StatefulWidget {
  const _AiProviderModelMenuItem({
    required this.option,
    required this.selected,
    required this.colors,
    required this.onPressed,
  });

  final _AiProviderModelOption option;
  final bool selected;
  final _AiAssistantColors colors;
  final VoidCallback onPressed;

  @override
  State<_AiProviderModelMenuItem> createState() =>
      _AiProviderModelMenuItemState();
}

class _AiProviderModelMenuItemState extends State<_AiProviderModelMenuItem> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final colors = widget.colors;
    final background = _hovered
        ? colors.inputBackground
        : widget.selected
        ? colors.accent.withValues(alpha: 0.10)
        : Colors.transparent;
    return MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: Material(
        color: background,
        borderRadius: BorderRadius.circular(6),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: widget.onPressed,
          hoverColor: Colors.transparent,
          splashColor: colors.accent.withValues(alpha: 0.14),
          highlightColor: colors.accent.withValues(alpha: 0.08),
          child: SizedBox(
            height: 44,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 9),
              child: Row(
                children: [
                  Expanded(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          widget.option.providerName,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            color: colors.muted,
                            fontSize: 10.5,
                            fontWeight: NautermFontWeights.medium,
                            height: 1.1,
                            letterSpacing: 0,
                            decoration: TextDecoration.none,
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          widget.option.model,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            color: colors.foreground,
                            fontSize: 11.5,
                            fontWeight: NautermFontWeights.medium,
                            height: 1.1,
                            letterSpacing: 0,
                            decoration: TextDecoration.none,
                          ),
                        ),
                      ],
                    ),
                  ),
                  if (widget.selected) ...[
                    const SizedBox(width: 8),
                    Icon(LucideIcons.check, size: 15, color: colors.accent),
                  ],
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
