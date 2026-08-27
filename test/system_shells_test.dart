import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/system_shells.dart';

void main() {
  test('distinguishes PowerShell from Windows PowerShell', () {
    expect(
      shellDisplayName(r'C:\Program Files\PowerShell\7\pwsh.exe'),
      'PowerShell',
    );
    expect(
      shellDisplayName(
        r'C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe',
      ),
      'Windows PowerShell',
    );
  });

  test('uses a friendly display name for Git Bash', () {
    expect(shellDisplayName(r'C:\Program Files\Git\bin\bash.exe'), 'Git Bash');
    expect(
      shellDisplayName(r'C:\Tools\PortableGit\usr\bin\bash.exe'),
      'Git Bash',
    );
    expect(shellDisplayName(r'C:\cygwin64\bin\bash.exe'), 'bash.exe');
  });

  test('normalizes Windows shell executable titles', () {
    expect(
      shellDisplayNameFromExecutableTitle(
        r'C:\Program Files\WindowsApps\Microsoft.PowerShell_7.6.5.0_x64__8wekyb3d8bbwe\pwsh.exe',
        isWindows: true,
      ),
      'PowerShell',
    );
    expect(
      shellDisplayNameFromExecutableTitle(
        r'C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe',
        isWindows: true,
      ),
      'Windows PowerShell',
    );
    expect(
      shellDisplayNameFromExecutableTitle(
        r'C:\Windows\System32\cmd.exe',
        isWindows: true,
      ),
      'Command Prompt',
    );
    expect(
      shellDisplayNameFromExecutableTitle(r'C:\Users\valurno', isWindows: true),
      isNull,
    );
    expect(
      shellDisplayNameFromExecutableTitle('/usr/bin/pwsh', isWindows: false),
      isNull,
    );
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

  test('discovers a Windows PowerShell app execution alias', () {
    if (!Platform.isWindows) return;
    final localAppData = Platform.environment['LOCALAPPDATA'];
    if (localAppData == null) return;

    final alias = '$localAppData\\Microsoft\\WindowsApps\\pwsh.exe';
    final type = FileSystemEntity.typeSync(alias, followLinks: false);
    if (type != FileSystemEntityType.link) return;

    expect(isSystemShellAvailable(alias), isTrue);
    expect(discoverSystemShells(), contains(alias));
  });
}
