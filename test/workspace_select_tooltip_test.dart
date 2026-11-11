import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  testWidgets('select menu follows its field when the parent scrolls', (
    tester,
  ) async {
    final scroll = ScrollController();
    addTearDown(scroll.dispose);
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Align(
            alignment: Alignment.topLeft,
            child: SizedBox(
              width: 300,
              height: 300,
              child: ListView(
                controller: scroll,
                children: [
                  const SizedBox(height: 100),
                  buildHostTagSelectForTesting(
                    tags: const [TagEntry(uuid: 'console', name: 'Console')],
                    selectedUuids: const {},
                    onChanged: (_) {},
                    onCreate: (_) => null,
                  ),
                  const SizedBox(height: 1000),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.byType(TextField));
    await tester.pump();
    final menu = find.byKey(const ValueKey('workspace-select-menu:Tags'));
    final field = find.byType(InputDecorator).first;
    expect(tester.getTopLeft(menu).dy, tester.getBottomLeft(field).dy + 4);
    scroll.jumpTo(40);
    await tester.pump();
    await tester.pump();
    expect(tester.getTopLeft(menu).dy, tester.getBottomLeft(field).dy + 4);
    expect(tester.takeException(), isNull);

    // Focus keeps the editable field alive after it leaves the ListView's
    // cache. Such children have no paint transform for anchoring a menu.
    scroll.jumpTo(700);
    await tester.pump();
    await tester.pump();
    expect(tester.takeException(), isNull);
    expect(menu, findsNothing);
    scroll.jumpTo(0);
    await tester.pump();
    await tester.tap(find.byType(TextField));
    await tester.pump();
    expect(menu, findsOneWidget);
    expect(tester.getTopLeft(menu).dy, tester.getBottomLeft(field).dy + 4);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('select options show native tooltips only when text overflows', (
    tester,
  ) async {
    const longLabel = 'cu.usbserial-ABC123  /dev/cu.usbserial-ABC123';
    const shortLabel = 'COM3';

    Future<void> openSelect(double width) async {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: Align(
              alignment: Alignment.topLeft,
              child: SizedBox(
                width: width,
                child: buildHostTagSelectForTesting(
                  tags: const [
                    TagEntry(uuid: 'long-port', name: longLabel),
                    TagEntry(uuid: 'short-port', name: shortLabel),
                  ],
                  selectedUuids: const {},
                  onChanged: (_) {},
                  onCreate: (_) => null,
                ),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.byType(TextField));
      await tester.pump();
    }

    await openSelect(220);
    expect(find.byTooltip(longLabel), findsOneWidget);
    expect(find.byTooltip(shortLabel), findsNothing);
    final tooltip = tester.widget<Tooltip>(
      find.byWidgetPredicate(
        (widget) => widget is Tooltip && widget.message == longLabel,
      ),
    );
    expect(tooltip.waitDuration, const Duration(milliseconds: 350));
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(find.text(longLabel)));
    await tester.pump(const Duration(milliseconds: 350));
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text(longLabel), findsNWidgets(2));
    await mouse.removePointer();
    await tester.pumpWidget(const SizedBox.shrink());

    await openSelect(700);
    expect(find.text(longLabel), findsOneWidget);
    expect(find.byTooltip(longLabel), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}
