import 'package:flutter/foundation.dart';

enum TerminalShellKind { zsh, bash, fish, ksh, tcsh, posix, powerShell, cmd }

bool terminalShellSupportsStructuredIntegration(TerminalShellKind kind) {
  return switch (kind) {
    TerminalShellKind.zsh ||
    TerminalShellKind.bash ||
    TerminalShellKind.fish => true,
    _ => false,
  };
}

@immutable
class TerminalShellIntegration {
  const TerminalShellIntegration(this.kind, this.token);

  final TerminalShellKind kind;
  final String token;

  String get parkLineSequence => '\x18\x1d';
}

TerminalShellKind? terminalShellKindFromPath(String? path) {
  final normalized = (path ?? '').trim().toLowerCase().replaceAll('\\', '/');
  final name = normalized.split('/').last;
  return switch (name) {
    'zsh' || 'zsh.exe' || 'rzsh' => TerminalShellKind.zsh,
    'bash' || 'bash.exe' || 'rbash' => TerminalShellKind.bash,
    'fish' || 'fish.exe' => TerminalShellKind.fish,
    'ksh' || 'ksh93' => TerminalShellKind.ksh,
    'tcsh' || 'csh' => TerminalShellKind.tcsh,
    'sh' || 'dash' || 'ash' => TerminalShellKind.posix,
    'pwsh' ||
    'pwsh.exe' ||
    'powershell' ||
    'powershell.exe' => TerminalShellKind.powerShell,
    'cmd' || 'cmd.exe' => TerminalShellKind.cmd,
    _ => null,
  };
}

class TerminalShellAnnouncementParser {
  static const _prefix = '\x1b]4545;Shell;';
  static const _maximumBufferedCharacters = 4096;

  String _pending = '';

  String? add(List<int> bytes) {
    if (bytes.isEmpty) {
      return null;
    }
    _pending += String.fromCharCodes(bytes);
    while (true) {
      final start = _pending.indexOf(_prefix);
      if (start < 0) {
        final retain = _prefix.length - 1;
        if (_pending.length > retain) {
          _pending = _pending.substring(_pending.length - retain);
        }
        return null;
      }

      final valueStart = start + _prefix.length;
      final bell = _pending.indexOf('\x07', valueStart);
      final stringTerminator = _pending.indexOf('\x1b\\', valueStart);
      final terminator = switch ((bell, stringTerminator)) {
        (-1, -1) => -1,
        (-1, final value) => value,
        (final value, -1) => value,
        (final left, final right) => left < right ? left : right,
      };
      if (terminator < 0) {
        if (_pending.length > _maximumBufferedCharacters) {
          _pending = _pending.substring(start);
        }
        return null;
      }

      final shell = _pending.substring(valueStart, terminator).trim();
      final terminatorLength = terminator == bell ? 1 : 2;
      _pending = _pending.substring(terminator + terminatorLength);
      if (shell.isNotEmpty) {
        return shell;
      }
    }
  }
}

String terminalShellSetupCommand(TerminalShellKind kind, String token) {
  final ready =
      "printf '\\033]777;nauterm-integration-ready=$token\\007'; "
      "printf '\\r\\033[2K'";
  final template = switch (kind) {
    TerminalShellKind.zsh => _zshSetup,
    TerminalShellKind.bash => _bashSetup,
    TerminalShellKind.fish => _fishSetup,
    _ => '',
  };
  return template.replaceAll('{{TOKEN}}', token).replaceAll('{{READY}}', ready);
}

const String _zshSetup =
    r''' typeset -g __nauterm_ai_armed=0; typeset -g __nauterm_command_active=0; typeset -g __nauterm_ai_token='{{TOKEN}}'; function __nauterm_command_preexec() { local __nauterm_command_encoded; __nauterm_command_encoded=$(printf '%s' "$1" | command base64 2>/dev/null | command tr -d '\r\n'); printf '\033]4545;CommandStarted;%s\007\033]133;C\007' "$__nauterm_command_encoded"; __nauterm_command_active=1; }; function __nauterm_ai_precmd() { local __nauterm_ai_status=$?; if [[ ${__nauterm_command_active:-0} -eq 1 ]]; then printf '\033]4545;CommandExited;%s\007\033]133;D;%s\007' "$__nauterm_ai_status" "$__nauterm_ai_status"; __nauterm_command_active=0; fi; if [[ ${__nauterm_ai_armed:-0} -eq 1 ]]; then printf '\033]777;nauterm-command-end=%s;%s\007' "$__nauterm_ai_token" "$__nauterm_ai_status"; __nauterm_ai_armed=0; fi; printf '\033]7;file://localhost%s\007\033]133;A\007' "$PWD"; }; function __nauterm_line_init() { printf '\033]133;B\007'; }; function __nauterm_ai_park_line() { if [[ -n "$BUFFER" ]]; then zle -I; print -r -- ''; BUFFER=''; CURSOR=0; fi; __nauterm_ai_armed=1; printf '\033]777;nauterm-line-ready=%s\007' "$__nauterm_ai_token"; }; autoload -Uz add-zsh-hook add-zle-hook-widget; add-zsh-hook -d preexec __nauterm_command_preexec 2>/dev/null; add-zsh-hook preexec __nauterm_command_preexec; add-zsh-hook -d precmd __nauterm_ai_precmd 2>/dev/null; add-zsh-hook precmd __nauterm_ai_precmd; add-zle-hook-widget -d line-init __nauterm_line_init 2>/dev/null; add-zle-hook-widget line-init __nauterm_line_init; zle -N __nauterm_ai_park_line; bindkey '^X^]' __nauterm_ai_park_line; {{READY}}''';

const String _bashSetup =
    r''' __nauterm_ai_armed=0; __nauterm_ai_token='{{TOKEN}}'; __nauterm_last_histcmd=$HISTCMD; __nauterm_ai_precmd() { local __nauterm_ai_status=$? __nauterm_command __nauterm_command_encoded; if [[ ${HISTCMD:-0} -gt ${__nauterm_last_histcmd:-0} ]]; then __nauterm_command=$(builtin fc -ln -1 2>/dev/null); if [[ -n "$__nauterm_command" && "$__nauterm_command" != *"__nauterm_ai_token="* ]]; then __nauterm_command_encoded=$(printf '%s' "$__nauterm_command" | command base64 2>/dev/null | command tr -d '\r\n'); printf '\033]4545;CommandStarted;%s\007\033]133;C\007\033]4545;CommandExited;%s\007\033]133;D;%s\007' "$__nauterm_command_encoded" "$__nauterm_ai_status" "$__nauterm_ai_status"; fi; __nauterm_last_histcmd=$HISTCMD; fi; if [[ ${__nauterm_ai_armed:-0} -eq 1 ]]; then printf '\033]777;nauterm-command-end=%s;%s\007' "$__nauterm_ai_token" "$__nauterm_ai_status"; __nauterm_ai_armed=0; fi; printf '\033]7;file://localhost%s\007\033]133;A\007' "$PWD"; }; __nauterm_ai_park_begin() { printf '\n'; __nauterm_ai_armed=1; }; __nauterm_ai_park_ready() { printf '\033]777;nauterm-line-ready=%s\007' "$__nauterm_ai_token"; }; bind -x '"\C-x\C-p":__nauterm_ai_park_begin'; bind -x '"\C-x\C-o":__nauterm_ai_park_ready'; bind '"\C-x\C-]":"\C-x\C-p\C-u\C-x\C-o"'; if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == "declare -a"* ]]; then __nauterm_ai_prompt_commands=(); for __nauterm_ai_prompt_command in "${PROMPT_COMMAND[@]}"; do [[ "$__nauterm_ai_prompt_command" == "__nauterm_ai_precmd" ]] || __nauterm_ai_prompt_commands+=("$__nauterm_ai_prompt_command"); done; PROMPT_COMMAND=(__nauterm_ai_precmd "${__nauterm_ai_prompt_commands[@]}"); unset __nauterm_ai_prompt_command __nauterm_ai_prompt_commands; else __nauterm_ai_prompt_command=${PROMPT_COMMAND:-}; __nauterm_ai_prompt_command=${__nauterm_ai_prompt_command//__nauterm_ai_precmd;/}; __nauterm_ai_prompt_command=${__nauterm_ai_prompt_command//;__nauterm_ai_precmd/}; [[ "$__nauterm_ai_prompt_command" == "__nauterm_ai_precmd" ]] && __nauterm_ai_prompt_command=''; PROMPT_COMMAND="__nauterm_ai_precmd${__nauterm_ai_prompt_command:+;$__nauterm_ai_prompt_command}"; unset __nauterm_ai_prompt_command; fi; [[ "$PS1" == *'\[\e]133;B\a\]' ]] || PS1="${PS1}"'\[\e]133;B\a\]'; [[ "$PS2" == *'\[\e]133;B\a\]' ]] || PS2="${PS2}"'\[\e]133;B\a\]'; {{READY}}''';

const String _fishSetup =
    r''' set -g __nauterm_ai_armed 0; set -g __nauterm_ai_token '{{TOKEN}}'; functions -e __nauterm_ai_postexec __nauterm_shell_prompt 2>/dev/null; function __nauterm_ai_postexec --on-event fish_postexec; set -l __nauterm_ai_status $status; set -l __nauterm_command_encoded (printf '%s' "$argv[1]" | command base64 2>/dev/null | string collect | string replace -a \n ''); if test -n "$__nauterm_command_encoded"; printf '\033]4545;CommandStarted;%s\007\033]133;C\007\033]4545;CommandExited;%s\007\033]133;D;%s\007' $__nauterm_command_encoded $__nauterm_ai_status $__nauterm_ai_status; end; if test $__nauterm_ai_armed -eq 1; printf '\033]777;nauterm-command-end=%s;%s\007' $__nauterm_ai_token $__nauterm_ai_status; set -g __nauterm_ai_armed 0; end; end; function __nauterm_shell_prompt --on-event fish_prompt; printf '\033]7;file://localhost%s\007\033]133;A\007' $PWD; end; if functions -q __nauterm_original_fish_prompt; functions -e fish_prompt; functions -c __nauterm_original_fish_prompt fish_prompt; end; functions -e __nauterm_original_fish_prompt 2>/dev/null; functions -c fish_prompt __nauterm_original_fish_prompt; function fish_prompt; __nauterm_original_fish_prompt; printf '\033]133;B\007'; end; function __nauterm_ai_park_line; if test -n (commandline); echo; commandline -r ''; commandline -f repaint; end; set -g __nauterm_ai_armed 1; printf '\033]777;nauterm-line-ready=%s\007' $__nauterm_ai_token; end; bind \cx\c] __nauterm_ai_park_line; {{READY}}''';
