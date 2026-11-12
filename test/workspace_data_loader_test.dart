import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ai/ai_config.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/data/ai_provider_store.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/data/workspace_data_loader.dart';
import 'package:nauterm/terminal/terminal_recording_config.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  _PresentedTestBinding();

  test(
    'workspace worker returns decrypted records, files and retention results',
    () async {
      final directory = Directory.systemTemp.createTempSync(
        'nauterm-workspace-load-',
      );
      addTearDown(() => directory.deleteSync(recursive: true));
      final paths = NautermPaths(
        configDirectory: directory,
        dataDirectory: directory,
      );
      final store = NautermDataStore.openPath(paths.databasePath);
      store.initEncryption();
      final groupId = store.saveGroup(const HostGroup(name: 'Production'));
      store.saveHost(
        HostEntry(
          name: 'Remote host',
          type: NautermHostType.remote,
          host: 'example.test',
          password: 'stored-password',
          groupId: groupId,
          config: const {
            'icon': 'cisco',
            'ssh': {'compatibility_mode': true},
          },
        ),
      );
      store.saveHost(
        const HostEntry(
          name: 'Serial host',
          type: NautermHostType.serial,
          serialPort: '/dev/cu.test-console',
          serialBaudRate: 115200,
        ),
      );
      store.saveKey(
        const KeyEntry(
          name: 'Saved key',
          privateKey: 'private key',
          passphrase: 'key passphrase',
        ),
      );
      store.saveAiProvider(
        const AiProviderEntry(
          name: 'Provider',
          protocol: 'openai',
          baseUrl: 'https://example.test',
          model: 'model',
          apiKey: 'provider-secret',
          config: {},
          active: true,
        ),
      );
      final old = DateTime.now().toUtc().subtract(const Duration(days: 10));
      store.saveTerminalLog(
        TerminalLogEntry(
          id: '01979f62-8548-7000-8000-000000000401',
          title: 'Expired log',
          captureFile: '',
          startedAt: old,
          endedAt: old,
        ),
      );
      store.dispose();
      await paths.knownHostsFile.writeAsString('example.test ssh-ed25519 test');
      await paths.shellHistoryFile.writeAsString('1700000000000;pwd\n');

      final data = await loadNautermWorkspaceData(
        paths,
        const NautermRecordingConfig(retentionDays: 1),
      );
      expect(data.groups.single.name, 'Production');
      expect(data.hosts, hasLength(2));
      final remote = data.hosts.singleWhere(
        (host) => host.type == NautermHostType.remote,
      );
      expect(remote.password, isNull);
      expect(remote.config['icon'], 'cisco');
      expect(data.keys.single.name, 'Saved key');
      expect(data.keys.single.privateKey, isNull);
      expect(data.keys.single.passphrase, isNull);
      expect(data.aiProviders.single.apiKey, 'provider-secret');
      expect(data.knownHostsText, contains('ssh-ed25519'));
      expect(data.shellHistory.single.command, 'pwd');
      expect(data.deletedLogIds, {'01979f62-8548-7000-8000-000000000401'});
      expect(data.terminalLogs, isEmpty);

      final reopened = NautermDataStore.openPath(paths.databasePath);
      addTearDown(reopened.dispose);
      expect(reopened.listHosts(), hasLength(2));
      expect(reopened.getHost(remote.id!)!.password, 'stored-password');
      expect(reopened.getKey(data.keys.single.id!)!.privateKey, 'private key');
      expect(
        reopened.getKey(data.keys.single.id!)!.passphrase,
        'key passphrase',
      );
      expect(reopened.listTerminalLogs(), isEmpty);
    },
  );

  test(
    'provider catalog loads asynchronously and seeds the UI cache',
    () async {
      final directory = Directory.systemTemp.createTempSync(
        'nauterm-provider-load-',
      );
      addTearDown(() => directory.deleteSync(recursive: true));
      final paths = NautermPaths(
        configDirectory: directory,
        dataDirectory: directory,
      );
      final store = NautermDataStore.openPath(paths.databasePath);
      store.initEncryption();
      store.saveAiProvider(
        const AiProviderEntry(
          name: 'Background provider',
          protocol: 'openai',
          baseUrl: 'https://example.test',
          model: 'model',
          apiKey: 'secret',
          config: {},
          active: true,
        ),
      );
      store.dispose();
      final providers = AiProviderStore(paths);
      final catalog = await providers.loadCatalogAsync(
        const AiAssistantConfig(),
      );
      expect(catalog.active.config.apiKey, 'secret');
      await paths.databaseFile.rename('${paths.databasePath}.backup');
      expect(providers.listProviders().single.name, 'Background provider');
      expect(
        (await providers.loadCatalogAsync(const AiAssistantConfig()))
            .active
            .config
            .apiKey,
        'secret',
      );
      expect(await paths.databaseFile.exists(), isFalse);
    },
  );

  testWidgets(
    'Groups and Hosts animate until the background data is available',
    (tester) async {
      final result = Completer<NautermWorkspaceData>();
      var loading = false;
      final controller = NautermWorkspaceController(
        dataLoader: (_, _) {
          loading = true;
          return result.future;
        },
      );
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        NautermApp(onOpenSettings: () {}, workspaceController: controller),
      );
      await tester.pump(const Duration(milliseconds: 150));
      for (var attempt = 0; attempt < 50 && !loading; attempt++) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 10)),
        );
        await tester.pump(const Duration(milliseconds: 20));
      }
      expect(loading, isTrue);
      final indicators = find.byType(CircularProgressIndicator);
      expect(indicators, findsNWidgets(2));
      CustomPainter painterAt(int index) => tester
          .widget<CustomPaint>(
            find.descendant(
              of: indicators.at(index),
              matching: find.byType(CustomPaint),
            ),
          )
          .painter!;
      final groupsBefore = painterAt(0);
      final hostsBefore = painterAt(1);
      await tester.pump(const Duration(milliseconds: 200));
      expect(painterAt(0).shouldRepaint(groupsBefore), isTrue);
      expect(painterAt(1).shouldRepaint(hostsBefore), isTrue);
      expect(result.isCompleted, isFalse);

      result.complete(
        const NautermWorkspaceData(
          groups: [HostGroup(id: 1, name: 'Loaded group')],
          hosts: [
            HostEntry(
              id: 2,
              name: 'Loaded host',
              type: NautermHostType.remote,
              host: 'example.test',
            ),
          ],
        ),
      );
      await tester.pump();
      expect(find.text('Loaded group'), findsOneWidget);
      expect(find.text('Loaded host'), findsOneWidget);
      expect(indicators, findsNothing);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}

class _PresentedTestBinding extends AutomatedTestWidgetsFlutterBinding {
  @override
  Future<void> get waitUntilFirstFrameRasterized => Future<void>.value();
}
