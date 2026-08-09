import 'package:flutter/material.dart';

@immutable
class NautermPalette extends ThemeExtension<NautermPalette> {
  const NautermPalette({
    required this.background,
    required this.surface,
    required this.surfaceContainer,
    required this.text,
    required this.mutedText,
    required this.faintText,
    required this.outline,
    required this.softOutline,
    required this.primary,
    required this.secondary,
  });

  final Color background;
  final Color surface;
  final Color surfaceContainer;
  final Color text;
  final Color mutedText;
  final Color faintText;
  final Color outline;
  final Color softOutline;
  final Color primary;
  final Color secondary;

  static const light = NautermPalette(
    background: Color(0xffedf3f3),
    surface: Colors.white,
    surfaceContainer: Color(0xfff6f9f9),
    text: Color(0xff151927),
    mutedText: Color(0xff647980),
    faintText: Color(0xff93a5aa),
    outline: Color(0xffdbe6e8),
    softOutline: Color(0xffe5ecef),
    primary: Color(0xff168df2),
    secondary: Color(0xff35d394),
  );

  static const dark = NautermPalette(
    background: Color(0xff202020),
    surface: Color(0xff2c2c2c),
    surfaceContainer: Color(0xff343434),
    text: Color(0xfff2f2f2),
    mutedText: Color(0xffb8b8b8),
    faintText: Color(0xff858585),
    outline: Color(0xff464646),
    softOutline: Color(0xff3a3a3a),
    primary: Color(0xff0a84ff),
    secondary: Color(0xff30d158),
  );

  @override
  NautermPalette copyWith({
    Color? background,
    Color? surface,
    Color? surfaceContainer,
    Color? text,
    Color? mutedText,
    Color? faintText,
    Color? outline,
    Color? softOutline,
    Color? primary,
    Color? secondary,
  }) {
    return NautermPalette(
      background: background ?? this.background,
      surface: surface ?? this.surface,
      surfaceContainer: surfaceContainer ?? this.surfaceContainer,
      text: text ?? this.text,
      mutedText: mutedText ?? this.mutedText,
      faintText: faintText ?? this.faintText,
      outline: outline ?? this.outline,
      softOutline: softOutline ?? this.softOutline,
      primary: primary ?? this.primary,
      secondary: secondary ?? this.secondary,
    );
  }

  @override
  NautermPalette lerp(ThemeExtension<NautermPalette>? other, double t) {
    if (other is! NautermPalette) {
      return this;
    }
    return NautermPalette(
      background: Color.lerp(background, other.background, t)!,
      surface: Color.lerp(surface, other.surface, t)!,
      surfaceContainer: Color.lerp(
        surfaceContainer,
        other.surfaceContainer,
        t,
      )!,
      text: Color.lerp(text, other.text, t)!,
      mutedText: Color.lerp(mutedText, other.mutedText, t)!,
      faintText: Color.lerp(faintText, other.faintText, t)!,
      outline: Color.lerp(outline, other.outline, t)!,
      softOutline: Color.lerp(softOutline, other.softOutline, t)!,
      primary: Color.lerp(primary, other.primary, t)!,
      secondary: Color.lerp(secondary, other.secondary, t)!,
    );
  }
}

extension NautermThemePalette on BuildContext {
  NautermPalette get nautermPalette =>
      Theme.of(this).extension<NautermPalette>() ??
      (Theme.of(this).brightness == Brightness.dark
          ? NautermPalette.dark
          : NautermPalette.light);
}

ColorScheme _nautermColorScheme(Brightness brightness) => ColorScheme.fromSeed(
  seedColor: const Color(0xff168df2),
  brightness: brightness,
  surface: brightness == Brightness.dark
      ? NautermPalette.dark.background
      : NautermPalette.light.background,
);

ThemeData nautermTheme([Brightness brightness = Brightness.light]) {
  final colorScheme = _nautermColorScheme(brightness);
  final dark = brightness == Brightness.dark;
  final palette = dark ? NautermPalette.dark : NautermPalette.light;

  return ThemeData(
    brightness: brightness,
    colorScheme: colorScheme,
    scaffoldBackgroundColor: palette.background,
    canvasColor: palette.surface,
    cardColor: palette.surface,
    dividerColor: palette.outline,
    extensions: [palette],
    textSelectionTheme: TextSelectionThemeData(
      cursorColor: palette.primary,
      selectionColor: dark ? const Color(0x550a84ff) : const Color(0x3356b8ff),
      selectionHandleColor: palette.primary,
    ),
    scrollbarTheme: ScrollbarThemeData(
      thumbColor: WidgetStateProperty.resolveWith((states) {
        if (states.contains(WidgetState.hovered) ||
            states.contains(WidgetState.dragged)) {
          return dark ? const Color(0xff767676) : const Color(0xff8da2aa);
        }
        return dark ? const Color(0xff555555) : const Color(0xffb7c7cc);
      }),
      trackColor: const WidgetStatePropertyAll(Colors.transparent),
    ),
    iconButtonTheme: dark
        ? IconButtonThemeData(
            style: ButtonStyle(
              foregroundColor: WidgetStateProperty.resolveWith((states) {
                if (states.contains(WidgetState.disabled)) {
                  return palette.faintText;
                }
                return palette.text;
              }),
              overlayColor: WidgetStatePropertyAll(
                palette.primary.withValues(alpha: 0.14),
              ),
            ),
          )
        : null,
    textButtonTheme: dark
        ? TextButtonThemeData(
            style: ButtonStyle(
              foregroundColor: WidgetStateProperty.resolveWith((states) {
                if (states.contains(WidgetState.disabled)) {
                  return palette.faintText;
                }
                return palette.primary;
              }),
              overlayColor: WidgetStatePropertyAll(
                palette.primary.withValues(alpha: 0.14),
              ),
            ),
          )
        : null,
    outlinedButtonTheme: dark
        ? OutlinedButtonThemeData(
            style: ButtonStyle(
              foregroundColor: WidgetStateProperty.resolveWith((states) {
                if (states.contains(WidgetState.disabled)) {
                  return palette.faintText;
                }
                return palette.text;
              }),
              side: WidgetStateProperty.resolveWith((states) {
                final color = states.contains(WidgetState.disabled)
                    ? palette.softOutline
                    : palette.outline;
                return BorderSide(color: color);
              }),
              overlayColor: WidgetStatePropertyAll(
                palette.primary.withValues(alpha: 0.14),
              ),
            ),
          )
        : null,
    filledButtonTheme: dark
        ? FilledButtonThemeData(
            style: ButtonStyle(
              backgroundColor: WidgetStateProperty.resolveWith((states) {
                if (states.contains(WidgetState.disabled)) {
                  return palette.surfaceContainer;
                }
                return palette.primary;
              }),
              foregroundColor: WidgetStateProperty.resolveWith((states) {
                if (states.contains(WidgetState.disabled)) {
                  return palette.faintText;
                }
                return Colors.white;
              }),
              overlayColor: WidgetStatePropertyAll(
                Colors.white.withValues(alpha: 0.12),
              ),
            ),
          )
        : null,
    useMaterial3: true,
  );
}

ThemeData settingsTheme([Brightness brightness = Brightness.light]) =>
    nautermTheme(brightness);
