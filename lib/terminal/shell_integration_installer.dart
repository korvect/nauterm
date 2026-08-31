import 'dart:io';

import '../data/nauterm_paths.dart';

const _blockStart = '# >>> Nauterm shell integration >>>';
const _blockEnd = '# <<< Nauterm shell integration <<<';
const _fishManagedHeader = '# Managed by Nauterm. Use Settings to uninstall.';

class ShellIntegrationInstallStatus {
  const ShellIntegrationInstallStatus({
    required this.zsh,
    required this.bash,
    required this.fish,
  });

  final bool zsh;
  final bool bash;
  final bool fish;

  bool get installed => zsh && bash && fish;
  bool get anyInstalled => zsh || bash || fish;
}

class ShellIntegrationInstaller {
  ShellIntegrationInstaller({NautermPaths? paths, Directory? homeDirectory})
    : paths = paths ?? NautermPaths.resolve(),
      homeDirectory =
          homeDirectory ??
          Directory(
            Platform.environment['HOME'] ??
                Platform.environment['USERPROFILE'] ??
                '.',
          );

  final NautermPaths paths;
  final Directory homeDirectory;

  File get _zshRc => File(_join(homeDirectory.path, '.zshrc'));
  File get _bashRc => File(_join(homeDirectory.path, '.bashrc'));
  File get _fishRc => File(
    _joinAll([homeDirectory.path, '.config', 'fish', 'conf.d', 'nauterm.fish']),
  );

  Future<ShellIntegrationInstallStatus> status() async {
    if (Platform.isWindows) {
      return const ShellIntegrationInstallStatus(
        zsh: false,
        bash: false,
        fish: false,
      );
    }
    final states = await Future.wait([
      _containsManagedBlock(_zshRc),
      _containsManagedBlock(_bashRc),
      _isManagedFishFile(),
    ]);
    return ShellIntegrationInstallStatus(
      zsh: states[0],
      bash: states[1],
      fish: states[2],
    );
  }

  Future<ShellIntegrationInstallStatus> install() async {
    if (Platform.isWindows) {
      throw UnsupportedError(
        'Persistent shell integration is only available on Unix platforms.',
      );
    }
    await _installBlock(
      _zshRc,
      _sourceBlock(paths.shellIntegrationDirectory, 'nauterm.zsh'),
    );
    await _installBlock(
      _bashRc,
      _sourceBlock(paths.shellIntegrationDirectory, 'nauterm.bash'),
    );
    await _fishRc.parent.create(recursive: true);
    await _fishRc.writeAsString(
      '$_fishManagedHeader\n'
      'if set -q NAUTERM_SESSION\n'
      '    source "${_fishQuote(_join(paths.shellIntegrationDirectory.path, 'nauterm.fish'))}"\n'
      'end\n',
      flush: true,
    );
    return status();
  }

  Future<ShellIntegrationInstallStatus> uninstall() async {
    if (Platform.isWindows) return status();
    await _removeManagedBlock(_zshRc);
    await _removeManagedBlock(_bashRc);
    if (await _isManagedFishFile()) {
      await _fishRc.delete();
    }
    return status();
  }

  Future<bool> _containsManagedBlock(File file) async {
    if (!await file.exists()) return false;
    final contents = await file.readAsString();
    return contents.contains(_blockStart) && contents.contains(_blockEnd);
  }

  Future<bool> _isManagedFishFile() async {
    if (!await _fishRc.exists()) return false;
    return (await _fishRc.readAsString()).startsWith(_fishManagedHeader);
  }

  Future<void> _installBlock(File file, String block) async {
    await file.parent.create(recursive: true);
    final original = await file.exists() ? await file.readAsString() : '';
    final withoutExisting = _withoutManagedBlock(original).trimRight();
    final next = withoutExisting.isEmpty
        ? '$block\n'
        : '$withoutExisting\n\n$block\n';
    await file.writeAsString(next, flush: true);
  }

  Future<void> _removeManagedBlock(File file) async {
    if (!await file.exists()) return;
    final original = await file.readAsString();
    final next = _withoutManagedBlock(original).trimRight();
    await file.writeAsString(next.isEmpty ? '' : '$next\n', flush: true);
  }
}

String _sourceBlock(Directory resources, String filename) {
  final path = _shellQuote(_join(resources.path, filename));
  return '$_blockStart\n'
      'if [ -n "\${NAUTERM_SESSION:-}" ] && '
      '[ -z "\${NAUTERM_SHELL_INTEGRATION_INJECT+x}" ]; then\n'
      "  . '$path'\n"
      'fi\n'
      '$_blockEnd';
}

String _withoutManagedBlock(String contents) {
  final start = contents.indexOf(_blockStart);
  if (start < 0) return contents;
  final endMarker = contents.indexOf(_blockEnd, start + _blockStart.length);
  if (endMarker < 0) return contents;
  var end = endMarker + _blockEnd.length;
  if (end < contents.length && contents.codeUnitAt(end) == 13) end++;
  if (end < contents.length && contents.codeUnitAt(end) == 10) end++;
  return '${contents.substring(0, start)}${contents.substring(end)}';
}

String _shellQuote(String value) => value.replaceAll("'", "'\\''");
String _fishQuote(String value) => value
    .replaceAll('\\', '\\\\')
    .replaceAll('"', '\\"')
    .replaceAll(r'$', r'\$');

String _join(String left, String right) => left.endsWith(Platform.pathSeparator)
    ? '$left$right'
    : '$left${Platform.pathSeparator}$right';

String _joinAll(List<String> parts) => parts.join(Platform.pathSeparator);
