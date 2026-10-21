import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';

void main() {
  testWidgets('snippet fields keep their gap and use the shared line height', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.pump(const Duration(seconds: 1));
    await tester.tap(find.text('Snippets'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));
    await tester.tap(find.text('New snippet'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final description = _decoratorForLabel('Description');
    final script = _decoratorForLabel('Script *');

    expect(tester.getRect(script).top - tester.getRect(description).bottom, 10);
    for (final decorator in [description, script]) {
      final field = tester.widget<TextField>(
        find.descendant(of: decorator, matching: find.byType(TextField)).first,
      );
      expect(field.style?.fontSize, 14);
      expect(field.style?.height, 1.5);
      expect(field.cursorHeight, isNull);
    }
    expect(tester.getSize(description).height, 40);
    expect(
      tester.getSize(script).height,
      closeTo(10 * 14 * 1.5 + 14 + 2, 0.01),
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('snippet package is available from toolbar and package select', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.pump(const Duration(seconds: 1));
    await tester.tap(find.text('Snippets'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('New snippet package'), findsNothing);
    await tester.tap(find.byIcon(Icons.keyboard_arrow_down_rounded).first);
    await tester.pump();
    expect(find.text('New snippet package'), findsOneWidget);

    await tester.tap(find.text('New snippet package'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('New Snippet Package'), findsOneWidget);
    await tester.tap(find.byTooltip('Close'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    await tester.tap(find.text('New snippet'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final packageField = _decoratorForLabel('Package');
    final packageInput = find.descendant(
      of: packageField,
      matching: find.byType(EditableText),
    );
    await tester.tap(packageInput);
    await tester.pump();
    await tester.enterText(packageInput, 'Tools');
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.text('Create package Tools'), findsOneWidget);

    await tester.tap(find.text('Create package Tools'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));
    expect(find.text('New Snippet Package'), findsNothing);
    expect(find.text('New Snippet'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });
}

Finder _decoratorForLabel(String label) {
  return find
      .ancestor(of: find.text(label), matching: find.byType(InputDecorator))
      .first;
}
