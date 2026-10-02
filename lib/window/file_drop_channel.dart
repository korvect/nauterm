import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/foundation.dart';
import 'package:nativeapi/nativeapi.dart' as native;

enum NautermFileDropEventType { dragging, dropped, exited }

class NautermFileDropEvent {
  const NautermFileDropEvent({
    required this.type,
    this.paths = const [],
    this.x,
    this.y,
  });

  final NautermFileDropEventType type;
  final List<String> paths;
  final double? x;
  final double? y;
}

class NautermFileDropChannel {
  NautermFileDropChannel._();

  static final NautermFileDropChannel instance = NautermFileDropChannel._();

  static const MethodChannel _channel = MethodChannel(
    'com.korvect.nauterm/file_drop',
  );

  final StreamController<NautermFileDropEvent> _events =
      StreamController<NautermFileDropEvent>.broadcast();
  bool _initialized = false;
  bool _enabled = false;
  native.Window? _window;
  native.DropTarget? _target;
  int? _listenerId;

  // macOS previously supplied these events from the vendored cnativeapi fork.
  // Keep the existing stream contract while using upstream's native drop target.
  void attachWindow(native.Window? window) {
    _releaseTarget();
    _window = window;
    _syncTarget();
  }

  void _releaseTarget() {
    final target = _target;
    if (target != null) {
      if (_listenerId case final id?) {
        target.removeListener(id);
      }
      target.dispose();
    }
    _target = null;
    _listenerId = null;
  }

  void _syncTarget() {
    if (!_enabled ||
        _window == null ||
        defaultTargetPlatform != TargetPlatform.macOS) {
      return;
    }
    _target ??= native.DropTarget.create(_window);
    _listenerId ??= _target?.addListener((event) {
      switch (event) {
        case native.DropTargetEnteredEvent(:final position):
        case native.DropTargetMovedEvent(:final position):
          _events.add(
            NautermFileDropEvent(
              type: NautermFileDropEventType.dragging,
              x: position.x,
              y: position.y,
            ),
          );
        case native.DropTargetExitedEvent():
          _events.add(
            const NautermFileDropEvent(type: NautermFileDropEventType.exited),
          );
        case native.DropTargetDroppedEvent(:final position, :final filePaths):
          if (filePaths.isNotEmpty) {
            _events.add(
              NautermFileDropEvent(
                type: NautermFileDropEventType.dropped,
                paths: filePaths,
                x: position.x,
                y: position.y,
              ),
            );
          } else {
            _events.add(
              const NautermFileDropEvent(type: NautermFileDropEventType.exited),
            );
          }
      }
    });
  }

  Stream<NautermFileDropEvent> get events => _events.stream;

  void ensureInitialized() {
    if (_initialized) {
      return;
    }
    _initialized = true;
    _channel.setMethodCallHandler(_handleMethodCall);
  }

  Future<void> setEnabled(bool enabled) async {
    ensureInitialized();
    _enabled = enabled;
    if (defaultTargetPlatform == TargetPlatform.macOS) {
      if (enabled) {
        _syncTarget();
      } else {
        _releaseTarget();
      }
      return;
    }
    try {
      await _channel.invokeMethod<void>('setEnabled', {'enabled': enabled});
    } on MissingPluginException {
      // File drop is only implemented by desktop runners that opt into it.
    }
  }

  Future<Object?> _handleMethodCall(MethodCall call) async {
    switch (call.method) {
      case 'filesDragging':
        _events.add(
          NautermFileDropEvent(
            type: NautermFileDropEventType.dragging,
            x: _coordinateFromArguments(call.arguments, 'x'),
            y: _coordinateFromArguments(call.arguments, 'y'),
          ),
        );
        return null;
      case 'filesDropped':
        final paths = _pathsFromArguments(call.arguments);
        if (paths.isNotEmpty) {
          _events.add(
            NautermFileDropEvent(
              type: NautermFileDropEventType.dropped,
              paths: paths,
              x: _coordinateFromArguments(call.arguments, 'x'),
              y: _coordinateFromArguments(call.arguments, 'y'),
            ),
          );
        }
        return null;
      case 'filesExited':
        _events.add(
          const NautermFileDropEvent(type: NautermFileDropEventType.exited),
        );
        return null;
      default:
        throw MissingPluginException(
          'Unknown file drop method: ${call.method}',
        );
    }
  }

  List<String> _pathsFromArguments(Object? arguments) {
    final Object? rawPaths = arguments is Map ? arguments['paths'] : arguments;
    if (rawPaths is! List) {
      return const [];
    }
    return [
      for (final path in rawPaths)
        if (path is String && path.trim().isNotEmpty) path,
    ];
  }

  double? _coordinateFromArguments(Object? arguments, String key) {
    if (arguments is! Map) {
      return null;
    }
    final value = arguments[key];
    if (value is num) {
      return value.toDouble();
    }
    return null;
  }
}
