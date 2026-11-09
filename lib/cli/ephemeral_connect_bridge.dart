import 'dart:async';

import '../data/nauterm_paths.dart';
import 'ephemeral_connect_ipc.dart';
import 'ephemeral_connect_request.dart';

/// Process-wide bridge so the IPC listener can start before the UI mounts.
class EphemeralConnectBridge {
  EphemeralConnectBridge._();

  static final EphemeralConnectBridge instance = EphemeralConnectBridge._();

  EphemeralConnectIpcServer? _server;
  EphemeralConnectHandler? _handler;
  final List<EphemeralConnectRequest> _pending = <EphemeralConnectRequest>[];

  Future<void> startPrimaryListener(NautermPaths paths) async {
    if (_server != null) {
      return;
    }
    final server = EphemeralConnectIpcServer(
      paths: paths,
      onRequest: _dispatch,
    );
    await server.start();
    _server = server;
  }

  void attachHandler(EphemeralConnectHandler handler) {
    _handler = handler;
    final queued = List<EphemeralConnectRequest>.of(_pending);
    _pending.clear();
    for (final request in queued) {
      unawaited(Future.sync(() => handler(request)));
    }
  }

  Future<void> close() async {
    final server = _server;
    _server = null;
    _handler = null;
    _pending.clear();
    await server?.close();
  }

  Future<void> _dispatch(EphemeralConnectRequest request) async {
    final handler = _handler;
    if (handler == null) {
      _pending.add(request);
      return;
    }
    await Future.sync(() => handler(request));
  }
}
