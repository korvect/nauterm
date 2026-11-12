import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/window/connection_uri_channel.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  const codec = StandardMethodCodec();
  late List<String> nativeQueue;

  setUp(() {
    nativeQueue = [];
    messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, (
      call,
    ) async {
      expect(call.method, 'takePendingUris');
      final uris = [...nativeQueue];
      nativeQueue.clear();
      return uris;
    });
  });
  tearDown(() {
    messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, null);
  });

  Future<void> notify([String method = 'urisAvailable']) async {
    await messenger.handlePlatformMessage(
      ConnectionUriChannel.channel.name,
      codec.encodeMethodCall(MethodCall(method)),
      (_) {},
    );
    await Future<void>.delayed(Duration.zero);
  }

  test(
    'drains cold-start URLs and receives running-instance URLs once',
    () async {
      nativeQueue.add('ssh://first');
      final received = <String>[];
      final channel = ConnectionUriChannel(
        onOpen: (uri) async => received.add(uri),
        onActivate: () {},
      )..start();
      addTearDown(channel.dispose);
      await Future<void>.delayed(Duration.zero);
      expect(received, ['ssh://first']);
      nativeQueue.add('sftp://second/tmp');
      await notify();
      await notify();
      expect(received, ['ssh://first', 'sftp://second/tmp']);
    },
  );

  test('keeps later requests queued while the workspace is loading', () async {
    final ready = Completer<void>();
    nativeQueue.add('ssh://first');
    final received = <String>[];
    final channel = ConnectionUriChannel(
      onOpen: (uri) async {
        received.add(uri);
        await ready.future;
      },
      onActivate: () {},
    )..start();
    addTearDown(channel.dispose);
    await Future<void>.delayed(Duration.zero);
    nativeQueue.add('telnet://second');
    await notify();
    expect(received, ['ssh://first']);
    expect(nativeQueue, ['telnet://second']);
    ready.complete();
    await Future<void>.delayed(Duration.zero);
    expect(received, ['ssh://first', 'telnet://second']);
  });

  test('a failed request does not discard the next request', () async {
    nativeQueue.addAll(['ssh://first', 'ssh://second']);
    final received = <String>[];
    final channel = ConnectionUriChannel(
      onOpen: (uri) async {
        received.add(uri);
        if (uri.endsWith('first')) throw StateError('request failed');
      },
      onActivate: () {},
    )..start();
    addTearDown(channel.dispose);
    await Future<void>.delayed(Duration.zero);
    expect(received, ['ssh://first', 'ssh://second']);
  });

  test(
    'activation reopens the window and disposal stops pending requests',
    () async {
      var activated = 0;
      final ready = Completer<void>();
      final received = <String>[];
      nativeQueue.addAll(['ssh://first', 'ssh://second']);
      final channel = ConnectionUriChannel(
        onOpen: (uri) async {
          received.add(uri);
          await ready.future;
        },
        onActivate: () => activated++,
      )..start();
      await notify('activate');
      expect(activated, 1);
      channel.dispose();
      ready.complete();
      await Future<void>.delayed(Duration.zero);
      expect(received, ['ssh://first']);
    },
  );
}
