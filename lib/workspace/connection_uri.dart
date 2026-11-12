enum ConnectionUriProtocol { ssh, sftp, telnet }

/// An external connection target. Credentials are entered in the connection UI.
class ConnectionUri {
  const ConnectionUri({
    required this.protocol,
    required this.host,
    required this.port,
    this.username = '',
    this.path,
  });

  final ConnectionUriProtocol protocol;
  final String host;
  final int port;
  final String username;
  final String? path;

  static ConnectionUri? tryParse(String value) {
    try {
      if (value.contains(RegExp(r'[\x00-\x20\x7f]|%(?![0-9a-fA-F]{2})'))) {
        return null;
      }
      // Uri normalizes an empty port and :0 away for unrecognized schemes.
      final authority = RegExp(r'^[^:]+://([^/?#]*)')
          .firstMatch(value)
          ?.group(1);
      if (authority == null ||
          authority.endsWith(':') ||
          RegExp(r':0+$').hasMatch(authority)) {
        return null;
      }
      final uri = Uri.parse(value);
      final protocol = switch (uri.scheme.toLowerCase()) {
        'ssh' => ConnectionUriProtocol.ssh,
        'sftp' => ConnectionUriProtocol.sftp,
        'telnet' => ConnectionUriProtocol.telnet,
        _ => null,
      };
      if (protocol == null ||
          !uri.hasAuthority ||
          uri.host.isEmpty ||
          uri.host.contains(RegExp(r'[\s\\/@?#]')) ||
          uri.authority.endsWith(':') ||
          uri.hasQuery ||
          uri.hasFragment) {
        return null;
      }
      // A literal colon separates the password; discard it before decoding.
      // An encoded colon remains part of the username.
      final username = Uri.decodeComponent(uri.userInfo.split(':').first);
      final path = Uri.decodeComponent(uri.path);
      if ('$username$path'.contains(RegExp(r'[\x00-\x1f\x7f]')) ||
          (protocol != ConnectionUriProtocol.sftp &&
              path.isNotEmpty &&
              path != '/')) {
        return null;
      }
      final port = uri.hasPort
          ? uri.port
          : protocol == ConnectionUriProtocol.telnet
          ? 23
          : 22;
      if (port < 1 || port > 65535) return null;
      return ConnectionUri(
        protocol: protocol,
        host: uri.host,
        port: port,
        username: username,
        path: path.isEmpty ? null : path,
      );
    } on FormatException {
      return null;
    }
  }
}
