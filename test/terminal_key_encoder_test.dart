import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/terminal/terminal_key_encoder.dart';
import 'package:nauterm/terminal/terminal_models.dart';

void main() {
  const encoder = TerminalKeyEncoder(platform: TargetPlatform.macOS);

  test(
    'bracketed paste preserves multiline shell input as one transaction',
    () {
      const command = 'python3 -c "\nprint(r"""\\r\\n""")\n" &\nsleep 1';
      expect(
        terminalPasteSequence(
          command,
          const TerminalKeyboardMode(bracketedPaste: true),
        ),
        '\x1b[200~python3 -c "\rprint(r"""\\r\\n""")\r" &\rsleep 1'
        '\x1b[201~',
      );
    },
  );

  test('encodes printable characters', () {
    expect(_sequence(encoder, LogicalKeyboardKey.keyA, character: 'a'), 'a');
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.keyA,
        character: 'A',
        modifiers: const TerminalKeyboardModifiers(shift: true),
      ),
      'A',
    );
  });

  test(
    'prompt insertion does not submit multiline text without bracketed paste',
    () {
      expect(
        terminalPromptInsertionSequence(
          'printf one\nprintf two',
          const TerminalKeyboardMode(),
        ),
        'printf one printf two',
      );
    },
  );

  test('prompt insertion uses bracketed paste when supported', () {
    expect(
      terminalPromptInsertionSequence(
        'printf one\nprintf two',
        const TerminalKeyboardMode(bracketedPaste: true),
      ),
      '\x1b[200~printf one\rprintf two\x1b[201~',
    );
  });

  test('prompt insertion keeps a single command free of paste markers', () {
    expect(
      terminalPromptInsertionSequence(
        'ls -al',
        const TerminalKeyboardMode(bracketedPaste: true),
      ),
      'ls -al',
    );
  });

  test('encodes control letters without relying on character text', () {
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.keyC,
        modifiers: const TerminalKeyboardModifiers(control: true),
      ),
      '\x03',
    );
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.keyD,
        modifiers: const TerminalKeyboardModifiers(control: true),
      ),
      '\x04',
    );
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.keyZ,
        modifiers: const TerminalKeyboardModifiers(control: true),
      ),
      '\x1a',
    );
  });

  test('encodes non-letter control shortcuts', () {
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.space,
        modifiers: const TerminalKeyboardModifiers(control: true),
      ),
      '\x00',
    );
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.bracketLeft,
        modifiers: const TerminalKeyboardModifiers(control: true),
      ),
      '\x1b',
    );
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.digit8,
        modifiers: const TerminalKeyboardModifiers(control: true),
      ),
      '\x7f',
    );
  });

  test('encodes alt as escape prefix for printable input', () {
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.keyF,
        character: 'f',
        modifiers: const TerminalKeyboardModifiers(alt: true),
      ),
      '\x1bf',
    );
  });

  test('can treat option text as regular input', () {
    const inputOptionEncoder = TerminalKeyEncoder(
      config: TerminalKeyboardConfig(useOptionAsMetaKey: false),
    );

    expect(
      _sequence(
        inputOptionEncoder,
        LogicalKeyboardKey.keyF,
        character: 'ƒ',
        modifiers: const TerminalKeyboardModifiers(alt: true),
      ),
      'ƒ',
    );
  });

  test('encodes navigation keys with xterm modifiers', () {
    expect(_sequence(encoder, LogicalKeyboardKey.arrowUp), '\x1b[A');
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.arrowLeft,
        modifiers: const TerminalKeyboardModifiers(control: true),
      ),
      '\x1b[1;5D',
    );
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.arrowDown,
        modifiers: const TerminalKeyboardModifiers(shift: true, alt: true),
      ),
      '\x1b[1;4B',
    );
  });

  test('encodes macOS option arrows as shell word movement', () {
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.arrowLeft,
        modifiers: const TerminalKeyboardModifiers(alt: true),
      ),
      '\x1bb',
    );
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.arrowRight,
        modifiers: const TerminalKeyboardModifiers(alt: true),
      ),
      '\x1bf',
    );
  });

  test('macOS option arrow word movement is independent of option text', () {
    const inputOptionEncoder = TerminalKeyEncoder(
      platform: TargetPlatform.macOS,
      config: TerminalKeyboardConfig(useOptionAsMetaKey: false),
    );

    expect(
      _sequence(
        inputOptionEncoder,
        LogicalKeyboardKey.arrowLeft,
        modifiers: const TerminalKeyboardModifiers(alt: true),
      ),
      '\x1bb',
    );
  });

  test('keeps xterm option arrow sequences outside macOS', () {
    const linuxEncoder = TerminalKeyEncoder(platform: TargetPlatform.linux);

    expect(
      _sequence(
        linuxEncoder,
        LogicalKeyboardKey.arrowLeft,
        modifiers: const TerminalKeyboardModifiers(alt: true),
      ),
      '\x1b[1;3D',
    );
    expect(
      _sequence(
        linuxEncoder,
        LogicalKeyboardKey.arrowRight,
        modifiers: const TerminalKeyboardModifiers(alt: true),
      ),
      '\x1b[1;3C',
    );
  });

  test('encodes cursor keys in application cursor mode', () {
    const applicationCursorEncoder = TerminalKeyEncoder(
      mode: TerminalKeyboardMode(applicationCursor: true),
    );

    expect(
      _sequence(applicationCursorEncoder, LogicalKeyboardKey.arrowUp),
      '\x1bOA',
    );
    expect(
      _sequence(
        applicationCursorEncoder,
        LogicalKeyboardKey.arrowUp,
        modifiers: const TerminalKeyboardModifiers(control: true),
      ),
      '\x1b[1;5A',
    );
  });

  test('encodes numpad keys in application keypad mode', () {
    const applicationKeypadEncoder = TerminalKeyEncoder(
      mode: TerminalKeyboardMode(applicationKeypad: true),
    );

    expect(
      _sequence(applicationKeypadEncoder, LogicalKeyboardKey.numpad1),
      '\x1bOq',
    );
    expect(
      _sequence(applicationKeypadEncoder, LogicalKeyboardKey.numpadDecimal),
      '\x1bOn',
    );
    expect(
      _sequence(applicationKeypadEncoder, LogicalKeyboardKey.numpadEnter),
      '\x1bOM',
    );
  });

  test('encodes editing and paging keys', () {
    expect(_sequence(encoder, LogicalKeyboardKey.home), '\x1b[H');
    expect(_sequence(encoder, LogicalKeyboardKey.end), '\x1b[F');
    expect(_sequence(encoder, LogicalKeyboardKey.insert), '\x1b[2~');
    expect(_sequence(encoder, LogicalKeyboardKey.delete), '\x1b[3~');
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.pageUp,
        modifiers: const TerminalKeyboardModifiers(control: true),
      ),
      '\x1b[5;5~',
    );
  });

  test('encodes tab variants', () {
    expect(_sequence(encoder, LogicalKeyboardKey.tab), '\t');
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.tab,
        modifiers: const TerminalKeyboardModifiers(shift: true),
      ),
      '\x1b[Z',
    );
  });

  test('encodes function keys', () {
    expect(_sequence(encoder, LogicalKeyboardKey.f1), '\x1bOP');
    expect(_sequence(encoder, LogicalKeyboardKey.f5), '\x1b[15~');
    expect(
      _sequence(
        encoder,
        LogicalKeyboardKey.f12,
        modifiers: const TerminalKeyboardModifiers(shift: true),
      ),
      '\x1b[24;2~',
    );
  });

  test('returns app actions for terminal shortcuts', () {
    expect(
      _action(
        encoder,
        LogicalKeyboardKey.keyC,
        modifiers: const TerminalKeyboardModifiers(meta: true),
      ),
      TerminalKeyboardAction.copy,
    );
    expect(
      _action(
        encoder,
        LogicalKeyboardKey.keyV,
        modifiers: const TerminalKeyboardModifiers(control: true, shift: true),
      ),
      isNull,
    );
    expect(
      _action(
        encoder,
        LogicalKeyboardKey.keyA,
        modifiers: const TerminalKeyboardModifiers(meta: true),
      ),
      TerminalKeyboardAction.selectAll,
    );
  });

  test('reserves Control and uses Control Shift outside macOS', () {
    const windowsEncoder = TerminalKeyEncoder(platform: TargetPlatform.windows);
    expect(
      _action(
        windowsEncoder,
        LogicalKeyboardKey.keyC,
        modifiers: const TerminalKeyboardModifiers(control: true),
      ),
      isNull,
    );
    expect(
      _sequence(
        windowsEncoder,
        LogicalKeyboardKey.keyC,
        modifiers: const TerminalKeyboardModifiers(control: true),
      ),
      '\x03',
    );
    expect(
      _action(
        windowsEncoder,
        LogicalKeyboardKey.keyC,
        modifiers: const TerminalKeyboardModifiers(control: true, shift: true),
      ),
      TerminalKeyboardAction.copy,
    );
    expect(
      _action(
        windowsEncoder,
        LogicalKeyboardKey.keyC,
        modifiers: const TerminalKeyboardModifiers(meta: true),
      ),
      isNull,
    );
  });
}

String? _sequence(
  TerminalKeyEncoder encoder,
  LogicalKeyboardKey logicalKey, {
  String? character,
  TerminalKeyboardModifiers modifiers = const TerminalKeyboardModifiers(),
}) {
  return encoder
      .encode(
        logicalKey: logicalKey,
        character: character,
        modifiers: modifiers,
      )
      .sequence;
}

TerminalKeyboardAction? _action(
  TerminalKeyEncoder encoder,
  LogicalKeyboardKey logicalKey, {
  TerminalKeyboardModifiers modifiers = const TerminalKeyboardModifiers(),
}) {
  return encoder
      .encode(logicalKey: logicalKey, character: null, modifiers: modifiers)
      .action;
}
