part of 'nauterm_workspace.dart';

class _SftpConnectionPage extends StatefulWidget {
  const _SftpConnectionPage({
    required this.state,
    required this.onRetry,
    required this.onTrustOnceAndRetry,
    required this.onTrustAndRetry,
    required this.onChangeHost,
    this.onClose,
    this.onSshSelected,
  });

  final _SftpConnectionState state;
  final VoidCallback onRetry;
  final VoidCallback onTrustOnceAndRetry;
  final VoidCallback onTrustAndRetry;
  final VoidCallback onChangeHost;
  final VoidCallback? onClose;
  final VoidCallback? onSshSelected;

  @override
  State<_SftpConnectionPage> createState() => _SftpConnectionPageState();
}

class _SftpConnectionPageState extends State<_SftpConnectionPage> {
  final List<_ConnectionIntervention> _interventions = [];

  @override
  void didUpdateWidget(covariant _SftpConnectionPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    final previousHost = oldWidget.state.host;
    final nextHost = widget.state.host;
    if (previousHost.id != nextHost.id ||
        previousHost.host != nextHost.host ||
        previousHost.port != nextHost.port) {
      _interventions.clear();
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    final mode = switch (state.phase) {
      _SftpConnectionPhase.connecting => _ConnectionPageMode.connecting,
      _SftpConnectionPhase.hostKey => _ConnectionPageMode.hostKey,
      _SftpConnectionPhase.connected => _ConnectionPageMode.connected,
      _SftpConnectionPhase.failed => _ConnectionPageMode.failed,
    };
    if (mode == _ConnectionPageMode.hostKey &&
        !_interventions.contains(_ConnectionIntervention.hostKey)) {
      _interventions.add(_ConnectionIntervention.hostKey);
    }
    final target = _sftpConnectionTarget(state.host);
    return ColoredBox(
      color: _surface,
      child: Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 32),
          child: _ConnectionPagePanel(
            title: state.host.name,
            subtitle: 'SFTP $target',
            actionLabel: widget.onSshSelected == null ? null : 'SSH',
            onAction: widget.onSshSelected,
            progress: _ConnectionProgress.forConnection(
              mode: mode,
              interventions: _interventions,
              protocolIcon: LucideIcons.cloud,
              destinationIcon: LucideIcons.folderOpen,
            ),
            body: switch (mode) {
              _ConnectionPageMode.hostKey => _HostKeyPrompt(
                key: const ValueKey('host-key'),
                target: target,
                fingerprint: state.fingerprint,
              ),
              _ConnectionPageMode.failed => _ConnectionFailureBody(
                key: const ValueKey('failed'),
                message: state.message ?? 'SFTP connection failed.',
              ),
              _ => const SizedBox(key: ValueKey('connecting'), height: 1),
            },
            footer: Row(
              children: [
                if (widget.onClose != null)
                  _ConnectionButton(label: 'Close', onPressed: widget.onClose),
                if (mode == _ConnectionPageMode.failed) ...[
                  if (widget.onClose != null) SizedBox(width: 10),
                  _ConnectionButton(
                    label: 'Change host',
                    onPressed: widget.onChangeHost,
                  ),
                ],
                const Spacer(),
                if (mode == _ConnectionPageMode.hostKey) ...[
                  _ConnectionButton(
                    label: 'Continue',
                    onPressed: widget.onTrustOnceAndRetry,
                  ),
                  SizedBox(width: 10),
                  _ConnectionButton(
                    label: 'Add and continue',
                    primary: true,
                    onPressed: widget.onTrustAndRetry,
                  ),
                ] else if (mode == _ConnectionPageMode.failed)
                  _ConnectionButton(
                    label: 'Start over',
                    primary: true,
                    onPressed: widget.onRetry,
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _SftpHostSelectorPane extends StatefulWidget {
  const _SftpHostSelectorPane({
    required this.groups,
    required this.hosts,
    required this.tags,
    required this.searchController,
    required this.onBack,
    this.onUseLocal,
    required this.onHostSelected,
  });

  final List<_GroupItem> groups;
  final List<_HostItem> hosts;
  final List<TagEntry> tags;
  final TextEditingController searchController;
  final VoidCallback onBack;
  final VoidCallback? onUseLocal;
  final ValueChanged<_HostItem> onHostSelected;

  @override
  State<_SftpHostSelectorPane> createState() => _SftpHostSelectorPaneState();
}

class _SftpHostSelectorPaneState extends State<_SftpHostSelectorPane> {
  int? _currentGroupId;
  Set<String> _selectedTagUuids = {};
  _WorkspaceSortOrder _sortOrder = _WorkspaceSortOrder.nameAscending;

  Set<String> get _effectiveTagUuids {
    final available = {
      for (final tag in widget.tags)
        if (tag.uuid != null) tag.uuid!,
    };
    return _selectedTagUuids.intersection(available);
  }

  int? get _effectiveGroupId {
    final groupId = _currentGroupId;
    if (groupId == null || widget.groups.any((group) => group.id == groupId)) {
      return groupId;
    }
    return null;
  }

  List<_GroupItem> _groupPath(int? groupId) {
    final byId = {for (final group in widget.groups) group.id: group};
    final path = <_GroupItem>[];
    final seen = <int>{};
    var current = groupId == null ? null : byId[groupId];
    while (current != null && seen.add(current.id)) {
      path.insert(0, current);
      current = current.parentId == null ? null : byId[current.parentId];
    }
    return path;
  }

  void _handleBack() {
    final groupId = _effectiveGroupId;
    if (groupId == null) {
      widget.onBack();
      return;
    }
    final group = widget.groups.where((item) => item.id == groupId).firstOrNull;
    setState(() => _currentGroupId = group?.parentId);
  }

  void _openGroup(_GroupItem group) {
    setState(() => _currentGroupId = group.id);
  }

  @override
  Widget build(BuildContext context) {
    final currentGroupId = _effectiveGroupId;
    final groupPath = _groupPath(currentGroupId);
    final groupNames = {
      for (final group in widget.groups) group.id: group.name,
    };
    final tagNamesByUuid = _tagNamesByUuid(widget.tags);
    final query = _HostSearchQuery.parse(widget.searchController.text);
    final scopedHosts = currentGroupId == null
        ? widget.hosts
        : widget.hosts.where((host) => host.groupId == currentGroupId);
    final selectedTagUuids = _effectiveTagUuids;
    final taggedHosts = selectedTagUuids.isEmpty
        ? scopedHosts
        : scopedHosts.where(
            (host) => host.tagUuids.any(selectedTagUuids.contains),
          );
    final filteredHosts = _sortWorkspaceItems(
      taggedHosts.where(
        (host) => query.matchesHost(
          host,
          groupNames[host.groupId],
          tagNames: _tagNamesForHost(host, tagNamesByUuid),
        ),
      ),
      _sortOrder,
      ordinal: (host) => host.id,
    ).toList();
    final filteredGroups =
        query.hosts.isNotEmpty ||
            query.usernames.isNotEmpty ||
            query.tags.isNotEmpty
        ? <_GroupItem>[]
        : _sortWorkspaceItems(
            widget.groups.where(
              (group) =>
                  group.parentId == currentGroupId &&
                  (query.isEmpty || query.matchesGroup(group.name)),
            ),
            _sortOrder,
            ordinal: (group) => group.id,
          ).toList();

    return ColoredBox(
      color: _surface,
      child: Column(
        children: [
          Container(
            height: 58,
            color: _card,
            padding: const EdgeInsets.fromLTRB(10, 10, 10, 7),
            child: Row(
              children: [
                _WorkspaceButton(
                  icon: Icons.arrow_back_rounded,
                  variant: _WorkspaceButtonVariant.text,
                  minWidth: 28,
                  horizontalPadding: 0,
                  onPressed: _handleBack,
                ),
                SizedBox(width: 10),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Text(
                        tr('sftp.label.selectHost', fallback: 'Select Host'),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: _text,
                          fontSize: NautermFontSizes.labelLarge,
                          fontWeight: NautermFontWeights.semibold,
                          letterSpacing: 0,
                        ),
                      ),
                      SizedBox(height: 2),
                      Row(
                        children: [
                          Flexible(
                            child: Text(
                              [
                                'Vaults',
                                for (final group in groupPath) group.name,
                              ].join(' / '),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                color: _mutedText,
                                fontSize: NautermFontSizes.labelSmall,
                                fontWeight: NautermFontWeights.medium,
                                letterSpacing: 0,
                              ),
                            ),
                          ),
                          if (groupPath.isEmpty) ...[
                            SizedBox(width: 3),
                            Icon(
                              Icons.keyboard_arrow_down_rounded,
                              size: 15,
                              color: _mutedText,
                            ),
                          ],
                        ],
                      ),
                    ],
                  ),
                ),
                if (widget.onUseLocal != null)
                  _WorkspaceButton(
                    icon: Icons.drive_folder_upload_rounded,
                    label: tr('common.label.local', fallback: 'Local'),
                    variant: _WorkspaceButtonVariant.solid,
                    type: _WorkspaceButtonType.info,
                    height: 30,
                    horizontalPadding: 8,
                    iconGap: 5,
                    onPressed: widget.onUseLocal,
                  ),
              ],
            ),
          ),
          Container(
            height: 42,
            color: _card,
            padding: const EdgeInsets.fromLTRB(10, 0, 10, 8),
            child: Row(
              children: [
                Icon(Icons.search_rounded, color: _text, size: 18),
                SizedBox(width: 8),
                Expanded(
                  child: TextField(
                    controller: widget.searchController,
                    style: TextStyle(
                      color: _text,
                      fontSize: NautermFontSizes.labelMedium,
                      fontWeight: NautermFontWeights.medium,
                      letterSpacing: 0,
                    ),
                    decoration: InputDecoration.collapsed(
                      hintText: tr(
                        'common.label.searchHosts',
                        fallback: 'Search hosts or tags',
                      ),
                      hintStyle: TextStyle(
                        color: _mutedText,
                        fontSize: NautermFontSizes.labelMedium,
                        fontWeight: NautermFontWeights.medium,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                ),
                _HostTagFilterButton(
                  tags: widget.tags,
                  selectedTagUuids: selectedTagUuids,
                  onSelectionChanged: (value) =>
                      setState(() => _selectedTagUuids = value),
                ),
                SizedBox(width: 6),
                _WorkspaceSortDropdownButton(
                  value: _sortOrder,
                  onChanged: (value) => setState(() => _sortOrder = value),
                ),
              ],
            ),
          ),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.fromLTRB(30, 20, 30, 24),
              children: [
                if (filteredGroups.isNotEmpty) ...[
                  const _SectionTitle('Groups'),
                  SizedBox(height: 16),
                  for (final group in filteredGroups)
                    _SftpHostGroupRow(
                      group: group,
                      hostCount: widget.hosts
                          .where((host) => host.groupId == group.id)
                          .length,
                      onTap: () => _openGroup(group),
                    ),
                  SizedBox(height: 26),
                ],
                if (filteredHosts.isNotEmpty) ...[
                  const _SectionTitle('Hosts'),
                  SizedBox(height: 16),
                  for (final host in filteredHosts)
                    _SftpHostRow(
                      host: host,
                      onTap: () => widget.onHostSelected(host),
                    ),
                ],
                if (filteredGroups.isEmpty && filteredHosts.isEmpty)
                  Padding(
                    padding: EdgeInsets.only(top: 50),
                    child: Text(
                      tr(
                        'common.label.noMatchingHosts',
                        fallback: 'No matching hosts',
                      ),
                      textAlign: TextAlign.center,
                      style: TextStyle(
                        color: _mutedText,
                        fontSize: NautermFontSizes.labelLarge,
                        fontWeight: NautermFontWeights.medium,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _SftpHostGroupRow extends StatelessWidget {
  const _SftpHostGroupRow({
    required this.group,
    required this.hostCount,
    required this.onTap,
  });

  final _GroupItem group;
  final int hostCount;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return _SftpHostSelectorRow(
      leading: _SftpIconTile(
        icon: Icons.dashboard_customize_rounded,
        color: group.color,
      ),
      title: group.name,
      subtitle: hostCount == 1 ? '1 Host' : '$hostCount Hosts',
      onTap: onTap,
    );
  }
}

class _SftpHostRow extends StatelessWidget {
  const _SftpHostRow({required this.host, required this.onTap});

  final _HostItem host;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final subtitle = _hostAddressLabel(host) ?? host.subtitle;
    return _SftpHostSelectorRow(
      leading: _BrandIcon(
        icon: host.icon,
        color: host.color,
        name: host.name,
        os: host.os,
        distro: host.distro,
      ),
      title: host.name,
      subtitle: subtitle,
      onTap: onTap,
    );
  }
}

class _SftpHostSelectorRow extends StatelessWidget {
  const _SftpHostSelectorRow({
    required this.leading,
    required this.title,
    required this.subtitle,
    required this.onTap,
  });

  final Widget leading;
  final String title;
  final String subtitle;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final hoverColor = Color.lerp(
      _sidebarHover,
      _sidebarPressed,
      _workspaceDark ? 0.28 : 0.38,
    )!;

    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          hoverColor: hoverColor,
          splashColor: _sidebarPressed.withValues(alpha: 0.36),
          highlightColor: _sidebarPressed.withValues(alpha: 0.72),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
            child: Row(
              children: [
                leading,
                SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        tr(title),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: _text,
                          fontSize: NautermFontSizes.labelLarge,
                          fontWeight: NautermFontWeights.semibold,
                          letterSpacing: 0,
                        ),
                      ),
                      SizedBox(height: 3),
                      Text(
                        tr(subtitle),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: _mutedText,
                          fontSize: NautermFontSizes.labelMedium,
                          fontWeight: NautermFontWeights.medium,
                          letterSpacing: 0,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _SftpIconTile extends StatelessWidget {
  const _SftpIconTile({required this.icon, required this.color});

  final IconData icon;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 40,
      height: 40,
      decoration: BoxDecoration(
        color: color,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Icon(icon, color: Colors.white, size: 22),
    );
  }
}
