enum WorkspacePathCompletionMode {
  filesAndDirectories,
  directoriesOnly;

  bool allows(WorkspacePathCompletionEntry entry) {
    return switch (this) {
      WorkspacePathCompletionMode.filesAndDirectories => true,
      WorkspacePathCompletionMode.directoriesOnly => entry.isDirectory,
    };
  }
}

enum WorkspaceShellPathQuote { none, single, double }

class WorkspacePathCompletionEntry {
  const WorkspacePathCompletionEntry({
    required this.name,
    required this.isDirectory,
  });

  final String name;
  final bool isDirectory;
}

class WorkspaceShellPathCompletionQuery {
  const WorkspaceShellPathCompletionQuery({
    required this.inputPrefix,
    required this.directoryPath,
    required this.parentArgument,
    required this.namePrefix,
    required this.mode,
    required this.quote,
  });

  final String inputPrefix;
  final String directoryPath;
  final String parentArgument;
  final String namePrefix;
  final WorkspacePathCompletionMode mode;
  final WorkspaceShellPathQuote quote;
}

class WorkspaceShellCommandCompletionQuery {
  const WorkspaceShellCommandCompletionQuery({
    required this.leadingWhitespace,
    required this.prefix,
  });

  final String leadingWhitespace;
  final String prefix;

  static WorkspaceShellCommandCompletionQuery? tryParse(String input) {
    if (input.contains('\n')) {
      return null;
    }
    final leadingMatch = RegExp(r'^\s*').firstMatch(input);
    final leadingWhitespace = leadingMatch?.group(0) ?? '';
    final commandPrefix = input.substring(leadingWhitespace.length);
    if (commandPrefix.isEmpty ||
        commandPrefix.contains(RegExp(r'\s')) ||
        commandPrefix.contains('/') ||
        commandPrefix.startsWith('-') ||
        commandPrefix.contains(';') ||
        commandPrefix.contains('|') ||
        commandPrefix.contains('&')) {
      return null;
    }
    return WorkspaceShellCommandCompletionQuery(
      leadingWhitespace: leadingWhitespace,
      prefix: commandPrefix,
    );
  }
}

class WorkspaceComposerCompletion {
  const WorkspaceComposerCompletion._();

  static WorkspaceShellPathCompletionQuery? shellPathQuery(
    String input, {
    required String workingDirectory,
    required bool expandHome,
    String? home,
  }) {
    if (input.isEmpty ||
        input.contains('\n') ||
        input.contains(';') ||
        input.contains('|') ||
        input.contains('&')) {
      return null;
    }

    final tokenStart = _currentShellTokenStart(input);
    final rawToken = input.substring(tokenStart);
    final inputBeforeToken = input.substring(0, tokenStart);
    if (inputBeforeToken.contains("'") || inputBeforeToken.contains('"')) {
      return null;
    }
    final contextWords = _simpleShellWords(inputBeforeToken);
    if (contextWords == null) {
      return null;
    }

    final hasCommand = contextWords.isNotEmpty;
    var inputPrefix = inputBeforeToken;
    var pathToken = rawToken;
    final command = _effectiveShellCommand(contextWords);
    var mode = _pathCompletionModeForContext(command);
    final optionAssignmentIndex = rawToken.indexOf('=');
    if (rawToken.startsWith('-') && optionAssignmentIndex != -1) {
      final option = rawToken.substring(0, optionAssignmentIndex);
      final optionMode = _pathCompletionModeForOption(option);
      if (optionMode == null) {
        return null;
      }
      inputPrefix =
          '$inputBeforeToken${rawToken.substring(0, optionAssignmentIndex + 1)}';
      pathToken = rawToken.substring(optionAssignmentIndex + 1);
      mode = optionMode;
    } else {
      final previousMode = contextWords.isEmpty
          ? null
          : _pathCompletionModeForOption(contextWords.last);
      mode = previousMode ?? mode;
      if (rawToken.startsWith('-') && !rawToken.contains('/')) {
        return null;
      }
    }

    final parsedToken = _parseShellPathToken(
      inputPrefix: inputPrefix,
      rawToken: pathToken,
    );
    if (parsedToken == null) {
      return null;
    }
    inputPrefix = parsedToken.inputPrefix;
    pathToken = parsedToken.pathToken;
    if (!hasCommand &&
        !pathToken.startsWith('/') &&
        !pathToken.startsWith('~')) {
      return null;
    }

    final separatorIndex = pathToken.lastIndexOf('/');
    final parentArgument = separatorIndex == -1
        ? ''
        : pathToken.substring(0, separatorIndex + 1);
    final namePrefix = separatorIndex == -1
        ? pathToken
        : pathToken.substring(separatorIndex + 1);
    final directoryPath = _resolveShellPath(
      parentArgument,
      workingDirectory: workingDirectory,
      expandHome: expandHome,
      home: home,
    );
    if (directoryPath == null) {
      return null;
    }

    return WorkspaceShellPathCompletionQuery(
      inputPrefix: inputPrefix,
      directoryPath: directoryPath,
      parentArgument: parentArgument,
      namePrefix: namePrefix,
      mode: mode,
      quote: parsedToken.quote,
    );
  }

  static List<String> pathCandidates(
    WorkspaceShellPathCompletionQuery query,
    Iterable<WorkspacePathCompletionEntry> entries, {
    int limit = 999,
  }) {
    final normalizedLimit = limit < 1 ? 1 : limit;
    final includeHidden = query.namePrefix.startsWith('.');
    final parentEntry =
        query.parentArgument.isEmpty &&
            (query.namePrefix.isEmpty || '..'.startsWith(query.namePrefix))
        ? const WorkspacePathCompletionEntry(name: '..', isDirectory: true)
        : null;
    final matched = [
      for (final entry in entries)
        if ((includeHidden || !entry.name.startsWith('.')) &&
            query.mode.allows(entry) &&
            entry.name.toLowerCase().startsWith(query.namePrefix.toLowerCase()))
          entry,
    ]..sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
    final ordered = [?parentEntry, for (final entry in matched) entry];

    final candidates = <String>[];
    for (final entry in ordered) {
      final argument =
          '${query.parentArgument}${entry.name}${entry.isDirectory ? '/' : ''}';
      candidates.add(
        '${query.inputPrefix}${_escapeShellPathArgument(argument, query.quote, entry.isDirectory)}',
      );
      if (candidates.length >= normalizedLimit) {
        break;
      }
    }
    return candidates;
  }

  static List<String> commandCandidates(
    WorkspaceShellCommandCompletionQuery query,
    Iterable<String> names, {
    int limit = 999,
  }) {
    final normalizedLimit = limit < 1 ? 1 : limit;
    final candidates = <String>[];
    final seen = <String>{};
    final sortedNames = [
      for (final name in names)
        if (name.isNotEmpty &&
            name.startsWith(query.prefix) &&
            name != query.prefix &&
            seen.add(name))
          name,
    ]..sort((a, b) => a.compareTo(b));

    for (final name in sortedNames) {
      candidates.add('${query.leadingWhitespace}$name');
      if (candidates.length >= normalizedLimit) {
        break;
      }
    }
    return candidates;
  }

  static String escapeShellPath(String path) {
    return path.replaceAllMapped(
      RegExp(r'''([\\\s'"$`!#&;|<>()\[\]{}*?])'''),
      (match) => '\\${match.group(1)}',
    );
  }

  static String unescapeShellPath(String path) {
    final buffer = StringBuffer();
    var escaping = false;
    for (final rune in path.runes) {
      final char = String.fromCharCode(rune);
      if (escaping) {
        buffer.write(char);
        escaping = false;
        continue;
      }
      if (char == r'\') {
        escaping = true;
        continue;
      }
      buffer.write(char);
    }
    return buffer.toString();
  }

  static int _currentShellTokenStart(String input) {
    var tokenStart = 0;
    var escaping = false;
    String? quote;
    for (var index = 0; index < input.length; index++) {
      final char = input[index];
      if (escaping) {
        escaping = false;
        continue;
      }
      if (quote == "'") {
        if (char == "'") {
          quote = null;
        }
        continue;
      }
      if (quote == '"') {
        if (char == r'\') {
          escaping = true;
          continue;
        }
        if (char == '"') {
          quote = null;
        }
        continue;
      }
      if (char == r'\') {
        escaping = true;
        continue;
      }
      if (char == "'" || char == '"') {
        quote = char;
        continue;
      }
      if (char.trim().isEmpty) {
        tokenStart = index + 1;
      }
    }
    return tokenStart;
  }

  static _ShellPathToken? _parseShellPathToken({
    required String inputPrefix,
    required String rawToken,
  }) {
    if (rawToken.isEmpty) {
      return _ShellPathToken(
        inputPrefix: inputPrefix,
        pathToken: '',
        quote: WorkspaceShellPathQuote.none,
      );
    }
    if (rawToken.startsWith("'")) {
      final value = rawToken.substring(1);
      if (value.contains("'")) {
        return null;
      }
      return _ShellPathToken(
        inputPrefix: "$inputPrefix'",
        pathToken: value,
        quote: WorkspaceShellPathQuote.single,
      );
    }
    if (rawToken.startsWith('"')) {
      final value = rawToken.substring(1);
      if (_containsUnescapedDoubleQuote(value)) {
        return null;
      }
      return _ShellPathToken(
        inputPrefix: '$inputPrefix"',
        pathToken: _unescapeDoubleQuotedShellPath(value),
        quote: WorkspaceShellPathQuote.double,
      );
    }
    if (rawToken.contains("'") || rawToken.contains('"')) {
      return null;
    }
    return _ShellPathToken(
      inputPrefix: inputPrefix,
      pathToken: unescapeShellPath(rawToken),
      quote: WorkspaceShellPathQuote.none,
    );
  }

  static bool _containsUnescapedDoubleQuote(String value) {
    var escaping = false;
    for (var index = 0; index < value.length; index++) {
      final char = value[index];
      if (escaping) {
        escaping = false;
        continue;
      }
      if (char == r'\') {
        escaping = true;
        continue;
      }
      if (char == '"') {
        return true;
      }
    }
    return false;
  }

  static String _unescapeDoubleQuotedShellPath(String path) {
    final buffer = StringBuffer();
    var escaping = false;
    for (final rune in path.runes) {
      final char = String.fromCharCode(rune);
      if (escaping) {
        buffer.write(char);
        escaping = false;
        continue;
      }
      if (char == r'\') {
        escaping = true;
        continue;
      }
      buffer.write(char);
    }
    if (escaping) {
      buffer.write(r'\');
    }
    return buffer.toString();
  }

  static List<String>? _simpleShellWords(String input) {
    final words = <String>[];
    final buffer = StringBuffer();
    var escaping = false;
    for (final rune in input.runes) {
      final char = String.fromCharCode(rune);
      if (escaping) {
        buffer.write(char);
        escaping = false;
        continue;
      }
      if (char == r'\') {
        escaping = true;
        continue;
      }
      if (char.trim().isEmpty) {
        if (buffer.isNotEmpty) {
          words.add(buffer.toString());
          buffer.clear();
        }
        continue;
      }
      buffer.write(char);
    }
    if (escaping) {
      return null;
    }
    if (buffer.isNotEmpty) {
      words.add(buffer.toString());
    }
    return words;
  }

  static String? _effectiveShellCommand(List<String> words) {
    var index = 0;
    while (index < words.length &&
        _looksLikeEnvironmentAssignment(words[index])) {
      index++;
    }
    while (index < words.length) {
      final word = words[index];
      if (word == 'sudo') {
        index++;
        while (index < words.length && words[index].startsWith('-')) {
          final option = words[index];
          index++;
          if (_sudoOptionTakesValue(option) && index < words.length) {
            index++;
          }
        }
        continue;
      }
      if (word == 'env') {
        index++;
        while (index < words.length &&
            _looksLikeEnvironmentAssignment(words[index])) {
          index++;
        }
        continue;
      }
      if (word == 'command' ||
          word == 'builtin' ||
          word == 'exec' ||
          word == 'nohup') {
        index++;
        continue;
      }
      return _pathBaseName(word);
    }
    return null;
  }

  static bool _looksLikeEnvironmentAssignment(String word) {
    final equalsIndex = word.indexOf('=');
    if (equalsIndex <= 0) {
      return false;
    }
    return RegExp(r'^[A-Za-z_][A-Za-z0-9_]*$')
        .hasMatch(word.substring(0, equalsIndex));
  }

  static bool _sudoOptionTakesValue(String option) {
    return option == '-u' ||
        option == '-g' ||
        option == '-h' ||
        option == '-p' ||
        option == '-C' ||
        option == '--user' ||
        option == '--group' ||
        option == '--host' ||
        option == '--prompt' ||
        option == '--close-from';
  }

  static WorkspacePathCompletionMode _pathCompletionModeForContext(
    String? command,
  ) {
    if (command == null) {
      return WorkspacePathCompletionMode.filesAndDirectories;
    }
    if (_directoryOnlyCompletionCommands.contains(command)) {
      return WorkspacePathCompletionMode.directoriesOnly;
    }
    return WorkspacePathCompletionMode.filesAndDirectories;
  }

  static WorkspacePathCompletionMode? _pathCompletionModeForOption(
    String option,
  ) {
    final normalized = option.contains('=')
        ? option.substring(0, option.indexOf('='))
        : option;
    if (_directoryValueOptions.contains(normalized) ||
        normalized.endsWith('-dir') ||
        normalized.endsWith('-directory')) {
      return WorkspacePathCompletionMode.directoriesOnly;
    }
    if (_fileValueOptions.contains(normalized) ||
        normalized.endsWith('-file') ||
        normalized.endsWith('-path')) {
      return WorkspacePathCompletionMode.filesAndDirectories;
    }
    return null;
  }

  static String? _resolveShellPath(
    String path, {
    required String workingDirectory,
    required bool expandHome,
    required String? home,
  }) {
    final expanded = expandHome ? _expandHomePath(path, home: home) : path;
    if (expanded.isEmpty) {
      return workingDirectory;
    }
    if (expanded == '~' ||
        expanded.startsWith('~/') ||
        expanded.startsWith('/')) {
      return expanded;
    }
    return _joinPath(workingDirectory, expanded);
  }

  static String _expandHomePath(String path, {required String? home}) {
    if (path == '~') {
      return home ?? path;
    }
    if (path.startsWith('~/')) {
      if (home == null || home.isEmpty) {
        return path;
      }
      return _joinPath(home, path.substring(2));
    }
    return path;
  }

  static String _joinPath(String parent, String child) {
    if (child.isEmpty) {
      return parent;
    }
    if (parent.endsWith('/')) {
      return '$parent$child';
    }
    return '$parent/$child';
  }

  static String _pathBaseName(String path) {
    final trimmed = path.endsWith('/')
        ? path.substring(0, path.length - 1)
        : path;
    final index = trimmed.lastIndexOf('/');
    return index == -1 ? trimmed : trimmed.substring(index + 1);
  }

  static String _escapeShellPathArgument(
    String argument,
    WorkspaceShellPathQuote quote,
    bool isDirectory,
  ) {
    return switch (quote) {
      WorkspaceShellPathQuote.none => escapeShellPath(argument),
      WorkspaceShellPathQuote.single =>
        argument.replaceAll("'", r"""'\''""") + (isDirectory ? '' : "'"),
      WorkspaceShellPathQuote.double =>
        argument
                .replaceAll(r'\', r'\\')
                .replaceAll('"', r'\"')
                .replaceAll(r'$', r'\$')
                .replaceAll('`', r'\`') +
            (isDirectory ? '' : '"'),
    };
  }
}

class _ShellPathToken {
  const _ShellPathToken({
    required this.inputPrefix,
    required this.pathToken,
    required this.quote,
  });

  final String inputPrefix;
  final String pathToken;
  final WorkspaceShellPathQuote quote;
}

const Set<String> _directoryOnlyCompletionCommands = {'cd', 'pushd', 'rmdir'};

const Set<String> _directoryValueOptions = {
  '-C',
  '--chdir',
  '--cwd',
  '--directory',
  '--working-directory',
};

const Set<String> _fileValueOptions = {
  '-c',
  '-f',
  '-i',
  '-o',
  '--cert',
  '--certificate',
  '--config',
  '--file',
  '--identity',
  '--identity-file',
  '--input',
  '--key',
  '--known-hosts',
  '--output',
  '--path',
};
