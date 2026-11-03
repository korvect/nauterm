import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_paths.dart';

void main() {
  test('isolates configuration and data in the override directory', () {
    final directory = [
      Directory.systemTemp.path,
      'nauterm isolated profile',
    ].join(Platform.pathSeparator);
    final paths = NautermPaths.resolve(
      environment: _TestEnvironment({'NAUTERM_DATA_DIR': directory}),
    );

    expect(paths.configDirectory.path, directory);
    expect(paths.dataDirectory.path, directory);
    expect(paths.additionalThemeDirectories, isEmpty);
    for (final file in [
      paths.configFile,
      paths.databaseFile,
      paths.workspaceStateFile,
      paths.knownHostsFile,
      paths.shellHistoryFile,
    ]) {
      expect(file.parent.path, directory);
    }
    for (final child in [
      paths.themesDirectory,
      paths.terminalLogsDirectory,
      paths.runtimeLogsDirectory,
      paths.shellIntegrationDirectory,
    ]) {
      expect(child.parent.path, directory);
    }
  });

  test('an empty override keeps the platform defaults', () {
    final original = NautermPaths.resolve(
      environment: const _TestEnvironment({'HOME': '/home/tester'}),
    );
    final paths = NautermPaths.resolve(
      environment: const _TestEnvironment({
        'HOME': '/home/tester',
        'NAUTERM_DATA_DIR': '',
      }),
    );

    expect(paths.dataDirectory.path, original.dataDirectory.path);
    expect(paths.configDirectory.path, original.configDirectory.path);
  });

  test('uses a product name for user data directories', () {
    const environment = _TestEnvironment({
      'HOME': '/home/tester',
      'USERPROFILE': r'C:\Users\tester',
      'APPDATA': r'C:\Users\tester\AppData\Roaming',
      'XDG_CONFIG_HOME': '/home/tester/.config',
      'XDG_DATA_HOME': '/home/tester/.local/share',
    });

    final paths = NautermPaths.resolve(environment: environment);

    if (Platform.isMacOS) {
      expect(
        paths.dataDirectory.path,
        endsWith(
          [
            'Library',
            'Application Support',
            'Nauterm',
          ].join(Platform.pathSeparator),
        ),
      );
      expect(
        paths.additionalThemeDirectories.single.path,
        '/home/tester/.config/Nauterm/themes',
      );
    } else if (Platform.isWindows) {
      expect(
        paths.dataDirectory.path,
        endsWith('${Platform.pathSeparator}Nauterm'),
      );
    } else {
      expect(paths.configDirectory.path, '/home/tester/.config/nauterm');
      expect(paths.dataDirectory.path, '/home/tester/.local/share/nauterm');
    }

    expect(paths.databaseFile.path, endsWith('nauterm.sqlite'));
  });
}

class _TestEnvironment extends PlatformEnvironment {
  const _TestEnvironment(this.values);

  final Map<String, String> values;

  @override
  String? value(String key) => values[key];
}
