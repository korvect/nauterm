import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';

void main() {
  testWidgets('editable select clears without opening and selects an option', (
    tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.pump();

    await tester.tap(find.text('Serial').first);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 1));

    final baudDecorator = find
        .ancestor(
          of: find.text('Baud Rate'),
          matching: find.byType(InputDecorator),
        )
        .first;
    final baudField = find
        .descendant(of: baudDecorator, matching: find.byType(TextField))
        .first;
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    addTearDown(mouse.removePointer);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(baudDecorator));
    await tester.pump();

    expect(find.byTooltip('Clear Baud Rate'), findsOneWidget);
    expect(tester.widget<TextField>(baudField).focusNode?.hasFocus, isFalse);

    await tester.tap(find.byTooltip('Clear Baud Rate'));
    await tester.pump();

    expect(tester.widget<TextField>(baudField).controller!.text, isEmpty);
    expect(tester.widget<TextField>(baudField).focusNode?.hasFocus, isFalse);
    expect(
      find.byKey(const ValueKey('workspace-select-menu:Baud Rate')),
      findsNothing,
    );

    final baudSuffix = find.byKey(
      const ValueKey('workspace-select-suffix:Baud Rate'),
    );
    await tester.tap(baudSuffix);
    await tester.pump();
    expect(
      find.byKey(const ValueKey('workspace-select-menu:Baud Rate')),
      findsOneWidget,
    );

    await tester.tap(baudSuffix);
    await tester.pump();
    await tester.pump();
    expect(
      find.byKey(const ValueKey('workspace-select-menu:Baud Rate')),
      findsNothing,
    );

    await tester.tap(baudDecorator);
    await tester.pump();
    expect(
      find.byKey(const ValueKey('workspace-select-menu:Baud Rate')),
      findsOneWidget,
    );

    await tester.tap(find.text('9600'));
    await tester.pump();

    expect(tester.widget<TextField>(baudField).controller!.text, '9600');
    expect(
      find.byKey(const ValueKey('workspace-select-menu:Baud Rate')),
      findsNothing,
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('editable select follows focus without leaving its menu open', (
    tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.pump();
    await tester.tap(find.text('Serial').first);
    await tester.pump();

    final serialPortField = find
        .descendant(
          of: find
              .ancestor(
                of: find.text('Serial Port'),
                matching: find.byType(InputDecorator),
              )
              .first,
          matching: find.byType(TextField),
        )
        .first;
    final serialFocusNode = tester
        .widget<TextField>(serialPortField)
        .focusNode!;
    serialFocusNode.unfocus();
    await tester.pump();
    serialFocusNode.requestFocus();
    await tester.pump();
    expect(serialFocusNode.hasFocus, isTrue);
    expect(
      find.byKey(const ValueKey('workspace-select-menu:Serial Port')),
      findsNothing,
    );
    await tester.tap(serialPortField);
    await tester.pump();
    expect(
      find.byKey(const ValueKey('workspace-select-menu:Serial Port')),
      findsOneWidget,
    );
    serialFocusNode.unfocus();
    await tester.pump();
    expect(
      find.byKey(const ValueKey('workspace-select-menu:Serial Port')),
      findsNothing,
    );
    await tester.pump(const Duration(milliseconds: 1));
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('serial dialog uses large selects with aligned arrows', (
    tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.pump();

    await tester.tap(find.text('Serial').first);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 1));

    expect(
      tester
          .getSize(find.byKey(const ValueKey('workspace-dialog-action:0')))
          .height,
      32,
    );
    expect(
      tester
          .getSize(find.byKey(const ValueKey('workspace-dialog-action:1')))
          .height,
      32,
    );
    final dialogHeader = tester.widget<Container>(
      find.byKey(const ValueKey('workspace-dialog-header')),
    );
    expect(
      (dialogHeader.decoration! as BoxDecoration).color,
      const Color(0xfff6f9f9),
    );

    for (final label in [
      'Serial Port',
      'Baud Rate',
      'Data Bits',
      'Stop Bits',
      'Parity',
      'Flow Control',
    ]) {
      final decorator = find
          .ancestor(of: find.text(label), matching: find.byType(InputDecorator))
          .first;
      final arrow = find.descendant(
        of: _suffixForLabel(label),
        matching: find.byIcon(Icons.keyboard_arrow_down_rounded),
      );
      final decoratorRect = tester.getRect(decorator);
      final arrowRect = tester.getRect(arrow);

      expect(decoratorRect.height, 40);
      expect(arrowRect.center.dy, decoratorRect.center.dy);
      expect(decoratorRect.right - arrowRect.center.dx, 19);
    }

    final serialPortDecorator = find
        .ancestor(
          of: find.text('Serial Port'),
          matching: find.byType(InputDecorator),
        )
        .first;
    final serialPortArrow = find.descendant(
      of: _suffixForLabel('Serial Port'),
      matching: find.byIcon(Icons.keyboard_arrow_down_rounded),
    );
    expect(find.byTooltip('Clear Serial Port').hitTestable(), findsNothing);
    expect(find.byTooltip('Clear Baud Rate').hitTestable(), findsNothing);
    expect(serialPortArrow, findsOneWidget);
    final serialPortArrowCenter = tester.getCenter(serialPortArrow);
    expect(tester.getSize(find.byTooltip('Refresh serial devices')).height, 40);

    final serialPortTextField = find
        .descendant(of: serialPortDecorator, matching: find.byType(TextField))
        .first;
    expect(tester.widget<TextField>(serialPortTextField).style?.fontSize, 14);
    expect(
      tester.widget<TextField>(serialPortTextField).style?.fontWeight,
      FontWeight.w400,
    );

    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    addTearDown(mouse.removePointer);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(serialPortDecorator));
    await tester.pump();

    expect(find.byTooltip('Clear Serial Port'), findsOneWidget);
    expect(
      find
          .descendant(
            of: _suffixForLabel('Serial Port'),
            matching: find.byIcon(Icons.keyboard_arrow_down_rounded),
          )
          .hitTestable(),
      findsNothing,
    );

    await mouse.moveTo(Offset.zero);
    await tester.pump();

    expect(find.byTooltip('Clear Serial Port').hitTestable(), findsNothing);
    expect(serialPortArrow, findsOneWidget);

    final serialPortRect = tester.getRect(serialPortDecorator);
    await tester.tapAt(
      Offset(serialPortRect.right - 2, serialPortRect.center.dy),
    );
    await tester.pump();

    expect(
      tester.widget<TextField>(serialPortTextField).focusNode?.hasFocus,
      isTrue,
    );
    expect(find.byTooltip('Clear Serial Port'), findsOneWidget);
    expect(find.byTooltip('Clear Baud Rate').hitTestable(), findsNothing);
    expect(
      find
          .descendant(
            of: _suffixForLabel('Serial Port'),
            matching: find.byIcon(Icons.keyboard_arrow_down_rounded),
          )
          .hitTestable(),
      findsNothing,
    );

    final serialPortClearSurface = find.descendant(
      of: find.byTooltip('Clear Serial Port'),
      matching: find.byWidgetPredicate(
        (widget) => widget is Material && widget.shape is CircleBorder,
      ),
    );
    expect(serialPortClearSurface, findsOneWidget);
    expect(tester.getSize(serialPortClearSurface), const Size.square(16));
    expect(tester.getCenter(serialPortClearSurface), serialPortArrowCenter);
    expect(
      tester.getCenter(serialPortClearSurface).dy,
      tester.getCenter(serialPortDecorator).dy,
    );
    expect(
      tester.getRect(serialPortDecorator).right -
          tester.getCenter(serialPortClearSurface).dx,
      19,
    );

    await tester.tap(find.byTooltip('Clear Serial Port'));
    await tester.pump();

    final serialPortField = tester.widget<TextField>(
      find
          .descendant(
            of: find.ancestor(
              of: find.text('Serial Port'),
              matching: find.byType(InputDecorator),
            ),
            matching: find.byType(TextField),
          )
          .first,
    );
    expect(serialPortField.controller!.text, isEmpty);
    expect(serialPortField.focusNode?.hasFocus, isTrue);
    expect(find.byTooltip('Clear Serial Port').hitTestable(), findsNothing);
    expect(find.byTooltip('Clear Baud Rate').hitTestable(), findsNothing);
    expect(
      find.descendant(
        of: _suffixForLabel('Serial Port'),
        matching: find.byIcon(Icons.keyboard_arrow_down_rounded),
      ),
      findsOneWidget,
    );
    expect(
      tester
          .widget<AnimatedRotation>(
            find.descendant(
              of: _suffixForLabel('Serial Port'),
              matching: find.byType(AnimatedRotation),
            ),
          )
          .turns,
      0.5,
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });
}

Finder _suffixForLabel(String label) {
  return find.byKey(ValueKey('workspace-select-suffix:$label'));
}
