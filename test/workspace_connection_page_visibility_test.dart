import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  test('an already-connected rebuilt terminal skips the connection page', () {
    expect(
      shouldBeginConnectionCompletionHold(
        phase: TerminalConnectionPhase.connected,
        previousPhase: null,
        connectionPageWasShown: false,
      ),
      isFalse,
    );
  });

  test('a visible connection page is held while connection completes', () {
    expect(
      shouldBeginConnectionCompletionHold(
        phase: TerminalConnectionPhase.connected,
        previousPhase: TerminalConnectionPhase.connecting,
        connectionPageWasShown: true,
      ),
      isTrue,
    );
  });

  test('terminal chrome remains active while reconnecting', () {
    expect(
      shouldUseTerminalChrome(
        terminalPageVisible: true,
        phase: TerminalConnectionPhase.connecting,
        hasConnectedOnce: true,
      ),
      isTrue,
    );
  });

  test('terminal chrome waits for the first successful connection', () {
    expect(
      shouldUseTerminalChrome(
        terminalPageVisible: true,
        phase: TerminalConnectionPhase.connecting,
        hasConnectedOnce: false,
      ),
      isFalse,
    );
  });

  test('SFTP pages do not use terminal chrome', () {
    expect(
      shouldUseTerminalChrome(
        terminalPageVisible: false,
        phase: TerminalConnectionPhase.connected,
        hasConnectedOnce: true,
      ),
      isFalse,
    );
  });
}
