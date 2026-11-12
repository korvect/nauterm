import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/settings/settings_panel.dart';

import 'support/connection_uri_defaults.dart';

const _masterKey = 'Correct-master-key1';
String _activeProviderId = 's3';

void main() {
  setUp(mockConnectionUriDefaults);

  setUp(() async {
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
    settingsRemoteSyncStatusLoaderOverride = (_) async => {
      'active_provider_id': _activeProviderId,
    };
    _cacheSettings();
  });

  tearDown(() {
    settingsRemoteSyncStatusLoaderOverride = null;
    settingsSyncRunnerOverride = null;
    clearGithubSyncSettingsCacheForTesting();
  });

  testWidgets(
    'failed first sync keeps the key across settings refresh and retry',
    (tester) async {
      final suppliedKeys = <String?>[];
      settingsSyncRunnerOverride = (providerId, _, masterKey, _, _) async {
        expect(providerId, 's3');
        suppliedKeys.add(masterKey);
        if (suppliedKeys.length == 1) {
          throw const NautermDataException('S3 connection failed for test.');
        }
        return {
          'etag': 'test-etag',
          'revision': suppliedKeys.length,
          'snapshot_id': 'test-snapshot',
          'imported_records': 0,
        };
      };
      await _openUnlockedSync(tester);

      await _tapSync(tester);
      expect(find.text('S3 connection failed for test.'), findsOneWidget);
      expect(suppliedKeys, [_masterKey]);

      // Saving provider settings updates this same cache. Re-entering the page
      // then reapplies the persisted settings, which still have no local DEK.
      _cacheSettings(filename: 'updated-sync.enc');
      await tester.tap(find.byKey(const ValueKey('settings-nav-general')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('settings-nav-sync')));
      await tester.pumpAndSettle();
      expect(find.text('Sync unlocked'), findsOneWidget);
      expect(
        find.byKey(const ValueKey('settings-sync-master-key')),
        findsNothing,
      );

      await _tapSync(tester);
      expect(suppliedKeys, [_masterKey, _masterKey]);
      expect(find.text('Sync ready'), findsOneWidget);

      await _tapSync(tester);
      expect(suppliedKeys, [_masterKey, _masterKey, null]);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'an uncompleted first sync does not keep the key in a new panel',
    (tester) async {
      settingsSyncRunnerOverride = (_, _, _, _, _) async {
        throw const NautermDataException('S3 connection failed for test.');
      };
      await _openUnlockedSync(tester);
      await _tapSync(tester);

      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pumpWidget(
        const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const ValueKey('settings-nav-sync')));
      await tester.pumpAndSettle();
      for (final key in [
        'settings-sync-master-key',
        'settings-sync-master-key-confirm',
      ]) {
        final input = find.descendant(
          of: find.byKey(ValueKey(key)),
          matching: find.byType(EditableText),
        );
        expect(tester.widget<EditableText>(input).controller.text, isEmpty);
      }
      expect(tester.takeException(), isNull);
    },
  );
  for (final backend in [
    (id: 'github_repository', card: 'settings-sync-provider-githubRepository'),
    (id: 'github_gist', card: 'settings-sync-provider-githubGist'),
    (id: 'cloud:test-provider', card: 'settings-cloud-provider-test-provider'),
  ]) {
    testWidgets(
      '${backend.id} keeps the key after failure and settings refresh',
      (tester) async {
        _cacheSettings(providerId: backend.id);
        final suppliedKeys = <String?>[];
        settingsSyncRunnerOverride = (providerId, _, masterKey, _, _) async {
          expect(providerId, backend.id);
          suppliedKeys.add(masterKey);
          throw const NautermDataException('Sync connection failed for test.');
        };
        await _openUnlockedSync(tester);
        await _tapSync(tester, cardKey: backend.card);
        expect(find.text('Sync connection failed for test.'), findsOneWidget);

        _cacheSettings(providerId: backend.id, filename: 'updated-sync.enc');
        await tester.tap(find.byKey(const ValueKey('settings-nav-general')));
        await tester.pumpAndSettle();
        await tester.tap(find.byKey(const ValueKey('settings-nav-sync')));
        await tester.pumpAndSettle();
        expect(find.text('Sync unlocked'), findsOneWidget);
        await _tapSync(tester, cardKey: backend.card);
        expect(suppliedKeys, [_masterKey, _masterKey]);
        expect(tester.takeException(), isNull);
      },
    );
  }
}

void _cacheSettings({
  String filename = 'nauterm-sync.enc',
  String providerId = 's3',
}) {
  _activeProviderId = providerId;
  cacheGithubSyncSettingsForTesting(NautermPaths.resolve().databasePath, {
    'hasLocalSyncKey': false,
    'hasToken': true,
    'hasGistToken': true,
    'hasS3Credentials': true,
    'config': {
      'repository_url': 'https://github.com/example/test-vault',
      'branch': 'main',
      'path': filename,
    },
    'gistConfig': {'gist_id': 'test-gist', 'filename': filename},
    's3Config': {
      'endpoint': 'https://s3.example.test',
      'region': 'auto',
      'bucket': 'test-bucket',
      'prefix': 'nauterm',
      'filename': filename,
    },
    'cloudProviders': [
      {
        'id': 'test-provider',
        'scheme': 'webdav',
        'vendor': 'webdav',
        'name': 'Test WebDAV',
        'has_credentials': true,
        'config': {
          'endpoint': 'https://webdav.example.test',
          'filename': filename,
        },
      },
    ],
    'syncPreferences': {'active_provider_id': providerId},
  });
}

Future<void> _openUnlockedSync(WidgetTester tester) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(1000, 820);
  addTearDown(() {
    tester.view.resetDevicePixelRatio();
    tester.view.resetPhysicalSize();
  });
  await tester.pumpWidget(
    const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const ValueKey('settings-nav-sync')));
  await tester.pumpAndSettle();
  for (final key in [
    'settings-sync-master-key',
    'settings-sync-master-key-confirm',
  ]) {
    await tester.enterText(
      find.descendant(
        of: find.byKey(ValueKey(key)),
        matching: find.byType(EditableText),
      ),
      _masterKey,
    );
  }
  await tester.tap(find.text('Continue'));
  await tester.pumpAndSettle();
}

Future<void> _tapSync(
  WidgetTester tester, {
  String cardKey = 'settings-sync-provider-s3Compatible',
}) async {
  final sync = find.descendant(
    of: find.byKey(ValueKey(cardKey)),
    matching: find.byTooltip('Sync'),
  );
  await tester.ensureVisible(sync);
  await tester.pumpAndSettle();
  await tester.tap(sync);
  await tester.pumpAndSettle();
}
