import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  for (final enabled in [false, true]) {
    test(
      'SFTP arguments retain compatibility mode after changing host trust: $enabled',
      () {
        final profile = SshConnectionProfile(
          host: 'appliance.test',
          port: 2022,
          username: 'admin',
          knownHostsPath: '/tmp/known_hosts',
          sshCompatibilityMode: enabled,
        );
        final arguments = sftpConnectionArgumentsForTesting(
          profile,
          directory: '/etc',
          hostKeyTrustMode: SshHostKeyTrustMode.acceptOnce,
        );
        expect(arguments['sshCompatibilityMode'], enabled);
        expect(arguments['host'], profile.host);
        expect(arguments['port'], profile.port);
        expect(arguments['directory'], '/etc');
        expect(arguments['knownHostsPath'], profile.knownHostsPath);
        expect(
          arguments['hostKeyTrustMode'],
          SshHostKeyTrustMode.acceptOnce.wireValue,
        );
      },
    );

    test('retry reloads saved host compatibility mode: $enabled', () {
      final current = SshConnectionProfile(
        host: 'appliance.test',
        port: 22,
        username: 'admin',
        knownHostsPath: '/tmp/known_hosts',
        hostId: 7,
        sshCompatibilityMode: !enabled,
      );
      final host = HostEntry(
        id: 7,
        name: 'Appliance',
        type: NautermHostType.remote,
        config: {
          'ssh': {'compatibility_mode': enabled},
        },
      );
      final refreshed = refreshSavedHostSshProfile(
        current: current,
        host: host,
        address: current.host,
        port: current.port,
        username: current.username,
        environment: const {'TERM_TEST': '1'},
      );
      expect(refreshed.sshCompatibilityMode, enabled);
      expect(
        refreshed.nativeEnvironment.containsKey(
          '__NAUTERM_SSH_COMPATIBILITY_MODE',
        ),
        enabled,
      );
      expect(refreshed.nativeEnvironment['TERM_TEST'], '1');
    });
  }

  test('saved host changes replace the failed connection profile', () {
    const current = SshConnectionProfile(
      host: 'old.example.com',
      port: 22,
      username: 'old-user',
      knownHostsPath: '/tmp/known_hosts',
      hostId: 7,
      identityId: 3,
      label: 'Old host',
      password: 'old-password',
      environment: {'OLD': '1'},
      encoding: 'UTF-8',
    );
    const host = HostEntry(
      id: 7,
      name: 'Updated host',
      identityId: 9,
      host: 'new.example.com',
      port: 2202,
      shellPath: '/bin/zsh',
      encoding: 'ISO-8859-1',
      type: NautermHostType.remote,
    );
    const proxy = TerminalProxyConfig(
      type: 'socks5',
      host: 'proxy.example.com',
      port: 1080,
    );

    final refreshed = refreshSavedHostSshProfile(
      current: current,
      host: host,
      address: 'new.example.com',
      port: 2202,
      username: 'new-user',
      password: 'new-password',
      proxy: proxy,
      environment: const {'UPDATED': '1'},
    );

    expect(refreshed.host, 'new.example.com');
    expect(refreshed.port, 2202);
    expect(refreshed.username, 'new-user');
    expect(refreshed.identityId, 9);
    expect(refreshed.label, 'Updated host');
    expect(refreshed.password, 'new-password');
    expect(refreshed.privateKey, isNull);
    expect(refreshed.proxy, same(proxy));
    expect(refreshed.shellPath, '/bin/zsh');
    expect(refreshed.environment, {'UPDATED': '1'});
    expect(refreshed.encoding, 'ISO-8859-1');
    expect(refreshed.knownHostsPath, '/tmp/known_hosts');
  });
}
