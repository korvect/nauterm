import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/scheduler.dart';
import 'package:flutter/services.dart';

typedef NautermOverlayDismiss = void Function();

class NautermOverlaySafeAreaScope extends InheritedWidget {
  const NautermOverlaySafeAreaScope({
    super.key,
    required this.padding,
    required super.child,
  });

  final EdgeInsets padding;

  static EdgeInsets maybeOf(BuildContext context) {
    return context
            .dependOnInheritedWidgetOfExactType<NautermOverlaySafeAreaScope>()
            ?.padding ??
        EdgeInsets.zero;
  }

  @override
  bool updateShouldNotify(NautermOverlaySafeAreaScope oldWidget) {
    return padding != oldWidget.padding;
  }
}

EdgeInsets nautermTransientOverlaySafePadding(
  BuildContext context, {
  EdgeInsets base = EdgeInsets.zero,
}) {
  final scoped = NautermOverlaySafeAreaScope.maybeOf(context);
  return EdgeInsets.fromLTRB(
    math.max(base.left, scoped.left),
    math.max(base.top, scoped.top),
    math.max(base.right, scoped.right),
    math.max(base.bottom, scoped.bottom),
  );
}

Rect positionNautermTransientOverlay({
  required BuildContext context,
  required Rect preferredRect,
  required Size overlaySize,
  EdgeInsets safePadding = EdgeInsets.zero,
  double margin = 8,
}) {
  final effectivePadding = nautermTransientOverlaySafePadding(
    context,
    base: safePadding,
  );
  final minLeft = effectivePadding.left + margin;
  final minTop = effectivePadding.top + margin;
  final maxLeft = math.max(
    minLeft,
    overlaySize.width - effectivePadding.right - preferredRect.width - margin,
  );
  final maxTop = math.max(
    minTop,
    overlaySize.height -
        effectivePadding.bottom -
        preferredRect.height -
        margin,
  );
  return Rect.fromLTWH(
    preferredRect.left.clamp(minLeft, maxLeft).toDouble(),
    preferredRect.top.clamp(minTop, maxTop).toDouble(),
    preferredRect.width,
    preferredRect.height,
  );
}

class NautermTransientOverlayHandle {
  NautermTransientOverlayHandle._(
    this.entry,
    this._dismiss,
    this._markNeedsBuild,
  );

  final OverlayEntry entry;
  VoidCallback? _dismiss;
  final VoidCallback _markNeedsBuild;
  bool _notifyOnDismiss = true;
  bool _buildScheduled = false;

  bool get mounted => _dismiss != null;

  void markNeedsBuild() {
    if (!mounted) {
      return;
    }
    if (WidgetsBinding.instance.schedulerPhase ==
        SchedulerPhase.persistentCallbacks) {
      if (_buildScheduled) {
        return;
      }
      _buildScheduled = true;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _buildScheduled = false;
        if (mounted) {
          _markNeedsBuild();
        }
      });
      return;
    }
    _markNeedsBuild();
  }

  void dismiss({bool notify = true}) {
    final dismiss = _dismiss;
    if (dismiss == null) {
      return;
    }
    _notifyOnDismiss = notify;
    dismiss();
  }

  void _didDismiss(VoidCallback? onDismissed) {
    final shouldNotify = _notifyOnDismiss;
    _dismiss = null;
    _notifyOnDismiss = true;
    if (shouldNotify) {
      onDismissed?.call();
    }
  }
}

class NautermOverlayController {
  final Map<Object, NautermOverlayDismiss> _transientOverlays = {};
  final Map<Object, OverlayEntry> _transientEntries = {};

  OverlayEntry showTransient({
    required BuildContext context,
    required Object token,
    required WidgetBuilder builder,
    bool dismissExisting = false,
    VoidCallback? onDismissed,
  }) {
    if (dismissExisting) {
      dismissTransientOverlays(except: token);
    }

    final existing = _transientEntries[token];
    if (existing != null) {
      existing.markNeedsBuild();
      return existing;
    }

    final overlay = Overlay.of(context);
    final entry = OverlayEntry(builder: builder);
    _transientEntries[token] = entry;
    _transientOverlays[token] = () {
      final entry = _transientEntries.remove(token);
      _transientOverlays.remove(token);
      entry?.remove();
      onDismissed?.call();
    };
    overlay.insert(entry);
    return entry;
  }

  void markTransientNeedsBuild(Object token) {
    _transientEntries[token]?.markNeedsBuild();
  }

  void dismissTransient(Object token) {
    _transientOverlays[token]?.call();
  }

  void dismissTransientOverlays({Object? except}) {
    final entries = _transientOverlays.entries.toList(growable: false);
    for (final entry in entries) {
      if (entry.key == except) {
        continue;
      }
      entry.value();
    }
  }

  void dispose() {
    dismissTransientOverlays();
    _transientOverlays.clear();
    _transientEntries.clear();
  }
}

class NautermOverlayScope extends InheritedWidget {
  const NautermOverlayScope({
    super.key,
    required this.controller,
    required super.child,
  });

  final NautermOverlayController controller;

  static NautermOverlayController? maybeOf(BuildContext context) {
    return context
        .dependOnInheritedWidgetOfExactType<NautermOverlayScope>()
        ?.controller;
  }

  static NautermOverlayController? find(BuildContext context) {
    final scope = context
        .getElementForInheritedWidgetOfExactType<NautermOverlayScope>()
        ?.widget;
    return scope is NautermOverlayScope ? scope.controller : null;
  }

  @override
  bool updateShouldNotify(NautermOverlayScope oldWidget) {
    return controller != oldWidget.controller;
  }
}

NautermTransientOverlayHandle showNautermTransientOverlay({
  required BuildContext context,
  required Object token,
  required WidgetBuilder builder,
  bool dismissExisting = false,
  VoidCallback? onDismissed,
}) {
  final scopedPadding = NautermOverlaySafeAreaScope.maybeOf(context);
  WidgetBuilder resolvedBuilder = builder;
  if (scopedPadding != EdgeInsets.zero) {
    resolvedBuilder = (overlayContext) => NautermOverlaySafeAreaScope(
      padding: scopedPadding,
      child: Builder(builder: builder),
    );
  }
  final controller = NautermOverlayScope.find(context);
  if (controller != null) {
    late final NautermTransientOverlayHandle handle;
    final entry = controller.showTransient(
      context: context,
      token: token,
      builder: resolvedBuilder,
      dismissExisting: dismissExisting,
      onDismissed: () => handle._didDismiss(onDismissed),
    );
    handle = NautermTransientOverlayHandle._(
      entry,
      () => controller.dismissTransient(token),
      () => controller.markTransientNeedsBuild(token),
    );
    return handle;
  }

  if (dismissExisting) {
    NautermOverlayScope.find(context)?.dismissTransientOverlays();
  }

  final entry = OverlayEntry(builder: resolvedBuilder);
  late final NautermTransientOverlayHandle handle;
  handle = NautermTransientOverlayHandle._(entry, () {
    entry.remove();
    handle._didDismiss(onDismissed);
  }, entry.markNeedsBuild);
  Overlay.of(context).insert(entry);
  return handle;
}

final Expando<Object> _activeNautermDialogTokens = Expando<Object>(
  'active Nauterm dialog token',
);

Future<T?> showNautermDialog<T>({
  required BuildContext context,
  required WidgetBuilder builder,
  bool barrierDismissible = true,
}) {
  final overlay = Overlay.of(context);
  if (_activeNautermDialogTokens[overlay] != null) {
    return Future<T?>.value();
  }
  final dialogToken = Object();
  _activeNautermDialogTokens[overlay] = dialogToken;

  final completer = Completer<T?>();
  late final NautermTransientOverlayHandle handle;
  var completed = false;

  void releaseDialog() {
    if (identical(_activeNautermDialogTokens[overlay], dialogToken)) {
      _activeNautermDialogTokens[overlay] = null;
    }
  }

  void complete(T? result, {bool dismiss = true}) {
    if (completed) {
      return;
    }
    completed = true;
    releaseDialog();
    completer.complete(result);
    if (dismiss) {
      handle.dismiss(notify: false);
    }
  }

  try {
    handle = showNautermTransientOverlay(
      context: context,
      token: dialogToken,
      dismissExisting: true,
      onDismissed: () => complete(null, dismiss: false),
      builder: (_) => _NautermDialogOverlay<T>(
        builder: builder,
        barrierDismissible: barrierDismissible,
        onComplete: (result) => complete(result),
      ),
    );
  } catch (_) {
    releaseDialog();
    rethrow;
  }

  return completer.future;
}

class _NautermDialogOverlay<T> extends StatefulWidget {
  const _NautermDialogOverlay({
    required this.builder,
    required this.barrierDismissible,
    required this.onComplete,
  });

  final WidgetBuilder builder;
  final bool barrierDismissible;
  final ValueChanged<T?> onComplete;

  @override
  State<_NautermDialogOverlay<T>> createState() =>
      _NautermDialogOverlayState<T>();
}

class _NautermDialogOverlayState<T> extends State<_NautermDialogOverlay<T>> {
  final GlobalKey<NavigatorState> _navigatorKey = GlobalKey<NavigatorState>();
  PageRoute<T>? _route;

  @override
  Widget build(BuildContext context) {
    return Positioned.fill(
      child: FocusScope(
        autofocus: true,
        child: Shortcuts(
          shortcuts: const {
            SingleActivator(LogicalKeyboardKey.escape): DismissIntent(),
          },
          child: Actions(
            actions: {
              DismissIntent: CallbackAction<DismissIntent>(
                onInvoke: (_) {
                  if (widget.barrierDismissible) {
                    _navigatorKey.currentState?.maybePop();
                  }
                  return null;
                },
              ),
            },
            child: Navigator(
              key: _navigatorKey,
              onGenerateRoute: (_) {
                final route = PageRouteBuilder<T>(
                  opaque: false,
                  barrierColor: Colors.transparent,
                  pageBuilder: (context, animation, secondaryAnimation) {
                    return Material(
                      type: MaterialType.transparency,
                      child: Stack(
                        children: [
                          Positioned.fill(
                            child: GestureDetector(
                              behavior: HitTestBehavior.opaque,
                              onTap: widget.barrierDismissible
                                  ? () => Navigator.of(context).maybePop()
                                  : null,
                              child: const ColoredBox(color: Color(0x8a000000)),
                            ),
                          ),
                          Builder(builder: widget.builder),
                        ],
                      ),
                    );
                  },
                  transitionsBuilder:
                      (context, animation, secondaryAnimation, child) {
                        final curved = CurvedAnimation(
                          parent: animation,
                          curve: Curves.easeOutCubic,
                          reverseCurve: Curves.easeInCubic,
                        );
                        return FadeTransition(
                          opacity: curved,
                          child: ScaleTransition(
                            scale: Tween<double>(
                              begin: 0.985,
                              end: 1,
                            ).animate(curved),
                            child: child,
                          ),
                        );
                      },
                );
                if (!identical(_route, route)) {
                  _route = route;
                  route.popped.then((result) {
                    if (mounted) {
                      widget.onComplete(result);
                    }
                  });
                }
                return route;
              },
            ),
          ),
        ),
      ),
    );
  }
}
