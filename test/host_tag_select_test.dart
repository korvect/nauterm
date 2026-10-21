import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_data_store.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  for (final query in ['', 'tag-query']) {
    testWidgets('tag clear removes all selections with query "$query"', (
      tester,
    ) async {
      const tags = [
        TagEntry(uuid: 'production', name: 'Production'),
        TagEntry(uuid: 'linux', name: 'Linux'),
      ];
      var selected = <String>{'production', 'linux'};
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: Center(
              child: SizedBox(
                width: 360,
                child: StatefulBuilder(
                  builder: (context, setState) => buildHostTagSelectForTesting(
                    tags: tags,
                    selectedUuids: selected,
                    onChanged: (value) => setState(() => selected = value),
                    onCreate: (_) => null,
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      final field = find.byType(TextField);
      final input = tester.widget<TextField>(field);
      expect(input.cursorHeight, isNull);
      final clear = find.byTooltip('Clear Tags');
      expect(find.byTooltip('Remove Production'), findsOneWidget);
      expect(find.byTooltip('Remove Linux'), findsOneWidget);

      await tester.tap(field);
      if (query.isNotEmpty) {
        await tester.enterText(field, query);
      }
      await tester.pump();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 150));
      expect(clear.hitTestable(), findsOneWidget);
      await tester.tap(clear);
      await tester.pump();
      await tester.pump();

      expect(selected, isEmpty);
      expect(tester.widget<TextField>(field).controller!.text, isEmpty);
      expect(tester.widget<TextField>(field).focusNode!.hasFocus, isTrue);
      expect(find.byTooltip('Remove Production'), findsNothing);
      expect(find.byTooltip('Remove Linux'), findsNothing);
      expect(clear.hitTestable(), findsNothing);

      // The tags remain available and can be selected again after clearing.
      await tester.tap(field);
      await tester.pump();
      await tester.pump();
      final menu = find.byKey(const ValueKey('workspace-select-menu:Tags'));
      expect(
        find.descendant(of: menu, matching: find.text('Linux')),
        findsOneWidget,
      );
      await tester.tap(
        find.descendant(of: menu, matching: find.text('Production')),
      );
      await tester.pump();
      expect(selected, {'production'});
      expect(find.byTooltip('Remove Production'), findsOneWidget);
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }
}
