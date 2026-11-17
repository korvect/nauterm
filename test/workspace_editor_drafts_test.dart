import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';

void main() {
  testWidgets('unsaved status follows text edits and clears when reverted', (
    tester,
  ) async {
    await _pumpApp(tester);
    await _open(tester, 'New host');
    final status = find.byKey(const ValueKey('workspace-editor-unsaved'));
    final originalName = _text(tester, 'Name *');
    expect(status, findsNothing);
    await tester.enterText(_field('Name *'), 'Changed name');
    await tester.pump();
    expect(status, findsOneWidget);
    expect(find.text('Unsaved'), findsOneWidget);
    await _close(tester);
    await _open(tester, 'New host');
    expect(status, findsOneWidget);
    await tester.enterText(_field('Name *'), originalName);
    await tester.pump();
    expect(status, findsNothing);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('only form values mark the editor unsaved', (tester) async {
    await _pumpApp(tester);
    await _open(tester, 'New host');
    final status = find.byKey(const ValueKey('workspace-editor-unsaved'));
    final more = find.byKey(const ValueKey('protocol-show-more'));
    tester.widget<InkWell>(more).onTap!();
    await tester.pump();
    expect(status, findsNothing);
    final compatibility = find.byKey(
      const ValueKey('workspace-toggle:Compatibility mode'),
    );
    await tester.ensureVisible(compatibility);
    await tester.pump(const Duration(milliseconds: 200));
    await tester.tap(compatibility);
    await tester.pump();
    expect(status, findsOneWidget);
    await tester.tap(compatibility);
    await tester.pump();
    expect(status, findsNothing);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('restored automatic names continue following the address', (
    tester,
  ) async {
    await _pumpApp(tester);
    await _open(tester, 'New host');
    await tester.enterText(_field('Host'), 'first.example');
    await _close(tester);
    await _open(tester, 'New host');
    await tester.enterText(_field('Host'), 'second.example');
    expect(_text(tester, 'Name *'), contains('second.example'));
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('Escape closes the editor and keeps the draft', (tester) async {
    await _pumpApp(tester);
    await _open(tester, 'New host');
    await tester.enterText(_field('Host'), 'escape.example');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump(const Duration(milliseconds: 250));
    expect(find.text('New Host'), findsNothing);
    await _open(tester, 'New host');
    expect(_text(tester, 'Host'), 'escape.example');
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('nested editors keep drafts within their parent form', (
    tester,
  ) async {
    await _pumpApp(tester);
    await _open(tester, 'New host');
    await tester.enterText(_field('Name *'), 'Parent host');
    await tester.enterText(_field('Parent group'), 'child-group');
    await tester.pump();
    await tester.tap(find.text('Create group child-group'));
    await tester.pump();
    await tester.enterText(_field('Name *'), 'Edited child group');
    await _close(tester);
    expect(_text(tester, 'Name *'), 'Parent host');
    await _close(tester);
    await _open(tester, 'New host');
    expect(find.text('New Group'), findsNothing);
    await tester.enterText(_field('Parent group'), 'child-group');
    await tester.pump();
    await tester.tap(find.text('Create group child-group'));
    await tester.pump();
    expect(_text(tester, 'Name *'), 'Edited child group');
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('failed saves keep the editor editable and preserve the draft', (
    tester,
  ) async {
    await _pumpApp(tester);
    await _open(tester, 'New host');
    await tester.enterText(_field('Name *'), 'Retry later');
    await tester.tap(find.text('Save'));
    await tester.pump();
    expect(find.textContaining('Database is not ready'), findsOneWidget);
    await _close(tester);
    await _open(tester, 'New host');
    expect(_text(tester, 'Name *'), 'Retry later');
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('closing a host keeps raw input and control values', (
    tester,
  ) async {
    await _pumpApp(tester);
    await _open(tester, 'New host');
    await tester.enterText(_field('Name *'), 'Unfinished host');
    await tester.enterText(_field('Host'), 'draft.example');
    await tester.enterText(
      find.descendant(
        of: find.byKey(const ValueKey('protocol-port:SSH')),
        matching: find.byType(TextField),
      ),
      '70000',
    );
    await _close(tester);
    await _open(tester, 'New host');
    expect(_text(tester, 'Name *'), 'Unfinished host');
    expect(_text(tester, 'Host'), 'draft.example');
    expect(
      tester
          .widget<TextField>(
            find.descendant(
              of: find.byKey(const ValueKey('protocol-port:SSH')),
              matching: find.byType(TextField),
            ),
          )
          .controller!
          .text,
      '70000',
    );
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('host and group drafts survive sidebar changes separately', (
    tester,
  ) async {
    await _pumpApp(tester);
    await _open(tester, 'New host');
    await tester.enterText(_field('Name *'), 'Host draft');
    await _close(tester);
    await _open(tester, 'New group');
    await tester.enterText(_field('Name *'), 'Group draft');
    await _close(tester);
    await tester.tap(find.text('Keychain'));
    await tester.pump(const Duration(milliseconds: 250));
    await tester.tap(find.text('Hosts'));
    await tester.pump(const Duration(milliseconds: 250));
    await _open(tester, 'New host');
    expect(_text(tester, 'Name *'), 'Host draft');
    await _close(tester);
    await _open(tester, 'New group');
    expect(_text(tester, 'Name *'), 'Group draft');
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('discard clears the current draft without clearing other forms', (
    tester,
  ) async {
    await _pumpApp(tester);
    await _open(tester, 'New host');
    await tester.enterText(_field('Name *'), 'Keep this host');
    await _close(tester);
    await _open(tester, 'New group');
    await tester.enterText(_field('Name *'), 'Discard this group');
    await tester.tap(find.byTooltip('More').last);
    await tester.pump(const Duration(milliseconds: 150));
    await tester.tap(find.text('Discard changes'));
    await tester.pump(const Duration(milliseconds: 250));
    await _open(tester, 'New group');
    expect(_text(tester, 'Name *'), isEmpty);
    await _close(tester);
    await _open(tester, 'New host');
    expect(_text(tester, 'Name *'), 'Keep this host');
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('key and certificate forms keep independent drafts', (
    tester,
  ) async {
    await _pumpApp(tester);
    await tester.tap(find.text('Keychain'));
    await tester.pump(const Duration(milliseconds: 250));
    await _open(tester, 'New key');
    await tester.enterText(_field('Name'), 'Key draft');
    await tester.enterText(_field('Private key *'), 'unfinished private key');
    await _close(tester);
    await _open(tester, 'Certificate');
    await tester.enterText(_field('Name'), 'Certificate draft');
    await tester.enterText(_field('Certificate'), 'unfinished certificate');
    await _close(tester);
    await _open(tester, 'New key');
    expect(_text(tester, 'Name'), 'Key draft');
    expect(_text(tester, 'Private key *'), 'unfinished private key');
    await _close(tester);
    await _open(tester, 'Certificate');
    expect(_text(tester, 'Name'), 'Certificate draft');
    expect(_text(tester, 'Certificate'), 'unfinished certificate');
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('drafts are released with the workspace', (tester) async {
    await _pumpApp(tester);
    await _open(tester, 'New host');
    await tester.enterText(_field('Name *'), 'Only in memory');
    await _close(tester);
    await tester.pumpWidget(const SizedBox.shrink());
    await _pumpApp(tester);
    await _open(tester, 'New host');
    expect(_text(tester, 'Name *'), isNot('Only in memory'));
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

Future<void> _pumpApp(WidgetTester tester) async {
  tester.view.physicalSize = const Size(1440, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
  await tester.pump(const Duration(milliseconds: 250));
}

Future<void> _open(WidgetTester tester, String action) async {
  if (action == 'New group') {
    await tester.tap(find.byTooltip('New host actions'));
    await tester.pump(const Duration(milliseconds: 150));
  }
  await tester.tap(find.text(action).first);
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 250));
}

Future<void> _close(WidgetTester tester) async {
  await tester.tap(find.byTooltip('Close').last);
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 250));
}

Finder _field(String label) => find.descendant(
  of: find.ancestor(
    of: find.text(label),
    matching: find.byType(InputDecorator),
  ),
  matching: find.byType(TextField),
);

String _text(WidgetTester tester, String label) =>
    tester.widget<TextField>(_field(label)).controller!.text;
