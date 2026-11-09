import 'dart:async';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/cli/ephemeral_connect_ipc.dart';
import 'package:nauterm/cli/ephemeral_connect_request.dart';
import 'package:nauterm/data/nauterm_paths.dart';

void main() {
  late Directory tempDir;
  late NautermPaths paths;

  setUp(() async {
    tempDir = await Directory.systemTemp.createTemp('nauterm-ipc-');
    paths = NautermPaths(
      configDirectory: tempDir,
      dataDirectory: tempDir,
    );
  });

  tearDown(() async {
    if (await tempDir.exists()) {
      await tempDir.delete(recursive: true);
    }
  });

  test('forwards connect request to the primary listener', () async {
    final received = Completer<EphemeralConnectRequest>();
    final server = EphemeralConnectIpcServer(
      paths: paths,
      onRequest: (request) {
        if (!received.isCompleted) {
          received.complete(request);
        }
      },
    );
    await server.start();
    addTearDown(server.close);

    final request = const EphemeralConnectRequest(
      protocol: EphemeralConnectProtocol.telnet,
      host: '192.168.1.1',
      port: 5001,
      title: 'R1',
    );
    final forwarded = await tryForwardEphemeralConnect(
      request,
      paths: paths,
    );

    expect(forwarded, isTrue);
    final accepted = await received.future.timeout(const Duration(seconds: 2));
    expect(accepted.host, '192.168.1.1');
    expect(accepted.port, 5001);
    expect(accepted.title, 'R1');
  });

  test('returns false when no primary listener is available', () async {
    final request = const EphemeralConnectRequest(
      protocol: EphemeralConnectProtocol.ssh,
      host: 'example.test',
      port: 22,
      username: 'root',
    );
    expect(await tryForwardEphemeralConnect(request, paths: paths), isFalse);
  });
}
