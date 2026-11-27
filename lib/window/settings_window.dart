// ignore_for_file: invalid_use_of_internal_member, implementation_imports

import 'package:material_ui/material_ui.dart';
import 'package:flutter/src/widgets/_window.dart';
import 'package:nativeapi_flutter/nativeapi_flutter.dart' show DisplayManager;
import 'package:nativeapi_flutter/windowing.dart';

import '../app/window_config.dart';
import '../settings/settings_app.dart';
import 'native_windowing.dart';

class SettingsWindow extends StatelessWidget {
  const SettingsWindow({super.key, required this.controller});

  final WindowController controller;

  @override
  Widget build(BuildContext context) {
    return Window(controller: controller, child: const NautermSettingsApp());
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
    // Let the root unmount the settings view before destroying the window.
    onCloseRequested();
  }

  @override
  void onWindowDestroyed() {
    onDestroyed();
  }
}
