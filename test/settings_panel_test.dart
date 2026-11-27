import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/settings/cloud_oauth.dart';
import 'package:nauterm/settings/github_device_flow.dart';
import 'package:nauterm/settings/settings_panel.dart';
import 'package:nauterm/terminal/system_shells.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/update/desktop_update.dart';
import 'package:nauterm/window/native_windowing.dart';
import 'package:package_info_plus/package_info_plus.dart';

import 'support/connection_uri_defaults.dart';

void main() {
  setUp(mockConnectionUriDefaults);

  testWidgets('terminal padding keeps fractional values and rejects NaN', (
    tester,
  ) async {
    final previous = terminalPadding;
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 820);
    addTearDown(() {
      terminalPadding = previous;
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();
    final field = find.byKey(const ValueKey('settings-terminal-padding'));
    await tester.scrollUntilVisible(
      field,
      250,
      scrollable: find
          .descendant(
            of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    final input = find.descendant(
      of: field,
      matching: find.byType(EditableText),
    );
    await tester.enterText(input, '1.25, 2.5, 3.75');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();
    expect(terminalPadding, const EdgeInsets.fromLTRB(2.5, 1.25, 2.5, 3.75));

    await tester.enterText(input, 'NaN');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();
    expect(
      tester.widget<EditableText>(input).controller.text,
      '1.25, 2.5, 3.75',
    );
    expect(terminalPadding, const EdgeInsets.fromLTRB(2.5, 1.25, 2.5, 3.75));
  });

  testWidgets('word boundary field decodes tabs and permits an empty set', (
    tester,
  ) async {
    final original = terminalWordBoundaries;
    terminalWordBoundaries = defaultTerminalWordBoundaries;
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 820);
    addTearDown(() {
      terminalWordBoundaries = original;
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();
    final field = find.byKey(
      const ValueKey('settings-terminal-word-boundaries'),
    );
    await tester.scrollUntilVisible(
      field,
      300,
      scrollable: find
          .descendant(
            of: find.byKey(const ValueKey('settings-terminal-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    final input = find.descendant(
      of: field,
      matching: find.byType(EditableText),
    );
    expect(
      tester.widget<EditableText>(input).controller.text,
      formatTerminalWordBoundaries(defaultTerminalWordBoundaries),
    );
    await tester.enterText(input, r' /\t');
    await tester.pump();
    expect(terminalWordBoundaries, ' /\t');
    await tester.enterText(input, '');
    await tester.pump();
    expect(terminalWordBoundaries, '');
  });

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
          ...GlobalMaterialLocalizations.delegates,
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
    final search = find.byKey(const ValueKey('settings-search-field'));
    await tester.enterText(search, '新终端窗格');
    await tester.pump();
    expect(
      find.byKey(const ValueKey('settings-search-result-terminal-Composer')),
      findsOneWidget,
    );
    await tester.enterText(search, '');
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('settings-nav-terminal')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.text('ANSI 颜色'), findsOneWidget);
    expect(find.text('Normal'), findsOneWidget);
    expect(find.text('Bright'), findsOneWidget);
    for (final color in [
      'Black',
      'Red',
      'Green',
      'Yellow',
      'Blue',
      'Magenta',
      'Cyan',
      'White',
    ]) {
      expect(find.text(color), findsNWidgets(2));
    }
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
          ...GlobalMaterialLocalizations.delegates,
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

  testWidgets('workspace restore selection updates and saves the behavior', (
    WidgetTester tester,
  ) async {
    final previous = workspaceRestoreBehavior;
    workspaceRestoreBehavior = WorkspaceRestoreBehavior.ask;
    NautermRuntimeSettings? savedSettings;
    settingsRuntimeSettingsSaverOverride = (settings) async {
      savedSettings = settings;
    };
    addTearDown(() {
      workspaceRestoreBehavior = previous;
      settingsRuntimeSettingsSaverOverride = null;
    });

    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 50)),
    );
    await tester.pump();

    final select = find.byKey(
      const ValueKey('settings-workspace-restore-behavior'),
    );
    await tester.scrollUntilVisible(
      select,
      200,
      scrollable: find
          .descendant(
            of: find.byKey(const ValueKey('settings-general-scroll-view')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    await tester.pumpAndSettle();
    expect(select, findsOneWidget);
    await tester.ensureVisible(select);
    await tester.pumpAndSettle();
    expect(find.text('Restore Workspace on Startup'), findsOneWidget);
    expect(find.text('Ask on Startup'), findsOneWidget);
    await tester.tap(select);
    await tester.pumpAndSettle();
    final menu = find.byKey(const ValueKey('settings-select-menu'));
    for (final label in ['Ask on Startup', 'Always', 'Never']) {
      expect(
        find.descendant(of: menu, matching: find.text(label)),
        findsOneWidget,
      );
    }
    await tester.tap(find.descendant(of: menu, matching: find.text('Always')));
    await tester.pumpAndSettle();

    expect(workspaceRestoreBehavior, WorkspaceRestoreBehavior.always);
    expect(
      savedSettings?.workspaceRestoreBehavior,
      WorkspaceRestoreBehavior.always,
    );
    expect(
      find.descendant(of: select, matching: find.text('Always')),
      findsOneWidget,
    );
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
    const sparkle = MethodChannel('com.korvect.nauterm/sparkle');
    final messenger = tester.binding.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(sparkle, (call) async {
      expect(call.method, 'isConfigured');
      return false;
    });
    addTearDown(() => messenger.setMockMethodCallHandler(sparkle, null));
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
    final platform = tester.widget<Text>(
      find.byKey(const ValueKey('settings-about-platform')),
    );
    expect(platform.data, matches(r'^[^·]+ · [a-z0-9_]+$'));
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
    if (Platform.isMacOS) {
      expect(find.text('Updates are managed by Sparkle.'), findsOneWidget);
      expect(
        find.text(
          'This build does not include a Sparkle update feed and public key.',
        ),
        findsOneWidget,
      );
      expect(
        tester
            .widget<OutlinedButton>(
              find.descendant(
                of: find.byKey(const ValueKey('settings-update-action')),
                matching: find.byType(OutlinedButton),
              ),
            )
            .onPressed,
        isNull,
      );
    }
    expect(find.text('Third-party licenses'), findsOneWidget);
    expect(
      find.byKey(const ValueKey('settings-about-third-party-licenses')),
      findsOneWidget,
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('about offers browser fallback and cancels in-app downloads', (
    WidgetTester tester,
  ) async {
    PackageInfo.setMockInitialValues(
      appName: 'Nauterm',
      packageName: 'com.korvect.nauterm',
      version: '1.2.3',
      buildNumber: '42',
      buildSignature: '',
    );
    const releaseUri = 'https://example.com/releases/v2.0.0';
    const packageUri = 'https://example.com/releases/nauterm-setup.exe';
    final manifestUri = Uri.parse(
      'https://example.com/releases/SHA256SUMS.txt',
    );
    final opened = <String>[];
    var releaseChecks = 0;
    settingsDesktopUpdateServiceFactoryOverride = () => DesktopUpdateService(
      repository: 'example/nauterm',
      target: const DesktopUpdateTarget(
        platform: DesktopUpdatePlatform.windows,
        architecture: DesktopUpdateArchitecture.x86_64,
      ),
      client: MockClient.streaming((request, _) async {
        if (request.url == manifestUri) {
          await (request as http.Abortable).abortTrigger!;
          throw http.RequestAbortedException(request.url);
        }
        releaseChecks++;
        return http.StreamedResponse(
          Stream.value(
            utf8.encode(
              jsonEncode({
                'tag_name': 'v2.0.0',
                'html_url': releaseUri,
                'body': 'Example release notes',
                'assets': [
                  {
                    'name': 'Nauterm-2.0.0-windows-x86_64-setup.exe',
                    'browser_download_url': packageUri,
                    'size': 8,
                  },
                  if (releaseChecks > 1)
                    {
                      'name': 'SHA256SUMS.txt',
                      'browser_download_url': manifestUri.toString(),
                      'size': 15,
                    },
                ],
              }),
            ),
          ),
          200,
        );
      }),
      processRunner: (executable, arguments) async {
        expect(executable, 'rundll32');
        expect(arguments.first, 'url.dll,FileProtocolHandler');
        opened.add(arguments.last);
        return ProcessResult(0, 0, '', '');
      },
    );
    addTearDown(() => settingsDesktopUpdateServiceFactoryOverride = null);
    await tester.pumpWidget(
      const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-about')));
    await tester.pumpAndSettle();
    final action = find.byKey(const ValueKey('settings-update-action'));
    await tester.scrollUntilVisible(
      action,
      300,
      scrollable: find.byType(Scrollable).last,
    );
    await tester.tap(action);
    await tester.pumpAndSettle();
    expect(find.text('Nauterm 2.0.0 is available.'), findsOneWidget);
    expect(find.text('Download update'), findsNothing);
    for (final label in ['Release details', 'Download in Browser']) {
      final button = find.widgetWithText(OutlinedButton, label);
      await tester.ensureVisible(button);
      await tester.pump();
      await tester.tap(button);
      await tester.pumpAndSettle();
    }
    expect(opened, [releaseUri, packageUri]);
    await tester.ensureVisible(action);
    await tester.pump();
    await tester.tap(action);
    await tester.pumpAndSettle();
    expect(find.text('Download update'), findsOneWidget);
    await tester.ensureVisible(action);
    await tester.pump();
    await tester.tap(action);
    await tester.pump(const Duration(milliseconds: 150));
    expect(find.text('Checking release checksums…'), findsOneWidget);
    final cancel = find.widgetWithText(OutlinedButton, 'Cancel download');
    await tester.ensureVisible(cancel);
    await tester.pump();
    await tester.tap(cancel);
    await tester.pumpAndSettle();
    expect(find.text('Download cancelled.'), findsOneWidget);
    expect(find.text('Download update'), findsOneWidget);
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
    final licenseButton = find.byKey(
      const ValueKey('settings-about-third-party-licenses'),
    );
    await tester.ensureVisible(licenseButton);
    await tester.pumpAndSettle();
    await tester.tap(licenseButton);
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
    final input = tester.widget<EditableText>(editable);
    expect(input.cursorHeight, isNull);
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
    final fontInput = tester.widget<TextField>(
      find.byKey(const ValueKey('settings-select-input')),
    );
    expect(fontInput.cursorHeight, isNull);
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
    final menuBorder = tester.widget<DecoratedBox>(
      find.descendant(
        of: fontMenu,
        matching: find.byKey(const ValueKey('settings-select-border')),
      ),
    );
    expect(menuBorder.position, DecorationPosition.foreground);
    expect((menuBorder.decoration as BoxDecoration).border, isNotNull);
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
    final selectedFontOption = tester.widget<Text>(
      find.descendant(
        of: fontMenu,
        matching: find.text(terminalFontConfig.resolvedFamily()),
      ),
    );
    expect(selectedFontOption.style?.fontFamily, isNull);
    final selectedFontPreview = tester.widget<Text>(
      find.descendant(
        of: fontMenu,
        matching: find.byKey(
          ValueKey(
            'settings-font-option-preview:${terminalFontConfig.resolvedFamily()}',
          ),
        ),
      ),
    );
    expect(
      selectedFontPreview.style?.fontFamily,
      terminalFontConfig.resolvedFamily(),
    );

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

    final searchField = find.byKey(const ValueKey('settings-search-field'));
    final terminalScrollView = find.byKey(
      const ValueKey('settings-terminal-scroll-view'),
    );
    const cases = [
      (query: 'font size', title: 'Terminal Font', section: 'FONT'),
      (query: 'ligatures', title: 'Ligatures', section: 'FONT'),
      (query: '连字', title: 'Ligatures', section: 'FONT'),
      (query: 'autocomplete', title: 'Composer', section: 'LAYOUT'),
      (query: 'multiple tabs', title: 'Multiple Tabs', section: 'SESSION'),
      (query: 'filename', title: 'Open Links and Files', section: 'INPUT'),
      (
        query: 'command block',
        title: 'Select Command Block on Click',
        section: 'SELECTION',
      ),
    ];
    for (final target in cases) {
      await tester.enterText(searchField, target.query);
      await tester.pump();

      final result = find.byKey(
        ValueKey('settings-search-result-terminal-${target.title}'),
      );
      expect(result, findsOneWidget);
      expect(find.text('1 result'), findsOneWidget);
      await tester.tap(result);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 16));
      await tester.pump(const Duration(milliseconds: 300));
      await tester.pump();

      final section = find.text(target.section);
      expect(section, findsOneWidget);
      expect(
        tester.getRect(terminalScrollView).contains(tester.getCenter(section)),
        isTrue,
      );
      expect(tester.widget<TextField>(searchField).controller?.text, isEmpty);
    }
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
    await Scrollable.ensureVisible(tester.element(select), alignment: 0.5);
    await tester.pump();
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
    expect(find.text('Press shortcut…'), findsOneWidget);

    await tester.tap(find.text('Shortcuts').last);
    await tester.pump();
    expect(find.text('Press shortcut…'), findsNothing);
  });

  testWidgets('data sync page exposes storage providers and their settings', (
    WidgetTester tester,
  ) async {
    githubDeviceFlowClientIdOverride = '';
    var refreshCount = 0;
    settingsRemoteSyncStatusLoaderOverride = (_) async {
      refreshCount++;
      return {};
    };
    cacheGithubSyncSettingsForTesting(NautermPaths.resolve().databasePath, {
      'hasLocalSyncKey': false,
    });
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 820);
    addTearDown(() {
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
      githubDeviceFlowClientIdOverride = null;
      settingsRemoteSyncStatusLoaderOverride = null;
      clearGithubSyncSettingsCacheForTesting();
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
    expect(find.text('Local —', findRichText: true), findsOneWidget);
    expect(find.text('Remote —', findRichText: true), findsOneWidget);
    final syncNow = find.byKey(const ValueKey('settings-sync-now'));
    expect(tester.widget<FilledButton>(syncNow).onPressed, isNull);
    final refresh = find.byTooltip('Refresh Status');
    final beforeRefresh = refreshCount;
    await tester.tap(refresh);
    await tester.pumpAndSettle();
    expect(refreshCount, beforeRefresh + 1);
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
    final authorization = _PendingCloudOAuth();
    settingsCloudOAuthClientFactoryOverride = () => authorization;
    addTearDown(() => settingsCloudOAuthClientFactoryOverride = null);
    await tester.tap(find.text('Connect'));
    await tester.pump();
    expect(find.text('Waiting for browser sign-in…'), findsOneWidget);
    await tester.tap(find.text('Open browser'));
    expect(authorization.browserReopens, 1);
    await tester.tap(find.text('Cancel sign-in'));
    await tester.pump();
    expect(authorization.isCancelled, isTrue);
    expect(find.text('Cancel sign-in'), findsNothing);
    expect(find.text('Add OneDrive'), findsOneWidget);
    expect(find.text('Connect'), findsOneWidget);
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

  testWidgets('sync provider removal asks before changing saved data', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 900);
    final directory = Directory.systemTemp.createTempSync(
      'nauterm-sync-remove-',
    );
    final databasePath = '${directory.path}/settings.sqlite';
    final store = NautermDataStore.openPath(databasePath);
    settingsRemoteSyncStatusLoaderOverride = (_) async => {};
    addTearDown(() {
      store.dispose();
      directory.deleteSync(recursive: true);
      settingsRemoteSyncStatusLoaderOverride = null;
      clearGithubSyncSettingsCacheForTesting();
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
    });
    store.s3SaveConfig(
      endpoint: 'https://legacy.example.com',
      region: 'auto',
      bucket: 'legacy',
      prefix: '',
      filename: 'nauterm-sync.enc',
    );
    store.s3SaveCredentials(accessKeyId: 'access', secretAccessKey: 'secret');
    store.cloudSaveProvider(
      provider: {
        'id': 'sync-removal-example',
        'scheme': 's3',
        'vendor': 'minio',
        'name': 'Lab MinIO',
        'config': {
          'endpoint': 'https://minio.example.com',
          'region': 'auto',
          'bucket': 'backup',
          'filename': 'nauterm-sync.enc',
        },
      },
      credentials: {'access_key_id': 'access', 'secret_access_key': 'secret'},
    );
    cacheGithubSyncSettingsForTesting(databasePath, {
      's3Config': store.s3LoadConfig(),
      'hasS3Credentials': true,
      'cloudProviders': store.cloudListProviders(),
      'hasLocalSyncKey': true,
    });

    Future<void> waitUntil(bool Function() condition, {String? reason}) async {
      final wait = Stopwatch()..start();
      while (!condition() && wait.elapsed < const Duration(seconds: 5)) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 15)),
        );
        await tester.pump();
      }
      expect(condition(), isTrue, reason: reason);
    }

    Future<void> openProvider(String cardKey, Finder whenOpen) async {
      final configure = find.descendant(
        of: find.byKey(ValueKey(cardKey)),
        matching: find.byWidgetPredicate(
          (widget) => widget is IconButton && widget.tooltip == 'Configure',
        ),
      );
      await waitUntil(
        () => tester
            .widgetList<IconButton>(configure)
            .any((button) => button.onPressed != null),
        reason: 'The Configure button for $cardKey should be available.',
      );
      await tester.ensureVisible(configure);
      await tester.pumpAndSettle();
      await tester.tap(configure);
      await waitUntil(() => whenOpen.evaluate().isNotEmpty);
      await tester.ensureVisible(whenOpen);
    }

    await tester.pumpWidget(
      MaterialApp(
        home: SettingsPanel(
          detectExternalEditors: false,
          syncDatabasePath: databasePath,
        ),
      ),
    );
    await tester.tap(find.byKey(const ValueKey('settings-nav-sync')));
    await tester.pumpAndSettle();

    final remove = find.text('Remove from Nauterm');
    await openProvider('settings-cloud-provider-sync-removal-example', remove);
    await tester.tap(remove);
    await tester.pumpAndSettle();
    expect(find.text('Delete Sync Provider'), findsOneWidget);
    expect(store.cloudListProviders(), hasLength(1));
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    await tester.ensureVisible(remove);
    await tester.tap(remove);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Delete'));
    await waitUntil(() => store.cloudListProviders().isEmpty);
    await waitUntil(() => find.text('Lab MinIO').evaluate().isEmpty);

    final disconnect = find.text('Disconnect');
    await openProvider('settings-sync-provider-s3Compatible', disconnect);
    await tester.tap(disconnect);
    await tester.pumpAndSettle();
    expect(find.text('Disconnect Sync Provider'), findsOneWidget);
    expect(store.s3HasCredentials(), isTrue);
    await tester.tap(disconnect);
    await waitUntil(() => !store.s3HasCredentials());
    expect(store.s3LoadConfig()?['bucket'], 'legacy');
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('sync key details confirms forgetting and validates rotation', (
    WidgetTester tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(760, 820);
    settingsRemoteSyncStatusLoaderOverride = (_) async => {
      'active_provider_id': 'cloud:provider-1',
    };
    final firstHistory =
        Completer<({List<Map<String, dynamic>> versions, bool canRestore})>();
    var historyReads = 0;
    settingsSyncHistoryLoaderOverride = (scope, _) async {
      expect(scope, 'cloud:provider-2');
      historyReads++;
      if (historyReads == 1) return firstHistory.future;
      return (
        versions: [
          {'version_id': 'current-archive', 'is_current': true},
          {'version_id': 'previous-archive', 'is_current': false},
        ],
        canRestore: historyReads > 2,
      );
    };
    settingsSyncKeyRotationReviewOverride = (_) async => (
      stamp: 'rotation-review',
      targets: [
        (scope: 'cloud:provider-1', name: 'Lab MinIO'),
        (scope: 'cloud:provider-2', name: 'Archive R2'),
        (scope: 'cloud:provider-3', name: 'Backup B2'),
      ],
    );
    final partialRotation = Completer<SyncKeyRotationResult>();
    var rotationAttempts = 0;
    settingsSyncKeyRotationRunnerOverride =
        (_, stamp, current, next, backupCount, onProgress) async {
          rotationAttempts++;
          expect(current, 'Current-master-key1');
          expect(next, 'New-master-key-value1');
          if (rotationAttempts == 1) {
            expect(stamp, 'rotation-review');
            onProgress((
              scope: 'cloud:provider-1',
              phase: SyncKeyRotationPhase.updated,
            ));
            onProgress((
              scope: 'cloud:provider-2',
              phase: SyncKeyRotationPhase.updating,
            ));
            return partialRotation.future;
          }
          expect(stamp, 'retry-review');
          return (
            completed: [
              'cloud:provider-1',
              'cloud:provider-2',
              'cloud:provider-3',
            ],
            failed: null,
            retryStamp: null,
            hasLocalKey: true,
          );
        };
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
      settingsRemoteSyncStatusLoaderOverride = null;
      settingsSyncHistoryLoaderOverride = null;
      settingsSyncKeyRotationReviewOverride = null;
      settingsSyncKeyRotationRunnerOverride = null;
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
    expect(find.byKey(const ValueKey('settings-sync-now')), findsOneWidget);
    expect(
      find.descendant(of: activeCard, matching: find.text('ACTIVE')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: inactiveCard, matching: find.text('Use')),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: inactiveCard,
        matching: find.byTooltip('Version History'),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(of: inactiveCard, matching: find.byTooltip('Configure')),
      findsOneWidget,
    );
    expect(
      tester.getSize(activeCard).width,
      tester.getSize(inactiveCard).width,
    );
    expect(tester.getSize(activeCard).width, tester.getSize(thirdCard).width);
    expect(
      tester.getTopLeft(activeCard).dy,
      lessThan(tester.getTopLeft(inactiveCard).dy),
    );
    expect(
      tester.getTopLeft(inactiveCard).dy,
      lessThan(tester.getTopLeft(thirdCard).dy),
    );
    final historyButton = find.descendant(
      of: inactiveCard,
      matching: find.byTooltip('Version History'),
    );
    await tester.ensureVisible(historyButton);
    await tester.pumpAndSettle();
    await tester.tap(historyButton);
    await tester.pump();
    expect(find.text('Archive R2 History'), findsOneWidget);
    expect(find.bySemanticsLabel('Loading history…'), findsOneWidget);
    firstHistory.completeError(StateError('provider temporarily unavailable'));
    await tester.pumpAndSettle();
    expect(
      find.text('Bad state: provider temporarily unavailable'),
      findsOneWidget,
    );

    await tester.tap(find.widgetWithText(OutlinedButton, 'Retry'));
    await tester.pumpAndSettle();
    expect(
      find.text(
        'Complete a sync on this device before restoring cloud history.',
      ),
      findsOneWidget,
    );
    final restore = find.widgetWithText(OutlinedButton, 'Restore');
    expect(tester.widget<OutlinedButton>(restore).onPressed, isNull);
    await tester.tap(find.byTooltip('Close'));
    await tester.pumpAndSettle();

    await tester.tap(historyButton);
    await tester.pumpAndSettle();
    expect(tester.widget<OutlinedButton>(restore).onPressed, isNotNull);
    await tester.tap(restore);
    await tester.pumpAndSettle();
    expect(find.text('Restore Cloud Version'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Close'));
    await tester.pumpAndSettle();

    await tester.ensureVisible(find.text('Key'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Key'));
    await tester.pumpAndSettle();
    expect(find.text('CONNECTED PROVIDERS · 3'), findsOneWidget);
    expect(find.text('Forget Master Key'), findsOneWidget);
    await tester.tap(find.text('Forget Master Key'));
    await tester.pumpAndSettle();
    final confirmation = find.byKey(
      const ValueKey('settings-sync-confirm-forget-key'),
    );
    expect(find.text('Forget Master Key?'), findsOneWidget);
    expect(confirmation, findsOneWidget);
    expect(find.byKey(const ValueKey('settings-sync-now')), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(confirmation, findsNothing);
    expect(find.byKey(const ValueKey('settings-sync-now')), findsOneWidget);
    await tester.tap(find.text('Key'));
    await tester.pumpAndSettle();
    expect(find.text('Sync DEK available locally'), findsOneWidget);
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

    final confirmKey = find.descendant(
      of: find.byKey(const ValueKey('settings-sync-new-master-key-confirm')),
      matching: find.byType(EditableText),
    );
    final submit = find.byKey(
      const ValueKey('settings-sync-change-master-key-submit'),
    );
    await tester.enterText(confirmKey, 'New-master-key-value1');
    await tester.ensureVisible(submit);
    await tester.pumpAndSettle();
    await tester.tap(submit);
    await tester.pumpAndSettle();
    expect(find.text('Updated'), findsOneWidget);
    expect(tester.widget<EditableText>(confirmKey).readOnly, isTrue);
    expect(
      tester
          .widget<IconButton>(find.widgetWithIcon(IconButton, LucideIcons.x))
          .onPressed,
      isNull,
    );

    partialRotation.complete((
      completed: ['cloud:provider-1'],
      failed: (scope: 'cloud:provider-2', error: 'temporary upload failure'),
      retryStamp: 'retry-review',
      hasLocalKey: true,
    ));
    await tester.pumpAndSettle();
    expect(
      find.text(
        'The Master Key change did not complete.\n'
        'Archive R2: temporary upload failure',
      ),
      findsOneWidget,
    );
    expect(find.textContaining('Retry with this same pair'), findsOneWidget);
    expect(tester.widget<EditableText>(confirmKey).readOnly, isTrue);
    await tester.ensureVisible(submit);
    await tester.pumpAndSettle();
    await tester.tap(submit);
    await tester.pumpAndSettle();
    expect(rotationAttempts, 2);
    expect(submit, findsNothing);
  });

  testWidgets('restoring a local sync backup requires confirmation', (
    WidgetTester tester,
  ) async {
    final directory = Directory.systemTemp.createTempSync(
      'nauterm_local_backup_confirm_',
    );
    final databasePath = '${directory.path}/settings.sqlite';
    final backupDirectory = Directory('${directory.path}/sync-backups')
      ..createSync();

    void createDatabase(String path, String groupName) {
      final store = NautermDataStore.openPath(path);
      try {
        store.saveGroup(HostGroup(name: groupName));
      } finally {
        store.dispose();
      }
    }

    String currentGroup() {
      final store = NautermDataStore.openPath(databasePath);
      try {
        return store.listGroups().single.name;
      } finally {
        store.dispose();
      }
    }

    createDatabase(databasePath, 'Current group');
    createDatabase(
      '${backupDirectory.path}/nauterm-local-backup-sync-fixture.sqlite',
      'Backed up group',
    );
    cacheGithubSyncSettingsForTesting(databasePath, {'hasLocalSyncKey': true});
    settingsRemoteSyncStatusLoaderOverride = (_) async => {};
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1000, 900);
    addTearDown(() {
      clearGithubSyncSettingsCacheForTesting();
      settingsRemoteSyncStatusLoaderOverride = null;
      tester.view.resetDevicePixelRatio();
      tester.view.resetPhysicalSize();
      directory.deleteSync(recursive: true);
    });

    Future<void> waitFor(Finder finder) async {
      for (
        var attempt = 0;
        attempt < 100 && finder.evaluate().isEmpty;
        attempt++
      ) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 15)),
        );
        await tester.pump();
      }
      expect(finder, findsWidgets);
    }

    await tester.pumpWidget(
      MaterialApp(
        home: SettingsPanel(
          detectExternalEditors: false,
          syncDatabasePath: databasePath,
        ),
      ),
    );
    await tester.pump();
    await tester.tap(find.byKey(const ValueKey('settings-nav-sync')));
    await tester.pumpAndSettle();
    final browse = find.text('Browse');
    await tester.ensureVisible(browse);
    await tester.tap(browse);
    await tester.pumpAndSettle();
    final restore = find.text('Restore');
    final confirmTitle = find.text('Restore Local Backup');
    await waitFor(restore);

    await tester.tap(restore);
    await tester.pumpAndSettle();
    expect(confirmTitle, findsOneWidget);
    expect(currentGroup(), 'Current group');
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(confirmTitle, findsNothing);
    expect(currentGroup(), 'Current group');

    await tester.tap(restore);
    await tester.pumpAndSettle();
    expect(confirmTitle, findsOneWidget);
    await tester.tap(find.widgetWithText(FilledButton, 'Restore'));
    await tester.pumpAndSettle();
    await waitFor(find.text('Local backup restored.'));
    expect(currentGroup(), 'Backed up group');
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

class _PendingCloudOAuth extends CloudOAuthClient {
  final Completer<CloudOAuthCredentials?> _result = Completer();
  int browserReopens = 0;

  @override
  Future<CloudOAuthCredentials?> authorize(
    CloudOAuthVendor vendor, {
    void Function(CloudOAuthPhase)? onPhase,
  }) {
    expect(vendor, CloudOAuthVendor.oneDrive);
    onPhase?.call(CloudOAuthPhase.waitingForBrowser);
    return _result.future;
  }

  @override
  Future<void> reopenBrowser() async {
    browserReopens++;
  }

  @override
  void cancel() {
    super.cancel();
    if (!_result.isCompleted) _result.complete(null);
  }
}
