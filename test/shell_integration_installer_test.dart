import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/terminal/shell_integration_installer.dart';

void main() {
  test('installs and removes guarded shell integration snippets', () async {
    final root = await Directory.systemTemp.createTemp(
      'nauterm-shell-installer-',
    );
    addTearDown(() => root.delete(recursive: true));
    final home = Directory('${root.path}/home')..createSync();
    final data = Directory('${root.path}/data')..createSync();
    final zshRc = File('${home.path}/.zshrc')
      ..writeAsStringSync('export A=1\n');
    final installer = ShellIntegrationInstaller(
      paths: NautermPaths(configDirectory: data, dataDirectory: data),
      homeDirectory: home,
    );

    final installed = await installer.install();
    expect(installed.installed, isTrue);
    expect(await zshRc.readAsString(), contains('export A=1'));
    expect(await zshRc.readAsString(), contains('NAUTERM_SESSION'));
    expect(await installer.install(), isA<ShellIntegrationInstallStatus>());
    expect(
      '# >>> Nauterm shell integration >>>'.allMatches(
        await zshRc.readAsString(),
      ),
      hasLength(1),
    );

    final removed = await installer.uninstall();
    expect(removed.anyInstalled, isFalse);
    expect(await zshRc.readAsString(), 'export A=1\n');
    expect(
      File('${home.path}/.config/fish/conf.d/nauterm.fish').existsSync(),
      isFalse,
    );
  });
}
