// ignore_for_file: invalid_use_of_internal_member, implementation_imports

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/src/widgets/_window.dart';

import '../app/nauterm_app.dart';
import '../app/window_config.dart';
import '../terminal/terminal_config.dart';
import '../workspace/nauterm_workspace.dart';
import 'native_windowing.dart';

class MainWindow extends StatefulWidget {
  const MainWindow({
    super.key,
    required this.workspaceController,
    required this.isContentMounted,
    required this.onWindowCloseRequested,
    required this.onWindowDestroyed,
  });

  final NautermWorkspaceController workspaceController;
  final bool isContentMounted;
  final Future<void> Function() onWindowCloseRequested;
  final VoidCallback onWindowDestroyed;

  @override
  State<MainWindow> createState() => _MainWindowState();
}

class _MainWindowState extends State<MainWindow> with WidgetsBindingObserver {
  Size get _resolvedWindowSize {
    final w = terminalWindowWidth;
    final h = terminalWindowHeight;
    if (w != null && h != null && w > 0 && h > 0) {
      return Size(
        w.clamp(mainWindowMinSize.width, double.infinity),
        h.clamp(mainWindowMinSize.height, double.infinity),
      );
    }
    return mainWindowSize;
  }

  late final RegularWindowController _windowController =
      RegularWindowController(
        size: _resolvedWindowSize,
        constraints: const BoxConstraints(minWidth: 750, minHeight: 440),
        title: mainWindowTitle,
        delegate: _MainWindowDelegate(
          onCloseRequested: widget.onWindowCloseRequested,
          onDestroyed: widget.onWindowDestroyed,
        ),
      );

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      hideMainWindowTitleBar();
    });
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangePlatformBrightness() {
    _reapplyTitleBarAfterSystemAppearanceChange();
  }

  void _reapplyTitleBarAfterSystemAppearanceChange() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      hideMainWindowTitleBar();
      unawaited(
        Future<void>.delayed(const Duration(milliseconds: 120), () {
          if (mounted) {
            hideMainWindowTitleBar();
          }
        }),
      );
    });
  }

  void _startWindowDrag() {
    startMainWindowDrag();
  }

  void _toggleWindowMaximized() {
    _windowController.setMaximized(!_windowController.isMaximized);
    hideMainWindowTitleBar();
  }

  @override
  Widget build(BuildContext context) {
    return RegularWindow(
      controller: _windowController,
      child: widget.isContentMounted
          ? NautermApp(
              onOpenSettings: showSettingsWindow,
              onOpenTerminalSettings: () =>
                  showSettingsWindow(page: NautermSettingsPage.terminal),
              workspaceController: widget.workspaceController,
              onStartWindowDrag: _startWindowDrag,
              onToggleWindowMaximized: _toggleWindowMaximized,
            )
          : const SizedBox.shrink(),
    );
  }
}

class _MainWindowDelegate with RegularWindowControllerDelegate {
  _MainWindowDelegate({
    required this.onCloseRequested,
    required this.onDestroyed,
  });

  final Future<void> Function() onCloseRequested;
  final VoidCallback onDestroyed;

  @override
  void onWindowCloseRequested(RegularWindowController controller) {
    unawaited(onCloseRequested());
  }

  @override
  void onWindowDestroyed() {
    onDestroyed();
  }
}
