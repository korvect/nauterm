import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  const plainKey = KeyEntry(id: 1, name: 'Plain key');
  const certificate = KeyEntry(
    id: 2,
    name: 'User certificate',
    certificate: 'ssh-ed25519-cert-v01@openssh.com',
  );

  test('Key picker includes keys and certificates', () {
    final options = filterSshCredentialKeysForTesting(const [
      plainKey,
      certificate,
    ], certificatesOnly: false);

    expect(options.map((entry) => entry.id), [1, 2]);
  });

  test('Certificate picker includes only certificate credentials', () {
    final options = filterSshCredentialKeysForTesting(const [
      plainKey,
      certificate,
    ], certificatesOnly: true);

    expect(options.map((entry) => entry.id), [2]);
    expect(sshCredentialUsesCertificateForTesting(certificate), isTrue);
    expect(sshCredentialUsesCertificateForTesting(plainKey), isFalse);
  });
}
