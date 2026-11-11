import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/cli/ephemeral_connect_request.dart';
import 'package:nauterm/cli/slash_connect_args.dart';

void main() {
  group('parseSlashConnectArgs', () {
    test('parses telnet ad-hoc launch flags', () {
      final request = parseSlashConnectArgs([
        '/T',
        '/N',
        'R1',
        '/TELNET',
        '192.168.1.1',
        '5001',
      ]);

      expect(request, isNotNull);
      expect(request!.protocol, EphemeralConnectProtocol.telnet);
      expect(request.host, '192.168.1.1');
      expect(request.port, 5001);
      expect(request.title, 'R1');
      expect(request.resolvedTitle, 'R1');
    });

    test('parses ssh2 flags with username and custom port', () {
      final request = parseSlashConnectArgs([
        '/t',
        '/n',
        'core',
        '/ssh2',
        '/L',
        'admin',
        '/P',
        '2222',
        '10.0.0.5',
      ]);

      expect(request, isNotNull);
      expect(request!.protocol, EphemeralConnectProtocol.ssh);
      expect(request.host, '10.0.0.5');
      expect(request.port, 2222);
      expect(request.username, 'admin');
      expect(request.title, 'core');
    });

    test('defaults ssh port to 22 when /P is omitted', () {
      final request = parseSlashConnectArgs([
        '/SSH2',
        '/L',
        'root',
        'example.test',
      ]);

      expect(request, isNotNull);
      expect(request!.port, 22);
      expect(request.username, 'root');
      expect(request.resolvedTitle, 'root@example.test');
    });

    test('returns null for unrelated arguments', () {
      expect(parseSlashConnectArgs(const []), isNull);
      expect(parseSlashConnectArgs(['--help']), isNull);
      expect(parseSlashConnectArgs(['/T', '/N', 'only-title']), isNull);
    });

    test('rejects invalid ports', () {
      expect(
        parseSlashConnectArgs(['/TELNET', '192.168.1.1', '0']),
        isNull,
      );
      expect(
        parseSlashConnectArgs(['/SSH2', '/L', 'u', '/P', '70000', 'h']),
        isNull,
      );
    });

    test('parses ssh password and identity path', () {
      final request = parseSlashConnectArgs([
        '/SSH2',
        '/L',
        'admin',
        '/PASSWORD',
        's3cret',
        '/IDENTITY',
        '/home/user/.ssh/id_ed25519',
        '10.0.0.5',
      ]);

      expect(request, isNotNull);
      expect(request!.password, 's3cret');
      expect(request.identityPath, '/home/user/.ssh/id_ed25519');
      expect(request.username, 'admin');
      expect(request.host, '10.0.0.5');
    });

    test('keeps password whitespace intact', () {
      final request = parseSlashConnectArgs([
        '/SSH2',
        '/L',
        'u',
        '/PASSWORD',
        '  spaced ',
        'host.test',
      ]);

      expect(request?.password, '  spaced ');
    });
  });
}
