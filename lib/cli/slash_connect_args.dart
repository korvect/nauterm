import 'ephemeral_connect_request.dart';

/// Parses slash-flag ad-hoc connect arguments.
///
/// Supported forms:
/// - `/T /N "{title}" /TELNET {host} {port}`
/// - `/T /N "{title}" /SSH2 /L {user} [/P {port}] [/PASSWORD {pwd}] [/IDENTITY {keypath}] {host}`
EphemeralConnectRequest? parseSlashConnectArgs(List<String> args) {
  if (args.isEmpty) {
    return null;
  }

  String? title;
  String? protocolToken;
  String? username;
  String? password;
  String? identityPath;
  int? explicitPort;
  final positional = <String>[];

  for (var index = 0; index < args.length; index++) {
    final raw = args[index].trim();
    if (raw.isEmpty) {
      continue;
    }
    final token = raw.toUpperCase();
    if (token == '/T') {
      continue;
    }
    if (token == '/N') {
      if (index + 1 >= args.length) {
        return null;
      }
      title = args[++index].trim();
      continue;
    }
    if (token == '/L') {
      if (index + 1 >= args.length) {
        return null;
      }
      username = args[++index].trim();
      continue;
    }
    if (token == '/PASSWORD') {
      if (index + 1 >= args.length) {
        return null;
      }
      password = args[++index];
      continue;
    }
    if (token == '/IDENTITY') {
      if (index + 1 >= args.length) {
        return null;
      }
      identityPath = args[++index].trim();
      continue;
    }
    if (token == '/P') {
      if (index + 1 >= args.length) {
        return null;
      }
      explicitPort = int.tryParse(args[++index].trim());
      if (explicitPort == null) {
        return null;
      }
      continue;
    }
    if (token == '/TELNET' || token == '/SSH2' || token == '/SSH') {
      protocolToken = token;
      continue;
    }
    if (raw.startsWith('/')) {
      // Unknown slash flag — ignore to stay permissive for extra client flags.
      continue;
    }
    positional.add(raw);
  }

  if (protocolToken == null) {
    return null;
  }

  if (protocolToken == '/TELNET') {
    if (positional.length < 2) {
      return null;
    }
    final host = positional[positional.length - 2];
    final port = int.tryParse(positional[positional.length - 1]);
    if (!_isValidPort(port)) {
      return null;
    }
    return EphemeralConnectRequest(
      protocol: EphemeralConnectProtocol.telnet,
      host: host,
      port: port!,
      title: _emptyToNull(title),
    );
  }

  if (positional.isEmpty) {
    return null;
  }
  final host = positional.last;
  final port = explicitPort ?? 22;
  if (!_isValidPort(port)) {
    return null;
  }
  final user = _emptyToNull(username);
  if (user == null) {
    return null;
  }
  return EphemeralConnectRequest(
    protocol: EphemeralConnectProtocol.ssh,
    host: host,
    port: port,
    title: _emptyToNull(title),
    username: user,
    password: password,
    identityPath: _emptyToNull(identityPath),
  );
}

bool _isValidPort(int? port) => port != null && port >= 1 && port <= 65535;

String? _emptyToNull(String? value) {
  final trimmed = value?.trim();
  if (trimmed == null || trimmed.isEmpty) {
    return null;
  }
  return trimmed;
}
