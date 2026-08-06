import 'nauterm_data_store.dart';

enum HostImportSource { csv, openSsh, putty, mobaXterm, secureCrt }

enum HostImportKeyKind { privateKey, publicKey, keyPair }

class HostImportBundle {
  const HostImportBundle({this.hosts = const [], this.keys = const []});

  final List<HostImportCandidate> hosts;
  final List<HostImportKeyCandidate> keys;
}

class HostImportKeyCandidate {
  const HostImportKeyCandidate({
    required this.name,
    required this.kind,
    this.privateKey,
    this.publicKey,
    this.privatePath,
    this.publicPath,
  });

  final String name;
  final HostImportKeyKind kind;
  final String? privateKey;
  final String? publicKey;
  final String? privatePath;
  final String? publicPath;

  String get detail => switch (kind) {
    HostImportKeyKind.privateKey => 'Private key',
    HostImportKeyKind.publicKey => 'Public key',
    HostImportKeyKind.keyPair => 'Public and private key',
  };
}

class OpenSshImportFile {
  const OpenSshImportFile({required this.path, required this.contents});

  final String path;
  final String contents;
}

List<HostImportKeyCandidate> collectOpenSshKeys(
  Iterable<OpenSshImportFile> files,
) {
  final partsByBasePath = <String, _OpenSshKeyParts>{};
  for (final file in files) {
    final name = file.path.split(RegExp(r'[/\\]')).last;
    if (isOpenSshPrivateKey(file.contents)) {
      final parts = partsByBasePath.putIfAbsent(
        file.path,
        () => _OpenSshKeyParts(name: name),
      );
      parts
        ..name = name
        ..privateKey = file.contents
        ..privatePath = file.path;
    } else if (isOpenSshPublicKey(file.contents)) {
      final basePath = file.path.toLowerCase().endsWith('.pub')
          ? file.path.substring(0, file.path.length - 4)
          : file.path;
      final parts = partsByBasePath.putIfAbsent(
        basePath,
        () => _OpenSshKeyParts(name: name),
      );
      parts
        ..publicKey = file.contents.trim()
        ..publicPath = file.path;
    }
  }
  return [for (final parts in partsByBasePath.values) parts.toCandidate()]
    ..sort((left, right) => left.name.compareTo(right.name));
}

bool isOpenSshPrivateKey(String input) => RegExp(
  r'-----BEGIN (?:OPENSSH |RSA |EC |DSA |ENCRYPTED )?PRIVATE KEY-----',
).hasMatch(input);

bool isOpenSshPublicKey(String input) {
  final value = input.trimLeft();
  return value.startsWith('ssh-rsa ') ||
      value.startsWith('ssh-ed25519 ') ||
      value.startsWith('ecdsa-sha2-') ||
      value.startsWith('sk-ssh-') ||
      value.startsWith('sk-ecdsa-');
}

class _OpenSshKeyParts {
  _OpenSshKeyParts({required this.name});

  String name;
  String? privateKey;
  String? publicKey;
  String? privatePath;
  String? publicPath;

  HostImportKeyCandidate toCandidate() {
    final kind = privateKey != null && publicKey != null
        ? HostImportKeyKind.keyPair
        : privateKey != null
        ? HostImportKeyKind.privateKey
        : HostImportKeyKind.publicKey;
    return HostImportKeyCandidate(
      name: name,
      kind: kind,
      privateKey: privateKey,
      publicKey: publicKey,
      privatePath: privatePath,
      publicPath: publicPath,
    );
  }
}

class HostImportCandidate {
  const HostImportCandidate({
    required this.source,
    required this.name,
    required this.host,
    this.protocol = 'ssh',
    this.port,
    this.username,
    this.password,
    this.groupPath,
    this.tags = const [],
    this.identityFile,
  });

  final HostImportSource source;
  final String name;
  final String host;
  final String protocol;
  final int? port;
  final String? username;
  final String? password;
  final String? groupPath;
  final List<String> tags;
  final String? identityFile;

  String get address => port == null ? host : '$host:$port';
}

List<HostImportCandidate> parseHostCsv(String input) {
  final rows = _parseCsv(input);
  if (rows.isEmpty) return const [];
  final headers = <String, int>{
    for (var index = 0; index < rows.first.length; index++)
      rows.first[index].trim().toLowerCase(): index,
  };
  String value(List<String> row, String header) {
    final index = headers[header];
    return index == null || index >= row.length ? '' : row[index].trim();
  }

  return [
    for (final row in rows.skip(1))
      if (value(row, 'hostname/ip').isNotEmpty)
        HostImportCandidate(
          source: HostImportSource.csv,
          name: value(row, 'label').isEmpty
              ? value(row, 'hostname/ip')
              : value(row, 'label'),
          host: value(row, 'hostname/ip'),
          protocol: value(row, 'protocol').isEmpty
              ? 'ssh'
              : value(row, 'protocol').toLowerCase(),
          port: int.tryParse(value(row, 'port')),
          username: _nullIfEmpty(value(row, 'username')),
          password: _nullIfEmpty(value(row, 'password')),
          groupPath: _nullIfEmpty(value(row, 'groups')),
          tags: value(row, 'tags')
              .split(',')
              .map((tag) => tag.trim())
              .where((tag) => tag.isNotEmpty)
              .toList(growable: false),
        ),
  ];
}

List<HostImportCandidate> parseOpenSshConfig(String input) {
  final global = <String, String>{};
  final blocks = <_SshHostBlock>[];
  _SshHostBlock? current;
  for (final rawLine in input.split(RegExp(r'\r?\n'))) {
    final line = _stripSshComment(rawLine).trim();
    if (line.isEmpty) continue;
    final match = RegExp(r'^(\S+)\s*(?:=\s*)?(.*)$').firstMatch(line);
    if (match == null) continue;
    final key = match.group(1)!.toLowerCase();
    final value = _unquote(match.group(2)!.trim());
    if (key == 'host') {
      current = _SshHostBlock(_splitSshWords(value));
      blocks.add(current);
    } else if (current == null) {
      global.putIfAbsent(key, () => value);
    } else {
      current.values.putIfAbsent(key, () => value);
    }
  }

  final result = <HostImportCandidate>[];
  final wildcardDefaults = <String, String>{};
  for (final block in blocks.where((block) => block.aliases.contains('*'))) {
    for (final entry in block.values.entries) {
      wildcardDefaults.putIfAbsent(entry.key, () => entry.value);
    }
  }
  for (final block in blocks) {
    final aliases = block.aliases.where(
      (alias) =>
          alias.isNotEmpty &&
          !alias.startsWith('!') &&
          !alias.contains('*') &&
          !alias.contains('?'),
    );
    for (final alias in aliases) {
      String? setting(String name) => _nullIfEmpty(
        block.values[name] ?? global[name] ?? wildcardDefaults[name] ?? '',
      );
      final hostname = setting('hostname') ?? alias;
      result.add(
        HostImportCandidate(
          source: HostImportSource.openSsh,
          name: alias,
          host: hostname,
          port: int.tryParse(setting('port') ?? ''),
          username: setting('user'),
          identityFile: setting('identityfile'),
        ),
      );
    }
  }
  return result;
}

List<HostImportCandidate> parseOpenSshKnownHosts(String input) {
  final result = <HostImportCandidate>[];
  for (final rawLine in input.split(RegExp(r'\r?\n'))) {
    final line = rawLine.trim();
    if (line.isEmpty || line.startsWith('#')) continue;
    final fields = line.split(RegExp(r'\s+'));
    final hostIndex = fields.first.startsWith('@') ? 1 : 0;
    if (fields.length <= hostIndex + 1) continue;
    for (final pattern in fields[hostIndex].split(',')) {
      final target = _parseKnownHostImportTarget(pattern.trim());
      if (target == null) continue;
      result.add(
        HostImportCandidate(
          source: HostImportSource.openSsh,
          name: target.host,
          host: target.host,
          port: target.port,
        ),
      );
    }
  }
  return result;
}

List<HostImportCandidate> parsePuttyRegistry(String input) {
  final result = <HostImportCandidate>[];
  String? session;
  final values = <String, String>{};

  void flush() {
    final host = values['hostname']?.trim();
    if (session == null || host == null || host.isEmpty) return;
    result.add(
      HostImportCandidate(
        source: HostImportSource.putty,
        name: Uri.decodeComponent(session.replaceAll('+', ' ')),
        host: host,
        port: int.tryParse(values['portnumber'] ?? ''),
        username: _nullIfEmpty(values['username'] ?? ''),
        identityFile: _nullIfEmpty(values['publickeyfile'] ?? ''),
      ),
    );
  }

  for (final raw in input.split(RegExp(r'\r?\n'))) {
    final line = raw.trim();
    final section = RegExp(
      r'^\[.*\\PuTTY\\Sessions\\([^\]]+)\]$',
      caseSensitive: false,
    ).firstMatch(line);
    if (section != null) {
      flush();
      session = section.group(1);
      values.clear();
      continue;
    }
    final stringValue = RegExp(r'^"([^"]+)"="(.*)"$').firstMatch(line);
    if (stringValue != null) {
      values[stringValue.group(1)!.toLowerCase()] = stringValue
          .group(2)!
          .replaceAll(r'\\', r'\')
          .replaceAll(r'\"', '"');
      continue;
    }
    final dword = RegExp(
      r'^"([^"]+)"=dword:([0-9a-f]+)$',
      caseSensitive: false,
    ).firstMatch(line);
    if (dword != null) {
      values[dword.group(1)!.toLowerCase()] = int.parse(
        dword.group(2)!,
        radix: 16,
      ).toString();
    }
  }
  flush();
  return result;
}

List<HostImportCandidate> parseMobaXtermSessions(String input) {
  final result = <HostImportCandidate>[];
  String? group;
  for (final raw in input.split(RegExp(r'\r?\n'))) {
    final line = raw.trim();
    if (line.isEmpty || line.startsWith(';')) continue;
    final section = RegExp(r'^\[([^\]]+)\]$').firstMatch(line);
    if (section != null) {
      final name = section.group(1)!;
      group = name.toLowerCase().startsWith('bookmarks') ? name : null;
      continue;
    }
    final equals = line.indexOf('=');
    if (equals <= 0) continue;
    final name = line.substring(0, equals).trim();
    final encoded = line.substring(equals + 1).trim();
    if (name.toLowerCase() == 'subrep') {
      group = _nullIfEmpty(encoded);
      continue;
    }
    if (!encoded.startsWith('#109#')) continue;
    final fields = encoded.split('%');
    if (fields.length < 2 || fields[1].trim().isEmpty) continue;
    result.add(
      HostImportCandidate(
        source: HostImportSource.mobaXterm,
        name: name,
        host: fields[1].trim(),
        port: fields.length > 2 ? int.tryParse(fields[2]) : null,
        username: fields.length > 3 ? _nullIfEmpty(fields[3].trim()) : null,
        groupPath: group == null || group.toLowerCase() == 'bookmarks'
            ? null
            : group,
      ),
    );
  }
  return result;
}

HostImportCandidate? parseSecureCrtSession(
  String input, {
  required String name,
}) {
  final values = <String, String>{};
  for (final raw in input.split(RegExp(r'\r?\n'))) {
    final line = raw.trim();
    final stringValue = RegExp(r'^S:"([^"]+)"=(.*)$').firstMatch(line);
    if (stringValue != null) {
      values[stringValue.group(1)!.toLowerCase()] = stringValue
          .group(2)!
          .trim();
      continue;
    }
    final dword = RegExp(r'^D:"([^"]+)"=([0-9a-f]+)$').firstMatch(line);
    if (dword != null) {
      values[dword.group(1)!.toLowerCase()] = int.parse(
        dword.group(2)!,
        radix: 16,
      ).toString();
    }
  }
  final host = values['hostname'];
  if (host == null || host.isEmpty) return null;
  return HostImportCandidate(
    source: HostImportSource.secureCrt,
    name: name,
    host: host,
    port: int.tryParse(values['[ssh2] port'] ?? values['port'] ?? ''),
    username: _nullIfEmpty(values['username'] ?? ''),
    identityFile: _nullIfEmpty(values['identity filename v2'] ?? ''),
  );
}

List<List<String>> _parseCsv(String input) {
  final rows = <List<String>>[];
  var row = <String>[];
  final field = StringBuffer();
  var quoted = false;
  for (var index = 0; index < input.length; index++) {
    final char = input[index];
    if (char == '"') {
      if (quoted && index + 1 < input.length && input[index + 1] == '"') {
        field.write('"');
        index++;
      } else {
        quoted = !quoted;
      }
    } else if (char == ',' && !quoted) {
      row.add(field.toString());
      field.clear();
    } else if ((char == '\n' || char == '\r') && !quoted) {
      if (char == '\r' &&
          index + 1 < input.length &&
          input[index + 1] == '\n') {
        index++;
      }
      row.add(field.toString());
      field.clear();
      if (row.any((value) => value.isNotEmpty)) rows.add(row);
      row = <String>[];
    } else {
      field.write(char);
    }
  }
  row.add(field.toString());
  if (row.any((value) => value.isNotEmpty)) rows.add(row);
  return rows;
}

class _SshHostBlock {
  _SshHostBlock(this.aliases);

  final List<String> aliases;
  final Map<String, String> values = {};
}

({String host, int port})? _parseKnownHostImportTarget(String pattern) {
  if (pattern.isEmpty ||
      pattern.startsWith('|') ||
      pattern.contains('*') ||
      pattern.contains('?')) {
    return null;
  }
  if (!pattern.startsWith('[')) return (host: pattern, port: 22);
  final closeBracket = pattern.indexOf(']');
  if (closeBracket <= 1 ||
      closeBracket + 2 >= pattern.length ||
      pattern[closeBracket + 1] != ':') {
    return null;
  }
  final port = int.tryParse(pattern.substring(closeBracket + 2));
  if (port == null || port < 1 || port > 65535) return null;
  return (host: pattern.substring(1, closeBracket), port: port);
}

String _stripSshComment(String input) {
  var quoted = false;
  for (var i = 0; i < input.length; i++) {
    if (input[i] == '"' || input[i] == "'") quoted = !quoted;
    if (input[i] == '#' && !quoted) return input.substring(0, i);
  }
  return input;
}

List<String> _splitSshWords(String input) => input
    .split(RegExp(r'\s+'))
    .map(_unquote)
    .where((value) => value.isNotEmpty)
    .toList(growable: false);

String _unquote(String input) {
  if (input.length >= 2 &&
      ((input.startsWith('"') && input.endsWith('"')) ||
          (input.startsWith("'") && input.endsWith("'")))) {
    return input.substring(1, input.length - 1);
  }
  return input;
}

String? _nullIfEmpty(String value) => value.isEmpty ? null : value;

/// Serializes hosts to the same CSV layout consumed by [parseHostCsv], so an
/// exported file can be re-imported losslessly.
String buildHostCsv(
  List<HostEntry> hosts,
  List<HostGroup> groups,
  List<TagEntry> tags,
) {
  const headers = [
    'Label',
    'Hostname/IP',
    'Protocol',
    'Port',
    'Username',
    'Password',
    'Groups',
    'Tags',
  ];
  final byId = {
    for (final group in groups)
      if (group.id != null) group.id!: group,
  };
  final tagByUuid = {
    for (final tag in tags)
      if (tag.uuid != null) tag.uuid!: tag,
  };
  final rows = <List<String>>[headers];
  for (final host in hosts) {
    if (host.type == NautermHostType.local) continue;
    final isTelnet = host.telnetEnabled;
    final tagNames = [
      for (final uuid in host.tagUuids)
        if (tagByUuid[uuid] case final tag?) tag.name,
    ];
    rows.add([
      host.name,
      host.host ?? '',
      isTelnet ? 'telnet' : 'ssh',
      host.port?.toString() ?? '',
      isTelnet ? host.telnetUsername ?? '' : host.username ?? '',
      isTelnet ? host.telnetPassword ?? '' : host.password ?? '',
      _exportGroupPath(byId, host.groupId),
      tagNames.join(','),
    ]);
  }
  return _writeHostCsv(rows);
}

String _exportGroupPath(Map<int, HostGroup> byId, int? groupId) {
  if (groupId == null) return '';
  final names = <String>[];
  final seen = <int>{};
  var current = byId[groupId];
  while (current != null && current.id != null && seen.add(current.id!)) {
    names.insert(0, current.name);
    final parentId = current.parentId;
    current = parentId == null ? null : byId[parentId];
  }
  return names.join('/');
}

String _writeHostCsv(List<List<String>> rows) {
  final buffer = StringBuffer();
  for (var rowIndex = 0; rowIndex < rows.length; rowIndex++) {
    if (rowIndex > 0) buffer.write('\r\n');
    buffer.write(
      [for (final field in rows[rowIndex]) _csvField(field)].join(','),
    );
  }
  return buffer.toString();
}

String _csvField(String value) {
  if (value.isEmpty) return '';
  final needsQuote =
      value.contains(',') ||
      value.contains('"') ||
      value.contains('\n') ||
      value.contains('\r');
  if (!needsQuote) return value;
  return '"${value.replaceAll('"', '""')}"';
}
