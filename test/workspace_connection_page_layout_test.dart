import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

const _viewportKey = ValueKey('test-connection-viewport');
const _headerKey = ValueKey('connection-page-header');
const _progressKey = ValueKey('connection-page-progress');
const _footerKey = ValueKey('connection-page-footer');

void main() {
  setUp(() async {
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
  });

  for (final inDrawer in [false, true]) {
    for (final height in [280.0, 700.0]) {
      testWidgets(
        'public-key list scrolls below fixed tabs at $height px, drawer=$inDrawer',
        (tester) async {
          final controller = _controller(
            TerminalConnectionEventKind.authFailed,
          );
          addTearDown(controller.dispose);
          final keys = List.generate(
            20,
            (index) => TerminalConnectionKeyOption(
              id: index + 1,
              name: 'Test key ${index + 1}',
              privateKey: 'test-private-key-${index + 1}',
              subtitle: 'ED25519',
            ),
          );
          await _pumpPage(
            tester,
            buildTerminalConnectionPageForTesting(
              controller,
              inDrawer: inDrawer,
              keys: keys,
            ),
            height: height,
            width: inDrawer ? 360 : 720,
          );
          await tester.tap(find.text('Public Key'));
          await tester.pump(const Duration(milliseconds: 200));
          final body = find.byKey(const ValueKey('connection-auth-scrollable'));
          final scrollable = find.descendant(
            of: body,
            matching: find.byType(Scrollable),
          );
          expect(scrollable, findsOneWidget);
          final state = tester.state<ScrollableState>(scrollable);
          expect(state.position.maxScrollExtent, greaterThan(0));
          final header = tester.getRect(find.byKey(_headerKey));
          final progress = tester.getRect(find.byKey(_progressKey));
          final footer = tester.getRect(find.byKey(_footerKey));
          final tabs = tester.getRect(find.text('Public Key'));
          expect(
            find.ancestor(
              of: find.text('Public Key'),
              matching: find.byType(Scrollable),
            ),
            findsNothing,
          );
          await tester.drag(body, const Offset(0, -100));
          await tester.pump();
          expect(state.position.pixels, greaterThan(0));
          expect(tester.getRect(find.text('Public Key')), tabs);
          expect(tester.getRect(find.byKey(_headerKey)), header);
          expect(tester.getRect(find.byKey(_progressKey)), progress);
          expect(tester.getRect(find.byKey(_footerKey)), footer);
          await Scrollable.ensureVisible(
            tester.element(find.text('Test key 20')),
            alignment: 0.5,
          );
          await tester.pump();
          await tester.tap(find.text('Test key 20'));
          await tester.pump();
          final continueButton = find.ancestor(
            of: find.text('Continue'),
            matching: find.byType(InkWell),
          );
          expect(tester.widget<InkWell>(continueButton).onTap, isNotNull);
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox.shrink());
        },
      );

      testWidgets(
        'header and progress stay at the top during content transitions, height=$height, drawer=$inDrawer',
        (tester) async {
          final driver = _ConnectionDriver(
            TerminalConnectionEventKind.connectStart,
          );
          final controller = TerminalController.ssh(
            host: 'example.test',
            port: 22,
            username: 'tester',
            knownHostsPath: '/tmp/known-hosts',
            driver: driver,
          );
          addTearDown(controller.dispose);
          await _pumpPage(
            tester,
            buildTerminalConnectionPageForTesting(
              controller,
              inDrawer: inDrawer,
            ),
            height: height,
            width: inDrawer ? 360 : 720,
          );
          final header = tester.getRect(find.byKey(_headerKey));
          final progress = tester.getRect(find.byKey(_progressKey));
          final viewport = tester.getRect(find.byKey(_viewportKey));
          expect(
            header.top - viewport.top,
            height < 420 ? 16 : (inDrawer ? 72 : 80),
          );

          void expectFixedTop() {
            expect(tester.getRect(find.byKey(_headerKey)), header);
            expect(tester.getRect(find.byKey(_progressKey)), progress);
            _expectChromeInsideViewport(tester);
          }

          Future<void> checkTransition() async {
            await tester.pump();
            expectFixedTop();
            await tester.pump(const Duration(milliseconds: 70));
            expectFixedTop();
            await tester.pump(const Duration(milliseconds: 200));
            expectFixedTop();
          }

          await tester.tap(find.text('Show logs'));
          await checkTransition();
          await tester.tap(find.text('Hide logs'));
          await checkTransition();

          for (final kinds in [
            [TerminalConnectionEventKind.authPassphraseRequired],
            [
              TerminalConnectionEventKind.connectStart,
              TerminalConnectionEventKind.hostKeyUnknown,
            ],
            [
              TerminalConnectionEventKind.hostKeyAccepted,
              TerminalConnectionEventKind.authFailed,
            ],
            [
              TerminalConnectionEventKind.authSuccess,
              TerminalConnectionEventKind.connected,
            ],
            [
              TerminalConnectionEventKind.connectStart,
              TerminalConnectionEventKind.error,
            ],
          ]) {
            driver._events.addAll(
              kinds.map(
                (kind) => TerminalConnectionEvent(
                  kind: kind,
                  message: 'Test $kind',
                  timestamp: DateTime.utc(2026, 11, 3),
                  method:
                      kind == TerminalConnectionEventKind.authPassphraseRequired
                      ? 'public_key'
                      : null,
                  fingerprint:
                      kind == TerminalConnectionEventKind.hostKeyUnknown
                      ? 'SHA256:test-fingerprint'
                      : null,
                ),
              ),
            );
            controller.refreshSnapshot();
            await checkTransition();
          }
          await tester.pumpWidget(const SizedBox.shrink());
        },
      );
    }

    for (final height in [
      280.0,
      306.0,
      396.0,
      419.0,
      420.0,
      439.0,
      479.0,
      480.0,
      700.0,
    ]) {
      testWidgets(
        'failed connection fits $height px container, drawer=$inDrawer',
        (tester) async {
          final controller = _controller(TerminalConnectionEventKind.error);
          addTearDown(controller.dispose);
          await _pumpPage(
            tester,
            buildTerminalConnectionPageForTesting(
              controller,
              inDrawer: inDrawer,
            ),
            height: height,
          );

          _expectChromeInsideViewport(tester);
          final compact = inDrawer || height < 420;
          final header = tester.getRect(find.byKey(_headerKey));
          final progress = tester.getRect(find.byKey(_progressKey));
          final footer = tester.getRect(find.byKey(_footerKey));
          expect(progress.top - header.bottom, height < 420 ? 12 : 22);
          final closeButton = find.ancestor(
            of: find.text('Close'),
            matching: find.byType(InkWell),
          );
          expect(tester.getSize(closeButton).height, compact ? 24 : 36);
          expect(header.width, compact ? 328 : 268);
          final rows = find.byKey(const ValueKey('connection-logs-scrollable'));
          expect(tester.getSize(rows).height, greaterThanOrEqualTo(16));
          expect(find.text('Copy logs'), findsOneWidget);
          expect(
            find.byKey(const ValueKey('connection-logs-copy')),
            findsOneWidget,
          );

          final scrollState = tester.state<ScrollableState>(
            find.descendant(of: rows, matching: find.byType(Scrollable)),
          );
          expect(scrollState.position.maxScrollExtent, greaterThan(0));
          final originalHeader = header;
          final originalFooter = footer;
          await tester.drag(rows, const Offset(0, -100));
          await tester.pump();
          expect(scrollState.position.pixels, greaterThan(0));
          expect(tester.getRect(find.byKey(_headerKey)), originalHeader);
          expect(tester.getRect(find.byKey(_footerKey)), originalFooter);
          expect(tester.takeException(), isNull);
          await tester.pumpWidget(const SizedBox.shrink());
        },
      );
    }
  }

  testWidgets('short connecting page can show and hide diagnostics', (
    tester,
  ) async {
    final controller = _controller(TerminalConnectionEventKind.connectStart);
    addTearDown(controller.dispose);
    await _pumpPage(
      tester,
      buildTerminalConnectionPageForTesting(controller),
      height: 280,
    );
    _expectChromeInsideViewport(tester);
    expect(find.byKey(const ValueKey('connection-logs-card')), findsNothing);
    await tester.tap(find.text('Show logs'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    _expectChromeInsideViewport(tester);
    expect(find.byKey(const ValueKey('connection-logs-card')), findsOneWidget);
    expect(find.byKey(const ValueKey('connection-logs-copy')), findsOneWidget);
    await tester.tap(find.text('Hide logs'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    _expectChromeInsideViewport(tester);
    expect(find.byKey(const ValueKey('connection-logs-card')), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  for (final kind in [
    TerminalConnectionEventKind.hostKeyUnknown,
    TerminalConnectionEventKind.authFailed,
  ]) {
    testWidgets('$kind keeps only the intervention body scrollable', (
      tester,
    ) async {
      final controller = _controller(kind);
      addTearDown(controller.dispose);
      final page = buildTerminalConnectionPageForTesting(
        controller,
        keys: List.generate(
          5,
          (index) => TerminalConnectionKeyOption(
            id: index + 1,
            name: 'Test key ${index + 1}',
            privateKey: 'test-private-key-${index + 1}',
          ),
        ),
      );
      await _pumpPage(tester, page, height: 280);
      _expectChromeInsideViewport(tester);

      if (kind == TerminalConnectionEventKind.authFailed) {
        await tester.enterText(find.byType(TextField), 'test-password');
        await _pumpPage(tester, page, height: 700, width: 750);
        expect(
          tester.widget<TextField>(find.byType(TextField)).controller!.text,
          'test-password',
        );
        await _pumpPage(tester, page, height: 280);
        expect(
          tester.widget<TextField>(find.byType(TextField)).controller!.text,
          'test-password',
        );
        await tester.tap(find.text('Public Key'));
        await tester.pump(const Duration(milliseconds: 200));
      }

      final body = find.byKey(
        ValueKey(
          kind == TerminalConnectionEventKind.authFailed
              ? 'connection-auth-scrollable'
              : 'connection-mode-scrollable',
        ),
      );
      final scrollState = tester.state<ScrollableState>(
        find.descendant(of: body, matching: find.byType(Scrollable)).first,
      );
      expect(scrollState.position.maxScrollExtent, greaterThan(0));
      final footer = tester.getRect(find.byKey(_footerKey));
      final header = tester.getRect(find.byKey(_headerKey));
      await tester.drag(body, const Offset(0, -100));
      await tester.pump();
      expect(scrollState.position.pixels, greaterThan(0));
      expect(tester.getRect(find.byKey(_headerKey)), header);
      expect(tester.getRect(find.byKey(_footerKey)), footer);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }

  testWidgets(
    'short protocol page scrolls cards but not its heading or footer',
    (tester) async {
      await _pumpPage(
        tester,
        buildPendingHostConnectionPageForTesting(
          const HostEntry(
            name: 'Test server',
            host: 'example.test',
            type: NautermHostType.remote,
            sshEnabled: true,
            moshEnabled: true,
            telnetEnabled: true,
          ),
        ),
        height: 280,
      );
      _expectChromeInsideViewport(tester);
      final heading = tester.getRect(find.text('Protocol'));
      final footer = tester.getRect(find.byKey(_footerKey));
      final cards = find.byKey(
        const ValueKey('connection-protocol-scrollable'),
      );
      final scrollState = tester.state<ScrollableState>(
        find.descendant(of: cards, matching: find.byType(Scrollable)).first,
      );
      expect(scrollState.position.maxScrollExtent, greaterThan(0));
      await tester.drag(cards, const Offset(0, -100));
      await tester.pump();
      expect(scrollState.position.pixels, greaterThan(0));
      expect(tester.getRect(find.text('Protocol')), heading);
      expect(tester.getRect(find.byKey(_footerKey)), footer);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  for (final needsHostKey in [true, false]) {
    testWidgets('short SFTP connection page fits, hostKey=$needsHostKey', (
      tester,
    ) async {
      await _pumpPage(
        tester,
        buildSftpConnectionPageForTesting(needsHostKey: needsHostKey),
        height: 280,
      );
      _expectChromeInsideViewport(tester);
      final footer = tester.getRect(find.byKey(_footerKey));
      expect(footer.height, greaterThanOrEqualTo(24));
      if (needsHostKey) {
        final body = find.byKey(const ValueKey('connection-mode-scrollable'));
        final scrollState = tester.state<ScrollableState>(
          find.descendant(of: body, matching: find.byType(Scrollable)).first,
        );
        expect(scrollState.position.maxScrollExtent, greaterThan(0));
        final header = tester.getRect(find.byKey(_headerKey));
        await tester.drag(body, const Offset(0, -100));
        await tester.pump();
        expect(scrollState.position.pixels, greaterThan(0));
        expect(tester.getRect(find.byKey(_headerKey)), header);
        expect(tester.getRect(find.byKey(_footerKey)), footer);
      }
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }
}

Future<void> _pumpPage(
  WidgetTester tester,
  Widget page, {
  required double height,
  double width = 360,
}) async {
  // The page must use its own container, not this taller app viewport.
  tester.view.physicalSize = const Size(900, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: Center(
          child: SizedBox(
            key: _viewportKey,
            width: width,
            height: height,
            child: page,
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

void _expectChromeInsideViewport(WidgetTester tester) {
  expect(tester.takeException(), isNull);
  final viewport = tester.getRect(find.byKey(_viewportKey));
  for (final key in [_headerKey, _progressKey, _footerKey]) {
    final rect = tester.getRect(find.byKey(key));
    expect(rect.left, greaterThanOrEqualTo(viewport.left));
    expect(rect.right, lessThanOrEqualTo(viewport.right));
    expect(rect.top, greaterThanOrEqualTo(viewport.top));
    expect(rect.bottom, lessThanOrEqualTo(viewport.bottom));
    expect(
      find.ancestor(of: find.byKey(key), matching: find.byType(Scrollable)),
      findsNothing,
    );
  }
}

TerminalController _controller(TerminalConnectionEventKind kind) =>
    TerminalController.ssh(
      host: 'example.test',
      port: 22,
      username: 'tester',
      knownHostsPath: '/tmp/known-hosts',
      driver: _ConnectionDriver(kind),
    );

class _ConnectionDriver extends MemoryTerminalDriver {
  _ConnectionDriver(TerminalConnectionEventKind kind)
    : _events = List.generate(
        kind == TerminalConnectionEventKind.error ? 20 : 1,
        (index) => TerminalConnectionEvent(
          kind: kind == TerminalConnectionEventKind.error && index < 19
              ? TerminalConnectionEventKind.knownHostCheck
              : kind,
          message: 'Connection diagnostic ${index + 1}.',
          timestamp: DateTime.utc(2026, 10, 23, 0, 0, index),
          fingerprint: kind == TerminalConnectionEventKind.hostKeyUnknown
              ? 'SHA256:test-fingerprint'
              : null,
        ),
      ),
      super(columns: 80, rows: 24);

  final List<TerminalConnectionEvent> _events;

  @override
  List<TerminalConnectionEvent> drainConnectionEvents() {
    final events = _events.toList();
    _events.clear();
    return events;
  }
}
