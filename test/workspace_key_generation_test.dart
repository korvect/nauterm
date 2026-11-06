import 'dart:async';

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
