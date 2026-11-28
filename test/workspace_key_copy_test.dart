import 'dart:async';
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/data/workspace_data_loader.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';
import 'package:nauterm/workspace/workspace_state_store.dart';

void main() {
  _PresentedTestBinding();

  testWidgets('key actions localize file export and copy only the public key', (
    tester,
  ) async {
    final previousLanguage = appLanguage;
    addTearDown(() => setAppLanguage(previousLanguage));
    const publicKey = 'ssh-ed25519 AAAATEST public-fixture';
    final directory = Directory.systemTemp.createTempSync('nauterm-key-copy-');
    final controller = NautermWorkspaceController(
      workspaceStateStore: WorkspaceStateStore(
        NautermPaths(configDirectory: directory, dataDirectory: directory),
      ),
      dataLoader: (_, _) async => const NautermWorkspaceData(
        keys: [
          KeyEntry(
            id: 1,
            name: 'Clipboard fixture',
            privateKey: 'private-fixture',
            publicKey: publicKey,
          ),
        ],
      ),
    );
    addTearDown(() async {
      await tester.pumpWidget(const SizedBox.shrink());
      controller.dispose();
      directory.deleteSync(recursive: true);
    });
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    final copied = <String>[];
    messenger.setMockMethodCallHandler(SystemChannels.platform, (call) async {
      if (call.method == 'Clipboard.setData') {
        copied.add((call.arguments as Map<Object?, Object?>)['text'] as String);
      }
      return null;
    });
    addTearDown(
      () => messenger.setMockMethodCallHandler(SystemChannels.platform, null),
    );

    await tester.pumpWidget(
      NautermApp(onOpenSettings: () {}, workspaceController: controller),
    );
    await tester.pump(const Duration(milliseconds: 250));
    var ready = false;
    unawaited(controller.initialDataReady.then((_) => ready = true));
    for (var attempt = 0; attempt < 200 && !ready; attempt++) {
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 10)),
      );
      await tester.pump(const Duration(milliseconds: 20));
    }
    expect(ready, isTrue);
    await tester.runAsync(
      () => NautermLocalizations.load(const Locale('zh', 'CN')),
    );
    setAppLanguage(AppLanguage.simplifiedChinese);
    await tester.pumpAndSettle();
    await tester.tap(
      find.descendant(
        of: find.byKey(const ValueKey('workspace-sidebar')),
        matching: find.byIcon(LucideIcons.key),
      ),
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    final card = find.text('Clipboard fixture');
    expect(card, findsOneWidget);
    await tester.tap(
      card,
      buttons: kSecondaryMouseButton,
      kind: PointerDeviceKind.mouse,
    );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 150));
    expect(find.text('导出到文件'), findsOneWidget);
    await tester.tap(find.text('复制公钥'));
    await tester.pump(const Duration(milliseconds: 150));

    expect(copied, [publicKey]);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

class _PresentedTestBinding extends AutomatedTestWidgetsFlutterBinding {
  @override
  Future<void> get waitUntilFirstFrameRasterized => Future<void>.value();
}
