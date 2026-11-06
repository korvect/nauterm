import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

import 'support/ssh_key_metadata.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('com.korvect.nauterm/file_drop'),
          (_) async => null,
        );
  });
  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('com.korvect.nauterm/file_drop'),
          null,
        );
  });

  for (final relatedCredential in [false, true]) {
    for (final importFile in [false, true]) {
      testWidgets(
        'encrypted key creation saves passphrase, related=$relatedCredential, import=$importFile',
        (tester) async {
          final saved = Completer<KeyEntry>();
          await _pumpNewKeyEditor(tester, relatedCredential, saved);
          expect(find.text('Passphrase'), findsNothing);
          final privateKey = openSshHeader('aes256-ctr');
          if (importFile) {
            if (relatedCredential) {
              await tester.tap(find.text('Import'));
              await tester.pump();
            }
            await _importKeyFile(tester, privateKey);
          } else {
            await tester.enterText(_privateKeyField(), privateKey);
            await tester.pump();
            await tester.pumpAndSettle();
            expect(
              tester.widget<TextField>(_privateKeyField()).focusNode!.hasFocus,
              isTrue,
            );
          }
          final passphrase = _passphraseField();
          expect(passphrase, findsOneWidget);
          expect(tester.widget<TextField>(passphrase).obscureText, isTrue);
          expect(find.text('Security'), findsNothing);
          expect(
            tester.getTopLeft(find.text('Name')).dy,
            lessThan(tester.getTopLeft(find.text('Passphrase')).dy),
          );
          expect(
            tester.getTopLeft(find.text('Passphrase')).dy,
            lessThan(tester.getTopLeft(find.text('Private key *')).dy),
          );
          await tester.enterText(passphrase, '  imported passphrase  ');
          await tester.tap(find.text('Save'));
          await tester.pump();
          expect(saved.isCompleted, isTrue);
          final key = await saved.future;
          expect(key.privateKey, privateKey);
          expect(key.passphrase, '  imported passphrase  ');
          await tester.pumpWidget(const SizedBox.shrink());
        },
      );
    }
    for (final savePassphrase in [false, true]) {
      testWidgets(
        'generated key respects Save passphrase, related=$relatedCredential, save=$savePassphrase',
        (tester) async {
          final saved = Completer<KeyEntry>();
          await _pumpGenerator(tester, relatedCredential, saved);
          await tester.enterText(_passphraseField(), '  key passphrase  ');
          if (savePassphrase) {
            await tester.tap(find.text('Save passphrase'));
            await tester.pump();
          }
          final key = await _generateAndSave(tester, saved);
          await tester.pump();
          expect(key.privateKey, contains('BEGIN OPENSSH PRIVATE KEY'));
          expect(key.publicKey, startsWith('ssh-ed25519 '));
          expect(
            key.passphrase,
            savePassphrase ? '  key passphrase  ' : isNull,
          );
          await tester.pumpWidget(const SizedBox.shrink());
        },
      );
    }
    testWidgets(
      'Save passphrase never persists an empty value, related=$relatedCredential',
      (tester) async {
        final saved = Completer<KeyEntry>();
        await _pumpGenerator(tester, relatedCredential, saved);
        await tester.tap(find.text('Save passphrase'));
        await tester.pump();
        final key = await _generateAndSave(tester, saved);
        await tester.pump();
        expect(key.passphrase, isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }
  testWidgets('new encrypted key can be saved without a passphrase', (
    tester,
  ) async {
    final saved = Completer<KeyEntry>();
    await _pumpNewKeyEditor(tester, false, saved);
    await tester.enterText(_privateKeyField(), openSshHeader('aes256-ctr'));
    await tester.pump();
    expect(_passphraseField(), findsOneWidget);
    await tester.tap(find.text('Save'));
    await tester.pump();
    expect((await saved.future).passphrase, isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });
  testWidgets('detected passphrase field animates its layout in and out', (
    tester,
  ) async {
    final saved = Completer<KeyEntry>();
    await _pumpNewKeyEditor(tester, false, saved);
    await tester.enterText(_privateKeyField(), openSshHeader('none'));
    await tester.pumpAndSettle();

    final animation = find.byKey(const ValueKey('key-passphrase-animation'));
    final privateKeyLabel = find.text('Private key *');
    final initialHeight = tester.getSize(animation).height;
    final initialPrivateKeyTop = tester.getTopLeft(privateKeyLabel).dy;
    expect(initialHeight, 0);

    await tester.enterText(_privateKeyField(), openSshHeader('aes256-ctr'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 90));
    final expandingHeight = tester.getSize(animation).height;
    final expandingPrivateKeyTop = tester.getTopLeft(privateKeyLabel).dy;
    expect(expandingHeight, greaterThan(0));
    expect(expandingPrivateKeyTop, greaterThan(initialPrivateKeyTop));
    expect(find.text('Passphrase'), findsOneWidget);

    await tester.pump(const Duration(milliseconds: 120));
    final expandedHeight = tester.getSize(animation).height;
    expect(expandedHeight, greaterThan(expandingHeight));

    await tester.enterText(_privateKeyField(), openSshHeader('none'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 90));
    final collapsingHeight = tester.getSize(animation).height;
    expect(find.text('Passphrase'), findsNothing);
    expect(collapsingHeight, greaterThan(0));
    expect(collapsingHeight, lessThan(expandedHeight));

    await tester.pump(const Duration(milliseconds: 120));
    expect(tester.getSize(animation).height, 0);
    expect(tester.getTopLeft(privateKeyLabel).dy, initialPrivateKeyTop);
    await tester.pumpWidget(const SizedBox.shrink());
  });
  testWidgets(
    'replacing a new encrypted key with an unencrypted key hides its passphrase',
    (tester) async {
      final saved = Completer<KeyEntry>();
      await _pumpNewKeyEditor(tester, false, saved);
      await tester.enterText(_privateKeyField(), openSshHeader('aes256-ctr'));
      await tester.pump();
      await tester.enterText(_passphraseField(), 'stale passphrase');
      final privateKey = openSshHeader('none');
      await tester.enterText(_privateKeyField(), privateKey);
      await tester.pump();
      expect(find.text('Passphrase'), findsNothing);
      expect(
        tester.widget<TextField>(_privateKeyField()).focusNode!.hasFocus,
        isTrue,
      );
      await tester.tap(find.text('Save'));
      await tester.pump();
      final key = await saved.future;
      expect(key.privateKey, privateKey);
      expect(key.passphrase, isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
  for (final passphrase in ['updated passphrase', '', '  exact spaces  ']) {
    testWidgets(
      'encrypted key passphrase can be edited or cleared: "$passphrase"',
      (tester) async {
        final saved = Completer<KeyEntry>();
        await _pumpKeyEditor(
          tester,
          saved,
          openSshHeader('aes256-ctr'),
          'old saved passphrase',
        );
        final field = _passphraseField();
        expect(
          tester.widget<TextField>(field).controller!.text,
          'old saved passphrase',
        );
        expect(tester.widget<TextField>(field).obscureText, isTrue);
        expect(find.text('Save passphrase'), findsNothing);
        expect(find.text('Security'), findsNothing);
        expect(
          tester.getTopLeft(find.text('Name')).dy,
          lessThan(tester.getTopLeft(find.text('Passphrase')).dy),
        );
        expect(
          tester.getTopLeft(find.text('Passphrase')).dy,
          lessThan(tester.getTopLeft(find.text('Private key *')).dy),
        );
        await tester.ensureVisible(field);
        await tester.enterText(field, passphrase);
        await tester.tap(find.text('Save'));
        await tester.pump();
        expect(saved.isCompleted, isTrue);
        expect(
          (await saved.future).passphrase,
          passphrase.isEmpty ? isNull : passphrase,
        );
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }
  testWidgets('encrypted key without a saved passphrase still offers editing', (
    tester,
  ) async {
    final saved = Completer<KeyEntry>();
    await _pumpKeyEditor(tester, saved, openSshHeader('aes256-ctr'), null);
    final field = _passphraseField();
    expect(tester.widget<TextField>(field).controller!.text, isEmpty);
    await tester.ensureVisible(field);
    await tester.enterText(field, 'new saved passphrase');
    await tester.tap(find.text('Save'));
    await tester.pump();
    expect((await saved.future).passphrase, 'new saved passphrase');
    await tester.pumpWidget(const SizedBox.shrink());
  });
  testWidgets(
    'unencrypted key hides the passphrase field and clears stale saved values',
    (tester) async {
      final saved = Completer<KeyEntry>();
      await _pumpKeyEditor(tester, saved, openSshHeader('none'), 'stale value');
      expect(find.text('Passphrase'), findsNothing);
      await tester.tap(find.text('Save'));
      await tester.pump();
      expect((await saved.future).passphrase, isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}

Future<void> _pumpNewKeyEditor(
  WidgetTester tester,
  bool relatedCredential,
  Completer<KeyEntry> saved,
) async {
  await tester.binding.setSurfaceSize(const Size(500, 1000));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: buildKeyEditorForTesting(
          relatedCredential: relatedCredential,
          onSave: (key) async => saved.complete(key),
        ),
      ),
    ),
  );
  await tester.pump();
}

Future<void> _importKeyFile(WidgetTester tester, String privateKey) async {
  final directory = Directory.systemTemp.createTempSync('nauterm-key-import-');
  addTearDown(() => directory.deleteSync(recursive: true));
  final file = File('${directory.path}/test-key')
    ..writeAsStringSync(privateKey);
  await tester.binding.defaultBinaryMessenger.handlePlatformMessage(
    'com.korvect.nauterm/file_drop',
    const StandardMethodCodec().encodeMethodCall(
      MethodCall('filesDropped', {
        'paths': [file.path],
      }),
    ),
    null,
  );
  await tester.pump();
  for (
    var attempt = 0;
    _passphraseField().evaluate().isEmpty && attempt < 50;
    attempt++
  ) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 10)),
    );
    await tester.pump();
  }
  expect(_passphraseField(), findsOneWidget);
  await tester.pumpAndSettle();
}

Finder _privateKeyField() => find.descendant(
  of: find.ancestor(
    of: find.text('Private key *'),
    matching: find.byType(InputDecorator),
  ),
  matching: find.byType(TextField),
);

Future<void> _pumpGenerator(
  WidgetTester tester,
  bool relatedCredential,
  Completer<KeyEntry> saved,
) async {
  await tester.binding.setSurfaceSize(const Size(500, 1000));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: buildKeyEditorForTesting(
          generate: true,
          relatedCredential: relatedCredential,
          onSave: (key) async => saved.complete(key),
          generateKey: (passphrase) async {
            expect(passphrase, anyOf('', '  key passphrase  '));
            return (
              privateKey: '-----BEGIN OPENSSH PRIVATE KEY-----\ntest-key',
              publicKey: 'ssh-ed25519 test-key',
            );
          },
        ),
      ),
    ),
  );
  await tester.pump();
  if (relatedCredential) {
    await tester.tap(find.text('Generate'));
    await tester.pump();
  }
  await tester.tap(find.text('ED25519'));
  await tester.pump();
  expect(find.text('Save passphrase'), findsOneWidget);
}

Finder _passphraseField() => find.descendant(
  of: find.ancestor(
    of: find.text('Passphrase'),
    matching: find.byType(InputDecorator),
  ),
  matching: find.byType(TextField),
);

Future<KeyEntry> _generateAndSave(
  WidgetTester tester,
  Completer<KeyEntry> saved,
) async {
  await tester.tap(find.text('Generate & save'));
  await tester.pump();
  expect(saved.isCompleted, isTrue);
  return saved.future;
}

Future<void> _pumpKeyEditor(
  WidgetTester tester,
  Completer<KeyEntry> saved,
  String privateKey,
  String? passphrase,
) async {
  await tester.binding.setSurfaceSize(const Size(500, 1000));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: buildKeyEditorForTesting(
          initial: KeyEntry(
            id: 1,
            name: 'Existing key',
            privateKey: privateKey,
            passphrase: passphrase,
          ),
          onSave: (key) async => saved.complete(key),
        ),
      ),
    ),
  );
  await tester.pump();
}
