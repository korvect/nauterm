import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

const _keys = [
  TerminalConnectionKeyOption(
    id: 7,
    name: 'Test key',
    privateKey: 'test-private-key',
    subtitle: 'ED25519',
  ),
];
void main() {
  setUp(() async {
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
  });
  for (final methods in [
    ['password'],
    ['publickey'],
    ['password', 'publickey'],
  ]) {
    for (final inDrawer in [false, true]) {
      testWidgets(
        'only offered authentication methods appear: $methods, drawer=$inDrawer',
        (tester) async {
          final driver = _MethodsDriver()..request(71, methods);
          final controller = _controller(driver);
          addTearDown(controller.dispose);
          await _pump(tester, controller, inDrawer: inDrawer);
          if (methods.length == 2) {
            expect(find.text('Password'), findsNWidgets(2));
            expect(find.text('Public Key'), findsOneWidget);
            await tester.tap(find.text('Public Key'));
            await tester.pump();
            expect(find.text('Test key'), findsOneWidget);
          } else if (methods.single == 'password') {
            expect(find.text('Password'), findsOneWidget);
            expect(find.text('Public Key'), findsNothing);
            expect(find.text('Test key'), findsNothing);
          } else {
            expect(find.text('Password'), findsNothing);
            expect(find.text('Public Key'), findsOneWidget);
            expect(find.text('Test key'), findsOneWidget);
          }
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox.shrink());
        },
      );
    }
  }

  testWidgets(
    'public key, unlock, password, and code advance on the same connection',
    (tester) async {
      final driver = _MethodsDriver()..request(71, ['publickey']);
      final controller = _controller(driver);
      addTearDown(controller.dispose);
      await _pump(tester, controller);
      await tester.tap(find.text('Test key'));
      await tester.pump();
      await tester.tap(find.text('Continue'));
      await tester.pump();
      expect(driver.credentials.single.method, 'publickey');
      expect(controller.sshProfile!.privateKey, 'test-private-key');
      expect(controller.authenticationRequestId, isNull);

      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.authPassphraseRequired,
          message: 'Configured key requires a passphrase.',
          method: 'public_key',
          authenticationRequestId: 72,
          authenticationMethods: ['publickey'],
        ),
      );
      controller.poll();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.text('Please Enter passphrase for key'), findsOneWidget);
      await tester.enterText(find.byType(TextField), 'unlock-secret');
      await tester.pump();
      await tester.tap(find.text('Continue'));
      await tester.pump();
      expect(driver.credentials.last.passphrase, 'unlock-secret');

      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.authPartialSuccess,
          message: 'Key accepted; additional authentication required.',
          authenticationMethods: ['password'],
        ),
      );
      driver.request(73, ['password']);
      controller.poll();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.text('Please Enter passphrase for key'), findsNothing);
      expect(find.text('Password'), findsOneWidget);
      expect(find.text('Public Key'), findsNothing);
      await tester.enterText(find.byType(TextField), 'password-secret');
      await tester.pump();
      await tester.tap(find.text('Continue'));
      await tester.pump();
      expect(driver.credentials.last.method, 'password');
      expect(driver.credentials.last.password, 'password-secret');
      expect(controller.sshProfile!.privateKey, 'test-private-key');

      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.authPartialSuccess,
          message: 'Password accepted; additional authentication required.',
          method: 'password',
          authenticationMethods: ['keyboard-interactive'],
        ),
      );
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.authInteractiveRequest,
          message: 'Code required.',
          method: 'keyboard-interactive',
          challenge: SshAuthenticationChallenge(
            id: 74,
            name: '',
            instructions: '',
            prompts: [
              SshAuthenticationPrompt(
                prompt: 'Verification code:',
                echo: false,
              ),
            ],
          ),
        ),
      );
      controller.poll();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.text('Password'), findsNothing);
      expect(find.text('Public Key'), findsNothing);
      expect(find.text('Verification code:'), findsOneWidget);
      expect(
        _progressLabel(
          'Public Key → Passphrase → Password → Verification code',
        ),
        findsOneWidget,
      );
      await tester.enterText(find.byType(TextField), '123456');
      await tester.tap(find.text('Continue'));
      await tester.pump();
      expect(driver.answers.single, ['123456']);
      expect(controller.nativeSessionId, isNull);
      final logs = controller.connectionEvents
          .map((event) => event.logLine)
          .join();
      for (final secret in ['unlock-secret', 'password-secret', '123456']) {
        expect(logs, isNot(contains(secret)));
      }
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  for (final order in [
    ['password', 'publickey'],
    ['publickey', 'password'],
  ]) {
    testWidgets('progress appends each completed factor: $order', (
      tester,
    ) async {
      final driver = _MethodsDriver()..request(81, [order[0]]);
      final controller = _controller(driver);
      addTearDown(controller.dispose);
      await _pump(tester, controller);
      final labels = <String>[];
      for (var index = 0; index < order.length; index++) {
        final method = order[index];
        labels.add(method == 'password' ? 'Password' : 'Public Key');
        expect(_progressLabel(labels.join(' → ')), findsOneWidget);
        if (method == 'password') {
          await tester.enterText(find.byType(TextField), 'phase-secret');
        } else {
          await tester.tap(find.text('Test key'));
        }
        await tester.pump();
        await tester.tap(find.text('Continue'));
        await tester.pump();
        driver.events.add(
          TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authPartialSuccess,
            message: 'Factor accepted.',
            method: method,
            authenticationMethods: [
              index == 0 ? order[1] : 'keyboard-interactive',
            ],
          ),
        );
        if (index == 0) {
          driver.request(82, [order[1]]);
        } else {
          driver.events.add(
            const TerminalConnectionEvent(
              kind: TerminalConnectionEventKind.authInteractiveRequest,
              message: 'Code required.',
              challenge: SshAuthenticationChallenge(
                id: 83,
                name: '',
                instructions: '',
                prompts: [
                  SshAuthenticationPrompt(
                    prompt: 'Verification code:',
                    echo: false,
                  ),
                ],
              ),
            ),
          );
        }
        controller.poll();
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 400));
      }
      labels.add('Verification code');
      expect(_progressLabel(labels.join(' → ')), findsOneWidget);
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.authInteractiveRequest,
          message: 'Next code required.',
          challenge: SshAuthenticationChallenge(
            id: 84,
            name: '',
            instructions: '',
            prompts: [
              SshAuthenticationPrompt(prompt: 'Next code:', echo: false),
            ],
          ),
        ),
      );
      controller.poll();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      labels.add('Verification code');
      expect(_progressLabel(labels.join(' → ')), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }

  for (final inDrawer in [false, true]) {
    testWidgets(
      'a new keyboard-interactive round appends after its password round, drawer=$inDrawer',
      (tester) async {
        final driver = _MethodsDriver()..request(100, ['publickey']);
        final controller = _controller(driver);
        addTearDown(controller.dispose);
        await _pump(tester, controller, inDrawer: inDrawer);
        await tester.tap(find.text('Test key'));
        await tester.pump();
        await tester.tap(find.text('Continue'));
        await tester.pump();
        driver.events.addAll([
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authKeyStart,
            message: 'Trying key.',
          ),
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authPartialSuccess,
            message: 'Key accepted.',
            method: 'public_key',
            authenticationMethods: ['keyboard-interactive'],
          ),
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authInteractiveStart,
            message: 'Trying challenge.',
            method: 'keyboard-interactive',
          ),
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authInteractiveRequest,
            message: 'Password required.',
            challenge: SshAuthenticationChallenge(
              id: 101,
              name: '',
              instructions: '',
              prompts: [
                SshAuthenticationPrompt(prompt: 'Password:', echo: false),
              ],
            ),
          ),
        ]);
        controller.poll();
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 400));
        await tester.pump(const Duration(milliseconds: 200));
        expect(_progressLabel('Public Key → Password'), findsOneWidget);
        expect(find.text('Password:'), findsOneWidget);
        await tester.enterText(find.byType(TextField), 'round-password');
        await tester.tap(find.text('Continue'));
        await tester.pump();
        expect(driver.answers.single, ['round-password']);
        driver.events.addAll([
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authInteractiveSubmitted,
            message: 'Response submitted.',
            method: 'keyboard-interactive',
          ),
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authInteractiveRequest,
            message: 'Code required.',
            challenge: SshAuthenticationChallenge(
              id: 102,
              name: '',
              instructions: '',
              prompts: [
                SshAuthenticationPrompt(
                  prompt: 'Verification code:',
                  echo: false,
                ),
              ],
            ),
          ),
        ]);
        controller.poll();
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 400));
        await tester.pump(const Duration(milliseconds: 200));
        expect(
          _progressLabel('Public Key → Password → Verification code'),
          findsOneWidget,
        );
        expect(find.text('Verification code:'), findsOneWidget);
        expect(find.text('Password:'), findsNothing);
        // Rebuilding or receiving the same request again does not add a node.
        driver.events.add(
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authInteractiveRequest,
            message: 'Code required.',
            challenge: SshAuthenticationChallenge(
              id: 102,
              name: '',
              instructions: '',
              prompts: [
                SshAuthenticationPrompt(
                  prompt: 'Verification code:',
                  echo: false,
                ),
              ],
            ),
          ),
        );
        controller.poll();
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 400));
        expect(
          _progressLabel('Public Key → Password → Verification code'),
          findsOneWidget,
        );
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  for (final afterPublicKey in [false, true]) {
    testWidgets(
      'password fallback adds a code node without partial success, key=$afterPublicKey',
      (tester) async {
        final driver = _MethodsDriver()
          ..request(91, [afterPublicKey ? 'publickey' : 'password']);
        final controller = _controller(driver);
        addTearDown(controller.dispose);
        await _pump(tester, controller);
        if (afterPublicKey) {
          driver.events.addAll([
            const TerminalConnectionEvent(
              kind: TerminalConnectionEventKind.authKeyStart,
              message: 'Trying key.',
            ),
            const TerminalConnectionEvent(
              kind: TerminalConnectionEventKind.authPartialSuccess,
              message: 'Key accepted.',
              method: 'public_key',
              authenticationMethods: ['password'],
            ),
          ]);
          driver.request(92, ['password']);
        }
        driver.events.add(
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authPasswordStart,
            message: 'Trying password.',
            method: 'password',
          ),
        );
        controller.poll();
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 400));
        final prefix = afterPublicKey ? 'Public Key → Password' : 'Password';
        expect(_progressLabel(prefix), findsOneWidget);
        // Retrying this password still uses the current node.
        driver.events.add(
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authPasswordStart,
            message: 'Retrying password.',
            method: 'password',
          ),
        );
        controller.poll();
        await tester.pump();
        expect(_progressLabel(prefix), findsOneWidget);
        driver.events.addAll([
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authPasswordRejected,
            message: 'Password rejected.',
            method: 'password',
            authenticationMethods: ['keyboard-interactive'],
          ),
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authInteractiveStart,
            message: 'Trying challenge.',
            method: 'keyboard-interactive',
          ),
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authInteractiveRequest,
            message: 'Code required.',
            challenge: SshAuthenticationChallenge(
              id: 93,
              name: '',
              instructions: '',
              prompts: [
                SshAuthenticationPrompt(
                  prompt: 'Verification code:',
                  echo: false,
                ),
              ],
            ),
          ),
        ]);
        controller.poll();
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 400));
        await tester.pump(const Duration(milliseconds: 200));
        expect(_progressLabel('$prefix → Verification code'), findsOneWidget);
        expect(find.text('Verification code:'), findsOneWidget);
        expect(find.text('Password'), findsNothing);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  testWidgets(
    'background agent attempts do not add steps or move a waiting key page backward',
    (tester) async {
      final driver = _MethodsDriver()
        ..events.add(
          const TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.connectStart,
            message: 'Connecting.',
          ),
        );
      final controller = _controller(driver);
      addTearDown(controller.dispose);
      await _pump(tester, controller);
      driver.events.addAll([
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.authAgentStart,
          message: 'Trying agent.',
        ),
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.authAgentIdentities,
          message: 'One identity.',
        ),
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.authAgentIdentityStart,
          message: 'Trying agent key.',
        ),
      ]);
      controller.poll();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      expect(_progressLabel('Public Key'), findsNothing);
      expect(_progressFraction(tester), 0);
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.authAgentIdentityRejected,
          message: 'Agent key rejected.',
        ),
      );
      driver.request(95, ['publickey']);
      controller.poll();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      await tester.pump(const Duration(milliseconds: 200));
      // The controller batches notifications; let the new page's layout
      // animation finish after the notification has reached the widget.
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.text('Test key'), findsOneWidget);
      expect(_progressLabel('Public Key'), findsOneWidget);
      _expectWaitingAtMiddleStep(tester, Icons.key_rounded);

      expect(
        controller.respondToAuthenticationCredentials(
          95,
          const SshAuthenticationCredentials.publicKey(
            privateKey: 'selected-key',
          ),
        ),
        isTrue,
      );
      await tester.pump();
      _expectWaitingAtMiddleStep(tester, Icons.key_rounded);
      driver.events.add(
        const TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.authKeyRejected,
          message: 'Configured key rejected.',
          authenticationMethods: ['publickey'],
        ),
      );
      driver.request(96, ['publickey']);
      controller.poll();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 16));
      _expectWaitingAtMiddleStep(tester, Icons.key_rounded);
      await tester.pump(const Duration(milliseconds: 400));
      expect(_progressLabel('Public Key'), findsOneWidget);
      _expectWaitingAtMiddleStep(tester, Icons.key_rounded);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  for (final method in ['public_key', 'fido2']) {
    testWidgets(
      'a configured key unlock only creates the visible page step: $method',
      (tester) async {
        final driver = _MethodsDriver()
          ..events.addAll([
            const TerminalConnectionEvent(
              kind: TerminalConnectionEventKind.authKeyStart,
              message: 'Checking key.',
            ),
            TerminalConnectionEvent(
              kind: TerminalConnectionEventKind.authPassphraseRequired,
              message: 'Key unlock required.',
              method: method,
            ),
            TerminalConnectionEvent(
              kind: TerminalConnectionEventKind.authPassphraseRequired,
              message: 'Key unlock required.',
              method: method,
              authenticationRequestId: 97,
              authenticationMethods: ['publickey'],
            ),
          ]);
        final controller = _controller(driver, privateKey: 'configured-key');
        addTearDown(controller.dispose);
        await _pump(tester, controller);
        final label = method == 'fido2' ? 'PIN' : 'Passphrase';
        expect(_progressLabel(label), findsOneWidget);
        expect(_progressLabel('Public Key'), findsNothing);
        expect(_progressFraction(tester), closeTo(0.5, 0.0001));
        driver.events.add(
          TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.authPassphraseRequired,
            message: 'Key unlock required.',
            method: method,
            authenticationRequestId: 98,
            authenticationMethods: ['publickey'],
          ),
        );
        controller.poll();
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 400));
        expect(_progressLabel(label), findsOneWidget);
        expect(_progressFraction(tester), closeTo(0.5, 0.0001));
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  test('server method metadata survives stamping and resets on reconnect', () {
    final driver = _MethodsDriver()
      ..events.add(
        TerminalConnectionEvent.fromJson({
          'kind': 'auth_credentials_required',
          'message': 'Credentials required.',
          'authentication_methods': ['publickey'],
          'authentication_request_id': 10,
        }),
      );
    final controller = _controller(driver);
    addTearDown(controller.dispose);
    expect(controller.authenticationMethods, ['publickey']);
    expect(controller.authenticationRequestId, 10);
    expect(
      controller.respondToAuthenticationCredentials(
        10,
        const SshAuthenticationCredentials.password('wrong-method'),
      ),
      isFalse,
    );
    driver.events.add(
      const TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.connectStart,
        message: 'Reconnecting',
      ),
    );
    controller.poll();
    expect(controller.authenticationMethods, isNull);
    expect(controller.authenticationRequestId, isNull);
  });
}

TerminalController _controller(_MethodsDriver driver, {String? privateKey}) =>
    TerminalController.ssh(
      host: 'example.test',
      port: 22,
      username: 'tester',
      knownHostsPath: '/tmp/known-hosts',
      privateKey: privateKey,
      driver: driver,
    );
Future<void> _pump(
  WidgetTester tester,
  TerminalController controller, {
  bool inDrawer = true,
}) async {
  controller.poll();
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Center(
          child: SizedBox(
            width: 360,
            height: 320,
            child: buildTerminalConnectionPageForTesting(
              controller,
              inDrawer: inDrawer,
              keys: _keys,
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

class _MethodsDriver extends MemoryTerminalDriver
    implements TerminalAuthenticationResponder {
  _MethodsDriver() : super(columns: 80, rows: 24);
  final events = <TerminalConnectionEvent>[];
  final credentials = <SshAuthenticationCredentials>[];
  final answers = <List<String>?>[];
  void request(int id, List<String> methods) => events.add(
    TerminalConnectionEvent(
      kind: TerminalConnectionEventKind.authCredentialsRequired,
      message: 'Credentials required.',
      authenticationMethods: methods,
      authenticationRequestId: id,
    ),
  );
  @override
  List<TerminalConnectionEvent> drainConnectionEvents() {
    final result = List<TerminalConnectionEvent>.of(events);
    events.clear();
    return result;
  }

  @override
  bool respondToAuthentication(int id, List<String>? responses) {
    answers.add(responses);
    return true;
  }

  @override
  bool respondToAuthenticationCredentials(
    int id,
    SshAuthenticationCredentials value,
  ) {
    credentials.add(value);
    return true;
  }
}

Finder _progressLabel(String label) => find.descendant(
  of: find.byKey(const ValueKey('connection-page-progress')),
  matching: find.bySemanticsLabel(label),
);

Finder _progressDescendant(Finder matching) => find.descendant(
  of: find.byKey(const ValueKey('connection-page-progress')),
  matching: matching,
);

double _progressFraction(WidgetTester tester) => tester
    .widget<FractionallySizedBox>(
      _progressDescendant(find.byType(FractionallySizedBox)),
    )
    .widthFactor!;

void _expectWaitingAtMiddleStep(WidgetTester tester, IconData icon) {
  expect(_progressFraction(tester), closeTo(0.5, 0.0001));
  final spinner = _progressDescendant(find.byType(CircularProgressIndicator));
  expect(spinner, findsOneWidget);
  expect(
    tester.getCenter(spinner).dx,
    closeTo(tester.getCenter(_progressDescendant(find.byIcon(icon))).dx, 0.1),
  );
}
