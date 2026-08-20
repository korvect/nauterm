part of 'nauterm_workspace.dart';

enum _WorkspaceSortOrder {
  nameAscending(
    'A-z',
    'Name A-z',
    'workspace.sort.nameAscending',
    LucideIcons.arrowDownAZ,
  ),
  nameDescending(
    'Z-a',
    'Name Z-a',
    'workspace.sort.nameDescending',
    LucideIcons.arrowDownZA,
  ),
  newestFirst(
    'Newest',
    'Newest to oldest',
    'workspace.sort.newestFirst',
    LucideIcons.calendarArrowDown,
  ),
  oldestFirst(
    'Oldest',
    'Oldest to newest',
    'workspace.sort.oldestFirst',
    LucideIcons.calendarArrowUp,
  );

  const _WorkspaceSortOrder(
    this.shortLabel,
    this.label,
    this.localizationKey,
    this.icon,
  );

  final String shortLabel;
  final String label;
  final String localizationKey;
  final IconData icon;

  String get localizedLabel => tr(localizationKey, fallback: label);
}

typedef _WorkspaceSortOrdinal<T> = int Function(T item);

List<T> _sortWorkspaceItems<T extends _WorkspaceItemData>(
  Iterable<T> items,
  _WorkspaceSortOrder order, {
  required _WorkspaceSortOrdinal<T> ordinal,
  String Function(T item)? name,
}) {
  final sorted = items.toList();
  final itemName = name ?? (item) => item.name;
  sorted.sort((a, b) {
    final nameResult = itemName(a)
        .toLowerCase()
        .compareTo(itemName(b).toLowerCase());
    final ordinalResult = ordinal(a).compareTo(ordinal(b));
    final createdResult = _compareCreatedAt(a, b) ?? ordinalResult;
    return switch (order) {
      _WorkspaceSortOrder.nameAscending =>
        nameResult == 0 ? ordinalResult : nameResult,
      _WorkspaceSortOrder.nameDescending =>
        nameResult == 0 ? -ordinalResult : -nameResult,
      _WorkspaceSortOrder.newestFirst => -createdResult,
      _WorkspaceSortOrder.oldestFirst => createdResult,
    };
  });
  return sorted;
}

int? _compareCreatedAt(_WorkspaceItemData a, _WorkspaceItemData b) {
  final left = a.createdAt;
  final right = b.createdAt;
  if (left == null || right == null) {
    return null;
  }
  final result = left.compareTo(right);
  return result == 0 ? null : result;
}

List<T> _filterWorkspaceItems<T extends _WorkspaceItemData>(
  Iterable<T> items,
  String query, {
  Iterable<String> Function(T item)? extraText,
}) {
  final tokens = query
      .trim()
      .toLowerCase()
      .split(RegExp(r'\s+'))
      .where((token) => token.isNotEmpty)
      .toList(growable: false);
  if (tokens.isEmpty) {
    return items.toList(growable: false);
  }

  return [
    for (final item in items)
      if (_workspaceItemMatchesTokens(item, tokens, extraText?.call(item)))
        item,
  ];
}

bool _workspaceItemMatchesTokens(
  _WorkspaceItemData item,
  List<String> tokens,
  Iterable<String>? extraText,
) {
  final haystack = [
    item.name,
    item.subtitle,
    ...?extraText,
  ].join('\n').toLowerCase();
  return tokens.every(haystack.contains);
}

class _WorkspaceSortDropdownButton extends StatelessWidget {
  const _WorkspaceSortDropdownButton({
    required this.value,
    required this.onChanged,
  });

  final _WorkspaceSortOrder value;
  final ValueChanged<_WorkspaceSortOrder> onChanged;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceDropdown<_WorkspaceSortOrder>(
      width: 210,
      entries: [
        for (final option in _WorkspaceSortOrder.values)
          NautermContextMenuAction<_WorkspaceSortOrder>(
            value: option,
            label: option.localizedLabel,
            icon: option.icon,
            selected: option == value,
          ),
      ],
      onSelected: (selected) {
        if (selected != value) {
          onChanged(selected);
        }
      },
      triggerBuilder: (openMenu) => Tooltip(
        message: tr(
          'workspace.tooltip.sort',
          fallback: 'Sort: {order}',
          args: {'order': value.localizedLabel},
        ),
        child: _SquareIconButton(icon: value.icon, onTap: openMenu),
      ),
    );
  }
}
