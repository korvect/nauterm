part of 'nauterm_workspace.dart';

typedef _LoadHostImportSource = Future<HostImportBundle?> Function(
  HostImportSource source,
);

class _HostImportSelection {
  const _HostImportSelection({required this.hosts, required this.keys});

  final List<HostImportCandidate> hosts;
  final List<HostImportKeyCandidate> keys;
}

class _HostImportDialog extends StatefulWidget {
  const _HostImportDialog({required this.onLoadSource});

  final _LoadHostImportSource onLoadSource;

  @override
  State<_HostImportDialog> createState() => _HostImportDialogState();
}

class _HostImportDialogState extends State<_HostImportDialog> {
  final TextEditingController _filterController = TextEditingController();
  HostImportSource? _source;
  HostImportBundle _bundle = const HostImportBundle();
  final Set<int> _selectedHosts = {};
  final Set<int> _selectedKeys = {};
  bool _hostsExpanded = true;
  bool _keysExpanded = true;
  bool _loading = false;
  String? _error;

  int get _selectionCount => _selectedHosts.length + _selectedKeys.length;

  @override
  void initState() {
    super.initState();
    _filterController.addListener(_handleFilterChanged);
  }

  @override
  void dispose() {
    _filterController.removeListener(_handleFilterChanged);
    _filterController.dispose();
    super.dispose();
  }

  void _handleFilterChanged() => setState(() {});

  Future<void> _load(HostImportSource source) async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final bundle = await widget.onLoadSource(source);
      if (!mounted || bundle == null) return;
      setState(() {
        _source = source;
        _bundle = bundle;
        _filterController.clear();
        _selectedHosts
          ..clear()
          ..addAll(List<int>.generate(bundle.hosts.length, (index) => index));
        _selectedKeys
          ..clear()
          ..addAll(List<int>.generate(bundle.keys.length, (index) => index));
        _hostsExpanded = true;
        _keysExpanded = true;
        if (bundle.hosts.isEmpty && bundle.keys.isEmpty) {
          _error = tr(
            'workspace.import.empty',
            fallback: 'No supported hosts or keys were found.',
          );
        }
      });
    } catch (error) {
      if (mounted) setState(() => _error = '$error');
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  void _submit() {
    Navigator.pop(
      context,
      _HostImportSelection(
        hosts: [
          for (var index = 0; index < _bundle.hosts.length; index++)
            if (_selectedHosts.contains(index)) _bundle.hosts[index],
        ],
        keys: [
          for (var index = 0; index < _bundle.keys.length; index++)
            if (_selectedKeys.contains(index)) _bundle.keys[index],
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final source = _source;
    final contentHeight = (MediaQuery.sizeOf(context).height - 178).clamp(
      300.0,
      510.0,
    );
    return _WorkspaceDialogFrame(
      width: 760,
      title: Row(
        children: [
          Icon(
            source == HostImportSource.openSsh
                ? LucideIcons.folderKey
                : Icons.file_download_outlined,
            size: 18,
            color: _blue,
          ),
          const SizedBox(width: 9),
          Text(
            source == null
                ? tr('workspace.import.title', fallback: 'Import to vault')
                : tr(
                    'workspace.import.fromTitle',
                    fallback: 'Import from {source}',
                    args: {'source': _hostImportSourceLabel(source)},
                  ),
          ),
        ],
      ),
      content: SizedBox(
        height: contentHeight,
        child: source == null ? _buildSources() : _buildPreview(),
      ),
      actions: [
        if (source != null)
          _WorkspaceButton(
            label: tr('common.action.back', fallback: 'Back'),
            variant: _WorkspaceButtonVariant.text,
            onPressed: _loading
                ? null
                : () => setState(() {
                    _source = null;
                    _bundle = const HostImportBundle();
                    _selectedHosts.clear();
                    _selectedKeys.clear();
                    _error = null;
                  }),
          ),
        _WorkspaceButton(
          label: source == null
              ? tr('common.action.cancel', fallback: 'Cancel')
              : tr('workspace.import.chooseAgain', fallback: 'Choose again'),
          variant: _WorkspaceButtonVariant.text,
          onPressed: _loading
              ? null
              : source == null
              ? () => Navigator.pop(context)
              : () => _load(source),
        ),
        if (source != null)
          _WorkspaceButton(
            label: tr(
              'workspace.import.selectedCount',
              fallback: 'Import {count}',
              args: {'count': _selectionCount},
            ),
            type: _WorkspaceButtonType.info,
            variant: _WorkspaceButtonVariant.solid,
            onPressed: _selectionCount == 0 || _loading ? null : _submit,
          ),
      ],
    );
  }

  Widget _buildSources() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          tr(
            'workspace.import.source.description',
            fallback: 'Choose a source. You can review hosts and keys before anything is saved.',
          ),
          style: TextStyle(
            color: _mutedText,
            fontSize: NautermFontSizes.labelLarge,
          ),
        ),
        const SizedBox(height: 18),
        Expanded(
          child: GridView.count(
            crossAxisCount: 2,
            childAspectRatio: 3.35,
            mainAxisSpacing: 10,
            crossAxisSpacing: 10,
            children: [
              _sourceTile(HostImportSource.csv, Icons.table_chart_rounded),
              _sourceTile(HostImportSource.openSsh, LucideIcons.folderKey),
              _sourceTile(HostImportSource.putty, Icons.computer_rounded),
              _sourceTile(HostImportSource.mobaXterm, Icons.dns_rounded),
              _sourceTile(HostImportSource.secureCrt, Icons.shield_rounded),
            ],
          ),
        ),
        if (_loading) const LinearProgressIndicator(minHeight: 2),
        if (_error != null) _ImportErrorText(_error!),
      ],
    );
  }

  Widget _sourceTile(HostImportSource source, IconData icon) {
    return _ImportSourceTile(
      icon: icon,
      title: _hostImportSourceLabel(source),
      description: _hostImportSourceDescription(source),
      enabled: !_loading,
      onTap: () => _load(source),
    );
  }

  Widget _buildPreview() {
    final query = _filterController.text.trim().toLowerCase();
    final visibleHosts = <int>[
      for (var index = 0; index < _bundle.hosts.length; index++)
        if (_matchesHost(_bundle.hosts[index], query)) index,
    ];
    final visibleKeys = <int>[
      for (var index = 0; index < _bundle.keys.length; index++)
        if (_matchesKey(_bundle.keys[index], query)) index,
    ];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _WorkspaceInput(
          controller: _filterController,
          label: tr('common.label.filter', fallback: 'Filter'),
          floatingLabel: false,
          hintText: tr(
            'workspace.label.filterHostsAndKeys',
            fallback: 'Filter hosts and keys',
          ),
          size: _WorkspaceControlSize.medium,
          trailing: Icon(Icons.search_rounded, size: 17, color: _mutedText),
        ),
        const SizedBox(height: 12),
        Expanded(
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: _surface,
              border: Border.all(color: _sidebarDivider),
              borderRadius: BorderRadius.circular(10),
            ),
            child: ClipRRect(
              borderRadius: BorderRadius.circular(10),
              child: ListView(
                padding: const EdgeInsets.fromLTRB(10, 8, 10, 10),
                children: [
                  _ImportSectionHeader(
                    icon: LucideIcons.server,
                    title: tr('common.label.hosts', fallback: 'Hosts'),
                    count: _bundle.hosts.length,
                    expanded: _hostsExpanded,
                    selection: _sectionSelection(
                      _selectedHosts,
                      _bundle.hosts.length,
                    ),
                    onExpanded: () =>
                        setState(() => _hostsExpanded = !_hostsExpanded),
                    onSelectionChanged: () =>
                        _toggleSection(_selectedHosts, _bundle.hosts.length),
                  ),
                  if (_hostsExpanded)
                    for (final index in visibleHosts)
                      _ImportItemRow(
                        icon: LucideIcons.server,
                        title: _bundle.hosts[index].name,
                        subtitle: _hostSubtitle(_bundle.hosts[index]),
                        selected: _selectedHosts.contains(index),
                        onTap: () => _toggleIndex(_selectedHosts, index),
                      ),
                  if (_hostsExpanded && visibleHosts.isEmpty)
                    _ImportEmptyRow(
                      tr(
                        'common.label.noMatchingHosts',
                        fallback: 'No matching hosts',
                      ),
                    ),
                  const SizedBox(height: 8),
                  _ImportSectionHeader(
                    icon: LucideIcons.keyRound,
                    title: tr('common.label.keys', fallback: 'Keys'),
                    count: _bundle.keys.length,
                    expanded: _keysExpanded,
                    selection: _sectionSelection(
                      _selectedKeys,
                      _bundle.keys.length,
                    ),
                    onExpanded: () =>
                        setState(() => _keysExpanded = !_keysExpanded),
                    onSelectionChanged: () =>
                        _toggleSection(_selectedKeys, _bundle.keys.length),
                  ),
                  if (_keysExpanded)
                    for (final index in visibleKeys)
                      _ImportItemRow(
                        icon: LucideIcons.keyRound,
                        title: _bundle.keys[index].name,
                        subtitle: _bundle.keys[index].detail,
                        selected: _selectedKeys.contains(index),
                        onTap: () => _toggleIndex(_selectedKeys, index),
                      ),
                  if (_keysExpanded && visibleKeys.isEmpty)
                    _ImportEmptyRow(
                      tr(
                        'workspace.import.noMatchingKeys',
                        fallback: 'No matching keys',
                      ),
                    ),
                ],
              ),
            ),
          ),
        ),
        if (_loading) const LinearProgressIndicator(minHeight: 2),
        if (_error != null) _ImportErrorText(_error!),
      ],
    );
  }

  bool _matchesHost(HostImportCandidate host, String query) {
    return query.isEmpty ||
        host.name.toLowerCase().contains(query) ||
        host.host.toLowerCase().contains(query) ||
        (host.username?.toLowerCase().contains(query) ?? false);
  }

  bool _matchesKey(HostImportKeyCandidate key, String query) {
    return query.isEmpty ||
        key.name.toLowerCase().contains(query) ||
        key.detail.toLowerCase().contains(query);
  }

  String _hostSubtitle(HostImportCandidate host) {
    final user = host.username;
    return user == null || user.isEmpty
        ? host.address
        : '$user@${host.address}';
  }

  bool? _sectionSelection(Set<int> selection, int count) {
    if (count == 0 || selection.isEmpty) return false;
    if (selection.length == count) return true;
    return null;
  }

  void _toggleSection(Set<int> selection, int count) {
    setState(() {
      if (selection.length == count) {
        selection.clear();
      } else {
        selection
          ..clear()
          ..addAll(List<int>.generate(count, (index) => index));
      }
    });
  }

  void _toggleIndex(Set<int> selection, int index) {
    setState(() {
      if (!selection.add(index)) selection.remove(index);
    });
  }
}

class _ImportSourceTile extends StatefulWidget {
  const _ImportSourceTile({
    required this.icon,
    required this.title,
    required this.description,
    required this.enabled,
    required this.onTap,
  });

  final IconData icon;
  final String title;
  final String description;
  final bool enabled;
  final VoidCallback onTap;

  @override
  State<_ImportSourceTile> createState() => _ImportSourceTileState();
}

class _ImportSourceTileState extends State<_ImportSourceTile> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: widget.enabled ? SystemMouseCursors.click : MouseCursor.defer,
      onEnter: widget.enabled ? (_) => setState(() => _hovered = true) : null,
      onExit: widget.enabled ? (_) => setState(() => _hovered = false) : null,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: widget.enabled ? widget.onTap : null,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 80),
          padding: const EdgeInsets.symmetric(horizontal: 15, vertical: 12),
          decoration: BoxDecoration(
            color: _hovered ? _sidebarHover : _card,
            border: Border.all(
              color: _hovered ? _blue.withValues(alpha: 0.45) : _sidebarDivider,
            ),
            borderRadius: BorderRadius.circular(9),
          ),
          child: Row(
            children: [
              Container(
                width: 36,
                height: 36,
                decoration: BoxDecoration(
                  color: _blue.withValues(alpha: 0.12),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Icon(widget.icon, size: 19, color: _blue),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      tr(widget.title),
                      style: TextStyle(
                        color: _text,
                        fontSize: NautermFontSizes.labelLarge,
                        fontWeight: NautermFontWeights.semibold,
                      ),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      widget.description,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(color: _mutedText, fontSize: 11),
                    ),
                  ],
                ),
              ),
              Icon(Icons.chevron_right_rounded, size: 18, color: _mutedText),
            ],
          ),
        ),
      ),
    );
  }
}

class _ImportSectionHeader extends StatelessWidget {
  const _ImportSectionHeader({
    required this.icon,
    required this.title,
    required this.count,
    required this.expanded,
    required this.selection,
    required this.onExpanded,
    required this.onSelectionChanged,
  });

  final IconData icon;
  final String title;
  final int count;
  final bool expanded;
  final bool? selection;
  final VoidCallback onExpanded;
  final VoidCallback onSelectionChanged;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 38,
      child: Row(
        children: [
          _WorkspaceButton(
            icon: expanded
                ? Icons.keyboard_arrow_down_rounded
                : Icons.keyboard_arrow_right_rounded,
            size: _WorkspaceControlSize.tiny,
            variant: _WorkspaceButtonVariant.text,
            height: 28,
            width: 28,
            onPressed: onExpanded,
          ),
          Icon(icon, size: 17, color: _text),
          const SizedBox(width: 8),
          Text(
            tr(title),
            style: TextStyle(
              color: _text,
              fontSize: NautermFontSizes.labelLarge,
              fontWeight: NautermFontWeights.semibold,
            ),
          ),
          const Spacer(),
          Text(tr('$count'), style: TextStyle(color: _mutedText, fontSize: 12)),
          const SizedBox(width: 8),
          _ImportSelectionMark(
            value: selection,
            enabled: count > 0,
            onTap: onSelectionChanged,
          ),
        ],
      ),
    );
  }
}

class _ImportItemRow extends StatelessWidget {
  const _ImportItemRow({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.selected,
    required this.onTap,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 5),
      child: Material(
        color: _card,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          hoverColor: _sidebarHover,
          onTap: onTap,
          child: Container(
            height: 52,
            padding: const EdgeInsets.symmetric(horizontal: 11),
            decoration: BoxDecoration(
              border: Border.all(color: _sidebarDivider),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Row(
              children: [
                Container(
                  width: 32,
                  height: 32,
                  decoration: BoxDecoration(
                    color: _blue.withValues(alpha: 0.12),
                    borderRadius: BorderRadius.circular(7),
                  ),
                  child: Icon(icon, size: 17, color: _blue),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        tr(title),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: _text,
                          fontSize: NautermFontSizes.labelLarge,
                          fontWeight: NautermFontWeights.medium,
                        ),
                      ),
                      const SizedBox(height: 2),
                      Text(
                        tr(subtitle),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(color: _mutedText, fontSize: 11),
                      ),
                    ],
                  ),
                ),
                _ImportSelectionMark(value: selected, onTap: onTap),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _ImportSelectionMark extends StatelessWidget {
  const _ImportSelectionMark({
    required this.value,
    required this.onTap,
    this.enabled = true,
  });

  final bool? value;
  final bool enabled;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final active = enabled && value != false;
    return Semantics(
      checked: value == true,
      mixed: value == null,
      enabled: enabled,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTap: enabled ? onTap : null,
        child: SizedBox(
          width: 28,
          height: 28,
          child: Center(
            child: AnimatedContainer(
              duration: const Duration(milliseconds: 90),
              width: 19,
              height: 19,
              decoration: BoxDecoration(
                color: active ? _blue : Colors.transparent,
                shape: BoxShape.circle,
                border: Border.all(
                  color: enabled
                      ? active
                            ? _blue
                            : _mutedText
                      : _workspaceMenuDisabledText,
                ),
              ),
              child: active
                  ? Icon(
                      value == null
                          ? Icons.remove_rounded
                          : Icons.check_rounded,
                      size: 13,
                      color: Colors.white,
                    )
                  : null,
            ),
          ),
        ),
      ),
    );
  }
}

class _ImportEmptyRow extends StatelessWidget {
  const _ImportEmptyRow(this.message);

  final String message;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(38, 8, 8, 12),
      child: Text(
        tr(message),
        style: TextStyle(color: _mutedText, fontSize: 12),
      ),
    );
  }
}

class _ImportErrorText extends StatelessWidget {
  const _ImportErrorText(this.message);

  final String message;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 8),
      child: Text(
        tr(message),
        style: const TextStyle(color: Color(0xffef4444), fontSize: 12),
      ),
    );
  }
}

String _hostImportSourceLabel(HostImportSource source) => switch (source) {
  HostImportSource.csv => tr(
    'workspace.import.source.csv.label',
    fallback: 'CSV file',
  ),
  HostImportSource.openSsh => '~/.ssh',
  HostImportSource.putty => 'PuTTY',
  HostImportSource.mobaXterm => 'MobaXterm',
  HostImportSource.secureCrt => 'SecureCRT',
};

String _hostImportSourceDescription(HostImportSource source) =>
    switch (source) {
      HostImportSource.csv => tr(
        'workspace.import.source.csv.description',
        fallback: 'Hosts, groups, tags and credentials',
      ),
      HostImportSource.openSsh => tr(
        'workspace.import.source.openSsh.description',
        fallback: 'Hosts and keys from the complete SSH directory',
      ),
      HostImportSource.putty => tr(
        'workspace.import.source.putty.description',
        fallback: 'Sessions from an exported .reg file',
      ),
      HostImportSource.mobaXterm => '.mxtsessions or MobaXterm.ini',
      HostImportSource.secureCrt => tr(
        'workspace.import.source.secureCrt.description',
        fallback: 'Sessions from a SecureCRT folder',
      ),
    };
