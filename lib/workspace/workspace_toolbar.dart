part of 'nauterm_workspace.dart';

const double _workspaceToolbarControlExtent = 30;

class _HostsToolbar extends StatelessWidget {
  const _HostsToolbar({
    required this.onCreateHost,
    required this.onCreateGroup,
    required this.onImportHosts,
    required this.onExportHosts,
    required this.onOpenLocalTerminal,
    required this.onOpenSerialTerminal,
    required this.sortOrder,
    required this.onSortOrderChanged,
    required this.viewMode,
    required this.onViewModeChanged,
    required this.tags,
    required this.selectedTagUuids,
    required this.onTagSelectionChanged,
    required this.onSaveTag,
    required this.onDeleteTag,
  });

  final VoidCallback onCreateHost;
  final VoidCallback? onCreateGroup;
  final VoidCallback onImportHosts;
  final VoidCallback onExportHosts;
  final VoidCallback onOpenLocalTerminal;
  final VoidCallback onOpenSerialTerminal;
  final _WorkspaceSortOrder sortOrder;
  final ValueChanged<_WorkspaceSortOrder> onSortOrderChanged;
  final _WorkspaceViewMode viewMode;
  final ValueChanged<_WorkspaceViewMode> onViewModeChanged;
  final List<TagEntry> tags;
  final Set<String> selectedTagUuids;
  final ValueChanged<Set<String>> onTagSelectionChanged;
  final ValueChanged<TagEntry> onSaveTag;
  final ValueChanged<TagEntry> onDeleteTag;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceToolbarFrame(
      children: [
        Expanded(
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              _WorkspaceDropdown<_HostCreateAction>(
                width: 180,
                entries: [
                  NautermContextMenuAction<_HostCreateAction>(
                    value: _HostCreateAction.group,
                    label: 'New group',
                    icon: Icons.create_new_folder_rounded,
                    enabled: onCreateGroup != null,
                  ),
                  const NautermContextMenuAction<_HostCreateAction>(
                    value: _HostCreateAction.import,
                    label: 'Import…',
                    icon: Icons.file_download_outlined,
                  ),
                  const NautermContextMenuAction<_HostCreateAction>(
                    value: _HostCreateAction.export,
                    label: 'Export…',
                    icon: Icons.file_upload_outlined,
                  ),
                ],
                onSelected: (selected) {
                  switch (selected) {
                    case _HostCreateAction.group:
                      onCreateGroup?.call();
                    case _HostCreateAction.import:
                      onImportHosts();
                    case _HostCreateAction.export:
                      onExportHosts();
                  }
                },
                triggerBuilder: (openMenu) => _ToolbarSplitButton(
                  icon: Icons.add_rounded,
                  label: 'New host',
                  onPrimaryPressed: onCreateHost,
                  onSecondaryPressed: (_) => openMenu(),
                ),
              ),
              SizedBox(width: 10),
              _ResponsiveModeButton(
                icon: LucideIcons.squareTerminal,
                label: 'Terminal',
                onTap: onOpenLocalTerminal,
              ),
              SizedBox(width: 6),
              _ResponsiveModeButton(
                icon: LucideIcons.usb,
                label: 'Serial',
                onTap: onOpenSerialTerminal,
              ),
            ],
          ),
        ),
        _HostTagFilterButton(
          tags: tags,
          selectedTagUuids: selectedTagUuids,
          onSelectionChanged: onTagSelectionChanged,
          onSave: onSaveTag,
          onDelete: onDeleteTag,
        ),
        SizedBox(width: 6),
        ..._defaultToolbarTrailingActions(
          sortOrder: sortOrder,
          onSortOrderChanged: onSortOrderChanged,
          viewMode: viewMode,
          onViewModeChanged: onViewModeChanged,
        ),
      ],
    );
  }
}

enum _HostCreateAction { group, import, export }

class _HostTagFilterButton extends StatefulWidget {
  const _HostTagFilterButton({
    required this.tags,
    required this.selectedTagUuids,
    required this.onSelectionChanged,
    this.onSave,
    this.onDelete,
  });

  final List<TagEntry> tags;
  final Set<String> selectedTagUuids;
  final ValueChanged<Set<String>> onSelectionChanged;
  final ValueChanged<TagEntry>? onSave;
  final ValueChanged<TagEntry>? onDelete;

  @override
  State<_HostTagFilterButton> createState() => _HostTagFilterButtonState();
}

class _HostTagFilterButtonState extends State<_HostTagFilterButton> {
  final TextEditingController _searchController = TextEditingController();
  final ScrollController _tagListController = ScrollController();
  NautermTransientOverlayHandle? _menu;

  @override
  void didUpdateWidget(covariant _HostTagFilterButton oldWidget) {
    super.didUpdateWidget(oldWidget);
    _menu?.markNeedsBuild();
  }

  @override
  void dispose() {
    _menu?.dismiss(notify: false);
    _searchController.dispose();
    _tagListController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return _WorkspaceDropdown<void>(
      width: 210,
      onOpen: _toggleMenu,
      triggerBuilder: (openMenu) => Tooltip(
        message: tr(
          'workspace.tooltip.filterByTags',
          fallback: 'Filter by tags',
        ),
        child: _SquareIconButton(icon: LucideIcons.tag, onTap: openMenu),
      ),
    );
  }

  void _toggleMenu(BuildContext buttonContext, Rect anchor) {
    if (_menu != null) {
      _closeMenu();
      return;
    }
    _searchController.clear();
    _menu = showNautermTransientOverlay(
      context: buttonContext,
      token: Object(),
      dismissExisting: true,
      onDismissed: () {
        _menu = null;
        if (mounted) setState(() {});
      },
      builder: (context) => _buildMenuOverlay(context, anchor),
    );
    setState(() {});
    WidgetsBinding.instance.addPostFrameCallback((_) => _resetListScroll());
  }

  void _closeMenu() {
    _menu?.dismiss();
  }

  void _handleSearchChanged(String _) {
    _resetListScroll();
    _menu?.markNeedsBuild();
    WidgetsBinding.instance.addPostFrameCallback((_) => _resetListScroll());
  }

  void _resetListScroll() {
    if (mounted && _tagListController.hasClients) {
      _tagListController.jumpTo(0);
    }
  }

  Widget _buildMenuOverlay(BuildContext context, Rect anchor) {
    final query = _searchController.text.trim().toLowerCase();
    final tags = widget.tags
        .where((tag) => query.isEmpty || tag.name.toLowerCase().contains(query))
        .toList(growable: false);
    final rowCount = tags.length + 1;
    final dividerHeight = tags.isNotEmpty
        ? nautermContextMenuDividerHeight
        : 0.0;
    final contentHeight =
        50.0 + rowCount * nautermContextMenuRowHeight + dividerHeight;
    return NautermAnchoredDropdownOverlay(
      anchor: anchor,
      width: 210,
      contentHeight: contentHeight,
      maxHeight: 340,
      onDismissed: _closeMenu,
      contentBuilder: (context, menuHeight) => Material(
        color: Colors.transparent,
        child: NautermDropdownSurface(
          child: SizedBox(
            width: 210,
            height: menuHeight,
            child: Padding(
              padding: const EdgeInsets.all(6),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  SizedBox(
                    height: nautermContextMenuRowHeight,
                    child: TextField(
                      controller: _searchController,
                      autofocus: true,
                      textAlignVertical: TextAlignVertical.center,
                      style: TextStyle(
                        color: _text,
                        fontSize: 12,
                        fontWeight: FontWeight.w500,
                        height: 1,
                        letterSpacing: 0,
                      ),
                      decoration: InputDecoration(
                        hintText: tr(
                          'workspace.label.searchTags',
                          fallback: 'Search tags',
                        ),
                        hintStyle: TextStyle(
                          color: _mutedText,
                          fontSize: 12,
                          fontWeight: FontWeight.w500,
                          height: 1,
                          letterSpacing: 0,
                        ),
                        prefixIcon: Icon(
                          LucideIcons.search,
                          size: 16,
                          color: _mutedText,
                        ),
                        prefixIconConstraints: const BoxConstraints.tightFor(
                          width: 36,
                          height: nautermContextMenuRowHeight,
                        ),
                        filled: true,
                        fillColor: _workspaceMenuHover,
                        isDense: true,
                        contentPadding: const EdgeInsets.only(right: 10),
                        constraints: const BoxConstraints.tightFor(
                          height: nautermContextMenuRowHeight,
                        ),
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(7),
                          borderSide: BorderSide.none,
                        ),
                      ),
                      onChanged: _handleSearchChanged,
                    ),
                  ),
                  const SizedBox(height: 6),
                  Expanded(
                    child: ListView(
                      controller: _tagListController,
                      padding: EdgeInsets.zero,
                      children: [
                        for (final tag in tags)
                          _HostTagMenuRow(
                            tag: tag,
                            selected: widget.selectedTagUuids.contains(
                              tag.uuid,
                            ),
                            onSelected: () {
                              final uuid = tag.uuid;
                              if (uuid == null) return;
                              final selected = {...widget.selectedTagUuids};
                              if (!selected.add(uuid)) {
                                selected.remove(uuid);
                              }
                              widget.onSelectionChanged(selected);
                              _menu?.markNeedsBuild();
                            },
                            onRename: widget.onSave == null
                                ? null
                                : () {
                                    _closeMenu();
                                    _renameTag(tag);
                                  },
                            onDelete: widget.onDelete == null
                                ? null
                                : () {
                                    final uuid = tag.uuid;
                                    if (uuid != null &&
                                        widget.selectedTagUuids.contains(
                                          uuid,
                                        )) {
                                      widget.onSelectionChanged({
                                        for (final value
                                            in widget.selectedTagUuids)
                                          if (value != uuid) value,
                                      });
                                    }
                                    widget.onDelete!(tag);
                                  },
                          ),
                        if (tags.isNotEmpty) const _HostTagDivider(),
                        _HostTagActionRow(
                          icon: LucideIcons.x,
                          label: tr(
                            'workspace.label.clearSelection',
                            fallback: 'Clear selection',
                          ),
                          enabled: widget.selectedTagUuids.isNotEmpty,
                          onTap: () {
                            widget.onSelectionChanged(const {});
                            _menu?.markNeedsBuild();
                          },
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _renameTag(TagEntry tag) async {
    final controller = TextEditingController(text: tag.name);
    final name = await showNautermDialog<String>(
      context: context,
      builder: (context) => _WorkspaceDialogFrame(
        width: 360,
        title: Text(tr('workspace.label.renameTag', fallback: 'Rename tag')),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            _WorkspaceInput(
              controller: controller,
              label: 'Name',
              autofocus: true,
            ),
          ],
        ),
        actions: [
          _WorkspaceButton(
            label: 'Cancel',
            variant: _WorkspaceButtonVariant.text,
            onPressed: () => Navigator.pop(context),
          ),
          _WorkspaceButton(
            label: 'Save',
            type: _WorkspaceButtonType.info,
            variant: _WorkspaceButtonVariant.solid,
            onPressed: () => Navigator.pop(context, controller.text.trim()),
          ),
        ],
      ),
    );
    controller.dispose();
    if (name == null || name.isEmpty || name == tag.name) {
      return;
    }
    widget.onSave!(
      TagEntry(
        id: tag.id,
        uuid: tag.uuid,
        name: name,
        createdAt: tag.createdAt,
        updatedAt: tag.updatedAt,
        version: tag.version,
        createdDeviceId: tag.createdDeviceId,
        updatedDeviceId: tag.updatedDeviceId,
      ),
    );
  }
}

class _HostTagMenuRow extends StatefulWidget {
  const _HostTagMenuRow({
    required this.tag,
    required this.selected,
    required this.onSelected,
    this.onRename,
    this.onDelete,
  });

  final TagEntry tag;
  final bool selected;
  final VoidCallback onSelected;
  final VoidCallback? onRename;
  final VoidCallback? onDelete;

  @override
  State<_HostTagMenuRow> createState() => _HostTagMenuRowState();
}

class _HostTagMenuRowState extends State<_HostTagMenuRow> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    return NautermDropdownRow(
      onTap: widget.onSelected,
      onHoverChanged: (value) {
        if (mounted) setState(() => _hovered = value);
      },
      child: Row(
        children: [
          Icon(
            widget.selected ? LucideIcons.circleCheck : LucideIcons.circle,
            size: 16,
            color: widget.selected ? _blue : _mutedText,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              widget.tag.name,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: _text,
                fontSize: 12,
                fontWeight: FontWeight.w500,
                height: 1,
                letterSpacing: 0,
                decoration: TextDecoration.none,
              ),
            ),
          ),
          if (widget.onRename != null || widget.onDelete != null)
            SizedBox(
              width: 42,
              child: IgnorePointer(
                ignoring: !_hovered,
                child: AnimatedOpacity(
                  key: ValueKey('host-tag-row-actions:${widget.tag.name}'),
                  opacity: _hovered ? 1 : 0,
                  duration: const Duration(milliseconds: 80),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      if (widget.onRename != null)
                        _HostTagRowIconButton(
                          tooltip: tr(
                            'workspace.label.renameTag',
                            fallback: 'Rename tag',
                          ),
                          icon: LucideIcons.pencil,
                          onTap: widget.onRename!,
                        ),
                      if (widget.onDelete != null)
                        _HostTagRowIconButton(
                          tooltip: tr(
                            'workspace.label.deleteTag',
                            fallback: 'Delete tag',
                          ),
                          icon: LucideIcons.trash2,
                          onTap: widget.onDelete!,
                        ),
                    ],
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _HostTagActionRow extends StatelessWidget {
  const _HostTagActionRow({
    required this.label,
    required this.onTap,
    this.icon,
    this.enabled = true,
  });

  final String label;
  final VoidCallback onTap;
  final IconData? icon;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    return NautermDropdownRow(
      enabled: enabled,
      onTap: onTap,
      child: Row(
        children: [
          if (icon != null) ...[
            Icon(
              icon,
              size: 16,
              color: enabled ? _mutedText : _workspaceMenuDisabledText,
            ),
            const SizedBox(width: 10),
          ],
          Expanded(
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: enabled ? _text : _workspaceMenuDisabledText,
                fontSize: 12,
                fontWeight: FontWeight.w500,
                height: 1,
                letterSpacing: 0,
                decoration: TextDecoration.none,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _HostTagDivider extends StatelessWidget {
  const _HostTagDivider();

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: nautermContextMenuDividerHeight,
      child: Center(
        child: Container(
          height: 1,
          margin: const EdgeInsets.symmetric(horizontal: 10),
          color: context.nautermPalette.softOutline,
        ),
      ),
    );
  }
}

class _HostTagRowIconButton extends StatelessWidget {
  const _HostTagRowIconButton({
    required this.tooltip,
    required this.icon,
    required this.onTap,
  });

  final String tooltip;
  final IconData icon;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tr(tooltip),
      child: InkWell(
        borderRadius: BorderRadius.circular(5),
        onTap: onTap,
        child: SizedBox(
          width: 20,
          height: 20,
          child: Icon(icon, size: 13, color: _mutedText),
        ),
      ),
    );
  }
}

typedef _WorkspaceDropdownTriggerBuilder = Widget Function(VoidCallback open);
typedef _WorkspaceCustomDropdownOpen =
    void Function(BuildContext context, Rect anchor);

/// Shared trigger, anchoring, and selection flow for workspace dropdowns.
class _WorkspaceDropdown<T> extends StatelessWidget {
  const _WorkspaceDropdown({
    required this.width,
    required this.triggerBuilder,
    this.entries = const [],
    this.onSelected,
    this.onOpen,
  }) : assert(entries.length > 0 || onOpen != null);

  final double width;
  final _WorkspaceDropdownTriggerBuilder triggerBuilder;
  final List<NautermContextMenuEntry<T>> entries;
  final ValueChanged<T>? onSelected;
  final _WorkspaceCustomDropdownOpen? onOpen;

  @override
  Widget build(BuildContext context) {
    return Builder(
      builder: (anchorContext) => triggerBuilder(() => _open(anchorContext)),
    );
  }

  Future<void> _open(BuildContext context) async {
    final renderObject = context.findRenderObject();
    final overlay = Overlay.of(context).context.findRenderObject();
    if (renderObject is! RenderBox || overlay is! RenderBox) {
      return;
    }
    final anchor =
        renderObject.localToGlobal(Offset.zero, ancestor: overlay) &
        renderObject.size;
    final customOpen = onOpen;
    if (customOpen != null) {
      customOpen(context, anchor);
      return;
    }
    final selected = await showNautermDropdownMenu<T>(
      context: context,
      anchor: anchor,
      width: width,
      entries: entries,
    );
    if (selected != null) {
      onSelected?.call(selected);
    }
  }
}

class _WorkspaceToolbarFrame extends StatelessWidget {
  const _WorkspaceToolbarFrame({required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return ExcludeFocus(
      key: const ValueKey('workspace-toolbar-tab-exclusion'),
      child: Container(
        height: _workspaceToolbarHeight,
        decoration: BoxDecoration(
          color: _card,
          border: Border(bottom: BorderSide(color: _sidebarDivider)),
        ),
        padding: const EdgeInsets.symmetric(horizontal: 10),
        child: Row(children: children),
      ),
    );
  }
}

class _ToolbarSplitButton extends StatelessWidget {
  const _ToolbarSplitButton({
    required this.icon,
    required this.label,
    required this.onPrimaryPressed,
    required this.onSecondaryPressed,
  });

  final IconData icon;
  final String label;
  final VoidCallback onPrimaryPressed;
  final ValueChanged<BuildContext>? onSecondaryPressed;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        _ToolbarPrimaryButton(
          icon: icon,
          label: label,
          shape: const RoundedRectangleBorder(
            borderRadius: BorderRadius.horizontal(left: Radius.circular(8)),
          ),
          onPressed: onPrimaryPressed,
        ),
        SizedBox(width: 2),
        Tooltip(
          message: tr('$label actions'),
          child: Builder(
            builder: (buttonContext) => _WorkspaceButton(
              icon: Icons.keyboard_arrow_down_rounded,
              size: _WorkspaceControlSize.tiny,
              variant: _WorkspaceButtonVariant.filled,
              height: _workspaceToolbarControlExtent,
              shape: const RoundedRectangleBorder(
                borderRadius: BorderRadius.horizontal(
                  right: Radius.circular(8),
                ),
              ),
              onPressed: onSecondaryPressed == null
                  ? null
                  : () => onSecondaryPressed!(buttonContext),
            ),
          ),
        ),
      ],
    );
  }
}

class _ToolbarPrimaryButton extends StatelessWidget {
  const _ToolbarPrimaryButton({
    required this.icon,
    required this.label,
    required this.onPressed,
    this.shape = const RoundedRectangleBorder(
      borderRadius: BorderRadius.all(Radius.circular(8)),
    ),
  });

  final IconData icon;
  final String label;
  final VoidCallback? onPressed;
  final OutlinedBorder shape;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceButton(
      icon: icon,
      label: label,
      size: _WorkspaceControlSize.tiny,
      variant: _WorkspaceButtonVariant.filled,
      height: _workspaceToolbarControlExtent,
      horizontalPadding: 10,
      shape: shape,
      onPressed: onPressed,
    );
  }
}

List<Widget> _defaultToolbarTrailingActions({
  required _WorkspaceSortOrder sortOrder,
  required ValueChanged<_WorkspaceSortOrder> onSortOrderChanged,
  String? searchQuery,
  ValueChanged<String>? onSearchQueryChanged,
  _WorkspaceViewMode? viewMode,
  ValueChanged<_WorkspaceViewMode>? onViewModeChanged,
}) {
  return [
    _WorkspaceSortDropdownButton(
      value: sortOrder,
      onChanged: onSortOrderChanged,
    ),
    SizedBox(width: 6),
    if (searchQuery != null && onSearchQueryChanged != null)
      _WorkspaceToolbarSearch(
        query: searchQuery,
        onChanged: onSearchQueryChanged,
      ),
    if (viewMode != null && onViewModeChanged != null)
      _WorkspaceViewModeButton(mode: viewMode, onChanged: onViewModeChanged),
  ];
}

class _ToolbarTrailingActions extends StatelessWidget {
  const _ToolbarTrailingActions({required this.children});

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.centerRight,
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        reverse: true,
        child: Row(mainAxisSize: MainAxisSize.min, children: children),
      ),
    );
  }
}

class _ModeButton extends StatelessWidget {
  const _ModeButton({required this.icon, required this.label, this.onTap});

  final IconData icon;
  final String label;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceButton(
      icon: icon,
      label: label,
      size: _WorkspaceControlSize.tiny,
      variant: _WorkspaceButtonVariant.text,
      height: _workspaceToolbarControlExtent,
      onPressed: onTap,
      horizontalPadding: 5,
    );
  }
}

class _ResponsiveModeButton extends StatelessWidget {
  const _ResponsiveModeButton({
    required this.icon,
    required this.label,
    this.onTap,
  });

  final IconData icon;
  final String label;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return Flexible(
      child: LayoutBuilder(
        builder: (context, constraints) {
          if (constraints.maxWidth < 10) {
            return const SizedBox.shrink();
          }
          if (constraints.maxWidth < 70) {
            return Tooltip(
              message: tr(label),
              child: _SquareIconButton(icon: icon, onTap: onTap),
            );
          }
          return _ModeButton(icon: icon, label: label, onTap: onTap);
        },
      ),
    );
  }
}

class _SquareIconButton extends StatelessWidget {
  const _SquareIconButton({required this.icon, required this.onTap});

  final IconData icon;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceButton(
      icon: icon,
      size: _WorkspaceControlSize.tiny,
      variant: _WorkspaceButtonVariant.text,
      onPressed: onTap,
      width: _workspaceToolbarControlExtent,
      height: _workspaceToolbarControlExtent,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
    );
  }
}

class _WorkspaceToolbarSearch extends StatefulWidget {
  const _WorkspaceToolbarSearch({required this.query, required this.onChanged});

  final String query;
  final ValueChanged<String> onChanged;

  @override
  State<_WorkspaceToolbarSearch> createState() =>
      _WorkspaceToolbarSearchState();
}

class _WorkspaceToolbarSearchState extends State<_WorkspaceToolbarSearch> {
  late final TextEditingController _controller;
  late final FocusNode _focusNode;
  bool _open = false;

  @override
  void initState() {
    super.initState();
    _open = widget.query.trim().isNotEmpty;
    _controller = TextEditingController(text: widget.query);
    _focusNode = FocusNode()..addListener(_handleFocusChanged);
  }

  @override
  void didUpdateWidget(covariant _WorkspaceToolbarSearch oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.query != _controller.text) {
      _controller.text = widget.query;
      _controller.selection = TextSelection.collapsed(
        offset: _controller.text.length,
      );
    }
    if (widget.query.trim().isNotEmpty && !_open) {
      setState(() => _open = true);
    }
  }

  @override
  void dispose() {
    _focusNode.removeListener(_handleFocusChanged);
    _focusNode.dispose();
    _controller.dispose();
    super.dispose();
  }

  void _handleFocusChanged() {
    if (_focusNode.hasFocus || !_open) {
      return;
    }
    if (_controller.text.trim().isNotEmpty) {
      return;
    }
    setState(() => _open = false);
  }

  @override
  Widget build(BuildContext context) {
    if (!_open) {
      return Tooltip(
        message: tr('common.action.search', fallback: 'Search'),
        child: _SquareIconButton(
          icon: widget.query.trim().isEmpty
              ? Icons.search_rounded
              : Icons.manage_search_rounded,
          onTap: () {
            setState(() => _open = true);
            WidgetsBinding.instance.addPostFrameCallback((_) {
              if (mounted) {
                _focusNode.requestFocus();
              }
            });
          },
        ),
      );
    }

    return SizedBox(
      width: 180,
      height: _workspaceToolbarControlExtent,
      child: TextField(
        controller: _controller,
        focusNode: _focusNode,
        autofocus: true,
        onChanged: widget.onChanged,
        textInputAction: TextInputAction.search,
        style: TextStyle(
          color: _text,
          fontSize: NautermFontSizes.labelMedium,
          fontWeight: NautermFontWeights.medium,
          letterSpacing: 0,
        ),
        decoration: InputDecoration(
          isDense: true,
          hintText: tr('common.action.search', fallback: 'Search'),
          hintStyle: TextStyle(
            color: _mutedText,
            fontSize: NautermFontSizes.labelMedium,
            fontWeight: NautermFontWeights.medium,
            letterSpacing: 0,
          ),
          prefixIcon: Icon(Icons.search_rounded, size: 16, color: _text),
          prefixIconConstraints: const BoxConstraints(
            minWidth: 28,
            minHeight: 28,
          ),
          suffixIcon: IconButton(
            icon: Icon(Icons.close_rounded, size: 15),
            color: _mutedText,
            padding: EdgeInsets.zero,
            style: _workspaceIconButtonInteractionStyle,
            constraints: const BoxConstraints(minWidth: 26, minHeight: 26),
            onPressed: () {
              widget.onChanged('');
              setState(() => _open = false);
            },
          ),
          filled: true,
          fillColor: _sidebarHover,
          contentPadding: const EdgeInsets.symmetric(
            horizontal: 8,
            vertical: 8,
          ),
          border: OutlineInputBorder(
            borderRadius: BorderRadius.circular(7),
            borderSide: BorderSide(color: _sidebarDivider),
          ),
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(7),
            borderSide: BorderSide(color: _sidebarDivider),
          ),
          focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(7),
            borderSide: BorderSide(color: _blue.withValues(alpha: 0.52)),
          ),
        ),
      ),
    );
  }
}

class _WorkspaceViewModeButton extends StatelessWidget {
  const _WorkspaceViewModeButton({required this.mode, required this.onChanged});

  final _WorkspaceViewMode mode;
  final ValueChanged<_WorkspaceViewMode> onChanged;

  @override
  Widget build(BuildContext context) {
    final nextMode = mode == _WorkspaceViewMode.grid
        ? _WorkspaceViewMode.list
        : _WorkspaceViewMode.grid;
    final icon = mode == _WorkspaceViewMode.grid
        ? Icons.grid_view_rounded
        : Icons.view_list_rounded;
    final tooltip = mode == _WorkspaceViewMode.grid
        ? tr('workspace.tooltip.gridView', fallback: 'Grid view')
        : tr('workspace.tooltip.listView', fallback: 'List view');
    return Tooltip(
      message: tooltip,
      child: _SquareIconButton(icon: icon, onTap: () => onChanged(nextMode)),
    );
  }
}

class _SectionTitle extends StatelessWidget {
  const _SectionTitle(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(
      context.tr(text),
      key: ValueKey('workspace-section-title:$text'),
      style: TextStyle(
        color: _text,
        fontSize: NautermFontSizes.labelLarge,
        fontWeight: NautermFontWeights.semibold,
        letterSpacing: 0,
      ),
    );
  }
}
