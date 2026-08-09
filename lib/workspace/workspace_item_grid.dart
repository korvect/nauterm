part of 'nauterm_workspace.dart';

typedef _WorkspaceContextAction<T extends _WorkspaceItemData> =
    void Function(T item, _ContextMenuActionId action);

typedef _WorkspaceContextActions<T extends _WorkspaceItemData> =
    void Function(List<T> items, _ContextMenuActionId action);

@visibleForTesting
ValueChanged<A> workspaceContextActionSnapshot<T, A>({
  required Iterable<T> items,
  void Function(T item, A action)? onSingle,
  void Function(List<T> items, A action)? onMultiple,
}) {
  final snapshot = List<T>.unmodifiable(items);
  return (action) {
    if (snapshot.length > 1 && onMultiple != null) {
      onMultiple(snapshot, action);
      return;
    }
    if (snapshot.length == 1 && onSingle != null) {
      onSingle(snapshot.single, action);
      return;
    }
    if (snapshot.isNotEmpty) {
      onMultiple?.call(snapshot, action);
    }
  };
}

const double _workspaceGridMinCardWidth = 210;
const double _workspaceGridMaxCardWidth = 320;
const double _workspaceGridColumnSpacing = 18;

@visibleForTesting
int workspaceGridColumnCount({
  required double width,
  required int itemCount,
  int maxColumns = 3,
  int? layoutItemCount,
}) {
  assert(maxColumns > 0);
  final effectiveItemCount = math.max(itemCount, layoutItemCount ?? 0);
  if (effectiveItemCount <= 0) return 1;
  final widthBoundColumns =
      ((width + _workspaceGridColumnSpacing) /
              (_workspaceGridMinCardWidth + _workspaceGridColumnSpacing))
          .floor()
          .clamp(1, maxColumns);
  return math.min(effectiveItemCount, widthBoundColumns);
}

bool _workspaceItemSupportsContextAction(
  _WorkspaceItemData item,
  _ContextMenuActionId action,
) {
  bool containsAction(Iterable<Object> rows) {
    for (final row in rows) {
      if (row is! _ContextMenuAction) {
        continue;
      }
      if (row.id == action || containsAction(row.submenuActions)) {
        return true;
      }
    }
    return false;
  }

  return containsAction(_contextMenuRowsFor(item));
}

_ContextMenuActionId? _workspaceItemCommandAction(KeyEvent event) {
  final shortcuts = terminalShortcutConfig;
  if (shortcutMatchesEvent(shortcuts.editWorkspaceItem, event)) {
    return _ContextMenuActionId.edit;
  }
  if (shortcutMatchesEvent(shortcuts.duplicateWorkspaceItem, event)) {
    return _ContextMenuActionId.duplicate;
  }
  return null;
}

Object _workspaceItemIdentity(_WorkspaceItemData item) => switch (item) {
  _GroupItem() => 'group:${item.id}',
  _HostItem() => 'host:${item.id}',
  _KeyItem() => 'key:${item.id}',
  _IdentityItem() => 'identity:${item.id}',
  _KnownHostItem() => 'known-host:${item.lineIndex}',
  _SnippetPackageItem() => 'snippet-package:${item.id}',
  _PortForwardItem() => 'port-forward:${item.id}',
  _ProxyItem() => 'proxy:${item.id}',
  _SnippetItem() => 'snippet:${item.id}',
  _ => '${item.runtimeType}:${item.name}',
};

Object _workspaceItemSelectionScopeId<T extends _WorkspaceItemData>(
  List<T> items,
) {
  assert(items.isNotEmpty);
  return items.first.runtimeType;
}

int? _retainedWorkspaceSelection<T extends _WorkspaceItemData>(
  List<T> oldItems,
  List<T> newItems,
  int? oldIndex,
) {
  if (oldIndex == null || oldIndex >= oldItems.length || newItems.isEmpty) {
    return null;
  }
  final identity = _workspaceItemIdentity(oldItems[oldIndex]);
  final nextIndex = newItems.indexWhere(
    (item) => _workspaceItemIdentity(item) == identity,
  );
  return nextIndex < 0 ? null : nextIndex;
}

bool get _workspaceItemToggleSelectionPressed {
  final keyboard = HardwareKeyboard.instance;
  return keyboard.isMetaPressed || keyboard.isControlPressed;
}

bool get _workspaceItemRangeSelectionPressed =>
    HardwareKeyboard.instance.isShiftPressed;

bool get _workspaceItemMultiSelectionPressed =>
    _workspaceItemToggleSelectionPressed || _workspaceItemRangeSelectionPressed;

@visibleForTesting
bool workspaceItemShouldPreserveMultiSelectionForActivation({
  required bool itemSelected,
  required int selectedItemCount,
  required bool hasDoubleTapAction,
  required bool selectionModifierPressed,
}) {
  return itemSelected &&
      selectedItemCount > 1 &&
      hasDoubleTapAction &&
      !selectionModifierPressed;
}

@visibleForTesting
({
  Set<Object> selectedIdentities,
  Object? activeIdentity,
  Object? anchorIdentity,
})
workspaceItemSelectionForClick({
  required Set<Object> selectedIdentities,
  required List<Object> visibleIdentities,
  required Object tappedIdentity,
  required Object? anchorIdentity,
  required bool toggleSelection,
  required bool extendSelection,
}) {
  if (extendSelection && anchorIdentity != null) {
    final anchorIndex = visibleIdentities.indexOf(anchorIdentity);
    final tappedIndex = visibleIdentities.indexOf(tappedIdentity);
    if (anchorIndex >= 0 && tappedIndex >= 0) {
      final first = math.min(anchorIndex, tappedIndex);
      final last = math.max(anchorIndex, tappedIndex);
      final range = visibleIdentities.sublist(first, last + 1).toSet();
      return (
        selectedIdentities: toggleSelection
            ? {...selectedIdentities, ...range}
            : range,
        activeIdentity: tappedIdentity,
        anchorIdentity: anchorIdentity,
      );
    }
  }

  if (toggleSelection) {
    final selection = {...selectedIdentities};
    if (!selection.add(tappedIdentity)) {
      selection.remove(tappedIdentity);
    }
    final activeIdentity = selection.contains(tappedIdentity)
        ? tappedIdentity
        : (selection.isEmpty ? null : selection.last);
    return (
      selectedIdentities: selection,
      activeIdentity: activeIdentity,
      anchorIdentity: activeIdentity,
    );
  }

  return (
    selectedIdentities: {tappedIdentity},
    activeIdentity: tappedIdentity,
    anchorIdentity: tappedIdentity,
  );
}

@visibleForTesting
({
  Set<Object> selectedIdentities,
  Object? activeIdentity,
  Object? anchorIdentity,
})
workspaceItemSelectionForDirectionalMove({
  required Set<Object> selectedIdentities,
  required List<Object> visibleIdentities,
  required Object currentIdentity,
  required Object targetIdentity,
  required Object? anchorIdentity,
  required bool toggleSelection,
  required bool extendSelection,
}) {
  if (!extendSelection || anchorIdentity != null) {
    return workspaceItemSelectionForClick(
      selectedIdentities: selectedIdentities,
      visibleIdentities: visibleIdentities,
      tappedIdentity: targetIdentity,
      anchorIdentity: anchorIdentity,
      toggleSelection: toggleSelection,
      extendSelection: extendSelection,
    );
  }

  final anchorSelection = workspaceItemSelectionForClick(
    selectedIdentities: selectedIdentities,
    visibleIdentities: visibleIdentities,
    tappedIdentity: currentIdentity,
    anchorIdentity: null,
    toggleSelection: false,
    extendSelection: false,
  );
  return workspaceItemSelectionForClick(
    selectedIdentities: anchorSelection.selectedIdentities,
    visibleIdentities: visibleIdentities,
    tappedIdentity: targetIdentity,
    anchorIdentity: anchorSelection.anchorIdentity,
    toggleSelection: toggleSelection,
    extendSelection: true,
  );
}

class _WorkspaceItemSelectionController extends ChangeNotifier {
  _WorkspaceItemSelectionController({this.onSelectionChanged});

  final ValueChanged<Set<Object>>? onSelectionChanged;
  Set<Object> _selectedIdentities = <Object>{};
  Object? _activeIdentity;
  Object? _anchorIdentity;
  Object? _selectionScopeId;

  Object? get selectedIdentity => _activeIdentity;

  Set<Object> get selectedIdentities => Set.unmodifiable(_selectedIdentities);

  bool isSelected({
    required Object identity,
    required Object selectionScopeId,
  }) =>
      _selectionScopeId == selectionScopeId &&
      _selectedIdentities.contains(identity);

  void select({required Object identity, required Object selectionScopeId}) {
    _setSelection(
      selectedIdentities: {identity},
      activeIdentity: identity,
      anchorIdentity: identity,
      selectionScopeId: selectionScopeId,
    );
  }

  void selectForClick({
    required List<Object> visibleIdentities,
    required Object tappedIdentity,
    required Object selectionScopeId,
    required bool toggleSelection,
    required bool extendSelection,
  }) {
    final sameScope = _selectionScopeId == selectionScopeId;
    final selection = workspaceItemSelectionForClick(
      selectedIdentities: sameScope ? _selectedIdentities : <Object>{},
      visibleIdentities: visibleIdentities,
      tappedIdentity: tappedIdentity,
      anchorIdentity: sameScope ? _anchorIdentity : null,
      toggleSelection: toggleSelection,
      extendSelection: extendSelection,
    );
    _setSelection(
      selectedIdentities: selection.selectedIdentities,
      activeIdentity: selection.activeIdentity,
      anchorIdentity: selection.anchorIdentity,
      selectionScopeId: selectionScopeId,
    );
  }

  void selectForDirectionalMove({
    required List<Object> visibleIdentities,
    required Object currentIdentity,
    required Object targetIdentity,
    required Object selectionScopeId,
    required bool toggleSelection,
    required bool extendSelection,
  }) {
    final sameScope = _selectionScopeId == selectionScopeId;
    final selection = workspaceItemSelectionForDirectionalMove(
      selectedIdentities: sameScope ? _selectedIdentities : <Object>{},
      visibleIdentities: visibleIdentities,
      currentIdentity: currentIdentity,
      targetIdentity: targetIdentity,
      anchorIdentity: sameScope ? _anchorIdentity : null,
      toggleSelection: toggleSelection,
      extendSelection: extendSelection,
    );
    _setSelection(
      selectedIdentities: selection.selectedIdentities,
      activeIdentity: selection.activeIdentity,
      anchorIdentity: selection.anchorIdentity,
      selectionScopeId: selectionScopeId,
    );
  }

  void _setSelection({
    required Set<Object> selectedIdentities,
    required Object? activeIdentity,
    required Object? anchorIdentity,
    required Object? selectionScopeId,
  }) {
    if (setEquals(_selectedIdentities, selectedIdentities) &&
        _activeIdentity == activeIdentity &&
        _anchorIdentity == anchorIdentity &&
        _selectionScopeId == selectionScopeId) {
      return;
    }
    _selectedIdentities = Set.unmodifiable(selectedIdentities);
    _activeIdentity = activeIdentity;
    _anchorIdentity = anchorIdentity;
    _selectionScopeId = selectionScopeId;
    notifyListeners();
    onSelectionChanged?.call(this.selectedIdentities);
  }

  void clear() {
    if (_selectedIdentities.isEmpty &&
        _activeIdentity == null &&
        _anchorIdentity == null &&
        _selectionScopeId == null) {
      return;
    }
    _setSelection(
      selectedIdentities: <Object>{},
      activeIdentity: null,
      anchorIdentity: null,
      selectionScopeId: null,
    );
  }
}

class _WorkspaceItemCollectionNavigationController {
  Object? _owner;
  VoidCallback? _focusFirst;
  VoidCallback? _focusLast;

  void _attach(
    Object owner, {
    required VoidCallback focusFirst,
    required VoidCallback focusLast,
  }) {
    _owner = owner;
    _focusFirst = focusFirst;
    _focusLast = focusLast;
  }

  void _detach(Object owner) {
    if (!identical(_owner, owner)) {
      return;
    }
    _owner = null;
    _focusFirst = null;
    _focusLast = null;
  }

  void focusFirst() => _focusFirst?.call();

  void focusLast() => _focusLast?.call();
}

class _WorkspaceItemSelectionScope extends StatelessWidget {
  const _WorkspaceItemSelectionScope({
    super.key,
    required this.controller,
    required this.child,
  });

  final _WorkspaceItemSelectionController controller;
  final Widget child;

  static _WorkspaceItemSelectionController? maybeOf(BuildContext context) {
    return context
        .dependOnInheritedWidgetOfExactType<_WorkspaceItemSelectionInherited>()
        ?.notifier;
  }

  @override
  Widget build(BuildContext context) {
    return _WorkspaceItemSelectionInherited(notifier: controller, child: child);
  }
}

class _WorkspaceItemSelectionInherited
    extends InheritedNotifier<_WorkspaceItemSelectionController> {
  const _WorkspaceItemSelectionInherited({
    required super.notifier,
    required super.child,
  });
}

class _WorkspaceItemTapTracker {
  DateTime? _lastTapAt;

  bool registerTap() {
    final now = DateTime.now();
    final lastTapAt = _lastTapAt;
    _lastTapAt = now;
    if (lastTapAt == null || now.difference(lastTapAt) > kDoubleTapTimeout) {
      return false;
    }
    _lastTapAt = null;
    return true;
  }

  void reset() {
    _lastTapAt = null;
  }
}

class _WorkspaceItemGrid<T extends _WorkspaceItemData> extends StatefulWidget {
  const _WorkspaceItemGrid({
    required this.items,
    this.maxColumns = 3,
    this.layoutItemCount,
    this.onItemTap,
    this.onItemDoubleTap,
    this.onContextAction,
    this.onContextActions,
    this.contextWorkspaceName,
    this.navigationController,
    this.onNavigateUp,
    this.onNavigateDown,
  });

  final List<T> items;
  final int maxColumns;
  final int? layoutItemCount;
  final ValueChanged<T>? onItemTap;
  final ValueChanged<T>? onItemDoubleTap;
  final _WorkspaceContextAction<T>? onContextAction;
  final _WorkspaceContextActions<T>? onContextActions;
  final String? contextWorkspaceName;
  final _WorkspaceItemCollectionNavigationController? navigationController;
  final VoidCallback? onNavigateUp;
  final VoidCallback? onNavigateDown;

  @override
  State<_WorkspaceItemGrid<T>> createState() => _WorkspaceItemGridState<T>();
}

class _WorkspaceItemGridState<T extends _WorkspaceItemData>
    extends State<_WorkspaceItemGrid<T>> {
  final FocusNode _focusNode = FocusNode();
  int? _selectedIndex;
  int _columns = 1;
  _WorkspaceItemSelectionController? _sharedSelection;

  int? get _effectiveSelectedIndex {
    final identity = _sharedSelection?.selectedIdentity;
    if (identity == null) {
      return _sharedSelection == null ? _selectedIndex : null;
    }
    final index = widget.items.indexWhere(
      (item) => _workspaceItemIdentity(item) == identity,
    );
    return index < 0 ? null : index;
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _sharedSelection = _WorkspaceItemSelectionScope.maybeOf(context);
    _restoreFocusForSelection();
  }

  @override
  void initState() {
    super.initState();
    _attachNavigationController();
  }

  @override
  void didUpdateWidget(covariant _WorkspaceItemGrid<T> oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(
      oldWidget.navigationController,
      widget.navigationController,
    )) {
      oldWidget.navigationController?._detach(this);
      _attachNavigationController();
    }
    if (_sharedSelection == null) {
      _selectedIndex = _retainedWorkspaceSelection(
        oldWidget.items,
        widget.items,
        _selectedIndex,
      );
    }
  }

  @override
  void dispose() {
    widget.navigationController?._detach(this);
    _focusNode.dispose();
    super.dispose();
  }

  void _attachNavigationController() {
    widget.navigationController?._attach(
      this,
      focusFirst: _focusFirst,
      focusLast: _focusLast,
    );
  }

  void _focusFirst() {
    if (widget.items.isEmpty) {
      return;
    }
    _select(0);
    _revealSelection();
  }

  void _focusLast() {
    if (widget.items.isEmpty) {
      return;
    }
    _select(widget.items.length - 1);
    _revealSelection();
  }

  void _revealSelection() {
    unawaited(
      Scrollable.ensureVisible(
        context,
        alignment: 0.25,
        duration: const Duration(milliseconds: 120),
      ),
    );
  }

  void _restoreFocusForSelection() {
    final selectedIdentity = _sharedSelection?.selectedIdentity;
    if (selectedIdentity == null ||
        !widget.items.any(
          (item) => _workspaceItemIdentity(item) == selectedIdentity,
        )) {
      return;
    }
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || _focusNode.hasFocus) {
        return;
      }
      // Restoring keyboard focus after returning to the workspace must not
      // reposition the outer pane. Focus-first/last navigation reveals its
      // target explicitly.
      _focusNode.requestFocus();
    });
  }

  bool _isSelected(int index) {
    final identity = _workspaceItemIdentity(widget.items[index]);
    return _sharedSelection?.isSelected(
          identity: identity,
          selectionScopeId: _workspaceItemSelectionScopeId(widget.items),
        ) ??
        _selectedIndex == index;
  }

  void _select(int index, {bool respectSelectionModifiers = false}) {
    final identity = _workspaceItemIdentity(widget.items[index]);
    if (_sharedSelection != null && respectSelectionModifiers) {
      _sharedSelection!.selectForClick(
        visibleIdentities: [
          for (final item in widget.items) _workspaceItemIdentity(item),
        ],
        tappedIdentity: identity,
        selectionScopeId: _workspaceItemSelectionScopeId(widget.items),
        toggleSelection: _workspaceItemToggleSelectionPressed,
        extendSelection: _workspaceItemRangeSelectionPressed,
      );
    } else {
      _sharedSelection?.select(
        identity: identity,
        selectionScopeId: _workspaceItemSelectionScopeId(widget.items),
      );
    }
    if (_sharedSelection == null && _selectedIndex != index) {
      setState(() => _selectedIndex = index);
    }
    _focusNode.requestFocus();
  }

  void _selectFromPointer(int index) {
    final preserveSelection =
        workspaceItemShouldPreserveMultiSelectionForActivation(
          itemSelected: _isSelected(index),
          selectedItemCount:
              _sharedSelection?.selectedIdentities.length ??
              (_selectedIndex == null ? 0 : 1),
          hasDoubleTapAction: widget.onItemDoubleTap != null,
          selectionModifierPressed: _workspaceItemMultiSelectionPressed,
        );
    if (preserveSelection) {
      _focusNode.requestFocus();
      return;
    }
    _select(index, respectSelectionModifiers: true);
  }

  List<T> _contextItemsFor(int index) {
    if (widget.onContextActions == null || !_isSelected(index)) {
      return [widget.items[index]];
    }
    return [
      for (var itemIndex = 0; itemIndex < widget.items.length; itemIndex++)
        if (_isSelected(itemIndex)) widget.items[itemIndex],
    ];
  }

  ValueChanged<_ContextMenuActionId> _contextActionFor(int index) {
    return workspaceContextActionSnapshot(
      items: _contextItemsFor(index),
      onSingle: widget.onContextAction,
      onMultiple: widget.onContextActions,
    );
  }

  void _selectForDirectionalMove(int current, int candidate) {
    final sharedSelection = _sharedSelection;
    if (sharedSelection == null) {
      _select(candidate);
      return;
    }
    sharedSelection.selectForDirectionalMove(
      visibleIdentities: [
        for (final item in widget.items) _workspaceItemIdentity(item),
      ],
      currentIdentity: _workspaceItemIdentity(widget.items[current]),
      targetIdentity: _workspaceItemIdentity(widget.items[candidate]),
      selectionScopeId: _workspaceItemSelectionScopeId(widget.items),
      toggleSelection: _workspaceItemToggleSelectionPressed,
      extendSelection: _workspaceItemRangeSelectionPressed,
    );
    _focusNode.requestFocus();
  }

  void _activateSelected() {
    final index = _effectiveSelectedIndex;
    if (index == null || index >= widget.items.length) {
      return;
    }
    (widget.onItemDoubleTap ?? widget.onItemTap)?.call(widget.items[index]);
  }

  bool _invokeSelectedContextAction(_ContextMenuActionId action) {
    final index = _effectiveSelectedIndex;
    final onContextAction = widget.onContextAction;
    if (index == null ||
        index >= widget.items.length ||
        onContextAction == null ||
        !_workspaceItemSupportsContextAction(widget.items[index], action)) {
      return false;
    }
    onContextAction(widget.items[index], action);
    return true;
  }

  KeyEventResult _handleKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent || widget.items.isEmpty) {
      return KeyEventResult.ignored;
    }
    final commandAction = _workspaceItemCommandAction(event);
    if (commandAction != null) {
      return _invokeSelectedContextAction(commandAction)
          ? KeyEventResult.handled
          : KeyEventResult.ignored;
    }
    if (event.logicalKey == LogicalKeyboardKey.enter) {
      _activateSelected();
      return KeyEventResult.handled;
    }
    final isArrow =
        event.logicalKey == LogicalKeyboardKey.arrowLeft ||
        event.logicalKey == LogicalKeyboardKey.arrowRight ||
        event.logicalKey == LogicalKeyboardKey.arrowUp ||
        event.logicalKey == LogicalKeyboardKey.arrowDown;
    if (!isArrow) {
      return KeyEventResult.ignored;
    }
    final current = _effectiveSelectedIndex ?? 0;
    final lastRowStart = ((widget.items.length - 1) ~/ _columns) * _columns;
    final candidate = switch (event.logicalKey) {
      LogicalKeyboardKey.arrowLeft when current > 0 => current - 1,
      LogicalKeyboardKey.arrowRight when current + 1 < widget.items.length =>
        current + 1,
      LogicalKeyboardKey.arrowUp when current >= _columns => current - _columns,
      LogicalKeyboardKey.arrowDown
          when current + _columns < widget.items.length =>
        current + _columns,
      LogicalKeyboardKey.arrowDown when current < lastRowStart =>
        widget.items.length - 1,
      _ => current,
    };
    if (candidate == current) {
      final onBoundaryNavigation = switch (event.logicalKey) {
        LogicalKeyboardKey.arrowLeft ||
        LogicalKeyboardKey.arrowUp => widget.onNavigateUp,
        LogicalKeyboardKey.arrowRight ||
        LogicalKeyboardKey.arrowDown => widget.onNavigateDown,
        _ => null,
      };
      if (onBoundaryNavigation != null) {
        onBoundaryNavigation();
      }
      return KeyEventResult.handled;
    }
    _selectForDirectionalMove(current, candidate);
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    return Focus(
      focusNode: _focusNode,
      autofocus: _effectiveSelectedIndex != null,
      onKeyEvent: _handleKeyEvent,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final width = constraints.maxWidth;
          final columns = workspaceGridColumnCount(
            width: width,
            itemCount: widget.items.length,
            maxColumns: widget.maxColumns,
            layoutItemCount: widget.layoutItemCount,
          );
          _columns = columns;
          final totalSpacing = _workspaceGridColumnSpacing * (columns - 1);
          final cardWidth = (width - totalSpacing) / columns;

          return Wrap(
            spacing: _workspaceGridColumnSpacing,
            runSpacing: 12,
            children: [
              for (var index = 0; index < widget.items.length; index++)
                _WorkspaceItemCard<T>(
                  key: ValueKey(
                    'workspace-item-card:${_workspaceItemIdentity(widget.items[index])}',
                  ),
                  width: cardWidth
                      .clamp(
                        _workspaceGridMinCardWidth,
                        _workspaceGridMaxCardWidth,
                      )
                      .toDouble(),
                  item: widget.items[index],
                  selected: _isSelected(index),
                  contextItems: _contextItemsFor(index),
                  onTap: () => _selectFromPointer(index),
                  onDoubleTap:
                      widget.onItemDoubleTap == null && widget.onItemTap == null
                      ? null
                      : () => (widget.onItemDoubleTap ?? widget.onItemTap)!(
                          widget.items[index],
                        ),
                  onContextAction:
                      widget.onContextAction == null &&
                          widget.onContextActions == null
                      ? null
                      : _contextActionFor(index),
                  contextWorkspaceName: widget.contextWorkspaceName,
                ),
            ],
          );
        },
      ),
    );
  }
}

class _WorkspaceItemList<T extends _WorkspaceItemData> extends StatefulWidget {
  const _WorkspaceItemList({
    required this.items,
    this.onItemTap,
    this.onItemDoubleTap,
    this.onContextAction,
    this.onContextActions,
    this.contextWorkspaceName,
    this.navigationController,
    this.onNavigateUp,
    this.onNavigateDown,
  });

  final List<T> items;
  final ValueChanged<T>? onItemTap;
  final ValueChanged<T>? onItemDoubleTap;
  final _WorkspaceContextAction<T>? onContextAction;
  final _WorkspaceContextActions<T>? onContextActions;
  final String? contextWorkspaceName;
  final _WorkspaceItemCollectionNavigationController? navigationController;
  final VoidCallback? onNavigateUp;
  final VoidCallback? onNavigateDown;

  @override
  State<_WorkspaceItemList<T>> createState() => _WorkspaceItemListState<T>();
}

class _WorkspaceItemListState<T extends _WorkspaceItemData>
    extends State<_WorkspaceItemList<T>> {
  final FocusNode _focusNode = FocusNode();
  int? _selectedIndex;
  _WorkspaceItemSelectionController? _sharedSelection;

  int? get _effectiveSelectedIndex {
    final identity = _sharedSelection?.selectedIdentity;
    if (identity == null) {
      return _sharedSelection == null ? _selectedIndex : null;
    }
    final index = widget.items.indexWhere(
      (item) => _workspaceItemIdentity(item) == identity,
    );
    return index < 0 ? null : index;
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _sharedSelection = _WorkspaceItemSelectionScope.maybeOf(context);
    _restoreFocusForSelection();
  }

  @override
  void initState() {
    super.initState();
    _attachNavigationController();
  }

  @override
  void didUpdateWidget(covariant _WorkspaceItemList<T> oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(
      oldWidget.navigationController,
      widget.navigationController,
    )) {
      oldWidget.navigationController?._detach(this);
      _attachNavigationController();
    }
    if (_sharedSelection == null) {
      _selectedIndex = _retainedWorkspaceSelection(
        oldWidget.items,
        widget.items,
        _selectedIndex,
      );
    }
  }

  @override
  void dispose() {
    widget.navigationController?._detach(this);
    _focusNode.dispose();
    super.dispose();
  }

  void _attachNavigationController() {
    widget.navigationController?._attach(
      this,
      focusFirst: _focusFirst,
      focusLast: _focusLast,
    );
  }

  void _focusFirst() {
    if (widget.items.isEmpty) {
      return;
    }
    _select(0);
    _revealSelection();
  }

  void _focusLast() {
    if (widget.items.isEmpty) {
      return;
    }
    _select(widget.items.length - 1);
    _revealSelection();
  }

  void _revealSelection() {
    unawaited(
      Scrollable.ensureVisible(
        context,
        alignment: 0.25,
        duration: const Duration(milliseconds: 120),
      ),
    );
  }

  void _restoreFocusForSelection() {
    final selectedIdentity = _sharedSelection?.selectedIdentity;
    if (selectedIdentity == null ||
        !widget.items.any(
          (item) => _workspaceItemIdentity(item) == selectedIdentity,
        )) {
      return;
    }
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || _focusNode.hasFocus) {
        return;
      }
      // Preserve the user's scroll offset when this collection is rebuilt.
      _focusNode.requestFocus();
    });
  }

  bool _isSelected(int index) {
    final identity = _workspaceItemIdentity(widget.items[index]);
    return _sharedSelection?.isSelected(
          identity: identity,
          selectionScopeId: _workspaceItemSelectionScopeId(widget.items),
        ) ??
        _selectedIndex == index;
  }

  void _select(int index, {bool respectSelectionModifiers = false}) {
    final identity = _workspaceItemIdentity(widget.items[index]);
    if (_sharedSelection != null && respectSelectionModifiers) {
      _sharedSelection!.selectForClick(
        visibleIdentities: [
          for (final item in widget.items) _workspaceItemIdentity(item),
        ],
        tappedIdentity: identity,
        selectionScopeId: _workspaceItemSelectionScopeId(widget.items),
        toggleSelection: _workspaceItemToggleSelectionPressed,
        extendSelection: _workspaceItemRangeSelectionPressed,
      );
    } else {
      _sharedSelection?.select(
        identity: identity,
        selectionScopeId: _workspaceItemSelectionScopeId(widget.items),
      );
    }
    if (_sharedSelection == null && _selectedIndex != index) {
      setState(() => _selectedIndex = index);
    }
    _focusNode.requestFocus();
  }

  void _selectFromPointer(int index) {
    final preserveSelection =
        workspaceItemShouldPreserveMultiSelectionForActivation(
          itemSelected: _isSelected(index),
          selectedItemCount:
              _sharedSelection?.selectedIdentities.length ??
              (_selectedIndex == null ? 0 : 1),
          hasDoubleTapAction: widget.onItemDoubleTap != null,
          selectionModifierPressed: _workspaceItemMultiSelectionPressed,
        );
    if (preserveSelection) {
      _focusNode.requestFocus();
      return;
    }
    _select(index, respectSelectionModifiers: true);
  }

  List<T> _contextItemsFor(int index) {
    if (widget.onContextActions == null || !_isSelected(index)) {
      return [widget.items[index]];
    }
    return [
      for (var itemIndex = 0; itemIndex < widget.items.length; itemIndex++)
        if (_isSelected(itemIndex)) widget.items[itemIndex],
    ];
  }

  ValueChanged<_ContextMenuActionId> _contextActionFor(int index) {
    return workspaceContextActionSnapshot(
      items: _contextItemsFor(index),
      onSingle: widget.onContextAction,
      onMultiple: widget.onContextActions,
    );
  }

  void _selectForDirectionalMove(int current, int candidate) {
    final sharedSelection = _sharedSelection;
    if (sharedSelection == null) {
      _select(candidate);
      return;
    }
    sharedSelection.selectForDirectionalMove(
      visibleIdentities: [
        for (final item in widget.items) _workspaceItemIdentity(item),
      ],
      currentIdentity: _workspaceItemIdentity(widget.items[current]),
      targetIdentity: _workspaceItemIdentity(widget.items[candidate]),
      selectionScopeId: _workspaceItemSelectionScopeId(widget.items),
      toggleSelection: _workspaceItemToggleSelectionPressed,
      extendSelection: _workspaceItemRangeSelectionPressed,
    );
    _focusNode.requestFocus();
  }

  KeyEventResult _handleKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent || widget.items.isEmpty) {
      return KeyEventResult.ignored;
    }
    final current = _effectiveSelectedIndex ?? 0;
    final commandAction = _workspaceItemCommandAction(event);
    final onContextAction = widget.onContextAction;
    if (commandAction != null) {
      if (onContextAction == null ||
          !_workspaceItemSupportsContextAction(
            widget.items[current],
            commandAction,
          )) {
        return KeyEventResult.ignored;
      }
      onContextAction(widget.items[current], commandAction);
      return KeyEventResult.handled;
    }
    if (event.logicalKey == LogicalKeyboardKey.enter) {
      (widget.onItemDoubleTap ?? widget.onItemTap)?.call(widget.items[current]);
      return KeyEventResult.handled;
    }
    final delta = switch (event.logicalKey) {
      LogicalKeyboardKey.arrowLeft || LogicalKeyboardKey.arrowUp => -1,
      LogicalKeyboardKey.arrowRight || LogicalKeyboardKey.arrowDown => 1,
      _ => 0,
    };
    if (delta == 0) {
      return KeyEventResult.ignored;
    }
    final candidate = (current + delta).clamp(0, widget.items.length - 1);
    if (candidate == current) {
      final onBoundaryNavigation = delta < 0
          ? widget.onNavigateUp
          : widget.onNavigateDown;
      if (onBoundaryNavigation != null) {
        onBoundaryNavigation();
      }
      return KeyEventResult.handled;
    }
    _selectForDirectionalMove(current, candidate);
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    return Focus(
      focusNode: _focusNode,
      autofocus: _effectiveSelectedIndex != null,
      onKeyEvent: _handleKeyEvent,
      child: Column(
        children: [
          for (var index = 0; index < widget.items.length; index++) ...[
            if (index > 0) SizedBox(height: 7),
            _WorkspaceItemListRow<T>(
              key: ValueKey(
                'workspace-item-row:${_workspaceItemIdentity(widget.items[index])}',
              ),
              item: widget.items[index],
              selected: _isSelected(index),
              contextItems: _contextItemsFor(index),
              onTap: () => _selectFromPointer(index),
              onDoubleTap:
                  widget.onItemDoubleTap == null && widget.onItemTap == null
                  ? null
                  : () => (widget.onItemDoubleTap ?? widget.onItemTap)!(
                      widget.items[index],
                    ),
              onContextAction:
                  widget.onContextAction == null &&
                      widget.onContextActions == null
                  ? null
                  : _contextActionFor(index),
              contextWorkspaceName: widget.contextWorkspaceName,
            ),
          ],
        ],
      ),
    );
  }
}

class _WorkspaceItemCollection<T extends _WorkspaceItemData>
    extends StatelessWidget {
  const _WorkspaceItemCollection({
    required this.items,
    required this.viewMode,
    this.maxColumns = 3,
    this.layoutItemCount,
    this.onItemTap,
    this.onItemDoubleTap,
    this.onContextAction,
    this.onContextActions,
    this.contextWorkspaceName,
    this.navigationController,
    this.onNavigateUp,
    this.onNavigateDown,
  });

  final List<T> items;
  final _WorkspaceViewMode viewMode;
  final int maxColumns;
  final int? layoutItemCount;
  final ValueChanged<T>? onItemTap;
  final ValueChanged<T>? onItemDoubleTap;
  final _WorkspaceContextAction<T>? onContextAction;
  final _WorkspaceContextActions<T>? onContextActions;
  final String? contextWorkspaceName;
  final _WorkspaceItemCollectionNavigationController? navigationController;
  final VoidCallback? onNavigateUp;
  final VoidCallback? onNavigateDown;

  @override
  Widget build(BuildContext context) {
    return switch (viewMode) {
      _WorkspaceViewMode.grid => _WorkspaceItemGrid<T>(
        items: items,
        maxColumns: maxColumns,
        layoutItemCount: layoutItemCount,
        onItemTap: onItemTap,
        onItemDoubleTap: onItemDoubleTap,
        onContextAction: onContextAction,
        onContextActions: onContextActions,
        contextWorkspaceName: contextWorkspaceName,
        navigationController: navigationController,
        onNavigateUp: onNavigateUp,
        onNavigateDown: onNavigateDown,
      ),
      _WorkspaceViewMode.list => _WorkspaceItemList<T>(
        items: items,
        onItemTap: onItemTap,
        onItemDoubleTap: onItemDoubleTap,
        onContextAction: onContextAction,
        onContextActions: onContextActions,
        contextWorkspaceName: contextWorkspaceName,
        navigationController: navigationController,
        onNavigateUp: onNavigateUp,
        onNavigateDown: onNavigateDown,
      ),
    };
  }
}

class _WorkspaceItemCard<T extends _WorkspaceItemData> extends StatefulWidget {
  const _WorkspaceItemCard({
    super.key,
    required this.width,
    required this.item,
    required this.selected,
    required this.contextItems,
    this.onTap,
    this.onDoubleTap,
    this.onContextAction,
    this.contextWorkspaceName,
  });

  final double width;
  final T item;
  final bool selected;
  final List<T> contextItems;
  final VoidCallback? onTap;
  final VoidCallback? onDoubleTap;
  final ValueChanged<_ContextMenuActionId>? onContextAction;
  final String? contextWorkspaceName;

  @override
  State<_WorkspaceItemCard<T>> createState() => _WorkspaceItemCardState<T>();
}

class _WorkspaceItemCardState<T extends _WorkspaceItemData>
    extends State<_WorkspaceItemCard<T>> {
  final Object _contextMenuOverlayToken = Object();
  final _WorkspaceItemTapTracker _tapTracker = _WorkspaceItemTapTracker();
  bool _hovered = false;
  NautermTransientOverlayHandle? _contextMenuOverlay;
  NautermOverlayController? _overlayController;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final nextController = NautermOverlayScope.maybeOf(context);
    if (identical(nextController, _overlayController)) {
      return;
    }
    _removeContextMenu(keepHover: true);
    _overlayController = nextController;
  }

  @override
  void didUpdateWidget(covariant _WorkspaceItemCard<T> oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.item, widget.item)) {
      _tapTracker.reset();
    }
  }

  void _handleTap() {
    widget.onTap?.call();
    if (_workspaceItemMultiSelectionPressed) {
      _tapTracker.reset();
      return;
    }
    final onDoubleTap = widget.onDoubleTap;
    if (onDoubleTap == null) {
      _tapTracker.reset();
      return;
    }
    if (_tapTracker.registerTap()) {
      onDoubleTap();
    }
  }

  @override
  void dispose() {
    _tapTracker.reset();
    final overlay = _contextMenuOverlay;
    _contextMenuOverlay = null;
    overlay?.dismiss(notify: false);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final canEdit =
        widget.contextItems.length == 1 &&
        widget.onContextAction != null &&
        _workspaceItemSupportsContextAction(
          widget.item,
          _ContextMenuActionId.edit,
        );
    return MouseRegion(
      cursor: widget.onTap == null && widget.onDoubleTap == null
          ? SystemMouseCursors.basic
          : SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onSecondaryTapDown:
            widget.onContextAction == null ||
                _contextMenuRowsForSelection(
                  widget.contextItems,
                  currentWorkspaceName: widget.contextWorkspaceName,
                ).isEmpty
            ? null
            : _showContextMenu,
        child: SizedBox(
          width: widget.width,
          height: 58,
          child: Material(
            color: _hovered ? _cardHover : _card,
            elevation: _hovered ? 5 : 2,
            shadowColor: const Color(
              0xff6d858c,
            ).withValues(alpha: _hovered ? 0.22 : 0.14),
            surfaceTintColor: Colors.transparent,
            animationDuration: const Duration(milliseconds: 120),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(12),
              side: widget.selected
                  ? BorderSide(color: _blue, width: 1.5)
                  : BorderSide.none,
            ),
            clipBehavior: Clip.antiAlias,
            child: InkWell(
              borderRadius: BorderRadius.circular(12),
              canRequestFocus: false,
              hoverColor: Colors.transparent,
              splashColor: _workspaceDark
                  ? _workspaceMenuPressed
                  : const Color(0xffdbe8eb).withValues(alpha: 0.42),
              highlightColor: _workspaceDark
                  ? _sidebarHover.withValues(alpha: 0.72)
                  : const Color(0xffe7eff1).withValues(alpha: 0.36),
              onTap: widget.onTap == null && widget.onDoubleTap == null
                  ? null
                  : _handleTap,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 11),
                child: Row(
                  children: [
                    _BrandIcon(
                      icon: widget.item.icon,
                      color: widget.item.color,
                      name: widget.item is _HostItem ? widget.item.name : null,
                      os: widget.item is _HostItem
                          ? (widget.item as _HostItem).os
                          : null,
                      distro: widget.item is _HostItem
                          ? (widget.item as _HostItem).distro
                          : null,
                    ),
                    SizedBox(width: 10),
                    Expanded(
                      child: _CardText(
                        title: widget.item.name,
                        subtitle: widget.item.subtitle,
                      ),
                    ),
                    if (canEdit) ...[
                      SizedBox(width: 6),
                      AnimatedOpacity(
                        opacity: _hovered ? 1 : 0,
                        duration: const Duration(milliseconds: 120),
                        curve: Curves.easeOut,
                        child: IgnorePointer(
                          ignoring: !_hovered,
                          child: _WorkspaceCardEditButton(
                            onPressed: () => widget.onContextAction!(
                              _ContextMenuActionId.edit,
                            ),
                          ),
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  void _showContextMenu(TapDownDetails details) {
    _removeContextMenu(keepHover: true);
    setState(() => _hovered = true);

    final overlayBox =
        Overlay.of(context).context.findRenderObject()! as RenderBox;
    _contextMenuOverlay = showNautermTransientOverlay(
      context: context,
      token: _contextMenuOverlayToken,
      dismissExisting: true,
      onDismissed: () {
        _contextMenuOverlay = null;
        if (mounted) {
          setState(() => _hovered = false);
        }
      },
      builder: (context) => _WorkspaceItemContextMenuOverlay<T>(
        items: widget.contextItems,
        anchor: details.globalPosition,
        overlaySize: overlayBox.size,
        onDismissed: _removeContextMenu,
        currentWorkspaceName: widget.contextWorkspaceName,
        onAction: widget.onContextAction,
      ),
    );
  }

  void _removeContextMenu({bool keepHover = false}) {
    final overlay = _contextMenuOverlay;
    if (overlay == null) {
      if (mounted && !keepHover) {
        setState(() => _hovered = false);
      }
      return;
    }
    if (keepHover) {
      _contextMenuOverlay = null;
      overlay.dismiss(notify: false);
      return;
    }
    overlay.dismiss();
  }
}

class _WorkspaceCardEditButton extends StatelessWidget {
  const _WorkspaceCardEditButton({required this.onPressed});

  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    const shape = CircleBorder();
    return Tooltip(
      message: tr('common.action.edit', fallback: 'Edit'),
      waitDuration: const Duration(milliseconds: 250),
      child: SizedBox(
        width: 26,
        height: 26,
        child: Material(
          color: _workspaceDark
              ? _workspaceMenuPressed.withValues(alpha: 0.72)
              : const Color(0xffedf3f5),
          shape: shape,
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            canRequestFocus: false,
            splashFactory: InkRipple.splashFactory,
            customBorder: shape,
            hoverColor: _workspaceDark
                ? _sidebarHover.withValues(alpha: 0.78)
                : const Color(0xffdce8ec),
            splashColor: _workspaceDark
                ? _workspaceMenuPressed
                : const Color(0xffcbdde2),
            highlightColor: _workspaceDark
                ? _workspaceMenuPressed
                : const Color(0xffd3e2e6),
            onTap: onPressed,
            child: Icon(LucideIcons.pencil, size: 15, color: _blue),
          ),
        ),
      ),
    );
  }
}

class _WorkspaceItemListRow<T extends _WorkspaceItemData>
    extends StatefulWidget {
  const _WorkspaceItemListRow({
    super.key,
    required this.item,
    required this.selected,
    required this.contextItems,
    this.onTap,
    this.onDoubleTap,
    this.onContextAction,
    this.contextWorkspaceName,
  });

  final T item;
  final bool selected;
  final List<T> contextItems;
  final VoidCallback? onTap;
  final VoidCallback? onDoubleTap;
  final ValueChanged<_ContextMenuActionId>? onContextAction;
  final String? contextWorkspaceName;

  @override
  State<_WorkspaceItemListRow<T>> createState() =>
      _WorkspaceItemListRowState<T>();
}

class _WorkspaceItemListRowState<T extends _WorkspaceItemData>
    extends State<_WorkspaceItemListRow<T>> {
  final Object _contextMenuOverlayToken = Object();
  final _WorkspaceItemTapTracker _tapTracker = _WorkspaceItemTapTracker();
  bool _hovered = false;
  NautermTransientOverlayHandle? _contextMenuOverlay;
  NautermOverlayController? _overlayController;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final nextController = NautermOverlayScope.maybeOf(context);
    if (identical(nextController, _overlayController)) {
      return;
    }
    _removeContextMenu(keepHover: true);
    _overlayController = nextController;
  }

  @override
  void didUpdateWidget(covariant _WorkspaceItemListRow<T> oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.item, widget.item)) {
      _tapTracker.reset();
    }
  }

  void _handleTap() {
    widget.onTap?.call();
    if (_workspaceItemMultiSelectionPressed) {
      _tapTracker.reset();
      return;
    }
    final onDoubleTap = widget.onDoubleTap;
    if (onDoubleTap == null) {
      _tapTracker.reset();
      return;
    }
    if (_tapTracker.registerTap()) {
      onDoubleTap();
    }
  }

  @override
  void dispose() {
    _tapTracker.reset();
    final overlay = _contextMenuOverlay;
    _contextMenuOverlay = null;
    overlay?.dismiss(notify: false);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: widget.onTap == null && widget.onDoubleTap == null
          ? SystemMouseCursors.basic
          : SystemMouseCursors.click,
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onSecondaryTapDown:
            widget.onContextAction == null ||
                _contextMenuRowsForSelection(
                  widget.contextItems,
                  currentWorkspaceName: widget.contextWorkspaceName,
                ).isEmpty
            ? null
            : _showContextMenu,
        child: Material(
          color: _hovered ? _cardHover : _card,
          elevation: _hovered ? 3 : 1,
          shadowColor: const Color(
            0xff6d858c,
          ).withValues(alpha: _hovered ? 0.18 : 0.1),
          surfaceTintColor: Colors.transparent,
          animationDuration: const Duration(milliseconds: 120),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(8),
            side: widget.selected
                ? BorderSide(color: _blue, width: 1.5)
                : BorderSide.none,
          ),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            borderRadius: BorderRadius.circular(8),
            canRequestFocus: false,
            hoverColor: Colors.transparent,
            splashColor: _workspaceDark
                ? _workspaceMenuPressed
                : const Color(0xffdbe8eb).withValues(alpha: 0.42),
            highlightColor: _workspaceDark
                ? _sidebarHover.withValues(alpha: 0.72)
                : const Color(0xffe7eff1).withValues(alpha: 0.36),
            onTap: widget.onTap == null && widget.onDoubleTap == null
                ? null
                : _handleTap,
            child: SizedBox(
              height: 52,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 11),
                child: Row(
                  children: [
                    _BrandIcon(
                      icon: widget.item.icon,
                      color: widget.item.color,
                      name: widget.item is _HostItem ? widget.item.name : null,
                      os: widget.item is _HostItem
                          ? (widget.item as _HostItem).os
                          : null,
                      distro: widget.item is _HostItem
                          ? (widget.item as _HostItem).distro
                          : null,
                    ),
                    SizedBox(width: 11),
                    Expanded(
                      child: _CardText(
                        title: widget.item.name,
                        subtitle: widget.item.subtitle,
                      ),
                    ),
                    SizedBox(width: 12),
                    Icon(
                      Icons.more_horiz_rounded,
                      size: 18,
                      color: _hovered
                          ? const Color(0xff71848c)
                          : const Color(0xffb0c0c6),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  void _showContextMenu(TapDownDetails details) {
    _removeContextMenu(keepHover: true);
    setState(() => _hovered = true);

    final overlayBox =
        Overlay.of(context).context.findRenderObject()! as RenderBox;
    _contextMenuOverlay = showNautermTransientOverlay(
      context: context,
      token: _contextMenuOverlayToken,
      dismissExisting: true,
      onDismissed: () {
        _contextMenuOverlay = null;
        if (mounted) {
          setState(() => _hovered = false);
        }
      },
      builder: (context) => _WorkspaceItemContextMenuOverlay<T>(
        items: widget.contextItems,
        anchor: details.globalPosition,
        overlaySize: overlayBox.size,
        onDismissed: _removeContextMenu,
        currentWorkspaceName: widget.contextWorkspaceName,
        onAction: widget.onContextAction,
      ),
    );
  }

  void _removeContextMenu({bool keepHover = false}) {
    final overlay = _contextMenuOverlay;
    if (overlay == null) {
      if (mounted && !keepHover) {
        setState(() => _hovered = false);
      }
      return;
    }
    if (keepHover) {
      _contextMenuOverlay = null;
      overlay.dismiss(notify: false);
      return;
    }
    overlay.dismiss();
  }
}

class _BrandIcon extends StatelessWidget {
  const _BrandIcon({
    required this.icon,
    required this.color,
    this.name,
    this.os,
    this.distro,
  });

  final IconData icon;
  final Color color;
  final String? name;
  final String? os;
  final String? distro;

  @override
  Widget build(BuildContext context) {
    final osSlug = _resolveOsSlug(os, distro);
    final mode = hostIconMode;

    // Mode: icon — replace the entire icon with the OS icon
    if (mode == HostIconMode.osIcon && osSlug != null) {
      final brandColor = _osBrandColor(osSlug);
      return Container(
        width: 40,
        height: 40,
        decoration: BoxDecoration(
          color: brandColor,
          borderRadius: BorderRadius.circular(10),
        ),
        child: Padding(
          padding: const EdgeInsets.all(8),
          child: SvgPicture.asset(
            'assets/icons/os/system-$osSlug.svg',
            width: 22,
            height: 22,
            colorFilter: const ColorFilter.mode(Colors.white, BlendMode.srcIn),
            placeholderBuilder: (_) =>
                Icon(icon, size: 22, color: Colors.white),
          ),
        ),
      );
    }

    // Default inner widget
    final Widget inner;
    if (name != null && name!.isNotEmpty && _isGenericIcon(icon)) {
      inner = _InitialIcon(name: name!);
    } else {
      inner = Container(
        width: 40,
        height: 40,
        decoration: BoxDecoration(
          color: color,
          borderRadius: BorderRadius.circular(10),
        ),
        child: Icon(icon, size: 22, color: Colors.white),
      );
    }

    // Mode: none or no OS detected — just the host icon
    if (mode != HostIconMode.osBadge || osSlug == null) return inner;

    // Mode: badge — host icon with small OS badge
    final brandColor = _osBrandColor(osSlug);
    return Stack(
      clipBehavior: Clip.none,
      children: [
        inner,
        Positioned(
          right: -3,
          bottom: -3,
          child: Container(
            width: 16,
            height: 16,
            decoration: BoxDecoration(
              color: brandColor,
              borderRadius: BorderRadius.circular(4),
              boxShadow: const [
                BoxShadow(
                  color: Color(0x1a000000),
                  blurRadius: 3,
                  offset: Offset(0, 1),
                ),
              ],
            ),
            child: Padding(
              padding: const EdgeInsets.all(2.5),
              child: SvgPicture.asset(
                'assets/icons/os/system-$osSlug.svg',
                width: 11,
                height: 11,
                colorFilter: const ColorFilter.mode(
                  Colors.white,
                  BlendMode.srcIn,
                ),
                placeholderBuilder: (_) => const SizedBox.shrink(),
              ),
            ),
          ),
        ),
      ],
    );
  }

  static bool _isGenericIcon(IconData icon) {
    return icon == LucideIcons.squareTerminal ||
        icon == Icons.public_rounded ||
        icon == Icons.dns_rounded;
  }

  /// Maps os/distro text to a local SVG asset slug (file: system-{slug}.svg).
  static String? _resolveOsSlug(String? os, String? distro) {
    final text = '${os ?? ''} ${distro ?? ''}'.toLowerCase().trim();
    if (text.isEmpty) return null;

    // Specific distros (order matters — more specific first)
    if (text.contains('ubuntu')) return 'ubuntu';
    if (text.contains('debian')) return 'debian';
    if (text.contains('centos')) return 'centos';
    if (text.contains('rhel') || text.contains('redhat')) return 'redhat';
    if (text.contains('fedora')) return 'fedora';
    if (text.contains('alma')) return 'almalinux';
    if (text.contains('rocky')) return 'rockylinux';
    if (text.contains('alpine')) return 'alpine';
    if (text.contains('arch')) return 'arch';
    if (text.contains('manjaro')) return 'manjaro';
    if (text.contains('gentoo')) return 'gentoo';
    if (text.contains('opensuse') || text.contains('suse')) return 'opensuse';
    if (text.contains('mint')) return 'linuxmint';
    if (text.contains('kali')) return 'kalilinux';
    if (text.contains('nixos') || text.contains('nix')) return 'nixos';
    if (text.contains('pop!_os') ||
        text.contains('popos') ||
        text.contains('pop_os')) {
      return 'popos';
    }
    if (text.contains('elementary')) return 'elementary';
    if (text.contains('zorin')) return 'zorin';
    if (text.contains('mxlinux') || text.contains('mx linux')) return 'mxlinux';
    if (text.contains('void')) return 'voidlinux';
    if (text.contains('slackware')) return 'slackware';
    if (text.contains('garuda')) return 'garudalinux';
    if (text.contains('endeavour')) return 'endeavouros';
    if (text.contains('artix')) return 'artixlinux';
    if (text.contains('cachyos') || text.contains('cachy')) return 'cachyos';
    if (text.contains('nobara')) return 'nobaralinux';
    if (text.contains('asahi')) return 'asahilinux';
    if (text.contains('graphene')) return 'grapheneos';
    if (text.contains('qubes')) return 'qubesos';
    if (text.contains('reactos') || text.contains('react')) return 'reactos';

    // BSD family
    if (text.contains('freebsd')) return 'freebsd';
    if (text.contains('openbsd')) return 'openbsd';
    if (text.contains('netbsd')) return 'netbsd';
    if (text.contains('bsd')) return 'bsd';

    // Server / NAS / Network
    if (text.contains('proxmox')) return 'proxmox';
    if (text.contains('vmware') || text.contains('esxi')) return 'vmware';
    if (text.contains('truenas') || text.contains('freenas')) return 'truenas';
    if (text.contains('unraid')) return 'unraid';
    if (text.contains('synology')) return 'synology';
    if (text.contains('qnap')) return 'qnap';
    if (text.contains('openwrt')) return 'openwrt';
    if (text.contains('opnsense')) return 'opnsense';
    if (text.contains('pfsense')) return 'pfsense';
    if (text.contains('mikrotik') || text.contains('routeros')) {
      return 'mikrotik';
    }

    // Mobile / Embedded
    if (text.contains('android')) return 'android';
    if (text.contains('raspberry') || text.contains('raspi')) {
      return 'raspberrypi';
    }

    // Generic Linux (after all specific distros)
    if (text.contains('linux') || text.contains('gnu')) return 'linux';

    // macOS
    if (text.contains('macos') ||
        text.contains('darwin') ||
        text.contains('osx')) {
      return 'apple';
    }

    // Windows
    if (text.contains('windows') ||
        text.contains('win32') ||
        text.contains('winnt')) {
      return 'windows';
    }

    if (text.contains('unix')) return 'linux';

    return null;
  }

  /// Brand color for each OS slug (from simple-icons).
  static Color _osBrandColor(String slug) {
    return switch (slug) {
      // Linux distros
      'linux' => const Color(0xffFCC624),
      'ubuntu' => const Color(0xffE95420),
      'debian' => const Color(0xffA81D33),
      'centos' => const Color(0xff262577),
      'redhat' => const Color(0xffEE0000),
      'fedora' => const Color(0xff51A2DA),
      'almalinux' => const Color(0xff000000),
      'rockylinux' => const Color(0xff10B981),
      'alpine' => const Color(0xff0D597F),
      'arch' => const Color(0xff1793D1),
      'manjaro' => const Color(0xff35BFA4),
      'gentoo' => const Color(0xff54487A),
      'opensuse' => const Color(0xff73BA25),
      'linuxmint' => const Color(0xff86BE43),
      'kalilinux' => const Color(0xff557C94),
      'nixos' => const Color(0xff5277C3),
      'popos' => const Color(0xff48B9C7),
      'elementary' => const Color(0xff64BAFF),
      'zorin' => const Color(0xff15A6F0),
      'mxlinux' => const Color(0xff000000),
      'voidlinux' => const Color(0xff478061),
      'slackware' => const Color(0xff000000),
      'garudalinux' => const Color(0xff8839EF),
      'endeavouros' => const Color(0xff7F7FFF),
      'artixlinux' => const Color(0xff10A0CC),
      'cachyos' => const Color(0xff00AA88),
      'nobaralinux' => const Color(0xff000000),
      'asahilinux' => const Color(0xffA61200),
      'grapheneos' => const Color(0xff0053A3),
      'qubesos' => const Color(0xff3874D8),
      'reactos' => const Color(0xff0088CC),

      // BSD
      'freebsd' => const Color(0xffAB2B28),
      'openbsd' => const Color(0xffF2CA30),
      'netbsd' => const Color(0xffFF6600),
      'bsd' => const Color(0xffAB2B28),

      // Server / NAS / Network
      'proxmox' => const Color(0xffE57000),
      'vmware' => const Color(0xff607078),
      'truenas' => const Color(0xff0095D5),
      'unraid' => const Color(0xffF15A2C),
      'synology' => const Color(0xffB5B5B6),
      'qnap' => const Color(0xff0C2E82),
      'openwrt' => const Color(0xff00B5E2),
      'opnsense' => const Color(0xffE44A20),
      'pfsense' => const Color(0xff212121),
      'mikrotik' => const Color(0xff293239),

      // Mobile / Embedded
      'android' => const Color(0xff3DDC84),
      'raspberrypi' => const Color(0xffA22846),

      // macOS / Windows
      'apple' => const Color(0xff000000),
      'windows' => const Color(0xff0078D4),

      _ => const Color(0xff6b7280),
    };
  }
}

class _InitialIcon extends StatelessWidget {
  const _InitialIcon({required this.name});

  final String name;

  @override
  Widget build(BuildContext context) {
    final initial = name.isNotEmpty ? name[0].toUpperCase() : '?';
    final bgColor = _deterministicColor(name);

    return Container(
      width: 40,
      height: 40,
      decoration: BoxDecoration(
        color: bgColor,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Center(
        child: Text(
          initial,
          style: TextStyle(
            color: Colors.white,
            fontSize: 20,
            fontWeight: FontWeight.w600,
          ),
        ),
      ),
    );
  }

  static Color _deterministicColor(String name) {
    if (name.isEmpty) return const Color(0xff7b8f9a);

    // Generate a simple hash from the name
    int hash = 0;
    for (int i = 0; i < name.length; i++) {
      hash = name.codeUnitAt(i) + ((hash << 5) - hash);
    }

    // Balanced palette - medium-high saturation, medium lightness
    const colors = [
      Color(0xff4d9cbd), // teal
      Color(0xff9965ba), // purple
      Color(0xffbd6767), // rose
      Color(0xff60b86a), // green
      Color(0xffbda050), // amber
      Color(0xff547abd), // blue
      Color(0xffbd558a), // magenta
      Color(0xff54bdb2), // cyan
      Color(0xff86bd55), // lime
      Color(0xff7458bd), // indigo
      Color(0xffbd8255), // orange
      Color(0xff54bd82), // emerald
    ];

    return colors[hash.abs() % colors.length];
  }
}

class _CardText extends StatelessWidget {
  const _CardText({required this.title, required this.subtitle});

  final String title;
  final String subtitle;

  @override
  Widget build(BuildContext context) {
    final hasSubtitle = subtitle.trim().isNotEmpty;

    return Column(
      mainAxisAlignment: MainAxisAlignment.center,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: TextStyle(
            color: _text,
            fontSize: NautermFontSizes.labelLarge,
            fontWeight: NautermFontWeights.semibold,
            letterSpacing: 0,
          ),
        ),
        if (hasSubtitle) ...[
          SizedBox(height: 2),
          Text(
            subtitle,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: _mutedText,
              fontSize: NautermFontSizes.labelSmall,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
        ],
      ],
    );
  }
}
