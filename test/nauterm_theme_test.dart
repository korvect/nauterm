import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_theme.dart';

void main() {
  test('dark palette uses neutral macOS-inspired surface layers', () {
    expect(NautermPalette.dark.background, const Color(0xff202020));
    expect(NautermPalette.dark.surface, const Color(0xff2c2c2c));
    expect(NautermPalette.dark.surfaceContainer, const Color(0xff343434));
    expect(NautermPalette.dark.outline, const Color(0xff464646));
    expect(NautermPalette.dark.primary, const Color(0xff0a84ff));
    expect(NautermPalette.dark.secondary, const Color(0xff30d158));
  });

  test('dark palette keeps readable foreground contrast', () {
    expect(
      _contrastRatio(NautermPalette.dark.text, NautermPalette.dark.background),
      greaterThanOrEqualTo(7),
    );
    expect(
      _contrastRatio(
        NautermPalette.dark.mutedText,
        NautermPalette.dark.surface,
      ),
      greaterThanOrEqualTo(4.5),
    );
  });
}

double _contrastRatio(Color foreground, Color background) {
  final lighter = foreground.computeLuminance();
  final darker = background.computeLuminance();
  final high = lighter > darker ? lighter : darker;
  final low = lighter > darker ? darker : lighter;
  return (high + 0.05) / (low + 0.05);
}
