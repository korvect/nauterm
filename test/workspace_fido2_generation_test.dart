import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/terminal/terminal_ffi.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

const _device = Fido2DeviceInfo(
  id: 'test-device',
  name: 'Test security key',
  vendorId: 0,
  productId: 0,
  hasPin: false,
);

const _generated = Fido2GeneratedKey(
  privateKey: 'generated-private-key',
  publicKey: 'generated-public-key',
  keyType: 'ecdsa',
  application: 'ssh:',
);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
  });

  testWidgets('closing keeps a generated key available to retry saving', (
    tester,
  ) async {
    var generations = 0;
    var closes = 0;
    final attempts = <KeyEntry>[];
    final editor = buildFido2KeyEditorForTesting(
      listDevices: () async => [_device],
      generateKey: () async {
        generations++;
        return _generated;
      },
      onSave: (key) async {
        attempts.add(key);
        if (attempts.length == 1) {
          throw const NautermDataException('database failure {error}');
        }
      },
      onClose: () => closes++,
    );
    await tester.pumpWidget(_app(editor));
    await _startGeneration(tester);
    expect(generations, 1);
    expect(attempts, hasLength(1));
    expect(find.text('Generate'), findsNothing);
    expect(find.textContaining('database failure {error}'), findsOneWidget);

    await tester.tap(find.byTooltip('Close'));
    await tester.pumpAndSettle();
    expect(closes, 1);
    await tester.pumpWidget(_app(const SizedBox.shrink()));
    await tester.pumpWidget(_app(editor));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Retry save'));
    await tester.pumpAndSettle();
    expect(generations, 1);
    expect(attempts, hasLength(2));
    expect(identical(attempts.first, attempts.last), isTrue);
    expect(attempts.last.publicKey, _generated.publicKey);
  });

  testWidgets('generation survives leaving and reopening the drawer', (
    tester,
  ) async {
    final generation = Completer<Fido2GeneratedKey>();
    final saved = <KeyEntry>[];
    final editor = buildFido2KeyEditorForTesting(
      listDevices: () async => [_device],
      generateKey: () => generation.future,
      onSave: (key) async => saved.add(key),
      onClose: () {},
    );
    await tester.pumpWidget(_app(editor));
    await _startGeneration(tester);
    await tester.pumpWidget(_app(const SizedBox.shrink()));
    generation.complete(_generated);
    await tester.pumpAndSettle();

    await tester.pumpWidget(_app(editor));
    await tester.pumpAndSettle();
    expect(saved, isEmpty);
    await tester.tap(find.text('Retry save'));
    await tester.pumpAndSettle();
    expect(saved.single.privateKey, _generated.privateKey);
  });
}

Widget _app(Widget editor) => MaterialApp(home: Scaffold(body: editor));

Future<void> _startGeneration(WidgetTester tester) async {
  await tester.pumpAndSettle();
  await tester.tap(find.text('Continue'));
  await tester.pumpAndSettle();
  await tester.tap(find.text('Generate'));
  await tester.pump();
}
