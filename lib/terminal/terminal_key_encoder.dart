import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'terminal_config.dart';
import 'terminal_models.dart';

enum TerminalKeyboardAction { copy, paste, selectAll }

String terminalPasteSequence(String text, TerminalKeyboardMode mode) {
  final normalized = text
      .replaceAll('\r\n', '\n')
      .replaceAll('\r', '\n')
      .replaceAll('\n', '\r');
  if (!mode.bracketedPaste) {
    return normalized;
  }
  return '\x1b[200~$normalized\x1b[201~';
}

String terminalPromptInsertionSequence(String text, TerminalKeyboardMode mode) {
  final normalized = text.replaceAll('\r\n', '\n').replaceAll('\r', '\n');
  if (!normalized.contains('\n')) {
    return normalized;
  }
  if (mode.bracketedPaste) {
    return terminalPasteSequence(normalized, mode);
  }
  return normalized.replaceAll('\n', ' ');
}

@immutable
class TerminalKeyboardInput {
  const TerminalKeyboardInput._({this.sequence, this.action});

  const TerminalKeyboardInput.sequence(String sequence)
    : this._(sequence: sequence);

  const TerminalKeyboardInput.action(TerminalKeyboardAction action)
    : this._(action: action);

  const TerminalKeyboardInput.ignored() : this._();

  final String? sequence;
  final TerminalKeyboardAction? action;

  bool get isIgnored => sequence == null && action == null;
}

@immutable
class TerminalKeyboardModifiers {
  const TerminalKeyboardModifiers({
    this.shift = false,
    this.alt = false,
    this.control = false,
    this.meta = false,
  });

  factory TerminalKeyboardModifiers.fromHardwareKeyboard(
    HardwareKeyboard keyboard,
  ) {
    return TerminalKeyboardModifiers(
      shift: keyboard.isShiftPressed,
      alt: keyboard.isAltPressed,
      control: keyboard.isControlPressed,
      meta: keyboard.isMetaPressed,
    );
  }

  final bool shift;
  final bool alt;
  final bool control;
  final bool meta;
}

class TerminalKeyEncoder {
  const TerminalKeyEncoder({
    this.config = const TerminalKeyboardConfig(),
    this.mode = const TerminalKeyboardMode(),
    this.platform,
  });

  final TerminalKeyboardConfig config;
  final TerminalKeyboardMode mode;
  final TargetPlatform? platform;

  TerminalKeyboardInput encodeEvent(
    KeyEvent event,
    TerminalKeyboardModifiers modifiers,
  ) {
    return encode(
      logicalKey: event.logicalKey,
      character: event.character,
      modifiers: modifiers,
    );
  }

  TerminalKeyboardInput encode({
    required LogicalKeyboardKey logicalKey,
    required String? character,
    required TerminalKeyboardModifiers modifiers,
  }) {
    final shortcut = _shortcutAction(logicalKey, modifiers);
    if (shortcut != null) {
      return TerminalKeyboardInput.action(shortcut);
    }

    if (modifiers.meta) {
      return const TerminalKeyboardInput.ignored();
    }

    final keypad = _applicationKeypadSequence(logicalKey);
    if (mode.applicationKeypad && keypad != null) {
      return TerminalKeyboardInput.sequence(keypad);
    }

    final special = _specialKeySequence(logicalKey, modifiers);
    if (special != null) {
      return TerminalKeyboardInput.sequence(special);
    }

    final control = _controlSequence(logicalKey, modifiers);
    if (control != null) {
      return TerminalKeyboardInput.sequence(control);
    }

    if (character == null || character.isEmpty || modifiers.control) {
      return const TerminalKeyboardInput.ignored();
    }

    return TerminalKeyboardInput.sequence(_withAltPrefix(character, modifiers));
  }

  TerminalKeyboardAction? _shortcutAction(
    LogicalKeyboardKey logicalKey,
    TerminalKeyboardModifiers modifiers,
  ) {
    final shortcut = terminalShortcutConfig;
    final isMacOS = (platform ?? defaultTargetPlatform) == TargetPlatform.macOS;
    final platformModifierPressed =
        isMacOS && modifiers.meta && !modifiers.control;
    if (platformModifierPressed) {
      if (shortcut.matchesCopy(
        logicalKey,
        shift: modifiers.shift,
        alt: modifiers.alt,
      )) {
        return TerminalKeyboardAction.copy;
      }
      if (shortcut.matchesPaste(
        logicalKey,
        shift: modifiers.shift,
        alt: modifiers.alt,
      )) {
        return TerminalKeyboardAction.paste;
      }
      if (shortcut.matchesSelectAll(
        logicalKey,
        shift: modifiers.shift,
        alt: modifiers.alt,
      )) {
        return TerminalKeyboardAction.selectAll;
      }
    }

    if (!isMacOS && modifiers.control && modifiers.shift) {
      if (shortcut.matchesCopyKey(logicalKey, alt: modifiers.alt)) {
        return TerminalKeyboardAction.copy;
      }
      if (shortcut.matchesPasteKey(logicalKey, alt: modifiers.alt)) {
        return TerminalKeyboardAction.paste;
      }
      if (shortcut.matchesSelectAllKey(logicalKey, alt: modifiers.alt)) {
        return TerminalKeyboardAction.selectAll;
      }
    }

    return null;
  }

  String? _specialKeySequence(
    LogicalKeyboardKey logicalKey,
    TerminalKeyboardModifiers modifiers,
  ) {
    final macOSOptionWordMovement = _macOSOptionWordMovement(
      logicalKey,
      modifiers,
    );
    if (macOSOptionWordMovement != null) {
      return macOSOptionWordMovement;
    }

    return switch (logicalKey) {
      LogicalKeyboardKey.enter ||
      LogicalKeyboardKey.numpadEnter => _withAltPrefix('\r', modifiers),
      LogicalKeyboardKey.backspace => _withAltPrefix(
        modifiers.control ? '\x08' : '\x7f',
        modifiers,
      ),
      LogicalKeyboardKey.tab => modifiers.shift ? '\x1b[Z' : '\t',
      LogicalKeyboardKey.escape => '\x1b',
      LogicalKeyboardKey.arrowUp => _cursorKey('A', modifiers),
      LogicalKeyboardKey.arrowDown => _cursorKey('B', modifiers),
      LogicalKeyboardKey.arrowRight => _cursorKey('C', modifiers),
      LogicalKeyboardKey.arrowLeft => _cursorKey('D', modifiers),
      LogicalKeyboardKey.home => _csiFinal('H', modifiers),
      LogicalKeyboardKey.end => _csiFinal('F', modifiers),
      LogicalKeyboardKey.insert => _csiTilde(2, modifiers),
      LogicalKeyboardKey.delete => _csiTilde(3, modifiers),
      LogicalKeyboardKey.pageUp => _csiTilde(5, modifiers),
      LogicalKeyboardKey.pageDown => _csiTilde(6, modifiers),
      LogicalKeyboardKey.f1 => _functionKey('P', null, modifiers),
      LogicalKeyboardKey.f2 => _functionKey('Q', null, modifiers),
      LogicalKeyboardKey.f3 => _functionKey('R', null, modifiers),
      LogicalKeyboardKey.f4 => _functionKey('S', null, modifiers),
      LogicalKeyboardKey.f5 => _functionKey(null, 15, modifiers),
      LogicalKeyboardKey.f6 => _functionKey(null, 17, modifiers),
      LogicalKeyboardKey.f7 => _functionKey(null, 18, modifiers),
      LogicalKeyboardKey.f8 => _functionKey(null, 19, modifiers),
      LogicalKeyboardKey.f9 => _functionKey(null, 20, modifiers),
      LogicalKeyboardKey.f10 => _functionKey(null, 21, modifiers),
      LogicalKeyboardKey.f11 => _functionKey(null, 23, modifiers),
      LogicalKeyboardKey.f12 => _functionKey(null, 24, modifiers),
      _ => null,
    };
  }

  String? _macOSOptionWordMovement(
    LogicalKeyboardKey logicalKey,
    TerminalKeyboardModifiers modifiers,
  ) {
    if ((platform ?? defaultTargetPlatform) != TargetPlatform.macOS ||
        !modifiers.alt ||
        modifiers.shift ||
        modifiers.control ||
        modifiers.meta) {
      return null;
    }

    return switch (logicalKey) {
      LogicalKeyboardKey.arrowLeft => '\x1bb',
      LogicalKeyboardKey.arrowRight => '\x1bf',
      _ => null,
    };
  }

  String? _applicationKeypadSequence(LogicalKeyboardKey logicalKey) {
    return switch (logicalKey) {
      LogicalKeyboardKey.numpad0 => '\x1bOp',
      LogicalKeyboardKey.numpad1 => '\x1bOq',
      LogicalKeyboardKey.numpad2 => '\x1bOr',
      LogicalKeyboardKey.numpad3 => '\x1bOs',
      LogicalKeyboardKey.numpad4 => '\x1bOt',
      LogicalKeyboardKey.numpad5 => '\x1bOu',
      LogicalKeyboardKey.numpad6 => '\x1bOv',
      LogicalKeyboardKey.numpad7 => '\x1bOw',
      LogicalKeyboardKey.numpad8 => '\x1bOx',
      LogicalKeyboardKey.numpad9 => '\x1bOy',
      LogicalKeyboardKey.numpadDecimal => '\x1bOn',
      LogicalKeyboardKey.numpadEnter => '\x1bOM',
      LogicalKeyboardKey.numpadAdd => '\x1bOk',
      LogicalKeyboardKey.numpadComma => '\x1bOl',
      LogicalKeyboardKey.numpadSubtract => '\x1bOm',
      LogicalKeyboardKey.numpadMultiply => '\x1bOj',
      LogicalKeyboardKey.numpadDivide => '\x1bOo',
      _ => null,
    };
  }

  String? _controlSequence(
    LogicalKeyboardKey logicalKey,
    TerminalKeyboardModifiers modifiers,
  ) {
    if (!modifiers.control) {
      return null;
    }

    final codeUnit = _controlCodeUnit(logicalKey);
    if (codeUnit == null) {
      return null;
    }

    return _withAltPrefix(String.fromCharCode(codeUnit), modifiers);
  }

  int? _controlCodeUnit(LogicalKeyboardKey logicalKey) {
    final letter = _letterIndex(logicalKey);
    if (letter != null) {
      return letter + 1;
    }

    return switch (logicalKey) {
      LogicalKeyboardKey.space || LogicalKeyboardKey.digit2 => 0x00,
      LogicalKeyboardKey.bracketLeft || LogicalKeyboardKey.digit3 => 0x1b,
      LogicalKeyboardKey.backslash || LogicalKeyboardKey.digit4 => 0x1c,
      LogicalKeyboardKey.bracketRight || LogicalKeyboardKey.digit5 => 0x1d,
      LogicalKeyboardKey.digit6 => 0x1e,
      LogicalKeyboardKey.slash ||
      LogicalKeyboardKey.digit7 ||
      LogicalKeyboardKey.minus => 0x1f,
      LogicalKeyboardKey.digit8 => 0x7f,
      _ => null,
    };
  }

  int? _letterIndex(LogicalKeyboardKey logicalKey) {
    return switch (logicalKey) {
      LogicalKeyboardKey.keyA => 0,
      LogicalKeyboardKey.keyB => 1,
      LogicalKeyboardKey.keyC => 2,
      LogicalKeyboardKey.keyD => 3,
      LogicalKeyboardKey.keyE => 4,
      LogicalKeyboardKey.keyF => 5,
      LogicalKeyboardKey.keyG => 6,
      LogicalKeyboardKey.keyH => 7,
      LogicalKeyboardKey.keyI => 8,
      LogicalKeyboardKey.keyJ => 9,
      LogicalKeyboardKey.keyK => 10,
      LogicalKeyboardKey.keyL => 11,
      LogicalKeyboardKey.keyM => 12,
      LogicalKeyboardKey.keyN => 13,
      LogicalKeyboardKey.keyO => 14,
      LogicalKeyboardKey.keyP => 15,
      LogicalKeyboardKey.keyQ => 16,
      LogicalKeyboardKey.keyR => 17,
      LogicalKeyboardKey.keyS => 18,
      LogicalKeyboardKey.keyT => 19,
      LogicalKeyboardKey.keyU => 20,
      LogicalKeyboardKey.keyV => 21,
      LogicalKeyboardKey.keyW => 22,
      LogicalKeyboardKey.keyX => 23,
      LogicalKeyboardKey.keyY => 24,
      LogicalKeyboardKey.keyZ => 25,
      _ => null,
    };
  }

  String _csiFinal(String finalByte, TerminalKeyboardModifiers modifiers) {
    final modifier = _xtermModifier(modifiers);
    if (modifier == 0) {
      return '\x1b[$finalByte';
    }

    return '\x1b[1;$modifier$finalByte';
  }

  String _cursorKey(String finalByte, TerminalKeyboardModifiers modifiers) {
    if (mode.applicationCursor && _xtermModifier(modifiers) == 0) {
      return '\x1bO$finalByte';
    }

    return _csiFinal(finalByte, modifiers);
  }

  String _csiTilde(int code, TerminalKeyboardModifiers modifiers) {
    final modifier = _xtermModifier(modifiers);
    if (modifier == 0) {
      return '\x1b[$code~';
    }

    return '\x1b[$code;$modifier~';
  }

  String _functionKey(
    String? ss3Final,
    int? csiCode,
    TerminalKeyboardModifiers modifiers,
  ) {
    final modifier = _xtermModifier(modifiers);
    if (ss3Final != null && modifier == 0) {
      return '\x1bO$ss3Final';
    }
    if (ss3Final != null) {
      return '\x1b[1;$modifier$ss3Final';
    }

    return _csiTilde(csiCode!, modifiers);
  }

  int _xtermModifier(TerminalKeyboardModifiers modifiers) {
    var value = 1;
    if (modifiers.shift) {
      value += 1;
    }
    if (modifiers.alt) {
      value += 2;
    }
    if (modifiers.control) {
      value += 4;
    }

    return value == 1 ? 0 : value;
  }

  String _withAltPrefix(String sequence, TerminalKeyboardModifiers modifiers) {
    if (!config.useOptionAsMetaKey || !modifiers.alt) {
      return sequence;
    }

    return '\x1b$sequence';
  }
}
