// ignore_for_file: invalid_use_of_internal_member, implementation_imports

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/src/widgets/_window.dart';
import 'package:nativeapi_flutter/nativeapi_flutter.dart' show DisplayManager;
import 'package:nativeapi_flutter/windowing.dart';

import '../app/nauterm_theme.dart';
import '../app/window_config.dart';
import '../settings/settings_app.dart';
import '../terminal/terminal_config.dart';
import 'native_windowing.dart';

class SettingsWindow extends StatefulWidget {
  const SettingsWindow({
    super.key,
    required this.controller,
    required this.isContentMounted,
  });

  final WindowController controller;
  final bool isContentMounted;

  @override
  State<SettingsWindow> createState() => _SettingsWindowState();
}

class _SettingsWindowState extends State<SettingsWindow> {
  late bool _contentReady;
  Timer? _contentReadyTimer;

  @override
  void initState() {
    super.initState();
    _contentReady = defaultTargetPlatform != TargetPlatform.linux;
    if (!_contentReady) {
      // Flutter 3.44 can submit the secondary view's first frame before GTK
      // has delivered its final content size. Present a lightweight themed
      // frame first, then mount the UI after native size negotiation settles.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted || _contentReady) {
          return;
        }
        _contentReadyTimer = Timer(const Duration(milliseconds: 250), () {
          if (!mounted || _contentReady) {
            return;
          }
          setState(() {
            _contentReady = true;
          });
        });
      });
    }
  }

  @override
  void dispose() {
    _contentReadyTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Window(
      controller: widget.controller,
      child: _contentReady && widget.isContentMounted
          ? const NautermSettingsApp()
          : const _SettingsWindowPlaceholder(),
    );
  }
}

class _SettingsWindowPlaceholder extends StatelessWidget {
  const _SettingsWindowPlaceholder();

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<AppThemeMode>(
      valueListenable: appThemeModeListenable,
      builder: (context, mode, _) {
        final brightness = switch (mode) {
          AppThemeMode.light => Brightness.light,
          AppThemeMode.dark => Brightness.dark,
          AppThemeMode.system =>
            WidgetsBinding.instance.platformDispatcher.platformBrightness,
        };
        return ColoredBox(
          color: brightness == Brightness.dark
              ? NautermPalette.dark.surface
              : NautermPalette.light.surface,
        );
      },
    );
  }
}

WindowController createSettingsWindowController({
  required VoidCallback onCloseRequested,
  required VoidCallback onDestroyed,
}) {
  final controller = WindowController(
    size: settingsWindowSize,
    constraints: BoxConstraints(
      minWidth: settingsWindowMinSize.width,
      minHeight: settingsWindowMinSize.height,
    ),
    title: settingsWindowTitle,
    delegate: _SettingsWindowDelegate(
      onCloseRequested: onCloseRequested,
      onDestroyed: onDestroyed,
    ),
  );
  final nativeWindow = controller.nativeWindow;
  if (nativeWindow != null) {
    initializeSettingsNativeWindow(
      nativeWindow,
      DisplayManager.instance.getPrimary(),
    );
  }
  return controller;
}

class _SettingsWindowDelegate with WindowControllerDelegate {
  _SettingsWindowDelegate({
    required this.onCloseRequested,
    required this.onDestroyed,
  });

  final VoidCallback onCloseRequested;
  final VoidCallback onDestroyed;

  @override
  void onWindowCloseRequested(WindowController controller) {
    if (defaultTargetPlatform == TargetPlatform.linux) {
      onCloseRequested();
      return;
    }

    hideSettingsNativeWindow();
    controller.destroy();
  }

  @override
  void onWindowDestroyed() {
    onDestroyed();
  }
}
