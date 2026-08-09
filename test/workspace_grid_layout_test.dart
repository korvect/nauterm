import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/workspace/nauterm_workspace.dart';

void main() {
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
