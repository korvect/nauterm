import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import '../app/nauterm_log.dart';
import '../data/nauterm_paths.dart';
import 'ephemeral_connect_request.dart';

typedef EphemeralConnectHandler =
    FutureOr<void> Function(EphemeralConnectRequest request);

File ephemeralConnectEndpointFile(NautermPaths paths) {
  return File('${paths.dataDirectory.path}/ephemeral-connect.endpoint');
}

class EphemeralConnectEndpoint {
  const EphemeralConnectEndpoint({required this.port, required this.token});

  final int port;
  final String token;

  Map<String, Object> toJson() => <String, Object>{'port': port, 'token': token};

  static EphemeralConnectEndpoint? tryParse(String raw) {
    final trimmed = raw.trim();
    if (trimmed.isEmpty) {
      return null;
    }
    try {
      final decoded = jsonDecode(trimmed);
      if (decoded is Map) {
        final port = decoded['port'];
        final token = decoded['token'];
        if (port is int &&
            port >= 1 &&
            port <= 65535 &&
            token is String &&
            token.isNotEmpty) {
          return EphemeralConnectEndpoint(port: port, token: token);
        }
      }
    } on Object {
      // Fall through to legacy "port-only" endpoint files.
    }

    final port = int.tryParse(trimmed.split('\n').first.trim());
    if (port == null || port < 1 || port > 65535) {
      return null;
    }
    return EphemeralConnectEndpoint(port: port, token: '');
  }
}

String generateEphemeralConnectToken({Random? random}) {
  final rng = random ?? Random.secure();
  final bytes = List<int>.generate(24, (_) => rng.nextInt(256));
  return base64Url.encode(bytes).replaceAll('=', '');
}

Future<void> writeEphemeralConnectEndpointFile(
  File endpoint,
  EphemeralConnectEndpoint value,
) async {
  await endpoint.writeAsString(
    '${jsonEncode(value.toJson())}\n',
    flush: true,
  );
  if (!Platform.isWindows) {
    try {
      await Process.run('chmod', <String>['600', endpoint.path]);
    } on Object {
      // Best-effort; the directory itself may already be user-private.
    }
  }
}

/// Tries to forward [request] to a running primary instance.
///
/// Returns `true` when the primary accepted the request.
Future<bool> tryForwardEphemeralConnect(
  EphemeralConnectRequest request, {
  required NautermPaths paths,
}) async {
  final endpointFile = ephemeralConnectEndpointFile(paths);
  if (!await endpointFile.exists()) {
    return false;
  }
  final endpoint = EphemeralConnectEndpoint.tryParse(
    await endpointFile.readAsString(),
  );
  if (endpoint == null) {
    return false;
  }

  try {
    final socket = await Socket.connect(
      InternetAddress.loopbackIPv4,
      endpoint.port,
      timeout: const Duration(milliseconds: 800),
    );
    try {
      final payload = <String, Object?>{
        ...request.toJson(),
        'token': endpoint.token,
      };
      socket.write('${jsonEncode(payload)}\n');
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
      if (await endpointFile.exists()) {
        await endpointFile.delete();
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
    String? token,
  }) : _onRequest = onRequest,
       _token = token ?? generateEphemeralConnectToken();

  final NautermPaths paths;
  final EphemeralConnectHandler _onRequest;
  final String _token;

  ServerSocket? _server;
  StreamSubscription<Socket>? _subscription;
  bool _closed = false;

  String get token => _token;

  Future<void> start() async {
    if (_closed || _server != null) {
      return;
    }
    await paths.ensureCreated();
    final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    _server = server;
    final endpoint = ephemeralConnectEndpointFile(paths);
    await writeEphemeralConnectEndpointFile(
      endpoint,
      EphemeralConnectEndpoint(port: server.port, token: _token),
    );
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
      final decoded = jsonDecode(line);
      if (decoded is! Map) {
        socket.write('error\n');
        await socket.flush();
        return;
      }
      final token = decoded['token'];
      if (_token.isNotEmpty && token != _token) {
        socket.write('unauthorized\n');
        await socket.flush();
        return;
      }
      final request = EphemeralConnectRequest.fromJson(
        Map<String, dynamic>.from(decoded),
      );
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
