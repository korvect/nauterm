// ignore_for_file: invalid_use_of_internal_member, implementation_imports

import 'dart:async';
import 'dart:io';
import 'dart:ui';

import 'package:flutter/src/foundation/_features.dart' as flutter_features;
import 'package:flutter/widgets.dart';
import 'package:package_info_plus/package_info_plus.dart';

import 'app/nauterm_log.dart';
import 'app/posthog_analytics.dart';
import 'app/third_party_licenses.dart';
import 'data/nauterm_config_store.dart';
import 'data/nauterm_data_store.dart';
import 'data/nauterm_paths.dart';
import 'terminal/terminal_config.dart';
import 'terminal/terminal_ffi.dart';
import 'window/native_windowing.dart';
import 'window/nauterm_root.dart';

const _firstLaunchMetadataKey = 'has_launched_before';
const _pendingFirstOpenMetadataKey = 'analytics_first_open_pending';
const _sentFirstOpenMetadataKey = 'analytics_first_open_sent';

Future<void> main() async {
  flutter_features.isWindowingEnabled = true;
  WidgetsFlutterBinding.ensureInitialized();
  final paths = NautermPaths.resolve();
  NautermLog.initialize(paths.runtimeLogsDirectory);
  _installUnhandledErrorLogging();
  NautermLog.info(
    'application',
    'Application startup initiated.',
    fields: {'platform': Platform.operatingSystem},
  );
  registerNautermThirdPartyLicenses();
  initializeNativeTerminalRuntime();
  final configOperation = NautermLog.begin(
    'configuration',
    'Runtime configuration load',
  );
  try {
    final configStore = NautermConfigStore(paths);
    final settings = await configStore.loadRuntimeSettings();
    applyNautermRuntimeSettings(settings);
    configOperation.succeed();
  } on Object catch (error, stackTrace) {
    // Keep the bundled defaults when the user config cannot be loaded.
    configOperation.fail(error, stackTrace: stackTrace);
  }
  configureNativeWindowing();
  runWidget(const NautermRoot());
  NautermLog.info('application', 'Application UI mounted.');
  unawaited(_logApplicationMetadata());
  unawaited(initAnalytics());
}

void _installUnhandledErrorLogging() {
  final previousFlutterError = FlutterError.onError;
  FlutterError.onError = (details) {
    NautermLog.error(
      'flutter',
      'Unhandled Flutter framework error.',
      error: details.exception,
      stackTrace: details.stack,
    );
    previousFlutterError?.call(details);
  };
  final previousPlatformError = PlatformDispatcher.instance.onError;
  PlatformDispatcher.instance.onError = (error, stackTrace) {
    NautermLog.error(
      'isolate',
      'Unhandled asynchronous error.',
      error: error,
      stackTrace: stackTrace,
    );
    return previousPlatformError?.call(error, stackTrace) ?? false;
  };
}

Future<void> _logApplicationMetadata() async {
  try {
    final packageInfo = await PackageInfo.fromPlatform();
    NautermLog.info(
      'application',
      'Application metadata loaded.',
      fields: {
        'version': packageInfo.version,
        'build': packageInfo.buildNumber,
      },
    );
  } on Object catch (error, stackTrace) {
    NautermLog.warning(
      'application',
      'Application metadata could not be loaded.',
      error: error,
      stackTrace: stackTrace,
    );
  }
}

Future<void> initAnalytics() async {
  const posthogKey = String.fromEnvironment('NAUTERM_POSTHOG_API_KEY');
  if (posthogKey.isEmpty) {
    NautermLog.info(
      'analytics',
      'Analytics is disabled because the project key is empty.',
    );
    return;
  }
  const posthogHost = String.fromEnvironment('NAUTERM_POSTHOG_HOST');

  try {
    final packageInfo = await PackageInfo.fromPlatform();
    final paths = NautermPaths.resolve();
    final store = NautermDataStore.openPath(paths.databasePath);
    late final String deviceId;
    int? firstLaunchAt;
    int? pendingFirstOpenAt;
    try {
      deviceId = store.deviceId;
      final existing = store.getAppMetadata(_firstLaunchMetadataKey);
      final pending = store.getAppMetadata(_pendingFirstOpenMetadataKey);
      final sent = store.getAppMetadata(_sentFirstOpenMetadataKey);
      if (existing == null) {
        firstLaunchAt = DateTime.now().millisecondsSinceEpoch;
        pendingFirstOpenAt = firstLaunchAt;
        store
          ..setAppMetadata(_firstLaunchMetadataKey, firstLaunchAt.toString())
          ..setAppMetadata(
            _pendingFirstOpenMetadataKey,
            firstLaunchAt.toString(),
          );
      } else {
        firstLaunchAt = int.tryParse(existing);
        if (pending != null && sent == null) {
          pendingFirstOpenAt = int.tryParse(pending);
        }
      }
    } finally {
      store.dispose();
    }

    PostHogAnalytics.init(
      apiKey: posthogKey,
      distinctId: deviceId,
      host: posthogHost,
      appName: packageInfo.appName,
      appNamespace: packageInfo.packageName,
      appVersion: packageInfo.version,
      appBuild: packageInfo.buildNumber,
    );

    final firstOpenTimestamp = pendingFirstOpenAt;
    NautermLog.info(
      'analytics',
      firstOpenTimestamp == null
          ? 'Analytics initialized.'
          : 'Analytics initialized with a pending first_open event.',
    );
    final firstOpenRequest = firstOpenTimestamp == null
        ? null
        : PostHogAnalytics.capture('first_open', {
            'first_launch_at': firstOpenTimestamp,
          });
    final screenRequest = PostHogAnalytics.screen(packageInfo.appName);
    final appStartedRequest = PostHogAnalytics.capture('app_started', {
      if (firstLaunchAt != null)
        r'$set_once': <String, Object>{
          'first_launch_at': DateTime.fromMillisecondsSinceEpoch(
            firstLaunchAt,
            isUtc: true,
          ).toIso8601String(),
        },
    });

    if (firstOpenRequest != null) {
      final accepted = await firstOpenRequest;
      if (accepted) {
        try {
          final markerStore = NautermDataStore.openPath(paths.databasePath);
          try {
            markerStore.setAppMetadata(
              _sentFirstOpenMetadataKey,
              firstOpenTimestamp.toString(),
            );
          } finally {
            markerStore.dispose();
          }
        } on Object {
          // A later launch can safely retry the same first-open event.
        }
      }
    }
    await screenRequest;
    await appStartedRequest;
  } on Object catch (error, stackTrace) {
    // Analytics must never prevent the application from starting.
    NautermLog.warning(
      'analytics',
      'Analytics initialization failed.',
      error: error,
      stackTrace: stackTrace,
    );
  }
}
