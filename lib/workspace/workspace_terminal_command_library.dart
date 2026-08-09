part of 'nauterm_workspace.dart';

class _TerminalCommandLibrary<T> extends StatefulWidget {
  const _TerminalCommandLibrary({
    super.key,
    required this.colors,
    required this.title,
    required this.searchHint,
    required this.emptyLabel,
    required this.items,
    required this.matches,
    required this.titleFor,
    required this.commandFor,
    required this.metadataFor,
    required this.runTooltip,
    required this.copyTooltip,
    required this.onCopy,
    required this.onRun,
    this.sortItems,
    this.onCreate,
  });

  final _AiAssistantColors colors;
  final String title;
  final String searchHint;
  final String emptyLabel;
  final List<T> items;
  final bool Function(T item, String query) matches;
  final String Function(T item) titleFor;
  final String Function(T item) commandFor;
  final String Function(T item) metadataFor;
  final String runTooltip;
  final String copyTooltip;
  final ValueChanged<T> onCopy;
  final ValueChanged<T> onRun;
  final List<T> Function(Iterable<T> items, _WorkspaceSortOrder order)?
  sortItems;
  final VoidCallback? onCreate;

  @override
  State<_TerminalCommandLibrary<T>> createState() =>
      _TerminalCommandLibraryState<T>();
}

class _TerminalCommandLibraryState<T>
    extends State<_TerminalCommandLibrary<T>> {
  final TextEditingController _searchController = TextEditingController();
  final GlobalKey _sortButtonKey = GlobalKey();
  _WorkspaceSortOrder _sortOrder = _WorkspaceSortOrder.nameAscending;

  @override
  void initState() {
    super.initState();
    _searchController.addListener(_handleSearchChanged);
  }

  @override
  void dispose() {
    _searchController.removeListener(_handleSearchChanged);
    _searchController.dispose();
    super.dispose();
  }

  void _handleSearchChanged() {
    setState(() {});
  }

  Future<void> _showSortMenu() async {
    final buttonBox = _sortButtonKey.currentContext?.findRenderObject();
    final overlayBox = Overlay.of(context).context.findRenderObject();
    if (buttonBox is! RenderBox || overlayBox is! RenderBox) {
      return;
    }
    final anchor =
        buttonBox.localToGlobal(Offset.zero, ancestor: overlayBox) &
        buttonBox.size;
    final selected = await showNautermDropdownMenu<_WorkspaceSortOrder>(
      context: context,
      anchor: anchor,
      width: 190,
      style: _terminalSftpMenuStyle(widget.colors),
      entries: [
        for (final option in _WorkspaceSortOrder.values)
          NautermContextMenuAction<_WorkspaceSortOrder>(
            value: option,
            label: option.localizedLabel,
            icon: option.icon,
            selected: option == _sortOrder,
          ),
      ],
    );
    if (selected != null && selected != _sortOrder && mounted) {
      setState(() => _sortOrder = selected);
    }
  }

  @override
  Widget build(BuildContext context) {
    final colors = widget.colors;
    final query = _searchController.text.trim().toLowerCase();
    final filteredItems = query.isEmpty
        ? widget.items
        : widget.items.where((item) => widget.matches(item, query));
    final items =
        widget.sortItems?.call(filteredItems, _sortOrder) ??
        filteredItems.toList(growable: false);
    return Expanded(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 12, 12, 10),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        widget.title,
                        style: TextStyle(
                          color: colors.foreground,
                          fontSize: 13,
                          fontWeight: NautermFontWeights.semibold,
                          letterSpacing: 0,
                        ),
                      ),
                    ),
                    if (widget.onCreate != null)
                      _WorkspaceDialogThemeScope(
                        colors: colors,
                        child: _WorkspaceButton(
                          icon: LucideIcons.plus,
                          label: 'New snippet',
                          size: _WorkspaceControlSize.tiny,
                          variant: _WorkspaceButtonVariant.filled,
                          height: 28,
                          horizontalPadding: 8,
                          iconGap: 5,
                          onPressed: widget.onCreate,
                        ),
                      ),
                    if (widget.onCreate != null && widget.sortItems != null)
                      const SizedBox(width: 2),
                    if (widget.sortItems != null)
                      IconButton(
                        key: _sortButtonKey,
                        tooltip: tr(
                          'workspace.tooltip.sort',
                          fallback: 'Sort: {order}',
                          args: {'order': _sortOrder.localizedLabel},
                        ),
                        onPressed: _showSortMenu,
                        icon: Icon(_sortOrder.icon, size: 16),
                        color: colors.muted,
                        hoverColor: colors.inputBackground,
                        highlightColor: colors.inputBackground,
                        padding: EdgeInsets.zero,
                        constraints: const BoxConstraints.tightFor(
                          width: 28,
                          height: 28,
                        ),
                      ),
                  ],
                ),
                const SizedBox(height: 10),
                _TerminalToolSearchField(
                  controller: _searchController,
                  hintText: widget.searchHint,
                  colors: colors,
                ),
              ],
            ),
          ),
          Divider(height: 1, color: colors.border),
          Expanded(
            child: ClipRect(
              child: ColoredBox(
                color: colors.background,
                child: items.isEmpty
                    ? Center(
                        child: Text(
                          query.isEmpty
                              ? widget.emptyLabel
                              : tr(
                                  'workspace.terminalTools.empty.noMatches',
                                  fallback: 'No matching results.',
                                ),
                          style: TextStyle(
                            color: colors.muted,
                            fontSize: 12,
                            fontWeight: NautermFontWeights.medium,
                            letterSpacing: 0,
                          ),
                        ),
                      )
                    : ListView.separated(
                        padding: const EdgeInsets.fromLTRB(8, 8, 8, 14),
                        itemCount: items.length,
                        separatorBuilder: (_, _) => const SizedBox(height: 4),
                        itemBuilder: (context, index) {
                          final item = items[index];
                          return _TerminalCommandLibraryRow(
                            colors: colors,
                            title: widget.titleFor(item),
                            command: widget.commandFor(item),
                            metadata: widget.metadataFor(item),
                            runTooltip: widget.runTooltip,
                            copyTooltip: widget.copyTooltip,
                            onCopy: () => widget.onCopy(item),
                            onRun: () => widget.onRun(item),
                          );
                        },
                      ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _TerminalSnippetEditor extends StatefulWidget {
  const _TerminalSnippetEditor({
    required this.colors,
    required this.packages,
    required this.targetLabel,
    required this.targetHostId,
    required this.onCreatePackage,
    required this.onCancel,
    required this.onSave,
  });

  final _AiAssistantColors colors;
  final List<_SnippetPackageItem> packages;
  final String targetLabel;
  final int? targetHostId;
  final _CreateRelatedEntry? onCreatePackage;
  final VoidCallback onCancel;
  final Future<void> Function(_SnippetDraft draft) onSave;

  @override
  State<_TerminalSnippetEditor> createState() => _TerminalSnippetEditorState();
}

class _TerminalSnippetEditorState extends State<_TerminalSnippetEditor> {
  final TextEditingController _descriptionController = TextEditingController();
  final TextEditingController _scriptController = TextEditingController();
  int? _packageId;
  bool _saving = false;
  String? _error;

  @override
  void dispose() {
    _descriptionController.dispose();
    _scriptController.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final script = _scriptController.text.trim();
    if (script.isEmpty) {
      setState(() => _error = 'Script is required.');
      return;
    }
    final description = _descriptionController.text.trim();
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await widget.onSave(
        _SnippetDraft(
          packageId: _packageId,
          scope: widget.targetHostId == null
              ? SnippetScope.global
              : SnippetScope.targeted,
          description: description.isEmpty ? 'Untitled snippet' : description,
          script: script,
          targetGroupIds: const [],
          targetHostIds: widget.targetHostId == null
              ? const []
              : [widget.targetHostId!],
        ),
      );
    } catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _saving = false;
        _error = 'Failed to save snippet: $error';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final colors = widget.colors;
    return Expanded(
      child: ColoredBox(
        color: colors.background,
        child: _WorkspaceDialogThemeScope(
          colors: colors,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Container(
                height: 48,
                padding: const EdgeInsets.symmetric(horizontal: 8),
                decoration: BoxDecoration(
                  border: Border(bottom: BorderSide(color: colors.border)),
                ),
                child: Row(
                  children: [
                    IconButton(
                      tooltip: tr(
                        'workspace.label.backToSnippets',
                        fallback: 'Back to snippets',
                      ),
                      onPressed: _saving ? null : widget.onCancel,
                      icon: const Icon(LucideIcons.arrowLeft, size: 16),
                      color: colors.foreground,
                      disabledColor: colors.muted,
                      hoverColor: colors.inputBackground,
                      padding: EdgeInsets.zero,
                      constraints: const BoxConstraints.tightFor(
                        width: 30,
                        height: 30,
                      ),
                    ),
                    const SizedBox(width: 4),
                    Expanded(
                      child: Text(
                        tr(
                          'workspace.label.newSnippet',
                          fallback: 'New snippet',
                        ),
                        style: TextStyle(
                          color: colors.foreground,
                          fontSize: 13,
                          fontWeight: NautermFontWeights.semibold,
                          letterSpacing: 0,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              Expanded(
                child: IgnorePointer(
                  ignoring: _saving,
                  child: ListView(
                    padding: const EdgeInsets.fromLTRB(12, 14, 12, 18),
                    children: [
                      _WorkspaceSelect<int?>(
                        label: 'Package',
                        value: _packageId,
                        size: _WorkspaceControlSize.medium,
                        editable: true,
                        searchable: true,
                        clearable: true,
                        colors: colors,
                        createLabel: widget.onCreatePackage == null
                            ? null
                            : 'Create package',
                        onCreate: widget.onCreatePackage == null
                            ? null
                            : (name) {
                                widget.onCreatePackage!(name, (id) {
                                  if (mounted) {
                                    setState(() => _packageId = id);
                                  }
                                });
                              },
                        createConflictLabels: [
                          for (final package in widget.packages) package.name,
                        ],
                        items: [
                          for (final package in widget.packages)
                            DropdownMenuItem<int?>(
                              value: package.id,
                              child: Text(package.name),
                            ),
                        ],
                        onChanged: (value) =>
                            setState(() => _packageId = value),
                      ),
                      const SizedBox(height: 12),
                      _WorkspaceInput(
                        controller: _descriptionController,
                        label: 'Description',
                        size: _WorkspaceControlSize.medium,
                      ),
                      const SizedBox(height: 12),
                      _WorkspaceInput(
                        controller: _scriptController,
                        label: 'Script',
                        isRequired: true,
                        size: _WorkspaceControlSize.medium,
                        autofocus: true,
                        minLines: 6,
                        maxLines: 10,
                      ),
                      if (widget.targetHostId != null) ...[
                        const SizedBox(height: 12),
                        Row(
                          children: [
                            Icon(
                              LucideIcons.server,
                              size: 14,
                              color: colors.muted,
                            ),
                            const SizedBox(width: 7),
                            Expanded(
                              child: Text(
                                tr('Available on ${widget.targetLabel}'),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: TextStyle(
                                  color: colors.muted,
                                  fontSize: 11.5,
                                  fontWeight: NautermFontWeights.medium,
                                  letterSpacing: 0,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ],
                      if (_error != null) ...[
                        const SizedBox(height: 12),
                        Text(
                          _error!,
                          style: const TextStyle(
                            color: Color(0xffef4444),
                            fontSize: 11.5,
                            letterSpacing: 0,
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
              Container(
                padding: const EdgeInsets.fromLTRB(12, 10, 12, 12),
                decoration: BoxDecoration(
                  border: Border(top: BorderSide(color: colors.border)),
                ),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    _WorkspaceButton(
                      label: 'Cancel',
                      size: _WorkspaceControlSize.medium,
                      variant: _WorkspaceButtonVariant.outlined,
                      height: 32,
                      onPressed: _saving ? null : widget.onCancel,
                    ),
                    const SizedBox(width: 8),
                    _WorkspaceButton(
                      label: _saving ? 'Saving…' : 'Save',
                      size: _WorkspaceControlSize.medium,
                      type: _WorkspaceButtonType.primary,
                      variant: _WorkspaceButtonVariant.solid,
                      height: 32,
                      onPressed: _saving ? null : _save,
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _TerminalToolSearchField extends StatelessWidget {
  const _TerminalToolSearchField({
    required this.controller,
    required this.hintText,
    required this.colors,
  });

  final TextEditingController controller;
  final String hintText;
  final _AiAssistantColors colors;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 34,
      child: TextField(
        controller: controller,
        cursorColor: colors.accent,
        style: TextStyle(
          color: colors.foreground,
          fontSize: 12,
          fontWeight: NautermFontWeights.medium,
          letterSpacing: 0,
        ),
        decoration: InputDecoration(
          isDense: true,
          hintText: hintText,
          hintStyle: TextStyle(
            color: colors.muted,
            fontSize: 12,
            letterSpacing: 0,
          ),
          prefixIcon: Icon(LucideIcons.search, size: 16, color: colors.muted),
          prefixIconConstraints: const BoxConstraints.tightFor(
            width: 32,
            height: 32,
          ),
          suffixIcon: controller.text.isEmpty
              ? null
              : IconButton(
                  tooltip: tr(
                    'workspace.label.clearSearch',
                    fallback: 'Clear search',
                  ),
                  onPressed: controller.clear,
                  icon: const Icon(LucideIcons.x, size: 14),
                  color: colors.muted,
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints.tightFor(
                    width: 30,
                    height: 30,
                  ),
                ),
          filled: true,
          fillColor: colors.inputBackground,
          contentPadding: const EdgeInsets.only(right: 8),
          enabledBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(8),
            borderSide: BorderSide(color: colors.border),
          ),
          focusedBorder: OutlineInputBorder(
            borderRadius: BorderRadius.circular(8),
            borderSide: BorderSide(color: colors.accent),
          ),
        ),
      ),
    );
  }
}

class _TerminalCommandLibraryRow extends StatelessWidget {
  const _TerminalCommandLibraryRow({
    required this.colors,
    required this.title,
    required this.command,
    required this.metadata,
    required this.runTooltip,
    required this.copyTooltip,
    required this.onCopy,
    required this.onRun,
  });

  final _AiAssistantColors colors;
  final String title;
  final String command;
  final String metadata;
  final String runTooltip;
  final String copyTooltip;
  final VoidCallback onCopy;
  final VoidCallback onRun;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: colors.inputBackground.withValues(alpha: 0.52),
      borderRadius: BorderRadius.circular(8),
      child: InkWell(
        borderRadius: BorderRadius.circular(8),
        hoverColor: colors.inputBackground,
        onDoubleTap: onRun,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(10, 8, 5, 8),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      title,
                      maxLines: command.isEmpty ? 2 : 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: colors.foreground,
                        fontSize: 12,
                        fontWeight: NautermFontWeights.medium,
                        height: 1.35,
                        letterSpacing: 0,
                        fontFamily: command.isEmpty ? 'monospace' : null,
                      ),
                    ),
                    if (command.isNotEmpty) ...[
                      const SizedBox(height: 3),
                      Text(
                        command,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: colors.muted,
                          fontSize: 11,
                          height: 1.35,
                          letterSpacing: 0,
                          fontFamily: 'monospace',
                        ),
                      ),
                    ],
                    if (metadata.isNotEmpty) ...[
                      const SizedBox(height: 5),
                      Text(
                        metadata,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: colors.muted,
                          fontSize: 10,
                          fontWeight: NautermFontWeights.medium,
                          letterSpacing: 0,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: 6),
              IconButton(
                tooltip: copyTooltip,
                onPressed: onCopy,
                icon: const Icon(LucideIcons.copy, size: 13),
                color: colors.muted,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(
                  width: 25,
                  height: 25,
                ),
                style: IconButton.styleFrom(
                  hoverColor: colors.inputBackground,
                  highlightColor: colors.accent.withValues(alpha: 0.12),
                ),
              ),
              const SizedBox(width: 1),
              IconButton(
                tooltip: runTooltip,
                onPressed: onRun,
                icon: const Icon(LucideIcons.play, size: 15),
                color: colors.accent,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(
                  width: 26,
                  height: 26,
                ),
                style: IconButton.styleFrom(
                  hoverColor: colors.accent.withValues(alpha: 0.12),
                  highlightColor: colors.accent.withValues(alpha: 0.18),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

String _shellHistoryMetadata(ShellHistoryEntry entry) {
  final parts = <String>[
    if (entry.cwd?.trim().isNotEmpty == true) entry.cwd!.trim(),
    if (entry.createdAt case final createdAt?)
      _compactTerminalDateTime(createdAt),
  ];
  return parts.join(' · ');
}

String _compactTerminalDateTime(DateTime value) {
  final local = value.toLocal();
  String two(int number) => number.toString().padLeft(2, '0');
  return '${local.year}-${two(local.month)}-${two(local.day)} '
      '${two(local.hour)}:${two(local.minute)}';
}

class _TerminalToolPlaceholder extends StatelessWidget {
  const _TerminalToolPlaceholder({required this.mode, required this.colors});

  final _TerminalToolPanelMode mode;
  final _AiAssistantColors colors;

  @override
  Widget build(BuildContext context) {
    final description = switch (mode) {
      _TerminalToolPanelMode.sftp => tr(
        'workspace.terminalTools.mode.sftp.description',
        fallback: 'Browse files for the current SSH session.',
      ),
      _TerminalToolPanelMode.systemInfo => tr(
        'workspace.terminalTools.mode.systemInfo.description',
        fallback: 'Inspect CPU, memory, disk, and host details.',
      ),
      _TerminalToolPanelMode.snippets => tr(
        'workspace.terminalTools.mode.snippets.description',
        fallback: 'Run and manage reusable terminal snippets here.',
      ),
      _TerminalToolPanelMode.shellHistory => tr(
        'workspace.terminalTools.mode.shellHistory.description',
        fallback: 'Search and reuse commands from this terminal session.',
      ),
      _TerminalToolPanelMode.settings => tr(
        'workspace.terminalTools.mode.settings.description',
        fallback: 'Font controls and terminal theme previews will appear here.',
      ),
      _TerminalToolPanelMode.ai => '',
    };
    return Expanded(
      key: ValueKey('terminal-tool-placeholder:${mode.name}'),
      child: Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 42,
                height: 42,
                decoration: BoxDecoration(
                  color: colors.inputBackground,
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(color: colors.border),
                ),
                child: Icon(mode.icon, size: 18, color: colors.accent),
              ),
              SizedBox(height: 12),
              Text(
                mode.label,
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: colors.foreground,
                  fontSize: 13,
                  fontWeight: NautermFontWeights.semibold,
                  letterSpacing: 0,
                ),
              ),
              SizedBox(height: 6),
              Text(
                description,
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: colors.muted,
                  fontSize: 11.5,
                  height: 1.4,
                  letterSpacing: 0,
                ),
              ),
              SizedBox(height: 8),
              Text(
                tr('workspace.label.comingSoon', fallback: 'Coming soon'),
                style: TextStyle(
                  color: colors.muted,
                  fontSize: 10.5,
                  fontWeight: NautermFontWeights.medium,
                  letterSpacing: 0,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
