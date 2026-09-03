import 'dart:io';

const _applicationDirectoryName = 'Nauterm';
const _xdgApplicationDirectoryName = 'nauterm';

class NautermPaths {
  const NautermPaths({
    required this.configDirectory,
    required this.dataDirectory,
    this.additionalThemeDirectories = const [],
  });

  final Directory configDirectory;
  final Directory dataDirectory;
  final List<Directory> additionalThemeDirectories;

  File get configFile => File(_join(configDirectory.path, 'config.json'));

  File get workspaceStateFile =>
      File(_join(dataDirectory.path, 'workspace-state.json'));

  Directory get themesDirectory =>
      Directory(_join(configDirectory.path, 'themes'));

  File get knownHostsFile => File(_join(configDirectory.path, 'known_hosts'));

  File get databaseFile => File(_join(dataDirectory.path, 'nauterm.sqlite'));

  Directory get terminalLogsDirectory =>
      Directory(_join(dataDirectory.path, 'terminal-logs'));

  Directory get runtimeLogsDirectory =>
      Directory(_join(dataDirectory.path, 'logs'));

  File get shellHistoryFile => File(_join(dataDirectory.path, 'shell-history'));

  Directory get shellIntegrationDirectory =>
      Directory(_join(dataDirectory.path, 'shell-integration'));

  String get databasePath => databaseFile.path;

  Future<void> ensureCreated() async {
    await configDirectory.create(recursive: true);
    await dataDirectory.create(recursive: true);
    await themesDirectory.create(recursive: true);
    for (final directory in additionalThemeDirectories) {
      await directory.create(recursive: true);
    }
    await terminalLogsDirectory.create(recursive: true);
    await runtimeLogsDirectory.create(recursive: true);
  }

  static NautermPaths resolve({
    PlatformEnvironment environment = const PlatformEnvironment(),
  }) {
    if (Platform.isMacOS) {
      final base = _homeRelative(
        environment,
        _joinAll(['Library', 'Application Support', _applicationDirectoryName]),
      );
      return NautermPaths(
        configDirectory: Directory(base),
        dataDirectory: Directory(base),
        additionalThemeDirectories: [
          Directory(
            _homeRelative(
              environment,
              _joinAll(['.config', _applicationDirectoryName, 'themes']),
            ),
          ),
        ],
      );
    }

    if (Platform.isWindows) {
      final roaming =
          environment.value('APPDATA') ??
          _joinAll([
            environment.value('USERPROFILE') ?? '.',
            'AppData',
            'Roaming',
          ]);
      final base = _join(roaming, _applicationDirectoryName);
      return NautermPaths(
        configDirectory: Directory(base),
        dataDirectory: Directory(base),
      );
    }

    final configBase =
        environment.value('XDG_CONFIG_HOME') ??
        _homeRelative(environment, _joinAll(['.config']));
    final dataBase =
        environment.value('XDG_DATA_HOME') ??
        _homeRelative(environment, _joinAll(['.local', 'share']));

    return NautermPaths(
      configDirectory: Directory(
        _join(configBase, _xdgApplicationDirectoryName),
      ),
      dataDirectory: Directory(_join(dataBase, _xdgApplicationDirectoryName)),
    );
  }
}

class PlatformEnvironment {
  const PlatformEnvironment();

  String? value(String key) => Platform.environment[key];
}

String _homeRelative(PlatformEnvironment environment, String path) {
  final home =
      environment.value('HOME') ?? environment.value('USERPROFILE') ?? '.';
  return _join(home, path);
}

String _join(String left, String right) {
  if (left.endsWith(Platform.pathSeparator)) {
    return '$left$right';
  }
  return '$left${Platform.pathSeparator}$right';
}

String _joinAll(List<String> parts) => parts.join(Platform.pathSeparator);
