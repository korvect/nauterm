// ignore_for_file: invalid_use_of_internal_member, implementation_imports

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/src/widgets/_window.dart';

import '../settings/settings_panel.dart';
import '../data/nauterm_paths.dart';
import '../data/sync_service.dart';
import '../workspace/nauterm_workspace.dart';
import '../update/desktop_update.dart';
import '../update/startup_update.dart';
import 'main_window.dart';
import 'native_windowing.dart';
import 'settings_window.dart';

class NautermRoot extends StatefulWidget {
  const NautermRoot({super.key});

  @override
  State<NautermRoot> createState() => _NautermRootState();
}

class _NautermRootState extends State<NautermRoot>
    implements NautermRootController {
  final NautermWorkspaceController _workspaceController =
      NautermWorkspaceController();
  late final SyncService _syncService;
  late final StartupUpdateCoordinator _updateCoordinator;
  bool _isMainWindowMounted = true;
  bool _isMainWindowContentMounted = true;
  bool _isSettingsWindowMounted = false;
  bool _isSettingsWindowContentMounted = false;
  RegularWindowController? _settingsWindowController;
  bool _isClosingMainWindow = false;
  int _mainWindowVisibilityRevision = 0;

  @override
  void initState() {
    super.initState();
    _syncService = SyncService(
      NautermPaths.resolve(),
      onSyncCompleted: notifyNautermSyncCompleted,
    );
    _updateCoordinator = StartupUpdateCoordinator(
      showNotice: _workspaceController.showUpdateNotice,
      loadSkippedVersion: _workspaceController.loadSkippedUpdateVersion,
      saveSkippedVersion: _workspaceController.saveSkippedUpdateVersion,
      restart: (disposition) => restartNautermApplication(
        launchInstalledExecutable:
            disposition == DesktopUpdateInstallDisposition.restartRequired,
      ),
    );
    registerNautermRoot(this);
    nautermDatabaseBulkChangeRevision.addListener(_reloadBulkDatabaseChanges);
    nautermSyncPreferencesRevision.addListener(_reloadSyncPreferences);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      unawaited(_preloadSecureDataAfterDatabaseReady());
      unawaited(_checkForUpdatesAfterWorkspaceReady());
    });
  }

  Future<void> _checkForUpdatesAfterWorkspaceReady() async {
    await _workspaceController.initialDataReady;
    if (mounted) {
      await _updateCoordinator.checkAtStartup();
    }
  }

  Future<void> _preloadSecureDataAfterDatabaseReady() async {
    await _workspaceController.initialDataReady;
    await preloadNautermSettingsSecureData();
    _syncService.start();
  }

  @override
  void dispose() {
    nautermDatabaseBulkChangeRevision.removeListener(
      _reloadBulkDatabaseChanges,
    );
    nautermSyncPreferencesRevision.removeListener(_reloadSyncPreferences);
    unregisterNautermRoot(this);
    unawaited(_syncService.close());
    _updateCoordinator.close();
    final settingsWindowController = _settingsWindowController;
    if (settingsWindowController != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        settingsWindowController.destroy();
      });
    }
    _workspaceController.dispose();
    super.dispose();
  }

  void _reloadBulkDatabaseChanges() {
    unawaited(_workspaceController.reloadData());
  }

  void _reloadSyncPreferences() {
    _syncService.preferencesChanged();
  }

  @override
  void showMainWindow() {
    _mainWindowVisibilityRevision++;
    if (_isMainWindowMounted) {
      requestMainWindowFocus();
      if (_isMainWindowContentMounted) {
        return;
      }
    }

    setState(() {
      _isMainWindowMounted = true;
      _isMainWindowContentMounted = true;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      requestMainWindowFocus();
    });
  }

  @override
  Future<void> closeMainWindow() async {
    if (_isClosingMainWindow) {
      return;
    }

    if (!_isMainWindowContentMounted) {
      if (_shouldHideMainWindowOnClose) {
        hideMainWindow();
      } else {
        await _syncService.close();
        await _workspaceController.flushAndClose();
        await quitNautermApplication();
      }
      return;
    }

    _isClosingMainWindow = true;
    final shouldClose = _shouldHideMainWindowOnClose
        ? !_workspaceController.hasActiveWorkspace ||
              await _workspaceController.confirmCloseWindowIfNeeded()
        : await _workspaceController.confirmQuitIfNeeded();
    _isClosingMainWindow = false;
    if (!mounted || !shouldClose) {
      return;
    }

    if (_shouldHideMainWindowOnClose) {
      await _workspaceController.saveRestorationStateForClose();
      final visibilityRevision = ++_mainWindowVisibilityRevision;
      markSettingsWindowRequested(false);
      hideMainWindow();
      hideSettingsNativeWindow();
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted || visibilityRevision != _mainWindowVisibilityRevision) {
          return;
        }
        setState(() {
          _isMainWindowContentMounted = false;
          _isSettingsWindowMounted = false;
          _isSettingsWindowContentMounted = false;
        });
      });
      WidgetsBinding.instance.scheduleFrame();
      return;
    }

    await _syncService.close();
    await _workspaceController.flushAndClose();
    await quitNautermApplication();
  }

  void _handleMainWindowDestroyed() {
    if (!_isMainWindowMounted) {
      return;
    }

    forgetMainWindow();
    markSettingsWindowRequested(false);
    setState(() {
      _isMainWindowMounted = false;
      _isMainWindowContentMounted = false;
      _isSettingsWindowMounted = false;
      _isSettingsWindowContentMounted = false;
    });
  }

  @override
  void showSettingsWindow() {
    markSettingsWindowRequested(true);

    if (_isSettingsWindowMounted || _settingsWindowController != null) {
      if (!_isSettingsWindowContentMounted) {
        setState(() {
          _isSettingsWindowContentMounted = true;
        });
        WidgetsBinding.instance.addPostFrameCallback((_) {
          if (mounted && _isSettingsWindowContentMounted) {
            requestSettingsWindowFocus();
          }
        });
      } else {
        requestSettingsWindowFocus();
      }
      return;
    }

    late final RegularWindowController controller;
    controller = createSettingsWindowController(
      onCloseRequested: hideSettingsWindow,
      onDestroyed: () => _handleSettingsWindowDestroyed(controller),
    );
    setState(() {
      _settingsWindowController = controller;
      _isSettingsWindowMounted = true;
      _isSettingsWindowContentMounted = true;
    });
  }

  void _handleSettingsWindowDestroyed(RegularWindowController controller) {
    if (!identical(_settingsWindowController, controller)) {
      return;
    }
    _settingsWindowController = null;
    markSettingsWindowRequested(false);
    if (mounted) {
      setState(() {
        _isSettingsWindowMounted = false;
        _isSettingsWindowContentMounted = false;
      });
    }
  }

  @override
  void hideSettingsWindow() {
    markSettingsWindowRequested(false);

    final controller = _settingsWindowController;
    if (!_isSettingsWindowMounted || controller == null) {
      return;
    }

    hideSettingsNativeWindow();
    if (defaultTargetPlatform == TargetPlatform.linux) {
      // Flutter 3.44 may continue presenting queued frames after a Linux
      // secondary view is destroyed. Keep the native view/compositor alive,
      // but unmount the expensive settings widget tree while it is hidden.
      setState(() {
        _isSettingsWindowContentMounted = false;
      });
      return;
    }

    setState(() {
      _settingsWindowController = null;
      _isSettingsWindowMounted = false;
      _isSettingsWindowContentMounted = false;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      controller.destroy();
    });
  }

  @override
  void closeSelectedTerminalTab() {
    _workspaceController.closeSelectedTerminalTab();
  }

  @override
  Future<bool> requestQuit() async {
    final shouldQuit = await _workspaceController.confirmQuitIfNeeded();
    if (!shouldQuit) {
      return false;
    }
    await _syncService.close();
    await _workspaceController.flushAndClose();
    return true;
  }

  @override
  Widget build(BuildContext context) {
    return ViewCollection(
      views: [
        if (_isSettingsWindowMounted && _settingsWindowController != null)
          SettingsWindow(
            controller: _settingsWindowController!,
            isContentMounted: _isSettingsWindowContentMounted,
          ),
        if (_isMainWindowMounted)
          MainWindow(
            workspaceController: _workspaceController,
            isContentMounted: _isMainWindowContentMounted,
            onWindowCloseRequested: closeMainWindow,
            onWindowDestroyed: _handleMainWindowDestroyed,
          ),
      ],
    );
  }
}

bool get _shouldHideMainWindowOnClose {
  return defaultTargetPlatform == TargetPlatform.macOS;
}
