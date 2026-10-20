part of 'nauterm_workspace.dart';

class _TerminalToolDropdown<T> extends StatefulWidget {
  const _TerminalToolDropdown({
    super.key,
    required this.value,
    required this.options,
    required this.colors,
    required this.fontSize,
    required this.fontWeight,
    required this.onChanged,
    this.outlined = false,
    this.filterable = false,
    this.customValueBuilder,
  });

  final T value;
  final List<NautermContextMenuAction<T>> options;
  final _AiAssistantColors colors;
  final double fontSize;
  final FontWeight fontWeight;
  final ValueChanged<T> onChanged;
  final bool outlined;
  final bool filterable;
  final T Function(String query)? customValueBuilder;

  @override
  State<_TerminalToolDropdown<T>> createState() =>
      _TerminalToolDropdownState<T>();
}

class _TerminalToolDropdownState<T> extends State<_TerminalToolDropdown<T>> {
  final GlobalKey _fieldKey = GlobalKey();
  late final TextEditingController _inputController;
  late String _committedLabel;
  VoidCallback? _dismissMenu;
  bool _hovered = false;
  bool _focused = false;
  bool _open = false;

  String get _selectedLabel =>
      widget.options
          .where((option) => option.value == widget.value)
          .firstOrNull
          ?.label ??
      widget.value.toString();

  @override
  void initState() {
    super.initState();
    _committedLabel = _selectedLabel;
    _inputController = TextEditingController(text: _committedLabel);
  }

  @override
  void didUpdateWidget(covariant _TerminalToolDropdown<T> oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.value != widget.value) {
      _committedLabel = _selectedLabel;
      if (!_open) {
        _inputController.text = _committedLabel;
      }
    }
  }

  @override
  void dispose() {
    _inputController.dispose();
    super.dispose();
  }

  Future<void> _showMenu() async {
    if (_open) {
      return;
    }
    final fieldBox = _fieldKey.currentContext?.findRenderObject() as RenderBox?;
    final overlayBox =
        Overlay.of(context).context.findRenderObject() as RenderBox?;
    if (fieldBox == null || overlayBox == null) {
      return;
    }
    final anchor =
        fieldBox.localToGlobal(Offset.zero, ancestor: overlayBox) &
        fieldBox.size;
    if (widget.filterable) {
      _inputController.selection = TextSelection(
        baseOffset: 0,
        extentOffset: _inputController.text.length,
      );
    }
    setState(() => _open = true);
    final selected = await _showTerminalToolDropdownMenu<T>(
      context: context,
      anchor: anchor,
      value: widget.value,
      options: widget.options,
      colors: widget.colors,
      fontSize: widget.fontSize,
      fontWeight: widget.fontWeight,
      selectedLabel: _committedLabel,
      filterable: widget.filterable,
      customValueBuilder: widget.customValueBuilder,
      searchController: _inputController,
      onOpened: (dismiss) => _dismissMenu = dismiss,
    );
    if (!mounted) {
      return;
    }
    _dismissMenu = null;
    if (selected != null) {
      _committedLabel =
          widget.options
              .where((option) => option.value == selected)
              .firstOrNull
              ?.label ??
          selected.toString();
      if (selected != widget.value) {
        widget.onChanged(selected);
      }
    }
    setState(() {
      _open = false;
      _inputController.text = _committedLabel;
    });
  }

  void _submitInput(String rawValue) {
    final query = rawValue.trim();
    if (query.isEmpty) {
      return;
    }
    final exact = widget.options
        .where(
          (option) => option.label.trim().toLowerCase() == query.toLowerCase(),
        )
        .firstOrNull;
    final nextValue = exact?.value ?? widget.customValueBuilder?.call(query);
    if (nextValue != null) {
      _committedLabel = exact?.label ?? query;
      if (nextValue != widget.value) {
        widget.onChanged(nextValue);
      }
    }
    _dismissMenu?.call();
  }

  @override
  Widget build(BuildContext context) {
    final selected = widget.options
        .where((option) => option.value == widget.value)
        .firstOrNull;
    final colors = widget.colors;
    final baseColor = widget.outlined
        ? colors.inputBackground
        : colors.background;
    final dark =
        ThemeData.estimateBrightnessForColor(baseColor) == Brightness.dark;
    final hoverColor = Color.alphaBlend(
      (dark ? Colors.white : Colors.black).withValues(alpha: 0.06),
      baseColor,
    );
    final radius = widget.outlined ? 8.0 : 6.0;

    final content = MouseRegion(
      cursor: SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: AnimatedContainer(
        key: _fieldKey,
        duration: MediaQuery.disableAnimationsOf(context)
            ? Duration.zero
            : const Duration(milliseconds: 150),
        curve: Curves.easeInOut,
        height: widget.outlined ? 34 : 26,
        decoration: BoxDecoration(
          color: _hovered || _open
              ? hoverColor
              : widget.outlined
              ? colors.inputBackground
              : Colors.transparent,
          borderRadius: BorderRadius.circular(radius),
          border: widget.outlined
              ? Border.all(
                  color: _open || _focused ? colors.accent : colors.border,
                )
              : null,
        ),
        clipBehavior: Clip.antiAlias,
        child: Material(
          type: MaterialType.transparency,
          child: InkWell(
            borderRadius: BorderRadius.circular(radius),
            hoverColor: Colors.transparent,
            splashColor: colors.accent.withValues(alpha: 0.16),
            highlightColor: colors.accent.withValues(alpha: 0.08),
            onTap: widget.filterable ? null : _showMenu,
            child: Padding(
              padding: EdgeInsets.symmetric(
                horizontal: widget.outlined ? 10 : 7,
              ),
              child: Row(
                children: [
                  Expanded(
                    child: widget.filterable
                        ? TextField(
                            key: const ValueKey('terminal-tool-dropdown-input'),
                            controller: _inputController,
                            onTap: _showMenu,
                            onChanged: (_) {
                              if (!_open) {
                                unawaited(_showMenu());
                              }
                            },
                            onSubmitted: _submitInput,
                            textInputAction: TextInputAction.done,
                            style: TextStyle(
                              color: colors.foreground,
                              fontSize: widget.fontSize,
                              fontWeight: widget.fontWeight,
                              height: 1.2,
                              letterSpacing: 0,
                            ),
                            decoration: const InputDecoration.collapsed(
                              hintText: null,
                            ),
                          )
                        : Text(
                            selected?.label ?? widget.value.toString(),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: colors.foreground,
                              fontSize: widget.fontSize,
                              fontWeight: widget.fontWeight,
                              letterSpacing: 0,
                            ),
                          ),
                  ),
                  SizedBox(width: 5),
                  Icon(
                    _open ? LucideIcons.chevronUp : LucideIcons.chevronDown,
                    size: widget.outlined ? 15 : 16,
                    color: colors.muted,
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    final focusableContent = Focus(
      canRequestFocus: false,
      onFocusChange: (focused) => setState(() => _focused = focused),
      child: content,
    );
    if (!widget.filterable) {
      return focusableContent;
    }
    return Listener(
      behavior: HitTestBehavior.opaque,
      onPointerDown: (_) {
        if (!_open) {
          unawaited(_showMenu());
        }
      },
      child: focusableContent,
    );
  }
}

Future<T?> _showTerminalToolDropdownMenu<T>({
  required BuildContext context,
  required Rect anchor,
  required T value,
  required List<NautermContextMenuAction<T>> options,
  required _AiAssistantColors colors,
  required double fontSize,
  required FontWeight fontWeight,
  required String selectedLabel,
  bool filterable = false,
  T Function(String query)? customValueBuilder,
  TextEditingController? searchController,
  ValueChanged<VoidCallback>? onOpened,
}) {
  final completer = Completer<T?>();
  late final NautermTransientOverlayHandle handle;
  var completed = false;

  void complete(T? result, {bool dismiss = true}) {
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
    builder: (context) => _TerminalToolDropdownMenuOverlay<T>(
      anchor: anchor,
      value: value,
      options: options,
      colors: colors,
      fontSize: fontSize,
      fontWeight: fontWeight,
      selectedLabel: selectedLabel,
      filterable: filterable,
      customValueBuilder: customValueBuilder,
      searchController: searchController,
      onSelected: complete,
      onDismissed: () => complete(null),
    ),
  );
  onOpened?.call(() => complete(null));
  return completer.future;
}

class _TerminalToolDropdownMenuOverlay<T> extends StatefulWidget {
  const _TerminalToolDropdownMenuOverlay({
    required this.anchor,
    required this.value,
    required this.options,
    required this.colors,
    required this.fontSize,
    required this.fontWeight,
    required this.selectedLabel,
    required this.filterable,
    required this.customValueBuilder,
    required this.searchController,
    required this.onSelected,
    required this.onDismissed,
  });

  final Rect anchor;
  final T value;
  final List<NautermContextMenuAction<T>> options;
  final _AiAssistantColors colors;
  final double fontSize;
  final FontWeight fontWeight;
  final String selectedLabel;
  final bool filterable;
  final T Function(String query)? customValueBuilder;
  final TextEditingController? searchController;
  final ValueChanged<T> onSelected;
  final VoidCallback onDismissed;

  @override
  State<_TerminalToolDropdownMenuOverlay<T>> createState() =>
      _TerminalToolDropdownMenuOverlayState<T>();
}

class _TerminalToolDropdownMenuOverlayState<T>
    extends State<_TerminalToolDropdownMenuOverlay<T>> {
  Rect get anchor => widget.anchor;
  T get value => widget.value;
  _AiAssistantColors get colors => widget.colors;
  double get fontSize => widget.fontSize;
  FontWeight get fontWeight => widget.fontWeight;
  bool get filterable => widget.filterable;
  ValueChanged<T> get onSelected => widget.onSelected;
  VoidCallback get onDismissed => widget.onDismissed;

  List<NautermContextMenuAction<T>> get options {
    final query = widget.searchController?.text.trim() ?? '';
    if (!filterable || query.isEmpty) {
      return widget.options;
    }
    final normalized = query.toLowerCase();
    if (normalized == widget.selectedLabel.trim().toLowerCase()) {
      return widget.options;
    }
    final matches = widget.options
        .where((option) => option.label.toLowerCase().contains(normalized))
        .toList(growable: true);
    final exact = matches.any(
      (option) => option.label.trim().toLowerCase() == normalized,
    );
    if (!exact && widget.customValueBuilder != null) {
      matches.insert(
        0,
        NautermContextMenuAction<T>(
          value: widget.customValueBuilder!(query),
          label: query,
        ),
      );
    }
    return matches;
  }

  @override
  void initState() {
    super.initState();
    widget.searchController?.addListener(_handleSearchChanged);
  }

  @override
  void dispose() {
    widget.searchController?.removeListener(_handleSearchChanged);
    super.dispose();
  }

  void _handleSearchChanged() {
    if (mounted) {
      setState(() {});
    }
  }

  @override
  Widget build(BuildContext context) {
    final overlaySize = MediaQuery.sizeOf(context);
    final safePadding = nautermTransientOverlaySafePadding(
      context,
      base: MediaQuery.paddingOf(context),
    );
    const margin = 8.0;
    const gap = 4.0;
    const rowHeight = 32.0;
    const menuPadding = 6.0;
    const menuBorder = 2.0;
    const maximumVisibleRows = 7;
    final optionStyle = TextStyle(
      fontSize: fontSize,
      fontWeight: fontWeight,
      height: 1.2,
      letterSpacing: 0,
    );
    final longestLabelWidth = options.fold<double>(
      0,
      (width, option) => math.max(
        width,
        _measureWorkspaceSelectorText(context, option.label, optionStyle),
      ),
    );
    final maximumWidth = math.max(
      1.0,
      overlaySize.width - safePadding.horizontal - margin * 2,
    );
    final menuWidth = math
        .max(anchor.width, longestLabelWidth + 48)
        .clamp(1.0, maximumWidth)
        .toDouble();
    final contentHeight =
        menuBorder + menuPadding * 2 + options.length * rowHeight;
    final desiredHeight = math.min(
      contentHeight,
      menuBorder + menuPadding * 2 + maximumVisibleRows * rowHeight,
    );
    final availableBelow =
        overlaySize.height - safePadding.bottom - margin - anchor.bottom - gap;
    final availableAbove = anchor.top - safePadding.top - margin - gap;
    final openAbove =
        availableBelow < desiredHeight && availableAbove > availableBelow;
    final availableHeight = math.max(
      rowHeight + menuPadding * 2,
      openAbove ? availableAbove : availableBelow,
    );
    final menuHeight = math.min(desiredHeight, availableHeight);
    final scrollable = menuHeight < contentHeight;
    final minLeft = safePadding.left + margin;
    final maxLeft = math.max(
      minLeft,
      overlaySize.width - safePadding.right - menuWidth - margin,
    );
    final left = anchor.left.clamp(minLeft, maxLeft).toDouble();
    final top = openAbove ? anchor.top - gap - menuHeight : anchor.bottom + gap;
    final rect = positionNautermTransientOverlay(
      context: context,
      preferredRect: Rect.fromLTWH(left, top, menuWidth, menuHeight),
      overlaySize: overlaySize,
      safePadding: safePadding,
    );

    Widget item(NautermContextMenuAction<T> option) {
      return _TerminalToolDropdownMenuItem<T>(
        option: option,
        selected: option.value == value,
        colors: colors,
        fontSize: fontSize,
        fontWeight: fontWeight,
        onPressed: () => onSelected(option.value),
      );
    }

    return Focus(
      autofocus: !filterable,
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
                    offset: Offset(0, (1 - animation) * 3),
                    child: child,
                  ),
                );
              },
              child: Container(
                key: const ValueKey('terminal-tool-dropdown-menu'),
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
                      blurRadius: 20,
                      offset: const Offset(0, 9),
                    ),
                  ],
                ),
                clipBehavior: Clip.antiAlias,
                child: Material(
                  type: MaterialType.transparency,
                  child: scrollable
                      ? _TerminalToolDropdownScrollableMenu<T>(
                          key: ValueKey(
                            'terminal-tool-dropdown-options:'
                            '${widget.searchController?.text}',
                          ),
                          options: options,
                          value: value,
                          colors: colors,
                          fontSize: fontSize,
                          fontWeight: fontWeight,
                          rowHeight: rowHeight,
                          menuPadding: menuPadding,
                          viewportHeight: menuHeight,
                          onSelected: onSelected,
                        )
                      : Padding(
                          padding: const EdgeInsets.all(menuPadding),
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              for (final option in options) item(option),
                            ],
                          ),
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

class _TerminalToolDropdownScrollableMenu<T> extends StatefulWidget {
  const _TerminalToolDropdownScrollableMenu({
    super.key,
    required this.options,
    required this.value,
    required this.colors,
    required this.fontSize,
    required this.fontWeight,
    required this.rowHeight,
    required this.menuPadding,
    required this.viewportHeight,
    required this.onSelected,
  });

  final List<NautermContextMenuAction<T>> options;
  final T value;
  final _AiAssistantColors colors;
  final double fontSize;
  final FontWeight fontWeight;
  final double rowHeight;
  final double menuPadding;
  final double viewportHeight;
  final ValueChanged<T> onSelected;

  @override
  State<_TerminalToolDropdownScrollableMenu<T>> createState() =>
      _TerminalToolDropdownScrollableMenuState<T>();
}

class _TerminalToolDropdownScrollableMenuState<T>
    extends State<_TerminalToolDropdownScrollableMenu<T>> {
  late final ScrollController _scrollController;

  @override
  void initState() {
    super.initState();
    final selectedIndex = widget.options.indexWhere(
      (option) => option.value == widget.value,
    );
    final contentViewport = math.max(
      widget.rowHeight,
      widget.viewportHeight - widget.menuPadding * 2,
    );
    final maximumOffset = math.max(
      0.0,
      widget.options.length * widget.rowHeight - contentViewport,
    );
    final centeredOffset = selectedIndex < 0
        ? 0.0
        : selectedIndex * widget.rowHeight -
              (contentViewport - widget.rowHeight) / 2;
    _scrollController = ScrollController(
      initialScrollOffset: centeredOffset.clamp(0.0, maximumOffset),
    );
  }

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: EdgeInsets.symmetric(vertical: widget.menuPadding),
      child: ListView.builder(
        key: const ValueKey('terminal-tool-dropdown-menu-scroll'),
        controller: _scrollController,
        padding: EdgeInsets.zero,
        itemExtent: widget.rowHeight,
        itemCount: widget.options.length,
        itemBuilder: (context, index) {
          final option = widget.options[index];
          return Padding(
            padding: EdgeInsets.symmetric(horizontal: widget.menuPadding),
            child: _TerminalToolDropdownMenuItem<T>(
              option: option,
              selected: option.value == widget.value,
              colors: widget.colors,
              fontSize: widget.fontSize,
              fontWeight: widget.fontWeight,
              onPressed: () => widget.onSelected(option.value),
            ),
          );
        },
      ),
    );
  }
}

class _TerminalToolDropdownMenuItem<T> extends StatefulWidget {
  const _TerminalToolDropdownMenuItem({
    required this.option,
    required this.selected,
    required this.colors,
    required this.fontSize,
    required this.fontWeight,
    required this.onPressed,
  });

  final NautermContextMenuAction<T> option;
  final bool selected;
  final _AiAssistantColors colors;
  final double fontSize;
  final FontWeight fontWeight;
  final VoidCallback onPressed;

  @override
  State<_TerminalToolDropdownMenuItem<T>> createState() =>
      _TerminalToolDropdownMenuItemState<T>();
}

class _TerminalToolDropdownMenuItemState<T>
    extends State<_TerminalToolDropdownMenuItem<T>> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final option = widget.option;
    final colors = widget.colors;
    final destructive = const Color(0xffef4444);
    final foreground = !option.enabled
        ? colors.muted.withValues(alpha: 0.38)
        : option.destructive
        ? destructive
        : widget.selected
        ? colors.accent
        : colors.foreground;
    final background = _hovered
        ? option.destructive
              ? destructive.withValues(alpha: 0.12)
              : colors.inputBackground
        : widget.selected
        ? colors.accent.withValues(alpha: 0.10)
        : Colors.transparent;

    return MouseRegion(
      cursor: option.enabled
          ? SystemMouseCursors.click
          : SystemMouseCursors.basic,
      onEnter: option.enabled ? (_) => setState(() => _hovered = true) : null,
      onExit: option.enabled ? (_) => setState(() => _hovered = false) : null,
      child: Material(
        color: background,
        borderRadius: BorderRadius.circular(6),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: option.enabled ? widget.onPressed : null,
          hoverColor: Colors.transparent,
          splashColor: colors.accent.withValues(alpha: 0.14),
          highlightColor: colors.accent.withValues(alpha: 0.08),
          child: SizedBox(
            height: 32,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 9),
              child: Row(
                children: [
                  if (option.icon != null) ...[
                    Icon(option.icon, size: 14, color: foreground),
                    const SizedBox(width: 7),
                  ],
                  Expanded(
                    child: Text(
                      option.label,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: foreground,
                        fontSize: widget.fontSize,
                        fontWeight: widget.fontWeight,
                        height: 1.2,
                        letterSpacing: 0,
                      ),
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
