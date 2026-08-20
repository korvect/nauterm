import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/host_import.dart';

void main() {
  group('host import parsers', () {
    test('parses host CSV including quoted tags', () {
      const input =
          '''Groups,Label,Tags,Hostname/IP,Protocol,Port,Username,Password
Project/Dev,Web Server,dev,192.168.1.1,ssh,2300,admin,password123
,Database,"db,mysql",db.example.com,ssh,4567,dbuser,
''';

      final hosts = parseHostCsv(input);

      expect(hosts, hasLength(2));
      expect(hosts.first.groupPath, 'Project/Dev');
      expect(hosts.first.port, 2300);
      expect(hosts.first.password, 'password123');
      expect(hosts.last.tags, ['db', 'mysql']);
    });

    test('parses concrete OpenSSH hosts and ignores wildcard blocks', () {
      const input = '''
User default-user
Host *
  ServerAliveInterval 30
  Port 2022
Host staging stage-alias
  HostName stage.example.com
  User deploy
  Port 2222
  IdentityFile ~/.ssh/id_ed25519
Host backup
  HostName backup.example.com
''';

      final hosts = parseOpenSshConfig(input);

      expect(hosts, hasLength(3));
      expect(hosts.first.name, 'staging');
      expect(hosts.first.host, 'stage.example.com');
      expect(hosts.first.username, 'deploy');
      expect(hosts.first.port, 2222);
      expect(hosts.first.identityFile, '~/.ssh/id_ed25519');
      expect(hosts.last.port, 2022);
    });

    test('parses resolvable known_hosts entries as host candidates', () {
      const input = '''
github.com,192.30.255.113 ssh-ed25519 AAAA
[git.example.com]:2222 ssh-rsa BBBB
|1|hashed|value ssh-ed25519 CCCC
@cert-authority *.example.com ssh-ed25519 DDDD
''';

      final hosts = parseOpenSshKnownHosts(input);

      expect(hosts, hasLength(3));
      expect(hosts[0].host, 'github.com');
      expect(hosts[1].host, '192.30.255.113');
      expect(hosts[2].host, 'git.example.com');
      expect(hosts[2].port, 2222);
    });

    test('pairs private and public keys from the ssh directory', () {
      final keys = collectOpenSshKeys(const [
        OpenSshImportFile(
          path: '/home/me/.ssh/id_ed25519.pub',
          contents: 'ssh-ed25519 AAAA me@example',
        ),
        OpenSshImportFile(
          path: '/home/me/.ssh/id_ed25519',
          contents: '-----BEGIN OPENSSH PRIVATE KEY-----\nsecret\n-----END OPENSSH PRIVATE KEY-----',
        ),
        OpenSshImportFile(
          path: '/home/me/.ssh/work.pub',
          contents: 'ssh-rsa BBBB work@example',
        ),
      ]);

      expect(keys, hasLength(2));
      expect(keys.first.name, 'id_ed25519');
      expect(keys.first.kind, HostImportKeyKind.keyPair);
      expect(keys.last.name, 'work.pub');
      expect(keys.last.kind, HostImportKeyKind.publicKey);
    });

    test('parses PuTTY registry exports', () {
      const input = r'''
Windows Registry Editor Version 5.00

[HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions\Production%20SSH]
"HostName"="prod.example.com"
"PortNumber"=dword:000008ae
"UserName"="root"
"PublicKeyFile"="C:\\Users\\me\\id.ppk"
''';

      final hosts = parsePuttyRegistry(input);

      expect(hosts, hasLength(1));
      expect(hosts.single.name, 'Production SSH');
      expect(hosts.single.host, 'prod.example.com');
      expect(hosts.single.port, 2222);
      expect(hosts.single.username, 'root');
    });

    test('parses MobaXterm SSH bookmark lines', () {
      const input = '''
[Bookmarks_1]
SubRep=Production
Prod=#109#0%prod.example.com%2200%deploy%%-1%-1%%%%%0%0%0
''';

      final hosts = parseMobaXtermSessions(input);

      expect(hosts, hasLength(1));
      expect(hosts.single.name, 'Prod');
      expect(hosts.single.host, 'prod.example.com');
      expect(hosts.single.port, 2200);
      expect(hosts.single.username, 'deploy');
      expect(hosts.single.groupPath, 'Production');
    });

    test('parses SecureCRT session files', () {
      const input = '''
S:"Hostname"=router.example.com
D:"[SSH2] Port"=00000016
S:"Username"=network
S:"Identity Filename V2"=/Users/me/.ssh/id_ed25519
''';

      final host = parseSecureCrtSession(input, name: 'Router');

      expect(host, isNotNull);
      expect(host!.name, 'Router');
      expect(host.host, 'router.example.com');
      expect(host.port, 22);
      expect(host.username, 'network');
    });
  });
}
