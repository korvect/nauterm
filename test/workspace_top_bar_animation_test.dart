import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/terminal/terminal_config.dart';

void main() {
  setUp(() async {
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
    final previousSftp = sftpTabEnabled;
    final previousWorkspace = workspacePageEnabled;
    setSftpTabEnabled(true);
    setWorkspacePageEnabled(true);
    addTearDown(() {
      setSftpTabEnabled(previousSftp);
      setWorkspacePageEnabled(previousWorkspace);
    });
  });

  for (final label in ['Vaults', 'SFTP']) {
    testWidgets('$label reveals its full-width label without moving its icon', (
      tester,
    ) async {
      await _pumpApp(tester);
      final vaults = _tab('vaults');
      final sftp = _tab('sftp');
      if (label == 'SFTP') {
        await tester.tap(sftp);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 300));
      }
      final tab = label == 'Vaults' ? vaults : sftp;
      final other = label == 'Vaults' ? sftp : vaults;
      final icon = label == 'Vaults'
          ? Icons.security_rounded
          : Icons.folder_rounded;
      final text = find.descendant(of: tab, matching: find.text(label));
      final naturalTextWidth = tester.getSize(text).width;
      final expandedWidth = tester.getSize(tab).width;
      expect(expandedWidth, greaterThan(30));
      _expectStableContent(tester, tab, text, icon, naturalTextWidth);

      await tester.tap(other);
      await tester.pump();
      // The painted background must not jump to its collapsed width while
      // an unrelated layout box is still shrinking around it.
      expect(tester.getSize(tab).width, expandedWidth);
      var previousWidth = expandedWidth;
      for (final duration in [16, 54, 70, 140]) {
        await tester.pump(Duration(milliseconds: duration));
        final width = tester.getSize(tab).width;
        expect(width, lessThanOrEqualTo(previousWidth));
        expect(width, greaterThanOrEqualTo(30));
        _expectStableContent(tester, tab, text, icon, naturalTextWidth);
        previousWidth = width;
      }
      expect(tester.getSize(tab).width, 30);
      expect(
        tester.getRect(text).left,
        greaterThan(tester.getRect(_clip(tab)).right),
      );

      await tester.tap(tab);
      await tester.pump();
      expect(tester.getSize(tab).width, 30);
      await tester.pump(const Duration(milliseconds: 70));
      final expectedWidth =
          30 + (expandedWidth - 30) * Curves.easeOutExpo.transform(0.25);
      expect(tester.getSize(tab).width, closeTo(expectedWidth, 0.01));
      _expectStableContent(tester, tab, text, icon, naturalTextWidth);
      await tester.pump(const Duration(milliseconds: 210));
      expect(tester.getSize(tab).width, expandedWidth);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }

  testWidgets('rapid tab switching retargets the current width and colors', (
    tester,
  ) async {
    await _pumpApp(tester);
    final vaults = _tab('vaults');
    final expandedWidth = tester.getSize(vaults).width;
    await tester.tap(_tab('sftp'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 35));
    final interruptedWidth = tester.getSize(vaults).width;
    final interruptedColor = _background(tester, vaults);
    expect(interruptedWidth, greaterThan(30));
    expect(interruptedWidth, lessThan(expandedWidth));

    await tester.tap(vaults);
    await tester.pump();
    expect(tester.getSize(vaults).width, closeTo(interruptedWidth, 0.01));
    expect(_background(tester, vaults), interruptedColor);
    await tester.pump(const Duration(milliseconds: 35));
    expect(tester.getSize(vaults).width, greaterThan(interruptedWidth));
    await tester.pump(const Duration(milliseconds: 245));
    expect(tester.getSize(vaults).width, expandedWidth);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('unrelated rebuilds do not restart tab width motion', (
    tester,
  ) async {
    await _pumpApp(tester);
    final vaults = _tab('vaults');
    await tester.tap(_tab('sftp'));
    await tester.pump();
    for (var frame = 0; frame < 8; frame++) {
      await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
      await tester.pump(const Duration(milliseconds: 35));
    }
    expect(tester.getSize(vaults).width, 30);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('reduced motion settles tab width and color immediately', (
    tester,
  ) async {
    tester.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(tester.platformDispatcher.clearAccessibilityFeaturesTestValue);
    await _pumpApp(tester);
    final vaults = _tab('vaults');
    final initialWidth = tester.getSize(vaults).width;
    final initialColor = _background(tester, vaults);
    await tester.tap(_tab('sftp'));
    await tester.pump();
    expect(tester.getSize(vaults).width, 30);
    final collapsedColor = _background(tester, vaults);
    expect(collapsedColor, isNot(initialColor));
    await tester.pump(const Duration(milliseconds: 16));
    expect(_background(tester, vaults), collapsedColor);
    await tester.tap(vaults);
    await tester.pump();
    expect(tester.getSize(vaults).width, initialWidth);
    expect(_background(tester, vaults), initialColor);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
    'terminal tabs retain their icon, title and close-button layout',
    (tester) async {
      await _pumpApp(tester);
      await tester.tap(find.text('Terminal'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
      final terminal = find.byWidgetPredicate(
        (widget) =>
            widget.key is ValueKey<String> &&
            (widget.key! as ValueKey<String>).value.startsWith(
              'terminal-top-tab:',
            ),
      );
      expect(terminal, findsOneWidget);
      expect(tester.getSize(terminal).width, 220);
      final label = find.descendant(of: terminal, matching: find.byType(Text));
      final title = tester.widget<Text>(label).data;
      expect(tester.widget<Text>(label).overflow, TextOverflow.ellipsis);
      await tester.tap(_tab('vaults'));
      await tester.pump();
      for (final duration in [16, 54, 70, 140]) {
        await tester.pump(Duration(milliseconds: duration));
        final rect = tester.getRect(terminal);
        final icon = tester.getRect(
          find.descendant(
            of: terminal,
            matching: find.byIcon(LucideIcons.squareTerminal),
          ),
        );
        final close = tester.getRect(
          find.descendant(
            of: terminal,
            matching: find.byIcon(Icons.close_rounded),
          ),
        );
        expect(icon.left - rect.left, 8);
        expect(rect.right - close.right, 8);
        expect(tester.widget<Text>(label).data, title);
        expect(tester.getSize(_chrome(terminal)).width, rect.width);
      }
      expect(tester.getSize(terminal).width, 142);
      await tester.tap(terminal);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 280));
      expect(tester.getSize(terminal).width, 220);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}

Future<void> _pumpApp(WidgetTester tester) async {
  tester.view.physicalSize = const Size(1200, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 500));
}

Finder _tab(String name) => find.byKey(ValueKey('workspace-top-tab:$name'));

Finder _chrome(Finder tab) => find.descendant(
  of: tab,
  matching: find.byKey(const ValueKey('top-tab-chrome')),
);

Finder _clip(Finder tab) =>
    find.descendant(of: tab, matching: find.byType(ClipRect));

Color _background(WidgetTester tester, Finder tab) =>
    (tester
                .widget<DecoratedBox>(
                  find.descendant(
                    of: _chrome(tab),
                    matching: find.byType(DecoratedBox),
                  ),
                )
                .decoration
            as BoxDecoration)
        .color!;

void _expectStableContent(
  WidgetTester tester,
  Finder tab,
  Finder text,
  IconData icon,
  double naturalTextWidth,
) {
  final rect = tester.getRect(tab);
  final leading = find.descendant(of: tab, matching: find.byIcon(icon));
  expect(tester.widget<Icon>(leading).size, 14);
  expect(tester.getRect(leading).left - rect.left, 8);
  expect(tester.getSize(text).width, naturalTextWidth);
  expect(tester.getSize(_chrome(tab)).width, rect.width);
  final label = tester.widget<Text>(text);
  expect(label.softWrap, isFalse);
  expect(label.overflow, TextOverflow.visible);
  expect(DefaultTextStyle.of(tester.element(text)).style.fontSize, 13);
  expect(
    find.descendant(of: tab, matching: find.byType(Opacity)),
    findsNothing,
  );
  expect(
    find.descendant(of: tab, matching: find.byType(AnimatedOpacity)),
    findsNothing,
  );
  expect(tester.takeException(), isNull);
}
