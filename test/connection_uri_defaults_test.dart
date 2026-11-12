import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/window/connection_uri_channel.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  tearDown(
    () =>
        messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, null),
  );

  test('queries each scheme while preserving an unknown result', () async {
    messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, (
      call,
    ) async {
      expect(call.method, 'getDefaultHandlers');
      return {'ssh': true, 'sftp': false, 'telnet': null};
    });
    expect(await ConnectionUriChannel.defaultHandlers(), {
      'ssh': true,
      'sftp': false,
      'telnet': null,
    });
  });

  test('a missing scheme result stays unknown', () async {
    messenger.setMockMethodCallHandler(
      ConnectionUriChannel.channel,
      (_) async => {'ssh': true},
    );
    expect(await ConnectionUriChannel.defaultHandlers(), {
      'ssh': true,
      'sftp': null,
      'telnet': null,
    });
  });

  test('sets only the requested supported scheme', () async {
    final requested = <String>[];
    messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, (
      call,
    ) async {
      expect(call.method, 'setDefaultHandler');
      requested.add(call.arguments as String);
      return null;
    });
    for (final scheme in ConnectionUriChannel.schemes) {
      await ConnectionUriChannel.setDefaultHandler(scheme);
    }
    expect(requested, ['ssh', 'sftp', 'telnet']);
    expect(
      () => ConnectionUriChannel.setDefaultHandler('https'),
      throwsArgumentError,
    );
    expect(requested, hasLength(3));
  });
}
