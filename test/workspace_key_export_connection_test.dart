import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

class ExportDriver extends MemoryTerminalDriver {
  ExportDriver() : super(columns: 80, rows: 24);
  final events = <TerminalConnectionEvent>[];
  bool disposed = false;
  @override
  List<TerminalConnectionEvent> drainConnectionEvents() {
    final result = events.toList();
    events.clear();
    return result;
  }

  @override
  void dispose() {
    disposed = true;
    super.dispose();
  }
}

void main() {
  testWidgets('host field handles a width smaller than its trailing icon', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: buildKeyExportDrawerForTesting(
            createConnection: () => throw StateError('Must not connect'),
            onExport: (_) async {},
          ),
        ),
      ),
    );
    final field = tester.widget(
      find.byKey(const ValueKey('key-export-host-selector')),
    );
    for (final width in [32.9, 40.0, 360.0]) {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: Center(
              child: SizedBox(width: width, child: field),
            ),
          ),
        ),
      );
      expect(tester.takeException(), isNull);
      expect(
        find.byIcon(Icons.chevron_right_rounded),
        width < 40 ? findsNothing : findsOneWidget,
      );
    }
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('host picker searches and returns to the preserved export form', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 360,
            child: buildKeyExportDrawerForTesting(
              createConnection: () =>
                  throw StateError('Must not connect while selecting'),
              onExport: (_) async {},
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.byKey(const ValueKey('key-export-host-selector')));
    await tester.pump();
    expect(find.text('Select Host'), findsOneWidget);
    expect(find.text('Local'), findsNothing);
    await tester.enterText(find.byType(TextField), 'not-a-host');
    await tester.pump();
    expect(find.text('Test server'), findsNothing);
    await tester.enterText(find.byType(TextField), 'Test');
    await tester.pump();
    await tester.tap(find.text('Test server'));
    await tester.pump();
    expect(find.text('Export to host'), findsOneWidget);
    expect(find.text('authorized_keys'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('key-export-host-selector')));
    await tester.pump();
    await tester.tap(find.byIcon(Icons.arrow_back_rounded));
    await tester.pump();
    expect(find.text('Export to host'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('drawer key authentication buttons fit a narrow drawer', (
    tester,
  ) async {
    final driver = ExportDriver();
    final connection = TerminalController.ssh(
      host: 'example.test',
      port: 22,
      username: 'tester',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: Center(
            child: SizedBox(
              width: 360,
              child: buildKeyExportDrawerForTesting(
                createConnection: () => connection,
                onExport: (_) async {},
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Export to host'));
    await tester.pump();
    driver.events.add(
      TerminalConnectionEvent(
        kind: TerminalConnectionEventKind.authFailed,
        message: 'Authentication failed',
        timestamp: DateTime.now(),
      ),
    );
    connection.refreshSnapshot();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    await tester.tap(find.text('Public Key'));
    await tester.pump();
    expect(find.text('Add key'), findsOneWidget);
    final closeButton = find.ancestor(
      of: find.text('Close'),
      matching: find.byType(InkWell),
    );
    expect(closeButton, findsOneWidget);
    expect(tester.getSize(closeButton).height, 24);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  for (final fails in [false, true]) {
    testWidgets(
      'export waits for drawer connection and executes once, failure=$fails',
      (tester) async {
        final driver = ExportDriver();
        final connection = TerminalController.ssh(
          host: 'example.test',
          port: 22,
          username: 'tester',
          knownHostsPath: '/tmp/known-hosts',
          driver: driver,
        );
        var count = 0;
        final completion = Completer<void>();
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: Center(
                child: SizedBox(
                  width: 460,
                  child: buildKeyExportDrawerForTesting(
                    createConnection: () => connection,
                    onExport: (actual) {
                      expectSync(identical(actual, connection), isTrue);
                      count++;
                      return completion.future;
                    },
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.tap(find.text('Export to host'));
        await tester.pump();
        expect(find.byIcon(Icons.keyboard_tab_rounded), findsOneWidget);
        expect(
          find.byKey(const ValueKey('connection-page-header')),
          findsOneWidget,
        );
        expect(count, 0);
        expect(
          tester
              .getSize(find.byKey(const ValueKey('connection-page-header')))
              .width,
          460 - 32,
        );
        driver.events.add(
          TerminalConnectionEvent(
            kind: TerminalConnectionEventKind.connected,
            message: 'Connected',
            timestamp: DateTime.now(),
          ),
        );
        connection.refreshSnapshot();
        await tester.pump();
        expect(count, 0);
        await tester.pump(const Duration(milliseconds: 332));
        expect(count, 0);
        expect(
          find.byKey(const ValueKey('connection-page-header')),
          findsOneWidget,
        );
        await tester.pump(const Duration(milliseconds: 1));
        expect(count, 1);
        expect(find.text('Exporting key...'), findsOneWidget);
        connection.refreshSnapshot();
        await tester.pump();
        expect(count, 1);
        expect(
          find.byKey(const ValueKey('key-export-result-page')),
          findsOneWidget,
        );
        expect(find.byIcon(Icons.keyboard_tab_rounded), findsOneWidget);
        if (fails) {
          completion.completeError(StateError('Permission denied'));
        } else {
          completion.complete();
        }
        await tester.pump();
        expect(
          find.textContaining(
            fails ? 'Permission denied' : 'Key exported successfully',
          ),
          findsOneWidget,
        );
        expect(count, 1);
        await tester.pumpWidget(const SizedBox.shrink());
        expect(driver.disposed, isTrue);
      },
    );
  }

  testWidgets('leaving connection page cancels the drawer session', (
    tester,
  ) async {
    final driver = ExportDriver();
    final connection = TerminalController.ssh(
      host: 'example.test',
      port: 22,
      username: 'tester',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: buildKeyExportDrawerForTesting(
            createConnection: () => connection,
            onExport: (_) async =>
                fail('Must not export before authentication'),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Export to host'));
    await tester.pump();
    await tester.tap(find.text('Back'));
    await tester.pump();
    expect(driver.disposed, isTrue);
    expect(find.text('Export to host'), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}
