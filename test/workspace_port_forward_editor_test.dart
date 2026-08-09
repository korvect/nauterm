import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';

void main() {
  testWidgets('port forward rule fields use the shared vertical gap', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('Port Forwarding'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));
    await tester.tap(find.text('New forwarding'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final ruleName = _decoratorForLabel('Rule name *');
    final type = _decoratorForLabel('Type');

    expect(tester.getSize(ruleName).height, 40);
    expect(tester.getSize(type).height, 40);
    expect(tester.getRect(type).top - tester.getRect(ruleName).bottom, 10);
    expect(find.text('Local port number *'), findsOneWidget);
    expect(find.text('Intermediate host *'), findsOneWidget);
    expect(find.text('Destination address *'), findsOneWidget);
    expect(find.text('Destination port number *'), findsOneWidget);
    _expectSearchableSelect(tester, 'Intermediate host *');
    _expectFieldsStacked(tester, 'Bind Address', 'Local port number *');
    _expectFieldsStacked(
      tester,
      'Destination address *',
      'Destination port number *',
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('port forward types are offered from the create dropdown', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('Port Forwarding'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('Local'), findsNothing);
    expect(find.text('Remote'), findsNothing);
    expect(find.text('Dynamic'), findsNothing);

    await tester.tap(find.byIcon(Icons.keyboard_arrow_down_rounded).first);
    await tester.pump();

    expect(find.text('Local'), findsOneWidget);
    expect(find.text('Remote'), findsOneWidget);
    expect(find.text('Dynamic'), findsOneWidget);

    await tester.tap(find.text('Remote'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('New forwarding'), findsNWidgets(2));
    final type = _decoratorForLabel('Type');
    expect(find.descendant(of: type, matching: find.text('Remote')), findsOne);
    expect(find.text('Remote port number *'), findsOneWidget);
    expect(find.text('Remote host *'), findsOneWidget);
    expect(find.text('Destination address *'), findsOneWidget);
    expect(find.text('Destination port number *'), findsOneWidget);
    _expectSearchableSelect(tester, 'Remote host *');
    _expectFieldsStacked(tester, 'Bind Address', 'Remote port number *');
    _expectFieldsStacked(
      tester,
      'Destination address *',
      'Destination port number *',
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('dynamic forward only requires its local port and host', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('Port Forwarding'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));
    await tester.tap(find.byIcon(Icons.keyboard_arrow_down_rounded).first);
    await tester.pump();
    await tester.tap(find.text('Dynamic'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('New forwarding'), findsNWidgets(2));
    expect(find.text('Local port number *'), findsOneWidget);
    expect(find.text('Intermediate host *'), findsOneWidget);
    expect(find.text('Destination address *'), findsNothing);
    expect(find.text('Destination port number *'), findsNothing);
    _expectSearchableSelect(tester, 'Intermediate host *');
    _expectFieldsStacked(tester, 'Bind Address', 'Local port number *');

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('port forward validation only runs on save and renders inline', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('Port Forwarding'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));
    await tester.tap(find.text('New forwarding'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    await tester.tap(_decoratorForLabel('Rule name *'));
    await tester.pump();
    await tester.tap(_decoratorForLabel('Bind Address'));
    await tester.pump();

    expect(
      find.byKey(const ValueKey('workspace-field-error:Rule name')),
      findsNothing,
    );

    await tester.tap(find.text('Save'));
    await tester.pump();

    expect(
      find.byKey(const ValueKey('workspace-field-error:Rule name')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('workspace-field-error:Local port number')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('workspace-field-error:Intermediate host')),
      findsOneWidget,
    );
    expect(
      find.byKey(const ValueKey('workspace-field-error:Destination address')),
      findsOneWidget,
    );
    expect(
      find.byKey(
        const ValueKey('workspace-field-error:Destination port number'),
      ),
      findsOneWidget,
    );
    expect(find.text('Rule name is required.'), findsOneWidget);

    final localPortField = find
        .descendant(
          of: _decoratorForLabel('Local port number *'),
          matching: find.byType(TextField),
        )
        .first;
    await tester.tap(localPortField);
    await tester.pump();

    expect(
      tester.widget<TextField>(localPortField).focusNode?.hasFocus,
      isTrue,
    );
    expect(
      find.byKey(const ValueKey('workspace-field-error:Local port number')),
      findsOneWidget,
    );

    final ruleNameField = find
        .descendant(
          of: _decoratorForLabel('Rule name *'),
          matching: find.byType(TextField),
        )
        .first;
    await tester.tap(ruleNameField);
    await tester.enterText(ruleNameField, 'Preview tunnel');
    await tester.pump();

    expect(tester.widget<TextField>(ruleNameField).focusNode?.hasFocus, isTrue);
    expect(
      find.byKey(const ValueKey('workspace-field-error:Rule name')),
      findsNothing,
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });
}

Finder _decoratorForLabel(String label) {
  return find
      .ancestor(of: find.text(label), matching: find.byType(InputDecorator))
      .first;
}

void _expectFieldsStacked(WidgetTester tester, String first, String second) {
  expect(
    tester.getTopLeft(_decoratorForLabel(second)).dy,
    greaterThan(tester.getBottomLeft(_decoratorForLabel(first)).dy),
  );
}

void _expectSearchableSelect(WidgetTester tester, String label) {
  expect(
    find.descendant(
      of: _decoratorForLabel(label),
      matching: find.byType(TextField),
    ),
    findsOneWidget,
  );
}
