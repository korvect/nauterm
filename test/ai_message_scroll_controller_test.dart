import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/ai/ai_message_scroll_controller.dart';

void main() {
  testWidgets('streaming pauses after scrolling up and resumes at bottom', (
    tester,
  ) async {
    final controller = AiMessageScrollController();
    addTearDown(controller.dispose);
    Future<void> render(int rows) async {
      controller.scrollToLatest();
      await tester.pumpWidget(_messages(controller, rows));
      await tester.pumpAndSettle();
    }

    Future<void> wheel(double delta) async {
      await tester.sendEventToBinding(
        PointerScrollEvent(
          position: tester.getCenter(find.byType(SingleChildScrollView)),
          scrollDelta: Offset(0, delta),
        ),
      );
      await tester.pumpAndSettle();
    }

    await render(60);
    expect(controller.position.extentAfter, closeTo(0, 0.1));
    await render(65);
    expect(controller.position.extentAfter, closeTo(0, 0.1));

    // A queued follow must also respect input arriving before the next frame.
    controller.scrollToLatest();
    await wheel(-200);
    final readingOffset = controller.offset;
    await render(70);
    await render(75);
    expect(controller.offset, closeTo(readingOffset, 0.1));

    await wheel(50);
    final partialOffset = controller.offset;
    await render(80);
    expect(controller.offset, closeTo(partialOffset, 0.1));

    await wheel(10000);
    await render(85);
    expect(controller.position.extentAfter, closeTo(0, 0.1));
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('drag interrupts follow and explicit navigation resumes it', (
    tester,
  ) async {
    final controller = AiMessageScrollController();
    addTearDown(controller.dispose);
    controller.scrollToLatest();
    await tester.pumpWidget(_messages(controller, 60));
    await tester.pumpAndSettle();
    controller.scrollToLatest();
    await tester.pumpWidget(_messages(controller, 80));
    await tester.pump(const Duration(milliseconds: 30));
    final gesture = await tester.startGesture(
      tester.getCenter(find.byType(SingleChildScrollView)),
    );
    await gesture.moveBy(const Offset(0, 100));
    await tester.pump();
    final readingOffset = controller.offset;
    controller.scrollToLatest();
    await tester.pumpWidget(_messages(controller, 90));
    await tester.pump(const Duration(milliseconds: 200));
    expect(controller.offset, closeTo(readingOffset, 0.1));
    await gesture.up();
    await tester.pumpAndSettle();
    controller.scrollToLatest(resume: true);
    await tester.pump();
    await tester.pumpAndSettle();
    expect(controller.position.extentAfter, closeTo(0, 0.1));
    await tester.pumpWidget(const SizedBox());
  });
}

Widget _messages(AiMessageScrollController controller, int rows) => MaterialApp(
  home: Center(
    child: SizedBox(
      width: 300,
      height: 300,
      child: NotificationListener<ScrollNotification>(
        onNotification: controller.handleScrollNotification,
        child: SingleChildScrollView(
          controller: controller,
          child: Column(
            children: List.generate(
              rows,
              (index) => SizedBox(height: 25, child: Text('Message $index')),
            ),
          ),
        ),
      ),
    ),
  ),
);
