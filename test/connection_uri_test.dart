import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/workspace/connection_uri.dart';

void main() {
  for (final (scheme, protocol, defaultPort) in [
    ('ssh', ConnectionUriProtocol.ssh, 22),
    ('sftp', ConnectionUriProtocol.sftp, 22),
    ('telnet', ConnectionUriProtocol.telnet, 23),
  ]) {
    test('$scheme parses username, host and port without shell syntax', () {
      final target = ConnectionUri.tryParse(
        '$scheme://Admin@example.com:2022',
      )!;
      expect(target.protocol, protocol);
      expect(target.username, 'Admin');
      expect(target.host, 'example.com');
      expect(target.port, 2022);
      expect(target.path, isNull);
      expect(
        ConnectionUri.tryParse('$scheme://example.com')!.port,
        defaultPort,
      );
    });

    test('$scheme discards the password and preserves the connection target', () {
      final target = ConnectionUri.tryParse(
        '$scheme://Admin:never%40use:this%00password@example.com:2022${scheme == 'sftp' ? '/home/user%20files' : ''}',
      )!;
      expect(target.protocol, protocol);
      expect(target.username, 'Admin');
      expect(target.host, 'example.com');
      expect(target.port, 2022);
      expect(target.path, scheme == 'sftp' ? '/home/user files' : isNull);
      expect(
        ConnectionUri.tryParse('$scheme://Admin:@example.com')!.username,
        'Admin',
      );
      expect(
        ConnectionUri.tryParse('$scheme://:discard@example.com')!.username,
        isEmpty,
      );
    });
  }

  test('an encoded colon belongs to the username rather than delimiting a password', () {
    expect(
      ConnectionUri.tryParse('ssh://first%3Alast:discard@example.com')!
          .username,
      'first:last',
    );
    expect(
      ConnectionUri.tryParse('ssh://user%3Asecret@example.com')!.username,
      'user:secret',
    );
  });

  test('decodes an SFTP path and a username exactly once', () {
    final target = ConnectionUri.tryParse(
      'sftp://first%20last@example.com/home/hello%20world/%E4%B8%AD%E6%96%87/%252F',
    )!;
    expect(target.username, 'first last');
    expect(target.path, '/home/hello world/中文/%2F');
    expect(ConnectionUri.tryParse('sftp://example.com/')!.path, '/');
  });

  test('accepts IPv4, IPv6, mixed-case schemes and an absent username', () {
    expect(ConnectionUri.tryParse('SSH://127.0.0.1:22')!.username, isEmpty);
    final ipv6 = ConnectionUri.tryParse('sftp://user@[2001:db8::1]:2022/tmp')!;
    expect(ipv6.host, '2001:db8::1');
    expect(ipv6.port, 2022);
    expect(ipv6.path, '/tmp');
  });

  for (final invalid in [
    '',
    'https://example.com',
    'ssh:example.com',
    'ssh:///example.com',
    'ssh://',
    'ssh://host:',
    'ssh://host:abc',
    'ssh://host:0',
    'ssh://host:65536',
    'ssh://host?password=secret',
    'sftp://host/tmp?token=secret',
    'telnet://host#secret',
    'ssh://host/run-command',
    'telnet://host/run-command',
    'ssh://host with spaces',
    'ssh://user%0Aname@host',
    'sftp://host/tmp/%00',
    'sftp://host/tmp/%invalid',
    'ssh://[invalid-ipv6]',
  ]) {
    test('rejects unsupported or unsafe URI: $invalid', () {
      expect(ConnectionUri.tryParse(invalid), isNull);
    });
  }
}
