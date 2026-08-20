part of 'nauterm_workspace.dart';

class _HostsPane extends StatefulWidget {
  const _HostsPane({
    required this.groups,
    required this.hosts,
    required this.tags,
    required this.loading,
    required this.onCreateHost,
    required this.onCreateGroup,
    required this.onImportHosts,
    required this.onExportHosts,
    required this.onGroupContextAction,
    required this.onGroupContextActions,
    required this.onHostContextAction,
    required this.onHostContextActions,
    required this.onHostConnected,
    required this.onConnectQuery,
    required this.onSaveTag,
    required this.onDeleteTag,
    required this.onOpenLocalTerminal,
    required this.onOpenSerialTerminal,
    required this.currentWorkspaceName,
  });

  final List<_GroupItem> groups;
  final List<_HostItem> hosts;
  final List<TagEntry> tags;
  final bool loading;
  final ValueChanged<int?> onCreateHost;
  final ValueChanged<int?> onCreateGroup;
  final VoidCallback onImportHosts;
  final VoidCallback onExportHosts;
  final _WorkspaceContextAction<_GroupItem> onGroupContextAction;
  final _WorkspaceContextActions<_GroupItem> onGroupContextActions;
  final _WorkspaceContextAction<_HostItem> onHostContextAction;
  final _WorkspaceContextActions<_HostItem> onHostContextActions;
  final ValueChanged<_HostItem> onHostConnected;
  final ValueChanged<String> onConnectQuery;
  final ValueChanged<TagEntry> onSaveTag;
  final ValueChanged<TagEntry> onDeleteTag;
  final VoidCallback onOpenLocalTerminal;
  final VoidCallback onOpenSerialTerminal;
  final String currentWorkspaceName;

  @override
  State<_HostsPane> createState() => _HostsPaneState();
}

class _HostsPaneState extends State<_HostsPane> {
  final _searchController = TextEditingController();
  final _groupNavigation = _WorkspaceItemCollectionNavigationController();
  final _hostNavigation = _WorkspaceItemCollectionNavigationController();
  late final FocusNode _searchFocusNode;
  _WorkspaceSortOrder _sortOrder = _WorkspaceSortOrder.nameAscending;
  _WorkspaceViewMode _viewMode = _WorkspaceViewMode.grid;
  int? _currentGroupId;
  Set<String> _selectedTagUuids = {};

  @override
  void initState() {
    super.initState();
    _searchFocusNode = FocusNode(
      debugLabel: 'host search',
      onKeyEvent: _handleSearchKeyEvent,
    );
  }

  @override
  void dispose() {
    _searchFocusNode.dispose();
    _searchController.dispose();
    super.dispose();
  }

  KeyEventResult _handleSearchKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    if (!_isQuickConnectKeyEvent(event, terminalShortcutConfig)) {
      return KeyEventResult.ignored;
    }
    if (event is KeyDownEvent) {
      Actions.invoke(context, const _QuickConnectIntent());
    }
    return KeyEventResult.handled;
  }

  int? get _effectiveGroupId {
    final groupId = _currentGroupId;
    if (groupId == null || widget.groups.any((group) => group.id == groupId)) {
      return groupId;
    }
    return null;
  }

  Set<String> get _effectiveSelectedTagUuids {
    final available = {
      for (final tag in widget.tags)
        if (tag.uuid != null) tag.uuid!,
    };
    return _selectedTagUuids.intersection(available);
  }

  List<_GroupItem> _filteredGroups(int? groupId) {
    final query = _HostSearchQuery.parse(_searchController.text);
    final scopedGroups = widget.groups.where(
      (group) => group.parentId == groupId,
    );
    if (query.isEmpty) {
      return _sortWorkspaceItems(
        scopedGroups,
        _sortOrder,
        ordinal: (group) => group.id,
      );
    }
    if (query.hosts.isNotEmpty ||
        query.usernames.isNotEmpty ||
        query.tags.isNotEmpty) {
      return const [];
    }
    return _sortWorkspaceItems(
      scopedGroups.where((group) => query.matchesGroup(group.name)),
      _sortOrder,
      ordinal: (group) => group.id,
    );
  }

  List<_HostItem> _filteredHosts(int? groupId) {
    final query = _HostSearchQuery.parse(_searchController.text);
    final tagNamesByUuid = _tagNamesByUuid(widget.tags);
    final groupHosts = groupId == null
        ? widget.hosts
        : widget.hosts.where((host) => host.groupId == groupId);
    final selectedTagUuids = _effectiveSelectedTagUuids;
    final scopedHosts = selectedTagUuids.isEmpty
        ? groupHosts
        : groupHosts.where(
            (host) => host.tagUuids.any(selectedTagUuids.contains),
          );
    if (query.isEmpty) {
      return _sortWorkspaceItems(
        scopedHosts,
        _sortOrder,
        ordinal: (host) => host.id,
      );
    }
    return _sortWorkspaceItems(
      scopedHosts.where((host) {
        final groupName = widget.groups
            .where((group) => group.id == host.groupId)
            .firstOrNull
            ?.name;
        return query.matchesHost(
          host,
          groupName,
          tagNames: _tagNamesForHost(host, tagNamesByUuid),
        );
      }),
      _sortOrder,
      ordinal: (host) => host.id,
    );
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final currentGroupId = _effectiveGroupId;
        final groups = _filteredGroups(currentGroupId);
        final hosts = _filteredHosts(currentGroupId);
        final sharedGridItemCount = math.max(groups.length, hosts.length);
        final breadcrumbPath = _groupBreadcrumbPath(currentGroupId);
        final showingGroup = currentGroupId != null;
        final canCreateGroup = breadcrumbPath.length < 2;
        final hasStoredItems =
            widget.groups.isNotEmpty || widget.hosts.isNotEmpty;
        final showEmptyState =
            !widget.loading &&
            !hasStoredItems &&
            _searchController.text.trim().isEmpty;

        return Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  _HostSearchBar(
                    controller: _searchController,
                    focusNode: _searchFocusNode,
                    onChanged: () => setState(() {}),
                    onConnect: widget.onConnectQuery,
                  ),
                  _HostsToolbar(
                    onCreateHost: () => widget.onCreateHost(currentGroupId),
                    onCreateGroup: canCreateGroup
                        ? () => widget.onCreateGroup(currentGroupId)
                        : null,
                    onImportHosts: widget.onImportHosts,
                    onExportHosts: widget.onExportHosts,
                    onOpenLocalTerminal: widget.onOpenLocalTerminal,
                    onOpenSerialTerminal: widget.onOpenSerialTerminal,
                    sortOrder: _sortOrder,
                    onSortOrderChanged: (value) =>
                        setState(() => _sortOrder = value),
                    viewMode: _viewMode,
                    onViewModeChanged: (value) =>
                        setState(() => _viewMode = value),
                    tags: widget.tags,
                    selectedTagUuids: _effectiveSelectedTagUuids,
                    onTagSelectionChanged: (value) =>
                        setState(() => _selectedTagUuids = value),
                    onSaveTag: widget.onSaveTag,
                    onDeleteTag: widget.onDeleteTag,
                  ),
                  Expanded(
                    child: showEmptyState
                        ? _WorkspaceEmptyState(
                            icon: LucideIcons.server,
                            title: 'No hosts yet',
                            description: 'Add a host to save connection details and open remote sessions quickly.',
                            actionLabel: 'Add host',
                            onAction: () => widget.onCreateHost(currentGroupId),
                          )
                        : SingleChildScrollView(
                            padding: _workspacePanePadding,
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                if (showingGroup) ...[
                                  _GroupBreadcrumb(
                                    path: breadcrumbPath,
                                    onSelected: _openGroupId,
                                  ),
                                  SizedBox(height: 18),
                                ],
                                if (widget.loading)
                                  const _WorkspaceLoadingLine()
                                else if (groups.isNotEmpty) ...[
                                  const _SectionTitle('Groups'),
                                  SizedBox(height: 12),
                                  _WorkspaceItemCollection(
                                    items: groups,
                                    viewMode: _viewMode,
                                    maxColumns: 3,
                                    layoutItemCount: sharedGridItemCount,
                                    navigationController: _groupNavigation,
                                    onNavigateDown: hosts.isEmpty
                                        ? null
                                        : _hostNavigation.focusFirst,
                                    onItemTap: _openGroup,
                                    onContextAction: _handleGroupContextAction,
                                    onContextActions:
                                        widget.onGroupContextActions,
                                  ),
                                ],
                                if (groups.isNotEmpty && hosts.isNotEmpty)
                                  SizedBox(height: 28),
                                if (widget.loading)
                                  const _WorkspaceLoadingLine()
                                else if (hosts.isNotEmpty) ...[
                                  const _SectionTitle('Hosts'),
                                  SizedBox(height: 12),
                                  _WorkspaceItemCollection(
                                    items: hosts,
                                    viewMode: _viewMode,
                                    layoutItemCount: sharedGridItemCount,
                                    navigationController: _hostNavigation,
                                    onNavigateUp: groups.isEmpty
                                        ? null
                                        : _groupNavigation.focusLast,
                                    onItemDoubleTap: widget.onHostConnected,
                                    onContextAction: widget.onHostContextAction,
                                    onContextActions:
                                        widget.onHostContextActions,
                                    contextWorkspaceName:
                                        widget.currentWorkspaceName,
                                  ),
                                ],
                                if (!widget.loading &&
                                    groups.isEmpty &&
                                    hosts.isEmpty)
                                  _WorkspaceInlineMessage(
                                    _searchController.text.trim().isEmpty
                                        ? 'No hosts in this group.'
                                        : 'No hosts found.',
                                  ),
                              ],
                            ),
                          ),
                  ),
                ],
              ),
            ),
          ],
        );
      },
    );
  }

  void _openGroup(_GroupItem group) {
    setState(() => _currentGroupId = group.id);
  }

  void _openGroupId(int? groupId) {
    setState(() => _currentGroupId = groupId);
  }

  void _handleGroupContextAction(
    _GroupItem group,
    _ContextMenuActionId action,
  ) {
    if (action == _ContextMenuActionId.open) {
      _openGroup(group);
      return;
    }
    widget.onGroupContextAction(group, action);
  }

  List<_GroupItem> _groupBreadcrumbPath(int? groupId) {
    final byId = {for (final group in widget.groups) group.id: group};
    final path = <_GroupItem>[];
    final seen = <int>{};
    var current = groupId == null ? null : byId[groupId];
    while (current != null && seen.add(current.id)) {
      path.insert(0, current);
      final parentId = current.parentId;
      current = parentId == null ? null : byId[parentId];
    }
    return path;
  }
}

class _WorkspaceLoadingLine extends StatelessWidget {
  const _WorkspaceLoadingLine();

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 58,
      child: Align(
        alignment: Alignment.centerLeft,
        child: SizedBox(
          width: 18,
          height: 18,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
      ),
    );
  }
}

class _WorkspaceInlineMessage extends StatelessWidget {
  const _WorkspaceInlineMessage(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 58,
      child: Align(
        alignment: Alignment.centerLeft,
        child: Text(
          tr(text),
          style: TextStyle(
            color: _mutedText,
            fontSize: NautermFontSizes.labelLarge,
            fontWeight: NautermFontWeights.regular,
            letterSpacing: 0,
          ),
        ),
      ),
    );
  }
}

class _WorkspaceEmptyState extends StatelessWidget {
  const _WorkspaceEmptyState({
    super.key,
    required this.icon,
    required this.title,
    this.description,
    this.actionLabel,
    this.onAction,
  }) : assert(
         (actionLabel == null) == (onAction == null),
         'actionLabel and onAction must be provided together',
       );

  final IconData icon;
  final String title;
  final String? description;
  final String? actionLabel;
  final VoidCallback? onAction;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 360),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 48,
                height: 48,
                decoration: BoxDecoration(
                  color: _sidebarHover,
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Icon(icon, size: 23, color: _mutedText),
              ),
              SizedBox(height: 14),
              Text(
                tr(title),
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: _text,
                  fontSize: NautermFontSizes.titleMedium,
                  fontWeight: NautermFontWeights.semibold,
                  letterSpacing: 0,
                ),
              ),
              if (description != null) ...[
                SizedBox(height: 6),
                Text(
                  tr(description!),
                  textAlign: TextAlign.center,
                  style: TextStyle(
                    color: _mutedText,
                    fontSize: NautermFontSizes.labelLarge,
                    fontWeight: NautermFontWeights.regular,
                    height: 1.4,
                    letterSpacing: 0,
                  ),
                ),
              ],
              if (actionLabel != null) ...[
                SizedBox(height: 18),
                KeyedSubtree(
                  key: ValueKey('empty-state-action:$actionLabel'),
                  child: _WorkspaceButton(
                    icon: Icons.add_rounded,
                    label: actionLabel!,
                    type: _WorkspaceButtonType.primary,
                    variant: _WorkspaceButtonVariant.solid,
                    size: _WorkspaceControlSize.medium,
                    onPressed: onAction!,
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _GroupBreadcrumb extends StatelessWidget {
  const _GroupBreadcrumb({required this.path, required this.onSelected});

  final List<_GroupItem> path;
  final ValueChanged<int?> onSelected;

  @override
  Widget build(BuildContext context) {
    final entries = <({String label, int? id})>[
      (label: 'Hosts', id: null),
      for (final group in path) (label: group.name, id: group.id),
    ];

    return SizedBox(
      height: 28,
      child: ListView.separated(
        scrollDirection: Axis.horizontal,
        itemCount: entries.length,
        separatorBuilder: (context, index) => Padding(
          padding: EdgeInsets.symmetric(horizontal: 3),
          child: Icon(
            Icons.chevron_right_rounded,
            size: 17,
            color: Color(0xff8da1a7),
          ),
        ),
        itemBuilder: (context, index) {
          final entry = entries[index];
          final current = index == entries.length - 1;
          return _GroupBreadcrumbButton(
            label: entry.label,
            current: current,
            onPressed: current ? null : () => onSelected(entry.id),
          );
        },
      ),
    );
  }
}

class _GroupBreadcrumbButton extends StatelessWidget {
  const _GroupBreadcrumbButton({
    required this.label,
    required this.current,
    required this.onPressed,
  });

  final String label;
  final bool current;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: current ? _sidebarHover : Colors.transparent,
      borderRadius: BorderRadius.circular(7),
      child: InkWell(
        borderRadius: BorderRadius.circular(7),
        onTap: onPressed,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 9),
          child: Center(
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: current ? _text : _mutedText,
                fontSize: NautermFontSizes.labelMedium,
                fontWeight: current
                    ? NautermFontWeights.semibold
                    : NautermFontWeights.medium,
                letterSpacing: 0,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _HostSearchBar extends StatelessWidget {
  const _HostSearchBar({
    required this.controller,
    required this.focusNode,
    required this.onChanged,
    required this.onConnect,
  });

  final TextEditingController controller;
  final FocusNode focusNode;
  final VoidCallback onChanged;
  final ValueChanged<String> onConnect;

  @override
  Widget build(BuildContext context) {
    final query = controller.text.trim();
    final canConnect = query.contains('@');

    return Container(
      height: _WorkspaceControlSize.medium.inputHeight + 16,
      color: _card,
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      child: Row(
        children: [
          Expanded(
            child: Container(
              key: const ValueKey('host-search-input'),
              height: _WorkspaceControlSize.medium.inputHeight,
              decoration: BoxDecoration(
                color: _sidebarHover,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: _sidebarDivider),
              ),
              padding: const EdgeInsets.only(left: 12, right: 3),
              alignment: Alignment.centerLeft,
              child: Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: controller,
                      focusNode: focusNode,
                      onChanged: (_) => onChanged(),
                      onSubmitted: canConnect ? onConnect : null,
                      style: TextStyle(
                        color: _text,
                        fontSize: _WorkspaceControlSize.medium.inputFontSize,
                        fontWeight: NautermFontWeights.regular,
                        letterSpacing: 0,
                      ),
                      decoration: InputDecoration(
                        isDense: true,
                        contentPadding: EdgeInsets.zero,
                        border: InputBorder.none,
                        hintText: tr(
                          'workspace.search.hosts.placeholder',
                          fallback: 'Search host:, group:, username:, tag:, or user@host',
                        ),
                        hintStyle: TextStyle(
                          color: _mutedText,
                          fontSize: _WorkspaceControlSize.medium.inputFontSize,
                          fontWeight: NautermFontWeights.regular,
                          letterSpacing: 0,
                        ),
                      ),
                    ),
                  ),
                  SizedBox(width: 8),
                  _WorkspaceButton(
                    label: 'Connect',
                    size: _WorkspaceControlSize.tiny,
                    variant: _WorkspaceButtonVariant.solid,
                    type: _WorkspaceButtonType.primary,
                    height: 28,
                    horizontalPadding: 13,
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(6),
                    ),
                    onPressed: canConnect ? () => onConnect(query) : null,
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _HostSearchQuery {
  const _HostSearchQuery({
    required this.terms,
    required this.hosts,
    required this.groups,
    required this.usernames,
    required this.tags,
  });

  final List<String> terms;
  final List<String> hosts;
  final List<String> groups;
  final List<String> usernames;
  final List<String> tags;

  bool get isEmpty =>
      terms.isEmpty &&
      hosts.isEmpty &&
      groups.isEmpty &&
      usernames.isEmpty &&
      tags.isEmpty;

  static _HostSearchQuery parse(String input) {
    final terms = <String>[];
    final hosts = <String>[];
    final groups = <String>[];
    final usernames = <String>[];
    final tags = <String>[];

    for (final token in input.trim().toLowerCase().split(RegExp(r'\s+'))) {
      if (token.isEmpty) {
        continue;
      }

      final separator = token.indexOf(':');
      if (separator > 0) {
        final key = token.substring(0, separator);
        final value = token.substring(separator + 1).trim();
        if (value.isNotEmpty) {
          switch (key) {
            case 'host':
              hosts.add(value);
              continue;
            case 'group':
              groups.add(value);
              continue;
            case 'username':
            case 'user':
              usernames.add(value);
              continue;
            case 'tag':
              tags.add(value);
              continue;
          }
        }
      }

      terms.add(token);
    }

    return _HostSearchQuery(
      terms: terms,
      hosts: hosts,
      groups: groups,
      usernames: usernames,
      tags: tags,
    );
  }

  bool matchesGroup(String groupName) {
    final normalizedGroup = groupName.toLowerCase();
    return _allMatch(terms, [normalizedGroup]) &&
        _allMatch(groups, [normalizedGroup]);
  }

  bool matchesHost(
    _HostItem host,
    String? groupName, {
    Iterable<String> tagNames = const [],
  }) {
    final hostValues = [
      host.name.toLowerCase(),
      if (host.host case final address?) address.toLowerCase(),
    ];
    final groupValues = [if (groupName case final name?) name.toLowerCase()];
    final usernameValues = [
      if (host.username case final username?) username.toLowerCase(),
    ];
    final tagValues = [for (final tagName in tagNames) tagName.toLowerCase()];
    final defaultValues = [
      ...hostValues,
      ...groupValues,
      ...usernameValues,
      ...tagValues,
    ];

    return _allMatch(terms, defaultValues) &&
        _allMatch(hosts, hostValues) &&
        _allMatch(groups, groupValues) &&
        _allMatch(usernames, usernameValues) &&
        _allMatch(tags, tagValues);
  }

  bool _allMatch(List<String> needles, List<String> haystacks) {
    return needles.every(
      (needle) => haystacks.any((haystack) => haystack.contains(needle)),
    );
  }
}

Iterable<String> _tagNamesForHost(
  _HostItem host,
  Map<String, String> tagNamesByUuid,
) {
  return host.tagUuids.map((uuid) => tagNamesByUuid[uuid]).whereType<String>();
}

Map<String, String> _tagNamesByUuid(Iterable<TagEntry> tags) {
  final names = <String, String>{};
  for (final tag in tags) {
    final uuid = tag.uuid;
    if (uuid != null) names[uuid] = tag.name;
  }
  return names;
}

class _DirectHostQuery {
  const _DirectHostQuery({
    required this.username,
    required this.host,
    required this.port,
  });

  final String username;
  final String host;
  final int port;

  String get label => port == 22 ? '$username@$host' : '$username@$host:$port';

  static _DirectHostQuery? parse(String input) {
    final value = input.trim();
    final at = value.indexOf('@');
    if (at <= 0 || at == value.length - 1) {
      return null;
    }

    final username = value.substring(0, at).trim();
    final destination = value.substring(at + 1).trim();
    if (username.isEmpty || destination.isEmpty) {
      return null;
    }

    if (destination.startsWith('[')) {
      final closeBracket = destination.indexOf(']');
      if (closeBracket <= 1) {
        return null;
      }
      final host = destination.substring(1, closeBracket);
      final rest = destination.substring(closeBracket + 1);
      final port = rest.isEmpty
          ? 22
          : rest.startsWith(':')
          ? int.tryParse(rest.substring(1))
          : null;
      if (port == null || !_isValidPort(port)) {
        return null;
      }
      return _DirectHostQuery(username: username, host: host, port: port);
    }

    final colon = destination.lastIndexOf(':');
    final hasPort = colon > 0 && destination.indexOf(':') == colon;
    final host = hasPort ? destination.substring(0, colon) : destination;
    final port = hasPort ? int.tryParse(destination.substring(colon + 1)) : 22;
    if (host.isEmpty || port == null || !_isValidPort(port)) {
      return null;
    }
    return _DirectHostQuery(username: username, host: host, port: port);
  }

  static bool _isValidPort(int port) => port >= 1 && port <= 65535;
}
