import 'dart:io';

import 'package:flutter/services.dart';

import '../terminal/terminal_theme.dart';

class StoredTerminalTheme {
  const StoredTerminalTheme({required this.id, required this.theme});

  final String id;
  final TerminalTheme theme;
}

const List<StoredTerminalTheme> builtInTerminalThemes = [
  StoredTerminalTheme(
    id: nysaLightTerminalThemeId,
    theme: nysaLightTerminalTheme,
  ),
  StoredTerminalTheme(
    id: nysaDarkTerminalThemeId,
    theme: nysaDarkTerminalTheme,
  ),
];

class TerminalThemeCatalog {
  const TerminalThemeCatalog(
    this.directory, {
    this.additionalDirectories = const [],
  });

  final Directory directory;
  final List<Directory> additionalDirectories;

  Iterable<Directory> get _directories => [directory, ...additionalDirectories];

  Future<List<String>> listThemeIds() async {
    final bundledIds = await listBundledTerminalThemeIds();
    final ids = <String>{...bundledIds};
    for (final directory in _directories) {
      ids.addAll(await TerminalThemeStore(directory).listThemeIds());
    }
    final sortedIds = ids.toList(growable: false);
    return sortedIds..sort(_compareTerminalThemeIds);
  }

  Future<TerminalTheme?> loadTheme(String? id) async {
    final normalizedId = normalizeTerminalThemeId(id);
    if (normalizedId == null) {
      return null;
    }

    final builtInTheme = _builtInTerminalTheme(normalizedId);
    if (builtInTheme != null) {
      return builtInTheme;
    }

    for (final directory in _directories.toList().reversed) {
      final userTheme = await TerminalThemeStore(directory)
          .loadTheme(normalizedId);
      if (userTheme != null) {
        return userTheme;
      }
    }
    return loadBundledTerminalTheme(normalizedId);
  }

  Future<List<StoredTerminalTheme>> loadThemes() async {
    final byId = <String, StoredTerminalTheme>{};

    for (final theme in await loadBundledTerminalThemes()) {
      byId[theme.id] = theme;
    }

    for (final directory in _directories) {
      for (final theme in await TerminalThemeStore(directory).loadThemes()) {
        if (_builtInTerminalTheme(theme.id) == null) {
          byId[theme.id] = theme;
        }
      }
    }

    final themes = byId.values.toList(growable: false);
    return themes..sort((a, b) => _compareTerminalThemeIds(a.id, b.id));
  }
}

class TerminalThemeStore {
  const TerminalThemeStore(this.directory);

  final Directory directory;

  Future<List<String>> listThemeIds() async {
    final files = await _listThemeFiles();
    return files.map((file) => themeIdFromPath(file.path)).toList();
  }

  Future<TerminalTheme?> loadTheme(String id) async {
    final normalizedId = normalizeTerminalThemeId(id);
    if (normalizedId == null) {
      return null;
    }

    final file = File(
      '${directory.path}${Platform.pathSeparator}$normalizedId.toml',
    );
    if (!await file.exists()) {
      return null;
    }

    return parseTerminalThemeToml(await file.readAsString());
  }

  Future<List<StoredTerminalTheme>> loadThemes() async {
    final files = await _listThemeFiles();
    final themes = <StoredTerminalTheme>[];
    for (final file in files) {
      final id = themeIdFromPath(file.path);
      final text = await file.readAsString();
      themes.add(
        StoredTerminalTheme(id: id, theme: parseTerminalThemeToml(text)),
      );
    }
    return themes;
  }

  Future<List<File>> _listThemeFiles() async {
    if (!await directory.exists()) {
      return const [];
    }

    final files = await directory
        .list()
        .where((entity) => entity is File && entity.path.endsWith('.toml'))
        .cast<File>()
        .toList();
    return files..sort((a, b) => a.path.compareTo(b.path));
  }
}

Future<List<String>> listBundledTerminalThemeIds() async {
  final paths = await _bundledTerminalThemePaths();
  final ids = <String>{
    for (final theme in builtInTerminalThemes) theme.id,
    ...paths.map(themeIdFromPath),
  }.toList(growable: false);
  return ids..sort(_compareTerminalThemeIds);
}

Future<TerminalTheme?> loadBundledTerminalTheme(String id) async {
  final normalizedId = normalizeTerminalThemeId(id);
  if (normalizedId == null) {
    return null;
  }

  final builtInTheme = _builtInTerminalTheme(normalizedId);
  if (builtInTheme != null) {
    return builtInTheme;
  }

  final paths = await _bundledTerminalThemePaths();
  for (final path in paths) {
    if (themeIdFromPath(path) == normalizedId) {
      return parseTerminalThemeToml(await rootBundle.loadString(path));
    }
  }
  return null;
}

Future<List<StoredTerminalTheme>> loadBundledTerminalThemes() async {
  final paths = await _bundledTerminalThemePaths();

  final themes = <StoredTerminalTheme>[...builtInTerminalThemes];
  for (final path in paths) {
    final id = themeIdFromPath(path);
    if (_builtInTerminalTheme(id) != null) {
      continue;
    }
    final text = await rootBundle.loadString(path);
    themes.add(
      StoredTerminalTheme(id: id, theme: parseTerminalThemeToml(text)),
    );
  }
  return themes..sort((a, b) => _compareTerminalThemeIds(a.id, b.id));
}

TerminalTheme? _builtInTerminalTheme(String id) {
  return switch (id) {
    'default' || nysaLightTerminalThemeId => nysaLightTerminalTheme,
    nysaDarkTerminalThemeId => nysaDarkTerminalTheme,
    _ => null,
  };
}

int _compareTerminalThemeIds(String left, String right) {
  final priorityComparison = _terminalThemePriority(left)
      .compareTo(_terminalThemePriority(right));
  return priorityComparison != 0 ? priorityComparison : left.compareTo(right);
}

int _terminalThemePriority(String id) {
  return switch (id) {
    nysaLightTerminalThemeId => 0,
    nysaDarkTerminalThemeId => 1,
    _ => 2,
  };
}

Future<List<String>> _bundledTerminalThemePaths() async {
  final manifest = await AssetManifest.loadFromAssetBundle(rootBundle);
  final paths =
      manifest
          .listAssets()
          .where((path) => path.startsWith('assets/themes/'))
          .where((path) => path.endsWith('.toml'))
          .toList()
        ..sort();
  return paths;
}

TerminalTheme parseTerminalThemeToml(String text) {
  final values = <String, String>{};
  var section = '';

  for (final rawLine in text.split(RegExp(r'\r?\n'))) {
    final line = _stripComment(rawLine).trim();
    if (line.isEmpty) {
      continue;
    }

    final sectionMatch = RegExp(r'^\[(.+)\]$').firstMatch(line);
    if (sectionMatch != null) {
      section = sectionMatch.group(1)!.trim();
      continue;
    }

    final separator = line.indexOf('=');
    if (separator == -1) {
      continue;
    }

    final key = line.substring(0, separator).trim();
    final value = line.substring(separator + 1).trim();
    values[section.isEmpty ? key : '$section.$key'] = _unquote(value);
  }

  Color color(String key, Color fallback) {
    final value = values[key];
    return value == null ? fallback : _parseHexColor(value, fallback);
  }

  return TerminalTheme(
    name: values['name'] ?? '',
    type: TerminalThemeType.fromString(values['type']),
    primary: TerminalPrimaryColors(
      accent: color(
        'colors.primary.accent',
        defaultTerminalTheme.primary.accent,
      ),
      background: color(
        'colors.primary.background',
        defaultTerminalTheme.primary.background,
      ),
      foreground: color(
        'colors.primary.foreground',
        defaultTerminalTheme.primary.foreground,
      ),
    ),
    cursor: TerminalCursorColors(
      cursor: color('colors.cursor.cursor', defaultTerminalTheme.cursor.cursor),
      text: color('colors.cursor.text', defaultTerminalTheme.cursor.text),
    ),
    selection: TerminalSelectionColors(
      background: color(
        'colors.selection.background',
        defaultTerminalTheme.selection.background,
      ),
      text: color('colors.selection.text', defaultTerminalTheme.selection.text),
    ),
    normal: TerminalAnsiColors(
      black: color('colors.normal.black', defaultTerminalTheme.normal.black),
      red: color('colors.normal.red', defaultTerminalTheme.normal.red),
      green: color('colors.normal.green', defaultTerminalTheme.normal.green),
      yellow: color('colors.normal.yellow', defaultTerminalTheme.normal.yellow),
      blue: color('colors.normal.blue', defaultTerminalTheme.normal.blue),
      magenta: color(
        'colors.normal.magenta',
        defaultTerminalTheme.normal.magenta,
      ),
      cyan: color('colors.normal.cyan', defaultTerminalTheme.normal.cyan),
      white: color('colors.normal.white', defaultTerminalTheme.normal.white),
    ),
    bright: TerminalAnsiColors(
      black: color('colors.bright.black', defaultTerminalTheme.bright.black),
      red: color('colors.bright.red', defaultTerminalTheme.bright.red),
      green: color('colors.bright.green', defaultTerminalTheme.bright.green),
      yellow: color('colors.bright.yellow', defaultTerminalTheme.bright.yellow),
      blue: color('colors.bright.blue', defaultTerminalTheme.bright.blue),
      magenta: color(
        'colors.bright.magenta',
        defaultTerminalTheme.bright.magenta,
      ),
      cyan: color('colors.bright.cyan', defaultTerminalTheme.bright.cyan),
      white: color('colors.bright.white', defaultTerminalTheme.bright.white),
    ),
  );
}

String themeIdFromPath(String path) {
  final name = path.split(RegExp(r'[/\\]')).last;
  return name.endsWith('.toml') ? name.substring(0, name.length - 5) : name;
}

String? normalizeTerminalThemeId(String? value) {
  final trimmed = value?.trim();
  if (trimmed == null || trimmed.isEmpty) {
    return null;
  }
  return themeIdFromPath(trimmed);
}

String _stripComment(String line) {
  var quoted = false;
  for (var i = 0; i < line.length; i++) {
    final char = line[i];
    if (char == '"') {
      quoted = !quoted;
    } else if (char == '#' && !quoted) {
      return line.substring(0, i);
    }
  }
  return line;
}

String _unquote(String value) {
  if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) {
    return value.substring(1, value.length - 1);
  }
  return value;
}

Color _parseHexColor(String value, Color fallback) {
  final hex = value.trim().replaceFirst('#', '');
  if (hex.length != 6 && hex.length != 8) {
    return fallback;
  }
  final parsed = int.tryParse(hex, radix: 16);
  if (parsed == null) {
    return fallback;
  }
  return Color(hex.length == 6 ? 0xff000000 | parsed : parsed);
}
