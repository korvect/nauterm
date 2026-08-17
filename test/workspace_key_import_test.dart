import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';

void main() {
  testWidgets('new key drawer offers private key file import', (tester) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.pump(const Duration(milliseconds: 200));
    await tester.tap(find.text('Keychain'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    await tester.pump();

    await tester.tap(find.text('New key'));
    await tester.pump(const Duration(milliseconds: 1));
    expect(tester.takeException(), isNull);
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.text('New Key'), findsOneWidget);
    expect(find.text('Import'), findsOneWidget);
    expect(find.text('Drag and drop a private key file'), findsOneWidget);
    expect(find.text('Import from key file'), findsOneWidget);
    expect(find.text('Name'), findsOneWidget);
    expect(
      tester.getTopLeft(find.text('Name')).dy,
      lessThan(tester.getTopLeft(find.text('Import')).dy),
    );

    final privateKeyDecorator = _keyFieldDecorator('Private key *');
    final publicKeyDecorator = _keyFieldDecorator('Public key');
    final initialHeight = tester.getSize(privateKeyDecorator).height;
    expect(initialHeight, 104);
    expect(initialHeight, tester.getSize(publicKeyDecorator).height);
    expect(
      find.ancestor(
        of: find.text('Certificate'),
        matching: find.byType(InputDecorator),
      ),
      findsNothing,
    );

    final privateKeyField = find
        .descendant(of: privateKeyDecorator, matching: find.byType(TextField))
        .first;
    final privateKeyScrollable = find
        .descendant(of: privateKeyField, matching: find.byType(Scrollable))
        .first;
    await tester.enterText(
      privateKeyField,
      List.generate(5, (_) => 'x').join('\n'),
    );
    await tester.pump();
    final expandedHeight = tester.getSize(privateKeyDecorator).height;
    expect(expandedHeight, greaterThan(initialHeight));
    expect(
      tester
          .state<ScrollableState>(privateKeyScrollable)
          .position
          .maxScrollExtent,
      0,
    );

    await tester.enterText(
      privateKeyField,
      List.generate(12, (index) => 'line-$index').join('\n'),
    );
    await tester.pump();
    final maximumHeight = tester.getSize(privateKeyDecorator).height;
    expect(maximumHeight, greaterThan(expandedHeight));

    await tester.enterText(
      privateKeyField,
      List.generate(20, (index) => 'line-$index').join('\n'),
    );
    await tester.pump();
    expect(tester.getSize(privateKeyDecorator).height, maximumHeight);
    expect(
      tester
          .state<ScrollableState>(privateKeyScrollable)
          .position
          .maxScrollExtent,
      greaterThan(0),
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('certificate toolbar action opens the certificate form', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('Keychain'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    await tester.tap(find.text('Certificate'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    expect(find.text('New Key'), findsOneWidget);
    expect(_keyFieldDecorator('Certificate'), findsOneWidget);
    expect(_keyField('Certificate'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
    'key drawer restores its dynamic name after another field changes',
    (tester) async {
      tester.view.physicalSize = const Size(1440, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
      await tester.tap(find.text('Keychain'));
      await tester.pump(const Duration(milliseconds: 250));
      await tester.tap(find.text('New key'));
      await tester.pump(const Duration(milliseconds: 250));

      final name = _keyField('Name');
      final publicKey = _keyField('Public key');
      expect(tester.widget<TextField>(name).controller?.text, 'SSH key');

      await tester.enterText(publicKey, 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA');
      await tester.pump();
      expect(tester.widget<TextField>(name).controller?.text, 'ED25519 key');

      await tester.enterText(name, 'Team deployment key');
      await tester.enterText(publicKey, 'ssh-rsa AAAAB3NzaC1yc2EAAAADAQAB');
      await tester.pump();
      expect(
        tester.widget<TextField>(name).controller?.text,
        'Team deployment key',
      );

      await tester.enterText(name, '');
      await tester.pump();
      expect(tester.widget<TextField>(name).controller?.text, isEmpty);

      await tester.enterText(publicKey, 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAA');
      await tester.pump();
      expect(tester.widget<TextField>(name).controller?.text, 'ED25519 key');

      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}

Finder _keyFieldDecorator(String label) {
  return find
      .ancestor(of: find.text(label), matching: find.byType(InputDecorator))
      .first;
}

Finder _keyField(String label) {
  return find
      .descendant(
        of: _keyFieldDecorator(label),
        matching: find.byType(TextField),
      )
      .first;
}
