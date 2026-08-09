import 'dart:io';
import 'dart:ui';

import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/terminal/terminal_open_target.dart';
import 'package:nauterm/terminal/terminal_selection.dart';
import 'package:nauterm/terminal/terminal_theme.dart';
import 'package:nauterm/terminal/terminal_widget.dart';
import 'package:nauterm/ui/nauterm_context_menu.dart';

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  var clipboardText = '';

  setUp(() {
    clipboardText = '';
    terminalAutocompleteEnabled = false;
    terminalScrollbarEnabled = true;
    terminalShortcutConfig = const TerminalShortcutConfig();
    binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      (methodCall) async {
        switch (methodCall.method) {
          case 'Clipboard.setData':
            final arguments = methodCall.arguments as Map<dynamic, dynamic>;
            clipboardText = arguments['text'] as String? ?? '';
          case 'Clipboard.getData':
            return <String, dynamic>{'text': clipboardText};
          case 'Clipboard.hasStrings':
            return <String, dynamic>{'value': clipboardText.isNotEmpty};
        }

        return null;
      },
    );
  });

  tearDown(() {
    terminalAutocompleteEnabled = false;
    terminalScrollbarEnabled = true;
    terminalShortcutConfig = const TerminalShortcutConfig();
    debugDefaultTargetPlatformOverride = null;
    binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      null,
    );
  });

  testWidgets('terminal context menu follows the terminal theme', (
    tester,
  ) async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 8),
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    final theme = TerminalTheme(
      name: 'Context menu test',
      type: TerminalThemeType.dark,
      primary: const TerminalPrimaryColors(
        accent: Color(0xff54c8ff),
        background: Color(0xff101820),
        foreground: Color(0xffdbe7ee),
      ),
      cursor: defaultTerminalTheme.cursor,
      selection: defaultTerminalTheme.selection,
      normal: defaultTerminalTheme.normal,
      bright: defaultTerminalTheme.bright,
    );
    await _pumpTerminal(tester, controller, theme: theme);

    final position = tester.getCenter(
      find.byKey(const ValueKey('terminal-renderer-region')),
    );
    final gesture = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
      buttons: kSecondaryMouseButton,
    );
    addTearDown(gesture.removePointer);
    await gesture.addPointer(location: position);
    await gesture.down(position);
    await gesture.up();
    await tester.pump();

    final menuFinder = find.byWidgetPredicate(
      (widget) => widget is NautermContextMenu,
    );
    expect(menuFinder, findsOneWidget);
    expect(
      find.ancestor(
        of: menuFinder,
        matching: find.byWidgetPredicate(
          (widget) => widget is TweenAnimationBuilder<double>,
        ),
      ),
      findsOneWidget,
    );
    expect(
      find.ancestor(of: menuFinder, matching: find.byType(Transform)),
      findsNothing,
    );
    final menu = tester.widget(menuFinder) as NautermContextMenu;
    final menuActions = menu.entries
        .whereType<NautermContextMenuAction>()
        .toList(growable: false);
    expect(menuActions.map((action) => action.label), isNot(contains('Reset')));
    expect(menuActions.last.label, 'Close');
    expect(menuActions.last.destructive, isTrue);
    expect(
      menu.style?.background,
      Color.lerp(theme.primary.background, theme.primary.foreground, 0.035),
    );
    expect(menu.style?.accent, theme.primary.accent);

    await tester.tap(find.text('Clear'));
    await tester.pump();

    expect(inputs, ['\x0c']);
  });

  testWidgets('terminal context menu reflects platform shortcut settings', (
    tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    terminalShortcutConfig = const TerminalShortcutConfig(
      copy: 'cmd+shift+x',
      paste: '',
      closeTab: 'cmd+alt+w',
      openSettings: '',
    );
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 8),
      onInput: (_) {},
    );
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller);

    await tester.tap(
      find.byKey(const ValueKey('terminal-renderer-region')),
      buttons: kSecondaryMouseButton,
    );
    await tester.pump();

    final menu =
        tester.widget(
              find.byWidgetPredicate((widget) => widget is NautermContextMenu),
            )
            as NautermContextMenu;
    final actions = menu.entries
        .whereType<NautermContextMenuAction<dynamic>>()
        .toList(growable: false);
    String? shortcutFor(String label) =>
        actions.singleWhere((action) => action.label == label).shortcut;

    expect(shortcutFor('Copy'), 'Ctrl+Shift+X');
    expect(shortcutFor('Paste'), isNull);
    expect(shortcutFor('Close'), 'Ctrl+Alt+W');
    expect(shortcutFor('Settings'), isNull);
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('terminal context menu invokes its settings callback', (
    tester,
  ) async {
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 8),
    );
    addTearDown(controller.dispose);
    var settingsRequests = 0;
    await _pumpTerminal(
      tester,
      controller,
      onSettingsRequested: () => settingsRequests++,
    );

    await tester.tap(
      find.byKey(const ValueKey('terminal-renderer-region')),
      buttons: kSecondaryMouseButton,
    );
    await tester.pump();
    await tester.tap(find.text('Settings'));
    await tester.pump();

    expect(settingsRequests, 1);
  });

  testWidgets('terminal context menu does not capture adjacent panels', (
    tester,
  ) async {
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 8),
    );
    addTearDown(controller.dispose);
    var adjacentSecondaryTaps = 0;

    await tester.pumpWidget(
      MaterialApp(
        home: Row(
          children: [
            Expanded(
              child: TerminalView(
                controller: controller,
                config: defaultTerminalConfig,
              ),
            ),
            GestureDetector(
              key: const ValueKey('adjacent-tool-panel'),
              behavior: HitTestBehavior.opaque,
              onSecondaryTapDown: (_) => adjacentSecondaryTaps++,
              child: const SizedBox(width: 240, height: double.infinity),
            ),
          ],
        ),
      ),
    );
    await tester.pump();

    final terminalPosition = tester.getCenter(
      find.byKey(const ValueKey('terminal-renderer-region')),
    );
    final terminalGesture = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
      buttons: kSecondaryMouseButton,
    );
    await terminalGesture.addPointer(location: terminalPosition);
    await terminalGesture.down(terminalPosition);
    await terminalGesture.up();
    await terminalGesture.removePointer();
    await tester.pump();
    final menu = find.byWidgetPredicate(
      (widget) => widget is NautermContextMenu,
    );
    expect(menu, findsOneWidget);

    final terminalRect = tester.getRect(
      find.byKey(const ValueKey('terminal-renderer-region')),
    );
    final secondTerminalPosition = terminalRect.bottomLeft.translate(20, -20);
    final secondTerminalGesture = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
      buttons: kSecondaryMouseButton,
    );
    await secondTerminalGesture.addPointer(location: secondTerminalPosition);
    await secondTerminalGesture.down(secondTerminalPosition);
    await secondTerminalGesture.up();
    await secondTerminalGesture.removePointer();
    await tester.pump();
    expect(menu, findsOneWidget);

    await tester.tap(
      find.byKey(const ValueKey('adjacent-tool-panel')),
      buttons: kSecondaryMouseButton,
    );
    await tester.pump();

    expect(adjacentSecondaryTaps, 1);
    expect(menu, findsNothing);
  });

  testWidgets('scrollbar follows terminal history, theme, and setting', (
    tester,
  ) async {
    final driver = _SnapshotDriver(
      TerminalSnapshot.blank(
        columns: 80,
        rows: 8,
        historyLines: 92,
        displayOffset: 0,
      ),
    );
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);
    final theme = TerminalTheme(
      name: 'Scrollbar test',
      type: TerminalThemeType.dark,
      primary: TerminalPrimaryColors(
        accent: Color(0xff22cc88),
        background: Color(0xff101820),
        foreground: Color(0xffd8e2ea),
      ),
      cursor: TerminalCursorColors(
        cursor: Color(0xffffffff),
        text: Color(0xff000000),
      ),
      selection: TerminalSelectionColors(
        background: Color(0xff224466),
        text: Color(0xffffffff),
      ),
      normal: defaultTerminalTheme.normal,
      bright: defaultTerminalTheme.bright,
    );

    await _pumpTerminal(tester, controller, theme: theme);

    final scrollbar = find.byKey(const ValueKey('terminal-scrollbar-thumb'));
    expect(scrollbar, findsOneWidget);
    expect(
      tester
          .widget<MouseRegion>(
            find.byKey(const ValueKey('terminal-scrollbar-hit-region')),
          )
          .cursor,
      SystemMouseCursors.basic,
    );
    final thumb = tester.widget<AnimatedContainer>(scrollbar);
    final decoration = thumb.decoration! as BoxDecoration;
    expect(decoration.color, theme.primary.foreground.withValues(alpha: 0));

    final thumbRect = tester.getRect(scrollbar);
    final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
    addTearDown(gesture.removePointer);
    await gesture.addPointer(location: Offset.zero);
    await gesture.moveTo(
      tester.getCenter(
        find.byKey(const ValueKey('terminal-scrollbar-hit-region')),
      ),
    );
    await tester.pump(const Duration(milliseconds: 120));
    final hoveredThumb = tester.widget<AnimatedContainer>(scrollbar);
    expect(
      (hoveredThumb.decoration! as BoxDecoration).color,
      theme.primary.foreground.withValues(alpha: 0.62),
    );

    await tester.dragFrom(thumbRect.center, const Offset(0, -80));
    await tester.pump();
    expect(driver.scrolledLines, greaterThan(0));

    await gesture.moveTo(const Offset(20, 20));
    await tester.pump(_terminalScrollbarScrollVisibilityDurationForTest);
    await tester.pump(const Duration(milliseconds: 120));
    final hiddenAfterScroll = tester.widget<AnimatedContainer>(scrollbar);
    expect(
      (hiddenAfterScroll.decoration! as BoxDecoration).color,
      theme.primary.foreground.withValues(alpha: 0),
    );

    controller.scrollLines(1);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 120));
    final scrollingThumb = tester.widget<AnimatedContainer>(scrollbar);
    expect(
      (scrollingThumb.decoration! as BoxDecoration).color,
      theme.primary.foreground.withValues(alpha: 0.62),
    );

    terminalScrollbarEnabled = false;
    terminalConfigNotifier.value++;
    await tester.pump();
    expect(scrollbar, findsNothing);
  });

  testWidgets('terminal wheel wins over an ancestor scroll view', (
    tester,
  ) async {
    final driver = _SnapshotDriver(
      TerminalSnapshot.blank(
        columns: 80,
        rows: 8,
        historyLines: 92,
        displayOffset: 40,
      ),
    );
    final controller = TerminalController(driver: driver);
    final outerController = ScrollController();
    addTearDown(controller.dispose);
    addTearDown(outerController.dispose);

    await tester.pumpWidget(
      MaterialApp(
        home: SizedBox(
          width: 800,
          height: 220,
          child: SingleChildScrollView(
            controller: outerController,
            child: Column(
              children: [
                SizedBox(
                  height: 180,
                  child: TerminalView(
                    controller: controller,
                    config: defaultTerminalConfig,
                    padding: EdgeInsets.zero,
                  ),
                ),
                const SizedBox(height: 500),
              ],
            ),
          ),
        ),
      ),
    );
    await tester.pump();

    await tester.sendEventToBinding(
      PointerScrollEvent(
        position: tester.getCenter(
          find.byKey(const ValueKey('terminal-renderer-region')),
        ),
        scrollDelta: const Offset(0, 60),
      ),
    );
    await tester.pump();

    expect(driver.scrolledLines, lessThan(0));
    expect(outerController.offset, 0);
  });

  testWidgets('terminal wheel reports the natural scroll direction to apps', (
    tester,
  ) async {
    final inputs = <String>[];
    final driver = _SnapshotDriver(
      TerminalSnapshot.blank(
        columns: 80,
        rows: 8,
        keyboardMode: const TerminalKeyboardMode(
          mouseReportClick: true,
          sgrMouse: true,
        ),
      ),
    );
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller);

    await tester.sendEventToBinding(
      PointerScrollEvent(
        position: tester.getCenter(
          find.byKey(const ValueKey('terminal-renderer-region')),
        ),
        scrollDelta: const Offset(0, 20),
      ),
    );
    await tester.pump();

    expect(inputs, hasLength(1));
    expect(inputs.single, startsWith('\x1b[<65;'));
  });

  testWidgets('mouse selection copies selected terminal text', (tester) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    await _pumpTerminal(tester, controller);

    controller.write('foo_bar-baz git:main*');
    await tester.pump();

    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await gesture.down(_cellCenter(metrics, column: 0, row: 0));
    await gesture.moveTo(_cellCenter(metrics, column: 10, row: 0));
    await gesture.up();
    await tester.pump();

    await _sendShortcut(tester, LogicalKeyboardKey.keyC);
    await tester.pump();

    final copied = await Clipboard.getData(Clipboard.kTextPlain);
    expect(copied?.text, 'foo_bar-baz');
  });

  testWidgets('single click toggles a prompt and output command block', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller, autofocusTerminal: true);
    controller.write(
      'user@host:~\$ ls\r\none\r\ntwo\r\nuser@host:~\$ pwd\r\n/home',
    );
    await tester.pump();

    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final outputPosition = _cellCenter(metrics, column: 1, row: 1);
    await tester.tapAt(outputPosition, kind: PointerDeviceKind.mouse);
    await tester.pump();

    expect(_terminalPainter(tester).selection, isNull);
    expect(
      _terminalPainter(tester).commandBlockSelection,
      TerminalSelection(start: 0, end: controller.snapshot.columns * 3),
    );

    await tester.pump(const Duration(milliseconds: 550));
    await tester.tapAt(outputPosition, kind: PointerDeviceKind.mouse);
    await tester.pump();
    expect(_terminalPainter(tester).commandBlockSelection, isNull);
  });

  testWidgets('click returning from composer only focuses the terminal', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller, autofocusTerminal: true);
    controller.write('user@host:~\$ ls\r\noutput');
    await tester.pump();

    final composer = find.byType(TextField).first;
    await tester.tap(composer);
    await tester.pump();
    expect(tester.widget<TextField>(composer).focusNode?.hasFocus, isTrue);

    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    await tester.tapAt(
      _cellCenter(metrics, column: 2, row: 1),
      kind: PointerDeviceKind.mouse,
    );
    await tester.pump();

    expect(_terminalPainter(tester).commandBlockSelection, isNull);
  });

  testWidgets('double click keeps word selection above command blocks', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller, autofocusTerminal: true);
    controller.write('user@host:~\$ echo\r\noutput');
    await tester.pump();

    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final position = _cellCenter(metrics, column: 2, row: 1);
    await tester.tapAt(position, kind: PointerDeviceKind.mouse);
    await tester.tapAt(position, kind: PointerDeviceKind.mouse);
    await tester.pump();

    expect(_terminalPainter(tester).commandBlockSelection, isNull);
    expect(
      terminalSelectedText(
        controller.snapshot,
        _terminalPainter(tester).selection,
      ),
      'output',
    );
  });

  testWidgets('triple click keeps line selection above command blocks', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller, autofocusTerminal: true);
    controller.write('user@host:~\$ echo\r\nfirst second');
    await tester.pump();

    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final position = _cellCenter(metrics, column: 7, row: 1);
    await tester.tapAt(position, kind: PointerDeviceKind.mouse);
    await tester.tapAt(position, kind: PointerDeviceKind.mouse);
    await tester.tapAt(position, kind: PointerDeviceKind.mouse);
    await tester.pump();

    expect(_terminalPainter(tester).commandBlockSelection, isNull);
    expect(
      terminalSelectedText(
        controller.snapshot,
        _terminalPainter(tester).selection,
      ),
      'first second',
    );
  });

  testWidgets('escape clears a selected command block before terminal input', (
    tester,
  ) async {
    final inputs = <String>[];
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller, autofocusTerminal: true);
    controller.write('user@host:~\$ ls\r\noutput');
    await tester.pump();

    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    await tester.tapAt(
      _cellCenter(metrics, column: 2, row: 1),
      kind: PointerDeviceKind.mouse,
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();

    expect(_terminalPainter(tester).commandBlockSelection, isNull);
    expect(inputs, isEmpty);
  });

  testWidgets('command click opens a local terminal path', (tester) async {
    final opened = <TerminalOpenTarget>[];
    final fileName =
        'nauterm-open-target-${DateTime.now().microsecondsSinceEpoch}.txt';
    final file = File('/tmp/$fileName')..writeAsStringSync('report');
    addTearDown(() => file.deleteSync());
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller, onOpenTarget: opened.add);
    controller.write('user@host:/tmp \$ ls\r\n$fileName');
    await tester.pump();

    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final position = _cellCenter(metrics, column: 3, row: 1);
    final openTargetModifier = Platform.isMacOS
        ? LogicalKeyboardKey.metaLeft
        : LogicalKeyboardKey.controlLeft;
    await tester.sendKeyDownEvent(openTargetModifier);
    final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
    addTearDown(gesture.removePointer);
    await gesture.addPointer(location: position);
    await gesture.moveTo(position.translate(1, 0));
    await tester.pump();
    expect(_terminalPainter(tester).openTargetSelection, isNotNull);
    await gesture.down(position);
    await gesture.up();
    await tester.sendKeyUpEvent(openTargetModifier);
    await tester.pump();

    expect(opened, hasLength(1));
    expect(opened.single.localPath, isTrue);
    expect(opened.single.uri.toFilePath(), file.path);
    expect(_terminalPainter(tester).commandBlockSelection, isNull);
  });

  testWidgets('drag selection beyond the viewport auto-scrolls terminal', (
    tester,
  ) async {
    final driver = _SnapshotDriver(
      TerminalSnapshot.blank(
        columns: 80,
        rows: 8,
        historyLines: 40,
        displayOffset: 10,
      ),
    );
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller);

    final renderer = find.byKey(const ValueKey('terminal-renderer-region'));
    final rendererRect = tester.getRect(renderer);
    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
    addTearDown(gesture.removePointer);
    await gesture.down(_cellCenter(metrics, column: 1, row: 2));
    await gesture.moveTo(
      Offset(rendererRect.center.dx, rendererRect.bottom + 30),
    );

    await tester.pump(const Duration(milliseconds: 160));

    expect(driver.scrolledLines, lessThan(0));
    final extendedSelection = _terminalPainter(tester).selection;
    expect(extendedSelection, isNotNull);
    expect(
      extendedSelection!.end - extendedSelection.start,
      greaterThan(driver.snapshot.rows * driver.snapshot.columns),
    );
    final scrolledLines = driver.scrolledLines;

    await gesture.moveTo(rendererRect.center);
    await tester.pump(const Duration(milliseconds: 160));
    expect(driver.scrolledLines, scrolledLines);

    await gesture.up();
  });

  testWidgets(
    'scrollbar scrolling extends an active selection past one screen',
    (tester) async {
      final driver = _SnapshotDriver(
        TerminalSnapshot.blank(
          columns: 80,
          rows: 8,
          historyLines: 80,
          displayOffset: 0,
        ),
      );
      final controller = TerminalController(driver: driver);
      addTearDown(controller.dispose);
      await _pumpTerminal(tester, controller);

      final metrics = TerminalMetrics.measure(
        defaultTerminalConfig.font.textStyle(),
      );
      final selectionGesture = await tester.createGesture(
        kind: PointerDeviceKind.mouse,
      );
      addTearDown(selectionGesture.removePointer);
      await selectionGesture.down(_cellCenter(metrics, column: 1, row: 2));
      await selectionGesture.moveTo(_cellCenter(metrics, column: 8, row: 3));

      final scrollbar = find.byKey(
        const ValueKey('terminal-scrollbar-hit-region'),
      );
      final scrollbarRect = tester.getRect(scrollbar);
      final scrollbarGesture = await tester.createGesture(
        kind: PointerDeviceKind.touch,
      );
      addTearDown(scrollbarGesture.removePointer);
      await scrollbarGesture.down(
        Offset(scrollbarRect.center.dx, scrollbarRect.top + 1),
      );
      await scrollbarGesture.up();
      await tester.pump();

      expect(driver.snapshot.displayOffset, driver.snapshot.historyLines);
      final selection = _terminalPainter(tester).selection;
      expect(selection, isNotNull);
      expect(
        selection!.end - selection.start,
        greaterThan(driver.snapshot.rows * driver.snapshot.columns),
      );

      await selectionGesture.up();
    },
  );

  testWidgets('wheel scrolling preserves an existing terminal selection', (
    tester,
  ) async {
    final driver = _SnapshotDriver(
      TerminalSnapshot.blank(
        columns: 80,
        rows: 8,
        historyLines: 40,
        displayOffset: 10,
      ),
    );
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller);

    final metrics = TerminalMetrics.measure(
      defaultTerminalConfig.font.textStyle(),
    );
    final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await gesture.down(_cellCenter(metrics, column: 1, row: 2));
    await gesture.moveTo(_cellCenter(metrics, column: 8, row: 3));
    await gesture.up();
    await tester.pump();
    final selectionBefore = _terminalPainter(tester).selection;
    expect(selectionBefore, isNotNull);

    await tester.sendEventToBinding(
      PointerScrollEvent(
        position: tester.getCenter(
          find.byKey(const ValueKey('terminal-renderer-region')),
        ),
        scrollDelta: const Offset(0, -40),
      ),
    );
    await tester.pump();

    expect(driver.snapshot.displayOffset, greaterThan(10));
    expect(_terminalPainter(tester).selection, selectionBefore);
  });

  testWidgets('select all shortcut copies visible terminal text', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    await _pumpTerminal(tester, controller);

    controller.write('alpha\r\nbeta');
    await tester.pump();

    await _sendShortcut(tester, LogicalKeyboardKey.keyA);
    await tester.pump();
    await _sendShortcut(tester, LogicalKeyboardKey.keyC);
    await tester.pump();

    final copied = await Clipboard.getData(Clipboard.kTextPlain);
    expect(copied?.text, 'alpha\nbeta');
  });

  testWidgets('select all copies terminal scrollback as well as the screen', (
    tester,
  ) async {
    final driver = _SnapshotDriver(
      TerminalSnapshot.blank(columns: 80, rows: 8, historyLines: 20),
      selectionTextValue: 'history one\nhistory two\nvisible',
    );
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller);

    await _sendShortcut(tester, LogicalKeyboardKey.keyA);
    await tester.pump();
    await _sendShortcut(tester, LogicalKeyboardKey.keyC);
    await tester.pump();

    final copied = await Clipboard.getData(Clipboard.kTextPlain);
    expect(copied?.text, 'history one\nhistory two\nvisible');
    expect(
      _terminalPainter(tester).selection,
      TerminalSelection.all(driver.snapshot),
    );
  });

  testWidgets('paste shortcut sends clipboard text to terminal input', (
    tester,
  ) async {
    final inputs = <String>[];
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    await _pumpTerminal(tester, controller);

    await Clipboard.setData(const ClipboardData(text: 'paste\nok'));

    await _sendShortcut(tester, LogicalKeyboardKey.keyV);
    await tester.pump();

    expect(inputs, ['paste\rok']);
  });

  testWidgets('paste shortcut wraps text when bracketed paste is active', (
    tester,
  ) async {
    final inputs = <String>[];
    final driver = _SnapshotDriver(
      TerminalSnapshot.blank(
        columns: 80,
        rows: 8,
        keyboardMode: const TerminalKeyboardMode(bracketedPaste: true),
      ),
    );
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    await _pumpTerminal(tester, controller);

    await Clipboard.setData(const ClipboardData(text: 'paste\nok'));

    await _sendShortcut(tester, LogicalKeyboardKey.keyV);
    await tester.pump();

    expect(inputs, ['\x1b[200~paste\rok\x1b[201~']);
  });

  testWidgets('search field keeps focus and accepts text input', (
    tester,
  ) async {
    final controller = TerminalController(
      driver: _SnapshotDriver(TerminalSnapshot.blank(columns: 80, rows: 8)),
    );
    addTearDown(controller.dispose);

    await _pumpTerminal(tester, controller);

    await _sendShortcut(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await tester.pump();

    final searchField = find.byKey(const ValueKey('terminal-search-field'));
    expect(searchField, findsOneWidget);
    final fieldFocusNode = tester.widget<EditableText>(searchField).focusNode;

    await tester.tap(searchField);
    await tester.pump();
    expect(tester.testTextInput.isVisible, isTrue);

    tester.testTextInput.updateEditingValue(
      const TextEditingValue(
        text: 'n',
        selection: TextSelection.collapsed(offset: 1),
      ),
    );
    await tester.pump();
    expect(fieldFocusNode.hasFocus, isTrue);

    tester.testTextInput.updateEditingValue(
      const TextEditingValue(
        text: 'needle',
        selection: TextSelection.collapsed(offset: 6),
      ),
    );
    await tester.pump();

    final field = tester.widget<EditableText>(searchField);
    expect(field.controller.text, 'needle');
    expect(fieldFocusNode.hasFocus, isTrue);
    expect(field.expands, isFalse);
    expect(field.maxLines, 1);
    expect(field.style.height, 1);
    expect(field.style.leadingDistribution, TextLeadingDistribution.even);
    expect(field.strutStyle.forceStrutHeight, isTrue);
  });

  testWidgets(
    'search overlay follows the terminal theme and uses centered input',
    (tester) async {
      final controller = TerminalController(
        driver: _SnapshotDriver(TerminalSnapshot.blank(columns: 80, rows: 8)),
      );
      addTearDown(controller.dispose);

      await _pumpTerminal(tester, controller, theme: nysaDarkTerminalTheme);
      await _sendShortcut(tester, LogicalKeyboardKey.keyF);
      await tester.pump();
      await tester.pump();

      final overlay = find.byKey(const ValueKey('terminal-search-overlay'));
      final searchField = find.byKey(const ValueKey('terminal-search-field'));
      final hint = find.byKey(const ValueKey('terminal-search-placeholder'));
      expect(overlay, findsOneWidget);
      expect(searchField, findsOneWidget);
      expect(hint, findsOneWidget);

      final surface = tester
          .widgetList<Container>(
            find.descendant(of: overlay, matching: find.byType(Container)),
          )
          .singleWhere((container) => container.decoration is BoxDecoration);
      final decoration = surface.decoration! as BoxDecoration;
      final foreground = nysaDarkTerminalTheme.primary.foreground;
      expect(
        decoration.color,
        Color.lerp(nysaDarkTerminalTheme.primary.background, foreground, 0.045),
      );
      expect(
        (decoration.border! as Border).top.color,
        foreground.withValues(alpha: 0.18),
      );

      final field = tester.widget<EditableText>(searchField);
      expect(field.style.color, foreground);
      expect(field.cursorColor, nysaDarkTerminalTheme.primary.accent);
      expect(field.cursorHeight, 13);
      final placeholder = tester.widget<Text>(hint);
      expect(placeholder.style?.color, foreground.withValues(alpha: 0.48));
      expect(
        tester.getCenter(hint).dy,
        closeTo(tester.getCenter(find.byIcon(Icons.search_rounded)).dy, 0.1),
      );
      expect(
        tester.getCenter(searchField).dy,
        closeTo(tester.getCenter(find.byIcon(Icons.search_rounded)).dy, 0.1),
      );

      RenderEditable? renderEditable;
      void findRenderEditable(RenderObject renderObject) {
        if (renderObject is RenderEditable) {
          renderEditable = renderObject;
          return;
        }
        renderObject.visitChildren(findRenderEditable);
      }

      findRenderEditable(tester.renderObject(searchField));
      expect(renderEditable, isNotNull);
      final caretRect = renderEditable!.getLocalRectForCaret(
        const TextPosition(offset: 0),
      );
      final caretCenter = renderEditable!.localToGlobal(caretRect.center).dy;
      expect(caretCenter, closeTo(tester.getCenter(hint).dy, 0.25));

      await tester.enterText(searchField, 'not-present');
      await tester.pump();
      final previousButton = tester.widget<IconButton>(
        find.widgetWithIcon(IconButton, Icons.keyboard_arrow_up_rounded),
      );
      final nextButton = tester.widget<IconButton>(
        find.widgetWithIcon(IconButton, Icons.keyboard_arrow_down_rounded),
      );
      expect(previousButton.onPressed, isNull);
      expect(nextButton.onPressed, isNull);
    },
  );

  testWidgets('macOS Control F is sent to the terminal instead of searching', (
    tester,
  ) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    final inputs = <String>[];
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 8),
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller);

    await _sendModifiedKey(
      tester,
      LogicalKeyboardKey.keyF,
      modifier: LogicalKeyboardKey.controlLeft,
    );
    await tester.pump();

    expect(inputs, ['\x06']);
    expect(find.byKey(const ValueKey('terminal-search-field')), findsNothing);
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('Windows search requires Control Shift F', (tester) async {
    debugDefaultTargetPlatformOverride = TargetPlatform.windows;
    final inputs = <String>[];
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 8),
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller);

    await _sendModifiedKey(
      tester,
      LogicalKeyboardKey.keyF,
      modifier: LogicalKeyboardKey.controlLeft,
    );
    await tester.pump();
    expect(inputs, ['\x06']);
    expect(find.byKey(const ValueKey('terminal-search-field')), findsNothing);

    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.keyF);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.keyF);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pump();

    expect(find.byKey(const ValueKey('terminal-search-field')), findsOneWidget);
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('ime committed text is sent to terminal input', (tester) async {
    final inputs = <String>[];
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    await _pumpTerminal(tester, controller);

    await tester.tap(find.byType(TerminalView));
    await tester.pump();

    expect(tester.testTextInput.hasAnyClients, isTrue);

    tester.testTextInput.updateEditingValue(
      const TextEditingValue(
        text: '你好',
        selection: TextSelection.collapsed(offset: 2),
      ),
    );
    await tester.pump();

    expect(inputs, ['你好']);
    expect(tester.testTextInput.editingState?['text'], isEmpty);
  });

  testWidgets('ime duplicate committed values are ignored during reset', (
    tester,
  ) async {
    final inputs = <String>[];
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    await _pumpTerminal(tester, controller);

    await tester.tap(find.byType(TerminalView));
    await tester.pump();

    const committedValue = TextEditingValue(
      text: '你好',
      selection: TextSelection.collapsed(offset: 2),
    );
    tester.testTextInput.updateEditingValue(committedValue);
    await tester.pump();
    tester.testTextInput.updateEditingValue(committedValue);
    await tester.pump();
    tester.testTextInput.updateEditingValue(committedValue);
    await tester.pump();

    expect(inputs, ['你好']);
    expect(tester.testTextInput.editingState?['text'], isEmpty);
  });

  testWidgets('ime composing text waits until commit', (tester) async {
    final inputs = <String>[];
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    await _pumpTerminal(tester, controller);

    await tester.tap(find.byType(TerminalView));
    await tester.pump();

    tester.testTextInput.updateEditingValue(
      const TextEditingValue(
        text: 'ni',
        selection: TextSelection.collapsed(offset: 2),
        composing: TextRange(start: 0, end: 2),
      ),
    );
    await tester.pump();

    expect(inputs, isEmpty);
    expect(_terminalPainter(tester).composingText, 'ni');

    tester.testTextInput.updateEditingValue(
      const TextEditingValue(
        text: '你',
        selection: TextSelection.collapsed(offset: 1),
      ),
    );
    await tester.pump();

    expect(inputs, ['你']);
    expect(_terminalPainter(tester).composingText, isNull);
  });

  testWidgets('ime composing text handles backspace before terminal input', (
    tester,
  ) async {
    final inputs = <String>[];
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    await _pumpTerminal(tester, controller);

    await tester.tap(find.byType(TerminalView));
    await tester.pump();

    tester.testTextInput.updateEditingValue(
      const TextEditingValue(
        text: 'ni',
        selection: TextSelection.collapsed(offset: 2),
        composing: TextRange(start: 0, end: 2),
      ),
    );
    await tester.pump();

    await tester.sendKeyDownEvent(LogicalKeyboardKey.backspace);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.backspace);
    await tester.pump();

    expect(inputs, isEmpty);

    tester.testTextInput.updateEditingValue(
      const TextEditingValue(
        text: 'n',
        selection: TextSelection.collapsed(offset: 1),
        composing: TextRange(start: 0, end: 1),
      ),
    );
    await tester.pump();

    expect(inputs, isEmpty);
  });

  testWidgets('control shortcuts are sent to the terminal', (tester) async {
    final inputs = <String>[];
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    await _pumpTerminal(tester, controller);

    await _sendModifiedKey(
      tester,
      LogicalKeyboardKey.keyC,
      modifier: LogicalKeyboardKey.controlLeft,
    );
    await tester.pump();

    expect(inputs, ['\x03']);
  });

  testWidgets('composer is centered as a floating surface', (tester) async {
    tester.view.physicalSize = const Size(1200, 400);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller);

    final terminal = tester.getRect(find.byType(TerminalView));
    final composer = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-surface')),
    );
    final renderer = tester.getRect(
      find.byKey(const ValueKey('terminal-renderer-region')),
    );

    expect(composer.width, 960);
    expect(composer.height, lessThanOrEqualTo(48));
    expect(composer.center.dx, terminal.center.dx);
    expect(terminal.bottom - composer.bottom, 8);
    expect(composer.left, greaterThan(terminal.left));
    expect(composer.right, lessThan(terminal.right));
    expect(renderer.bottom, closeTo(composer.top, 0.01));
    final controlsRegion = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-controls-region')),
    );
    final sendButton = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-send')),
    );
    expect(controlsRegion.height, greaterThanOrEqualTo(sendButton.height));
    expect(controlsRegion.top, lessThanOrEqualTo(sendButton.top));
    expect(controlsRegion.bottom, greaterThanOrEqualTo(sendButton.bottom));
    BoxDecoration composerDecoration() {
      final composerContainer = tester
          .widgetList<Container>(
            find.descendant(
              of: find.byKey(const ValueKey('terminal-composer-surface')),
              matching: find.byType(Container),
            ),
          )
          .firstWhere(
            (container) =>
                container.decoration is BoxDecoration &&
                (container.decoration! as BoxDecoration)
                        .boxShadow
                        ?.isNotEmpty ==
                    true,
          );
      return composerContainer.decoration! as BoxDecoration;
    }

    final decoration = composerDecoration();
    expect(decoration.color, defaultTerminalTheme.primary.background);
    final border = decoration.border! as Border;
    expect(border.top.color.a, closeTo(0.10, 0.01));
    expect(decoration.boxShadow, isNotEmpty);

    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    addTearDown(mouse.removePointer);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(composer.center);
    await tester.pump();
    final hoverBorder = composerDecoration().border! as Border;
    expect(hoverBorder.top.color.a, closeTo(0.10, 0.01));

    await mouse.moveTo(Offset.zero);
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey('terminal-composer-surface')),
        matching: find.byType(TextField),
      ),
    );
    await tester.pump();
    final activeBorder = composerDecoration().border! as Border;
    expect(
      activeBorder.top.color,
      defaultTerminalTheme.primary.accent.withValues(alpha: 0.78),
    );
  });

  testWidgets('expanded composer overlays without resizing the terminal', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(800, 320);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 8),
    );
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller);

    final terminal = tester.getRect(find.byType(TerminalView));
    await tester.enterText(find.byType(TextField), 'echo focused');
    await tester.pump();
    final rendererBefore = tester.getRect(
      find.byKey(const ValueKey('terminal-renderer-region')),
    );
    final composerBefore = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-surface')),
    );
    final collapseBefore = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-expand')),
    );
    final sendBefore = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-send')),
    );
    expect(collapseBefore.center.dy, closeTo(sendBefore.center.dy, 0.01));
    expect(collapseBefore.right, closeTo(sendBefore.left - 2, 0.01));
    expect(sendBefore.bottom, closeTo(composerBefore.bottom - 7, 0.01));
    await tester.tap(find.byTooltip('Expand Composer'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 90));

    final composerDuringExpansion = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-surface')),
    );
    expect(composerDuringExpansion.top, lessThan(composerBefore.top));
    expect(composerDuringExpansion.top, greaterThan(terminal.top + 44));
    final collapseDuringExpansion = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-expand')),
    );
    final sendDuringExpansion = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-send')),
    );
    expect(collapseDuringExpansion.center.dy, lessThan(sendBefore.center.dy));
    expect(
      collapseDuringExpansion.center.dy,
      lessThan(sendDuringExpansion.center.dy),
    );
    expect(
      sendDuringExpansion.bottom,
      closeTo(composerDuringExpansion.bottom - 7, 0.01),
    );
    expect(
      tester.getRect(find.byKey(const ValueKey('terminal-renderer-region'))),
      rendererBefore,
    );
    expect(tester.takeException(), isNull);

    await tester.pumpAndSettle();

    final rendererAfter = tester.getRect(
      find.byKey(const ValueKey('terminal-renderer-region')),
    );
    final expanded = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-surface')),
    );
    final field = tester.widget<TextField>(find.byType(TextField));

    expect(rendererAfter, rendererBefore);
    expect(expanded.left, terminal.left + 12);
    expect(expanded.top, terminal.top + 44);
    expect(expanded.right, terminal.right - 12);
    expect(expanded.bottom, terminal.bottom - 12);
    expect(field.expands, isTrue);
    expect(find.byTooltip('Collapse Composer'), findsOneWidget);
    final collapseExpanded = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-expand')),
    );
    final sendExpanded = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-send')),
    );
    expect(collapseExpanded.top, closeTo(expanded.top + 7, 0.01));
    expect(collapseExpanded.right, closeTo(expanded.right - 8, 0.01));
    expect(sendExpanded.bottom, closeTo(expanded.bottom - 7, 0.01));
    expect(sendExpanded.right, closeTo(expanded.right - 8, 0.01));

    await tester.tap(find.byTooltip('Collapse Composer'));
    await tester.pumpAndSettle();

    expect(
      tester.getRect(find.byKey(const ValueKey('terminal-renderer-region'))),
      rendererBefore,
    );
    expect(tester.widget<TextField>(find.byType(TextField)).expands, isFalse);
  });

  testWidgets('multiline composer does not resize the terminal', (
    tester,
  ) async {
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 8),
    );
    addTearDown(controller.dispose);
    await _pumpTerminal(tester, controller);

    final rendererBefore = tester.getRect(
      find.byKey(const ValueKey('terminal-renderer-region')),
    );
    final composerBefore = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-surface')),
    );

    await tester.enterText(find.byType(TextField), 'one\ntwo\nthree');
    await tester.pump();

    final rendererAfter = tester.getRect(
      find.byKey(const ValueKey('terminal-renderer-region')),
    );
    final composerAfter = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-surface')),
    );
    expect(rendererAfter, rendererBefore);
    expect(composerAfter.height, greaterThan(composerBefore.height));
    expect(composerAfter.top, lessThan(composerBefore.top));
    final expandButtonAfter = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-expand')),
    );
    final sendButtonAfter = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-send')),
    );
    expect(expandButtonAfter.bottom, closeTo(composerAfter.bottom - 7, 0.01));
    expect(sendButtonAfter.bottom, closeTo(composerAfter.bottom - 7, 0.01));
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller?.text,
      'one\ntwo\nthree',
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('composer toggle only affects the current terminal split', (
    tester,
  ) async {
    final leftController = TerminalController(
      driver: MemoryTerminalDriver(columns: 40, rows: 8),
    );
    final rightController = TerminalController(
      driver: MemoryTerminalDriver(columns: 40, rows: 8),
    );
    addTearDown(leftController.dispose);
    addTearDown(rightController.dispose);
    const toolbar = TerminalViewTabToolbar(
      tabs: [TerminalViewTabToolbarTab(title: 'Terminal', selected: true)],
    );

    await tester.pumpWidget(
      MaterialApp(
        home: Row(
          children: [
            Expanded(
              child: KeyedSubtree(
                key: const ValueKey('left-terminal-split'),
                child: TerminalView(
                  controller: leftController,
                  config: defaultTerminalConfig,
                  tabToolbar: toolbar,
                ),
              ),
            ),
            Expanded(
              child: KeyedSubtree(
                key: const ValueKey('right-terminal-split'),
                child: TerminalView(
                  controller: rightController,
                  config: defaultTerminalConfig,
                  tabToolbar: toolbar,
                ),
              ),
            ),
          ],
        ),
      ),
    );
    await tester.pump();

    final leftSplit = find.byKey(const ValueKey('left-terminal-split'));
    final rightSplit = find.byKey(const ValueKey('right-terminal-split'));
    expect(
      find.descendant(
        of: leftSplit,
        matching: find.byKey(const ValueKey('terminal-composer-surface')),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: rightSplit,
        matching: find.byKey(const ValueKey('terminal-composer-surface')),
      ),
      findsOneWidget,
    );

    await tester.tap(
      find.descendant(of: leftSplit, matching: find.byTooltip('Hide Composer')),
    );
    await tester.pump();

    expect(
      find.descendant(
        of: leftSplit,
        matching: find.byKey(const ValueKey('terminal-composer-surface')),
      ),
      findsNothing,
    );
    expect(
      find.descendant(
        of: rightSplit,
        matching: find.byKey(const ValueKey('terminal-composer-surface')),
      ),
      findsOneWidget,
    );
  });

  testWidgets('session title follows its terminal title listenable', (
    tester,
  ) async {
    final controller = TerminalController(
      driver: MemoryTerminalDriver(columns: 80, rows: 8),
    );
    final sessionTitle = ValueNotifier<String>('initial shell title');
    addTearDown(controller.dispose);
    addTearDown(sessionTitle.dispose);

    await tester.pumpWidget(
      MaterialApp(
        home: TerminalView(
          controller: controller,
          config: defaultTerminalConfig,
          tabToolbar: TerminalViewTabToolbar(
            tabs: const [
              TerminalViewTabToolbarTab(title: 'Saved host', selected: true),
            ],
            sessionTitle: () => sessionTitle.value,
            sessionTitleListenable: sessionTitle,
          ),
        ),
      ),
    );

    expect(find.text('initial shell title'), findsOneWidget);
    expect(find.text('Saved host'), findsNothing);

    sessionTitle.value = 'updated shell title';
    await tester.pump();

    expect(find.text('initial shell title'), findsNothing);
    expect(find.text('updated shell title'), findsOneWidget);
  });

  testWidgets('composer accepts suggestion with second tab', (tester) async {
    final inputs = <String>[];
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    await _pumpTerminal(
      tester,
      controller,
      composerSuggestions: const ['git status', 'grep TODO'],
    );

    await tester.enterText(find.byType(TextField), 'git');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.testTextInput.receiveAction(TextInputAction.send);
    await tester.pump();
    expect(inputs.last, 'git\r');

    await tester.enterText(find.byType(TextField), 'git');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.testTextInput.receiveAction(TextInputAction.send);
    await tester.pump();

    expect(inputs.last, 'git status\r');
  });

  testWidgets('composer autocomplete is disabled by configuration', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    var resolverCalls = 0;
    addTearDown(controller.dispose);
    await _pumpTerminal(
      tester,
      controller,
      autocompleteEnabled: false,
      composerSuggestions: const ['git status'],
      composerSuggestionResolver: (input, limit) {
        resolverCalls++;
        return const ['git push'];
      },
    );

    await tester.enterText(find.byType(TextField), 'git');
    await tester.pump();
    await tester.pump();

    expect(find.text('git status'), findsNothing);
    expect(find.text('git push'), findsNothing);
    expect(resolverCalls, 0);
  });

  testWidgets('composer autocomplete follows runtime setting updates', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);
    await _pumpTerminal(
      tester,
      controller,
      composerSuggestions: const ['git status'],
    );

    await tester.enterText(find.byType(TextField), 'git');
    await tester.pump();
    await tester.pump();
    expect(find.text('git status'), findsOneWidget);

    terminalAutocompleteEnabled = false;
    terminalConfigNotifier.value++;
    await tester.pump();
    await tester.pump();

    expect(find.text('git status'), findsNothing);
  });

  testWidgets('composer navigates shell history with arrows', (tester) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    await _pumpTerminal(
      tester,
      controller,
      composerHistory: const ['git status', 'ls -la'],
      composerSuggestions: const ['from snippet'],
    );

    await tester.enterText(find.byType(TextField), 'draft');
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pump();
    expect(find.byType(TextField), findsOneWidget);
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller?.text,
      'git status',
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pump();
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller?.text,
      'ls -la',
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller?.text,
      'git status',
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller?.text,
      'draft',
    );
  });

  testWidgets('composer arrows keep history priority over completion', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    await _pumpTerminal(
      tester,
      controller,
      composerHistory: const ['git log'],
      composerSuggestions: const ['git status', 'git push'],
    );

    await tester.enterText(find.byType(TextField), 'git');
    await tester.pump();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pump();

    expect(
      tester.widget<TextField>(find.byType(TextField)).controller?.text,
      'git log',
    );
  });

  testWidgets('composer shows completion without preselected candidate', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    await _pumpTerminal(
      tester,
      controller,
      composerSuggestions: const ['git status'],
    );

    await tester.enterText(find.byType(TextField), 'git');
    await tester.pump();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();

    expect(
      tester.widget<TextField>(find.byType(TextField)).controller?.text,
      'git',
    );
  });

  testWidgets('composer arrows navigate completion after tab activation', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    await _pumpTerminal(
      tester,
      controller,
      composerHistory: const ['git log'],
      composerSuggestions: const ['git status', 'git push'],
    );

    await tester.enterText(find.byType(TextField), 'git');
    await tester.pump();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();

    expect(
      tester.widget<TextField>(find.byType(TextField)).controller?.text,
      'git push',
    );
  });

  testWidgets('composer escape suppresses completion until tab reactivates', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    await _pumpTerminal(
      tester,
      controller,
      composerSuggestions: const ['git status'],
    );

    await tester.enterText(find.byType(TextField), 'git');
    await tester.pump();
    await tester.pump();
    expect(find.text('git status'), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    await tester.pump();
    expect(find.text('git status'), findsNothing);

    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.pump();
    expect(find.text('git status'), findsOneWidget);
  });

  testWidgets('composer shows dynamic suggestion candidates', (tester) async {
    final inputs = <String>[];
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    await _pumpTerminal(
      tester,
      controller,
      composerSuggestionResolver: (input, limit) =>
          input == 'cd ' ? const ['cd ..', 'cd src/'] : const [],
    );

    await tester.enterText(find.byType(TextField), 'cd ');
    await tester.pump();
    await tester.pump();

    expect(find.text('cd ..'), findsOneWidget);
    expect(find.text('cd src/'), findsOneWidget);

    await tester.tap(find.text('cd src/'));
    await tester.pump();
    await tester.testTextInput.receiveAction(TextInputAction.send);
    await tester.pump();

    expect(inputs.last, 'cd src/\r');
  });

  testWidgets('composer obscures sensitive prompt input', (tester) async {
    final inputs = <String>[];
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    addTearDown(controller.dispose);

    await _pumpTerminal(
      tester,
      controller,
      composerSuggestions: const ['secret command'],
    );
    controller.write('Password:');
    await tester.pump();

    final field = tester.widget<TextField>(find.byType(TextField));
    expect(field.obscureText, isTrue);
    expect(field.enableSuggestions, isFalse);
    expect(field.maxLines, 1);

    await tester.enterText(find.byType(TextField), 'secret');
    await tester.pump();
    await tester.testTextInput.receiveAction(TextInputAction.send);
    await tester.pump();

    expect(inputs.last, 'secret\r');
  });

  testWidgets('expanded composer keeps sensitive input single-line', (
    tester,
  ) async {
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver);
    addTearDown(controller.dispose);

    await _pumpTerminal(tester, controller);
    await tester.tap(find.byTooltip('Expand Composer'));
    await tester.pumpAndSettle();
    final expandedBeforePrompt = tester.getRect(
      find.byKey(const ValueKey('terminal-composer-surface')),
    );

    controller.write('Password:');
    await tester.pump();

    final field = tester.widget<TextField>(find.byType(TextField));
    expect(field.obscureText, isTrue);
    expect(field.expands, isFalse);
    expect(field.minLines, 1);
    expect(field.maxLines, 1);
    expect(
      tester.getRect(find.byKey(const ValueKey('terminal-composer-surface'))),
      expandedBeforePrompt,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('composer obscures sudo-rs authentication prompt input', (
    tester,
  ) async {
    final inputs = <String>[];
    final driver = MemoryTerminalDriver(columns: 80, rows: 8);
    final controller = TerminalController(driver: driver, onInput: inputs.add);
    addTearDown(controller.dispose);

    await _pumpTerminal(
      tester,
      controller,
      composerSuggestions: const ['Password: visible command'],
    );
    controller.write('[sudo: authenticate] Password:');
    await tester.pump();

    final field = tester.widget<TextField>(find.byType(TextField));
    expect(field.obscureText, isTrue);
    expect(field.enableSuggestions, isFalse);

    await tester.enterText(find.byType(TextField), 'secret');
    await tester.pump();
    await tester.testTextInput.receiveAction(TextInputAction.send);
    await tester.pump();

    expect(inputs.last, 'secret\r');
  });

  testWidgets('composer obscures input when driver reports echo off', (
    tester,
  ) async {
    final inputs = <String>[];
    final controller = TerminalController(
      driver: _SnapshotDriver(
        TerminalSnapshot.blank(columns: 80, rows: 8, inputEchoEnabled: false),
      ),
      onInput: inputs.add,
    );
    addTearDown(controller.dispose);

    await _pumpTerminal(
      tester,
      controller,
      composerSuggestions: const ['secret command'],
    );

    final field = tester.widget<TextField>(find.byType(TextField));
    expect(field.obscureText, isTrue);
    expect(field.enableSuggestions, isFalse);

    await tester.enterText(find.byType(TextField), 'secret');
    await tester.pump();
    await tester.testTextInput.receiveAction(TextInputAction.send);
    await tester.pump();

    expect(inputs.last, 'secret\r');
  });
}

class _SnapshotDriver implements TerminalDriver {
  _SnapshotDriver(this._snapshot, {this.selectionTextValue});

  TerminalSnapshot _snapshot;
  final String? selectionTextValue;
  int scrolledLines = 0;

  @override
  TerminalSnapshot get snapshot => _snapshot;

  @override
  bool get isExited => false;

  @override
  bool poll() => false;

  @override
  void resize(int columns, int rows, {int cellWidth = 1, int cellHeight = 1}) {
    _snapshot = TerminalSnapshot.blank(
      columns: columns,
      rows: rows,
      historyLines: _snapshot.historyLines,
      displayOffset: _snapshot.displayOffset,
      keyboardMode: _snapshot.keyboardMode,
      inputEchoEnabled: _snapshot.inputEchoEnabled,
    );
  }

  @override
  bool sendInput(String data) => false;

  @override
  Uint8List drainOutputCapture() => Uint8List(0);

  @override
  bool suppressOutputUntil(Uint8List marker) => false;

  @override
  bool cancelOutputSuppression() => false;

  @override
  void writeBytes(Uint8List bytes) {}

  @override
  bool scrollLines(int lines) {
    final displayOffset = (_snapshot.displayOffset + lines).clamp(
      0,
      _snapshot.historyLines,
    );
    if (displayOffset == _snapshot.displayOffset) {
      return false;
    }
    scrolledLines += lines;
    _snapshot = TerminalSnapshot.blank(
      columns: _snapshot.columns,
      rows: _snapshot.rows,
      historyLines: _snapshot.historyLines,
      displayOffset: displayOffset,
      keyboardMode: _snapshot.keyboardMode,
      inputEchoEnabled: _snapshot.inputEchoEnabled,
    );
    return true;
  }

  @override
  bool scrollPageUp() => false;

  @override
  bool scrollPageDown() => false;

  @override
  bool scrollToBottom() => false;

  @override
  TerminalSearchResult search(
    String query, {
    required TerminalSearchDirection direction,
    required TerminalCellPosition origin,
  }) {
    return const TerminalSearchResult.notFound();
  }

  @override
  String selectionText(TerminalSelection selection) {
    return selectionTextValue ?? terminalSelectedText(_snapshot, selection);
  }

  @override
  TerminalCommandBlock? commandBlockAt(TerminalCellPosition position) {
    final selection = terminalCommandBlockAt(_snapshot, position);
    return selection == null
        ? null
        : TerminalCommandBlock(selection: selection);
  }

  @override
  void clear() {}

  @override
  void reset() {}

  @override
  List<TerminalConnectionEvent> drainConnectionEvents() => const [];

  @override
  void write(String data) {}

  @override
  void dispose() {}
}

const _terminalScrollbarScrollVisibilityDurationForTest = Duration(
  milliseconds: 700,
);

Future<void> _pumpTerminal(
  WidgetTester tester,
  TerminalController controller, {
  List<String> composerHistory = const [],
  List<String> composerSuggestions = const [],
  TerminalComposerSuggestionResolver? composerSuggestionResolver,
  TerminalTheme theme = defaultTerminalTheme,
  bool autocompleteEnabled = true,
  bool autofocusTerminal = false,
  TerminalOpenTargetCallback? onOpenTarget,
  VoidCallback? onSettingsRequested,
}) async {
  await tester.pumpWidget(
    MaterialApp(
      home: SizedBox(
        width: 800,
        height: 240,
        child: TerminalView(
          controller: controller,
          config: defaultTerminalConfig.copyWith(
            composer: TerminalComposerConfig(
              autocompleteEnabled: autocompleteEnabled,
            ),
          ),
          theme: theme,
          autofocusTerminal: autofocusTerminal,
          composerHistory: composerHistory,
          composerSuggestions: composerSuggestions,
          composerSuggestionResolver: composerSuggestionResolver,
          padding: EdgeInsets.zero,
          onOpenTarget: onOpenTarget,
          onSettingsRequested: onSettingsRequested,
        ),
      ),
    ),
  );
  await tester.pump();
}

Offset _cellCenter(
  TerminalMetrics metrics, {
  required int column,
  required int row,
}) {
  return Offset(
    (column + 0.5) * metrics.cellSize.width,
    (row + 0.5) * metrics.cellSize.height,
  );
}

Future<void> _sendShortcut(
  WidgetTester tester,
  LogicalKeyboardKey logicalKey,
) async {
  if (defaultTargetPlatform == TargetPlatform.macOS) {
    await _sendModifiedKey(
      tester,
      logicalKey,
      modifier: LogicalKeyboardKey.metaLeft,
    );
    return;
  }
  final needsShift =
      logicalKey == LogicalKeyboardKey.keyC ||
      logicalKey == LogicalKeyboardKey.keyV ||
      logicalKey == LogicalKeyboardKey.keyA ||
      logicalKey == LogicalKeyboardKey.keyF;
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  if (needsShift) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  }
  await tester.sendKeyDownEvent(logicalKey);
  await tester.sendKeyUpEvent(logicalKey);
  if (needsShift) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  }
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
}

Future<void> _sendModifiedKey(
  WidgetTester tester,
  LogicalKeyboardKey logicalKey, {
  required LogicalKeyboardKey modifier,
}) async {
  await tester.sendKeyDownEvent(modifier);
  await tester.sendKeyDownEvent(logicalKey);
  await tester.sendKeyUpEvent(logicalKey);
  await tester.sendKeyUpEvent(modifier);
}

TerminalPainter _terminalPainter(WidgetTester tester) {
  final customPaint = tester.widget<CustomPaint>(
    find.byWidgetPredicate(
      (widget) => widget is CustomPaint && widget.painter is TerminalPainter,
    ),
  );
  return customPaint.painter! as TerminalPainter;
}
