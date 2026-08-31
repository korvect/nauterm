import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_shell_integration.dart';

void main() {
  group('terminal shell integration', () {
    test('recognizes supported shell executable paths', () {
      expect(
        terminalShellKindFromPath('/opt/homebrew/bin/zsh'),
        TerminalShellKind.zsh,
      );
      expect(
        terminalShellKindFromPath(r'C:\Program Files\PowerShell\7\pwsh.exe'),
        TerminalShellKind.powerShell,
      );
      expect(
        terminalShellKindFromPath(r'C:\Program Files\Git\bin\bash.exe'),
        TerminalShellKind.bash,
      );
      expect(terminalShellKindFromPath('/bin/unknown-shell'), isNull);
    });

    test('Git Bash enables startup shell integration', () {
      expect(
        terminalShellPathSupportsStructuredIntegration(
          r'C:\Program Files\Git\bin\bash.exe',
        ),
        isTrue,
      );
      expect(
        terminalShellPathSupportsStructuredIntegration(
          '/bin/bash',
          isMacOS: true,
        ),
        isFalse,
      );
    });

    test('Unix setup commands are hidden from compatible shell history', () {
      for (final kind in [
        TerminalShellKind.zsh,
        TerminalShellKind.bash,
        TerminalShellKind.fish,
      ]) {
        expect(
          terminalShellSetupCommand(kind, 'token'),
          startsWith(' '),
          reason: '$kind setup must remain a leading-space command',
        );
      }
    });

    test('rich shell hooks emit the shared command block protocol', () {
      for (final kind in [
        TerminalShellKind.zsh,
        TerminalShellKind.bash,
        TerminalShellKind.fish,
      ]) {
        final command = terminalShellSetupCommand(kind, 'token');
        expect(command, contains('4545;CommandStarted;'));
        expect(command, contains('4545;CommandExited;'));
        expect(command, contains(r'\033]133;A;cl=line'));
        expect(command, contains('133;B'));
        expect(command, contains(r'\033]133;C'));
        expect(command, contains(r'\033]133;D;'));
        expect(command, contains('aid='));
      }
    });

    test('bash command completion markers avoid BEL terminators', () {
      final command = terminalShellSetupCommand(
        TerminalShellKind.bash,
        'token',
      );
      expect(command, contains(r"nauterm-command-end=%s;%s\033\\"));
      expect(command, isNot(contains(r"nauterm-command-end=%s;%s\007")));
    });

    test('bash setup avoids path redirects rejected by rbash', () {
      final command = terminalShellSetupCommand(
        TerminalShellKind.bash,
        'token',
      );
      expect(command, isNot(contains('/dev/null')));
      expect(command, contains(r'2>&-'));
    });

    test(
      'only history-safe bootstrap shells enable structured integration',
      () {
        expect(
          terminalShellSupportsStructuredIntegration(TerminalShellKind.zsh),
          isTrue,
        );
        expect(
          terminalShellSupportsStructuredIntegration(TerminalShellKind.bash),
          isTrue,
        );
        expect(
          terminalShellSupportsStructuredIntegration(TerminalShellKind.fish),
          isTrue,
        );
        expect(
          terminalShellSupportsStructuredIntegration(TerminalShellKind.ksh),
          isFalse,
        );
        expect(
          terminalShellSupportsStructuredIntegration(
            TerminalShellKind.powerShell,
          ),
          isFalse,
        );
      },
    );

    test('shell announcements can span output chunks', () {
      final parser = TerminalShellAnnouncementParser();

      expect(parser.add('\x1b]4545;Sh'.codeUnits), isNull);
      expect(parser.add('ell;/usr/bin/zsh\x07'.codeUnits), '/usr/bin/zsh');
    });

    test('terminal controller retains the announced remote shell', () {
      final controller = TerminalController(
        driver: MemoryTerminalDriver(columns: 80, rows: 24),
      );
      addTearDown(controller.dispose);

      controller.write('\x1b]4545;Shell;/usr/bin/bash\x07');

      expect(controller.reportedShellPath, '/usr/bin/bash');
    });
  });
}
