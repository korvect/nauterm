import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app/nauterm_theme.dart';
import '../app/nauterm_localizations.dart';
import 'nauterm_overlay.dart';

const double nautermContextMenuRowHeight = 32;
const double nautermContextMenuDividerHeight = 9;
const double nautermContextMenuVerticalPadding = 6;
const int nautermDropdownMenuMaxVisibleRows = 7;
const Duration nautermContextMenuAnimationDuration = Duration(milliseconds: 84);

class NautermSubmenuAimController {
  static const _intentTimeout = Duration(milliseconds: 300);
  static const _pointerHistoryLimit = 8;
  static const _aimTolerance = 8.0;

  final List<Offset> _pointerHistory = [];
  Timer? _intentTimer;
  VoidCallback? _pendingChange;
  _NautermSubmenuAim? _aim;

  void trackPointer(Offset position) {
    _pointerHistory.add(position);
    if (_pointerHistory.length > _pointerHistoryLimit) {
      _pointerHistory.removeRange(
        0,
        _pointerHistory.length - _pointerHistoryLimit,
      );
    }

    final change = _pendingChange;
    final aim = _aim;
    if (change == null || aim == null || _isInsideSubmenuAim(position, aim)) {
      return;
    }
    cancel();
    change();
  }

  void applyOrDefer({
    required Offset pointerPosition,
    required Rect submenuRect,
    required VoidCallback change,
  }) {
    cancel();
    final origin = _aimOrigin(pointerPosition);
    if (origin == null ||
        !_setAimIfMovingTowardSubmenu(
          origin: origin,
          pointerPosition: pointerPosition,
          submenuRect: submenuRect,
        )) {
      change();
      return;
    }

    _pendingChange = change;
    _intentTimer = Timer(_intentTimeout, () {
      final pendingChange = _pendingChange;
      if (pendingChange == null) return;
      cancel();
      pendingChange();
    });
  }

  void cancel() {
    _intentTimer?.cancel();
    _intentTimer = null;
    _pendingChange = null;
    _aim = null;
  }

  void dispose() {
    cancel();
    _pointerHistory.clear();
  }

  Offset? _aimOrigin(Offset currentPosition) {
    for (final position in _pointerHistory.reversed) {
      if ((position - currentPosition).distance >= 4) return position;
    }
    return null;
  }

  bool _setAimIfMovingTowardSubmenu({
    required Offset origin,
    required Offset pointerPosition,
    required Rect submenuRect,
  }) {
    final opensRight = submenuRect.center.dx > origin.dx;
    final horizontalProgress = pointerPosition.dx - origin.dx;
    if ((opensRight && horizontalProgress <= 0) ||
        (!opensRight && horizontalProgress >= 0)) {
      return false;
    }
    final edgeX = opensRight ? submenuRect.left : submenuRect.right;
    final aim = _NautermSubmenuAim(
      origin: origin,
      upperCorner: Offset(edgeX, submenuRect.top - _aimTolerance),
      lowerCorner: Offset(edgeX, submenuRect.bottom + _aimTolerance),
    );
    if (!_isInsideSubmenuAim(pointerPosition, aim)) return false;
    _aim = aim;
    return true;
  }
}

@immutable
class NautermContextMenuStyle {
  const NautermContextMenuStyle({
    required this.background,
    required this.foreground,
    required this.mutedForeground,
    required this.disabledForeground,
    required this.border,
    required this.hoverBackground,
    required this.accent,
    this.destructive = const Color(0xffef4444),
    this.shadows = const [],
  });

  final Color background;
  final Color foreground;
  final Color mutedForeground;
  final Color disabledForeground;
  final Color border;
  final Color hoverBackground;
  final Color accent;
  final Color destructive;
  final List<BoxShadow> shadows;
}

sealed class NautermContextMenuEntry<T> {
  const NautermContextMenuEntry();
}

class NautermContextMenuAction<T> extends NautermContextMenuEntry<T> {
  const NautermContextMenuAction({
    required this.value,
    required this.label,
    this.icon,
    this.iconBytes,
    this.shortcut,
    this.enabled = true,
    this.destructive = false,
    this.selected = false,
    this.children = const [],
  });

  final T value;
  final String label;
  final IconData? icon;
  final Uint8List? iconBytes;
  final String? shortcut;
  final bool enabled;
  final bool destructive;
  final bool selected;
  final List<NautermContextMenuEntry<T>> children;

  bool get hasSubmenu => children.isNotEmpty;
}

class NautermContextMenuDivider<T> extends NautermContextMenuEntry<T> {
  const NautermContextMenuDivider();
}

double nautermContextMenuHeight<T>(
  Iterable<NautermContextMenuEntry<T>> entries,
) {
  return nautermContextMenuVerticalPadding * 2 +
      entries.fold<double>(
        0,
        (height, entry) =>
            height +
            (entry is NautermContextMenuDivider<T>
                ? nautermContextMenuDividerHeight
                : nautermContextMenuRowHeight),
      );
}

Rect nautermContextMenuRect({
  required Offset anchor,
  required Size overlaySize,
  required Size menuSize,
  EdgeInsets safePadding = EdgeInsets.zero,
  double margin = 8,
  double anchorGap = 6,
}) {
  final minLeft = safePadding.left + margin;
  final minTop = safePadding.top + margin;
  final maxLeft = math.max(
    minLeft,
    overlaySize.width - safePadding.right - menuSize.width - margin,
  );
  final maxTop = math.max(
    minTop,
    overlaySize.height - safePadding.bottom - menuSize.height - margin,
  );
  final fitsRight =
      anchor.dx + anchorGap + menuSize.width <=
      overlaySize.width - safePadding.right - margin;
  final fitsBelow =
      anchor.dy + anchorGap + menuSize.height <=
      overlaySize.height - safePadding.bottom - margin;
  final preferredLeft = fitsRight
      ? anchor.dx + anchorGap
      : anchor.dx - anchorGap - menuSize.width;
  final preferredTop = fitsBelow
      ? anchor.dy + anchorGap
      : anchor.dy - anchorGap - menuSize.height;

  return Rect.fromLTWH(
    preferredLeft.clamp(minLeft, maxLeft).toDouble(),
    preferredTop.clamp(minTop, maxTop).toDouble(),
    menuSize.width,
    menuSize.height,
  );
}

Future<T?> showNautermContextMenu<T>({
  required BuildContext context,
  required Offset position,
  required List<NautermContextMenuEntry<T>> entries,
  double width = 220,
  NautermContextMenuStyle? style,
  bool animate = true,
  bool scaleAnimation = false,
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
    builder: (context) => _NautermContextMenuOverlay<T>(
      position: position,
      entries: entries,
      width: width,
      style: style,
      animate: animate,
      scaleAnimation: scaleAnimation,
      onSelected: complete,
      onDismissed: () => complete(null),
    ),
  );

  return completer.future;
}

Future<T?> showNautermDropdownMenu<T>({
  required BuildContext context,
  required Rect anchor,
  required List<NautermContextMenuEntry<T>> entries,
  required double width,
  NautermContextMenuStyle? style,
  bool showScrollbarOnHover = false,
  bool scaleAnimation = false,
  double maxHeight =
      nautermContextMenuVerticalPadding * 2 +
      nautermContextMenuRowHeight * nautermDropdownMenuMaxVisibleRows,
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
    builder: (context) {
      final hasSubmenus = entries.any(
        (entry) => entry is NautermContextMenuAction<T> && entry.hasSubmenu,
      );
      if (hasSubmenus) {
        return _NautermContextMenuOverlay<T>(
          position: anchor.bottomLeft,
          dropdownAnchor: anchor,
          entries: entries,
          width: width,
          maxHeight: maxHeight,
          style: style,
          animate: true,
          scaleAnimation: scaleAnimation,
          showScrollbarOnHover: showScrollbarOnHover,
          onSelected: complete,
          onDismissed: () => complete(null),
        );
      }
      return _NautermDropdownMenuOverlay<T>(
        anchor: anchor,
        entries: entries,
        width: width,
        maxHeight: maxHeight,
        style: style,
        showScrollbarOnHover: showScrollbarOnHover,
        scaleAnimation: scaleAnimation,
        onSelected: complete,
        onDismissed: () => complete(null),
      );
    },
  );

  return completer.future;
}

class _NautermContextMenuOverlay<T> extends StatefulWidget {
  const _NautermContextMenuOverlay({
    required this.position,
    required this.entries,
    required this.width,
    required this.onSelected,
    required this.onDismissed,
    required this.animate,
    required this.scaleAnimation,
    this.style,
    this.dropdownAnchor,
    this.maxHeight,
    this.showScrollbarOnHover = false,
  });

  final Offset position;
  final List<NautermContextMenuEntry<T>> entries;
  final double width;
  final ValueChanged<T> onSelected;
  final VoidCallback onDismissed;
  final bool animate;
  final bool scaleAnimation;
  final NautermContextMenuStyle? style;
  final Rect? dropdownAnchor;
  final double? maxHeight;
  final bool showScrollbarOnHover;

  @override
  State<_NautermContextMenuOverlay<T>> createState() =>
      _NautermContextMenuOverlayState<T>();
}

class _NautermContextMenuOverlayState<T>
    extends State<_NautermContextMenuOverlay<T>> {
  final Object _tapRegionGroup = Object();
  final List<_NautermOpenSubmenu<T>> _openSubmenus = [];
  final NautermSubmenuAimController _submenuAimController =
      NautermSubmenuAimController();
  Size _overlaySize = Size.zero;
  EdgeInsets _safePadding = EdgeInsets.zero;

  void _cancelSubmenuIntent() {
    _submenuAimController.cancel();
  }

  Rect _submenuRectFor(_NautermOpenSubmenu<T> submenu) {
    final contentHeight = nautermContextMenuHeight(submenu.action.children);
    final height = math.min(
      contentHeight,
      _overlaySize.height - _safePadding.vertical - 16,
    );
    return _nautermSubmenuRect(
      anchor: submenu.anchor,
      overlaySize: _overlaySize,
      safePadding: _safePadding,
      width: widget.width,
      height: height,
    );
  }

  void _applyOrDeferSubmenuChange({
    required int menuDepth,
    required Offset pointerPosition,
    required VoidCallback change,
  }) {
    if (menuDepth >= _openSubmenus.length || _overlaySize == Size.zero) {
      change();
      return;
    }
    _submenuAimController.applyOrDefer(
      pointerPosition: pointerPosition,
      submenuRect: _submenuRectFor(_openSubmenus[menuDepth]),
      change: change,
    );
  }

  void _showSubmenu(
    int menuDepth,
    NautermContextMenuAction<T> action,
    Rect anchor,
    Offset pointerPosition,
  ) {
    final current = menuDepth < _openSubmenus.length
        ? _openSubmenus[menuDepth]
        : null;
    if (current?.action == action && current?.anchor == anchor) {
      _cancelSubmenuIntent();
      return;
    }

    void open() {
      if (!mounted) return;
      setState(() {
        if (_openSubmenus.length > menuDepth) {
          _openSubmenus.removeRange(menuDepth, _openSubmenus.length);
        }
        _openSubmenus.add(
          _NautermOpenSubmenu<T>(action: action, anchor: anchor),
        );
      });
    }

    if (current == null) {
      _cancelSubmenuIntent();
      open();
    } else {
      _applyOrDeferSubmenuChange(
        menuDepth: menuDepth,
        pointerPosition: pointerPosition,
        change: open,
      );
    }
  }

  void _handleActionHovered(
    int menuDepth,
    NautermContextMenuAction<T> action,
    Offset pointerPosition,
  ) {
    if (action.hasSubmenu || _openSubmenus.length <= menuDepth) return;
    void close() {
      if (!mounted || _openSubmenus.length <= menuDepth) return;
      setState(() {
        _openSubmenus.removeRange(menuDepth, _openSubmenus.length);
      });
    }

    _applyOrDeferSubmenuChange(
      menuDepth: menuDepth,
      pointerPosition: pointerPosition,
      change: close,
    );
  }

  void _closeDeepestSubmenu() {
    _cancelSubmenuIntent();
    setState(() {
      _openSubmenus.removeLast();
    });
  }

  @override
  void dispose() {
    _submenuAimController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final overlaySize = MediaQuery.sizeOf(context);
    final safePadding = nautermTransientOverlaySafePadding(
      context,
      base: MediaQuery.paddingOf(context),
    );
    _overlaySize = overlaySize;
    _safePadding = safePadding;
    final contentHeight = nautermContextMenuHeight(widget.entries);
    final overlayAvailableHeight = math.max(
      0.0,
      overlaySize.height - safePadding.vertical - 16,
    );
    final dropdownAnchor = widget.dropdownAnchor;
    late final double menuHeight;
    late final Rect preferredRect;
    if (dropdownAnchor == null) {
      menuHeight = math.min(contentHeight, overlayAvailableHeight);
      preferredRect = nautermContextMenuRect(
        anchor: widget.position,
        overlaySize: overlaySize,
        menuSize: Size(widget.width, menuHeight),
        safePadding: safePadding,
      );
    } else {
      const margin = 8.0;
      const gap = 4.0;
      final desiredHeight = math.min(
        contentHeight,
        widget.maxHeight ?? contentHeight,
      );
      final availableBelow = math.max(
        0.0,
        overlaySize.height -
            safePadding.bottom -
            margin -
            dropdownAnchor.bottom -
            gap,
      );
      final availableAbove = math.max(
        0.0,
        dropdownAnchor.top - safePadding.top - margin - gap,
      );
      final openAbove =
          availableBelow < desiredHeight && availableAbove > availableBelow;
      menuHeight = math.min(
        desiredHeight,
        math.min(
          overlayAvailableHeight,
          openAbove ? availableAbove : availableBelow,
        ),
      );
      final minLeft = safePadding.left + margin;
      final maxLeft = math.max(
        minLeft,
        overlaySize.width - safePadding.right - widget.width - margin,
      );
      final left = dropdownAnchor.left.clamp(minLeft, maxLeft).toDouble();
      final top = openAbove
          ? dropdownAnchor.top - gap - menuHeight
          : dropdownAnchor.bottom + gap;
      preferredRect = Rect.fromLTWH(left, top, widget.width, menuHeight);
    }
    final rect = positionNautermTransientOverlay(
      context: context,
      preferredRect: preferredRect,
      overlaySize: overlaySize,
      safePadding: safePadding,
    );

    return Focus(
      autofocus: true,
      onKeyEvent: (_, event) {
        if (event is KeyDownEvent &&
            event.logicalKey == LogicalKeyboardKey.escape) {
          if (_openSubmenus.isNotEmpty) {
            _closeDeepestSubmenu();
          } else {
            widget.onDismissed();
          }
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: Listener(
        onPointerHover: (event) =>
            _submenuAimController.trackPointer(event.position),
        child: Stack(
          children: [
            Positioned.fromRect(
              rect: rect,
              child: TapRegion(
                groupId: _tapRegionGroup,
                onTapOutside: (_) => widget.onDismissed(),
                child: MouseRegion(
                  onEnter: (_) => _cancelSubmenuIntent(),
                  child: _ContextMenuEntrance(
                    animate: widget.animate,
                    scaleAnimation: widget.scaleAnimation,
                    child: NautermContextMenu<T>(
                      entries: widget.entries,
                      width: widget.width,
                      height: menuHeight < contentHeight ? menuHeight : null,
                      style: widget.style,
                      showScrollbarOnHover:
                          widget.showScrollbarOnHover ||
                          menuHeight < contentHeight,
                      onSelected: widget.onSelected,
                      onSubmenuPointerRequested: (action, anchor, position) =>
                          _showSubmenu(0, action, anchor, position),
                      onActionPointerEntered: (action, position) =>
                          _handleActionHovered(0, action, position),
                    ),
                  ),
                ),
              ),
            ),
            for (var index = 0; index < _openSubmenus.length; index++)
              _buildSubmenu(
                context: context,
                submenu: _openSubmenus[index],
                menuDepth: index + 1,
                overlaySize: overlaySize,
                safePadding: safePadding,
              ),
          ],
        ),
      ),
    );
  }

  Widget _buildSubmenu({
    required BuildContext context,
    required _NautermOpenSubmenu<T> submenu,
    required int menuDepth,
    required Size overlaySize,
    required EdgeInsets safePadding,
  }) {
    final contentHeight = nautermContextMenuHeight(submenu.action.children);
    final height = math.min(
      contentHeight,
      overlaySize.height - safePadding.vertical - 16,
    );
    final preferredRect = _nautermSubmenuRect(
      anchor: submenu.anchor,
      overlaySize: overlaySize,
      safePadding: safePadding,
      width: widget.width,
      height: height,
    );
    final rect = positionNautermTransientOverlay(
      context: context,
      preferredRect: preferredRect,
      overlaySize: overlaySize,
      safePadding: safePadding,
    );

    return Positioned.fromRect(
      rect: rect,
      child: TapRegion(
        groupId: _tapRegionGroup,
        onTapOutside: (_) => widget.onDismissed(),
        child: MouseRegion(
          onEnter: (_) => _cancelSubmenuIntent(),
          child: _ContextMenuEntrance(
            animate: widget.animate,
            scaleAnimation: false,
            child: NautermContextMenu<T>(
              entries: submenu.action.children,
              width: widget.width,
              height: height < contentHeight ? height : null,
              style: widget.style,
              showScrollbarOnHover: true,
              onSelected: widget.onSelected,
              onSubmenuPointerRequested: (action, anchor, position) =>
                  _showSubmenu(menuDepth, action, anchor, position),
              onActionPointerEntered: (action, position) =>
                  _handleActionHovered(menuDepth, action, position),
            ),
          ),
        ),
      ),
    );
  }
}

class _NautermOpenSubmenu<T> {
  const _NautermOpenSubmenu({required this.action, required this.anchor});

  final NautermContextMenuAction<T> action;
  final Rect anchor;
}

class _NautermSubmenuAim {
  const _NautermSubmenuAim({
    required this.origin,
    required this.upperCorner,
    required this.lowerCorner,
  });

  final Offset origin;
  final Offset upperCorner;
  final Offset lowerCorner;
}

bool _isInsideSubmenuAim(Offset point, _NautermSubmenuAim aim) {
  final first = _triangleSide(point, aim.origin, aim.upperCorner);
  final second = _triangleSide(point, aim.upperCorner, aim.lowerCorner);
  final third = _triangleSide(point, aim.lowerCorner, aim.origin);
  final hasNegative = first < 0 || second < 0 || third < 0;
  final hasPositive = first > 0 || second > 0 || third > 0;
  return !(hasNegative && hasPositive);
}

double _triangleSide(Offset point, Offset start, Offset end) {
  return (point.dx - end.dx) * (start.dy - end.dy) -
      (start.dx - end.dx) * (point.dy - end.dy);
}

Rect _nautermSubmenuRect({
  required Rect anchor,
  required Size overlaySize,
  required EdgeInsets safePadding,
  required double width,
  required double height,
}) {
  const margin = 8.0;
  const gap = 4.0;
  final minLeft = safePadding.left + margin;
  final maxRight = overlaySize.width - safePadding.right - margin;
  final fitsRight = anchor.right + gap + width <= maxRight;
  final left = fitsRight ? anchor.right + gap : anchor.left - gap - width;
  final minTop = safePadding.top + margin;
  final maxTop = math.max(
    minTop,
    overlaySize.height - safePadding.bottom - margin - height,
  );
  return Rect.fromLTWH(
    left.clamp(minLeft, math.max(minLeft, maxRight - width)).toDouble(),
    (anchor.top - nautermContextMenuVerticalPadding)
        .clamp(minTop, maxTop)
        .toDouble(),
    width,
    height,
  );
}

class _ContextMenuEntrance extends StatelessWidget {
  const _ContextMenuEntrance({
    required this.animate,
    required this.scaleAnimation,
    required this.child,
  });

  final bool animate;
  final bool scaleAnimation;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (!animate) {
      return child;
    }
    return TweenAnimationBuilder<double>(
      tween: Tween<double>(begin: 0, end: 1),
      duration: nautermContextMenuAnimationDuration,
      curve: Curves.easeOutCubic,
      builder: (context, value, child) {
        return Opacity(
          opacity: value,
          child: scaleAnimation
              ? Transform.scale(
                  scale: 0.985 + value * 0.015,
                  alignment: Alignment.topLeft,
                  child: child,
                )
              : child,
        );
      },
      child: child,
    );
  }
}

class _NautermDropdownMenuOverlay<T> extends StatelessWidget {
  const _NautermDropdownMenuOverlay({
    required this.anchor,
    required this.entries,
    required this.width,
    required this.maxHeight,
    required this.onSelected,
    required this.onDismissed,
    required this.showScrollbarOnHover,
    required this.scaleAnimation,
    this.style,
  });

  final Rect anchor;
  final List<NautermContextMenuEntry<T>> entries;
  final double width;
  final double maxHeight;
  final ValueChanged<T> onSelected;
  final VoidCallback onDismissed;
  final NautermContextMenuStyle? style;
  final bool showScrollbarOnHover;
  final bool scaleAnimation;

  @override
  Widget build(BuildContext context) {
    final contentHeight = nautermContextMenuHeight(entries);
    return NautermAnchoredDropdownOverlay(
      anchor: anchor,
      width: width,
      contentHeight: contentHeight,
      maxHeight: maxHeight,
      scaleAnimation: scaleAnimation,
      onDismissed: onDismissed,
      contentBuilder: (context, menuHeight) => NautermContextMenu<T>(
        entries: entries,
        width: width,
        height: menuHeight < contentHeight ? menuHeight : null,
        style: style,
        showScrollbarOnHover: showScrollbarOnHover,
        onSelected: onSelected,
      ),
    );
  }
}

typedef NautermAnchoredDropdownBuilder = Widget Function(
  BuildContext context,
  double menuHeight,
);

/// Shared positioning, dismissal, and entrance animation for anchored dropdowns.
class NautermAnchoredDropdownOverlay extends StatelessWidget {
  const NautermAnchoredDropdownOverlay({
    super.key,
    required this.anchor,
    required this.width,
    required this.contentHeight,
    required this.maxHeight,
    required this.onDismissed,
    required this.contentBuilder,
    this.scaleAnimation = false,
  });

  final Rect anchor;
  final double width;
  final double contentHeight;
  final double maxHeight;
  final VoidCallback onDismissed;
  final NautermAnchoredDropdownBuilder contentBuilder;
  final bool scaleAnimation;

  @override
  Widget build(BuildContext context) {
    final overlaySize = MediaQuery.sizeOf(context);
    final safePadding = nautermTransientOverlaySafePadding(
      context,
      base: MediaQuery.paddingOf(context),
    );
    const margin = 8.0;
    const gap = 4.0;
    final desiredHeight = math.min(contentHeight, maxHeight);
    final availableBelow =
        overlaySize.height - safePadding.bottom - margin - anchor.bottom - gap;
    final availableAbove = anchor.top - safePadding.top - margin - gap;
    final openAbove =
        availableBelow < desiredHeight && availableAbove > availableBelow;
    final availableHeight = math.max(
      nautermContextMenuRowHeight + nautermContextMenuVerticalPadding * 2,
      openAbove ? availableAbove : availableBelow,
    );
    final menuHeight = math.min(desiredHeight, availableHeight);
    final minLeft = safePadding.left + margin;
    final maxLeft = math.max(
      minLeft,
      overlaySize.width - safePadding.right - width - margin,
    );
    final left = anchor.left.clamp(minLeft, maxLeft).toDouble();
    final top = openAbove ? anchor.top - gap - menuHeight : anchor.bottom + gap;
    final rect = positionNautermTransientOverlay(
      context: context,
      preferredRect: Rect.fromLTWH(left, top, width, menuHeight),
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
            width: width,
            height: menuHeight,
            child: TweenAnimationBuilder<double>(
              tween: Tween<double>(begin: 0, end: 1),
              duration: nautermContextMenuAnimationDuration,
              curve: Curves.easeOutCubic,
              builder: (context, value, child) => Opacity(
                opacity: value,
                child: scaleAnimation
                    ? Transform.scale(
                        scale: 0.985 + value * 0.015,
                        alignment: openAbove
                            ? Alignment.bottomLeft
                            : Alignment.topLeft,
                        child: child,
                      )
                    : child,
              ),
              child: contentBuilder(context, menuHeight),
            ),
          ),
        ],
      ),
    );
  }
}

class NautermContextMenu<T> extends StatelessWidget {
  const NautermContextMenu({
    super.key,
    required this.entries,
    required this.onSelected,
    this.width = 220,
    this.height,
    this.style,
    this.showScrollbarOnHover = false,
    this.onSubmenuRequested,
    this.onSubmenuPointerRequested,
    this.onActionHovered,
    this.onActionPointerEntered,
  });

  final List<NautermContextMenuEntry<T>> entries;
  final ValueChanged<T> onSelected;
  final double width;
  final double? height;
  final NautermContextMenuStyle? style;
  final bool showScrollbarOnHover;
  final void Function(NautermContextMenuAction<T> action, Rect anchor)?
  onSubmenuRequested;
  final void Function(
    NautermContextMenuAction<T> action,
    Rect anchor,
    Offset position,
  )?
  onSubmenuPointerRequested;
  final ValueChanged<NautermContextMenuAction<T>>? onActionHovered;
  final void Function(NautermContextMenuAction<T> action, Offset position)?
  onActionPointerEntered;

  @override
  Widget build(BuildContext context) {
    final palette = context.nautermPalette;
    final rows = [
      for (final entry in entries)
        switch (entry) {
          NautermContextMenuDivider<T>() => _NautermContextMenuDivider(
            palette: palette,
            style: style,
          ),
          NautermContextMenuAction<T>() => _NautermContextMenuItem<T>(
            entry: entry,
            palette: palette,
            style: style,
            onSelected: onSelected,
            onSubmenuRequested: onSubmenuRequested,
            onSubmenuPointerRequested: onSubmenuPointerRequested,
            onActionHovered: onActionHovered,
            onActionPointerEntered: onActionPointerEntered,
          ),
        },
    ];
    final content = height == null
        ? Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: 6,
              vertical: nautermContextMenuVerticalPadding,
            ),
            child: Column(mainAxisSize: MainAxisSize.min, children: rows),
          )
        : Padding(
            padding: const EdgeInsets.symmetric(
              vertical: nautermContextMenuVerticalPadding,
            ),
            child: showScrollbarOnHover
                ? _NautermHoverScrollbarList(
                    children: [
                      for (final row in rows)
                        Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 6),
                          child: row,
                        ),
                    ],
                  )
                : ListView(
                    padding: EdgeInsets.zero,
                    children: [
                      for (final row in rows)
                        Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 6),
                          child: row,
                        ),
                    ],
                  ),
          );
    return NautermDropdownSurface(
      style: style,
      child: SizedBox(width: width, height: height, child: content),
    );
  }
}

class _NautermHoverScrollbarList extends StatefulWidget {
  const _NautermHoverScrollbarList({required this.children});

  final List<Widget> children;

  @override
  State<_NautermHoverScrollbarList> createState() =>
      _NautermHoverScrollbarListState();
}

class _NautermHoverScrollbarListState
    extends State<_NautermHoverScrollbarList> {
  final ScrollController _controller = ScrollController();
  bool _hovered = false;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: Scrollbar(
        controller: _controller,
        thumbVisibility: _hovered,
        interactive: true,
        child: ListView(
          controller: _controller,
          padding: EdgeInsets.zero,
          children: widget.children,
        ),
      ),
    );
  }
}

/// Shared chrome for standard and custom anchored dropdown menus.
class NautermDropdownSurface extends StatelessWidget {
  const NautermDropdownSurface({super.key, required this.child, this.style});

  final Widget child;
  final NautermContextMenuStyle? style;

  @override
  Widget build(BuildContext context) {
    final palette = context.nautermPalette;
    final dark = Theme.of(context).brightness == Brightness.dark;
    return DecoratedBox(
      decoration: BoxDecoration(
        color:
            style?.background ??
            (dark ? palette.surfaceContainer : palette.surface),
        borderRadius: BorderRadius.circular(11),
        border: Border.all(color: style?.border ?? palette.outline),
        boxShadow:
            style?.shadows ??
            [
              BoxShadow(
                color: Colors.black.withValues(alpha: dark ? 0.34 : 0.12),
                blurRadius: 22,
                offset: const Offset(0, 10),
              ),
              BoxShadow(
                color: Colors.black.withValues(alpha: dark ? 0.18 : 0.05),
                blurRadius: 3,
                offset: const Offset(0, 1),
              ),
            ],
      ),
      child: ClipRRect(borderRadius: BorderRadius.circular(11), child: child),
    );
  }
}

/// The standard interactive row used by Nauterm dropdown menus.
class NautermDropdownRow extends StatefulWidget {
  const NautermDropdownRow({
    super.key,
    required this.child,
    required this.onTap,
    this.enabled = true,
    this.destructive = false,
    this.style,
    this.onHoverChanged,
    this.onPointerEnter,
  });

  final Widget child;
  final VoidCallback onTap;
  final bool enabled;
  final bool destructive;
  final NautermContextMenuStyle? style;
  final ValueChanged<bool>? onHoverChanged;
  final ValueChanged<PointerEnterEvent>? onPointerEnter;

  @override
  State<NautermDropdownRow> createState() => _NautermDropdownRowState();
}

class _NautermDropdownRowState extends State<NautermDropdownRow> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final palette = context.nautermPalette;
    final style = widget.style;
    final destructive = style?.destructive ?? const Color(0xffef4444);
    final dark = Theme.of(context).brightness == Brightness.dark;
    final neutralBase = dark ? palette.surfaceContainer : palette.surface;
    final hoverBackground = Color.alphaBlend(
      (dark ? Colors.white : Colors.black).withValues(
        alpha: dark ? 0.07 : 0.045,
      ),
      neutralBase,
    );
    final background = _hovered
        ? widget.destructive
              ? destructive.withValues(alpha: 0.12)
              : style?.hoverBackground ?? hoverBackground
        : Colors.transparent;
    void setHovered(bool value) {
      if (_hovered == value) return;
      setState(() => _hovered = value);
      widget.onHoverChanged?.call(value);
    }

    return MouseRegion(
      cursor: widget.enabled
          ? SystemMouseCursors.click
          : SystemMouseCursors.basic,
      onEnter: widget.enabled
          ? (event) {
              setHovered(true);
              widget.onPointerEnter?.call(event);
            }
          : null,
      onExit: widget.enabled ? (_) => setHovered(false) : null,
      child: Container(
        height: nautermContextMenuRowHeight,
        decoration: BoxDecoration(
          color: background,
          borderRadius: BorderRadius.circular(6),
        ),
        child: Material(
          type: MaterialType.transparency,
          child: InkWell(
            borderRadius: BorderRadius.circular(6),
            hoverColor: Colors.transparent,
            splashColor: widget.destructive
                ? destructive.withValues(alpha: 0.16)
                : (style?.accent ?? palette.primary).withValues(alpha: 0.16),
            highlightColor: widget.destructive
                ? destructive.withValues(alpha: 0.10)
                : (style?.accent ?? palette.primary).withValues(alpha: 0.08),
            onTap: widget.enabled ? widget.onTap : null,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10),
              child: widget.child,
            ),
          ),
        ),
      ),
    );
  }
}

class _NautermContextMenuItem<T> extends StatefulWidget {
  const _NautermContextMenuItem({
    required this.entry,
    required this.palette,
    required this.onSelected,
    this.onSubmenuRequested,
    this.onSubmenuPointerRequested,
    this.onActionHovered,
    this.onActionPointerEntered,
    this.style,
  });

  final NautermContextMenuAction<T> entry;
  final NautermPalette palette;
  final ValueChanged<T> onSelected;
  final void Function(NautermContextMenuAction<T> action, Rect anchor)?
  onSubmenuRequested;
  final void Function(
    NautermContextMenuAction<T> action,
    Rect anchor,
    Offset position,
  )?
  onSubmenuPointerRequested;
  final ValueChanged<NautermContextMenuAction<T>>? onActionHovered;
  final void Function(NautermContextMenuAction<T> action, Offset position)?
  onActionPointerEntered;
  final NautermContextMenuStyle? style;

  @override
  State<_NautermContextMenuItem<T>> createState() =>
      _NautermContextMenuItemState<T>();
}

class _NautermContextMenuItemState<T>
    extends State<_NautermContextMenuItem<T>> {
  void _requestSubmenu([Offset? pointerPosition]) {
    final renderObject = context.findRenderObject();
    if (renderObject is! RenderBox) return;
    final anchor = renderObject.localToGlobal(Offset.zero) & renderObject.size;
    widget.onSubmenuRequested?.call(widget.entry, anchor);
    widget.onSubmenuPointerRequested?.call(
      widget.entry,
      anchor,
      pointerPosition ?? anchor.center,
    );
  }

  @override
  Widget build(BuildContext context) {
    final entry = widget.entry;
    final palette = widget.palette;
    final style = widget.style;
    final destructive = style?.destructive ?? const Color(0xffef4444);
    final foreground = !entry.enabled
        ? style?.disabledForeground ?? palette.faintText
        : entry.destructive
        ? destructive
        : entry.selected
        ? style?.accent ?? palette.primary
        : style?.foreground ?? palette.text;
    return NautermDropdownRow(
      enabled: entry.enabled,
      destructive: entry.destructive,
      style: style,
      onPointerEnter: (event) {
        widget.onActionHovered?.call(entry);
        widget.onActionPointerEntered?.call(entry, event.position);
        if (entry.hasSubmenu) {
          _requestSubmenu(event.position);
        }
      },
      onTap: entry.hasSubmenu
          ? _requestSubmenu
          : () => widget.onSelected(entry.value),
      child: Row(
        children: [
          if (entry.iconBytes case final iconBytes?) ...[
            ClipRRect(
              borderRadius: BorderRadius.circular(4),
              child: Image.memory(
                iconBytes,
                width: 18,
                height: 18,
                fit: BoxFit.contain,
                gaplessPlayback: true,
                errorBuilder: (context, error, stackTrace) =>
                    const SizedBox(width: 18, height: 18),
              ),
            ),
            const SizedBox(width: 8),
          ] else if (entry.icon != null) ...[
            Icon(entry.icon, size: 16, color: foreground),
            const SizedBox(width: 10),
          ],
          Expanded(
            child: Text(
              context.tr(entry.label),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: foreground,
                fontSize: 12,
                fontWeight: FontWeight.w500,
                height: 1,
                letterSpacing: 0,
                decoration: TextDecoration.none,
              ),
            ),
          ),
          if (entry.shortcut != null) ...[
            const SizedBox(width: 14),
            Text(
              entry.shortcut!,
              style: TextStyle(
                color: entry.enabled
                    ? style?.mutedForeground ?? palette.faintText
                    : style?.disabledForeground ?? palette.outline,
                fontSize: 11,
                fontWeight: FontWeight.w500,
                height: 1,
                letterSpacing: 0,
                decoration: TextDecoration.none,
              ),
            ),
          ] else if (entry.hasSubmenu) ...[
            const SizedBox(width: 14),
            Transform.translate(
              offset: const Offset(4, 0),
              child: Icon(
                Icons.chevron_right_rounded,
                size: 16,
                color: foreground,
              ),
            ),
          ] else if (entry.selected) ...[
            const SizedBox(width: 14),
            Icon(
              Icons.check_rounded,
              size: 16,
              color: style?.accent ?? palette.primary,
            ),
          ],
        ],
      ),
    );
  }
}

class _NautermContextMenuDivider extends StatelessWidget {
  const _NautermContextMenuDivider({required this.palette, this.style});

  final NautermPalette palette;
  final NautermContextMenuStyle? style;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: nautermContextMenuDividerHeight,
      child: Center(
        child: Container(
          height: 1,
          margin: const EdgeInsets.symmetric(horizontal: 10),
          color: style?.border ?? palette.softOutline,
        ),
      ),
    );
  }
}
