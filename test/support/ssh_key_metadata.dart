import 'dart:convert';

// Metadata-only fixtures: key material is not needed to inspect the cipher name.
String openSshHeader(String cipher, [String algorithm = 'ssh-ed25519']) {
  List<int> string(List<int> bytes) => [
    (bytes.length >> 24) & 255,
    (bytes.length >> 16) & 255,
    (bytes.length >> 8) & 255,
    bytes.length & 255,
    ...bytes,
  ];
  final body = base64.encode([
    ...utf8.encode('openssh-key-v1\x00'),
    ...string(utf8.encode(cipher)),
    ...string(utf8.encode(cipher == 'none' ? 'none' : 'bcrypt')),
    ...string([]),
    0,
    0,
    0,
    1,
    ...string(utf8.encode(algorithm)),
    ...string([]),
  ]);
  return '-----BEGIN OPENSSH PRIVATE KEY-----\n$body\n-----END OPENSSH PRIVATE KEY-----';
}
