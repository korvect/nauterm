enum HostConnectionCommand { ssh, telnet }

/// A temporary connection target parsed without invoking a shell.
class HostConnectionQuery {
  const HostConnectionQuery({
    required this.host,
    this.username = '',
    this.port,
    this.command,
  });

  final String host;
  final String username;

  /// Null preserves each protocol's default port until a protocol is chosen.
  final int? port;
  final HostConnectionCommand? command;

  static bool isExplicitConnectionQuery(String input) {
    final tokens = _splitArguments(input);
    if (tokens.isEmpty) return false;
    if (tokens.first.toLowerCase() == 'ssh' ||
        tokens.first.toLowerCase() == 'telnet') {
      return true;
    }
    return tokens.any(
      (token) =>
          token.contains('@') ||
          token == '-p' ||
          token.startsWith('-p') ||
          token == '-l' ||
          token.startsWith('-l'),
    );
  }

  String get label {
    final address = host.contains(':') ? '[$host]' : host;
    final target = username.isEmpty ? address : '$username@$address';
    return port == null ? target : '$target -p $port';
  }

  static HostConnectionQuery? parse(String input) {
    final tokens = _splitArguments(input);
    if (tokens.isEmpty) return null;
    final command = switch (tokens.first.toLowerCase()) {
      'ssh' => HostConnectionCommand.ssh,
      'telnet' => HostConnectionCommand.telnet,
      _ => null,
    };
    final start = command == null ? 0 : 1;
    String? loginName;
    int? port;
    var options = true;
    final positional = <String>[];

    for (var index = start; index < tokens.length; index++) {
      final token = tokens[index];
      if (options && token == '--') {
        options = false;
        continue;
      }
      if (options && token.startsWith('-') && token.length > 1) {
        final option = token.substring(0, 2);
        if (option == '-p' || option == '-l') {
          final attached = token.substring(2);
          final value = attached.isNotEmpty
              ? attached
              : index + 1 < tokens.length && !tokens[index + 1].startsWith('-')
              ? tokens[++index]
              : '';
          if (option == '-p') {
            port = int.tryParse(value) ?? port;
          } else if (value.isNotEmpty) {
            loginName = value;
          }
        } else if (_optionsWithValues.contains(option)) {
          if (token.length == 2 &&
              index + 1 < tokens.length &&
              !tokens[index + 1].startsWith('-')) {
            index++;
          }
        }
        continue;
      }
      positional.add(token);
    }

    final destination =
        positional.where((token) => token.contains('@')).firstOrNull ??
        positional.firstOrNull;
    if (destination == null || destination.isEmpty) return null;
    final at = destination.lastIndexOf('@');
    final username = at < 0
        ? loginName ?? ''
        : loginName ?? destination.substring(0, at);
    var host = at < 0 ? destination : destination.substring(at + 1);
    if (host.startsWith('[') && host.endsWith(']')) {
      host = host.substring(1, host.length - 1);
    }

    if (port == null && command == HostConnectionCommand.telnet) {
      final destinationIndex = positional.indexOf(destination);
      final next =
          destinationIndex < 0 || destinationIndex + 1 >= positional.length
          ? null
          : int.tryParse(positional[destinationIndex + 1]);
      if (next != null) port = next;
    }

    return host.isEmpty
        ? null
        : HostConnectionQuery(
            host: host,
            username: username,
            port: port,
            command: command,
          );
  }

  static const _optionsWithValues = {
    '-B',
    '-b',
    '-c',
    '-D',
    '-E',
    '-e',
    '-F',
    '-I',
    '-i',
    '-J',
    '-L',
    '-m',
    '-O',
    '-Q',
    '-R',
    '-S',
    '-W',
    '-w',
    '-o',
  };

  static List<String> _splitArguments(String input) {
    final arguments = <String>[];
    var argument = StringBuffer();
    String? quote;
    var started = false;
    for (var index = 0; index < input.length; index++) {
      final character = input[index];
      if (character == '\\' && quote != "'") {
        if (index + 1 == input.length) {
          argument.write(character);
        } else {
          final next = input[index + 1];
          if (quote == null || '"\\\$'.contains(next)) {
            argument.write(next);
            index++;
          } else {
            argument.write(character);
          }
        }
        started = true;
      } else if (quote != null) {
        if (character == quote) {
          quote = null;
        } else {
          argument.write(character);
        }
        started = true;
      } else if (character == "'" || character == '"') {
        quote = character;
        started = true;
      } else if (character.trim().isEmpty) {
        if (started) {
          arguments.add(argument.toString());
          argument = StringBuffer();
          started = false;
        }
      } else {
        argument.write(character);
        started = true;
      }
    }
    if (started) arguments.add(argument.toString());
    return arguments;
  }
}
