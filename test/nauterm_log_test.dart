import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_log.dart';

void main() {
  test('writes structured local logs and redacts sensitive fields', () async {
    final directory = await Directory.systemTemp.createTemp('nauterm-log-');
    addTearDown(() => directory.delete(recursive: true));
    NautermLog.initialize(directory);

    NautermLog.info(
      'sync',
      'Sync completed.',
      fields: const {
        'provider': 's3',
        'object_count': 3,
        'access_token': 'must-not-appear',
        'database_path': '/private/data.sqlite',
      },
    );
    await NautermLog.flush();

    final contents = await File(
      '${directory.path}${Platform.pathSeparator}nauterm.log',
    ).readAsString();
    expect(contents, contains('INFO [sync] Sync completed.'));
    expect(contents, contains('"provider":"s3"'));
    expect(contents, contains('"object_count":3'));
    expect(contents, isNot(contains('must-not-appear')));
    expect(contents, isNot(contains('/private/data.sqlite')));
    expect(contents, contains('[redacted]'));
  });

  test('operation logs include an id and duration', () async {
    final directory = await Directory.systemTemp.createTemp('nauterm-log-op-');
    addTearDown(() => directory.delete(recursive: true));
    NautermLog.initialize(directory);

    final operation = NautermLog.begin('database', 'Database open');
    operation.succeed(fields: const {'schema_version': 1});
    await NautermLog.flush();

    final lines = await File(
      '${directory.path}${Platform.pathSeparator}nauterm.log',
    ).readAsLines();
    expect(lines, hasLength(2));
    final completedFields = jsonDecode(
      lines.last.substring(lines.last.indexOf('{')),
    ) as Map<String, dynamic>;
    expect(completedFields['operation_id'], startsWith('op-'));
    expect(completedFields['duration_ms'], isA<int>());
    expect(completedFields['schema_version'], 1);
  });
}
