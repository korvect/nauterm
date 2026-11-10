import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

const _challenge = SshAuthenticationChallenge(
  id: 41,
  name: 'Server authentication',
  instructions: 'Answer the server prompts',
  prompts: [
    SshAuthenticationPrompt(prompt: 'Account:', echo: true),
    SshAuthenticationPrompt(prompt: 'Verification code:', echo: false),
  ],
);

void main() {
  setUp(() async {
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
  });

  test('native challenge payload survives timestamp stamping', () {
    final event = TerminalConnectionEvent.fromJson({
      'kind': 'auth_interactive_request',
      'message': 'SSH authentication requires a response.',
      'challenge': {
        'id': 17,
        'name': 'PAM',
        'instructions': 'Provide both responses.',
        'prompts': [
          {'prompt': 'Account: ', 'echo': true},
          {'prompt': 'OTP: ', 'echo': false},
        ],
      },
    }).copyWith(timestamp: DateTime.utc(2026));
    expect(event.kind, TerminalConnectionEventKind.authInteractiveRequest);
    expect(event.challenge!.id, 17);
    expect(event.challenge!.name, 'PAM');
    expect(event.challenge!.prompts[0].echo, isTrue);
    expect(event.challenge!.prompts[1].echo, isFalse);
    expect(event.challenge!.prompts[1].prompt, 'OTP: ');
    expect(event.logLine, isNot(contains('Provide both responses.')));
  });

  for (final inDrawer in [false, true]) {
    for (final height in [280.0, 700.0]) {
      testWidgets(
        'server challenges and subsequent rounds fit at $height, drawer=$inDrawer',
        (tester) async {
          final driver = _InteractiveDriver();
          final controller = _controller(driver);
          addTearDown(controller.dispose);
          await _pump(tester, controller, inDrawer, height);
          expect(find.text(_challenge.name), findsOneWidget);
          expect(find.text(_challenge.instructions), findsOneWidget);
          expect(find.text('Verification code:'), findsOneWidget);
          expect(find.text('Interactive authentication'), findsNothing);
          expect(find.text('Continue & Save'), findsNothing);
          expect(find.text('Public Key'), findsNothing);
          final fields = find.byType(TextField);
          expect(fields, findsNWidgets(2));
          expect(tester.widget<TextField>(fields.at(0)).obscureText, isFalse);
          expect(tester.widget<TextField>(fields.at(1)).obscureText, isTrue);
          await tester.ensureVisible(fields.at(0));
          await tester.enterText(fields.at(0), ' account ');
          await tester.ensureVisible(fields.at(1));
          await tester.enterText(fields.at(1), '123456');
          await tester.tap(find.text('Continue'));
          await tester.pump();
          expect(driver.responses.single, [' account ', '123456']);
          expect(controller.authenticationChallenge, isNull);
          expect(controller.sshProfile!.password, isNull);
          expect(
            controller.connectionEvents.map((event) => event.logLine).join(),
            isNot(contains('123456')),
          );
          expect(
            controller.respondToAuthentication(41, ['duplicate']),
            isFalse,
          );

          driver.addChallenge(
            const SshAuthenticationChallenge(
              id: 42,
              name: '',
              instructions: 'One more code',
              prompts: [
                SshAuthenticationPrompt(prompt: 'Next code:', echo: false),
              ],
            ),
          );
          controller.poll();
          await tester.pump();
          await tester.pump(const Duration(milliseconds: 200));
          expect(find.text('Interactive authentication'), findsNothing);
          expect(find.byType(TextField), findsOneWidget);
          expect(
            tester.widget<TextField>(find.byType(TextField)).controller!.text,
            isEmpty,
          );
          expect(controller.respondToAuthentication(41, ['stale']), isFalse);
          await tester.ensureVisible(find.byType(TextField));
          await tester.enterText(find.byType(TextField), '654321');
          await tester.testTextInput.receiveAction(TextInputAction.go);
          await tester.pump();
          expect(driver.responses.last, ['654321']);
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox.shrink());
        },
      );
    }
  }

  testWidgets('unnamed challenge keeps the server password and code prompts', (
    tester,
  ) async {
    final driver = _InteractiveDriver()..events.clear();
    driver.addChallenge(
      const SshAuthenticationChallenge(
        id: 43,
        name: '',
        instructions: '',
        prompts: [
          SshAuthenticationPrompt(prompt: 'Password:', echo: false),
          SshAuthenticationPrompt(prompt: 'Verification code:', echo: false),
        ],
      ),
    );
    final controller = _controller(driver);
    addTearDown(controller.dispose);
    await _pump(tester, controller, true, 280);
    expect(find.text('Interactive authentication'), findsNothing);
    expect(find.text('Password:'), findsOneWidget);
    expect(find.text('Verification code:'), findsOneWidget);
    expect(find.byType(TextField), findsNWidgets(2));
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
    'showing logs preserves answers without adding them to diagnostics',
    (tester) async {
      final driver = _InteractiveDriver();
      final controller = _controller(driver);
      addTearDown(controller.dispose);
      await _pump(tester, controller, true, 700);
      await tester.enterText(find.byType(TextField).at(0), 'account');
      await tester.enterText(find.byType(TextField).at(1), '123456');
      await tester.tap(find.text('Show logs'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.byType(TextField), findsNothing);
      expect(find.textContaining('123456'), findsNothing);
      await tester.tap(find.text('Hide logs'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(
        tester.widget<TextField>(find.byType(TextField).at(1)).controller!.text,
        '123456',
      );
      await tester.tap(find.text('Continue'));
      await tester.pump();
      expect(driver.responses.single, ['account', '123456']);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets(
    'closing a challenge cancels authentication without saving an answer',
    (tester) async {
      final driver = _InteractiveDriver();
      final controller = _controller(driver);
      addTearDown(controller.dispose);
      await _pump(tester, controller, true, 700);
      await tester.enterText(find.byType(TextField).at(1), 'discard-this-code');
      await tester.tap(find.text('Close'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(driver.responses, [null]);
      expect(controller.authenticationChallenge, isNull);
      expect(
        controller.connectionEvents.map((event) => event.logLine).join(),
        isNot(contains('discard-this-code')),
      );
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets('a transport error during a challenge displays failure', (
    tester,
  ) async {
    final driver = _InteractiveDriver();
    final controller = _controller(driver);
    addTearDown(controller.dispose);
    await _pump(tester, controller, true, 700);
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.error,
        message: 'Server disconnected during authentication.',
      ),
    );
    controller.poll();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(controller.connectionStatus.phase, TerminalConnectionPhase.failed);
    expect(controller.authenticationChallenge, isNull);
    expect(find.byType(TextField), findsNothing);
    expect(find.textContaining('Server disconnected'), findsWidgets);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('an information-only challenge can continue with no fields', (
    tester,
  ) async {
    final driver = _InteractiveDriver()..events.clear();
    driver.addChallenge(
      const SshAuthenticationChallenge(
        id: 9,
        name: 'Notice',
        instructions: 'Continue to authenticate.',
        prompts: [],
      ),
    );
    final controller = _controller(driver);
    addTearDown(controller.dispose);
    await _pump(tester, controller, true, 700);
    expect(find.byType(TextField), findsNothing);
    await tester.tap(find.text('Continue'));
    await tester.pump();
    expect(driver.responses.single, isEmpty);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

TerminalController _controller(_InteractiveDriver driver) =>
    TerminalController.ssh(
      host: 'example.test',
      port: 22,
      username: 'tester',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );

Future<void> _pump(
  WidgetTester tester,
  TerminalController controller,
  bool inDrawer,
  double height,
) async {
  controller.poll();
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
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

class _InteractiveDriver extends MemoryTerminalDriver
    implements TerminalAuthenticationResponder {
  _InteractiveDriver() : super(columns: 80, rows: 24) {
    addChallenge(_challenge);
  }
  final events = <TerminalConnectionEvent>[];
  final responses = <List<String>?>[];
  void addChallenge(SshAuthenticationChallenge challenge) {
    events.add(
      TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.authInteractiveRequest,
        message: 'SSH authentication requires a response.',
        method: 'keyboard-interactive',
        challenge: challenge,
      ),
    );
  }

  @override
  List<TerminalConnectionEvent> drainConnectionEvents() {
    final result = List<TerminalConnectionEvent>.of(events);
    events.clear();
    return result;
  }

  @override
  bool respondToAuthenticationCredentials(
    int id,
    SshAuthenticationCredentials credentials,
  ) => false;
  @override
  bool respondToAuthentication(int id, List<String>? response) {
    responses.add(response);
    return true;
  }
}
