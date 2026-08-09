import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:nauterm/update/desktop_update.dart';
import 'package:nauterm/update/startup_update.dart';
import 'package:package_info_plus/package_info_plus.dart';

void main() {
  test('startup update check presents a newer release', () async {
    StartupUpdateNotice? notice;
    final coordinator = _coordinator(
      showNotice: (value) => notice = value,
      loadSkippedVersion: () async => null,
    );

    await coordinator.checkAtStartup();

    expect(notice?.version, '0.2.0');
    expect(notice?.phase, StartupUpdatePhase.available);
    coordinator.close();
  });

  test('startup update check does not present a skipped release', () async {
    StartupUpdateNotice? notice;
    final coordinator = _coordinator(
      showNotice: (value) => notice = value,
      loadSkippedVersion: () async => '0.2.0',
    );

    await coordinator.checkAtStartup();

    expect(notice, isNull);
    coordinator.close();
  });

  test(
    'skip persists the available version and dismisses the notice',
    () async {
      StartupUpdateNotice? notice;
      String? skippedVersion;
      final coordinator = _coordinator(
        showNotice: (value) => notice = value,
        loadSkippedVersion: () async => null,
        saveSkippedVersion: (version) async => skippedVersion = version,
      );
      await coordinator.checkAtStartup();

      await coordinator.skip();

      expect(skippedVersion, '0.2.0');
      expect(notice, isNull);
      coordinator.close();
    },
  );
}

StartupUpdateCoordinator _coordinator({
  required StartupUpdateNoticeSink showNotice,
  required SkippedUpdateVersionLoader loadSkippedVersion,
  SkippedUpdateVersionSaver? saveSkippedVersion,
}) {
  final service = DesktopUpdateService(
    repository: 'korvect/nauterm',
    target: const DesktopUpdateTarget(
      platform: DesktopUpdatePlatform.windows,
      architecture: DesktopUpdateArchitecture.x86_64,
    ),
    client: MockClient((_) async {
      return http.Response(
        jsonEncode({
          'tag_name': '0.2.0',
          'name': 'Nauterm 0.2.0',
          'body': 'Release notes',
          'html_url': 'https://example.com/releases/0.2.0',
          'assets': [
            {
              'name': 'Nauterm-0.2.0-windows-x86_64-setup.exe',
              'browser_download_url': 'https://example.com/nauterm.exe',
              'size': 100,
            },
            {
              'name': 'SHA256SUMS.txt',
              'browser_download_url': 'https://example.com/SHA256SUMS.txt',
              'size': 100,
            },
          ],
        }),
        200,
      );
    }),
  );
  return StartupUpdateCoordinator(
    showNotice: showNotice,
    loadSkippedVersion: loadSkippedVersion,
    saveSkippedVersion: saveSkippedVersion ?? (_) async {},
    restart: (_) async {},
    service: service,
    loadPackageInfo: () async => PackageInfo(
      appName: 'Nauterm',
      packageName: 'com.korvect.nauterm',
      version: '0.1.0',
      buildNumber: '1',
    ),
  );
}
