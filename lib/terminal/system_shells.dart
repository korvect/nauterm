import 'dart:io';

List<String> discoverSystemShells({String? current}) {
  final shells = <String>{};
  if (Platform.isWindows) {
    shells.addAll(_windowsShells());
  } else {
    final environmentShell = _nonEmpty(Platform.environment['SHELL']);
    if (environmentShell != null) shells.add(environmentShell);

    final shellsFile = File('/etc/shells');
    if (shellsFile.existsSync()) {
      for (final rawLine in shellsFile.readAsLinesSync()) {
        final line = rawLine.trim();
        if (line.startsWith('/') && File(line).existsSync()) shells.add(line);
      }
    }
  }
  final configured = _nonEmpty(current);
  if (configured != null) shells.add(configured);

  final values = shells.toList(growable: false)
    ..sort((a, b) {
      final defaultPath = systemDefaultShellPath();
      final aDefault = a == defaultPath;
      final bDefault = b == defaultPath;
      if (aDefault != bDefault) return aDefault ? -1 : 1;
      final nameOrder = shellDisplayName(a).compareTo(shellDisplayName(b));
      return nameOrder != 0 ? nameOrder : a.compareTo(b);
    });
  return values;
}

String? systemDefaultShellPath() {
  return _nonEmpty(
    Platform.isWindows
        ? Platform.environment['COMSPEC']
        : Platform.environment['SHELL'],
  );
}

String shellDisplayName(String path) {
  final normalized = path.replaceAll('\\', '/');
  final name = normalized.split('/').last;
  return switch (name.toLowerCase()) {
    'pwsh' || 'pwsh.exe' => 'PowerShell 7',
    'powershell' || 'powershell.exe' => 'Windows PowerShell',
    'cmd' || 'cmd.exe' => 'Command Prompt',
    'bash.exe' when _looksLikeGitBashPath(path) => 'Git Bash',
    _ => name,
  };
}

List<String> _windowsShells() {
  final shells = <String>{};
  final environment = Platform.environment;
  final systemRoot = _nonEmpty(environment['SystemRoot']);
  final programFiles = _nonEmpty(environment['ProgramFiles']);
  final programFilesX86 = _nonEmpty(environment['ProgramFiles(x86)']);
  final localAppData = _nonEmpty(environment['LOCALAPPDATA']);
  final candidates = <String>[
    if (programFiles != null) '$programFiles\\PowerShell\\7\\pwsh.exe',
    ..._executablesFromPath('pwsh.exe'),
    if (systemRoot != null)
      '$systemRoot\\System32\\WindowsPowerShell\\v1.0\\powershell.exe',
    ..._executablesFromPath('powershell.exe'),
    ?_nonEmpty(environment['COMSPEC']),
    if (systemRoot != null) '$systemRoot\\System32\\cmd.exe',
    ..._executablesFromPath('cmd.exe'),
    if (programFiles != null) '$programFiles\\Git\\bin\\bash.exe',
    if (programFilesX86 != null) '$programFilesX86\\Git\\bin\\bash.exe',
    if (localAppData != null) '$localAppData\\Programs\\Git\\bin\\bash.exe',
    ..._executablesFromPath('bash.exe').where(_looksLikeGitBashPath),
  ];
  for (final candidate in candidates) {
    if (File(candidate).existsSync()) shells.add(candidate);
  }
  return shells.toList(growable: false);
}

bool _looksLikeGitBashPath(String path) {
  final normalized = path.trim().toLowerCase().replaceAll('\\', '/');
  final segments = normalized.split('/');
  if (segments.length < 3 || segments.last != 'bash.exe') return false;
  final binIndex = segments.length - 2;
  if (segments[binIndex] != 'bin') return false;
  final rootIndex = segments[binIndex - 1] == 'usr'
      ? binIndex - 2
      : binIndex - 1;
  if (rootIndex < 0) return false;
  final root = segments[rootIndex];
  return root == 'git' || root.startsWith('portablegit');
}

List<String> _executablesFromPath(String executable) {
  final path = _nonEmpty(Platform.environment['PATH']);
  if (path == null) return const [];
  return [
    for (final directory in path.split(';'))
      if (directory.trim().isNotEmpty) '${directory.trim()}\\$executable',
  ];
}

String? _nonEmpty(String? value) {
  final trimmed = value?.trim();
  return trimmed == null || trimmed.isEmpty ? null : trimmed;
}
