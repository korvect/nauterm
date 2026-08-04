import 'package:flutter/services.dart';

const List<String> fallbackMonospaceFontFamilies = [
  'Menlo',
  'Cascadia Mono',
  'JetBrains Mono',
  'Fira Code',
  'Source Code Pro',
  'Consolas',
  'DejaVu Sans Mono',
  'Noto Sans Mono',
  'Liberation Mono',
  'Ubuntu Mono',
  'Courier New',
];

const MethodChannel _systemFontsChannel = MethodChannel(
  'com.korvect.nauterm/system_fonts',
);

const Set<String> _knownTerminalFontFamilies = {
  'iosevka',
  'hack',
  'consolas',
  'menlo',
  'monaco',
  'inconsolata',
  'mononoki',
  'fantasque sans mono',
  'anonymous pro',
  'liberation mono',
  'dejavu sans mono',
  'droid sans mono',
  'ubuntu mono',
  'roboto mono',
  'source code pro',
  'fira code',
  'fira mono',
  'jetbrains mono',
  'cascadia code',
  'cascadia mono',
  'victor mono',
  'ibm plex mono',
  'sf mono',
  'operator mono',
  'input mono',
  'pragmata pro',
  'berkeley mono',
  'monaspace',
  'geist mono',
  'comic mono',
  'courier new',
  'lucida console',
  'pt mono',
  'overpass mono',
  'space mono',
  'go mono',
  'noto sans mono',
  'sarasa mono',
  'maple mono',
  'meslolgs nf',
  'lxgw wenkai mono',
};

const List<String> _terminalFontNameIndicators = [
  'mono',
  'monospace',
  'code',
  'terminal',
  'console',
];

bool isLikelyTerminalFontFamily(String familyName) {
  final family = familyName.trim().toLowerCase();
  if (family.isEmpty || family == 'monospace' || family == 'courier') {
    return false;
  }
  for (final knownFamily in _knownTerminalFontFamilies) {
    if (family == knownFamily || family.startsWith('$knownFamily ')) {
      return true;
    }
  }
  return _terminalFontNameIndicators.any(
    (indicator) =>
        family == indicator ||
        family.endsWith(' $indicator') ||
        family.endsWith('-$indicator') ||
        family.contains(' $indicator '),
  );
}

String? preferredMonospaceFontFamily(List<String> families) {
  if (families.isEmpty) return null;
  const preferredFamilies = [
    'Cascadia Mono',
    'Consolas',
    'JetBrains Mono',
    'Fira Code',
    'DejaVu Sans Mono',
    'Noto Sans Mono',
    'Ubuntu Mono',
    'Liberation Mono',
  ];
  for (final preferred in preferredFamilies) {
    for (final family in families) {
      if (family.toLowerCase() == preferred.toLowerCase()) {
        return family;
      }
    }
  }
  return families.first;
}

Future<List<String>> loadMonospaceFontFamilies() async {
  try {
    final values = await _systemFontsChannel.invokeListMethod<String>(
      'listMonospaceFamilies',
    );
    if (values == null || values.isEmpty) {
      return fallbackMonospaceFontFamilies;
    }
    return _normalizedFontFamilies(values.where(isLikelyTerminalFontFamily));
  } on MissingPluginException {
    return fallbackMonospaceFontFamilies;
  } on PlatformException {
    return fallbackMonospaceFontFamilies;
  }
}

Future<List<String>> loadSystemFontFamilies() async {
  try {
    final values = await _systemFontsChannel.invokeListMethod<String>(
      'listFontFamilies',
    );
    if (values == null || values.isEmpty) {
      return fallbackMonospaceFontFamilies;
    }
    return _normalizedFontFamilies(values);
  } on MissingPluginException {
    return fallbackMonospaceFontFamilies;
  } on PlatformException {
    return fallbackMonospaceFontFamilies;
  }
}

List<String> _normalizedFontFamilies(Iterable<String> values) {
  return {
    for (final value in values)
      if (value.trim().isNotEmpty) value.trim(),
  }.toList()..sort((left, right) {
    return left.toLowerCase().compareTo(right.toLowerCase());
  });
}
