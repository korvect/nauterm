import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
  testWidgets(
    'command A requires selection and keeps groups separate from hosts',
    (tester) async {
      var selected = <Object>{};
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 600,
              height: 400,
              child: buildWorkspaceSelectionSurfaceForTesting(
                onSelectionChanged: (value) => selected = value,
                child: Column(
                  children: [
                    buildWorkspaceGroupGridForTesting(),
                    const SizedBox(height: 20),
                    buildWorkspaceCardEditGridForTesting(
                      onEdit: (_) {},
                      onActivate: () {},
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      );
      Future<void> selectAll() async {
        await tester.sendKeyDownEvent(LogicalKeyboardKey.metaLeft);
        await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.metaLeft);
        await tester.pump();
      }

      await selectAll();
      expect(selected, isEmpty);
      await tester.tap(find.text('Host 1'));
      await tester.pump();
      await selectAll();
      expect(selected, {'host:1', 'host:2'});
      await tester.tap(find.text('Group 1'));
      await tester.pump();
      await selectAll();
      expect(selected, {'group:1', 'group:2'});
      await tester.tapAt(const Offset(300, 350));
      await tester.pump();
      await selectAll();
      expect(selected, isEmpty);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets(
    'ordinary selection follows the editor but context and multiselect do not',
    (tester) async {
      final edited = <String>[];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 600,
              height: 400,
              child: buildWorkspaceSelectionSurfaceForTesting(
                editingHostId: 1,
                onSelectionChanged: (_) {},
                onItemSelected: edited.add,
                child: buildWorkspaceCardEditGridForTesting(
                  onEdit: (_) {},
                  onActivate: () {},
                ),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Host 2'));
      await tester.pump();
      expect(edited, ['Host 2']);
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.tap(find.text('Host 1'));
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      expect(edited, ['Host 2']);
      await tester.tap(find.text('Host 1'), buttons: kSecondaryMouseButton);
      await tester.pumpAndSettle();
      expect(edited, ['Host 2']);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets(
    'editing card stays highlighted during a temporary context selection',
    (tester) async {
      var selected = <Object>{};
      var closed = false;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 600,
              height: 400,
              child: buildWorkspaceSelectionSurfaceForTesting(
                editingHostId: 1,
                onCloseEditor: () => closed = true,
                onSelectionChanged: (value) => selected = value,
                child: buildWorkspaceCardEditGridForTesting(
                  onEdit: (_) {},
                  onActivate: () {},
                ),
              ),
            ),
          ),
        ),
      );
      bool highlighted(int id) {
        final material = tester.widget<Material>(
          find
              .descendant(
                of: find.byKey(ValueKey('workspace-item-card:host:$id')),
                matching: find.byType(Material),
              )
              .first,
        );
        return (material.shape! as RoundedRectangleBorder).side !=
            BorderSide.none;
      }

      await tester.tap(find.text('Host 2'), buttons: kSecondaryMouseButton);
      await tester.pumpAndSettle();
      expect(selected, {'host:2'});
      expect(highlighted(1), isTrue);
      expect(highlighted(2), isTrue);
      await tester.tapAt(const Offset(700, 450));
      await tester.pumpAndSettle();
      expect(selected, {'host:1'});
      expect(highlighted(1), isTrue);
      expect(highlighted(2), isFalse);
      expect(closed, isFalse);
      await tester.tapAt(const Offset(300, 350));
      await tester.pumpAndSettle();
      expect(closed, isTrue);
      expect(selected, isEmpty);
      expect(highlighted(1), isFalse);
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets(
    'right click preserves selected targets and blank space clears them',
    (tester) async {
      var selected = <Object>{};
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 600,
              height: 400,
              child: buildWorkspaceSelectionSurfaceForTesting(
                onSelectionChanged: (value) => selected = value,
                child: buildWorkspaceCardEditGridForTesting(
                  onEdit: (_) {},
                  onActivate: () {},
                ),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Host 1'));
      await tester.pump();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.tap(find.text('Host 2'));
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      expect(selected, {'host:1', 'host:2'});
      await tester.tap(find.text('Host 1'), buttons: kSecondaryMouseButton);
      await tester.pumpAndSettle();
      expect(selected, {'host:1', 'host:2'});
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();
      await tester.tapAt(const Offset(300, 350));
      await tester.pumpAndSettle();
      // The menu barrier consumes the first outside click to dismiss itself.
      await tester.tapAt(const Offset(300, 350));
      await tester.pumpAndSettle();
      expect(selected, isEmpty);
      await tester.tap(find.text('Host 1'));
      await tester.pump();
      await tester.tap(find.text('Host 2'), buttons: kSecondaryMouseButton);
      await tester.pumpAndSettle();
      expect(selected, {'host:2'});
      await tester.pumpWidget(const SizedBox.shrink());
    },
  );

  testWidgets('hover edit selects the edited card without activating it', (
    tester,
  ) async {
    String? edited;
    var activations = 0;
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SizedBox(
            width: 600,
            child: buildWorkspaceCardEditGridForTesting(
              onEdit: (name) => edited = name,
              onActivate: () => activations++,
            ),
          ),
        ),
      ),
    );
    final first = find.byKey(const ValueKey('workspace-item-card:host:1'));
    final second = find.byKey(const ValueKey('workspace-item-card:host:2'));
    await tester.tap(find.text('Host 1'));
    await tester.pump();
    final mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(second));
    await tester.pumpAndSettle();
    final edit = find.descendant(of: second, matching: find.byType(Tooltip));
    await tester.tap(edit);
    await tester.pumpAndSettle();
    expect(edited, 'Host 2');
    expect(activations, 0);
    BorderSide border(Finder card) {
      final material = tester.widget<Material>(
        find.descendant(of: card, matching: find.byType(Material)).first,
      );
      return (material.shape! as RoundedRectangleBorder).side;
    }

    expect(border(first), BorderSide.none);
    expect(border(second).width, 1.5);
    await mouse.removePointer();
    await tester.pumpWidget(const SizedBox.shrink());
  });

  test('workspace grid never allocates more columns than items', () {
    expect(workspaceGridColumnCount(width: 680, itemCount: 2), 2);
    expect(workspaceGridColumnCount(width: 1200, itemCount: 1), 1);
  });

  test('workspace sections can share the same responsive column count', () {
    const sharedSectionItemCount = 3;

    final groupsColumns = workspaceGridColumnCount(
      width: 680,
      itemCount: 1,
      layoutItemCount: sharedSectionItemCount,
    );
    final hostsColumns = workspaceGridColumnCount(
      width: 680,
      itemCount: 3,
      layoutItemCount: sharedSectionItemCount,
    );

    expect(groupsColumns, 3);
    expect(hostsColumns, groupsColumns);
  });

  test('workspace grid still responds to available width and max columns', () {
    expect(workspaceGridColumnCount(width: 680, itemCount: 3), 3);
    expect(workspaceGridColumnCount(width: 437, itemCount: 3), 1);
    expect(
      workspaceGridColumnCount(width: 900, itemCount: 4, maxColumns: 2),
      2,
    );
  });

  test('empty workspace grid keeps a safe layout column', () {
    expect(workspaceGridColumnCount(width: 0, itemCount: 0), 1);
  });

  test('workspace cards toggle and extend a selection', () {
    const items = ['alpha', 'bravo', 'charlie', 'delta'];
    final first = workspaceItemSelectionForClick(
      selectedIdentities: {},
      visibleIdentities: items,
      tappedIdentity: 'alpha',
      anchorIdentity: null,
      toggleSelection: false,
      extendSelection: false,
    );
    final commandClick = workspaceItemSelectionForClick(
      selectedIdentities: first.selectedIdentities,
      visibleIdentities: items,
      tappedIdentity: 'charlie',
      anchorIdentity: first.anchorIdentity,
      toggleSelection: true,
      extendSelection: false,
    );
    final shiftClick = workspaceItemSelectionForClick(
      selectedIdentities: commandClick.selectedIdentities,
      visibleIdentities: items,
      tappedIdentity: 'delta',
      anchorIdentity: commandClick.anchorIdentity,
      toggleSelection: false,
      extendSelection: true,
    );

    expect(first.selectedIdentities, {'alpha'});
    expect(commandClick.selectedIdentities, {'alpha', 'charlie'});
    expect(shiftClick.selectedIdentities, {'charlie', 'delta'});
    expect(shiftClick.anchorIdentity, 'charlie');
  });

  test('command-shift adds the selected range without clearing cards', () {
    const items = ['alpha', 'bravo', 'charlie', 'delta'];
    final selection = workspaceItemSelectionForClick(
      selectedIdentities: {'alpha'},
      visibleIdentities: items,
      tappedIdentity: 'delta',
      anchorIdentity: 'bravo',
      toggleSelection: true,
      extendSelection: true,
    );

    expect(selection.selectedIdentities, {
      'alpha',
      'bravo',
      'charlie',
      'delta',
    });
    expect(selection.anchorIdentity, 'bravo');
  });

  test('shift plus direction creates and extends a selection range', () {
    const items = ['alpha', 'bravo', 'charlie', 'delta'];
    final firstMove = workspaceItemSelectionForDirectionalMove(
      selectedIdentities: {},
      visibleIdentities: items,
      currentIdentity: 'alpha',
      targetIdentity: 'bravo',
      anchorIdentity: null,
      toggleSelection: false,
      extendSelection: true,
    );
    final secondMove = workspaceItemSelectionForDirectionalMove(
      selectedIdentities: firstMove.selectedIdentities,
      visibleIdentities: items,
      currentIdentity: 'bravo',
      targetIdentity: 'charlie',
      anchorIdentity: firstMove.anchorIdentity,
      toggleSelection: false,
      extendSelection: true,
    );

    expect(firstMove.selectedIdentities, {'alpha', 'bravo'});
    expect(firstMove.anchorIdentity, 'alpha');
    expect(secondMove.selectedIdentities, {'alpha', 'bravo', 'charlie'});
    expect(secondMove.anchorIdentity, 'alpha');
  });

  test('context actions keep the selected targets after selection clears', () {
    final selectedItems = ['alpha', 'bravo'];
    List<String>? invokedItems;
    String? invokedAction;
    final invoke = workspaceContextActionSnapshot<String, String>(
      items: selectedItems,
      onSingle: (item, action) {
        invokedItems = [item];
        invokedAction = action;
      },
      onMultiple: (items, action) {
        invokedItems = items;
        invokedAction = action;
      },
    );

    selectedItems
      ..clear()
      ..add('charlie');
    invoke('delete');

    expect(invokedItems, ['alpha', 'bravo']);
    expect(invokedAction, 'delete');
    expect(() => invokedItems!.add('delta'), throwsUnsupportedError);
  });

  test('activating a selected host preserves the multi-selection', () {
    expect(
      workspaceItemShouldPreserveMultiSelectionForActivation(
        itemSelected: true,
        selectedItemCount: 3,
        hasDoubleTapAction: true,
        selectionModifierPressed: false,
      ),
      isTrue,
    );
    expect(
      workspaceItemShouldPreserveMultiSelectionForActivation(
        itemSelected: false,
        selectedItemCount: 3,
        hasDoubleTapAction: true,
        selectionModifierPressed: false,
      ),
      isFalse,
    );
  });
}
