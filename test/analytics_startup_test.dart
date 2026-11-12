import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/main.dart' as application;

void main() {
  final binding = _StartupTestBinding();
  testWidgets('analytics waits for the base UI before startup work', (
    tester,
  ) async {
    binding.rasterizedFrame = Completer<void>();
    const packageInfoChannel = MethodChannel(
      'dev.fluttercommunity.plus/package_info',
    );
    var metadataReads = 0;
    binding.defaultBinaryMessenger.setMockMethodCallHandler(
      packageInfoChannel,
      (_) async {
        metadataReads++;
        // Exercise enabled analytics without opening a database or sending events.
        throw PlatformException(code: 'stop-before-database');
      },
    );
    addTearDown(() {
      binding.defaultBinaryMessenger.setMockMethodCallHandler(
        packageInfoChannel,
        null,
      );
    });
    var completed = false;
    final initialization = application.initAnalytics().then((_) {
      completed = true;
    });

    expect(completed, isFalse);
    await tester.pumpWidget(const ColoredBox(color: Color(0xff123456)));
    expect(find.byType(ColoredBox), findsOneWidget);
    expect(completed, isFalse);

    await tester.pump(const Duration(seconds: 1));
    expect(completed, isFalse);
    expect(metadataReads, 0);
    binding.rasterizedFrame.complete();
    await tester.pump();

    await tester.pump(const Duration(milliseconds: 119));
    expect(completed, isFalse);
    await tester.pump(const Duration(milliseconds: 1));
    await tester.pump();
    expect(completed, isTrue);
    const key = String.fromEnvironment('NAUTERM_POSTHOG_API_KEY');
    expect(metadataReads, key.isEmpty ? 0 : 1);
    await initialization;
  });
}

class _StartupTestBinding extends AutomatedTestWidgetsFlutterBinding {
  late Completer<void> rasterizedFrame;

  @override
  Future<void> get waitUntilFirstFrameRasterized => rasterizedFrame.future;
}
