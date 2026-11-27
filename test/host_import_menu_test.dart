import 'dart:io';

import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';

void main() {
  testWidgets('new host dropdown exposes group and host import flow', (
    tester,
  ) async {
    const picker = MethodChannel('plugins.flutter.io/file_selector');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    final calls = <MethodCall>[];
    messenger.setMockMethodCallHandler(picker, (call) async {
      calls.add(call);
      return null;
    });
    addTearDown(() => messenger.setMockMethodCallHandler(picker, null));
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await tester.tap(find.byTooltip('New host actions'));
    await tester.pump();

    expect(find.text('New group'), findsOneWidget);
    expect(find.text('Import…'), findsOneWidget);

    await tester.tap(find.text('Import…'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 150));

    expect(find.text('Import to Vaults'), findsOneWidget);
    expect(find.text('CSV file'), findsOneWidget);
    expect(find.text('OpenSSH'), findsOneWidget);
    expect(
      find.text('Sessions from a .mxtsessions or .ini file'),
      findsOneWidget,
    );
    expect(find.text('PuTTY'), findsOneWidget);
    expect(find.text('MobaXterm'), findsOneWidget);
    expect(find.text('SecureCRT'), findsOneWidget);

    await tester.tap(find.text('CSV file'));
    await tester.pump(const Duration(milliseconds: 200));
    await tester.tap(find.text('OpenSSH'));
    await tester.pump(const Duration(milliseconds: 200));

    expect(calls.map((call) => call.method), ['openFile', 'getDirectoryPath']);
    for (final call in calls) {
      expect(call.arguments['confirmButtonText'], isNull);
    }
    final fileTypes = calls.first.arguments['acceptedTypeGroups'] as List;
    expect(fileTypes.single['label'], isNull);
    expect(fileTypes.single['extensions'], ['csv']);
    expect(find.text('Import to Vaults'), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
    'host import preview shows filter results and retains selection',
    (tester) async {
      final folder = Directory.systemTemp.createTempSync(
        'nauterm-import-preview-',
      );
      addTearDown(() => folder.deleteSync(recursive: true));
      File('${folder.path}/config').writeAsStringSync(
        'Host example-preview\n  HostName example.test\n  User admin\n',
      );
      File('${folder.path}/oversized-key').openSync(mode: FileMode.write)
        ..truncateSync(1024 * 1024 + 1)
        ..closeSync();
      const picker = MethodChannel('plugins.flutter.io/file_selector');
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      messenger.setMockMethodCallHandler(picker, (call) async {
        expect(call.method, 'getDirectoryPath');
        return folder.path;
      });
      addTearDown(() => messenger.setMockMethodCallHandler(picker, null));

      await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
      await tester.tap(find.byTooltip('New host actions'));
      await tester.pump();
      await tester.tap(find.text('Import…'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 150));

      final preview = find.text('Import from OpenSSH');
      await tester.runAsync(() async {
        await tester.tap(find.text('OpenSSH'));
      });
      for (
        var attempt = 0;
        attempt < 100 && preview.evaluate().isEmpty;
        attempt++
      ) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 10)),
        );
        await tester.pump();
      }
      expect(preview, findsOneWidget);
      expect(find.text('example-preview'), findsOneWidget);
      expect(
        find.text('Unreadable or oversized files skipped (1).'),
        findsOneWidget,
      );
      expect(
        find.textContaining('Existing entries are reused'),
        findsOneWidget,
      );

      await tester.enterText(
        find.byWidgetPredicate(
          (widget) =>
              widget is TextField &&
              widget.decoration?.hintText == 'Filter hosts and keys',
        ),
        'no-preview-match',
      );
      await tester.pump();
      expect(find.text('No matching hosts or keys.'), findsOneWidget);
      expect(find.text('example-preview'), findsNothing);
      expect(find.text('Import 1'), findsOneWidget);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}
