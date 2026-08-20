import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/posthog_analytics.dart';

void main() {
  test('capture sends the stable device and application version', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));

    Map<String, dynamic>? payload;
    server.listen((request) async {
      payload = jsonDecode(
        await utf8.decoder.bind(request).join(),
      ) as Map<String, dynamic>;
      request.response.statusCode = HttpStatus.ok;
      await request.response.close();
    });

    PostHogAnalytics.init(
      apiKey: 'test-key',
      distinctId: 'stable-device-id',
      host: 'http://${server.address.host}:${server.port}',
      appName: 'Nauterm',
      appNamespace: 'com.korvect.nauterm',
      appVersion: '1.2.3',
      appBuild: '42',
    );

    final accepted = await PostHogAnalytics.capture('app_started', {
      r'$set_once': <String, Object>{
        'first_launch_at': '2026-08-01T00:00:00.000Z',
      },
    });

    expect(accepted, isTrue);
    expect(payload?['event'], 'app_started');
    expect(payload?['distinct_id'], 'stable-device-id');
    final properties = payload?['properties'] as Map<String, dynamic>;
    expect(properties[r'$app_version'], '1.2.3');
    expect(properties[r'$app_build'], '42');
    expect(properties[r'$geoip_disable'], isTrue);
    expect(
      (properties[r'$set_once'] as Map<String, dynamic>)['first_launch_at'],
      '2026-08-01T00:00:00.000Z',
    );
  });

  test('capture reports a rejected event without throwing', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    server.listen((request) async {
      await request.drain<void>();
      request.response.statusCode = HttpStatus.badRequest;
      await request.response.close();
    });

    PostHogAnalytics.init(
      apiKey: 'test-key',
      distinctId: 'stable-device-id',
      host: 'http://${server.address.host}:${server.port}',
    );

    expect(await PostHogAnalytics.capture('app_started'), isFalse);
  });

  test('screen sends PostHog standard screen event', () async {
    Map<String, dynamic>? payload;
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final requestFuture = server.first.then((request) async {
      payload = jsonDecode(
        await utf8.decoder.bind(request).join(),
      ) as Map<String, dynamic>;
      request.response.statusCode = HttpStatus.ok;
      await request.response.close();
    });

    PostHogAnalytics.init(
      apiKey: 'test-key',
      distinctId: 'device-1',
      host: 'http://${server.address.address}:${server.port}',
    );

    expect(await PostHogAnalytics.screen('Nauterm'), isTrue);
    await requestFuture;

    expect(payload?['event'], r'$screen');
    expect(
      (payload?['properties'] as Map<String, dynamic>?)?[r'$screen_name'],
      'Nauterm',
    );
  });
}
