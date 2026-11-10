import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/terminal/terminal_controller.dart';

void main() {
  setUp(() async {
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
  });

  for (final command in ['ssh', 'telnet']) {
    testWidgets('$command commands skip protocol selection', (tester) async {
      await _pumpApp(tester);
      final input = command == 'ssh'
          ? "ssh -p 2022 -l 'Admin' -- 'host.invalid'"
          : 'telnet -l Admin host.invalid 2022';
      await _submit(tester, input);
      expect(find.text('Protocol'), findsNothing);
      final controller = _remoteController(tester);
      final ssh = controller.sshProfile;
      final telnet = controller.telnetProfile;
      if (command == 'ssh') {
        expect(ssh!.host, 'host.invalid');
        expect(ssh.username, 'Admin');
        expect(ssh.port, 2022);
        expect(ssh.hostId, isNull);
        expect(telnet, isNull);
      } else {
        expect(telnet!.host, 'host.invalid');
        expect(telnet.username, 'Admin');
        expect(telnet.port, 2022);
        expect(telnet.hostId, isNull);
        expect(ssh, isNull);
      }
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }

  testWidgets('SSH without a login name opens the username step', (
    tester,
  ) async {
    await _pumpApp(tester);
    await _submit(tester, 'ssh host.invalid');
    expect(find.text('Protocol'), findsNothing);
    expect(find.text('Please enter username'), findsOneWidget);
    expect(find.text('Continue'), findsOneWidget);
    expect(find.text('Continue & Save'), findsNothing);
    expect(_remoteController(tester).sshProfile!.port, 22);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('an unprefixed address keeps each protocol default port', (
    tester,
  ) async {
    await _pumpApp(tester);
    await _submit(tester, 'Admin@host.invalid');
    expect(find.text('Protocol'), findsOneWidget);
    _expectPorts(tester, ssh: 22, mosh: 22, telnet: 23);
    await tester.tap(find.text('Telnet'));
    await tester.pump();
    await tester.tap(find.text('Connect'));
    await _pumpConnection(tester);
    final profile = _remoteController(tester).telnetProfile!;
    expect(profile.username, 'Admin');
    expect(profile.port, 23);
    expect(profile.hostId, isNull);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  for (final protocol in ['SSH', 'Mosh', 'Telnet']) {
    testWidgets('an explicit port is passed to the selected $protocol', (
      tester,
    ) async {
      await _pumpApp(tester);
      await _submit(tester, 'Admin@host.invalid -p 2022');
      expect(find.text('Protocol'), findsOneWidget);
      _expectPorts(tester, ssh: 2022, mosh: 2022, telnet: 2022);
      await tester.tap(find.text(protocol));
      await tester.pump();
      await tester.tap(find.text('Connect'));
      await _pumpConnection(tester);
      expect(find.text('Protocol'), findsNothing);
      final controller = _remoteController(tester);
      expect(controller.isMoshSession, protocol == 'Mosh');
      if (protocol == 'Telnet') {
        expect(controller.telnetProfile!.username, 'Admin');
        expect(controller.telnetProfile!.port, 2022);
      } else {
        expect(controller.sshProfile!.username, 'Admin');
        expect(controller.sshProfile!.port, 2022);
      }
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }

  testWidgets('the connect action uses recognized target information', (
    tester,
  ) async {
    await _pumpApp(tester);
    for (final input in [
      'ssh host.invalid -p',
      'ssh -i key admin@host.invalid',
    ]) {
      await tester.enterText(_search, input);
      await tester.pump();
      expect(tester.widget<TextField>(_search).onSubmitted, isNotNull);
    }
    await tester.enterText(_search, 'ssh host.invalid -p invalid');
    await tester.pump();
    expect(tester.widget<TextField>(_search).onSubmitted, isNotNull);
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await _pumpConnection(tester);
    expect(find.text('Protocol'), findsNothing);
    expect(_remoteController(tester).sshProfile!.host, 'host.invalid');
    expect(_remoteController(tester).sshProfile!.port, 22);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  for (final input in [
    'ssh -p 2022 -l Admin host.invalid',
    'telnet -l Admin host.invalid 2022',
    'Admin@host.invalid -p 2022',
  ]) {
    testWidgets('quick connect uses the same command routing: $input', (
      tester,
    ) async {
      await _pumpApp(tester);
      await tester.tap(find.byTooltip('Quick Connect'));
      await tester.pump(const Duration(milliseconds: 200));
      final search = find.byType(TextField).last;
      await tester.enterText(search, input);
      await tester.pump();
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await _pumpConnection(tester);
      expect(find.text('Quick Connect'), findsNothing);
      if (input.startsWith('Admin@')) {
        expect(find.text('Protocol'), findsOneWidget);
        _expectPorts(tester, ssh: 2022, mosh: 2022, telnet: 2022);
      } else {
        expect(find.text('Protocol'), findsNothing);
        final controller = _remoteController(tester);
        expect(controller.telnetProfile != null, input.startsWith('telnet'));
        expect(controller.sshProfile != null, input.startsWith('ssh'));
      }
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }
}

Finder get _search => find.descendant(
  of: find.byKey(const ValueKey('host-search-input')),
  matching: find.byType(TextField),
);

Future<void> _pumpApp(WidgetTester tester) async {
  tester.view.physicalSize = const Size(1200, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 300));
}

Future<void> _submit(WidgetTester tester, String input) async {
  await tester.enterText(_search, input);
  await tester.pump();
  await tester.testTextInput.receiveAction(TextInputAction.done);
  await _pumpConnection(tester);
}

Future<void> _pumpConnection(WidgetTester tester) async {
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 300));
  await tester.pump(const Duration(milliseconds: 300));
}

void _expectPorts(
  WidgetTester tester, {
  required int ssh,
  required int mosh,
  required int telnet,
}) {
  for (final (protocol, port) in [
    ('SSH', ssh),
    ('Mosh', mosh),
    ('Telnet', telnet),
  ]) {
    expect(find.text(protocol), findsOneWidget);
    expect(
      tester
          .widget<TextField>(
            find.byKey(ValueKey('host-protocol-port:$protocol')),
          )
          .controller!
          .text,
      port.toString(),
    );
  }
}

TerminalController _remoteController(WidgetTester tester) => tester
    .widgetList<AnimatedBuilder>(find.byType(AnimatedBuilder))
    .map((builder) => builder.animation)
    .whereType<TerminalController>()
    .where(
      (controller) =>
          controller.sshProfile != null || controller.telnetProfile != null,
    )
    .toSet()
    .single;
