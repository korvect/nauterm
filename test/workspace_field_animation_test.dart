import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';

void main() {
  testWidgets('input clear button fades without changing the text layout', (
    tester,
  ) async {
    await _openHostEditor(tester);
    final field = _textField('Name *');
    final clear = find.byTooltip('Clear Name');
    final initialRect = tester.getRect(field);
    expect(_affixOpacity(tester, clear), 0);
    expect(clear.hitTestable(), findsNothing);

    await tester.enterText(field, 'Production');
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 75));
    expect(_affixOpacity(tester, clear), allOf(greaterThan(0), lessThan(1)));
    await tester.pump(const Duration(milliseconds: 100));
    expect(_affixOpacity(tester, clear), 1);
    expect(tester.getRect(field), initialRect);

    tester.widget<TextField>(field).focusNode!.unfocus();
    await tester.pump();
    expect(clear.hitTestable(), findsNothing);
    expect(
      tester
          .widget<ExcludeSemantics>(
            find
                .ancestor(of: clear, matching: find.byType(ExcludeSemantics))
                .first,
          )
          .excluding,
      isTrue,
    );
    await tester.pump(const Duration(milliseconds: 75));
    expect(_affixOpacity(tester, clear), allOf(greaterThan(0), lessThan(1)));
    await tester.pump(const Duration(milliseconds: 100));
    expect(_affixOpacity(tester, clear), 0);
    expect(tester.getRect(field), initialRect);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('select crossfades its arrow and clear button in the same slot', (
    tester,
  ) async {
    await _openHostEditor(tester);
    final field = _textField('Parent group');
    final clear = find.byTooltip('Clear Parent group');
    final suffix = find.byKey(
      const ValueKey('workspace-select-suffix:Parent group'),
    );
    final arrow = find
        .descendant(of: suffix, matching: find.byType(Icon))
        .first;
    final initialRect = tester.getRect(suffix);
    expect(_affixOpacity(tester, clear), 0);
    expect(_affixOpacity(tester, arrow), 1);

    await tester.enterText(field, 'group-query');
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 75));
    final clearOpacity = _affixOpacity(tester, clear);
    expect(clearOpacity, allOf(greaterThan(0), lessThan(1)));
    expect(_affixOpacity(tester, arrow), closeTo(1 - clearOpacity, 0.001));
    await tester.pump(const Duration(milliseconds: 100));
    expect(_affixOpacity(tester, clear), 1);
    expect(_affixOpacity(tester, arrow), 0);
    expect(tester.getRect(suffix), initialRect);

    await tester.tap(clear);
    await tester.pump();
    await tester.pump();
    expect(clear.hitTestable(), findsNothing);
    await tester.pump(const Duration(milliseconds: 75));
    expect(_affixOpacity(tester, clear), allOf(greaterThan(0), lessThan(1)));
    await tester.pump(const Duration(milliseconds: 100));
    expect(_affixOpacity(tester, clear), 0);
    expect(_affixOpacity(tester, arrow), 1);
    expect(tester.widget<TextField>(field).controller!.text, isEmpty);
    expect(tester.widget<TextField>(field).focusNode!.hasFocus, isTrue);
    expect(tester.getRect(suffix), initialRect);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('multiline input clear button fades in and out', (tester) async {
    _setDesktopSize(tester);
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('Snippets'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));
    await tester.tap(find.text('New snippet'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final field = _textField('Script *');
    final clear = find.byTooltip('Clear Script');
    expect(_affixOpacity(tester, clear), 0);
    await tester.enterText(field, 'echo hello\necho world');
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 75));
    expect(_affixOpacity(tester, clear), allOf(greaterThan(0), lessThan(1)));
    await tester.pump(const Duration(milliseconds: 100));
    await tester.tap(clear);
    await tester.pump();
    expect(clear.hitTestable(), findsNothing);
    await tester.pump(const Duration(milliseconds: 75));
    expect(_affixOpacity(tester, clear), allOf(greaterThan(0), lessThan(1)));
    await tester.pump(const Duration(milliseconds: 100));
    expect(_affixOpacity(tester, clear), 0);
    expect(tester.widget<TextField>(field).controller!.text, isEmpty);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('clear button respects reduced motion', (tester) async {
    tester.platformDispatcher.accessibilityFeaturesTestValue =
        FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(tester.platformDispatcher.clearAccessibilityFeaturesTestValue);
    await _openHostEditor(tester);
    final field = _textField('Name *');
    final clear = find.byTooltip('Clear Name');
    await tester.enterText(field, 'Production');
    await tester.pump();
    expect(_affixOpacity(tester, clear), 1);
    await tester.tap(clear);
    await tester.pump();
    expect(_affixOpacity(tester, clear), 0);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

void _setDesktopSize(WidgetTester tester) {
  tester.view.physicalSize = const Size(1440, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
}

Future<void> _openHostEditor(WidgetTester tester) async {
  _setDesktopSize(tester);
  await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
  await tester.tap(find.text('New host'));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 250));
}

Finder _textField(String label) => find
    .descendant(
      of: find
          .ancestor(of: find.text(label), matching: find.byType(InputDecorator))
          .first,
      matching: find.byType(TextField),
    )
    .first;

double _affixOpacity(WidgetTester tester, Finder child) {
  final opacity = find
      .ancestor(of: child, matching: find.byType(AnimatedOpacity))
      .first;
  final fade = find
      .descendant(of: opacity, matching: find.byType(FadeTransition))
      .first;
  return tester.widget<FadeTransition>(fade).opacity.value;
}
