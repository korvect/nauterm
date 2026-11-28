import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  testWidgets('slow exit work shows one dialog until the work finishes', (
    tester,
  ) async {
    final controller = NautermWorkspaceController();
    addTearDown(controller.dispose);
    await tester.pumpWidget(
      NautermApp(onOpenSettings: () {}, workspaceController: controller),
    );
    final work = Completer<void>();
    final closing = controller.withExitProgress(() => work.future);
    final prompt = find.text('Waiting for background work to finish…');
    await tester.pump(const Duration(milliseconds: 100));
    expect(prompt, findsNothing);
    await tester.pump(const Duration(milliseconds: 200));
    expect(prompt, findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(prompt, findsOneWidget);

    work.complete();
    await tester.pump();
    await closing;
    expect(prompt, findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  test('workspace shutdown is idempotent without an attached view', () async {
    final controller = NautermWorkspaceController();
    addTearDown(controller.dispose);

    final first = controller.flushAndClose();
    final second = controller.flushAndClose();

    expect(identical(first, second), isTrue);
    await first;
  });
}
