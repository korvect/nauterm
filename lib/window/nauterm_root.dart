// ignore_for_file: invalid_use_of_internal_member, implementation_imports

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/src/widgets/_window.dart';
import 'package:nativeapi_flutter/windowing.dart';

import '../settings/settings_panel.dart';
import '../data/nauterm_paths.dart';
import '../data/sync_service.dart';
import '../workspace/nauterm_workspace.dart';
import '../update/desktop_update.dart';
import '../update/startup_update.dart';
import 'main_window.dart';
import 'connection_uri_channel.dart';
import 'native_windowing.dart';
import 'settings_window.dart';

class NautermRoot extends StatefulWidget {
  const NautermRoot({super.key, this.onInitialDataReady});

  final Future<void> Function()? onInitialDataReady;

  @override
  State<NautermRoot> createState() => _NautermRootState();
}

class _NautermRootState extends State<NautermRoot>
    implements NautermRootController {
  final NautermWorkspaceController _workspaceController =
      NautermWorkspaceController();
  late final SyncService _syncService;
  late final StartupUpdateCoordinator _updateCoordinator;
  late final ConnectionUriChannel _connectionUris;
  bool _isMainWindowMounted = true;
  bool _isMainWindowContentMounted = true;
  bool _isSettingsWindowMounted = false;
  WindowController? _settingsWindowController;
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
    _connectionUris = ConnectionUriChannel(
      onActivate: showMainWindow,
      onOpen: (uri) async {
        showMainWindow();
        await _workspaceController.openConnectionUri(uri);
      },
    )..start();
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
    if (!mounted) return;
    final onInitialDataReady = widget.onInitialDataReady;
    if (onInitialDataReady != null) unawaited(onInitialDataReady());
    await preloadNautermSettingsSecureData();
    _syncService.start();
  }

  @override
  void dispose() {
    _connectionUris.dispose();
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
    try {
      final shouldClose = _shouldHideMainWindowOnClose
          ? !_workspaceController.hasActiveWorkspace ||
                await _workspaceController.confirmCloseWindowIfNeeded()
          : await _workspaceController.confirmQuitIfNeeded();
      if (!mounted || !shouldClose) {
        return;
      }

      if (_shouldHideMainWindowOnClose) {
        await _workspaceController.withExitProgress(() async {
          await _workspaceController.saveRestorationStateForClose();
          await _workspaceController.flushSftpForClose();
        });
        if (!mounted) return;
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
          });
        });
        WidgetsBinding.instance.scheduleFrame();
        return;
      }

      await _finishQuitting();
      await quitNautermApplication();
    } finally {
      _isClosingMainWindow = false;
    }
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
    });
  }

  @override
  void showSettingsWindow() {
    markSettingsWindowRequested(true);

    if (_isSettingsWindowMounted || _settingsWindowController != null) {
      requestSettingsWindowFocus();
      return;
    }

    late final WindowController controller;
    controller = createSettingsWindowController(
      onCloseRequested: closeSettingsWindow,
      onDestroyed: () => _handleSettingsWindowDestroyed(controller),
    );
    setState(() {
      _settingsWindowController = controller;
      _isSettingsWindowMounted = true;
    });
    if (defaultTargetPlatform == TargetPlatform.windows) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted || !identical(_settingsWindowController, controller)) {
          return;
        }
        final window = controller.nativeWindow;
        if (window != null && !window.isVisible) {
          // The show hook can leave a recreated Win32 window hidden. Activate
          // the HWND directly after its Flutter view is mounted.
          controller.activate();
        }
      });
    }
  }

  void _handleSettingsWindowDestroyed(WindowController controller) {
    if (!identical(_settingsWindowController, controller)) {
      return;
    }
    _settingsWindowController = null;
    markSettingsWindowRequested(false);
    if (mounted) {
      setState(() {
        _isSettingsWindowMounted = false;
      });
    }
  }

  @override
  void closeSettingsWindow() {
    markSettingsWindowRequested(false);

    final controller = _settingsWindowController;
    if (!_isSettingsWindowMounted || controller == null) {
      return;
    }

    hideSettingsNativeWindow();
    setState(() {
      _settingsWindowController = null;
      _isSettingsWindowMounted = false;
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
  Future<void> openNewLocalTerminal() async {
    showMainWindow();
    await WidgetsBinding.instance.endOfFrame;
    await _workspaceController.openNewLocalTerminal();
  }

  @override
  Future<bool> requestQuit() async {
    if (_isClosingMainWindow) return false;
    _isClosingMainWindow = true;
    try {
      if (!await _workspaceController.confirmQuitIfNeeded()) return false;
      await _finishQuitting();
      return true;
    } finally {
      _isClosingMainWindow = false;
    }
  }

  Future<void> _finishQuitting() =>
      _workspaceController.withExitProgress(() async {
        await _syncService.close();
        await _workspaceController.flushAndClose();
      });

  @override
  Widget build(BuildContext context) {
    return ViewCollection(
      views: [
        if (_isSettingsWindowMounted && _settingsWindowController != null)
          SettingsWindow(controller: _settingsWindowController!),
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
