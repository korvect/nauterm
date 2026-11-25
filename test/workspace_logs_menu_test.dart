import 'dart:async';
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:material_ui/material_ui.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/data/workspace_data_loader.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';
import 'package:nauterm/workspace/workspace_state_store.dart';

void main() {
  _PresentedTestBinding();

  testWidgets(
    'cross-day logs keep their offset visible and expose row actions',
    (tester) async {
      final previousLanguage = appLanguage;
      addTearDown(() => setAppLanguage(previousLanguage));
      final directory = Directory.systemTemp.createTempSync(
        'nauterm-logs-menu-',
      );
      final log = TerminalLogEntry(
        id: 'cross-day-log',
        title: 'Long running session',
        cwd: '/work',
        captureFile: 'capture-fixture',
        captureSha256: 'hash-fixture',
        startedAt: DateTime(2026, 1, 1, 23, 55),
        endedAt: DateTime(2026, 1, 121, 2, 3),
      );
      final controller = NautermWorkspaceController(
        workspaceStateStore: WorkspaceStateStore(
          NautermPaths(configDirectory: directory, dataDirectory: directory),
        ),
        dataLoader: (_, _) async => NautermWorkspaceData(terminalLogs: [log]),
      );
      addTearDown(() async {
        await tester.pumpWidget(const SizedBox.shrink());
        controller.dispose();
        directory.deleteSync(recursive: true);
      });

      await tester.pumpWidget(
        NautermApp(onOpenSettings: () {}, workspaceController: controller),
      );
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
        () => NautermLocalizations.load(const Locale('en')),
      );
      setAppLanguage(AppLanguage.english);
      await tester.pumpAndSettle();
      await tester.tap(
        find.descendant(
          of: find.byKey(const ValueKey('workspace-sidebar')),
          matching: find.byIcon(LucideIcons.clock),
        ),
      );
      await tester.pumpAndSettle();

      final range = find.text('23:55 - 02:03');
      final offset = find.text('(+120d)');
      expect(range, findsOneWidget);
      expect(offset, findsOneWidget);
      expect(
        tester.getTopLeft(offset).dy,
        greaterThan(tester.getTopLeft(range).dy),
      );

      await tester.tap(
        find.text(log.title),
        buttons: kSecondaryMouseButton,
        kind: PointerDeviceKind.mouse,
      );
      await tester.pumpAndSettle();
      expect(find.text('Replay'), findsOneWidget);
      expect(find.text('Export terminal capture'), findsOneWidget);
      expect(find.text('Delete log and capture'), findsOneWidget);
      await tester.tap(find.text('Session details'));
      await tester.pumpAndSettle();
      expect(find.text('Session details'), findsOneWidget);
      expect(find.text('Directory: /work'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );
}

class _PresentedTestBinding extends AutomatedTestWidgetsFlutterBinding {
  @override
  Future<void> get waitUntilFirstFrameRasterized => Future<void>.value();
}
