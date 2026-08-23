import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/shell_history_file_store.dart';
import 'package:nauterm/data/shell_history_reader.dart';
import 'package:nauterm/terminal/terminal_ffi.dart';

void main() {
  final readAt = DateTime.fromMillisecondsSinceEpoch(1720000000000);

  test('parses zsh extended history without deduplicating', () {
    final entries = ShellHistoryReader.parse(
      ': 1720000000:0;git status\n: 1720000001:0;git status\n',
      format: ShellHistoryFormat.zsh,
      readAt: readAt,
      shellPath: '/bin/zsh',
    );
    expect(entries.map((entry) => entry.command), ['git status', 'git status']);
    expect(
      entries.first.createdAt,
      DateTime.fromMillisecondsSinceEpoch(1720000000000),
    );
  });

  test('uses read time for bash entries without timestamps', () {
    final entries = ShellHistoryReader.parse(
      '#1720000000\nls\npwd\n',
      format: ShellHistoryFormat.bash,
      readAt: readAt,
      shellPath: '/bin/bash',
    );
    expect(entries.map((entry) => entry.command), ['ls', 'pwd']);
    expect(
      entries.first.createdAt,
      DateTime.fromMillisecondsSinceEpoch(1720000000000),
    );
  });

  test('parses fish command and when fields', () {
    final entries = ShellHistoryReader.parse(
      '- cmd: git status\n  when: 1720000000\n- cmd: pwd\n',
      format: ShellHistoryFormat.fish,
      readAt: readAt,
      shellPath: '/usr/bin/fish',
    );
    expect(entries.map((entry) => entry.command), ['git status', 'pwd']);
    expect(
      entries.first.createdAt,
      DateTime.fromMillisecondsSinceEpoch(1720000000000),
    );
    expect(entries.last.createdAt, readAt);
  });

  test('reads PowerShell history as plaintext', () {
    final entries = ShellHistoryReader.parse(
      'Get-ChildItem\nGet-Date\n',
      format: ShellHistoryFormat.powerShell,
      readAt: readAt,
      shellPath: 'pwsh',
    );
    expect(entries.map((entry) => entry.command), [
      'Get-ChildItem',
      'Get-Date',
    ]);
  });

  test('normalizes shell paths returned with remote whitespace', () {
    expect(
      ShellHistoryReader.formatForShell('/bin/zsh\r\n'),
      ShellHistoryFormat.zsh,
    );
  });

  test('recognizes Git Bash history from a Windows executable path', () {
    expect(
      ShellHistoryReader.formatForShell(r'C:\Program Files\Git\bin\bash.exe'),
      ShellHistoryFormat.bash,
    );
  });

  test("reads local history containing malformed UTF-8 bytes", () async {
    final directory = await Directory.systemTemp.createTemp(
      "nauterm-history-encoding-",
    );
    addTearDown(() => directory.delete(recursive: true));
    await File("${directory.path}/.zsh_history").writeAsBytes([
      ...": 1720000000:0;echo ".codeUnits,
      0xff,
      ..."\n".codeUnits,
    ]);

    final entries = await ShellHistoryReader.readLocal(
      shellPath: "/bin/zsh",
      homeDirectory: directory.path,
    );

    expect(entries, hasLength(1));
    expect(entries.single.command, "echo \uFFFD");
  });

  test('orders chronological remote history newest first for navigation', () {
    final entries = ShellHistoryReader.newestFirst([
      ShellHistoryEntry(command: 'oldest', createdAt: readAt),
      ShellHistoryEntry(command: 'repeated', createdAt: readAt),
      ShellHistoryEntry(command: 'newest', createdAt: readAt),
      ShellHistoryEntry(command: 'repeated', createdAt: readAt),
    ]);

    expect(entries.map((entry) => entry.command), [
      'repeated',
      'newest',
      'oldest',
    ]);
  });

  test(
    'reads remote history in an isolate without capturing widget state',
    () async {
      final result = await FfiRemoteShellHistoryReader.readSessionInBackground(
        0,
      );

      expect(result.error, isNotNull);
    },
  );

  test('does not claim persisted history support for dash', () {
    expect(ShellHistoryReader.formatForShell('/bin/dash'), isNull);
  });

  test('parses ksh fc listings instead of binary history bytes', () {
    final entries = ShellHistoryReader.parse(
      '  41  echo first\n  42  echo second\n',
      format: ShellHistoryFormat.ksh,
      readAt: readAt,
      shellPath: '/bin/ksh',
    );
    expect(entries.map((entry) => entry.command), [
      'echo first',
      'echo second',
    ]);
  });

  test('parses Nushell history JSON with plaintext and SQLite timestamps', () {
    final entries = ShellHistoryReader.parse(
      '[{"command":"ls","index":0},'
      '{"command":"git status","start_timestamp":1720000001000},'
      '{"command":"pwd","start_timestamp":"2024-07-03T09:46:42Z"}]',
      format: ShellHistoryFormat.nushell,
      readAt: readAt,
      shellPath: '/opt/homebrew/bin/nu',
    );

    expect(entries.map((entry) => entry.command), ['ls', 'git status', 'pwd']);
    expect(entries.first.createdAt, readAt);
    expect(
      entries[1].createdAt,
      DateTime.fromMillisecondsSinceEpoch(1720000001000),
    );
    expect(entries.last.createdAt, DateTime.parse('2024-07-03T09:46:42Z'));
    expect(
      ShellHistoryReader.formatForShell('/opt/homebrew/bin/nu'),
      ShellHistoryFormat.nushell,
    );
  });

  test(
    'aggregate history keeps newest duplicate and round-trips commands',
    () async {
      final directory = await Directory.systemTemp.createTemp(
        'nauterm-history-',
      );
      addTearDown(() => directory.delete(recursive: true));
      final store = ShellHistoryFileStore(File('${directory.path}/history'));
      final older = readAt.subtract(const Duration(seconds: 1));
      final newer = readAt.add(const Duration(seconds: 1));

      await store.merge([
        ShellHistoryEntry(command: 'echo repeated', createdAt: older),
        ShellHistoryEntry(command: r'printf "a\\b"', createdAt: readAt),
        ShellHistoryEntry(command: 'echo repeated', createdAt: newer),
        ShellHistoryEntry(command: 'printf "one\ntwo"', createdAt: newer),
      ]);
      final entries = await store.read();

      expect(entries.map((entry) => entry.command), [
        'printf "one\ntwo"',
        'echo repeated',
        r'printf "a\\b"',
      ]);
      expect(entries[1].createdAt, newer);
    },
  );

  test('aggregate history retains up to 10000 unique commands', () async {
    final directory = await Directory.systemTemp.createTemp('nauterm-history-');
    addTearDown(() => directory.delete(recursive: true));
    final store = ShellHistoryFileStore(File('${directory.path}/history'));

    final entries = await store.merge([
      for (var index = 0; index <= ShellHistoryFileStore.defaultLimit; index++)
        ShellHistoryEntry(command: 'command-$index', createdAt: readAt),
    ]);

    expect(entries, hasLength(10000));
    expect(entries.first.command, 'command-10000');
    expect(entries.last.command, 'command-1');
  });

  test('aggregate history can be cleared', () async {
    final directory = await Directory.systemTemp.createTemp('nauterm-history-');
    addTearDown(() => directory.delete(recursive: true));
    final store = ShellHistoryFileStore(File('${directory.path}/history'));

    await store.append(
      ShellHistoryEntry(command: 'echo clear me', createdAt: readAt),
    );
    await store.clear();

    expect(await store.read(), isEmpty);
  });
}
