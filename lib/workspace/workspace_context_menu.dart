part of 'nauterm_workspace.dart';

class _WorkspaceItemContextMenuOverlay<T extends _WorkspaceItemData>
    extends StatefulWidget {
  const _WorkspaceItemContextMenuOverlay({
    required this.items,
    required this.anchor,
    required this.overlaySize,
    required this.onDismissed,
    this.currentWorkspaceName,
    this.onAction,
  });

  final List<T> items;
  final Offset anchor;
  final Size overlaySize;
  final VoidCallback onDismissed;
  final String? currentWorkspaceName;
  final ValueChanged<_ContextMenuActionId>? onAction;

  @override
  State<_WorkspaceItemContextMenuOverlay<T>> createState() =>
      _WorkspaceItemContextMenuOverlayState<T>();
}

class _WorkspaceItemContextMenuOverlayState<T extends _WorkspaceItemData>
    extends State<_WorkspaceItemContextMenuOverlay<T>> {
  bool _visible = false;
  bool _dismissed = false;
  _ContextMenuAction? _openSubmenu;
  Rect? _menuRect;
  Rect? _submenuRect;
  final NautermSubmenuAimController _submenuAimController =
      NautermSubmenuAimController();

  @override
  void initState() {
    super.initState();
    GestureBinding.instance.pointerRouter.addGlobalRoute(_handleGlobalPointer);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        setState(() => _visible = true);
      }
    });
  }

  @override
  void dispose() {
    _submenuAimController.dispose();
    GestureBinding.instance.pointerRouter.removeGlobalRoute(
      _handleGlobalPointer,
    );
    super.dispose();
  }

  void _handleGlobalPointer(PointerEvent event) {
    if (event is PointerHoverEvent) {
      _submenuAimController.trackPointer(event.position);
      return;
    }
    if (event is! PointerDownEvent || _dismissed) {
      return;
    }

    final position = event.position;
    if ((_menuRect?.contains(position) ?? false) ||
        (_submenuRect?.contains(position) ?? false)) {
      return;
    }

    final secondaryClick = (event.buttons & kSecondaryMouseButton) != 0;
    _dismiss(immediate: secondaryClick);
  }

  void _dismiss({bool immediate = false}) {
    if (_dismissed) {
      return;
    }
    _dismissed = true;
    _submenuAimController.cancel();
    if (immediate || !_visible) {
      widget.onDismissed();
      return;
    }

    setState(() {
      _visible = false;
      _openSubmenu = null;
    });
    Future<void>.delayed(_contextMenuAnimationDuration, () {
      if (mounted) {
        widget.onDismissed();
      }
    });
  }

  void _setOpenSubmenu(_ContextMenuAction? row, Offset pointerPosition) {
    final current = _openSubmenu;
    if (current != null &&
        row != null &&
        _sameContextMenuAction(current, row)) {
      _submenuAimController.cancel();
      return;
    }

    void change() {
      if (mounted) setState(() => _openSubmenu = row);
    }

    if (current == null || _submenuRect == null) {
      _submenuAimController.cancel();
      change();
      return;
    }
    _submenuAimController.applyOrDefer(
      pointerPosition: pointerPosition,
      submenuRect: _submenuRect!,
      change: change,
    );
  }

  void _openSubmenuImmediately(_ContextMenuAction row) {
    _submenuAimController.cancel();
    final current = _openSubmenu;
    if (current == null || !_sameContextMenuAction(current, row)) {
      setState(() => _openSubmenu = row);
    }
  }

  @override
  Widget build(BuildContext context) {
    final rows = _contextMenuRowsForSelection(
      widget.items,
      currentWorkspaceName: widget.currentWorkspaceName,
    );
    final menuHeight = _contextMenuHeightForRows(rows);
    final preferredMenuRect = Rect.fromLTWH(
      widget.anchor.dx
          .clamp(8, widget.overlaySize.width - _contextMenuWidth - 8)
          .toDouble(),
      widget.anchor.dy
          .clamp(8, widget.overlaySize.height - menuHeight - 8)
          .toDouble(),
      _contextMenuWidth,
      menuHeight,
    );
    final menuRect = positionNautermTransientOverlay(
      context: context,
      preferredRect: preferredMenuRect,
      overlaySize: widget.overlaySize,
    );
    final left = menuRect.left;
    final top = menuRect.top;
    final submenu = _openSubmenu;
    _menuRect = menuRect;
    _submenuRect = submenu == null
        ? null
        : positionNautermTransientOverlay(
            context: context,
            preferredRect: _submenuRectFor(
              rows: rows,
              submenu: submenu,
              menuLeft: left,
              menuTop: top,
              overlaySize: widget.overlaySize,
            ),
            overlaySize: widget.overlaySize,
          );

    return Stack(
      children: [
        Positioned(
          left: left,
          top: top,
          child: _ContextMenuMotion(
            visible: _visible,
            child: _WorkspaceItemContextMenu(
              rows: rows,
              openSubmenu: submenu,
              onDismiss: _dismiss,
              onSubmenuChanged: _setOpenSubmenu,
              onSubmenuTapped: _openSubmenuImmediately,
              onAction: widget.onAction,
            ),
          ),
        ),
        if (submenu != null)
          _ContextSubmenuPositioned(
            rows: rows,
            submenu: submenu,
            menuLeft: left,
            menuTop: top,
            overlaySize: widget.overlaySize,
            visible: _visible,
            onDismiss: _dismiss,
            onPointerEntered: _submenuAimController.cancel,
            onAction: widget.onAction,
          ),
      ],
    );
  }
}

class _WorkspaceItemContextMenu extends StatelessWidget {
  const _WorkspaceItemContextMenu({
    required this.rows,
    required this.openSubmenu,
    required this.onDismiss,
    required this.onSubmenuChanged,
    required this.onSubmenuTapped,
    this.onAction,
  });

  final List<Object> rows;
  final _ContextMenuAction? openSubmenu;
  final VoidCallback onDismiss;
  final void Function(_ContextMenuAction? row, Offset pointerPosition)
  onSubmenuChanged;
  final ValueChanged<_ContextMenuAction> onSubmenuTapped;
  final ValueChanged<_ContextMenuActionId>? onAction;

  @override
  Widget build(BuildContext context) {
    return _ContextMenuSurface(
      width: _contextMenuWidth,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final row in rows)
            if (row == _MenuDivider.instance)
              const _ContextMenuDivider()
            else
              _ContextMenuRow(
                row: row as _ContextMenuAction,
                active:
                    openSubmenu != null &&
                    _sameContextMenuAction(row, openSubmenu!),
                onPointerEntered: (position) =>
                    onSubmenuChanged(row.hasSubmenu ? row : null, position),
                onTap: () {
                  if (row.hasSubmenu) {
                    onSubmenuTapped(row);
                    return;
                  }
                  final id = row.id;
                  if (id != null) {
                    onAction?.call(id);
                  }
                  onDismiss();
                },
              ),
        ],
      ),
    );
  }
}

class _ContextMenuRow extends StatelessWidget {
  const _ContextMenuRow({
    required this.row,
    required this.active,
    required this.onPointerEntered,
    required this.onTap,
  });

  final _ContextMenuAction row;
  final bool active;
  final ValueChanged<Offset> onPointerEntered;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final color = row.destructive ? const Color(0xffef3f37) : _text;
    final shortcutColor = row.destructive
        ? const Color(0xffef3f37)
        : _workspaceMenuDisabledText;

    return MouseRegion(
      onEnter: (event) => onPointerEntered(event.position),
      child: SizedBox(
        height: _contextMenuRowHeight,
        child: Material(
          color: active ? _workspaceMenuHover : Colors.transparent,
          borderRadius: BorderRadius.circular(7),
          child: InkWell(
            borderRadius: BorderRadius.circular(7),
            hoverColor: row.destructive
                ? const Color(0xffef3f37).withValues(alpha: 0.12)
                : _workspaceMenuHover,
            splashColor: Colors.transparent,
            highlightColor: Colors.transparent,
            onTap: onTap,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8),
              child: Row(
                children: [
                  Icon(row.icon, size: 17, color: color),
                  SizedBox(width: 14),
                  Expanded(
                    child: Text(
                      row.localizedLabel,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: color,
                        fontSize: NautermFontSizes.labelLarge,
                        fontWeight: NautermFontWeights.semibold,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                  if (row.displayShortcut != null)
                    _ShortcutBadge(
                      label: row.displayShortcut!,
                      color: shortcutColor,
                      destructive: row.destructive,
                    )
                  else if (row.hasSubmenu)
                    Icon(Icons.chevron_right_rounded, size: 18, color: color),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _ContextSubmenuPositioned extends StatelessWidget {
  const _ContextSubmenuPositioned({
    required this.rows,
    required this.submenu,
    required this.menuLeft,
    required this.menuTop,
    required this.overlaySize,
    required this.visible,
    required this.onDismiss,
    required this.onPointerEntered,
    this.onAction,
  });

  final List<Object> rows;
  final _ContextMenuAction submenu;
  final double menuLeft;
  final double menuTop;
  final Size overlaySize;
  final bool visible;
  final VoidCallback onDismiss;
  final VoidCallback onPointerEntered;
  final ValueChanged<_ContextMenuActionId>? onAction;

  @override
  Widget build(BuildContext context) {
    final rect = positionNautermTransientOverlay(
      context: context,
      preferredRect: _submenuRectFor(
        rows: rows,
        submenu: submenu,
        menuLeft: menuLeft,
        menuTop: menuTop,
        overlaySize: overlaySize,
      ),
      overlaySize: overlaySize,
    );

    return Positioned(
      left: rect.left,
      top: rect.top,
      child: MouseRegion(
        onEnter: (_) => onPointerEntered(),
        child: _ContextMenuMotion(
          visible: visible,
          child: _ContextMenuSurface(
            width: _contextSubmenuWidth,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final row in submenu.submenuActions)
                  _ContextMenuRow(
                    row: row,
                    active: false,
                    onPointerEntered: (_) {},
                    onTap: () {
                      final id = row.id;
                      if (id != null) {
                        onAction?.call(id);
                      }
                      onDismiss();
                    },
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

Rect _submenuRectFor({
  required List<Object> rows,
  required _ContextMenuAction submenu,
  required double menuLeft,
  required double menuTop,
  required Size overlaySize,
}) {
  final submenuHeight = _contextMenuHeightForRows(submenu.submenuActions);
  final rowTop = _contextMenuTopForRow(rows, submenu);
  final menuRight = menuLeft + _contextMenuWidth;
  final rightLeft = menuRight + 6;
  final leftLeft = menuLeft - _contextSubmenuWidth - 6;
  final maxLeft = overlaySize.width - _contextSubmenuWidth - 8;
  final canOpenRight = rightLeft <= maxLeft;
  final canOpenLeft = leftLeft >= 8;
  final availableRight = overlaySize.width - menuRight - 8;
  final availableLeft = menuLeft - 8;
  final preferredLeft = canOpenRight
      ? rightLeft
      : canOpenLeft
      ? leftLeft
      : availableRight >= availableLeft
      ? rightLeft
      : leftLeft;
  final left = _clampContextMenuPosition(preferredLeft, 8, maxLeft);
  final top = _clampContextMenuPosition(
    menuTop + rowTop,
    8,
    overlaySize.height - submenuHeight - 8,
  );

  return Rect.fromLTWH(left, top, _contextSubmenuWidth, submenuHeight);
}

double _clampContextMenuPosition(double value, double min, double max) {
  if (max < min) {
    return min;
  }
  return value.clamp(min, max).toDouble();
}

class _ContextMenuMotion extends StatelessWidget {
  const _ContextMenuMotion({required this.visible, required this.child});

  final bool visible;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return AnimatedOpacity(
      opacity: visible ? 1 : 0,
      duration: _contextMenuAnimationDuration,
      curve: Curves.easeOutCubic,
      child: AnimatedScale(
        scale: visible ? 1 : 0.985,
        alignment: Alignment.topLeft,
        duration: _contextMenuAnimationDuration,
        curve: Curves.easeOutCubic,
        child: child,
      ),
    );
  }
}

class _ContextMenuSurface extends StatelessWidget {
  const _ContextMenuSurface({required this.width, required this.child});

  final double width;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: Container(
        width: width,
        padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 10),
        decoration: BoxDecoration(
          color: _workspaceMenuBackground.withValues(alpha: 0.98),
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: _workspaceMenuBorder),
          boxShadow: _workspaceMenuShadows,
        ),
        child: child,
      ),
    );
  }
}

class _ShortcutBadge extends StatelessWidget {
  const _ShortcutBadge({
    required this.label,
    required this.color,
    required this.destructive,
  });

  final String label;
  final Color color;
  final bool destructive;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 24,
      constraints: const BoxConstraints(minWidth: 38),
      padding: const EdgeInsets.symmetric(horizontal: 7),
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: destructive
            ? const Color(0xffef3f37).withValues(alpha: 0.12)
            : _workspaceMenuHover,
        borderRadius: BorderRadius.circular(6),
      ),
      child: Text(
        tr(label),
        style: TextStyle(
          color: color,
          fontSize: NautermFontSizes.labelMedium,
          fontWeight: NautermFontWeights.semibold,
          letterSpacing: 0,
        ),
      ),
    );
  }
}

class _ContextMenuDivider extends StatelessWidget {
  const _ContextMenuDivider();

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: EdgeInsets.symmetric(vertical: 6),
      child: Divider(height: 1, color: _workspaceMenuBorder),
    );
  }
}
