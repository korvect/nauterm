import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:nauterm/ai/ai_message_scroll_controller.dart';
import 'package:nauterm/ai/ai_message_viewport.dart';

void main() {
  testWidgets('floating button animates during output and returns to bottom', (
    tester,
  ) async {
    final controller = AiMessageScrollController();
    addTearDown(controller.dispose);
    Widget view(bool streaming) => MaterialApp(
      home: Scaffold(
        body: SizedBox(
          width: 300,
          height: 300,
          child: AiMessageViewport(
            controller: controller,
            streaming: streaming,
            background: Colors.white,
            foreground: Colors.black,
            border: Colors.grey,
            scrollToBottomLabel: 'Scroll to bottom',
            child: SingleChildScrollView(
              controller: controller,
              child: const SizedBox(height: 1200),
            ),
          ),
        ),
      ),
    );
    final button = find.byKey(const ValueKey('ai-scroll-to-bottom'));
    controller.scrollToLatest();
    await tester.pumpWidget(view(false));
    await tester.pumpAndSettle();
    expect(button, findsNothing);
    await tester.sendEventToBinding(
      PointerScrollEvent(
        position: tester.getCenter(find.byType(SingleChildScrollView)),
        scrollDelta: const Offset(0, -250),
      ),
    );
    await tester.pumpAndSettle();
    expect(button, findsOneWidget);
    expect(find.byIcon(LucideIcons.arrowDown), findsOneWidget);
    await tester.pumpWidget(view(true));
    final dots = find.descendant(of: button, matching: find.byType(Transform));
    expect(dots, findsNWidgets(3));
    final before = tester
        .widget<Transform>(dots.first)
        .transform
        .getTranslation()
        .y;
    await tester.pump(const Duration(milliseconds: 200));
    expect(
      tester.widget<Transform>(dots.first).transform.getTranslation().y,
      isNot(before),
    );
    final offset = controller.offset;
    await tester.pumpWidget(view(false));
    await tester.pumpAndSettle();
    expect(controller.offset, offset);
    expect(find.byIcon(LucideIcons.arrowDown), findsOneWidget);
    await tester.tap(button);
    await tester.pumpAndSettle();
    expect(controller.position.extentAfter, closeTo(0, 0.1));
    expect(button, findsNothing);
    await tester.pumpWidget(const SizedBox());
  });
}
