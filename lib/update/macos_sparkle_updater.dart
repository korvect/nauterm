import 'dart:io';

import 'package:flutter/services.dart';

import '../app/nauterm_log.dart';

class MacosSparkleUpdater {
  const MacosSparkleUpdater();

  static const MethodChannel _channel = MethodChannel(
    'com.korvect.nauterm/sparkle',
  );

  Future<bool> isConfigured() async =>
      await _channel.invokeMethod<bool>('isConfigured') ?? false;

  Future<void> checkForUpdates() async {
    final operation = NautermLog.begin('update', 'Check Sparkle update');
    try {
      if (!Platform.isMacOS) {
        throw UnsupportedError('Sparkle is only available on macOS.');
      }
      await _channel.invokeMethod<void>('checkForUpdates');
      operation.succeed();
    } on Object catch (error, stackTrace) {
      operation.fail(error, stackTrace: stackTrace);
      rethrow;
    }
  }
}
