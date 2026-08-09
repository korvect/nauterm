import 'dart:async';
import 'dart:io';

import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/update/desktop_update.dart';

void main() {
  group('compareVersions', () {
    test('compares numeric components instead of lexical text', () {
      expect(compareVersions('1.10.0', '1.9.9'), greaterThan(0));
      expect(compareVersions('v2.0.0', '1.99.99'), greaterThan(0));
      expect(compareVersions('1.2', '1.2.0'), 0);
      expect(compareVersions('1.2.3+8', '1.2.3+2'), 0);
    });
  });

  test('parses GNU and binary SHA-256 manifest lines', () {
    const first =
        'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
    const second =
        'BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB';
    final values = parseSha256Sums(
      '$first  Nauterm-1.0.0-macos-arm64.dmg\n$second *Nauterm-1.0.0-windows-x86_64-setup.exe\n',
    );
    expect(values['Nauterm-1.0.0-macos-arm64.dmg'], first);
    expect(values['Nauterm-1.0.0-windows-x86_64-setup.exe'], second);
  });

  group('selectUpdateAsset', () {
    final assets = [
      _asset('Nauterm-1.0.0-windows-x86_64.zip'),
      _asset('Nauterm-1.0.0-windows-x86_64-setup.exe'),
      _asset('Nauterm-1.0.0-linux-x86_64.AppImage.tar.gz'),
      _asset('nauterm_1.2.0-1_amd64.deb'),
      _asset('nauterm-1.2.0-1.x86_64.rpm'),
    ];

    test('selects the Windows installer and never the zip', () {
      final match = selectUpdateAsset(
        assets,
        const DesktopUpdateTarget(
          platform: DesktopUpdatePlatform.windows,
          architecture: DesktopUpdateArchitecture.x86_64,
        ),
      );
      expect(match?.$1.name, 'Nauterm-1.0.0-windows-x86_64-setup.exe');
      expect(match?.$2, DesktopUpdateAssetKind.windowsInstaller);
    });

    test('selects the signed macOS app archive for Sparkle discovery', () {
      final match = selectUpdateAsset(
        [
          _asset('Nauterm-1.0.0-macos-arm64.dmg'),
          _asset('Nauterm-1.0.0-macos-arm64.app.zip'),
        ],
        const DesktopUpdateTarget(
          platform: DesktopUpdatePlatform.macos,
          architecture: DesktopUpdateArchitecture.arm64,
        ),
      );
      expect(match?.$1.name, 'Nauterm-1.0.0-macos-arm64.app.zip');
      expect(match?.$2, DesktopUpdateAssetKind.macosArchive);
    });

    test('honors Debian and RPM Linux package preferences', () {
      for (final (preference, expectedName) in [
        (DesktopUpdateAssetKind.deb, 'nauterm_1.2.0-1_amd64.deb'),
        (DesktopUpdateAssetKind.rpm, 'nauterm-1.2.0-1.x86_64.rpm'),
        (
          DesktopUpdateAssetKind.appImage,
          'Nauterm-1.0.0-linux-x86_64.AppImage.tar.gz',
        ),
      ]) {
        final match = selectUpdateAsset(
          assets,
          DesktopUpdateTarget(
            platform: DesktopUpdatePlatform.linux,
            architecture: DesktopUpdateArchitecture.x86_64,
            linuxPackagePreference: preference,
          ),
        );
        expect(match?.$1.name, expectedName);
        expect(match?.$2, preference);
      }
    });
  });

  test('update check requires a configured repository', () async {
    final service = DesktopUpdateService(
      client: MockClient((_) async => http.Response('{}', 500)),
      repository: '',
      target: const DesktopUpdateTarget(
        platform: DesktopUpdatePlatform.windows,
        architecture: DesktopUpdateArchitecture.x86_64,
      ),
    );

    await expectLater(
      service.check('1.0.0'),
      throwsA(
        isA<DesktopUpdateException>().having(
          (error) => error.message,
          'message',
          contains('NAUTERM_UPDATE_REPOSITORY'),
        ),
      ),
    );
    service.close();
  });

  test(
    'Linux package installation waits for the installer to finish',
    () async {
      final completed = Completer<ProcessResult>();
      String? executable;
      List<String>? arguments;
      final service = DesktopUpdateService(
        repository: 'korvect/nauterm',
        target: const DesktopUpdateTarget(
          platform: DesktopUpdatePlatform.linux,
          architecture: DesktopUpdateArchitecture.x86_64,
        ),
        processRunner: (value, values) {
          executable = value;
          arguments = values;
          return completed.future;
        },
      );

      var finished = false;
      final installation = service
          .launchInstaller(File('/tmp/nauterm.deb'), DesktopUpdateAssetKind.deb)
          .then((value) {
            finished = true;
            return value;
          });
      await Future<void>.delayed(Duration.zero);

      expect(finished, isFalse);
      expect(executable, 'pkexec');
      expect(arguments, ['dpkg', '-i', '/tmp/nauterm.deb']);

      completed.complete(ProcessResult(1, 0, '', ''));
      expect(
        await installation,
        DesktopUpdateInstallDisposition.restartRequired,
      );
      service.close();
    },
  );

  test('Linux package installation reports a failed installer', () async {
    final service = DesktopUpdateService(
      repository: 'korvect/nauterm',
      target: const DesktopUpdateTarget(
        platform: DesktopUpdatePlatform.linux,
        architecture: DesktopUpdateArchitecture.x86_64,
      ),
      processRunner: (_, _) async => ProcessResult(1, 1, '', 'failed'),
    );

    await expectLater(
      service.launchInstaller(
        File('/tmp/nauterm.rpm'),
        DesktopUpdateAssetKind.rpm,
      ),
      throwsA(
        isA<DesktopUpdateException>().having(
          (error) => error.message,
          'message',
          contains('exit code 1'),
        ),
      ),
    );
    service.close();
  });
}

({String name, Uri? uri, int size}) _asset(String name) =>
    (name: name, uri: Uri.parse('https://example.com/$name'), size: 100);
