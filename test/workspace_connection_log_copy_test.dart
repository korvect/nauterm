import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/app/nauterm_localizations.dart';
import 'package:nauterm/terminal/terminal_controller.dart';
import 'package:nauterm/terminal/terminal_driver.dart';
import 'package:nauterm/terminal/terminal_models.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  setUp(() async {
    setAppLanguage(AppLanguage.english);
    NautermLocalizations.current = await NautermLocalizations.load(
      const Locale('en'),
    );
  });

  testWidgets('failed connection logs expose both copy actions', (
    WidgetTester tester,
  ) async {
    String? clipboardText;
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(SystemChannels.platform, (call) async {
      if (call.method == 'Clipboard.setData') {
        clipboardText =
            (call.arguments as Map<Object?, Object?>)['text'] as String?;
      }
      return null;
    });
    addTearDown(
      () => messenger.setMockMethodCallHandler(SystemChannels.platform, null),
    );

    final driver = _FailedConnectionDriver(eventCount: 2);
    final controller = TerminalController.ssh(
      host: 'example.test',
      port: 22,
      username: 'tester',
      knownHostsPath: '/tmp/known-hosts',
      driver: driver,
    );
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: buildTerminalConnectionPageForTesting(controller)),
      ),
    );
    await tester.pump();

    expect(find.text('Copy logs'), findsOneWidget);
    expect(find.byKey(const ValueKey('connection-logs-copy')), findsOneWidget);
    expect(
      tester.widget(find.byKey(const ValueKey('connection-logs-copy'))),
      isA<IconButton>(),
    );
    expect(find.byType(SelectionArea), findsOneWidget);
    final selectableMessage = find.descendant(
      of: find.byType(SelectionArea),
      matching: find.textContaining('Authentication failed.'),
    );
    expect(selectableMessage, findsOneWidget);
    expect(tester.widget(selectableMessage), isA<Text>());

    await tester.tap(find.byKey(const ValueKey('connection-logs-copy')));
    await tester.pump();
    expect(clipboardText, contains('Nauterm connection diagnostics'));
    expect(clipboardText, contains('Authentication failed.'));
    expect(find.text('Diagnostics copied.'), findsOneWidget);

    clipboardText = null;
    await tester.tap(find.text('Copy logs'));
    await tester.pump();
    expect(clipboardText, contains('Authentication failed.'));

    clipboardText = null;
    final selectionContext = tester.element(selectableMessage);
    Actions.invoke(
      selectionContext,
      const SelectAllTextIntent(SelectionChangedCause.keyboard),
    );
    Actions.invoke(selectionContext, CopySelectionTextIntent.copy);
    await tester.pump();
    expect(clipboardText, contains('  transport  Authentication failed.\n'));
    expect(RegExp(r'\d{2}:\d{2}:\d{2}[a-z]').hasMatch(clipboardText!), isFalse);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('connection page stays centered and scrolls only log rows', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(700, 700));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final controller = TerminalController.ssh(
      host: 'example.test',
      port: 22,
      username: 'tester',
      knownHostsPath: '/tmp/known-hosts',
      driver: _FailedConnectionDriver(eventCount: 12),
    );
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: buildTerminalConnectionPageForTesting(controller)),
      ),
    );
    await tester.pump();

    final header = find.byKey(const ValueKey('connection-page-header'));
    final progress = find.byKey(const ValueKey('connection-page-progress'));
    final scrollable = find.byKey(const ValueKey('connection-logs-scrollable'));
    final footer = find.byKey(const ValueKey('connection-page-footer'));
    final failureHeading = find.text('Connection failed with connection log:');

    final topMargin = tester.getTopLeft(header).dy;
    final bottomMargin = 700 - tester.getBottomRight(footer).dy;
    expect(topMargin, greaterThan(32));
    expect((topMargin - bottomMargin).abs(), lessThan(0.01));

    await tester.binding.setSurfaceSize(const Size(750, 440));
    await tester.pump();

    final headerBefore = tester.getTopLeft(header);
    final progressBefore = tester.getTopLeft(progress);
    final footerBefore = tester.getTopLeft(footer);
    final failureHeadingBefore = tester.getTopLeft(failureHeading);
    final scrollableState = tester.state<ScrollableState>(
      find.descendant(of: scrollable, matching: find.byType(Scrollable)),
    );
    expect(scrollableState.position.maxScrollExtent, greaterThan(0));

    await tester.drag(scrollable, const Offset(0, -80));
    await tester.pump();

    expect(tester.getTopLeft(header), headerBefore);
    expect(tester.getTopLeft(progress), progressBefore);
    expect(tester.getTopLeft(footer), footerBefore);
    expect(tester.getTopLeft(failureHeading), failureHeadingBefore);
    expect(scrollableState.position.pixels, greaterThan(0));

    await tester.pumpWidget(const SizedBox.shrink());
  });
}

class _FailedConnectionDriver extends MemoryTerminalDriver {
  _FailedConnectionDriver({int eventCount = 1})
    : _events = List.generate(
        eventCount,
        (index) => TerminalConnectionEvent(
          kind: TerminalConnectionEventKind.error,
          message: index == 0
              ? 'Authentication failed.'
              : 'Connection diagnostic ${index + 1}.',
          timestamp: DateTime.utc(2026, 9, 4, 0, 0, index),
        ),
      ),
      super(columns: 80, rows: 24);

  final List<TerminalConnectionEvent> _events;

  @override
  List<TerminalConnectionEvent> drainConnectionEvents() {
    final result = List<TerminalConnectionEvent>.of(_events);
    _events.clear();
    return result;
  }
}
