enum EphemeralConnectProtocol { telnet, ssh }

class EphemeralConnectRequest {
  const EphemeralConnectRequest({
    required this.protocol,
    required this.host,
    required this.port,
    this.title,
    this.username,
    this.password,
    this.identityPath,
  });

  final EphemeralConnectProtocol protocol;
  final String host;
  final int port;
  final String? title;
  final String? username;
  final String? password;
  final String? identityPath;

  String get resolvedTitle {
    final explicit = title?.trim();
    if (explicit != null && explicit.isNotEmpty) {
      return explicit;
    }
    if (protocol == EphemeralConnectProtocol.ssh) {
      final user = username?.trim();
      if (user != null && user.isNotEmpty) {
        return '$user@$host';
      }
    }
    return '$host:$port';
  }

  Map<String, Object?> toJson() => {
    'protocol': protocol.name,
    'host': host,
    'port': port,
    if (title != null) 'title': title,
    if (username != null) 'username': username,
    if (password != null) 'password': password,
    if (identityPath != null) 'identityPath': identityPath,
  };

  static EphemeralConnectRequest? fromJson(Object? raw) {
    if (raw is! Map) {
      return null;
    }
    final map = raw.cast<String, Object?>();
    final protocolName = (map['protocol'] as String?)?.trim().toLowerCase();
    final host = (map['host'] as String?)?.trim();
    final portValue = map['port'];
    final port = switch (portValue) {
      int value => value,
      String value => int.tryParse(value.trim()),
      _ => null,
    };
    if (host == null || host.isEmpty || port == null) {
      return null;
    }
    final protocol = switch (protocolName) {
      'telnet' => EphemeralConnectProtocol.telnet,
      'ssh' => EphemeralConnectProtocol.ssh,
      _ => null,
    };
    if (protocol == null) {
      return null;
    }
    return EphemeralConnectRequest(
      protocol: protocol,
      host: host,
      port: port,
      title: (map['title'] as String?)?.trim(),
      username: (map['username'] as String?)?.trim(),
      password: (map['password'] as String?),
      identityPath: (map['identityPath'] as String?)?.trim(),
    );
  }
}
