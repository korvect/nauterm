import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/third_party_licenses.dart';

void main() {
  testWidgets('loads bundled Rust and native component licenses', (
    WidgetTester tester,
  ) async {
    final packages = <String>{};
    await tester.runAsync(() async {
      await for (final entry in loadNautermBundledLicenses()) {
        packages.addAll(entry.packages);
      }
    });

    expect(packages, contains('Sparkle 2.9.4'));
    expect(packages, contains('Nerd Fonts Symbols Only 3.4.0'));
    expect(packages.any((package) => package.endsWith('(Rust)')), isTrue);
  });

  test('parses compact cargo-about output into Flutter license entries', () {
    final entries = parseRustLicenseEntries(r'''
      {
        "licenses": [
          {
            "id": "MIT",
            "name": "MIT License",
            "text": "MIT license body",
            "packages": [
              {"name": "alpha", "version": "1.2.3", "source": "registry+crates.io"},
              {"name": "beta", "version": "4.5.6", "source": "registry+crates.io"}
            ]
          },
          {
            "id": "Apache-2.0",
            "name": "Apache License 2.0",
            "text": "Apache license body",
            "packages": [
              {"name": "gamma", "version": "", "source": "git+https://example.test/gamma"}
            ]
          }
        ]
      }
    ''');

    expect(entries, hasLength(2));
    expect(
      entries.first.packages,
      containsAll(<String>['alpha 1.2.3 (Rust)', 'beta 4.5.6 (Rust)']),
    );
    expect(_licenseText(entries.first), 'MIT license body');
    expect(entries.last.packages, contains('gamma (Rust)'));
  });

  test('ignores empty license records and deduplicates packages', () {
    final entries = parseRustLicenseEntries(r'''
      {
        "licenses": [
          {
            "text": "same body",
            "packages": [
              {"name": "alpha", "version": "1.0.0", "source": "registry+crates.io"},
              {"name": "alpha", "version": "1.0.0", "source": "registry+crates.io"}
            ]
          },
          {
            "text": "",
            "packages": [{"name": "ignored", "version": "1.0.0", "source": "registry+crates.io"}]
          }
          ,
          {
            "text": "first-party body",
            "packages": [{"name": "nauterm-mosh", "version": "0.1.0", "source": null}]
          }
        ]
      }
    ''');

    expect(entries, hasLength(1));
    expect(entries.single.packages, <String>['alpha 1.0.0 (Rust)']);
  });
}

String _licenseText(LicenseEntry entry) {
  return entry.paragraphs.map((paragraph) => paragraph.text).join('\n');
}
