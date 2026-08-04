import 'dart:ui';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/terminal/terminal_config.dart';

void main() {
  test('configured CJK font precedes the platform fallback chain', () {
    const font = TerminalFontConfig(cjkFamily: 'Sarasa Mono SC');

    expect(font.resolvedFallback().first, 'Sarasa Mono SC');
    expect(font.textStyle().fontFamilyFallback?.first, 'Sarasa Mono SC');
  });

  test('application language is passed to the renderer as a locale', () {
    const font = TerminalFontConfig();

    expect(
      font.resolvedLocale(language: AppLanguage.simplifiedChinese),
      const Locale('zh', 'CN'),
    );
    expect(
      font.resolvedLocale(language: AppLanguage.english),
      const Locale('en'),
    );
    expect(font.resolvedFallback(), isNotEmpty);
    expect(font.textStyle().fontFamilyFallback, isNotEmpty);
  });

  test('the default family never leaves the terminal without a fallback', () {
    const font = TerminalFontConfig();

    // Even without a custom family or CJK font configured, a chain of
    // widely available monospaced fonts is always present so the terminal
    // never silently renders with a non-monospaced fallback font.
    expect(font.resolvedFallback(), isNotEmpty);
    expect(
      font.resolvedFallback().map((f) => f.toLowerCase()),
      isNot(contains(font.family.toLowerCase())),
    );
  });

  test('system language is used when the application follows the system', () {
    const font = TerminalFontConfig();

    expect(
      font.resolvedLocale(
        language: AppLanguage.system,
        systemLocale: const Locale('ja'),
      ),
      const Locale('ja'),
    );
  });

  test('text style carries the resolved locale to the font renderer', () {
    const font = TerminalFontConfig();
    final previousLanguage = appLanguage;
    addTearDown(() => setAppLanguage(previousLanguage));
    setAppLanguage(AppLanguage.simplifiedChinese);

    expect(font.textStyle().locale, const Locale('zh', 'CN'));
  });

  test('the generic monospace family resolves to a concrete platform font', () {
    const font = TerminalFontConfig();

    expect(
      font.resolvedFamily(windows: true, linux: false, macos: false),
      'Consolas',
    );
    expect(
      font.resolvedFamily(windows: false, linux: true, macos: false),
      'DejaVu Sans Mono',
    );
    expect(
      font.resolvedFamily(windows: false, linux: false, macos: true),
      'Menlo',
    );
  });

  test('a custom family is never overridden by the platform default', () {
    const font = TerminalFontConfig(family: 'JetBrains Mono');

    expect(
      font.resolvedFamily(windows: true, linux: false, macos: false),
      'JetBrains Mono',
    );
    expect(
      font.resolvedFamily(windows: false, linux: true, macos: false),
      'JetBrains Mono',
    );
  });
}
