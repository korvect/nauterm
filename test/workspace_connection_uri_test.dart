import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/data/workspace_data_loader.dart';
import 'package:nauterm/workspace/workspace_state_store.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  _PresentedTestBinding();

  for (final scheme in ['ssh', 'sftp', 'telnet']) {
    testWidgets(
      '$scheme URI opens a temporary connection without protocol selection',
      (tester) async {
        final controller = await _pumpWorkspace(tester);
        final opened = controller.openConnectionUri(
          '$scheme://Admin@host.invalid:2022',
        );
        await _pumpConnection(tester);
        await opened;
        final terminal = _remoteController(tester);
        expect(find.text('Protocol'), findsNothing);
        if (scheme == 'telnet') {
          final profile = terminal.telnetProfile!;
          expect(profile.host, 'host.invalid');
          expect(profile.username, 'Admin');
          expect(profile.port, 2022);
          expect(profile.hostId, isNull);
        } else {
          final profile = terminal.sshProfile!;
          expect(profile.host, 'host.invalid');
          expect(profile.username, 'Admin');
          expect(profile.port, 2022);
          expect(profile.hostId, isNull);
          expect(profile.password, isNull);
        }
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  testWidgets(
    'a cold-start URI waits for workspace data and requests a missing username',
    (tester) async {
      final data = Completer<NautermWorkspaceData>();
      final controller = await _pumpWorkspace(tester, data: data);
      var complete = false;
      final opened = controller
          .openConnectionUri('sftp://host.invalid/home/user')
          .then((_) => complete = true);
      await tester.pump();
      expect(complete, isFalse);
      expect(find.byType(CircularProgressIndicator), findsNWidgets(2));
      data.complete(const NautermWorkspaceData());
      await _pumpConnection(tester);
      await opened;
      expect(find.text('Please enter username'), findsOneWidget);
      expect(find.text('Continue & Save'), findsNothing);
      expect(_remoteController(tester).sshProfile!.username, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets('URI passwords are discarded before opening a connection', (
    tester,
  ) async {
    final controller = await _pumpWorkspace(tester);
    final opened = controller.openConnectionUri(
      'ssh://user:never-display-me@host.invalid:2022',
    );
    await _pumpConnection(tester);
    await opened;
    final profile = _remoteController(tester).sshProfile!;
    expect(profile.username, 'user');
    expect(profile.host, 'host.invalid');
    expect(profile.port, 2022);
    expect(profile.password, isNull);
    expect(find.textContaining('never-display-me'), findsNothing);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('invalid links show an error without exposing query parameters', (
    tester,
  ) async {
    final controller = await _pumpWorkspace(tester);
    final opened = controller.openConnectionUri(
      'ssh://user@host.invalid?password=never-display-me',
    );
    await tester.pump();
    await opened;
    await tester.pump(const Duration(milliseconds: 200));
    expect(controller.hasTerminalTabs, isFalse);
    expect(
      find.textContaining('Unable to open this connection link.'),
      findsOneWidget,
    );
    expect(find.textContaining('never-display-me'), findsNothing);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
    'a link can reopen a remounted workspace with the same controller',
    (tester) async {
      final controller = await _pumpWorkspace(tester);
      await tester.pumpWidget(const SizedBox.shrink());
      final opened = controller.openConnectionUri('ssh://host.invalid');
      await tester.pumpWidget(
        NautermApp(onOpenSettings: () {}, workspaceController: controller),
      );
      await _pumpConnection(tester);
      await opened;
      expect(find.text('Please enter username'), findsOneWidget);
      expect(_remoteController(tester).sshProfile!.host, 'host.invalid');
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );
}

Future<NautermWorkspaceController> _pumpWorkspace(
  WidgetTester tester, {
  Completer<NautermWorkspaceData>? data,
}) async {
  final directory = Directory.systemTemp.createTempSync(
    'nauterm-uri-workspace-',
  );
  addTearDown(() => directory.deleteSync(recursive: true));
  final paths = NautermPaths(
    configDirectory: directory,
    dataDirectory: directory,
  );
  final controller = NautermWorkspaceController(
    workspaceStateStore: WorkspaceStateStore(paths),
    dataLoader: (_, _) async =>
        data == null ? const NautermWorkspaceData() : await data.future,
  );
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox.shrink());
    controller.dispose();
  });
  tester.view.physicalSize = const Size(1200, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    NautermApp(onOpenSettings: () {}, workspaceController: controller),
  );
  await _pumpConnection(tester);
  if (data == null) {
    var ready = false;
    unawaited(controller.initialDataReady.then((_) => ready = true));
    await _pumpConnection(tester);
    expect(
      ready,
      isTrue,
      reason: 'The workspace should finish its initial load.',
    );
  }
  return controller;
}

Future<void> _pumpConnection(WidgetTester tester) async {
  for (var attempt = 0; attempt < 50; attempt++) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 10)),
    );
    await tester.pump(const Duration(milliseconds: 100));
  }
}

TerminalController _remoteController(WidgetTester tester) => tester
    .widgetList<AnimatedBuilder>(find.byType(AnimatedBuilder))
    .map((builder) => builder.animation)
    .whereType<TerminalController>()
    .where(
      (controller) =>
          controller.sshProfile != null || controller.telnetProfile != null,
    )
    .toSet()
    .single;

class _PresentedTestBinding extends AutomatedTestWidgetsFlutterBinding {
  @override
  Future<void> get waitUntilFirstFrameRasterized => Future<void>.value();
}
