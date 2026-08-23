part of 'nauterm_workspace.dart';

const double _quickConnectShellButtonHeight = 36;

const _quickConnectShellPreviewCount = 8;

List<_QuickConnectShell> _quickConnectShells() {
  final shellsByLabel = <String, _QuickConnectShell>{};
  for (final shellPath in _systemShells()) {
    if (!_isExecutableShell(shellPath)) {
      continue;
    }
    final label = _quickConnectShellLabel(shellPath);
    shellsByLabel.putIfAbsent(
      label,
      () => _QuickConnectShell(label, shellPath),
    );
  }

  final shells = shellsByLabel.values.toList(growable: false);
  shells.sort((a, b) {
    final priority = _quickConnectShellPriority(a.path)
        .compareTo(_quickConnectShellPriority(b.path));
    if (priority != 0) {
      return priority;
    }
    return a.label.compareTo(b.label);
  });
  if (shells.isEmpty) {
    return shells;
  }
  final environmentShell = io.Platform.isWindows
      ? null
      : _emptyToNull(io.Platform.environment['SHELL']);
  final defaultIndex = environmentShell == null
      ? 0
      : shells.indexWhere((shell) => shell.path == environmentShell);
  final resolvedDefaultIndex = defaultIndex < 0 ? 0 : defaultIndex;
  final defaultShell = shells[resolvedDefaultIndex];
  return [
    defaultShell,
    for (var index = 0; index < shells.length; index++)
      if (index != resolvedDefaultIndex) shells[index],
  ];
}

bool _isExecutableShell(String path) {
  final type = io.FileSystemEntity.typeSync(path, followLinks: true);
  return type == io.FileSystemEntityType.file ||
      type == io.FileSystemEntityType.link;
}

String _quickConnectShellLabel(String path) {
  return shellDisplayName(path);
}

String _quickConnectShellExecutableName(String path) {
  final trimmed = path.endsWith(io.Platform.pathSeparator)
      ? path.substring(0, path.length - 1)
      : path;
  final index = trimmed.lastIndexOf(io.Platform.pathSeparator);
  return index == -1 ? trimmed : trimmed.substring(index + 1);
}

int _quickConnectShellPriority(String path) {
  if (!io.Platform.isWindows) {
    return 100;
  }
  final executableName = _quickConnectShellExecutableName(path).toLowerCase();
  return switch (executableName) {
    'pwsh.exe' => 0,
    'powershell.exe' => 1,
    'cmd.exe' => 2,
    'bash.exe' => 3,
    _ => 100,
  };
}

class _QuickConnectShell {
  const _QuickConnectShell(this.label, this.path);

  final String label;
  final String path;
}

sealed class _QuickConnectResult {
  const _QuickConnectResult();
}

class _QuickConnectLocalShell extends _QuickConnectResult {
  const _QuickConnectLocalShell(this.shell);

  final _QuickConnectShell shell;
}

class _QuickConnectHostSsh extends _QuickConnectResult {
  const _QuickConnectHostSsh(this.host);

  final _HostItem host;
}

class _QuickConnectHostMosh extends _QuickConnectResult {
  const _QuickConnectHostMosh(this.host);

  final _HostItem host;
}

class _QuickConnectHostOpen extends _QuickConnectResult {
  const _QuickConnectHostOpen(this.host);

  final _HostItem host;
}

class _QuickConnectHostTelnet extends _QuickConnectResult {
  const _QuickConnectHostTelnet(this.host);

  final _HostItem host;
}

class _QuickConnectHostSftp extends _QuickConnectResult {
  const _QuickConnectHostSftp(this.host);

  final _HostItem host;
}

class _QuickConnectDirectSsh extends _QuickConnectResult {
  const _QuickConnectDirectSsh(this.query);

  final _DirectHostQuery query;
}

class _QuickConnectDialog extends StatefulWidget {
  const _QuickConnectDialog({
    required this.hosts,
    required this.groups,
    required this.tags,
    required this.loading,
  });

  final List<_HostItem> hosts;
  final List<_GroupItem> groups;
  final List<TagEntry> tags;
  final bool loading;

  @override
  State<_QuickConnectDialog> createState() => _QuickConnectDialogState();
}

class _QuickConnectDialogState extends State<_QuickConnectDialog> {
  final TextEditingController _searchController = TextEditingController();
  bool _showAllShells = false;

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

  void _submitSearch() {
    final directQuery = _directQuery;
    if (directQuery != null) {
      _close(_QuickConnectDirectSsh(directQuery));
      return;
    }

    final firstHost = _filteredHosts.firstOrNull;
    if (firstHost != null) {
      _close(_QuickConnectHostOpen(firstHost));
    }
  }

  void _close(_QuickConnectResult result) {
    Navigator.of(context).pop(result);
  }

  void _dismiss() {
    Navigator.of(context).pop();
  }

  KeyEventResult _handleKeyEvent(FocusNode node, KeyEvent event) {
    if (event is KeyDownEvent &&
        event.logicalKey == LogicalKeyboardKey.escape) {
      _dismiss();
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  Map<int, String> get _groupNamesById => {
    for (final group in widget.groups) group.id: group.name,
  };

  _DirectHostQuery? get _directQuery {
    final text = _searchController.text.trim();
    return text.contains('@') ? _DirectHostQuery.parse(text) : null;
  }

  List<_HostItem> get _filteredHosts {
    final query = _HostSearchQuery.parse(_searchController.text);
    final groupNames = _groupNamesById;
    final tagNamesByUuid = _tagNamesByUuid(widget.tags);
    final hosts = query.isEmpty
        ? widget.hosts
        : widget.hosts.where((host) {
            return query.matchesHost(
              host,
              groupNames[host.groupId],
              tagNames: _tagNamesForHost(host, tagNamesByUuid),
            );
          });
    return hosts.toList(growable: false)
      ..sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
  }

  @override
  Widget build(BuildContext context) {
    final hosts = _filteredHosts;
    final directQuery = _directQuery;
    final shells = _quickConnectShells();
    final hasMoreShells = shells.length > _quickConnectShellPreviewCount;
    final visibleShells = _showAllShells
        ? shells
        : shells.take(_quickConnectShellPreviewCount).toList(growable: false);
    return Focus(
      onKeyEvent: _handleKeyEvent,
      child: Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 18),
          child: Material(
            color: Colors.transparent,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 500, maxHeight: 560),
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: _surface,
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(color: _sidebarDivider),
                ),
                child: ClipRRect(
                  borderRadius: BorderRadius.circular(10),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      _QuickConnectHeader(
                        controller: _searchController,
                        onSubmitted: _submitSearch,
                        onDismissed: _dismiss,
                      ),
                      Flexible(
                        child: ListView(
                          padding: const EdgeInsets.fromLTRB(14, 10, 14, 14),
                          children: [
                            _QuickConnectSectionHeader(
                              label: 'Shells',
                              trailing: hasMoreShells
                                  ? _WorkspaceButton(
                                      label: _showAllShells
                                          ? tr(
                                              'common.label.showLess',
                                              fallback: 'Show Less',
                                            )
                                          : tr(
                                              'common.label.showAll',
                                              fallback: 'Show All',
                                            ),
                                      icon: _showAllShells
                                          ? Icons.expand_less_rounded
                                          : Icons.expand_more_rounded,
                                      variant: _WorkspaceButtonVariant.text,
                                      size: _WorkspaceControlSize.tiny,
                                      horizontalPadding: 8,
                                      onPressed: () {
                                        setState(() {
                                          _showAllShells = !_showAllShells;
                                        });
                                      },
                                    )
                                  : null,
                            ),
                            SizedBox(height: 6),
                            _QuickConnectShellGrid(
                              shells: visibleShells,
                              onSelected: (shell) {
                                _close(_QuickConnectLocalShell(shell));
                              },
                            ),
                            SizedBox(height: 14),
                            const _QuickConnectSectionLabel('Hosts'),
                            SizedBox(height: 6),
                            if (directQuery != null) ...[
                              _QuickConnectDirectRow(
                                query: directQuery,
                                onConnect: () =>
                                    _close(_QuickConnectDirectSsh(directQuery)),
                              ),
                              SizedBox(height: 6),
                            ],
                            if (widget.loading)
                              const _QuickConnectEmptyState('Loading hosts...')
                            else if (hosts.isEmpty)
                              const _QuickConnectEmptyState('No hosts found.')
                            else
                              _QuickConnectHostList(
                                hosts: hosts,
                                groupNamesById: _groupNamesById,
                                onOpen: (host) =>
                                    _close(_QuickConnectHostOpen(host)),
                                onSsh: (host) =>
                                    _close(_QuickConnectHostSsh(host)),
                                onMosh: (host) =>
                                    _close(_QuickConnectHostMosh(host)),
                                onTelnet: (host) =>
                                    _close(_QuickConnectHostTelnet(host)),
                                onSftp: (host) =>
                                    _close(_QuickConnectHostSftp(host)),
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
        ),
      ),
    );
  }
}

class _QuickConnectHeader extends StatelessWidget {
  const _QuickConnectHeader({
    required this.controller,
    required this.onSubmitted,
    required this.onDismissed,
  });

  final TextEditingController controller;
  final VoidCallback onSubmitted;
  final VoidCallback onDismissed;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      key: const ValueKey('quick-connect-direct-row'),
      decoration: BoxDecoration(
        color: _card,
        border: Border(bottom: BorderSide(color: _sidebarDivider)),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    tr('common.label.quickConnect', fallback: 'Quick Connect'),
                    style: TextStyle(
                      color: _text,
                      fontSize: NautermFontSizes.titleSmall,
                      fontWeight: NautermFontWeights.semibold,
                      letterSpacing: 0,
                    ),
                  ),
                ),
                _WorkspaceButton(
                  icon: Icons.close_rounded,
                  tooltip: tr('common.action.close', fallback: 'Close'),
                  variant: _WorkspaceButtonVariant.text,
                  size: _WorkspaceControlSize.tiny,
                  onPressed: onDismissed,
                ),
              ],
            ),
            SizedBox(height: 9),
            Container(
              height: 36,
              decoration: BoxDecoration(
                color: _card,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: _sidebarDivider),
              ),
              padding: const EdgeInsets.only(left: 10, right: 8),
              child: Row(
                children: [
                  Icon(
                    Icons.search_rounded,
                    size: 16,
                    color: Color(0xff6f848c),
                  ),
                  SizedBox(width: 8),
                  Expanded(
                    child: Shortcuts(
                      shortcuts: const {
                        SingleActivator(LogicalKeyboardKey.escape):
                            DismissIntent(),
                      },
                      child: Actions(
                        actions: {
                          DismissIntent: CallbackAction<DismissIntent>(
                            onInvoke: (_) {
                              onDismissed();
                              return null;
                            },
                          ),
                        },
                        child: TextField(
                          controller: controller,
                          autofocus: true,
                          onSubmitted: (_) => onSubmitted(),
                          textInputAction: TextInputAction.go,
                          style: TextStyle(
                            color: _text,
                            fontSize: NautermFontSizes.labelLarge,
                            fontWeight: NautermFontWeights.regular,
                            letterSpacing: 0,
                          ),
                          decoration: InputDecoration(
                            border: InputBorder.none,
                            isDense: true,
                            contentPadding: EdgeInsets.zero,
                            hintText: tr(
                              'workspace.search.quickConnect.placeholder',
                              fallback:
                                  'Search host:, group:, tag:, or user@host',
                            ),
                            hintStyle: TextStyle(
                              color: _mutedText,
                              fontSize: NautermFontSizes.labelLarge,
                              fontWeight: NautermFontWeights.regular,
                              letterSpacing: 0,
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _QuickConnectSectionLabel extends StatelessWidget {
  const _QuickConnectSectionLabel(this.label);

  final String label;

  @override
  Widget build(BuildContext context) {
    return Text(
      label,
      style: TextStyle(
        color: _text,
        fontSize: NautermFontSizes.labelMedium,
        fontWeight: NautermFontWeights.semibold,
        letterSpacing: 0,
      ),
    );
  }
}

class _QuickConnectSectionHeader extends StatelessWidget {
  const _QuickConnectSectionHeader({required this.label, this.trailing});

  final String label;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Expanded(child: _QuickConnectSectionLabel(label)),
        ?trailing,
      ],
    );
  }
}

class _QuickConnectShellGrid extends StatelessWidget {
  const _QuickConnectShellGrid({
    required this.shells,
    required this.onSelected,
  });

  final List<_QuickConnectShell> shells;
  final ValueChanged<_QuickConnectShell> onSelected;

  @override
  Widget build(BuildContext context) {
    if (shells.isEmpty) {
      return const SizedBox.shrink();
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final columnCount = constraints.maxWidth < 360 ? 2 : 3;
        return GridView.builder(
          shrinkWrap: true,
          physics: const NeverScrollableScrollPhysics(),
          itemCount: shells.length,
          gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
            crossAxisCount: columnCount,
            mainAxisSpacing: 6,
            crossAxisSpacing: 6,
            mainAxisExtent: _quickConnectShellButtonHeight,
          ),
          itemBuilder: (context, index) {
            final shell = shells[index];
            return _WorkspaceButton(
              label: shell.label,
              tooltip: shell.label,
              variant: _WorkspaceButtonVariant.outlined,
              size: _WorkspaceControlSize.small,
              fullWidth: true,
              onPressed: () => onSelected(shell),
            );
          },
        );
      },
    );
  }
}

class _QuickConnectHostList extends StatelessWidget {
  const _QuickConnectHostList({
    required this.hosts,
    required this.groupNamesById,
    required this.onOpen,
    required this.onSsh,
    required this.onMosh,
    required this.onTelnet,
    required this.onSftp,
  });

  final List<_HostItem> hosts;
  final Map<int, String> groupNamesById;
  final ValueChanged<_HostItem> onOpen;
  final ValueChanged<_HostItem> onSsh;
  final ValueChanged<_HostItem> onMosh;
  final ValueChanged<_HostItem> onTelnet;
  final ValueChanged<_HostItem> onSftp;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: _card,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: _sidebarDivider),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (var index = 0; index < hosts.length; index++) ...[
            _QuickConnectHostRow(
              host: hosts[index],
              groupName: groupNamesById[hosts[index].groupId],
              onOpen: () => onOpen(hosts[index]),
              onSsh: () => onSsh(hosts[index]),
              onMosh: () => onMosh(hosts[index]),
              onTelnet: () => onTelnet(hosts[index]),
              onSftp: () => onSftp(hosts[index]),
            ),
            if (index != hosts.length - 1)
              Divider(height: 1, color: _sidebarDivider),
          ],
        ],
      ),
    );
  }
}

class _QuickConnectHostRow extends StatelessWidget {
  const _QuickConnectHostRow({
    required this.host,
    required this.groupName,
    required this.onOpen,
    required this.onSsh,
    required this.onMosh,
    required this.onTelnet,
    required this.onSftp,
  });

  final _HostItem host;
  final String? groupName;
  final VoidCallback onOpen;
  final VoidCallback onSsh;
  final VoidCallback onMosh;
  final VoidCallback onTelnet;
  final VoidCallback onSftp;

  @override
  Widget build(BuildContext context) {
    final address = _hostAddressLabel(host);
    final details = [?host.username, ?address, ?groupName].join('  ');
    final hasSsh = host.type == _connectionTypeRemote && host.sshEnabled;
    final hasMosh = host.moshEnabled;
    final hasTelnet = host.telnetEnabled;

    return InkWell(
      onTap: onOpen,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(10, 8, 8, 8),
        child: Row(
          children: [
            _QuickConnectHostIcon(host: host),
            SizedBox(width: 9),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    host.name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: _text,
                      fontSize: NautermFontSizes.labelLarge,
                      fontWeight: NautermFontWeights.semibold,
                      letterSpacing: 0,
                    ),
                  ),
                  if (details.isNotEmpty) ...[
                    SizedBox(height: 2),
                    Text(
                      details,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: _mutedText,
                        fontSize: NautermFontSizes.labelMedium,
                        fontWeight: NautermFontWeights.regular,
                        letterSpacing: 0,
                      ),
                    ),
                  ],
                ],
              ),
            ),
            SizedBox(width: 8),
            _WorkspaceButton(
              label: hasSsh ? 'SSH' : 'Open',
              variant: _WorkspaceButtonVariant.outlined,
              size: _WorkspaceControlSize.tiny,
              horizontalPadding: 11,
              onPressed: hasSsh ? onSsh : onOpen,
            ),
            if (hasMosh) ...[
              SizedBox(width: 6),
              _WorkspaceButton(
                label: 'Mosh',
                tooltip: tr(
                  'workspace.label.connectWithMosh',
                  fallback: 'Connect with Mosh',
                ),
                variant: _WorkspaceButtonVariant.outlined,
                size: _WorkspaceControlSize.tiny,
                horizontalPadding: 11,
                onPressed: onMosh,
              ),
            ],
            if (hasTelnet) ...[
              SizedBox(width: 6),
              _WorkspaceButton(
                label: 'Telnet',
                tooltip: tr(
                  'workspace.label.connectWithTelnet',
                  fallback: 'Connect with Telnet',
                ),
                variant: _WorkspaceButtonVariant.outlined,
                size: _WorkspaceControlSize.tiny,
                horizontalPadding: 11,
                onPressed: onTelnet,
              ),
            ],
            if (hasSsh) ...[
              SizedBox(width: 6),
              _WorkspaceButton(
                label: 'SFTP',
                tooltip: tr(
                  'workspace.label.connectWithSftp',
                  fallback: 'Connect with SFTP',
                ),
                variant: _WorkspaceButtonVariant.outlined,
                size: _WorkspaceControlSize.tiny,
                horizontalPadding: 11,
                onPressed: onSftp,
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _QuickConnectHostIcon extends StatelessWidget {
  const _QuickConnectHostIcon({required this.host});

  final _HostItem host;

  @override
  Widget build(BuildContext context) {
    final osSlug = _BrandIcon._resolveOsSlug(host.os, host.distro);
    final mode = hostIconMode;

    // Mode: icon — replace the entire icon with the OS icon
    if (mode == HostIconMode.osIcon && osSlug != null) {
      final brandColor = _BrandIcon._osBrandColor(osSlug);
      return Container(
        width: 36,
        height: 36,
        decoration: BoxDecoration(
          color: brandColor,
          borderRadius: BorderRadius.circular(9),
        ),
        child: Padding(
          padding: const EdgeInsets.all(7),
          child: SvgPicture.asset(
            'assets/icons/os/system-$osSlug.svg',
            width: 20,
            height: 20,
            colorFilter: const ColorFilter.mode(Colors.white, BlendMode.srcIn),
            placeholderBuilder: (_) =>
                Icon(host.icon, size: 20, color: Colors.white),
          ),
        ),
      );
    }

    // Default inner widget
    final Widget inner;
    if (_BrandIcon._isGenericIcon(host.icon) && host.name.isNotEmpty) {
      final initial = host.name[0].toUpperCase();
      final bgColor = _InitialIcon._deterministicColor(host.name);
      inner = Container(
        width: 36,
        height: 36,
        decoration: BoxDecoration(
          color: bgColor,
          borderRadius: BorderRadius.circular(9),
        ),
        child: Center(
          child: Text(
            initial,
            style: TextStyle(
              color: Colors.white,
              fontSize: 15,
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
      );
    } else {
      inner = Container(
        width: 36,
        height: 36,
        decoration: BoxDecoration(
          color: _blend(Colors.white, host.color, 0.18),
          borderRadius: BorderRadius.circular(9),
        ),
        child: Icon(host.icon, color: host.color, size: 20),
      );
    }

    // Mode: none or no OS detected — just the host icon
    if (mode != HostIconMode.osBadge || osSlug == null) return inner;

    // Mode: badge — host icon with small OS badge
    final brandColor = _BrandIcon._osBrandColor(osSlug);
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
}

class _QuickConnectDirectRow extends StatelessWidget {
  const _QuickConnectDirectRow({required this.query, required this.onConnect});

  final _DirectHostQuery query;
  final VoidCallback onConnect;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: _card,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: _sidebarDivider),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(10, 8, 8, 8),
        child: Row(
          children: [
            Container(
              width: 30,
              height: 30,
              decoration: BoxDecoration(
                color: _blend(Colors.white, _blue, 0.16),
                borderRadius: BorderRadius.circular(7),
              ),
              child: Icon(Icons.bolt_rounded, color: _blue, size: 17),
            ),
            SizedBox(width: 9),
            Expanded(
              child: Text(
                query.label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: _text,
                  fontSize: NautermFontSizes.labelLarge,
                  fontWeight: NautermFontWeights.semibold,
                  letterSpacing: 0,
                ),
              ),
            ),
            SizedBox(width: 8),
            _WorkspaceButton(
              label: 'SSH',
              variant: _WorkspaceButtonVariant.solid,
              type: _WorkspaceButtonType.primary,
              size: _WorkspaceControlSize.tiny,
              horizontalPadding: 12,
              onPressed: onConnect,
            ),
          ],
        ),
      ),
    );
  }
}

class _QuickConnectEmptyState extends StatelessWidget {
  const _QuickConnectEmptyState(this.message);

  final String message;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 52,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: _card,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: _sidebarDivider),
      ),
      child: Text(
        tr(message),
        style: TextStyle(
          color: _mutedText,
          fontSize: NautermFontSizes.labelLarge,
          fontWeight: NautermFontWeights.medium,
          letterSpacing: 0,
        ),
      ),
    );
  }
}

String? _hostAddressLabel(_HostItem host) {
  final address = host.host;
  if (address == null || address.isEmpty) {
    return null;
  }
  final port = host.port;
  return port == null || port == 22 ? address : '$address:$port';
}
