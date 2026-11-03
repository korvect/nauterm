import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_controller.dart';

void main() {
  for (final mosh in [false, true]) {
    for (final username in ['', '   ']) {
      test(
        'missing username does not create a native connection, mosh=$mosh, username="$username"',
        () {
          final controller = mosh
              ? TerminalController.mosh(
                  host: '127.0.0.1',
                  port: 0,
                  username: username,
                  knownHostsPath: '/tmp/unused-username-test-known-hosts',
                )
              : TerminalController.ssh(
                  host: '127.0.0.1',
                  port: 0,
                  username: username,
                  knownHostsPath: '/tmp/unused-username-test-known-hosts',
                );
          addTearDown(controller.dispose);
          expect(controller.nativeSessionId, isNull);
          expect(controller.connectionEvents, isEmpty);
          expect(
            controller.snapshot.cells.map((cell) => cell.text).join().trim(),
            isEmpty,
          );
          expect(
            controller.connectionStatus.phase,
            TerminalConnectionPhase.connecting,
          );
          expect(controller.reconnectSsh(username: username), isTrue);
          expect(controller.nativeSessionId, isNull);
          expect(controller.connectionEvents.map((event) => event.kind.name), [
            'retry',
          ]);
          expect(
            controller.snapshot.cells.map((cell) => cell.text).join().trim(),
            isEmpty,
          );
        },
      );
    }
  }
}
