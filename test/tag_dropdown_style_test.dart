import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:nauterm/app/nauterm_app.dart';
import 'package:nauterm/terminal/terminal_config.dart';
import 'package:nauterm/ui/nauterm_context_menu.dart';

void main() {
  testWidgets('toolbar tag menu uses the shared Nauterm dropdown surface', (
    tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    expect(find.byTooltip('Filter by tags'), findsOneWidget);
    await tester.tap(find.byIcon(LucideIcons.tag));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));

    expect(find.text('Search tags'), findsOneWidget);
    expect(find.byType(NautermAnchoredDropdownOverlay), findsOneWidget);
    expect(find.byType(NautermDropdownSurface), findsOneWidget);
    expect(tester.getSize(find.byType(NautermDropdownSurface)).width, 210);
    final clearSelection = find.text('Clear selection');
    final clearSelectionRow = find.ancestor(
      of: clearSelection,
      matching: find.byType(NautermDropdownRow),
    );
    expect(clearSelectionRow, findsOneWidget);
    expect(
      tester.getSize(clearSelectionRow).height,
      nautermContextMenuRowHeight,
    );
    final clearSelectionText = tester.widget<Text>(clearSelection);
    expect(clearSelectionText.style?.fontSize, 12);
    expect(clearSelectionText.style?.fontWeight, FontWeight.w500);

    await tester.enterText(find.byType(TextField).last, 'brand-new-tag');
    await tester.pump();
    expect(find.textContaining('Create'), findsNothing);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('host tag select uses the shared Nauterm dropdown surface', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1440, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.text('New host'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 250));

    final decorator = find
        .ancestor(of: find.text('Tags'), matching: find.byType(InputDecorator))
        .first;
    final field = find
        .descendant(of: decorator, matching: find.byType(TextField))
        .first;
    await tester.tap(field);
    await tester.enterText(field, 'new-tag');
    await tester.pump();

    expect(find.byType(NautermDropdownSurface), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('SFTP host selector uses functional tag and sort dropdowns', (
    tester,
  ) async {
    final previousSftpTabEnabled = sftpTabEnabled;
    setSftpTabEnabled(true);
    addTearDown(() => setSftpTabEnabled(previousSftpTabEnabled));

    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.byIcon(Icons.folder_rounded).first);
    await tester.pump();
    await tester.tap(find.text('Select host').first);
    await tester.pump();

    expect(find.text('Select Host'), findsOneWidget);
    expect(find.text('Search hosts or tags'), findsOneWidget);
    expect(find.byTooltip('Filter by tags'), findsOneWidget);
    expect(find.byIcon(LucideIcons.tag), findsOneWidget);
    expect(find.byIcon(LucideIcons.arrowDownAZ), findsOneWidget);

    await tester.tap(find.byIcon(LucideIcons.arrowDownAZ));
    await tester.pump();
    expect(find.text('Name Z-a'), findsOneWidget);
    await tester.tap(find.text('Name Z-a'));
    await tester.pump();
    expect(find.byIcon(LucideIcons.arrowDownZA), findsOneWidget);

    await tester.tap(find.byIcon(LucideIcons.tag));
    await tester.pump();
    expect(find.text('Search tags'), findsOneWidget);
    expect(find.byType(NautermAnchoredDropdownOverlay), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('Quick Connect advertises host tag search', (tester) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));
    await tester.tap(find.byTooltip('Quick Connect'));
    await tester.pump();

    expect(
      find.text('Search host:, group:, tag:, or user@host'),
      findsOneWidget,
    );

    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('keychain and snippet menus use the shared dropdown overlay', (
    tester,
  ) async {
    await tester.pumpWidget(NautermApp(onOpenSettings: () {}));

    await tester.tap(find.text('Keychain'));
    await tester.pump();
    expect(find.byTooltip('Search'), findsOneWidget);
    await tester.tap(find.byTooltip('New key actions'));
    await tester.pump();
    expect(find.text('Generate key'), findsOneWidget);
    expect(find.text('New identity'), findsOneWidget);
    expect(find.byType(NautermAnchoredDropdownOverlay), findsOneWidget);

    await tester.tapAt(const Offset(8, 8));
    await tester.pump();
    await tester.tap(find.text('Snippets'));
    await tester.pump();
    await tester.tap(find.byTooltip('New snippet actions'));
    await tester.pump();
    expect(find.text('New snippet package'), findsOneWidget);
    expect(find.byType(NautermAnchoredDropdownOverlay), findsOneWidget);

    await tester.pumpWidget(const SizedBox.shrink());
  });
}
