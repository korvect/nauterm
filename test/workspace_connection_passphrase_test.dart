import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

const _savedProfile = SshConnectionProfile(
  host: '127.0.0.1',
  port: 22,
  username: 'tester',
  knownHostsPath: '/tmp/unused-test-known-hosts',
  privateKey: 'encrypted-test-key',
  certificate: 'test-certificate',
  passphrase: 'outdated-saved-passphrase',
);

void main() {
  setUp(() async {
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
  });

  for (final inDrawer in [false, true]) {
    for (final height in [280.0, 700.0]) {
      testWidgets(
        'missing username comes before unlocking at $height px, drawer=$inDrawer',
        (tester) async {
          final controller = _ConnectionController(
            profile: _savedProfile.copyWith(username: '  '),
          );
          addTearDown(controller.dispose);
          await _pumpPage(
            tester,
            controller,
            inDrawer: inDrawer,
            height: height,
          );
          expect(find.text('Please enter username'), findsOneWidget);
          expect(find.text('Please Enter passphrase for key'), findsNothing);
          expect(find.text('Public Key'), findsNothing);
          final field = find.byType(TextField);
          expect(tester.widget<TextField>(field).obscureText, isFalse);
          await tester.tap(find.text('Continue'));
          await tester.pump();
          expect(controller.attempts, isEmpty);
          await tester.enterText(field, '   ');
          await tester.testTextInput.receiveAction(TextInputAction.go);
          await tester.pump();
          expect(controller.attempts, isEmpty);
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox.shrink());
        },
      );

      testWidgets('private-key unlock fits $height px, drawer=$inDrawer', (
        tester,
      ) async {
        final controller = _ConnectionController();
        addTearDown(controller.dispose);
        await _pumpPage(tester, controller, inDrawer: inDrawer, height: height);
        expect(find.text('Please Enter passphrase for key'), findsOneWidget);
        expect(find.text('Password'), findsNothing);
        expect(find.text('Public Key'), findsNothing);
        expect(find.text('Continue & Save'), findsNothing);
        final field = find.byType(TextField);
        expect(tester.widget<TextField>(field).obscureText, isTrue);
        await tester.ensureVisible(field);
        await tester.enterText(field, 'session-passphrase');
        await tester.pump();
        await tester.ensureVisible(find.byIcon(Icons.visibility_off_rounded));
        await tester.tap(find.byIcon(Icons.visibility_off_rounded));
        await tester.pump();
        expect(tester.widget<TextField>(field).obscureText, isFalse);
        final footer = find.byKey(const ValueKey('connection-page-footer'));
        expect(
          find.ancestor(of: footer, matching: find.byType(Scrollable)),
          findsNothing,
        );
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      });
    }
  }

  for (final changedHost in [false, true]) {
    testWidgets(
      'entered username survives credential steps for the same host, changed=$changedHost',
      (tester) async {
        var savedProfile = _savedProfile.copyWith(username: '');
        final controller = _ConnectionController(profile: savedProfile);
        addTearDown(controller.dispose);
        await _pumpPage(
          tester,
          controller,
          inDrawer: true,
          reloadProfile: () => savedProfile,
        );
        await tester.enterText(find.byType(TextField), '  runtime-user  ');
        await tester.testTextInput.receiveAction(TextInputAction.go);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 200));
        expect(controller.attempts.single.profile.username, 'runtime-user');
        expect(find.text('Please Enter passphrase for key'), findsOneWidget);
        if (changedHost) {
          savedProfile = savedProfile.copyWith(host: 'other.example.test');
        }
        await tester.enterText(find.byType(TextField), 'session-passphrase');
        await tester.pump();
        await tester.tap(find.text('Continue'));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 200));
        if (changedHost) {
          expect(controller.attempts.last.profile.username, isEmpty);
          expect(find.text('Please enter username'), findsOneWidget);
        } else {
          expect(controller.attempts.last.profile.username, 'runtime-user');
          expect(find.text('Please enter username'), findsNothing);
          await tester.tap(find.text('Continue'));
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 200));
          expect(controller.attempts.last.profile.username, 'runtime-user');
          expect(
            controller.attempts.last.profile.passphrase,
            'session-passphrase',
          );
        }
        expect(savedProfile.username, isEmpty);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  for (final changedKey in [false, true]) {
    testWidgets(
      'host-key continuation preserves unlock only for the same key, changed=$changedKey',
      (tester) async {
        final controller = _ConnectionController();
        addTearDown(controller.dispose);
        var savedProfile = _savedProfile;
        await _pumpPage(
          tester,
          controller,
          inDrawer: true,
          reloadProfile: () => savedProfile,
        );
        await tester.tap(find.text('Continue'));
        await tester.pump();
        expect(controller.attempts, isEmpty);
        await tester.enterText(find.byType(TextField), 'session-passphrase');
        await tester.pump();
        await tester.tap(find.text('Continue'));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 200));
        expect(
          controller.attempts.single.profile.passphrase,
          'session-passphrase',
        );
        expect(
          controller.attempts.single.profile.certificate,
          'test-certificate',
        );
        expect(find.text('Please Enter passphrase for key'), findsNothing);

        if (changedKey) {
          savedProfile = _savedProfile.copyWith(
            privateKey: 'replacement-key',
            passphrase: null,
          );
        }
        await tester.tap(find.text('Continue'));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 200));
        expect(controller.attempts.last.trust, SshHostKeyTrustMode.acceptOnce);
        expect(
          controller.attempts.last.profile.passphrase,
          changedKey ? null : 'session-passphrase',
        );
        expect(find.text('Password'), findsWidgets);
        expect(find.text('Public Key'), findsOneWidget);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  testWidgets('FIDO2 PIN keeps its own authentication form', (tester) async {
    final controller = _ConnectionController(method: 'fido2');
    addTearDown(controller.dispose);
    await _pumpPage(tester, controller);
    expect(find.text('Please Enter passphrase for key'), findsNothing);
    expect(find.text('PIN'), findsOneWidget);
    expect(find.text('Public Key'), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
    'rejected key shows host confirmation then password without unlocking',
    (tester) async {
      final controller = _ConnectionController();
      addTearDown(controller.dispose);
      controller._events = [
        _event(TerminalConnectionEventKind.connectStart),
        _event(TerminalConnectionEventKind.authKeyRejected),
        _event(TerminalConnectionEventKind.hostKeyUnknown),
        _event(TerminalConnectionEventKind.error),
      ];
      controller._phase = TerminalConnectionPhase.hostKey;
      await _pumpPage(tester, controller, inDrawer: true);
      expect(find.text('Please Enter passphrase for key'), findsNothing);
      expect(find.byType(TextField), findsNothing);
      await tester.tap(find.text('Continue'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(controller.attempts.single.trust, SshHostKeyTrustMode.acceptOnce);
      expect(find.text('Please Enter passphrase for key'), findsNothing);
      expect(find.text('Password'), findsWidgets);
      expect(find.text('Public Key'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}

Future<void> _pumpPage(
  WidgetTester tester,
  TerminalController controller, {
  bool inDrawer = false,
  double height = 700,
  SshConnectionProfile? Function()? reloadProfile,
}) async {
  tester.view.physicalSize = const Size(900, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Center(
          child: SizedBox(
            width: 360,
            height: height,
            child: buildTerminalConnectionPageForTesting(
              controller,
              inDrawer: inDrawer,
              reloadProfile: reloadProfile,
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

class _ConnectionController extends TerminalController {
  _ConnectionController({
    String method = 'public_key',
    SshConnectionProfile profile = _savedProfile,
  }) : super.ssh(
         host: _savedProfile.host,
         port: _savedProfile.port,
         username: _savedProfile.username,
         knownHostsPath: _savedProfile.knownHostsPath,
         driver: MemoryTerminalDriver(columns: 80, rows: 24),
       ) {
    _profile = profile;
    _events = [
      _event(
        TerminalConnectionEventKind.authPassphraseRequired,
        method: method,
      ),
    ];
  }

  SshConnectionProfile _profile = _savedProfile;
  late List<TerminalConnectionEvent> _events;
  TerminalConnectionPhase _phase = TerminalConnectionPhase.authentication;
  final attempts =
      <({SshConnectionProfile profile, SshHostKeyTrustMode trust})>[];

  @override
  SshConnectionProfile get sshProfile => _profile;
  @override
  List<TerminalConnectionEvent> get connectionEvents => _events;
  @override
  TerminalConnectionStatus get connectionStatus =>
      TerminalConnectionStatus(phase: _phase);

  @override
  bool reconnectSsh({
    SshConnectionProfile? profile,
    String? host,
    int? port,
    String? username,
    int? identityId,
    Object? password,
    Object? privateKey,
    Object? certificate,
    Object? passphrase,
    Object? fido2Pin,
    String? moshServerCommand,
    SshHostKeyTrustMode hostKeyTrustMode = SshHostKeyTrustMode.strict,
  }) {
    final next = profile ?? _profile;
    final needsUnlock =
        _profile.username.trim().isEmpty && username?.trim().isNotEmpty == true;
    _profile = next.copyWith(
      host: host,
      port: port,
      username: username,
      identityId: identityId,
      privateKey: privateKey is String ? privateKey : next.privateKey,
      certificate: certificate is String ? certificate : next.certificate,
      passphrase: passphrase is String ? passphrase : next.passphrase,
    );
    attempts.add((profile: _profile, trust: hostKeyTrustMode));
    final needsHostKey = hostKeyTrustMode == SshHostKeyTrustMode.strict;
    _phase = needsHostKey && !needsUnlock
        ? TerminalConnectionPhase.hostKey
        : TerminalConnectionPhase.authentication;
    _events = [
      _event(TerminalConnectionEventKind.connectStart),
      _event(
        needsUnlock
            ? TerminalConnectionEventKind.authPassphraseRequired
            : needsHostKey
            ? TerminalConnectionEventKind.hostKeyUnknown
            : TerminalConnectionEventKind.authFailed,
        method: needsUnlock ? 'public_key' : null,
      ),
    ];
    notifyListeners();
    return true;
  }
}

TerminalConnectionEvent _event(
  TerminalConnectionEventKind kind, {
  String? method,
}) => TerminalConnectionEvent(
  kind: kind,
  message: 'Test connection event',
  timestamp: DateTime.utc(2026, 11, 3),
  method: method,
  fingerprint: kind == TerminalConnectionEventKind.hostKeyUnknown
      ? 'SHA256:test-host-key'
      : null,
);
