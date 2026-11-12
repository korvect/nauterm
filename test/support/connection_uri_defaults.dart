import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/window/connection_uri_channel.dart';

void mockConnectionUriDefaults() {
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, (
    call,
  ) async {
    switch (call.method) {
      case 'getDefaultHandlers':
        return {
          for (final scheme in ConnectionUriChannel.schemes) scheme: false,
        };
      case 'takePendingUris':
        return <String>[];
      default:
        throw MissingPluginException();
    }
  });
  addTearDown(
    () =>
        messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, null),
  );
}
