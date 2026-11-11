import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/workspace/host_connection_query.dart';

void main() {
  for (final input in [
    'ssh -p 2022 -l admin host.example',
    "ssh -p 2022 -l 'admin' -- 'host.example'",
    'ssh -ladmin -p2022 host.example',
    'ssh host.example -p 2022 -l admin',
    'ssh admin@host.example -p 2022',
    '  ssh   "admin"@"host.example"\t-p 2022  ',
  ]) {
    test('parses SSH login name, address and port: $input', () {
      final query = HostConnectionQuery.parse(input)!;
      expect(query.command, HostConnectionCommand.ssh);
      expect(query.host, 'host.example');
      expect(query.username, 'admin');
      expect(query.port, 2022);
    });
  }

  for (final input in [
    'telnet -l admin host.example 2022',
    'telnet admin@host.example 2022',
    'telnet -p2022 -ladmin host.example',
    'telnet host.example -l admin -p 2022',
    "telnet -l 'admin' -- 'host.example' 2022",
  ]) {
    test('parses Telnet login name, address and port: $input', () {
      final query = HostConnectionQuery.parse(input)!;
      expect(query.command, HostConnectionCommand.telnet);
      expect(query.host, 'host.example');
      expect(query.username, 'admin');
      expect(query.port, 2022);
    });
  }

  for (final input in [
    'admin@host.example -p 2022',
    '-p2022 admin@host.example',
    '-l admin host.example -p 2022',
  ]) {
    test('leaves the protocol unspecified: $input', () {
      final query = HostConnectionQuery.parse(input)!;
      expect(query.command, isNull);
      expect(query.username, 'admin');
      expect(query.host, 'host.example');
      expect(query.port, 2022);
    });
  }

  for (final input in ['host.example', 'admin@host.example', '10.0.0.1']) {
    test('keeps default ports unspecified: $input', () {
      final query = HostConnectionQuery.parse(input)!;
      expect(query.command, isNull);
      expect(query.port, isNull);
    });
  }

  for (final command in ['ssh', 'telnet']) {
    test('$command permits a missing username and a default port', () {
      final query = HostConnectionQuery.parse('$command host.example')!;
      expect(query.username, isEmpty);
      expect(query.port, isNull);
    });
  }

  test('preserves quoted backslashes and username case', () {
    final query = HostConnectionQuery.parse(
      r'ssh -l "DOMAIN\Admin" HOST.EXAMPLE',
    )!;
    expect(query.username, r'DOMAIN\Admin');
    expect(query.host, 'HOST.EXAMPLE');
  });

  test('explicit login name takes precedence over user@host', () {
    final query = HostConnectionQuery.parse('ssh -l admin other@host.example')!;
    expect(query.username, 'admin');
  });

  for (final address in ['2001:db8::1', '[2001:db8::1]']) {
    test('accepts IPv6 without confusing it with a port: $address', () {
      final query = HostConnectionQuery.parse('ssh admin@$address -p 2022')!;
      expect(query.host, '2001:db8::1');
      expect(query.port, 2022);
      expect(query.label, 'admin@[2001:db8::1] -p 2022');
    });
  }

  for (final input in [
    'ssh host.example -p',
    'ssh host.example -p invalid',
    'ssh -i key admin@host.example',
    'ssh -o Port=2022 admin@host.example',
    'ssh host.example command',
    'ssh host.example; whoami',
    'ssh host.example\ntelnet other.example',
    'ssh host.example | cat',
    'ssh [2001:db8::1',
    'host:example',
    'group:test',
  ]) {
    test('uses the target information it can parse: $input', () {
      expect(HostConnectionQuery.parse(input), isNotNull);
    });
  }

  test('keeps a parsed host when a port value is missing or nonnumeric', () {
    for (final input in ['ssh host.example -p', 'ssh host.example -p abc']) {
      final query = HostConnectionQuery.parse(input)!;
      expect(query.host, 'host.example');
      expect(query.port, isNull);
    }
  });

  for (final quote in ["'", '"']) {
    test('an unclosed $quote preserves a separately supplied host', () {
      for (final input in [
        'ssh host.example -l ${quote}admin',
        'ssh -l admin ${quote}host.example',
      ]) {
        final query = HostConnectionQuery.parse(input)!;
        expect(query.host, 'host.example');
        expect(query.username, 'admin');
      }
    });
  }

  test('uses numeric ports as entered without range validation', () {
    expect(HostConnectionQuery.parse('ssh host.example -p 0')!.port, 0);
    expect(HostConnectionQuery.parse('ssh host.example -p 65536')!.port, 65536);
  });

  for (final input in [
    '',
    'ssh',
    'telnet',
    'ssh -l admin',
    'ssh admin@',
    "ssh -l 'admin host.example",
    'ssh -l "admin host.example',
  ]) {
    test('requires a host address to connect: $input', () {
      expect(HostConnectionQuery.parse(input), isNull);
    });
  }
}
