import 'dart:async';

import 'package:flutter/services.dart';

import '../app/nauterm_log.dart';

/// Drains the native queue after installing the handler, including cold launches.
class ConnectionUriChannel {
  ConnectionUriChannel({required this.onOpen, required this.onActivate});

  final Future<void> Function(String uri) onOpen;
  final void Function() onActivate;
  static const channel = MethodChannel('com.korvect.nauterm/connection_uri');
  static const schemes = ['ssh', 'sftp', 'telnet'];

  static Future<Map<String, bool?>> defaultHandlers() async {
    final values = await channel.invokeMapMethod<String, bool?>(
      'getDefaultHandlers',
    );
    return {for (final scheme in schemes) scheme: values?[scheme]};
  }

  static Future<void> setDefaultHandler(String scheme) {
    if (!schemes.contains(scheme)) {
      throw ArgumentError.value(scheme, 'scheme', 'Unsupported URI scheme');
    }
    return channel.invokeMethod<void>('setDefaultHandler', scheme);
  }

  Future<void> _pending = Future<void>.value();
  bool _disposed = false;

  void start() {
    channel.setMethodCallHandler((call) async {
      if (_disposed) return;
      switch (call.method) {
        case 'urisAvailable':
          unawaited(_drain());
        case 'activate':
          onActivate();
      }
    });
    unawaited(_drain());
  }

  Future<void> _drain() {
    return _pending = _pending.then((_) async {
      if (_disposed) return;
      try {
        final uris = await channel.invokeListMethod<String>('takePendingUris');
        for (final uri in uris ?? const <String>[]) {
          if (_disposed) return;
          try {
            await onOpen(uri);
          } on Object {
            NautermLog.warning('connection', 'Unable to open connection link.');
          }
        }
      } on MissingPluginException {
        // Unit tests and older runners do not have this channel.
      } on Object {
        // Never include external URIs or platform error details in diagnostics.
        NautermLog.warning('connection', 'Unable to open connection link.');
      }
    });
  }

  void dispose() {
    _disposed = true;
    channel.setMethodCallHandler(null);
  }
}
