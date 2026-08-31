import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/settings/github_device_flow.dart';
import 'package:nauterm/settings/settings_panel.dart';
import 'package:nauterm/terminal/system_shells.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/window/native_windowing.dart';
import 'package:package_info_plus/package_info_plus.dart';

void main() {
  setUp(() async {
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
  });

  testWidgets('settings render in Simplified Chinese', (
    WidgetTester tester,
  ) async {
    setAppLanguage(AppLanguage.simplifiedChinese);
    await tester.runAsync(() async {
      NautermLocalizations.current = await NautermLocalizations.load(
        const Locale('zh', 'CN'),
      );
    });
    addTearDown(() {
      setAppLanguage(AppLanguage.english);
      NautermLocalizations.current = const NautermLocalizations(Locale('en'));
    });
    await tester.pumpWidget(
      const MaterialApp(
        locale: Locale('zh', 'CN'),
        supportedLocales: NautermLocalizations.supportedLocales,
        localizationsDelegates: [
          NautermLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        home: SettingsPanel(detectExternalEditors: false),
      ),
    );
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 50)),
    );
    await tester.pump();

    expect(find.text('常规设置'), findsOneWidget);
    expect(find.text('语言'), findsOneWidget);
    expect(find.text('外观'), findsOneWidget);
    expect(find.text('终端'), findsOneWidget);
    expect(find.text('同步与备份'), findsOneWidget);
    final sidebarFocusGroup = find.byKey(
      const ValueKey('settings-sidebar-focus-group'),
    );
    final contentFocusGroup = find.byKey(
      const ValueKey('settings-content-focus-group'),
    );
    expect(sidebarFocusGroup, findsOneWidget);
    expect(contentFocusGroup, findsOneWidget);
    expect(
      find.descendant(
        of: sidebarFocusGroup,
        matching: find.byKey(const ValueKey('settings-nav-general')),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: contentFocusGroup,
        matching: find.byKey(const ValueKey('settings-search-field')),
      ),
      findsOneWidget,
    );

    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    final cjkFontField = find.byKey(
      const ValueKey('settings-terminal-cjk-font-select'),
    );
    await tester.scrollUntilVisible(
      cjkFontField,
      300,
      scrollable: find
          .descendant(
            of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    expect(find.text('CJK 字体'), findsOneWidget);
    expect(cjkFontField, findsOneWidget);
    await Scrollable.ensureVisible(
      tester.element(cjkFontField),
      alignment: 0.5,
    );
    await tester.pump();
    await tester.tap(cjkFontField);
    await tester.pump();
    expect(find.text('自动'), findsWidgets);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('language options use autonyms in English', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        locale: Locale('en'),
        supportedLocales: NautermLocalizations.supportedLocales,
        localizationsDelegates: [
          NautermLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        home: SettingsPanel(detectExternalEditors: false),
      ),
    );
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 50)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-language-select')));
    await tester.pumpAndSettle();

    expect(find.text('English'), findsWidgets);
    expect(find.text('简体中文'), findsOneWidget);
    expect(find.text('language.english.autonym'), findsNothing);
    expect(find.text('language.simplifiedChinese.autonym'), findsNothing);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('sidebar and content keep independent tab traversal loops', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pumpAndSettle();

    final sidebarScope = find.byKey(
      const ValueKey('settings-sidebar-focus-scope'),
    );
    final contentScope = find.byKey(
      const ValueKey('settings-content-focus-scope'),
    );
    expect(sidebarScope, findsOneWidget);
    expect(contentScope, findsOneWidget);

    bool primaryFocusIsInside(Finder scope) {
      final focusContext = FocusManager.instance.primaryFocus?.context;
      if (focusContext == null) return false;
      return find
          .descendant(
            of: scope,
            matching: find.byElementPredicate(
              (element) => identical(element, focusContext),
            ),
          )
          .evaluate()
          .isNotEmpty;
    }

    await tester.tap(find.byKey(const ValueKey('settings-search-field')));
    await tester.pump();
    expect(primaryFocusIsInside(contentScope), isTrue);
    for (var index = 0; index < 12; index++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(primaryFocusIsInside(contentScope), isTrue);
    }

    final generalNav = find.byKey(const ValueKey('settings-nav-general'));
    await tester.tap(generalNav);
    await tester.pump();
    final sidebarFocusScope = FocusScope.of(tester.element(generalNav));
    sidebarFocusScope.traversalDescendants.first.requestFocus();
    await tester.pump();
    expect(primaryFocusIsInside(sidebarScope), isTrue);
    for (var index = 0; index < 12; index++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(primaryFocusIsInside(sidebarScope), isTrue);
    }

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('credential reveal buttons are skipped by tab traversal', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-nav-sync')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));

    final revealButtons = find.byKey(
      const ValueKey('settings-credential-reveal'),
    );
    expect(revealButtons, findsWidgets);
    for (final reveal in tester.widgetList<ExcludeFocusTraversal>(
      revealButtons,
    )) {
      expect(reveal.excluding, isTrue);
    }
    expect(
      find.descendant(of: revealButtons, matching: find.byType(IconButton)),
      findsWidgets,
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('about page shows application information and updates', (
    WidgetTester tester,
  ) async {
    PackageInfo.setMockInitialValues(
      appName: 'Nauterm',
      packageName: 'com.korvect.nauterm',
      version: '1.2.3',
      buildNumber: '42',
      buildSignature: '',
    );
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-about')));
    await tester.pumpAndSettle();

    expect(find.text('About Nauterm'), findsOneWidget);
    expect(find.text('1.2.3'), findsOneWidget);
    expect(find.text('42'), findsOneWidget);
    expect(find.text('com.korvect.nauterm'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('settings-about-build-number')),
      findsOneWidget,
    );
    await tester.scrollUntilVisible(
      find.byKey(const ValueKey('settings-update-action')),
      300,
      scrollable: find.byType(Scrollable).last,
    );
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('settings-update-action')),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('settings-update-action')),
        matching: find.byType(OutlinedButton),
      ),
      findsOneWidget,
    );
    expect(find.text('Third-party licenses'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('settings-about-third-party-licenses')),
      findsOneWidget,
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('about page opens third-party licenses', (
    WidgetTester tester,
  ) async {
    LicenseRegistry.addLicense(
      () => Stream<LicenseEntry>.fromIterable(const <LicenseEntry>[
        LicenseEntryWithLineBreaks(<String>[
          'nauterm_test_dependency',
        ], 'Test dependency license text.'),
        LicenseEntryWithLineBreaks(<String>[
          'nauterm_test_dependency_two',
        ], 'Test dependency license text.'),
        LicenseEntryWithLineBreaks(<String>[
          'nauterm_test_dependency',
        ], 'Second test dependency notice.'),
      ]),
    );
    PackageInfo.setMockInitialValues(
      appName: 'Nauterm',
      packageName: 'com.korvect.nauterm',
      version: '1.2.3',
      buildNumber: '42',
      buildSignature: '',
    );
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-about')));
    await tester.pumpAndSettle();
    await tester.tap(
      find.byKey(const ValueKey('settings-about-third-party-licenses')),
    );
    await tester.pumpAndSettle();

    expect(find.byType(LicensePage), findsNothing);
    expect(
      find.byKey(const ValueKey('third-party-license-browser')),
      findsOneWidget,
    );
    expect(find.text('Nauterm 1.2.3 (42)'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('third-party-license-nauterm_test_dependency')),
      findsOneWidget,
    );
    expect(
      find.byKey(
        const ValueKey('third-party-license-nauterm_test_dependency_two'),
      ),
      findsOneWidget,
    );
    expect(find.textContaining('nauterm_test_dependency +'), findsNothing);
    await tester.tap(
      find.byKey(const ValueKey('third-party-license-nauterm_test_dependency')),
    );
    await tester.pump();
    expect(find.text('2 license notices'), findsOneWidget);
    expect(find.text('Test dependency license text.'), findsOneWidget);
    expect(find.text('Second test dependency notice.'), findsOneWidget);

    await tester.enterText(
      find.byKey(const ValueKey('third-party-license-search')),
      'missing package',
    );
    await tester.pump();
    expect(find.text('No matching packages.'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('about menu request opens the about settings page', (
    WidgetTester tester,
  ) async {
    showSettingsWindow(page: NautermSettingsPage.about);

    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pumpAndSettle();

    expect(find.text('About Nauterm'), findsOneWidget);
    expect(find.byKey(const ValueKey('settings-nav-about')), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('terminal request opens the terminal settings page', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pumpAndSettle();

    showSettingsWindow(page: NautermSettingsPage.terminal);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));

    expect(
      find.byKey(const ValueKey('settings-terminal-scroll-view')),
      findsOneWidget,
    );
    final selectCommandBlockOnClickSwitch = find.byKey(
      const ValueKey('settings-terminal-select-command-block-on-click-switch'),
    );
    await tester.scrollUntilVisible(
      selectCommandBlockOnClickSwitch,
      300,
      scrollable: find
          .descendant(
            of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    expect(find.text('Select Command Block on Click'), findsOneWidget);
    expect(selectCommandBlockOnClickSwitch, findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('settings input supports mouse drag selection', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const ValueKey('settings-nav-sftp')));
    await tester.pumpAndSettle();

    final field = find.byKey(const ValueKey('settings-sftp-ssh-editor-input'));
    await Scrollable.ensureVisible(tester.element(field), alignment: 0.5);
    await tester.pumpAndSettle();
    await tester.enterText(
      find.descendant(of: field, matching: find.byType(EditableText)),
      '1234567890',
    );
    await tester.pump();

    final editable = find.descendant(
      of: field,
      matching: find.byType(EditableText),
    );
    final rect = tester.getRect(editable);
    final gesture = await tester.startGesture(
      Offset(rect.right - 4, rect.center.dy),
      kind: PointerDeviceKind.mouse,
    );
    await tester.pump();
    await gesture.moveTo(Offset(rect.left + 4, rect.center.dy));
    await tester.pump();
    await gesture.up();
    await tester.pump();

    final selection = tester
        .widget<EditableText>(editable)
        .controller
        .selection;
    expect(selection.isCollapsed, isFalse);
  });

  testWidgets('settings layout adapts to compact widths', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(620, 440);
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });
    try {
      await tester.pumpWidget(
        const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
      );
      await tester.pumpAndSettle();

      expect(find.text('Nauterm'), findsNothing);
      expect(find.byTooltip('General'), findsOneWidget);
      expect(
        find.byKey(const ValueKey('application-theme-traffic-lights')),
        findsNWidgets(3),
      );
      expect(tester.takeException(), isNull);

      await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
      await tester.pump();

      final fontField = find.byKey(
        const ValueKey('settings-terminal-font-select'),
      );
      final sizeField = find.byKey(
        const ValueKey('settings-terminal-font-size-select'),
      );
      await tester.scrollUntilVisible(
        fontField,
        300,
        scrollable: find
            .descendant(
              of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
              matching: find.byType(Scrollable),
            )
            .first,
      );
      await tester.pump();
      expect(
        tester.getTopLeft(sizeField).dy,
        greaterThan(tester.getTopLeft(fontField).dy),
      );
      expect(tester.takeException(), isNull);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });

  testWidgets('settings select shows compact options and selected check', (
    WidgetTester tester,
  ) async {
    final originalFont = terminalFontConfig;
    addTearDown(() => terminalFontConfig = originalFont);
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey('application-theme-light')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('application-theme-system')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('application-theme-dark')),
      findsOneWidget,
    );
    await tester.tap(find.byKey(const ValueKey('application-theme-system')));
    await tester.pumpAndSettle();
    expect(
      find.byKey(const ValueKey('application-theme-selected-system')),
      findsOneWidget,
    );

    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();
    await tester.scrollUntilVisible(
      find.byKey(const ValueKey('settings-terminal-font-select')),
      300,
      scrollable: find
          .descendant(
            of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    await tester.pump();
    final fontSelectRect = tester.getRect(
      find.byKey(const ValueKey('settings-terminal-font-select')),
    );
    final fontInputRect = tester.getRect(
      find.byKey(const ValueKey('settings-select-input')),
    );
    for (final x in [fontSelectRect.right - 19, fontSelectRect.right - 3]) {
      await tester.tapAt(Offset(x, fontInputRect.center.dy));
      await tester.pump();
      expect(
        find.byKey(const ValueKey('settings-select-menu')),
        findsOneWidget,
      );
      await tester.tapAt(const Offset(4, 4));
      await tester.pump();
      expect(find.byKey(const ValueKey('settings-select-menu')), findsNothing);
    }
    await tester.tap(
      find.byKey(const ValueKey('settings-terminal-font-select')),
    );
    await tester.pump();

    final fontMenu = find.byKey(const ValueKey('settings-select-menu'));
    expect(tester.getSize(fontMenu).height, lessThanOrEqualTo(34 * 6));
    final fontMenuScrollbar = find.descendant(
      of: fontMenu,
      matching: find.byKey(const ValueKey('settings-select-scrollbar')),
    );
    expect(fontMenuScrollbar, findsOneWidget);
    expect(tester.widget<Scrollbar>(fontMenuScrollbar).thumbVisibility, isTrue);
    expect(tester.widget<Scrollbar>(fontMenuScrollbar).interactive, isTrue);

    expect(find.text('01'), findsNothing);
    expect(find.byIcon(Icons.check_rounded), findsOneWidget);
    expect(find.text(terminalFontConfig.resolvedFamily()), findsWidgets);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();

    expect(find.byIcon(Icons.check_rounded), findsNothing);

    await tester.tap(
      find.byKey(const ValueKey('settings-terminal-font-select')),
    );
    await tester.pump();
    await tester.enterText(
      find.byKey(const ValueKey('settings-select-input')),
      'MesloLGS NF Custom',
    );
    await tester.pump();
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('settings-select-menu')),
        matching: find.text('MesloLGS NF Custom'),
      ),
      findsOneWidget,
    );
    expect(
      tester.getSize(find.byKey(const ValueKey('settings-select-menu'))).height,
      34,
    );
    expect(
      tester
          .widget<Scrollbar>(
            find.byKey(const ValueKey('settings-select-scrollbar')),
          )
          .thumbVisibility,
      isFalse,
    );
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();
    expect(terminalFontConfig.family, 'MesloLGS NF Custom');
    expect(find.byKey(const ValueKey('settings-select-menu')), findsNothing);

    await tester.tap(
      find.byKey(const ValueKey('settings-terminal-font-select')),
    );
    await tester.pump();
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('settings-select-menu')),
        matching: find.text('Menlo'),
      ),
      findsOneWidget,
    );
    await tester.enterText(
      find.byKey(const ValueKey('settings-select-input')),
      'Cascadia',
    );
    await tester.pump();
    expect(
      find.descendant(
        of: find.byKey(const ValueKey('settings-select-menu')),
        matching: find.text('Menlo'),
      ),
      findsNothing,
    );
    await tester.tapAt(const Offset(4, 4));
    await tester.pump();
    expect(
      tester
          .widget<TextField>(
            find.byKey(const ValueKey('settings-select-input')),
          )
          .controller
          ?.text,
      'MesloLGS NF Custom',
    );

    await tester.tap(
      find.byKey(const ValueKey('settings-terminal-font-size-select')),
    );
    await tester.pump();

    expect(find.textContaining(RegExp(r'^\d+(?:\.\d+)?\s*px$')), findsNothing);
    expect(find.byIcon(Icons.check_rounded), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('settings-nav-ai')));
    await tester.pumpAndSettle();

    expect(find.text('AI Assistant'), findsWidgets);
    expect(find.text('PROVIDERS'), findsOneWidget);
    expect(find.text('Terminal Context'.toUpperCase()), findsOneWidget);

    final protocolSelect = find.byKey(
      const ValueKey('settings-ai-protocol-select'),
    );
    if (protocolSelect.evaluate().isNotEmpty) {
      expect(find.text('Advanced'), findsOneWidget);
      expect(find.text('Max Tokens'), findsNothing);
      await tester.tap(find.text('Advanced'));
      await tester.pumpAndSettle();
      expect(find.text('Max Tokens'), findsOneWidget);
      expect(
        find.byKey(const ValueKey('settings-ai-max-tokens-field')),
        findsOneWidget,
      );

      await tester.tap(protocolSelect);
      await tester.pump();
      expect(find.text('Anthropic'), findsOneWidget);
      expect(find.text('OpenAI Compatible'), findsNothing);
      expect(find.text('Google Gemini'), findsNothing);
    }

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('settings search finds and opens matching settings', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 760);
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();

    await tester.enterText(
      find.byKey(const ValueKey('settings-search-field')),
      'autocomplete',
    );
    await tester.pump();

    expect(find.text('Composer'), findsOneWidget);
    expect(find.text('1 matching settings'), findsOneWidget);
    await tester.tap(
      find.byKey(const ValueKey('settings-search-result-terminal-Composer')),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 16));
    await tester.pump(const Duration(milliseconds: 300));
    await tester.pump();

    expect(find.text('Interaction'.toUpperCase()), findsOneWidget);
    final terminalScrollable = tester.state<ScrollableState>(
      find
          .descendant(
            of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    expect(terminalScrollable.position.pixels, greaterThan(0));
    expect(find.byKey(const ValueKey('settings-search-field')), findsOneWidget);
    expect(
      tester
          .widget<TextField>(
            find.byKey(const ValueKey('settings-search-field')),
          )
          .controller
          ?.text,
      isEmpty,
    );
  });

  testWidgets('shell path uses a dropdown with system default first', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 820);
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();

    final select = find.byKey(const ValueKey('settings-terminal-shell-select'));
    await tester.scrollUntilVisible(
      select,
      300,
      scrollable: find
          .descendant(
            of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    await tester.ensureVisible(select);
    await tester.pump();
    await tester.tap(select);
    await tester.pump();

    final defaultPath = systemDefaultShellPath();
    final defaultLabel = defaultPath == null
        ? 'System Default'
        : 'System Default — ${shellDisplayNameWithVersion(defaultPath)} ($defaultPath)';
    expect(find.text(defaultLabel), findsWidgets);
    if (defaultPath != null) {
      expect(
        find.text('${shellDisplayNameWithVersion(defaultPath)} — $defaultPath'),
        findsNothing,
      );
    }
    await tester.tap(find.text(defaultLabel).last);
    await tester.pump();

    expect(terminalShellPath, isNull);
    expect(find.text(defaultLabel), findsOneWidget);
  });

  testWidgets('terminal emulator selector applies to new sessions', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 820);
    final originalBackend = terminalEmulatorBackend;
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
      terminalEmulatorBackend = originalBackend;
    });
    terminalEmulatorBackend = TerminalEmulatorBackend.alacritty;

    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();

    final select = find.byKey(
      const ValueKey('settings-terminal-emulator-select'),
    );
    await tester.scrollUntilVisible(
      select,
      300,
      scrollable: find
          .descendant(
            of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    await Scrollable.ensureVisible(tester.element(select), alignment: 0.5);
    await tester.pump();
    await tester.tap(select);
    await tester.pump();
    await tester.tap(find.text('Ghostty').last);
    await tester.pump();

    expect(terminalEmulatorBackend, TerminalEmulatorBackend.ghostty);
  });

  testWidgets('SSH prediction selector updates new-session behavior', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 820);
    final originalMode = terminalSshPredictionMode;
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
      terminalSshPredictionMode = originalMode;
    });
    terminalSshPredictionMode = TerminalSshPredictionMode.adaptive;

    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();

    final select = find.byKey(
      const ValueKey('settings-terminal-ssh-prediction-select'),
    );
    await tester.scrollUntilVisible(
      select,
      300,
      scrollable: find
          .descendant(
            of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    await Scrollable.ensureVisible(tester.element(select), alignment: 0.5);
    await tester.pump();
    await tester.tap(select);
    await tester.pump();
    await tester.tap(find.text('Always').last);
    await tester.pump();

    expect(terminalSshPredictionMode, TerminalSshPredictionMode.always);
  });

  testWidgets('settings select uses native tooltips only for overflow', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(800, 820);
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });
    const longFontFamily =
        'A deliberately long CJK font family name that must be truncated';
    final originalFont = terminalFontConfig;
    terminalFontConfig = originalFont.copyWith(cjkFamily: longFontFamily);
    addTearDown(() => terminalFontConfig = originalFont);

    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();

    final select = find.byKey(
      const ValueKey('settings-terminal-cjk-font-select'),
    );
    await tester.scrollUntilVisible(
      select,
      300,
      scrollable: find
          .descendant(
            of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    await tester.pump();
    await tester.tap(select);
    await tester.pump();

    final menu = find.byKey(const ValueKey('settings-select-menu'));
    expect(menu, findsOneWidget);
    final tooltips = find.descendant(of: menu, matching: find.byType(Tooltip));
    expect(tooltips, findsOneWidget);
    final tooltip = tester.widget<Tooltip>(tooltips);
    expect(tooltip.message, longFontFamily);
    expect(tooltip.waitDuration, const Duration(milliseconds: 300));
    expect(tooltip.constraints, const BoxConstraints(maxWidth: 360));
    expect(
      find.descendant(of: menu, matching: find.byTooltip('Auto')),
      findsNothing,
    );
  });

  testWidgets('settings select menu follows its field while scrolling', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 820);
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();

    final select = find.byKey(
      const ValueKey('settings-terminal-cjk-font-select'),
    );
    final scrollable = find
        .descendant(
          of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
          matching: find.byType(Scrollable),
        )
        .first;
    await tester.scrollUntilVisible(select, 300, scrollable: scrollable);
    await tester.tap(select);
    await tester.pump();

    final menu = find.byKey(const ValueKey('settings-select-menu'));
    final before = tester.getTopLeft(menu);
    final position = tester.state<ScrollableState>(scrollable).position;
    position.jumpTo(
      (position.pixels + 24).clamp(
        position.minScrollExtent,
        position.maxScrollExtent,
      ),
    );
    await tester.pump();
    final after = tester.getTopLeft(menu);

    expect(after.dy, closeTo(before.dy - 24, 1));
  });

  testWidgets('terminal retention limits are configurable', (
    WidgetTester tester,
  ) async {
    final previous = terminalRecordingConfig;
    Directory? openedDirectory;
    settingsDirectoryOpenerOverride = (directory) async {
      openedDirectory = directory;
    };
    addTearDown(() => terminalRecordingConfig = previous);
    addTearDown(() => settingsDirectoryOpenerOverride = null);
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 1200);
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();

    final totalLimit = find.byKey(
      const ValueKey('settings-terminal-total-limit-select'),
    );
    final scrollable = tester.state<ScrollableState>(
      find
          .descendant(
            of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    scrollable.position.jumpTo(scrollable.position.maxScrollExtent);
    await tester.pump();

    expect(totalLimit, findsOneWidget);
    expect(find.text('Record Terminal Sessions'), findsOneWidget);
    expect(find.text('Raw Terminal Output'), findsNothing);
    expect(find.text('Reserved · not enforced'), findsNothing);
    expect(find.text('HISTORY & STORAGE'), findsOneWidget);
    expect(
      find.textContaining(
        'Deleting a history item also deletes its capture file.',
      ),
      findsNothing,
    );

    await tester.tap(totalLimit);
    await tester.pump();
    await tester.tap(find.text('5 GB'));
    await tester.pump();
    expect(terminalRecordingConfig.maxTotalBytes, 5 * 1024 * 1024 * 1024);

    await tester.tap(
      find.byKey(const ValueKey('settings-terminal-recording-location')),
    );
    await tester.pump();
    expect(
      openedDirectory?.path,
      NautermPaths.resolve().terminalLogsDirectory.path,
    );
  });

  testWidgets('settings reset menu resets the current page', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 760);
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('application-theme-dark')));
    await tester.pump();
    expect(
      find.byKey(const ValueKey('application-theme-selected-dark')),
      findsOneWidget,
    );

    await tester.tap(find.byKey(const ValueKey('settings-reset-menu')));
    await tester.pumpAndSettle();
    expect(find.text('Reset Current Page'), findsOneWidget);
    expect(find.text('Reset All Settings'), findsOneWidget);
    await tester.tap(find.text('Reset Current Page'));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const ValueKey('application-theme-selected-light')),
      findsOneWidget,
    );
    expect(find.text('Page reset to defaults.'), findsOneWidget);
  });

  testWidgets('shortcut recording cancels on an outside tap', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 900);
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });

    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-shortcuts')));
    await tester.pumpAndSettle();

    final recorder = find.byKey(
      const ValueKey('shortcut-recorder:Quick Connect'),
    );
    final recorderSurface = find.byKey(
      const ValueKey('shortcut-recorder-surface:Quick Connect'),
    );
    expect(recorder, findsOneWidget);
    expect(tester.getSize(recorderSurface).height, 28);
    expect(find.text('Reset all'), findsOneWidget);

    await tester.tap(recorder);
    await tester.pump();
    expect(find.text('Press shortcut...'), findsOneWidget);

    await tester.tap(find.text('Shortcuts').last);
    await tester.pump();
    expect(find.text('Press shortcut...'), findsNothing);
  });

  testWidgets('data sync page exposes storage providers and their settings', (
    WidgetTester tester,
  ) async {
    githubDeviceFlowClientIdOverride = '';
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 820);
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
      githubDeviceFlowClientIdOverride = null;
    });

    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-sync')));
    await tester.pumpAndSettle();

    expect(find.text('Master Key'), findsWidgets);
    expect(find.text('Confirm Master Key'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('settings-sync-master-key-requirements')),
      findsOneWidget,
    );
    expect(find.text('12+ characters'), findsOneWidget);
    expect(find.text('Uppercase'), findsOneWidget);
    expect(find.text('Lowercase'), findsOneWidget);
    expect(find.text('Number'), findsOneWidget);
    expect(find.text('Symbol'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('settings-sync-master-key-strength')),
      findsNothing,
    );
    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('settings-sync-master-key')),
        matching: find.byType(EditableText),
      ),
      'short',
    );
    await tester.pump();
    expect(find.text('Password strength: Weak'), findsOneWidget);
    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('settings-sync-master-key')),
        matching: find.byType(EditableText),
      ),
      'correct-master-key',
    );
    await tester.pump();
    expect(find.text('Password strength: Weak'), findsOneWidget);
    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('settings-sync-master-key')),
        matching: find.byType(EditableText),
      ),
      'Correct-master-key1',
    );
    await tester.pump();
    expect(find.text('Password strength: Good'), findsOneWidget);
    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('settings-sync-master-key-confirm')),
        matching: find.byType(EditableText),
      ),
      'Correct-master-key1',
    );
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();

    expect(find.text('Sync & Backup'), findsWidgets);
    expect(find.text('Merge Strategy'), findsOneWidget);
    expect(find.text('Smart Merge'), findsOneWidget);
    expect(find.text('Automatic Sync'), findsOneWidget);
    expect(find.text('minutes'), findsOneWidget);
    expect(find.text('Local Backups'), findsOneWidget);
    expect(find.text('Local Version'), findsOneWidget);
    expect(find.text('Remote Version'), findsOneWidget);
    expect(find.text('Sync Status'), findsNothing);
    expect(find.text('No sync providers yet'), findsNothing);
    expect(find.text('GitHub Repository'), findsOneWidget);
    expect(find.text('GitHub Gist'), findsOneWidget);
    expect(find.text('S3'), findsOneWidget);
    expect(find.text('Add Provider'), findsOneWidget);
    expect(find.text('GITHUB AUTHENTICATION'), findsNothing);

    await tester.tap(find.text('S3'));
    await tester.pumpAndSettle();
    expect(find.text('S3 CONNECTION'), findsNothing);
    expect(
      find.byKey(const ValueKey('settings-sync-provider-info')),
      findsOneWidget,
    );
    expect(
      find.byTooltip(
        'Supports Amazon S3, Cloudflare R2, Backblaze B2, DigitalOcean '
        'Spaces, Wasabi, Alibaba Cloud OSS, Tencent Cloud COS, and other '
        'services exposing an S3-compatible API. MinIO is also available as '
        'a separate provider preset.',
      ),
      findsOneWidget,
    );
    await tester.tap(find.byTooltip('Close'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Add Provider'));
    await tester.pumpAndSettle();
    expect(find.text('Add Sync Provider'), findsOneWidget);
    expect(find.text('GitHub Repository'), findsOneWidget);
    expect(find.text('GitHub Gist'), findsOneWidget);
    expect(find.text('Amazon S3'), findsNothing);
    expect(find.text('Cloudflare R2'), findsNothing);
    expect(find.text('Backblaze B2'), findsNothing);
    expect(find.text('DigitalOcean Spaces'), findsNothing);
    expect(find.text('Wasabi'), findsNothing);
    expect(find.text('MinIO'), findsOneWidget);
    expect(find.text('S3'), findsOneWidget);
    expect(find.text('Google Drive'), findsOneWidget);
    expect(find.text('WebDAV'), findsOneWidget);
    expect(find.text('OneDrive'), findsOneWidget);
    expect(find.text('Alibaba Cloud OSS'), findsNothing);
    expect(find.text('Tencent Cloud COS'), findsNothing);
    expect(find.text('Azure Blob Storage'), findsOneWidget);
    expect(find.text('Dropbox'), findsOneWidget);
    expect(find.text('Google Cloud Storage'), findsOneWidget);

    await tester.tap(find.byTooltip('Close'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('GitHub Repository'));
    await tester.pumpAndSettle();
    expect(find.text('GITHUB AUTHENTICATION'), findsOneWidget);
    expect(find.text('Access Token'), findsOneWidget);
    expect(find.text('Repository URL'), findsOneWidget);
    expect(find.text('Save Repository'), findsOneWidget);
    await tester.tap(find.byTooltip('Close'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('GitHub Gist'));
    await tester.pumpAndSettle();
    expect(find.text('Connect to GitHub'), findsOneWidget);
    expect(find.text('Access Token'), findsNothing);
    expect(find.textContaining('NAUTERM_GITHUB_CLIENT_ID'), findsOneWidget);
    await tester.tap(find.byTooltip('Close'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Add Provider'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('MinIO'));
    await tester.pumpAndSettle();
    expect(find.text('Add MinIO'), findsOneWidget);
    expect(find.text('Endpoint'), findsOneWidget);
    expect(find.text('Region'), findsOneWidget);
    expect(find.text('Bucket'), findsOneWidget);
    expect(find.text('Access Key ID'), findsOneWidget);
    expect(find.text('Secret Access Key'), findsOneWidget);
    expect(find.text('Folder'), findsOneWidget);
    expect(find.text('Filename'), findsOneWidget);
    expect(find.byTooltip('Show credential'), findsNWidgets(2));
    expect(find.textContaining('stored securely'), findsNothing);
    expect(
      find.text('Optional folder path for the encrypted backup.'),
      findsNothing,
    );
    await tester.tap(find.byTooltip('Close'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Add Provider'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('OneDrive'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('OneDrive'));
    await tester.pumpAndSettle();
    expect(find.text('Root Folder'), findsOneWidget);
    expect(find.text('Authorization'), findsOneWidget);
    expect(find.text('Connect'), findsOneWidget);
    expect(find.text('Access Token'), findsNothing);
    expect(find.text('Folder'), findsNothing);
    await tester.tap(find.byTooltip('Close'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Add Provider'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Dropbox'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Dropbox'));
    await tester.pumpAndSettle();
    expect(find.text('Root Folder'), findsOneWidget);
    expect(find.text('/Nauterm/'), findsOneWidget);
    expect(
      find.byWidgetPredicate(
        (widget) =>
            widget is TextField && widget.decoration?.hintText == 'Backups',
      ),
      findsOneWidget,
    );
    await tester.tap(find.byTooltip('Close'));
    await tester.pumpAndSettle();

    expect(tester.takeException(), isNull);
  });

  testWidgets('data sync requires matching Master Key confirmation', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 820);
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });

    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-sync')));
    await tester.pumpAndSettle();

    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('settings-sync-master-key')),
        matching: find.byType(EditableText),
      ),
      'Correct-master-key1',
    );
    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('settings-sync-master-key-confirm')),
        matching: find.byType(EditableText),
      ),
      'Different-master-key2',
    );
    final continueButton = find.text('Continue');
    await tester.ensureVisible(continueButton);
    await tester.pumpAndSettle();
    await tester.tap(continueButton);
    await tester.pump();

    expect(
      find.text('Master Key confirmation does not match.'),
      findsOneWidget,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('sync key details exposes Master Key rotation validation', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(760, 820);
    cacheGithubSyncSettingsForTesting(
      NautermPaths.resolve().databasePath,
      <String, Object?>{
        'config': null,
        'gistConfig': <String, Object?>{'gist_id': ''},
        's3Config': null,
        'cloudProviders': <Object?>[
          <String, Object?>{
            'id': 'provider-1',
            'scheme': 's3',
            'vendor': 'minio',
            'name': 'Lab MinIO',
            'config': <String, String>{
              'endpoint': 'https://storage.example.com',
              'region': 'auto',
              'bucket': 'vault',
              'prefix': 'backups',
              'filename': 'nauterm-sync.enc',
            },
            'has_credentials': true,
          },
          <String, Object?>{
            'id': 'provider-2',
            'scheme': 's3',
            'vendor': 'r2',
            'name': 'Archive R2',
            'config': <String, String>{
              'endpoint': 'https://r2.example.com',
              'region': 'auto',
              'bucket': 'archive',
              'prefix': 'backups',
              'filename': 'nauterm-sync.enc',
            },
            'has_credentials': true,
          },
          <String, Object?>{
            'id': 'provider-3',
            'scheme': 's3',
            'vendor': 'b2',
            'name': 'Backup B2',
            'config': <String, String>{
              'endpoint': 'https://b2.example.com',
              'region': 'auto',
              'bucket': 'backup',
              'prefix': 'backups',
              'filename': 'nauterm-sync.enc',
            },
            'has_credentials': true,
          },
        ],
        'syncPreferences': <String, Object?>{
          'active_provider_id': 'cloud:provider-1',
        },
        'hasToken': false,
        'hasGistToken': false,
        'hasS3Credentials': false,
        'hasLocalSyncKey': true,
      },
    );
    addTearDown(() {
      clearGithubSyncSettingsCacheForTesting();
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });

    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump(const Duration(milliseconds: 200));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('settings-nav-sync')));
    await tester.pumpAndSettle();
    expect(find.text('Lab MinIO'), findsOneWidget);
    final activeCard = find.byKey(
      const ValueKey('settings-cloud-provider-provider-1'),
    );
    final inactiveCard = find.byKey(
      const ValueKey('settings-cloud-provider-provider-2'),
    );
    final thirdCard = find.byKey(
      const ValueKey('settings-cloud-provider-provider-3'),
    );
    expect(
      find.descendant(of: activeCard, matching: find.byTooltip('Sync')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: activeCard, matching: find.text('Active')),
      findsNothing,
    );
    expect(
      find.descendant(
        of: activeCard,
        matching: find.textContaining('Connected'),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(of: inactiveCard, matching: find.byTooltip('Sync')),
      findsNothing,
    );
    expect(
      find.descendant(of: inactiveCard, matching: find.byTooltip('History')),
      findsNothing,
    );
    expect(
      find.descendant(of: inactiveCard, matching: find.text('Activate')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: inactiveCard, matching: find.textContaining('Ready')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: inactiveCard, matching: find.byTooltip('Settings')),
      findsOneWidget,
    );
    expect(
      tester.getSize(activeCard).width,
      tester.getSize(inactiveCard).width,
    );
    expect(tester.getSize(activeCard).width, tester.getSize(thirdCard).width);
    expect(
      tester.getTopLeft(activeCard).dy,
      tester.getTopLeft(inactiveCard).dy,
    );
    expect(tester.getTopLeft(activeCard).dy, tester.getTopLeft(thirdCard).dy);
    await tester.tap(find.text('Key details'));
    await tester.pumpAndSettle();
    expect(find.text('Forget Master Key'), findsOneWidget);
    await tester.tap(find.text('Change Master Key'));
    await tester.pumpAndSettle();

    expect(find.byType(SettingsPanel), findsOneWidget);
    expect(find.text('Sync & Backup'), findsWidgets);
    expect(
      find.byKey(const ValueKey('settings-sync-current-master-key')),
      findsOneWidget,
    );
    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('settings-sync-current-master-key')),
        matching: find.byType(EditableText),
      ),
      'Current-master-key1',
    );
    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('settings-sync-new-master-key')),
        matching: find.byType(EditableText),
      ),
      'New-master-key-value1',
    );
    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('settings-sync-new-master-key-confirm')),
        matching: find.byType(EditableText),
      ),
      'Different-new-key1',
    );
    await tester.tap(
      find.byKey(const ValueKey('settings-sync-change-master-key-submit')),
    );
    await tester.pump();

    expect(
      find.text('New Master Key confirmation does not match.'),
      findsOneWidget,
    );
  });

  test('data sync isolate tasks do not capture widget state', () async {
    final directory = Directory.systemTemp.createTempSync(
      'nauterm_settings_sync_',
    );
    addTearDown(() => directory.deleteSync(recursive: true));
    final databasePath = '${directory.path}/settings.sqlite';

    await saveGithubConfigInBackgroundForTesting(
      databasePath: databasePath,
      repositoryUrl: 'git@github.com:octocat/nauterm-vault.git',
      branch: 'main',
      path: 'nauterm-sync.enc',
    );

    final store = NautermDataStore.openPath(databasePath);
    try {
      expect(store.githubLoadConfig(), {
        'repository_url': 'git@github.com:octocat/nauterm-vault.git',
        'branch': 'main',
        'path': 'nauterm-sync.enc',
      });
      store.githubGistSaveConfig(gistId: 'abc123');
      expect(store.githubGistLoadConfig(), {
        'gist_id': 'abc123',
        'filename': 'nauterm-sync.enc',
      });
    } finally {
      store.dispose();
    }
  });
}
