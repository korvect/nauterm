import 'dart:async';
import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ai/ai_config.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/data/ai_provider_store.dart';
import 'package:nauterm/data/nauterm_config_store.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/data/workspace_data_loader.dart';
import 'package:nauterm/terminal/terminal_recording_config.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';
import 'package:nauterm/workspace/workspace_state_store.dart';

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
      final remoteId = store.saveHost(
        HostEntry(
          name: 'Remote host',
          type: NautermHostType.remote,
          host: 'example.test',
          password: 'stored-password',
          telnetEnabled: true,
          telnetPassword: 'stored-telnet-password',
          environmentVariables: const [
            HostEnvironmentVariable(variable: 'TERM', value: 'xterm'),
          ],
          groupId: groupId,
          config: const {
            'icon': 'cisco',
            'ssh': {'compatibility_mode': true},
          },
        ),
      );
      final serialId = store.saveHost(
        const HostEntry(
          name: 'Serial host',
          type: NautermHostType.serial,
          serialPort: '/dev/cu.test-console',
          serialBaudRate: 115200,
        ),
      );
      store.saveSftpFavoritePath(
        SftpFavoritePathEntry(
          scope: SftpFavoriteScope.remote,
          hostId: remoteId,
          path: '/srv/data',
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
      final details = await loadNautermHostDetails(
        paths.databasePath,
        remote.id!,
      );
      expect(details?.uuid, remote.uuid);
      expect(details?.password, 'stored-password');
      expect(details?.telnetPassword, 'stored-telnet-password');
      expect(details?.environmentVariables.single.toJson(), {
        'variable': 'TERM',
        'value': 'xterm',
      });
      expect(details?.config, remote.config);
      final favorites = await loadNautermRemoteSftpFavorites(
        paths.databasePath,
        remoteId,
      );
      expect(favorites.map((favorite) => favorite.path), ['/srv/data']);
      expect(
        await loadNautermRemoteSftpFavorites(paths.databasePath, serialId),
        isEmpty,
      );
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

  test('unreadable shell history does not block loading hosts', () async {
    final directory = Directory.systemTemp.createTempSync(
      'nauterm-workspace-history-',
    );
    addTearDown(() => directory.deleteSync(recursive: true));
    final paths = NautermPaths(
      configDirectory: directory,
      dataDirectory: directory,
    );
    final store = NautermDataStore.openPath(paths.databasePath);
    try {
      store.initEncryption();
      store.saveHost(
        const HostEntry(
          name: 'Host',
          type: NautermHostType.remote,
          host: 'example.test',
        ),
      );
    } finally {
      store.dispose();
    }
    await paths.shellHistoryFile.writeAsBytes([0xff]);

    final workspace = await loadNautermWorkspaceData(
      paths,
      const NautermRecordingConfig(),
    );
    expect(workspace.hosts.single.host, 'example.test');
    expect(workspace.shellHistory, isEmpty);
  });

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
      final wait = Stopwatch()..start();
      while (!loading && wait.elapsed < const Duration(seconds: 5)) {
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

  testWidgets('Logs can retry a failed initial load', (tester) async {
    final requests = <Completer<NautermWorkspaceData>>[];
    final controller = NautermWorkspaceController(
      dataLoader: (_, _) {
        final request = Completer<NautermWorkspaceData>();
        requests.add(request);
        return request.future;
      },
    );
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      NautermApp(onOpenSettings: () {}, workspaceController: controller),
    );
    await tester.pump(const Duration(milliseconds: 150));
    await tester.tap(find.text('Logs'));
    await tester.pump(const Duration(milliseconds: 200));
    await _pumpUntil(tester, () => requests.length == 1);
    expect(find.text('Loading logs…'), findsOneWidget);
    expect(find.text('No terminal sessions recorded yet.'), findsNothing);

    requests.single.completeError(
      StateError('Log store temporarily unavailable'),
    );
    final retry = find.text('Retry loading logs');
    await _pumpUntil(tester, () => retry.evaluate().isNotEmpty);
    expect(
      find.text('Bad state: Log store temporarily unavailable'),
      findsOneWidget,
    );
    await tester.tap(retry);
    await _pumpUntil(tester, () => requests.length == 2);
    expect(find.text('Loading logs…'), findsOneWidget);

    requests.last.complete(
      NautermWorkspaceData(
        terminalLogs: [
          TerminalLogEntry(
            id: '01979f62-8548-7000-8000-000000000402',
            title: 'Recovered log',
            captureFile: '',
            startedAt: DateTime.utc(2026, 11, 26),
          ),
        ],
      ),
    );
    final recovered = find.byKey(
      const ValueKey('01979f62-8548-7000-8000-000000000402'),
    );
    await _pumpUntil(tester, () => recovered.evaluate().isNotEmpty);
    expect(retry, findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('loaded empty logs keep the table header and empty body', (
    tester,
  ) async {
    final controller = NautermWorkspaceController(
      dataLoader: (_, _) async => const NautermWorkspaceData(),
    );
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      NautermApp(onOpenSettings: () {}, workspaceController: controller),
    );
    await tester.tap(find.text('Logs'));
    final empty = find.text('No terminal sessions recorded yet.');
    await _pumpUntil(tester, () => empty.evaluate().isNotEmpty);

    expect(
      find.descendant(
        of: find.byKey(const ValueKey('logs-table-body')),
        matching: empty,
      ),
      findsOneWidget,
    );
    for (final label in const ['Date', 'User', 'Host', 'Actions']) {
      expect(find.text(label), findsOneWidget);
    }
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('loaded empty known hosts follow the selected language', (
    tester,
  ) async {
    final previousLanguage = appLanguage;
    final previousLocalizations = NautermLocalizations.current;
    await tester.runAsync(
      () => NautermLocalizations.load(const Locale('zh', 'CN')),
    );
    setAppLanguage(AppLanguage.english);
    addTearDown(() {
      setAppLanguage(previousLanguage);
      NautermLocalizations.current = previousLocalizations;
    });
    final controller = NautermWorkspaceController(
      dataLoader: (_, _) async => const NautermWorkspaceData(),
    );
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      NautermApp(onOpenSettings: () {}, workspaceController: controller),
    );
    await tester.tap(find.text('Known Hosts'));
    await _pumpUntil(
      tester,
      () => find.text('No known hosts yet.').evaluate().isNotEmpty,
    );

    setAppLanguage(AppLanguage.simplifiedChinese);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text('暂无已知主机。'), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('new group can create and select a root parent group', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final directory = Directory.systemTemp.createTempSync(
      'nauterm-workspace-group-',
    );
    addTearDown(() => directory.deleteSync(recursive: true));
    final paths = NautermPaths(
      configDirectory: directory,
      dataDirectory: directory,
    );
    File(nautermLinuxDefaultConfigAsset).copySync(paths.configFile.path);
    final controller = NautermWorkspaceController(
      dataPaths: paths,
      dataLoader: (_, _) async => const NautermWorkspaceData(),
    );
    addTearDown(controller.dispose);
    var ready = false;
    unawaited(controller.initialDataReady.then((_) => ready = true));
    await tester.pumpWidget(
      NautermApp(onOpenSettings: () {}, workspaceController: controller),
    );
    await _pumpUntil(tester, () => ready);

    Finder field(String label) => find.descendant(
      of: find.ancestor(
        of: find.text(label),
        matching: find.byType(InputDecorator),
      ),
      matching: find.byType(TextField),
    );

    await tester.tap(find.byTooltip('New host actions'));
    await tester.pump(const Duration(milliseconds: 150));
    await tester.tap(find.text('New group'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));
    await tester.enterText(field('Name *'), 'Child group');

    final parentField = field('Parent group');
    await tester.tap(parentField);
    await tester.enterText(parentField, 'Root parent');
    await tester.pump();
    await tester.tap(find.text('Create group Root parent'));
    await tester.pump();
    expect(parentField, findsNothing);
    expect(
      tester.widget<TextField>(field('Name *')).controller!.text,
      'Root parent',
    );

    await tester.tap(find.text('Save'));
    await tester.pump();
    expect(
      tester.widget<TextField>(field('Name *')).controller!.text,
      'Child group',
    );
    expect(
      tester.widget<TextField>(parentField).controller!.text,
      'Root parent',
    );
    expect(
      find.byKey(const ValueKey('workspace-select-menu:Parent group')),
      findsNothing,
    );
    await tester.tap(find.text('Save'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));
    await tester.pumpWidget(const SizedBox.shrink());

    final store = NautermDataStore.openPath(paths.databasePath);
    addTearDown(store.dispose);
    final groups = store.listGroups();
    final parent = groups.singleWhere((group) => group.name == 'Root parent');
    final child = groups.singleWhere((group) => group.name == 'Child group');
    expect(parent.parentId, isNull);
    expect(child.parentId, parent.id);
  });

  testWidgets('workspace toolbar searches can focus and show empty results', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final directory = Directory.systemTemp.createTempSync(
      'nauterm-workspace-search-',
    );
    addTearDown(() => directory.deleteSync(recursive: true));
    final controller = NautermWorkspaceController(
      workspaceStateStore: WorkspaceStateStore(
        NautermPaths(configDirectory: directory, dataDirectory: directory),
      ),
      dataLoader: (_, _) async => const NautermWorkspaceData(),
    );
    addTearDown(controller.dispose);
    var ready = false;
    unawaited(controller.initialDataReady.then((_) => ready = true));
    await tester.pumpWidget(
      NautermApp(onOpenSettings: () {}, workspaceController: controller),
    );
    await _pumpUntil(tester, () => ready);

    const pages = {
      'Keychain': 'No keychain items found.',
      'Proxies': 'No matching proxies',
      'Port Forwarding': 'No matching forwarding rules',
      'Snippets': 'No snippets found.',
      'Known Hosts': 'No matching known hosts.',
    };
    final field = find.byKey(const ValueKey('workspace-toolbar-search-field'));
    for (final MapEntry(key: page, value: emptyTitle) in pages.entries) {
      final tooltip = find.byTooltip(page);
      await tester.tap(
        tooltip.evaluate().isNotEmpty ? tooltip : find.text(page),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
      await tester.tap(find.byTooltip('Search'));
      await tester.pump();
      await tester.pump();
      expect(
        tester.widget<TextField>(field).focusNode?.hasFocus,
        isTrue,
        reason: '$page search did not receive focus.',
      );
      await tester.enterText(field, 'missing-test-result');
      await tester.pump();
      expect(find.text(emptyTitle), findsOneWidget);
    }
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

Future<void> _pumpUntil(WidgetTester tester, bool Function() condition) async {
  final wait = Stopwatch()..start();
  while (!condition() && wait.elapsed < const Duration(seconds: 5)) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 10)),
    );
    await tester.pump(const Duration(milliseconds: 20));
  }
  expect(condition(), isTrue);
}

class _PresentedTestBinding extends AutomatedTestWidgetsFlutterBinding {
  @override
  Future<void> get waitUntilFirstFrameRasterized => Future<void>.value();
}
