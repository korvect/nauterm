import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/ssh_private_key.dart';

import 'support/ssh_key_metadata.dart';

void main() {
  for (final algorithm in [
    'ssh-ed25519',
    'ecdsa-sha2-nistp256',
    'ecdsa-sha2-nistp384',
    'ecdsa-sha2-nistp521',
    'ssh-rsa',
    'ssh-dss',
    'sk-ssh-ed25519@openssh.com',
    'sk-ecdsa-sha2-nistp256@openssh.com',
  ]) {
    for (final cipher in [
      'none',
      'aes256-ctr',
      'aes256-cbc',
      'chacha20-poly1305@openssh.com',
    ]) {
      test('OpenSSH $algorithm reads the $cipher encryption header', () {
        expect(
          sshPrivateKeyRequiresPassphrase(openSshHeader(cipher, algorithm)),
          cipher != 'none',
        );
      });
    }
  }
  for (final type in ['RSA', 'DSA', 'EC']) {
    test('legacy PEM $type detects encryption with CRLF lines', () {
      final pem =
          '-----BEGIN $type PRIVATE KEY-----\r\n'
          'Proc-Type: 4,ENCRYPTED\r\nDEK-Info: AES-256-CBC,0123456789\r\n'
          '\r\ndGVzdA==\r\n-----END $type PRIVATE KEY-----';
      expect(sshPrivateKeyRequiresPassphrase(pem), isTrue);
      expect(
        sshPrivateKeyRequiresPassphrase(
          '-----BEGIN $type PRIVATE KEY-----\ndGVzdA==\n-----END $type PRIVATE KEY-----',
        ),
        isFalse,
      );
    });
  }
  test('PKCS8 distinguishes encrypted and unencrypted containers', () {
    expect(
      sshPrivateKeyRequiresPassphrase(
        '-----BEGIN ENCRYPTED PRIVATE KEY-----\ndGVzdA==\n-----END ENCRYPTED PRIVATE KEY-----',
      ),
      isTrue,
    );
    expect(
      sshPrivateKeyRequiresPassphrase(
        '-----BEGIN PRIVATE KEY-----\ndGVzdA==\n-----END PRIVATE KEY-----',
      ),
      isFalse,
    );
  });
  for (final version in [2, 3]) {
    test(
      'PuTTY PPK v$version reads Encryption independently of the key algorithm',
      () {
        for (final type in ['ssh-rsa', 'ssh-ed25519', 'ecdsa-sha2-nistp256']) {
          for (final encryption in ['none', 'aes256-cbc']) {
            final ppk =
                'PuTTY-User-Key-File-$version: $type\r\nEncryption: $encryption\r\nComment: Test\r\n';
            expect(sshPrivateKeyRequiresPassphrase(ppk), encryption != 'none');
          }
        }
      },
    );
  }
  test('public keys, certificates, and malformed data never imply a passphrase', () {
    for (final key in [
      '',
      'ssh-ed25519 dGVzdA==',
      'ssh-ed25519-cert-v01@openssh.com dGVzdA==',
      '-----BEGIN OPENSSH PRIVATE KEY-----\ninvalid!\n-----END OPENSSH PRIVATE KEY-----',
      '-----BEGIN OPENSSH PRIVATE KEY-----\n${base64.encode(utf8.encode('openssh-key-v1\x00'))}\n-----END OPENSSH PRIVATE KEY-----',
      '-----BEGIN OPENSSH PRIVATE KEY-----\n${base64.encode([...utf8.encode('openssh-key-v1\x00'), 255, 255, 255, 255])}\n-----END OPENSSH PRIVATE KEY-----',
      'PuTTY-User-Key-File-3: ssh-rsa\nComment: Unknown encryption',
    ]) {
      expect(sshPrivateKeyRequiresPassphrase(key), isFalse);
    }
  });
}
