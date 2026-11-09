import 'dart:async';
import 'dart:convert';
import 'dart:io';

import '../app/nauterm_log.dart';
import '../data/nauterm_paths.dart';
import 'ephemeral_connect_request.dart';

typedef EphemeralConnectHandler =
    FutureOr<void> Function(EphemeralConnectRequest request);

File ephemeralConnectEndpointFile(NautermPaths paths) {
  return File('${paths.dataDirectory.path}/ephemeral-connect.port');
}

/// Tries to forward [request] to a running primary instance.
///
/// Returns `true` when the primary accepted the request.
Future<bool> tryForwardEphemeralConnect(
  EphemeralConnectRequest request, {
  required NautermPaths paths,
}) async {
  final endpoint = ephemeralConnectEndpointFile(paths);
  if (!await endpoint.exists()) {
    return false;
  }
  final port = int.tryParse((await endpoint.readAsString()).trim());
  if (port == null || port < 1 || port > 65535) {
    return false;
  }

  try {
    final socket = await Socket.connect(
      InternetAddress.loopbackIPv4,
      port,
      timeout: const Duration(milliseconds: 800),
    );
    try {
      socket.write('${jsonEncode(request.toJson())}\n');
      await socket.flush();
      final response = await _readLine(socket).timeout(
        const Duration(seconds: 2),
      );
      return response.trim() == 'ok';
    } finally {
      await socket.close();
    }
  } on Object {
    // Drop stale endpoint files so the next cold start can become primary.
    try {
      if (await endpoint.exists()) {
        await endpoint.delete();
      }
    } on Object {
      // Best-effort cleanup.
    }
    return false;
  }
}

class EphemeralConnectIpcServer {
  EphemeralConnectIpcServer({
    required this.paths,
    required EphemeralConnectHandler onRequest,
  }) : _onRequest = onRequest;

  final NautermPaths paths;
  final EphemeralConnectHandler _onRequest;

  ServerSocket? _server;
  StreamSubscription<Socket>? _subscription;
  bool _closed = false;

  Future<void> start() async {
    if (_closed || _server != null) {
      return;
    }
    await paths.ensureCreated();
    final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    _server = server;
    final endpoint = ephemeralConnectEndpointFile(paths);
    await endpoint.writeAsString('${server.port}\n', flush: true);
    _subscription = server.listen(
      _handleClient,
      onError: (Object error, StackTrace stackTrace) {
        NautermLog.warning(
          'cli-ipc',
          'Ephemeral connect listener error.',
          error: error,
          stackTrace: stackTrace,
        );
      },
      cancelOnError: false,
    );
    NautermLog.info(
      'cli-ipc',
      'Ephemeral connect listener ready.',
      fields: {'port': server.port},
    );
  }

  Future<void> close() async {
    _closed = true;
    await _subscription?.cancel();
    _subscription = null;
    await _server?.close();
    _server = null;
    final endpoint = ephemeralConnectEndpointFile(paths);
    try {
      if (await endpoint.exists()) {
        await endpoint.delete();
      }
    } on Object {
      // Best-effort cleanup.
    }
  }

  Future<void> _handleClient(Socket socket) async {
    try {
      final line = await _readLine(socket).timeout(const Duration(seconds: 2));
      final request = EphemeralConnectRequest.fromJson(jsonDecode(line));
      if (request == null) {
        socket.write('error\n');
        await socket.flush();
        return;
      }
      // Acknowledge before UI work so secondary processes can exit quickly.
      socket.write('ok\n');
      await socket.flush();
      await socket.close();
      await Future.sync(() => _onRequest(request));
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'cli-ipc',
        'Failed to handle ephemeral connect request.',
        error: error,
        stackTrace: stackTrace,
      );
      try {
        socket.write('error\n');
        await socket.flush();
      } on Object {
        // Ignore write failures on a broken client socket.
      }
      try {
        await socket.close();
      } on Object {
        // Already closed.
      }
    }
  }
}

Future<String> _readLine(Socket socket) {
  return utf8.decoder.bind(socket).transform(const LineSplitter()).first;
}
