import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ai/ai_config.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/data/ai_provider_store.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/settings/settings_panel.dart';
import 'package:nauterm/window/native_windowing.dart';

import 'support/connection_uri_defaults.dart';

void main() {
  setUp(mockConnectionUriDefaults);

  setUp(() async {
    final previous = aiAssistantConfig;
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
    settingsRuntimeSettingsSaverOverride = (_) async {};
    addTearDown(() {
      settingsRuntimeSettingsSaverOverride = null;
      setAiAssistantConfig(previous);
    });
  });

  testWidgets('AI page requests work before and after opening settings', (
    tester,
  ) async {
    showSettingsWindow(page: NautermSettingsPage.ai);
    await _pumpSettings(tester, _ProviderStore());
    expect(find.text('Example'), findsOneWidget);
    showSettingsWindow(page: NautermSettingsPage.about);
    await tester.pumpAndSettle();
    expect(find.text('Example'), findsNothing);
    showSettingsWindow(page: NautermSettingsPage.ai);
    await tester.pumpAndSettle();
    expect(find.text('Example'), findsOneWidget);
    expect(takeRequestedSettingsPage(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('provider edits remain a draft until Save, with validation', (
    tester,
  ) async {
    final store = _ProviderStore();
    await _pumpSettings(tester, store);
    expect(find.byKey(const ValueKey('settings-ai-name-field')), findsNothing);
    await tester.tap(find.text('Example'));
    await tester.pumpAndSettle();
    await tester.enterText(_field('name'), 'Renamed');
    await tester.tap(find.byKey(const ValueKey('settings-ai-advanced')));
    await tester.pumpAndSettle();
    await tester.enterText(_field('maxTokens'), '0');
    await tester.tap(find.byKey(const ValueKey('settings-ai-save-provider')));
    await tester.pumpAndSettle();
    expect(
      find.text('Max Tokens must be a positive whole number or blank.'),
      findsOneWidget,
    );
    expect(store.saves, 0);
    expect(store.entries.single.name, 'Example');
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(store.saves, 0);

    await tester.tap(find.text('Example'));
    await tester.pumpAndSettle();
    expect(
      tester.widget<TextField>(_field('name')).controller!.text,
      'Example',
    );
    await tester.enterText(_field('name'), 'Renamed');
    await tester.enterText(_field('models'), 'new-model, other-model');
    await tester.tap(find.byKey(const ValueKey('settings-ai-save-provider')));
    await tester.pumpAndSettle();
    expect(store.saves, 1);
    expect(find.text('Renamed'), findsOneWidget);
    expect(find.byKey(const ValueKey('settings-ai-name-field')), findsNothing);
    expect(store.entries.single.model, 'new-model, other-model');
    expect(aiAssistantConfig.model, 'new-model, other-model');
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
    'the last provider can be deleted and a new draft can be cancelled',
    (tester) async {
      final store = _ProviderStore();
      await _pumpSettings(tester, store);
      await tester.tap(find.byTooltip('Delete provider'));
      await tester.pumpAndSettle();
      expect(find.text('Delete Example?'), findsOneWidget);
      await tester.tap(
        find.byKey(const ValueKey('settings-ai-confirm-delete')),
      );
      await tester.pumpAndSettle();
      expect(store.entries, isEmpty);
      expect(aiAssistantConfig.apiKey, isEmpty);
      expect(
        find.text('No AI providers yet. Add one to use the assistant.'),
        findsOneWidget,
      );

      await tester.tap(find.byKey(const ValueKey('settings-ai-add-provider')));
      await tester.pumpAndSettle();
      final blank = find.text('Create Blank Provider');
      if (blank.evaluate().isNotEmpty) {
        await tester.tap(blank);
        await tester.pumpAndSettle();
      }
      expect(
        find.byKey(const ValueKey('settings-ai-save-provider')),
        findsOneWidget,
      );
      expect(
        tester.widget<TextField>(_field('name')).controller!.text,
        'Custom Provider',
      );
      await tester.enterText(_field('name'), 'Cancelled');
      expect(store.saves, 0);
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();
      expect(store.entries, isEmpty);
      expect(store.saves, 0);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}

Finder _field(String name) => find.descendant(
  of: find.byKey(ValueKey('settings-ai-$name-field')),
  matching: find.byType(TextField),
);

Future<void> _pumpSettings(WidgetTester tester, _ProviderStore store) async {
  tester.view.physicalSize = const Size(1000, 820);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    MaterialApp(
      home: SettingsPanel(detectExternalEditors: false, aiProviderStore: store),
    ),
  );
  await tester.pump();
  await tester.tap(find.byKey(const ValueKey('settings-nav-ai')));
  await tester.pumpAndSettle();
}

class _ProviderStore extends AiProviderStore {
  _ProviderStore()
    : super(
        NautermPaths(
          configDirectory: Directory('/tmp/nauterm-ai-editor-test'),
          dataDirectory: Directory('/tmp/nauterm-ai-editor-test'),
        ),
      );

  var saves = 0;
  List<AiProviderEntry> entries = [
    const AiProviderEntry(
      id: 1,
      uuid: 'provider-1',
      name: 'Example',
      protocol: 'openai',
      baseUrl: 'https://example.com/v1',
      model: 'old-model',
      apiKey: 'test-key',
      config: {},
      active: true,
    ),
  ];

  @override
  List<AiProviderEntry> listProviders() => List.unmodifiable(entries);

  @override
  LoadedAiProviderCatalog loadCatalog(AiAssistantConfig fallback) =>
      LoadedAiProviderCatalog(
        active: LoadedAiProvider(config: fallback),
        providers: listProviders(),
      );

  @override
  Future<LoadedAiProviderCatalog> loadCatalogAsync(
    AiAssistantConfig fallback,
  ) async => loadCatalog(fallback);

  @override
  AiProviderEntry save(
    AiAssistantConfig config, {
    AiProviderEntry? existing,
    String? name,
    bool? active,
  }) {
    saves++;
    final entry = AiProviderEntry(
      id: existing?.id ?? 2,
      uuid: existing?.uuid ?? 'provider-2',
      name: name ?? existing!.name,
      protocol: config.protocol.storageValue,
      baseUrl: config.baseUrl,
      model: config.model,
      apiKey: config.apiKey,
      config: {
        if (config.maxTokens != null) 'max_tokens': config.maxTokens,
        if (config.temperature != null) 'temperature': config.temperature,
      },
      active: active ?? existing?.active ?? true,
    );
    entries = [...entries.where((p) => p.id != entry.id), entry];
    return entry;
  }

  @override
  void deleteProvider(AiProviderEntry entry) =>
      entries.removeWhere((p) => p.id == entry.id);
}
