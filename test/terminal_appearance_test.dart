import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/data/nauterm_config.dart';
import 'package:nauterm/data/nauterm_config_store.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/data/terminal_theme_store.dart';
import 'package:nauterm/settings/settings_panel.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/terminal/terminal_theme.dart';
import 'package:nauterm/terminal/terminal_widget.dart';
import 'package:nauterm/ui/terminal_theme_preview.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUp(() {
    final previous = currentNautermRuntimeSettings();
    addTearDown(() => applyNautermRuntimeSettings(previous));
  });

  test(
    'old configs preserve their fixed theme and default dark to Nysa Dark',
    () {
      final config = NautermTerminalAppearanceConfig.fromJson({
        'theme': {'id': 'ayu-light'},
      });
      expect(config.themeId, 'ayu-light');
      expect(config.followAppTheme, isFalse);
      expect(config.darkThemeId, nysaDarkTerminalThemeId);
    },
  );

  test(
    'both theme slots and follow preference round-trip independently',
    () async {
      final directory = Directory.systemTemp.createTempSync(
        'nauterm-dual-theme-config-',
      );
      addTearDown(() => directory.deleteSync(recursive: true));
      final paths = NautermPaths(
        configDirectory: directory,
        dataDirectory: directory,
      );
      final store = NautermConfigStore(paths);
      await store.saveRuntimeSettings(
        NautermRuntimeSettings(
          copyOnSelect: false,
          font: const TerminalFontConfig(),
          keyboard: const TerminalKeyboardConfig(),
          themeId: 'ayu-light',
          darkThemeId: 'ayu-dark',
          followAppTheme: true,
          customThemeJson: nysaLightTerminalTheme.toJson(),
          darkCustomThemeJson: nysaDarkTerminalTheme.toJson(),
        ),
      );
      await store.saveTerminalTheme(
        themeId: 'custom',
        customThemeJson: nysaDarkTerminalTheme.toJson(),
        dark: true,
      );
      final saved = await store.loadConfig();
      expect(saved.terminal.appearance.customThemeJson!['name'], 'Nysa Light');
      final settings = await store.loadRuntimeSettings();
      expect(settings.themeId, 'ayu-light');
      expect(settings.darkThemeId, 'custom');
      expect(settings.followAppTheme, isTrue);
      expect(
        settings.customThemeJson,
        (await loadBundledTerminalTheme('ayu-light'))!.toJson(),
      );
      expect(settings.darkCustomThemeJson!['name'], 'Nysa Dark');
    },
  );

  test('SSH and Telnet dark themes survive group/host writes, edits, and summaries', () {
    final directory = Directory.systemTemp.createTempSync('nauterm-dual-host-');
    final store = NautermDataStore.openPath('${directory.path}/test.sqlite');
    addTearDown(() {
      store.dispose();
      directory.deleteSync(recursive: true);
    });
    final groupId = store.saveGroup(
      const HostGroup(
        name: 'Group',
        themeId: 'ayu-light',
        darkThemeId: 'ayu-dark',
        telnetThemeId: 'nysa-light',
        telnetDarkThemeId: 'nysa-dark',
      ),
    );
    final group = store.listGroups().single;
    expect(group.darkThemeId, 'ayu-dark');
    expect(group.telnetDarkThemeId, 'nysa-dark');
    final hostId = store.saveHost(
      HostEntry(
        type: NautermHostType.remote,
        name: 'Host',
        groupId: groupId,
        host: 'example.test',
        themeId: group.themeId,
        darkThemeId: group.darkThemeId,
        telnetThemeId: group.telnetThemeId,
        telnetDarkThemeId: group.telnetDarkThemeId,
      ),
    );
    var host = store.getHost(hostId)!;
    expect(host.darkThemeId, 'ayu-dark');
    expect(host.telnetDarkThemeId, 'nysa-dark');
    store.saveHost(
      HostEntry(
        type: NautermHostType.remote,
        id: hostId,
        name: 'Updated',
        host: 'example.test',
        darkThemeId: 'nysa-dark',
        telnetDarkThemeId: 'ayu-dark',
      ),
    );
    host = store.listHosts().single;
    expect(host.darkThemeId, 'nysa-dark');
    expect(host.telnetDarkThemeId, 'ayu-dark');
    expect(HostEntry.fromJson(host.toJson()).telnetDarkThemeId, 'ayu-dark');
  });

  test('existing dark-session cells map to the new light theme without reconnecting', () {
    final painter = TerminalPainter(
      snapshot: TerminalSnapshot.blank(columns: 2, rows: 1),
      metrics: TerminalMetrics.measure(defaultTerminalConfig.font.textStyle()),
      textStyle: defaultTerminalConfig.font.textStyle(),
      showCursor: false,
      composingText: null,
      selection: null,
      focused: true,
      theme: nysaLightTerminalTheme,
      sourceTheme: nysaDarkTerminalTheme,
    );
    expect(
      painter.resolveColorForTesting(nysaDarkTerminalTheme.primary.background),
      nysaLightTerminalTheme.primary.background,
    );
    expect(
      painter.resolveColorForTesting(nysaDarkTerminalTheme.primary.foreground),
      nysaLightTerminalTheme.primary.foreground,
    );
  });

  testWidgets('both previews occupy the existing left editor column', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1150, 1050);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    settingsRuntimeSettingsSaverOverride = (_) async {};
    addTearDown(() => settingsRuntimeSettingsSaverOverride = null);
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();
    final light = find.byKey(const ValueKey('settings-light-theme-slot'));
    final dark = find.byKey(const ValueKey('settings-dark-theme-slot'));
    final scrollable = find
        .descendant(
          of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
          matching: find.byType(Scrollable),
        )
        .first;
    await tester.scrollUntilVisible(light, 200, scrollable: scrollable);
    expect(tester.getTopLeft(light).dy, tester.getTopLeft(dark).dy);
    await tester.tap(dark);
    await tester.pump();
    await tester.ensureVisible(
      find.byKey(const ValueKey('settings-terminal-follow-appearance')),
    );
    await tester.pump();
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey('settings-terminal-follow-appearance')),
        matching: find.byType(CupertinoSwitch),
      ),
    );
    await tester.pump();
    expect(terminalFollowAppTheme, isTrue);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
    'custom preview names remain visible when either slot is inactive',
    (tester) async {
      terminalThemeId = 'custom';
      terminalDarkThemeId = 'custom';
      terminalCustomTheme = nysaLightTerminalTheme;
      terminalDarkCustomTheme = nysaDarkTerminalTheme;
      tester.view.physicalSize = const Size(1150, 1050);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(
        const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
      );
      await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
      await tester.pump();
      final light = find.byKey(const ValueKey('settings-light-theme-slot'));
      final dark = find.byKey(const ValueKey('settings-dark-theme-slot'));
      await tester.scrollUntilVisible(
        light,
        200,
        scrollable: find
            .descendant(
              of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
              matching: find.byType(Scrollable),
            )
            .first,
      );
      for (final slot in [dark, light]) {
        await tester.tap(slot);
        await tester.pump();
        for (final preview in [light, dark]) {
          expect(
            tester
                .widget<TerminalThemePreviewCard>(
                  find.descendant(
                    of: preview,
                    matching: find.byType(TerminalThemePreviewCard),
                  ),
                )
                .title,
            'Custom',
          );
        }
      }
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets(
    'host defaults preview the customized light and dark application themes',
    (tester) async {
      tester.view.physicalSize = const Size(1440, 1050);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(() async {
        await tester.pumpWidget(const SizedBox.shrink());
      });
      terminalThemeId = 'custom';
      terminalDarkThemeId = 'custom';
      terminalCustomTheme = TerminalTheme.fromJson({
        ...nysaLightTerminalTheme.toJson(),
        'background': '#f0e0d0',
      });
      terminalDarkCustomTheme = TerminalTheme.fromJson({
        ...nysaDarkTerminalTheme.toJson(),
        'background': '#102030',
      });
      await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
      await tester.tap(find.text('New host'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 250));
      tester
          .widget<InkWell>(find.byKey(const ValueKey('protocol-show-more')))
          .onTap!();
      await tester.pump();
      final editor = find.byKey(const ValueKey('workspace-editor-scroll-view'));
      final previews = find.descendant(
        of: editor,
        matching: find.byType(TerminalThemePreviewCard),
      );
      for (final dark in [false, true]) {
        final preview = dark ? previews.last : previews.first;
        final expected = dark ? terminalDarkCustomTheme : terminalCustomTheme;
        await tester.scrollUntilVisible(
          preview,
          200,
          scrollable: find
              .descendant(of: editor, matching: find.byType(Scrollable))
              .first,
        );
        await Scrollable.ensureVisible(tester.element(preview), alignment: 0.5);
        await tester.pump();
        expect(
          tester.widget<TerminalThemePreviewCard>(preview).title,
          'Default',
        );
        expect(
          tester.widget<TerminalThemePreviewCard>(preview).theme.toJson(),
          expected.toJson(),
        );
        await tester.tap(preview);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 250));
        await tester.pump();
        final cards = find.descendant(
          of: find.byType(GridView),
          matching: find.byType(TerminalThemePreviewCard),
        );
        final defaultCard = cards
            .evaluate()
            .map((element) => element.widget as TerminalThemePreviewCard)
            .singleWhere((card) => card.title == 'Default');
        expect(defaultCard.theme.toJson(), expected.toJson());
        expect(defaultCard.selected, isTrue);
        final defaultPreview = find
            .descendant(
              of: find.byType(GridView),
              matching: find.widgetWithText(
                TerminalThemePreviewCard,
                'Default',
              ),
            )
            .hitTestable();
        expect(defaultPreview, findsOneWidget);
        await tester.tap(defaultPreview);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 250));
        expect(
          tester.widget<TerminalThemePreviewCard>(preview).title,
          'Default',
        );
        expect(
          tester.widget<TerminalThemePreviewCard>(preview).theme.toJson(),
          expected.toJson(),
        );
      }
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets(
    'an open terminal follows forced appearance changes without reconnecting',
    (tester) async {
      terminalFollowAppTheme = true;
      terminalThemeId = null;
      terminalCustomTheme = nysaLightTerminalTheme;
      terminalDarkThemeId = nysaDarkTerminalThemeId;
      terminalDarkCustomTheme = nysaDarkTerminalTheme;
      setAppThemeMode(AppThemeMode.dark);
      await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
      await tester.pump();
      await tester.tap(find.text('Terminal'));
      await tester.pump();
      final view = find.byType(TerminalView);
      final controller = tester.widget<TerminalView>(view).controller;
      expect(tester.widget<TerminalView>(view).theme, nysaDarkTerminalTheme);
      setAppThemeMode(AppThemeMode.light);
      await tester.pump();
      await tester.pump();
      expect(tester.widget<TerminalView>(view).theme, nysaLightTerminalTheme);
      expect(tester.widget<TerminalView>(view).controller, same(controller));
      setAppThemeMode(AppThemeMode.dark);
      await tester.pump();
      await tester.pump();
      expect(tester.widget<TerminalView>(view).theme, nysaDarkTerminalTheme);
      terminalFollowAppTheme = false;
      terminalConfigNotifier.value++;
      await tester.pump();
      await tester.pump();
      expect(tester.widget<TerminalView>(view).theme, nysaLightTerminalTheme);
      terminalFollowAppTheme = true;
      tester.binding.platformDispatcher.platformBrightnessTestValue =
          Brightness.light;
      addTearDown(
        tester.binding.platformDispatcher.clearPlatformBrightnessTestValue,
      );
      setAppThemeMode(AppThemeMode.system);
      await tester.pump();
      await tester.pump();
      expect(tester.widget<TerminalView>(view).theme, nysaLightTerminalTheme);
      tester.binding.platformDispatcher.platformBrightnessTestValue =
          Brightness.dark;
      await tester.pump();
      await tester.pump();
      expect(tester.widget<TerminalView>(view).theme, nysaDarkTerminalTheme);
      expect(tester.widget<TerminalView>(view).controller, same(controller));
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}
