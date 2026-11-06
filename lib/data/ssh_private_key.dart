import 'dart:convert';

/// Reads encryption metadata without attempting to unlock the private key.
bool sshPrivateKeyRequiresPassphrase(String privateKey) {
  if (RegExp(
    r'^PuTTY-User-Key-File-[23]:',
    multiLine: true,
  ).hasMatch(privateKey)) {
    final encryption = RegExp(
      r'^Encryption:[ \t]*(\S+)[ \t]*\r?$',
      multiLine: true,
    ).firstMatch(privateKey)?.group(1);
    return encryption != null && encryption != 'none';
  }
  if (privateKey.contains('-----BEGIN ENCRYPTED PRIVATE KEY-----') ||
      (RegExp(
            r'^-----BEGIN (?:RSA|DSA|EC) PRIVATE KEY-----\r?$',
            multiLine: true,
          ).hasMatch(privateKey) &&
          RegExp(
            r'^Proc-Type:\s*4,ENCRYPTED\s*$',
            multiLine: true,
            caseSensitive: false,
          ).hasMatch(privateKey))) {
    return true;
  }
  const begin = '-----BEGIN OPENSSH PRIVATE KEY-----';
  const end = '-----END OPENSSH PRIVATE KEY-----';
  final start = privateKey.indexOf(begin);
  final finish = privateKey.indexOf(end);
  if (start < 0 || finish < start + begin.length) return false;
  try {
    final body = privateKey
        .substring(start + begin.length, finish)
        .replaceAll(RegExp(r'\s'), '');
    final bytes = base64.decode(body);
    const magic = 'openssh-key-v1\x00';
    if (bytes.length < magic.length + 4 ||
        String.fromCharCodes(bytes.take(magic.length)) != magic) {
      return false;
    }
    final length = bytes
        .skip(magic.length)
        .take(4)
        .fold<int>(0, (n, byte) => (n << 8) | byte);
    final offset = magic.length + 4;
    if (length == 0 || length > bytes.length - offset) return false;
    return utf8.decode(bytes.sublist(offset, offset + length)) != 'none';
  } on FormatException {
    return false;
  }
}
