import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/terminal_theme_store.dart';
import 'package:nauterm/terminal/terminal_theme.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('uses Nysa Light as the default terminal theme', () {
    expect(defaultTerminalTheme.name, 'Nysa Light');
    expect(defaultTerminalTheme, same(nysaLightTerminalTheme));
    expect(defaultTerminalTheme.primary.background, const Color(0xfffbfbf8));
    expect(
      defaultTerminalTheme.normal.white,
      isNot(defaultTerminalTheme.primary.background),
    );
    expect(
      defaultTerminalTheme.bright.white,
      isNot(defaultTerminalTheme.primary.background),
    );
  });

  test('keeps built-in Nysa themes first without theme files', () async {
    final themes = await loadBundledTerminalThemes();

    expect(themes.take(2).map((entry) => entry.id), [
      nysaLightTerminalThemeId,
      nysaDarkTerminalThemeId,
    ]);
    expect(themes[0].theme.name, 'Nysa Light');
    expect(themes[1].theme.name, 'Nysa Dark');
    expect(
      await loadBundledTerminalTheme('default'),
      same(nysaLightTerminalTheme),
    );
    expect(
      await loadBundledTerminalTheme(nysaDarkTerminalThemeId),
      same(nysaDarkTerminalTheme),
    );
  });

  test('loads terminal themes from toml files using file name as id', () async {
    final directory = Directory.systemTemp.createTempSync(
      'nauterm_theme_test_',
    );
    addTearDown(() => directory.deleteSync(recursive: true));

    File('${directory.path}${Platform.pathSeparator}one-light.toml')
        .writeAsStringSync('''
[colors.primary]
background = "#fafafa"
foreground = "#383a42"

[colors.cursor]
cursor = "#526fff"
text = "#fafafa"

[colors.selection]
background = "#dbe9ff"
text = "#1f2329"

[colors.normal]
black = "#000000"
red = "#e45649"
green = "#50a14f"
yellow = "#c18401"
blue = "#4078f2"
magenta = "#a626a4"
cyan = "#0184bc"
white = "#a0a1a7"

[colors.bright]
black = "#696c77"
red = "#df6c75"
green = "#6aaf69"
yellow = "#e4c07b"
blue = "#61afef"
magenta = "#c678dd"
cyan = "#56b6c2"
white = "#ffffff"
''');

    final themes = await TerminalThemeStore(directory).loadThemes();

    expect(themes.single.id, 'one-light');
    expect(themes.single.theme.primary.background, const Color(0xfffafafa));
    expect(themes.single.theme.normal.red, const Color(0xffe45649));
    expect(themes.single.theme.bright.white, const Color(0xffffffff));
  });

  test('loads a single terminal theme by file name id', () async {
    final directory = Directory.systemTemp.createTempSync(
      'nauterm_theme_test_',
    );
    addTearDown(() => directory.deleteSync(recursive: true));

    File('${directory.path}${Platform.pathSeparator}atom-one-light.toml')
        .writeAsStringSync('''
name = "Atom One Light"
type = "light"

[colors.primary]
accent = "#4078f2"
background = "#fafafa"
foreground = "#383a42"
''');

    final store = TerminalThemeStore(directory);
    final ids = await store.listThemeIds();
    final theme = await store.loadTheme('atom-one-light');

    expect(ids, ['atom-one-light']);
    expect(theme?.name, 'Atom One Light');
    expect(theme?.type, TerminalThemeType.light);
    expect(theme?.primary.accent, const Color(0xff4078f2));
    expect(theme?.primary.background, const Color(0xfffafafa));
  });

  test(
    'additional terminal theme directory overrides the primary one',
    () async {
      final root = Directory.systemTemp.createTempSync(
        'nauterm_theme_catalog_test_',
      );
      addTearDown(() => root.deleteSync(recursive: true));
      final primary = Directory('${root.path}${Platform.pathSeparator}primary')
        ..createSync();
      final additional = Directory(
        '${root.path}${Platform.pathSeparator}additional',
      )..createSync();
      File('${primary.path}${Platform.pathSeparator}custom.toml')
          .writeAsStringSync('name = "Primary"');
      File('${additional.path}${Platform.pathSeparator}custom.toml')
          .writeAsStringSync('name = "Additional"');

      final catalog = TerminalThemeCatalog(
        primary,
        additionalDirectories: [additional],
      );

      expect(await catalog.listThemeIds(), contains('custom'));
      expect((await catalog.loadTheme('custom'))?.name, 'Additional');
      expect(
        (await catalog.loadThemes())
            .singleWhere((theme) => theme.id == 'custom')
            .theme
            .name,
        'Additional',
      );
    },
  );

  test('keeps missing terminal theme name blank', () {
    final theme = parseTerminalThemeToml('''
type = "light"

[colors.primary]
background = "#fafafa"
foreground = "#383a42"
''');

    expect(theme.name, isEmpty);
    expect(theme.type, TerminalThemeType.light);
  });

  test('normalizes stored host theme id to file name without extension', () {
    final host = HostEntry(
      name: 'server',
      themeId: '/tmp/themes/atom-one-dark.toml',
      type: NautermHostType.remote,
    );

    expect(host.toJson()['theme_id'], 'atom-one-dark');
    expect(
      HostEntry.fromJson({
        'name': 'server',
        'theme_id': 'ayu-light.toml',
        'type': 'remote',
      }).themeId,
      'ayu-light',
    );
  });
}
