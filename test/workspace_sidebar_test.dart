import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/app/nauterm_localizations.dart';

const _sidebarKey = ValueKey('workspace-sidebar');
const _toggleKey = ValueKey('workspace-sidebar-toggle');

void main() {
  setUp(() => setAppLanguage(AppLanguage.english));

  testWidgets(
    'divider toggle stays centered while collapsing and survives section changes',
    (tester) async {
      await _pumpApp(tester, width: 1100);
      expect(tester.getSize(find.byKey(_sidebarKey)).width, 190);
      _expectToggleOnDivider(tester);
      expect(find.byTooltip('Collapse sidebar'), findsOneWidget);
      final rotation = find.descendant(
        of: find.byKey(const ValueKey('workspace-sidebar-toggle-arrow')),
        matching: find.byType(RotationTransition),
      );
      double turns() => tester.widget<RotationTransition>(rotation).turns.value;
      expect(turns(), 0);

      await tester.tap(find.byKey(_toggleKey));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 80));
      expect(
        tester.getSize(find.byKey(_sidebarKey)).width,
        allOf(greaterThan(56), lessThan(190)),
      );
      _expectToggleOnDivider(tester);
      expect(turns(), allOf(greaterThan(0), lessThan(0.5)));
      await tester.pump(const Duration(milliseconds: 100));
      expect(tester.getSize(find.byKey(_sidebarKey)).width, 56);
      expect(turns(), 0.5);
      expect(find.byTooltip('Expand sidebar'), findsOneWidget);

      await tester.tap(
        find.descendant(
          of: find.byKey(_sidebarKey),
          matching: find.byIcon(LucideIcons.key),
        ),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(tester.getSize(find.byKey(_sidebarKey)).width, 56);
      _expectToggleOnDivider(tester);

      // The half of the toggle on the content side must also accept clicks.
      await tester.tapAt(
        tester.getCenter(find.byKey(_toggleKey)) + const Offset(8, 0),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(tester.getSize(find.byKey(_sidebarKey)).width, 190);
      expect(turns(), 0);
      _expectToggleOnDivider(tester);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets('narrow sidebar stays collapsed until there is enough space', (
    tester,
  ) async {
    await _pumpApp(tester, width: 700);
    expect(tester.getSize(find.byKey(_sidebarKey)).width, 56);
    _expectToggleOnDivider(tester);
    await tester.tap(find.byKey(_toggleKey));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(tester.getSize(find.byKey(_sidebarKey)).width, 56);
    expect(
      find.byTooltip('Widen the window to expand the sidebar'),
      findsOneWidget,
    );
    tester.view.physicalSize = const Size(1100, 800);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(tester.getSize(find.byKey(_sidebarKey)).width, 190);
    expect(find.byTooltip('Collapse sidebar'), findsOneWidget);
    _expectToggleOnDivider(tester);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('shrinking the window overrides manual expansion temporarily', (
    tester,
  ) async {
    await _pumpApp(tester, width: 1100);
    for (var i = 0; i < 2; i++) {
      await tester.tap(find.byKey(_toggleKey));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
    }
    expect(tester.getSize(find.byKey(_sidebarKey)).width, 190);
    tester.view.physicalSize = const Size(700, 800);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(tester.getSize(find.byKey(_sidebarKey)).width, 56);
    await tester.tap(find.byKey(_toggleKey));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(tester.getSize(find.byKey(_sidebarKey)).width, 56);
    _expectToggleOnDivider(tester);
    tester.view.physicalSize = const Size(1100, 800);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(tester.getSize(find.byKey(_sidebarKey)).width, 190);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

Future<void> _pumpApp(WidgetTester tester, {required double width}) async {
  tester.view.physicalSize = Size(width, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 200));
}

void _expectToggleOnDivider(WidgetTester tester) {
  final sidebar = tester.getRect(find.byKey(_sidebarKey));
  final center = tester.getCenter(find.byKey(_toggleKey));
  expect(center.dx, closeTo(sidebar.right, 0.01));
  expect(center.dy, closeTo(sidebar.center.dy, 0.01));
}
