import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/ui/terminal_theme_preview.dart';

void main() {
  testWidgets('SSH compatibility help does not change the setting', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));
    expect(find.text('Compatibility mode'), findsNothing);
    tester
        .widget<InkWell>(find.byKey(const ValueKey('protocol-show-more')))
        .onTap!();
    await tester.pump();

    final toggle = find.byKey(
      const ValueKey('workspace-toggle:Compatibility mode'),
    );
    await tester.ensureVisible(toggle);
    await tester.pump(const Duration(milliseconds: 200));
    final sshSection = find
        .ancestor(
          of: find.byKey(const ValueKey('protocol-port:SSH')),
          matching: find.byType(DecoratedBox),
        )
        .first;
    expect(find.descendant(of: sshSection, matching: toggle), findsOneWidget);
    expect(
      tester.getTopLeft(toggle).dy,
      greaterThan(
        tester
            .getBottomLeft(
              find.byKey(const ValueKey('workspace-select-focus:Mosh')),
            )
            .dy,
      ),
    );
    expect(
      tester.getTopLeft(find.text('Light theme')).dy,
      greaterThan(tester.getBottomLeft(toggle).dy),
    );
    final semantics = find.ancestor(
      of: toggle,
      matching: find.byWidgetPredicate(
        (widget) => widget is Semantics && widget.properties.toggled != null,
      ),
    );
    expect(tester.widget<Semantics>(semantics).properties.toggled, isFalse);

    const help = 'Skip shell detection and use compatible SSH algorithms.';
    final info = find.descendant(
      of: find.byTooltip(help),
      matching: find.byType(IconButton),
    );
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(info));
    await tester.pump(const Duration(milliseconds: 550));
    await mouse.down(tester.getCenter(info));
    await mouse.up();
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.text(help), findsOneWidget);
    expect(tester.widget<Semantics>(semantics).properties.toggled, isFalse);

    await mouse.moveTo(tester.getCenter(find.text('Compatibility mode')));
    await mouse.down(tester.getCenter(find.text('Compatibility mode')));
    await mouse.up();
    await tester.pump();
    expect(tester.widget<Semantics>(semantics).properties.toggled, isTrue);
    await mouse.removePointer();
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('protocol header uses a wrapping layout', (tester) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final port = find.byKey(const ValueKey('protocol-port:SSH'));
    expect(port, findsOneWidget);
    expect(
      find.ancestor(of: port, matching: find.byType(Wrap)),
      findsOneWidget,
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('mosh uses a select and reveals its command when enabled', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final showMoreButton = find.byKey(const ValueKey('protocol-show-more'));
    tester.widget<InkWell>(showMoreButton).onTap!();
    await tester.pump();

    final mosh = find.byKey(const ValueKey('workspace-select-focus:Mosh'));
    final editorScrollable = find
        .descendant(
          of: find.byKey(const ValueKey('workspace-editor-scroll-view')),
          matching: find.byType(Scrollable),
        )
        .first;
    final position = tester.state<ScrollableState>(editorScrollable).position;
    for (var index = 0; mosh.evaluate().isEmpty && index < 12; index++) {
      position.jumpTo(
        (position.pixels + 250).clamp(0.0, position.maxScrollExtent).toDouble(),
      );
      await tester.pump();
    }

    expect(mosh, findsOneWidget);
    await Scrollable.ensureVisible(tester.element(mosh), alignment: 0.5);
    await tester.pump();
    await tester.tap(mosh);
    await tester.pump();
    await tester.tap(find.text('Enabled').last);
    await tester.pump();
    expect(find.text('Mosh server command'), findsOneWidget);
    final themePreviews = find.descendant(
      of: find.byKey(const ValueKey('workspace-editor-scroll-view')),
      matching: find.byType(TerminalThemePreviewCard),
    );
    expect(themePreviews, findsNWidgets(2));
    expect(
      tester.getTopLeft(themePreviews.last).dy,
      greaterThan(tester.getBottomRight(themePreviews.first).dy),
    );
    expect(
      tester
          .widgetList<TextField>(find.byType(TextField))
          .any((field) => field.controller?.text == defaultMoshServerCommand),
      isTrue,
    );
    expect(
      tester.getTopLeft(themePreviews.first).dy,
      greaterThan(tester.getTopLeft(mosh).dy),
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('telnet can be added and removed as a host protocol', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final addProtocol = find.byKey(const ValueKey('add-protocol'));
    final editorScrollable = find
        .descendant(
          of: find.byKey(const ValueKey('workspace-editor-scroll-view')),
          matching: find.byType(Scrollable),
        )
        .first;
    final position = tester.state<ScrollableState>(editorScrollable).position;
    for (var index = 0; addProtocol.evaluate().isEmpty && index < 12; index++) {
      position.jumpTo(
        (position.pixels + 250).clamp(0.0, position.maxScrollExtent).toDouble(),
      );
      await tester.pump();
    }
    expect(addProtocol, findsOneWidget);
    tester.widget<InkWell>(addProtocol).onTap!();
    await tester.pump();
    await tester.tap(find.text('Telnet').last);
    await tester.pump();

    final telnetActions = find.byKey(
      const ValueKey('protocol-actions:Telnet'),
      skipOffstage: false,
    );
    expect(telnetActions, findsOneWidget);
    expect(
      find.text('Telnet on', findRichText: true, skipOffstage: false),
      findsOneWidget,
    );

    await tester.ensureVisible(telnetActions);
    await tester.pump();
    await tester.tap(telnetActions);
    await tester.pump();
    expect(find.text('Create group'), findsOneWidget);
    await tester.tap(find.text('Remove protocol'));
    await tester.pump();
    expect(telnetActions, findsNothing);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('protocol actions create a group from the current protocol', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final hostPort = find.descendant(
      of: find.byKey(const ValueKey('protocol-port:SSH')),
      matching: find.byType(TextField),
    );
    await tester.enterText(hostPort, '2202');
    await tester.pump();

    await tester.tap(find.byKey(const ValueKey('protocol-actions:SSH')));
    await tester.pump();
    await tester.tap(find.text('Create group'));
    await tester.pump();

    final groupDrawer = find.byKey(const ValueKey<Object>('group:new:root'));
    expect(groupDrawer, findsOneWidget);
    final groupPort = find.descendant(
      of: find.descendant(
        of: groupDrawer,
        matching: find.byKey(const ValueKey('protocol-port:SSH')),
      ),
      matching: find.byType(TextField),
    );
    expect(tester.widget<TextField>(groupPort).controller?.text, '2202');
    expect(
      find.descendant(
        of: groupDrawer,
        matching: find.text('Telnet on', findRichText: true),
      ),
      findsNothing,
    );

    await tester.tap(
      find.descendant(
        of: groupDrawer,
        matching: find.byKey(const ValueKey('protocol-actions:SSH')),
      ),
    );
    await tester.pump();
    expect(find.text('Create group'), findsNothing);
    expect(find.text('Remove protocol'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });
}
