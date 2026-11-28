import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:nauterm/settings/cloud_oauth.dart';

void main() {
  test(
    'cancelling sign-in releases the callback port for a fresh attempt',
    () async {
      var exchanges = 0;
      final transport = MockClient((request) async {
        exchanges++;
        expect(request.bodyFields['code'], 'fresh-code');
        return http.Response(
          '{"access_token":"access","refresh_token":"refresh"}',
          200,
        );
      });
      addTearDown(transport.close);
      final first = _Authorization(transport);
      addTearDown(first.client.close);
      await first.waiting.future;
      final oldUrl = first.opened.single;
      await first.client.reopenBrowser();
      expect(first.opened, [oldUrl, oldUrl]);

      first.client.cancel();
      expect(await first.result, isNull);
      expect(exchanges, 0);
      await first.client.reopenBrowser();
      expect(first.opened, hasLength(2));

      final next = _Authorization(
        transport,
        callbackPort: _redirect(oldUrl).port,
      );
      addTearDown(next.client.close);
      await next.waiting.future;
      final freshUrl = next.opened.single;
      expect(
        freshUrl.queryParameters['state'],
        isNot(oldUrl.queryParameters['state']),
      );

      final stale = await _callback(oldUrl, 'old-code');
      expect(stale.body, contains('Authorization failed'));
      expect(exchanges, 0);

      final response = await _callback(freshUrl, 'fresh-code');
      expect(response.body, contains('Authorization complete'));
      expect((await next.result)?.refreshToken, 'refresh');
      expect(exchanges, 1);
    },
  );

  test('cancelling during token exchange aborts the POST', () async {
    final transport = _PendingTokenExchange();
    addTearDown(transport.close);
    final authorization = _Authorization(transport);
    addTearDown(authorization.client.close);
    await authorization.waiting.future;
    await _callback(authorization.opened.single, 'exchange-code');
    final request = await transport.started.future;
    expect(request.bodyFields['code'], 'exchange-code');
    expect(authorization.phase, CloudOAuthPhase.exchanging);

    authorization.client.cancel();
    expect(await authorization.result, isNull);
    await transport.aborted.future;
  });
}

Uri _redirect(Uri authorizationUri) =>
    Uri.parse(authorizationUri.queryParameters['redirect_uri']!);

Future<http.Response> _callback(Uri authorizationUri, String code) => http.get(
  _redirect(authorizationUri).replace(
    queryParameters: {
      'state': authorizationUri.queryParameters['state'],
      'code': code,
    },
  ),
);

class _Authorization {
  _Authorization(http.Client transport, {int callbackPort = 0}) {
    client = CloudOAuthClient.forTesting(
      transport,
      clientId: 'fixture-client-id',
      openBrowser: (uri) async => opened.add(uri),
      callbackPort: callbackPort,
    );
    result = client.authorize(
      CloudOAuthVendor.googleDrive,
      onPhase: (value) {
        phase = value;
        if (value == CloudOAuthPhase.waitingForBrowser) waiting.complete();
      },
    );
  }

  late final CloudOAuthClient client;
  late final Future<CloudOAuthCredentials?> result;
  final List<Uri> opened = [];
  final Completer<void> waiting = Completer<void>();
  CloudOAuthPhase? phase;
}

class _PendingTokenExchange extends http.BaseClient {
  final Completer<http.AbortableRequest> started =
      Completer<http.AbortableRequest>();
  final Completer<void> aborted = Completer<void>();

  @override
  Future<http.StreamedResponse> send(http.BaseRequest request) async {
    final abortable = request as http.AbortableRequest;
    started.complete(abortable);
    await abortable.abortTrigger;
    aborted.complete();
    throw http.RequestAbortedException(request.url);
  }
}
