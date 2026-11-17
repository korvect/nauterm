import 'package:material_ui/material_ui.dart';

const String nysaLightTerminalThemeId = 'nysa-light';
const String nysaDarkTerminalThemeId = 'nysa-dark';

const Color terminalDefaultBackground = Color(0xfffbfbf8);
const Color terminalDefaultForeground = Color(0xff343842);
const Color terminalDefaultCursor = Color(0xff9ec6ee);

const TerminalTheme nysaLightTerminalTheme = TerminalTheme(
  name: 'Nysa Light',
  type: TerminalThemeType.light,
  primary: TerminalPrimaryColors(
    accent: Color(0xff3fa36f),
    background: terminalDefaultBackground,
    foreground: terminalDefaultForeground,
  ),
  cursor: TerminalCursorColors(
    cursor: terminalDefaultCursor,
    text: terminalDefaultForeground,
  ),
  selection: TerminalSelectionColors(
    background: Color(0xffd7eaf6),
    text: terminalDefaultForeground,
  ),
  normal: TerminalAnsiColors(
    black: Color(0xff343842),
    red: Color(0xffd95f56),
    green: Color(0xff43a46f),
    yellow: Color(0xffb88416),
    blue: Color(0xff1f73d8),
    magenta: Color(0xff9d52a8),
    cyan: Color(0xff1298aa),
    white: Color(0xff68707d),
  ),
  bright: TerminalAnsiColors(
    black: Color(0xff555b68),
    red: Color(0xffe46f67),
    green: Color(0xff63b987),
    yellow: Color(0xffd6a73d),
    blue: Color(0xff4b9cf0),
    magenta: Color(0xffb86ac6),
    cyan: Color(0xff47b7c6),
    white: Color(0xff858d99),
  ),
);

const TerminalTheme nysaDarkTerminalTheme = TerminalTheme(
  name: 'Nysa Dark',
  type: TerminalThemeType.dark,
  primary: TerminalPrimaryColors(
    accent: Color(0xff7fba89),
    background: Color(0xff24262a),
    foreground: Color(0xffbcc1c9),
  ),
  cursor: TerminalCursorColors(
    cursor: Color(0xff929dad),
    text: Color(0xff24262a),
  ),
  selection: TerminalSelectionColors(
    background: Color(0xff383d45),
    text: Color(0xffbcc1c9),
  ),
  normal: TerminalAnsiColors(
    black: Color(0xff24262a),
    red: Color(0xffdf7176),
    green: Color(0xff80ba8a),
    yellow: Color(0xffd7ad5f),
    blue: Color(0xff5798df),
    magenta: Color(0xffb772c8),
    cyan: Color(0xff50b0bf),
    white: Color(0xffbcc1c9),
  ),
  bright: TerminalAnsiColors(
    black: Color(0xff4b5058),
    red: Color(0xffe58489),
    green: Color(0xff92cb9b),
    yellow: Color(0xffe2c078),
    blue: Color(0xff70adeb),
    magenta: Color(0xffc985d8),
    cyan: Color(0xff6ac6d1),
    white: Color(0xffd2d6dd),
  ),
);

const TerminalTheme defaultTerminalTheme = nysaLightTerminalTheme;

@immutable
class TerminalTheme {
  const TerminalTheme({
    required this.name,
    required this.type,
    required this.primary,
    required this.cursor,
    required this.selection,
    required this.normal,
    required this.bright,
  });

  final String name;
  final TerminalThemeType type;
  final TerminalPrimaryColors primary;
  final TerminalCursorColors cursor;
  final TerminalSelectionColors selection;
  final TerminalAnsiColors normal;
  final TerminalAnsiColors bright;

  Map<String, Object?> toJson() {
    return <String, Object?>{
      'name': name,
      'type': type.storageValue,
      'background': primary.background.toHex(),
      'foreground': primary.foreground.toHex(),
      'accent': primary.accent.toHex(),
      'cursor': cursor.cursor.toHex(),
      'cursorText': cursor.text.toHex(),
      'selectionBackground': selection.background.toHex(),
      'selectionText': selection.text.toHex(),
      'normal': normal.toJson(),
      'bright': bright.toJson(),
    };
  }

  static TerminalTheme fromJson(Map<String, Object?> json) {
    return TerminalTheme(
      name: json['name'] as String? ?? 'Custom',
      type: TerminalThemeType.fromString(json['type'] as String?),
      primary: TerminalPrimaryColors(
        accent:
            _colorFromHex(json['accent']) ??
            defaultTerminalTheme.primary.accent,
        background:
            _colorFromHex(json['background']) ??
            defaultTerminalTheme.primary.background,
        foreground:
            _colorFromHex(json['foreground']) ??
            defaultTerminalTheme.primary.foreground,
      ),
      cursor: TerminalCursorColors(
        cursor:
            _colorFromHex(json['cursor']) ?? defaultTerminalTheme.cursor.cursor,
        text:
            _colorFromHex(json['cursorText']) ??
            defaultTerminalTheme.cursor.text,
      ),
      selection: TerminalSelectionColors(
        background:
            _colorFromHex(json['selectionBackground']) ??
            defaultTerminalTheme.selection.background,
        text:
            _colorFromHex(json['selectionText']) ??
            defaultTerminalTheme.selection.text,
      ),
      normal: TerminalAnsiColors.fromJson(
        json['normal'] as Map<String, Object?>?,
      ),
      bright: TerminalAnsiColors.fromJson(
        json['bright'] as Map<String, Object?>?,
      ),
    );
  }

  static Color? _colorFromHex(Object? value) {
    if (value is! String) return null;
    final hex = value.replaceFirst('#', '');
    if (hex.length == 6) {
      return Color(int.parse('ff$hex', radix: 16));
    }
    if (hex.length == 8) {
      return Color(int.parse(hex, radix: 16));
    }
    return null;
  }
}

enum TerminalThemeType {
  light('light'),
  dark('dark');

  const TerminalThemeType(this.storageValue);

  final String storageValue;

  static TerminalThemeType fromString(String? value) {
    return switch (value) {
      'dark' => TerminalThemeType.dark,
      'light' => TerminalThemeType.light,
      _ => defaultTerminalTheme.type,
    };
  }
}

@immutable
class TerminalPrimaryColors {
  const TerminalPrimaryColors({
    required this.accent,
    required this.background,
    required this.foreground,
  });

  final Color accent;
  final Color background;
  final Color foreground;
}

@immutable
class TerminalCursorColors {
  const TerminalCursorColors({required this.cursor, required this.text});

  final Color cursor;
  final Color text;
}

@immutable
class TerminalSelectionColors {
  const TerminalSelectionColors({required this.background, required this.text});

  final Color background;
  final Color text;
}

@immutable
class TerminalAnsiColors {
  const TerminalAnsiColors({
    required this.black,
    required this.red,
    required this.green,
    required this.yellow,
    required this.blue,
    required this.magenta,
    required this.cyan,
    required this.white,
  });

  final Color black;
  final Color red;
  final Color green;
  final Color yellow;
  final Color blue;
  final Color magenta;
  final Color cyan;
  final Color white;

  Map<String, Object?> toJson() {
    return <String, Object?>{
      'black': black.toHex(),
      'red': red.toHex(),
      'green': green.toHex(),
      'yellow': yellow.toHex(),
      'blue': blue.toHex(),
      'magenta': magenta.toHex(),
      'cyan': cyan.toHex(),
      'white': white.toHex(),
    };
  }

  static TerminalAnsiColors fromJson(Map<String, Object?>? json) {
    if (json == null) return defaultTerminalTheme.normal;
    Color c(String key, Color fallback) =>
        TerminalTheme._colorFromHex(json[key]) ?? fallback;
    final d = defaultTerminalTheme.normal;
    return TerminalAnsiColors(
      black: c('black', d.black),
      red: c('red', d.red),
      green: c('green', d.green),
      yellow: c('yellow', d.yellow),
      blue: c('blue', d.blue),
      magenta: c('magenta', d.magenta),
      cyan: c('cyan', d.cyan),
      white: c('white', d.white),
    );
  }

  Color byIndex(int index) {
    return switch (index) {
      0 => black,
      1 => red,
      2 => green,
      3 => yellow,
      4 => blue,
      5 => magenta,
      6 => cyan,
      7 => white,
      _ => black,
    };
  }

  TerminalAnsiColors copyWithIndex(int index, Color color) {
    return switch (index) {
      0 => TerminalAnsiColors(
        black: color,
        red: red,
        green: green,
        yellow: yellow,
        blue: blue,
        magenta: magenta,
        cyan: cyan,
        white: white,
      ),
      1 => TerminalAnsiColors(
        black: black,
        red: color,
        green: green,
        yellow: yellow,
        blue: blue,
        magenta: magenta,
        cyan: cyan,
        white: white,
      ),
      2 => TerminalAnsiColors(
        black: black,
        red: red,
        green: color,
        yellow: yellow,
        blue: blue,
        magenta: magenta,
        cyan: cyan,
        white: white,
      ),
      3 => TerminalAnsiColors(
        black: black,
        red: red,
        green: green,
        yellow: color,
        blue: blue,
        magenta: magenta,
        cyan: cyan,
        white: white,
      ),
      4 => TerminalAnsiColors(
        black: black,
        red: red,
        green: green,
        yellow: yellow,
        blue: color,
        magenta: magenta,
        cyan: cyan,
        white: white,
      ),
      5 => TerminalAnsiColors(
        black: black,
        red: red,
        green: green,
        yellow: yellow,
        blue: blue,
        magenta: color,
        cyan: cyan,
        white: white,
      ),
      6 => TerminalAnsiColors(
        black: black,
        red: red,
        green: green,
        yellow: yellow,
        blue: blue,
        magenta: magenta,
        cyan: color,
        white: white,
      ),
      7 => TerminalAnsiColors(
        black: black,
        red: red,
        green: green,
        yellow: yellow,
        blue: blue,
        magenta: magenta,
        cyan: cyan,
        white: color,
      ),
      _ => this,
    };
  }
}

extension ColorHex on Color {
  String toHex() {
    return '#${(toARGB32() & 0xffffff).toRadixString(16).padLeft(6, '0')}';
  }
}
