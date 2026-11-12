import 'dart:async';
import 'dart:ui' show ViewFocusDirection, ViewFocusEvent, ViewFocusState;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/settings/settings_panel.dart';
import 'package:nauterm/window/connection_uri_channel.dart';

void main() {
  final binding = TestWidgetsFlutterBinding.ensureInitialized();
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  late Map<String, bool?> handlers;
  late List<String> requested;
  late int queries;

  setUp(() {
    handlers = {'ssh': true, 'sftp': false, 'telnet': false};
    requested = [];
    queries = 0;
    messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, (
      call,
    ) async {
      if (call.method == 'getDefaultHandlers') {
        queries++;
        return {...handlers};
      }
      expect(call.method, 'setDefaultHandler');
      requested.add(call.arguments as String);
      handlers[call.arguments as String] = true;
      return null;
    });
  });
  tearDown(() {
    messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, null);
  });

  testWidgets('shows a green white check or a red white cross per scheme', (
    tester,
  ) async {
    await _pumpSettings(tester);
    _expectStatus(tester, 'ssh', true);
    _expectStatus(tester, 'sftp', false);
    _expectStatus(tester, 'telnet', false);
    expect(queries, 1);
    expect(requested, isEmpty);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('sets the clicked scheme and reads back the actual association', (
    tester,
  ) async {
    await _pumpSettings(tester);
    await tester.tap(_button('sftp'));
    await tester.pump();
    expect(requested, ['sftp']);
    expect(queries, 2);
    _expectStatus(tester, 'ssh', true);
    _expectStatus(tester, 'sftp', true);
    _expectStatus(tester, 'telnet', false);
    // A green button still only offers setting the default; it never clears it.
    await tester.tap(_button('sftp'));
    await tester.pump();
    expect(requested, ['sftp', 'sftp']);
    _expectStatus(tester, 'sftp', true);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('Windows stays red until the user changes the system setting', (
    tester,
  ) async {
    messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, (
      call,
    ) async {
      if (call.method == 'getDefaultHandlers') return {...handlers};
      requested.add(call.arguments as String);
      return null;
    });
    await _pumpSettings(tester);
    expect(
      tester.widget<IconButton>(_button('telnet')).tooltip,
      'Open default application settings for telnet.',
    );
    await tester.tap(_button('telnet'));
    await tester.pump();
    expect(requested, ['telnet']);
    _expectStatus(tester, 'telnet', false);
    handlers['telnet'] = true;
    binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pump();
    _expectStatus(tester, 'telnet', true);
    await tester.pumpWidget(const SizedBox.shrink());
  }, variant: TargetPlatformVariant.only(TargetPlatform.windows));

  testWidgets('a failed request preserves the current state and allows retry', (
    tester,
  ) async {
    messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, (
      call,
    ) async {
      if (call.method == 'getDefaultHandlers') return {...handlers};
      throw PlatformException(code: 'set_default_failed');
    });
    await _pumpSettings(tester);
    await tester.tap(_button('sftp'));
    await tester.pump();
    _expectStatus(tester, 'sftp', false);
    expect(
      find.text('Unable to set Nauterm as the default application.'),
      findsOneWidget,
    );
    expect(tester.widget<IconButton>(_button('sftp')).onPressed, isNotNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('an older query cannot overwrite a later system state', (
    tester,
  ) async {
    final first = Completer<Map<String, bool?>>();
    final second = Completer<Map<String, bool?>>();
    messenger.setMockMethodCallHandler(ConnectionUriChannel.channel, (_) async {
      queries++;
      return queries == 1 ? first.future : second.future;
    });
    await _pumpSettings(tester);
    binding.handleViewFocusChanged(
      ViewFocusEvent(
        viewId: tester.view.viewId,
        state: ViewFocusState.focused,
        direction: ViewFocusDirection.undefined,
      ),
    );
    await tester.pump();
    second.complete({'ssh': true, 'sftp': true, 'telnet': true});
    await tester.pump();
    first.complete({'ssh': false, 'sftp': false, 'telnet': false});
    await tester.pump();
    _expectStatus(tester, 'ssh', true);
    _expectStatus(tester, 'sftp', true);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

Future<void> _pumpSettings(WidgetTester tester) async {
  tester.view.physicalSize = const Size(1000, 1000);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    const MaterialApp(home: SettingsPanel(detectExternalEditors: false)),
  );
  await tester.pump();
  await tester.ensureVisible(_button('ssh'));
  await tester.pump();
}

Finder _button(String scheme) =>
    find.byKey(ValueKey('settings-default-handler:$scheme'));

void _expectStatus(WidgetTester tester, String scheme, bool isDefault) {
  final status = find.byKey(
    ValueKey('settings-default-handler-status:$scheme'),
  );
  final box = tester.widget<Container>(status).decoration! as BoxDecoration;
  expect(box.shape, BoxShape.circle);
  expect(
    box.color,
    isDefault ? const Color(0xff22c55e) : const Color(0xffef4444),
  );
  final icon = tester.widget<Icon>(
    find.descendant(of: status, matching: find.byType(Icon)),
  );
  expect(icon.icon, isDefault ? Icons.check_rounded : Icons.close_rounded);
  expect(icon.color, Colors.white);
}
