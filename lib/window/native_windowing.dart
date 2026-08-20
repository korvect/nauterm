// ignore_for_file: invalid_use_of_internal_member, implementation_imports

import 'dart:async';
import 'dart:io' as io;
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:nativeapi/nativeapi.dart';
import 'package:cnativeapi/cnativeapi.dart';

import '../app/window_config.dart';
import '../data/nauterm_config_store.dart';
import '../data/nauterm_paths.dart';
import '../terminal/terminal_config.dart';

abstract interface class NautermRootController {
  void showMainWindow();

  Future<void> closeMainWindow();

  void showSettingsWindow();

  void hideSettingsWindow();

  void closeSelectedTerminalTab();

  Future<bool> requestQuit();
}

bool _configured = false;
bool _settingsWindowRequested = false;
NautermSettingsPage? _requestedSettingsPage;
final ValueNotifier<int> settingsPageRequestRevision = ValueNotifier<int>(0);
int? _mainWindowId;
NautermRootController? _rootController;
final Set<int> _positionedWindowIds = <int>{};
Timer? _windowGeometryPersistTimer;
bool _mainWindowMaximized = false;
const MethodChannel _appMenuChannel = MethodChannel(
  'com.korvect.nauterm/app_menu',
);
const MethodChannel _titleBarRegionsChannel = MethodChannel(
  'com.korvect.nauterm/title_bar_regions',
);
final Map<Object, Rect> _mainWindowTitleBarInteractiveRegions =
    <Object, Rect>{};
bool _mainWindowTitleBarRegionsUpdateScheduled = false;

enum NautermSettingsPage { terminal, about }

NautermSettingsPage? takeRequestedSettingsPage() {
  final page = _requestedSettingsPage;
  _requestedSettingsPage = null;
  return page;
}

Future<void> waitForNautermUiPresentation() async {
  final binding = WidgetsBinding.instance;
  await binding.waitUntilFirstFrameRasterized;
  // Rasterization is reported just before the platform compositor presents the
  // texture. Give macOS one display turn before a Keychain sheet can block it.
  await Future<void>.delayed(const Duration(milliseconds: 120));
}

void registerNautermRoot(NautermRootController controller) {
  _rootController = controller;
}

void unregisterNautermRoot(NautermRootController controller) {
  if (identical(_rootController, controller)) {
    _rootController = null;
  }
}

void configureNativeWindowing() {
  if (_configured) {
    return;
  }
  _configured = true;
  _appMenuChannel.setMethodCallHandler(_handleAppMenuMethodCall);

  final primaryDisplay = DisplayManager.instance.getPrimary();

  WindowManager.instance.addCallbackListener<WindowMaximizedEvent>((
    WindowMaximizedEvent event,
  ) {
    if (event.windowId == _mainWindowId) {
      _mainWindowMaximized = true;
      hideMainWindowTitleBar();
    }
  });
  WindowManager.instance.addCallbackListener<WindowRestoredEvent>((
    WindowRestoredEvent event,
  ) {
    if (event.windowId == _mainWindowId) {
      _mainWindowMaximized = false;
      hideMainWindowTitleBar();
      _scheduleMainWindowGeometryPersist();
    }
  });
  WindowManager.instance.addCallbackListener<WindowMovedEvent>((event) {
    if (event.windowId != _mainWindowId || _mainWindowMaximized) {
      return;
    }
    _scheduleMainWindowGeometryPersist();
  });
  WindowManager.instance.addCallbackListener<WindowResizedEvent>((event) {
    if (event.windowId != _mainWindowId || _mainWindowMaximized) {
      return;
    }
    _scheduleMainWindowGeometryPersist();
  });

  WindowManager.instance.setWillShowHook((windowId) {
    final window = WindowManager.instance.getById(windowId);
    if (window != null && primaryDisplay != null) {
      switch (window.title) {
        case mainWindowTitle:
          _mainWindowId = windowId;
          _applyMainWindowChrome(window);
          if (_positionedWindowIds.add(windowId)) {
            _restoreMainWindowGeometry(window, primaryDisplay);
          } else {
            window.setMinimumSize(
              mainWindowMinSize.width,
              mainWindowMinSize.height,
            );
          }
          break;
        case settingsWindowTitle:
          if (_positionedWindowIds.add(windowId)) {
            _positionWindow(
              window,
              primaryDisplay,
              size: settingsWindowSize,
              minSize: settingsWindowMinSize,
            );
          } else {
            window.setMinimumSize(
              settingsWindowMinSize.width,
              settingsWindowMinSize.height,
            );
          }
          return _settingsWindowRequested;
      }
    }
    return true;
  });
}

Future<Object?> _handleAppMenuMethodCall(MethodCall call) async {
  switch (call.method) {
    case 'showSettings':
      showSettingsWindow();
      return null;
    case 'showAbout':
      showSettingsWindow(page: NautermSettingsPage.about);
      return null;
    case 'showMainWindow':
      showMainWindow();
      return null;
    case 'closeSelectedTerminalTab':
      _rootController?.closeSelectedTerminalTab();
      return null;
    case 'requestQuit':
      final shouldQuit =
          await (_rootController?.requestQuit() ?? Future.value(true));
      if (shouldQuit) {
        captureMainWindowGeometry();
        await persistMainWindowGeometry();
      }
      return shouldQuit;
    case 'fullscreenChanged':
      fullscreenNotifier.value = call.arguments as bool? ?? false;
      return null;
    default:
      throw MissingPluginException('Unknown app menu method: ${call.method}');
  }
}

void showMainWindow() {
  _rootController?.showMainWindow();
}

bool requestMainWindowClose() {
  final rootController = _rootController;
  if (rootController == null) {
    return false;
  }

  unawaited(rootController.closeMainWindow());
  return true;
}

void showSettingsWindow({NautermSettingsPage? page}) {
  _requestedSettingsPage = page;
  if (page != null) {
    settingsPageRequestRevision.value++;
  }
  _rootController?.showSettingsWindow();
}

void requestSettingsWindowFocus() {
  for (final window in _allWindows()) {
    if (window.title == settingsWindowTitle) {
      window.show();
      window.focus();
      return;
    }
  }
}

void hideSettingsNativeWindow() {
  for (final window in _allWindows()) {
    if (window.title == settingsWindowTitle) {
      window.hide();
    }
  }
}

void requestMainWindowFocus() {
  withMainWindow((window) {
    window.show();
    if (window.isMinimized) {
      window.restore();
    }
    window.focus();
    _centerTrafficLights(window);
  });
}

void hideMainWindow() {
  final window = _mainWindow();
  if (window == null) {
    return;
  }
  window.hide();
  if (_isNormalWindow(window)) {
    _applyMainWindowBounds(window.bounds);
    _scheduleMainWindowGeometryPersist();
  }
}

Future<void> quitNautermApplication({int exitCode = 0}) async {
  captureMainWindowGeometry();
  await persistMainWindowGeometry();

  if (defaultTargetPlatform == TargetPlatform.macOS) {
    return;
  }

  hideMainWindow();
  await Future<void>.delayed(Duration.zero);
  io.exit(exitCode);
}

Future<bool> restartNautermApplication({
  bool launchInstalledExecutable = true,
}) async {
  final shouldQuit =
      await (_rootController?.requestQuit() ?? Future.value(true));
  if (!shouldQuit) {
    return false;
  }
  if (launchInstalledExecutable) {
    await io.Process.start(
      io.Platform.resolvedExecutable,
      const [],
      mode: io.ProcessStartMode.detached,
    );
  }
  await quitNautermApplication();
  return true;
}

void forgetMainWindow() {
  _windowGeometryPersistTimer?.cancel();
  _windowGeometryPersistTimer = null;
  _mainWindowId = null;
  _mainWindowMaximized = false;
}

void hideSettingsWindow() {
  _rootController?.hideSettingsWindow();
}

void markSettingsWindowRequested(bool requested) {
  _settingsWindowRequested = requested;
}

void hideMainWindowTitleBar() {
  final id = _mainWindowId;
  if (id != null) {
    final window = WindowManager.instance.getById(id);
    if (window != null) {
      _applyMainWindowChrome(window);
    }
  } else {
    withMainWindow((window) {
      _mainWindowId = window.id;
      _applyMainWindowChrome(window);
    });
  }
}

void _applyMainWindowChrome(Window window) {
  window.titleBarStyle = TitleBarStyle.hidden;
  window.windowControlButtonsVisible = _usesNativeWindowControls();
  _centerTrafficLights(window);
}

void updateMainWindowTitleBarInteractiveRegion(Object key, Rect region) {
  if (defaultTargetPlatform != TargetPlatform.macOS) {
    return;
  }
  if (_mainWindowTitleBarInteractiveRegions[key] == region) {
    return;
  }
  _mainWindowTitleBarInteractiveRegions[key] = region;
  _scheduleMainWindowTitleBarRegionsUpdate();
}

void removeMainWindowTitleBarInteractiveRegion(Object key) {
  if (defaultTargetPlatform != TargetPlatform.macOS ||
      _mainWindowTitleBarInteractiveRegions.remove(key) == null) {
    return;
  }
  _scheduleMainWindowTitleBarRegionsUpdate();
}

void _scheduleMainWindowTitleBarRegionsUpdate() {
  if (_mainWindowTitleBarRegionsUpdateScheduled) {
    return;
  }
  _mainWindowTitleBarRegionsUpdateScheduled = true;
  scheduleMicrotask(() async {
    _mainWindowTitleBarRegionsUpdateScheduled = false;
    final regions = _mainWindowTitleBarInteractiveRegions.values
        .map(
          (region) => <String, double>{
            'x': region.left,
            'y': region.top,
            'width': region.width,
            'height': region.height,
          },
        )
        .toList(growable: false);
    try {
      await _titleBarRegionsChannel.invokeMethod<void>(
        'setInteractiveRegions',
        regions,
      );
    } on MissingPluginException {
      // Widget tests and non-macOS embedders do not install the native bridge.
    }
  });
}

void _centerTrafficLights(Window window) {
  if (defaultTargetPlatform != TargetPlatform.macOS) return;
  // Tauri uses: titleBarHeight = buttonHeight + y
  // Top bar is 44px, buttons are ~12px, so y = 44 - 12 = 32
  // x is the left offset (default macOS is 8).
  cnativeApiBindings.native_window_set_window_control_buttons_position(
    window.nativeHandle,
    8,
    32,
  );
}

void startMainWindowDrag() {
  withMainWindow((window) => window.startDragging());
}

final ValueNotifier<bool> fullscreenNotifier = ValueNotifier<bool>(false);

bool isMainWindowFullscreen() => fullscreenNotifier.value;

Size resizeMainWindow(Size requestedSize) {
  final window = _mainWindow();
  final display =
      _displayForWindow(window) ?? DisplayManager.instance.getPrimary();
  final size = _clampMainWindowSize(requestedSize, display?.workArea);
  terminalWindowWidth = size.width;
  terminalWindowHeight = size.height;
  if (window != null) {
    if (window.isMaximized) {
      window.unmaximize();
      _mainWindowMaximized = false;
    }
    window.setSize(size.width, size.height);
  }
  _scheduleMainWindowGeometryPersist();
  return size;
}

void captureMainWindowGeometry() {
  final window = _mainWindow();
  if (window == null || !_isNormalWindow(window)) {
    return;
  }
  _applyMainWindowBounds(window.bounds);
  _scheduleMainWindowGeometryPersist();
}

Future<void> persistMainWindowGeometry() async {
  _windowGeometryPersistTimer?.cancel();
  _windowGeometryPersistTimer = null;
  try {
    await NautermConfigStore(NautermPaths.resolve())
        .saveRuntimeSettings(currentNautermRuntimeSettings());
  } on Object {
    // Geometry persistence must never disrupt window interaction or shutdown.
  }
}

void withMainWindow(void Function(Window window) action) {
  final mainWindow = _mainWindow();
  if (mainWindow != null) {
    action(mainWindow);
  }
}

Window? _mainWindow() {
  final id = _mainWindowId;
  if (id != null) {
    final window = WindowManager.instance.getById(id);
    if (window != null) {
      return window;
    }
  }

  for (final window in _allWindows()) {
    if (window.title == mainWindowTitle) {
      _mainWindowId = window.id;
      return window;
    }
  }

  return null;
}

List<Window> _allWindows() {
  try {
    return WindowManager.instance.getAll();
  } on Object {
    return const [];
  }
}

void _restoreMainWindowGeometry(Window window, Display primaryDisplay) {
  final savedSize = Size(
    terminalWindowWidth ?? mainWindowSize.width,
    terminalWindowHeight ?? mainWindowSize.height,
  );
  final savedPosition = terminalWindowX != null && terminalWindowY != null
      ? Offset(terminalWindowX!, terminalWindowY!)
      : null;
  final targetDisplay = savedPosition == null
      ? primaryDisplay
      : _displayForSavedGeometry(savedPosition, savedSize) ?? primaryDisplay;
  final size = _clampMainWindowSize(savedSize, targetDisplay.workArea);

  window.setMinimumSize(mainWindowMinSize.width, mainWindowMinSize.height);
  window.setSize(size.width, size.height);
  if (savedPosition != null &&
      _windowPositionIsVisible(savedPosition, size, targetDisplay.workArea)) {
    window.setPosition(savedPosition.dx, savedPosition.dy);
  } else {
    _centerWindow(window, targetDisplay, size);
  }
}

Display? _displayForSavedGeometry(Offset position, Size size) {
  final bounds = position & size;
  for (final display in _allDisplays()) {
    final visible = bounds.intersect(display.workArea);
    if (visible.width >= 64 && visible.height >= 48) {
      return display;
    }
  }
  return null;
}

Display? _displayForWindow(Window? window) {
  if (window == null) {
    return null;
  }
  final bounds = window.bounds;
  Display? best;
  var bestArea = 0.0;
  for (final display in _allDisplays()) {
    final visible = bounds.intersect(display.workArea);
    final area = math.max(0.0, visible.width) * math.max(0.0, visible.height);
    if (area > bestArea) {
      bestArea = area;
      best = display;
    }
  }
  return best;
}

List<Display> _allDisplays() {
  try {
    return DisplayManager.instance.getAll();
  } on Object {
    return const [];
  }
}

Size _clampMainWindowSize(Size size, Rect? workArea) {
  return _clampWindowSize(size, mainWindowMinSize, workArea);
}

Size _clampWindowSize(Size size, Size minSize, Rect? workArea) {
  final maximumWidth = math.max(minSize.width, workArea?.width ?? size.width);
  final maximumHeight = math.max(
    minSize.height,
    workArea?.height ?? size.height,
  );
  final width =
      (_isValidWindowDimension(size.width) ? size.width : mainWindowSize.width)
          .clamp(minSize.width, maximumWidth)
          .toDouble();
  final height =
      (_isValidWindowDimension(size.height)
              ? size.height
              : mainWindowSize.height)
          .clamp(minSize.height, maximumHeight)
          .toDouble();
  return Size(width, height);
}

bool _windowPositionIsVisible(Offset position, Size size, Rect workArea) {
  final visible = (position & size).intersect(workArea);
  return visible.width >= 64 && visible.height >= 48;
}

bool _isNormalWindow(Window window) {
  return !_mainWindowMaximized &&
      !window.isMaximized &&
      !window.isMinimized &&
      !window.isFullscreen;
}

void _applyMainWindowBounds(Rect bounds) {
  if (_isValidWindowDimension(bounds.width) &&
      _isValidWindowDimension(bounds.height)) {
    terminalWindowWidth = bounds.width;
    terminalWindowHeight = bounds.height;
  }
  if (bounds.left.isFinite && bounds.top.isFinite) {
    terminalWindowX = bounds.left;
    terminalWindowY = bounds.top;
  }
}

bool _isValidWindowDimension(double value) => value.isFinite && value > 0;

void _scheduleMainWindowGeometryPersist() {
  _windowGeometryPersistTimer?.cancel();
  _windowGeometryPersistTimer = Timer(
    const Duration(milliseconds: 400),
    () => unawaited(persistMainWindowGeometry()),
  );
}

void _positionWindow(
  Window window,
  Display display, {
  required Size size,
  required Size minSize,
}) {
  final workArea = display.workArea;
  final clampedSize = _clampWindowSize(size, minSize, workArea);

  window.setMinimumSize(minSize.width, minSize.height);
  window.setSize(clampedSize.width, clampedSize.height);
  _centerWindow(window, display, clampedSize);
}

void _centerWindow(Window window, Display display, Size size) {
  final workArea = display.workArea;
  final startX = workArea.left + (workArea.width - size.width) / 2;
  final startY = workArea.top + (workArea.height - size.height) / 2;
  window.setPosition(startX, startY);
}

bool _usesNativeWindowControls() {
  return defaultTargetPlatform == TargetPlatform.macOS;
}
