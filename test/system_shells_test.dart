import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/system_shells.dart';

void main() {
  test('uses a friendly display name for Git Bash', () {
    expect(shellDisplayName(r'C:\Program Files\Git\bin\bash.exe'), 'Git Bash');
    expect(
      shellDisplayName(r'C:\Tools\PortableGit\usr\bin\bash.exe'),
      'Git Bash',
    );
    expect(shellDisplayName(r'C:\cygwin64\bin\bash.exe'), 'bash.exe');
  });

  test('discovers a standard Git Bash installation when present', () {
    if (!Platform.isWindows) return;
    final environment = Platform.environment;
    final candidates = [
      if (environment['ProgramFiles'] case final root?)
        '$root\\Git\\bin\\bash.exe',
      if (environment['ProgramFiles(x86)'] case final root?)
        '$root\\Git\\bin\\bash.exe',
      if (environment['LOCALAPPDATA'] case final root?)
        '$root\\Programs\\Git\\bin\\bash.exe',
    ].where((path) => File(path).existsSync());

    final discovered = discoverSystemShells();
    for (final candidate in candidates) {
      expect(discovered, contains(candidate));
    }
  });
}
