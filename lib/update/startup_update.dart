import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:package_info_plus/package_info_plus.dart';

import '../app/nauterm_log.dart';
import 'desktop_update.dart';
import 'macos_sparkle_updater.dart';

enum StartupUpdatePhase {
  available,
  downloading,
  installing,
  restartRequired,
  installerLaunched,
  error,
}

@immutable
class StartupUpdateNotice {
  const StartupUpdateNotice({
    required this.version,
    required this.phase,
    required this.onUpdate,
    required this.onSkip,
    required this.onDismiss,
    this.progress,
    this.error,
  });

  final String version;
  final StartupUpdatePhase phase;
  final double? progress;
  final String? error;
  final VoidCallback onUpdate;
  final VoidCallback onSkip;
  final VoidCallback onDismiss;
}

typedef StartupUpdateNoticeSink = void Function(StartupUpdateNotice? notice);
typedef SkippedUpdateVersionLoader = Future<String?> Function();
typedef SkippedUpdateVersionSaver = Future<void> Function(String version);
typedef NautermUpdateRestarter = Future<void> Function(
  DesktopUpdateInstallDisposition disposition,
);

class StartupUpdateCoordinator {
  StartupUpdateCoordinator({
    required this._showNotice,
    required this._loadSkippedVersion,
    required this._saveSkippedVersion,
    required this._restart,
    this._service,
    this._sparkle = const MacosSparkleUpdater(),
    Future<PackageInfo> Function()? loadPackageInfo,
  }) : _loadPackageInfo = loadPackageInfo ?? PackageInfo.fromPlatform;

  final StartupUpdateNoticeSink _showNotice;
  final SkippedUpdateVersionLoader _loadSkippedVersion;
  final SkippedUpdateVersionSaver _saveSkippedVersion;
  final NautermUpdateRestarter _restart;
  final MacosSparkleUpdater _sparkle;
  final Future<PackageInfo> Function() _loadPackageInfo;
  DesktopUpdateService? _service;
  DesktopUpdateRelease? _release;
  DesktopUpdateCancellation? _downloadCancellation;
  DesktopUpdateInstallDisposition? _installDisposition;
  bool _checked = false;
  bool _busy = false;

  Future<void> checkAtStartup() async {
    if (_checked) return;
    _checked = true;
    try {
      final info = await _loadPackageInfo();
      final service = _service ??= DesktopUpdateService();
      final result = await service.check(info.version);
      final release = result.release;
      if (release == null || (!Platform.isMacOS && !release.canDownloadInApp)) {
        return;
      }
      final skippedVersion = await _loadSkippedVersion();
      if (skippedVersion != null &&
          compareVersions(release.version, skippedVersion) == 0) {
        return;
      }
      _release = release;
      _emit(StartupUpdatePhase.available);
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'update',
        'Automatic update check failed.',
        error: error,
        stackTrace: stackTrace,
      );
    }
  }

  Future<void> update() async {
    final release = _release;
    if (release == null || _busy) return;
    if (_installDisposition case final disposition?) {
      await _restart(disposition);
      return;
    }
    _busy = true;
    VerifiedDesktopUpdate? download;
    var handedOff = false;
    try {
      if (Platform.isMacOS) {
        await _sparkle.checkForUpdates();
        _showNotice(null);
        return;
      }
      final service = _service!;
      final cancellation = DesktopUpdateCancellation();
      _downloadCancellation = cancellation;
      _emit(StartupUpdatePhase.downloading, progress: 0);
      download = await service.download(
        release,
        cancellation: cancellation,
        onProgress: (received, total) {
          _emit(
            StartupUpdatePhase.downloading,
            progress: total > 0 ? (received / total).clamp(0, 1) : null,
          );
        },
      );
      if (cancellation.isCancelled) return;
      _emit(StartupUpdatePhase.installing);
      final disposition = await service.installVerified(
        download,
        release.asset.kind,
      );
      handedOff =
          disposition != DesktopUpdateInstallDisposition.restartRequired;
      if (disposition == DesktopUpdateInstallDisposition.installerLaunched) {
        _emit(StartupUpdatePhase.installerLaunched);
        return;
      }
      _installDisposition = disposition;
      _emit(StartupUpdatePhase.restartRequired);
    } on DesktopUpdateDownloadCancelled {
      return;
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'update',
        'Automatic update installation failed.',
        error: error,
        stackTrace: stackTrace,
      );
      _emit(StartupUpdatePhase.error, error: error.toString());
    } finally {
      _downloadCancellation = null;
      _busy = false;
      if (download != null && !handedOff) {
        try {
          await download.discard();
        } on Object catch (error, stackTrace) {
          NautermLog.warning(
            'update',
            'Unable to remove a staged update package.',
            error: error,
            stackTrace: stackTrace,
          );
        }
      }
    }
  }

  Future<void> skip() async {
    final release = _release;
    if (release == null || _busy) return;
    try {
      await _saveSkippedVersion(release.version);
      _showNotice(null);
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'update',
        'Unable to save the skipped update version.',
        error: error,
        stackTrace: stackTrace,
      );
      _emit(StartupUpdatePhase.error, error: error.toString());
    }
  }

  void dismiss() => _showNotice(null);

  void _emit(StartupUpdatePhase phase, {double? progress, String? error}) {
    final release = _release;
    if (release == null) return;
    _showNotice(
      StartupUpdateNotice(
        version: release.version,
        phase: phase,
        progress: progress,
        error: error,
        onUpdate: () => unawaited(update()),
        onSkip: () => unawaited(skip()),
        onDismiss: dismiss,
      ),
    );
  }

  void close() {
    _downloadCancellation?.cancel();
    _service?.close();
    _service = null;
  }
}
