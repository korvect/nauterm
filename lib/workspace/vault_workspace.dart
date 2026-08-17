part of 'nauterm_workspace.dart';

class _VaultWorkspace extends StatelessWidget {
  const _VaultWorkspace({
    required this.selectedSection,
    required this.groups,
    required this.hosts,
    required this.tags,
    required this.keys,
    required this.identities,
    required this.portForwards,
    required this.proxies,
    required this.snippetPackages,
    required this.snippets,
    required this.terminalLogs,
    required this.terminalLogsHasMore,
    required this.terminalLogsLoading,
    required this.terminalCaptureDiskUsage,
    required this.shellHistory,
    required this.selectedLogId,
    required this.knownHostsText,
    required this.loadingData,
    required this.onSectionSelected,
    required this.onCreateGroup,
    required this.onCreateHost,
    required this.onImportHosts,
    required this.onExportHosts,
    required this.onCreateKey,
    required this.onCreateCertificate,
    required this.onGenerateKey,
    required this.onCreateIdentity,
    required this.onCreateSnippet,
    required this.onCreateSnippetPackage,
    required this.onCreatePortForward,
    required this.onPortForwardEdit,
    required this.onPortForwardToggle,
    required this.onCreateProxy,
    required this.onProxyEdit,
    required this.onProxyContextAction,
    required this.onProxyContextActions,
    required this.onGroupContextAction,
    required this.onGroupContextActions,
    required this.onHostContextAction,
    required this.onHostContextActions,
    required this.onHostConnected,
    required this.onHostQueryConnected,
    required this.onSaveTag,
    required this.onDeleteTag,
    required this.onSnippetContextAction,
    required this.onSnippetContextActions,
    required this.onSnippetRun,
    required this.onSnippetPackageContextAction,
    required this.onSnippetPackageContextActions,
    required this.onShowShellHistory,
    required this.onLogReplay,
    required this.onLogSelected,
    required this.onLogDelete,
    required this.onLogExport,
    required this.onClearLogs,
    required this.onLoadMoreLogs,
    required this.onImportKnownHosts,
    required this.onKnownHostContextAction,
    required this.onKnownHostContextActions,
    required this.onKeyContextAction,
    required this.onKeyContextActions,
    required this.onIdentityContextAction,
    required this.onIdentityContextActions,
    required this.onOpenSettings,
    required this.onOpenLocalTerminal,
    required this.onOpenSerialTerminal,
    required this.currentWorkspaceName,
    this.dataStore,
  });

  final _SidebarSection selectedSection;
  final NautermDataStore? dataStore;
  final List<_GroupItem> groups;
  final List<_HostItem> hosts;
  final List<TagEntry> tags;
  final List<_KeyItem> keys;
  final List<_IdentityItem> identities;
  final List<_PortForwardItem> portForwards;
  final List<_ProxyItem> proxies;
  final List<_SnippetPackageItem> snippetPackages;
  final List<_SnippetItem> snippets;
  final List<TerminalLogEntry> terminalLogs;
  final bool terminalLogsHasMore;
  final bool terminalLogsLoading;
  final int terminalCaptureDiskUsage;
  final List<ShellHistoryEntry> shellHistory;
  final String? selectedLogId;
  final String knownHostsText;
  final bool loadingData;
  final ValueChanged<_SidebarSection> onSectionSelected;
  final ValueChanged<int?> onCreateGroup;
  final ValueChanged<int?> onCreateHost;
  final VoidCallback onImportHosts;
  final VoidCallback onExportHosts;
  final VoidCallback onCreateKey;
  final VoidCallback onCreateCertificate;
  final VoidCallback onGenerateKey;
  final VoidCallback onCreateIdentity;
  final ValueChanged<int?> onCreateSnippet;
  final VoidCallback onCreateSnippetPackage;
  final ValueChanged<String> onCreatePortForward;
  final ValueChanged<_PortForwardItem> onPortForwardEdit;
  final void Function(_PortForwardItem item, bool enabled) onPortForwardToggle;
  final VoidCallback onCreateProxy;
  final ValueChanged<_ProxyItem> onProxyEdit;
  final _WorkspaceContextAction<_ProxyItem> onProxyContextAction;
  final _WorkspaceContextActions<_ProxyItem> onProxyContextActions;
  final _WorkspaceContextAction<_GroupItem> onGroupContextAction;
  final _WorkspaceContextActions<_GroupItem> onGroupContextActions;
  final _WorkspaceContextAction<_HostItem> onHostContextAction;
  final _WorkspaceContextActions<_HostItem> onHostContextActions;
  final ValueChanged<_HostItem> onHostConnected;
  final ValueChanged<String> onHostQueryConnected;
  final ValueChanged<TagEntry> onSaveTag;
  final ValueChanged<TagEntry> onDeleteTag;
  final _WorkspaceContextAction<_SnippetItem> onSnippetContextAction;
  final _WorkspaceContextActions<_SnippetItem> onSnippetContextActions;
  final ValueChanged<_SnippetItem> onSnippetRun;
  final _WorkspaceContextAction<_SnippetPackageItem>
  onSnippetPackageContextAction;
  final _WorkspaceContextActions<_SnippetPackageItem>
  onSnippetPackageContextActions;
  final VoidCallback onShowShellHistory;
  final ValueChanged<TerminalLogEntry> onLogReplay;
  final ValueChanged<TerminalLogEntry> onLogSelected;
  final ValueChanged<TerminalLogEntry> onLogDelete;
  final _TerminalLogExportCallback onLogExport;
  final VoidCallback onClearLogs;
  final VoidCallback onLoadMoreLogs;
  final VoidCallback onImportKnownHosts;
  final _WorkspaceContextAction<_KnownHostItem> onKnownHostContextAction;
  final _WorkspaceContextActions<_KnownHostItem> onKnownHostContextActions;
  final _WorkspaceContextAction<_KeyItem> onKeyContextAction;
  final _WorkspaceContextActions<_KeyItem> onKeyContextActions;
  final _WorkspaceContextAction<_IdentityItem> onIdentityContextAction;
  final _WorkspaceContextActions<_IdentityItem> onIdentityContextActions;
  final VoidCallback onOpenSettings;
  final VoidCallback onOpenLocalTerminal;
  final VoidCallback onOpenSerialTerminal;
  final String currentWorkspaceName;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final collapsed = constraints.maxWidth < _sidebarCollapseBreakpoint;
        final content = switch (selectedSection) {
          _SidebarSection.hosts => _HostsPane(
            groups: groups,
            hosts: hosts,
            tags: tags,
            loading: loadingData,
            onCreateHost: onCreateHost,
            onCreateGroup: onCreateGroup,
            onImportHosts: onImportHosts,
            onExportHosts: onExportHosts,
            onGroupContextAction: onGroupContextAction,
            onGroupContextActions: onGroupContextActions,
            onHostContextAction: onHostContextAction,
            onHostContextActions: onHostContextActions,
            onHostConnected: onHostConnected,
            onConnectQuery: onHostQueryConnected,
            onSaveTag: onSaveTag,
            onDeleteTag: onDeleteTag,
            onOpenLocalTerminal: onOpenLocalTerminal,
            onOpenSerialTerminal: onOpenSerialTerminal,
            currentWorkspaceName: currentWorkspaceName,
          ),
          _SidebarSection.keychain => _KeychainPane(
            keys: keys,
            identities: identities,
            onCreateKey: onCreateKey,
            onCreateCertificate: onCreateCertificate,
            onGenerateKey: onGenerateKey,
            onCreateIdentity: onCreateIdentity,
            onKeyContextAction: onKeyContextAction,
            onKeyContextActions: onKeyContextActions,
            onIdentityContextAction: onIdentityContextAction,
            onIdentityContextActions: onIdentityContextActions,
          ),
          _SidebarSection.portForwarding => _PortForwardingPane(
            forwards: portForwards,
            onCreateForward: onCreatePortForward,
            onEditForward: onPortForwardEdit,
            onToggleForward: onPortForwardToggle,
          ),
          _SidebarSection.proxies => _ProxiesPane(
            proxies: proxies,
            onCreateProxy: onCreateProxy,
            onEditProxy: onProxyEdit,
            onContextAction: onProxyContextAction,
            onContextActions: onProxyContextActions,
          ),
          _SidebarSection.snippets => _SnippetsPane(
            packages: snippetPackages,
            snippets: snippets,
            onCreateSnippet: onCreateSnippet,
            onCreatePackage: onCreateSnippetPackage,
            onShowShellHistory: onShowShellHistory,
            onPackageContextAction: onSnippetPackageContextAction,
            onPackageContextActions: onSnippetPackageContextActions,
            onSnippetContextAction: onSnippetContextAction,
            onSnippetContextActions: onSnippetContextActions,
            onSnippetRun: onSnippetRun,
          ),
          _SidebarSection.knownHosts => _KnownHostsPane(
            text: knownHostsText,
            onImport: onImportKnownHosts,
            onContextAction: onKnownHostContextAction,
            onContextActions: onKnownHostContextActions,
          ),
          _SidebarSection.logs => _LogsPane(
            terminalLogs: terminalLogs,
            hasMore: terminalLogsHasMore,
            loadingMore: terminalLogsLoading,
            captureDiskUsage: terminalCaptureDiskUsage,
            shellHistory: shellHistory,
            selectedLogId: selectedLogId,
            onLogReplay: onLogReplay,
            onLogSelected: onLogSelected,
            onLogDelete: onLogDelete,
            onLogExport: onLogExport,
            onClearLogs: onClearLogs,
            onLoadMore: onLoadMoreLogs,
            dataStore: dataStore,
          ),
        };

        return Row(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            ExcludeFocus(
              key: const ValueKey('workspace-sidebar-tab-exclusion'),
              child: _Sidebar(
                selectedSection: selectedSection,
                collapsed: collapsed,
                onSectionSelected: onSectionSelected,
                onOpenSettings: onOpenSettings,
              ),
            ),
            Expanded(
              child: _WorkspacePageTransition(
                pageKey: 'vault-section:${selectedSection.name}',
                child: content,
              ),
            ),
          ],
        );
      },
    );
  }
}

class _Sidebar extends StatelessWidget {
  const _Sidebar({
    required this.selectedSection,
    required this.collapsed,
    required this.onSectionSelected,
    required this.onOpenSettings,
  });

  final _SidebarSection selectedSection;
  final bool collapsed;
  final ValueChanged<_SidebarSection> onSectionSelected;
  final VoidCallback onOpenSettings;

  @override
  Widget build(BuildContext context) {
    return AnimatedContainer(
      duration: const Duration(milliseconds: 160),
      curve: Curves.easeOutCubic,
      width: collapsed ? _sidebarCollapsedWidth : _sidebarExpandedWidth,
      decoration: BoxDecoration(
        color: _sidebar,
        border: Border(right: BorderSide(color: _sidebarDivider)),
      ),
      padding: const EdgeInsets.fromLTRB(
        _sidebarHorizontalInset,
        8,
        _sidebarHorizontalInset,
        0,
      ),
      child: Column(
        children: [
          for (final section in _SidebarSection.values)
            _SidebarItemButton(
              icon: section.icon,
              label: context.tr(
                section.localizationKey,
                fallback: section.label,
              ),
              selected: section == selectedSection,
              collapsed: collapsed,
              margin: const EdgeInsets.only(bottom: 6),
              onTap: () => onSectionSelected(section),
            ),
          Spacer(),
          _SidebarItemButton(
            key: const ValueKey('sidebar-settings-button'),
            icon: LucideIcons.settings,
            label: context.tr(
              'workspace.sidebar.settings.label',
              fallback: 'Settings',
            ),
            collapsed: collapsed,
            margin: const EdgeInsets.only(bottom: 10),
            onTap: onOpenSettings,
          ),
        ],
      ),
    );
  }
}

class _SidebarItemButton extends StatelessWidget {
  const _SidebarItemButton({
    super.key,
    required this.icon,
    required this.label,
    required this.collapsed,
    required this.onTap,
    this.selected = false,
    this.margin = EdgeInsets.zero,
  });

  final IconData icon;
  final String label;
  final bool selected;
  final bool collapsed;
  final VoidCallback onTap;
  final EdgeInsetsGeometry margin;

  @override
  Widget build(BuildContext context) {
    final selectedHoverColor = _workspaceDark
        ? Color.lerp(_sidebarHover, _sidebarPressed, 0.36)!
        : const Color(0xffdde9ea);
    final splashColor = _workspaceDark
        ? Color.lerp(_sidebarPressed, _text, 0.22)!
        : _sidebarPressed;
    final button = Container(
      height: _sidebarItemHeight,
      margin: margin,
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(7),
        clipBehavior: Clip.antiAlias,
        child: Ink(
          decoration: BoxDecoration(
            color: selected
                ? (_workspaceDark ? _sidebarHover : const Color(0xffe6eeee))
                : Colors.transparent,
            borderRadius: BorderRadius.circular(7),
          ),
          child: InkWell(
            borderRadius: BorderRadius.circular(7),
            hoverColor: selected ? selectedHoverColor : _sidebarHover,
            splashColor: splashColor,
            highlightColor: splashColor.withValues(alpha: 0.58),
            onTap: onTap,
            child: LayoutBuilder(
              builder: (context, constraints) {
                final availableLabelWidth =
                    constraints.maxWidth -
                    _sidebarIconSlotSize -
                    _sidebarIconLabelGap;
                final labelProgress = (availableLabelWidth / 56).clamp(
                  0.0,
                  1.0,
                );
                final showLabel = !collapsed && availableLabelWidth > 1;

                return ClipRect(
                  child: OverflowBox(
                    alignment: Alignment.centerLeft,
                    minWidth: _sidebarIconSlotSize,
                    maxWidth: double.infinity,
                    child: Row(
                      children: [
                        SizedBox(
                          width: _sidebarIconSlotSize,
                          child: Icon(icon, size: 16, color: _text),
                        ),
                        if (showLabel) ...[
                          SizedBox(width: _sidebarIconLabelGap),
                          SizedBox(
                            width: math.max(0, availableLabelWidth),
                            child: ClipRect(
                              child: Opacity(
                                opacity: labelProgress,
                                child: Text(
                                  label,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                    color: _text,
                                    fontSize: NautermFontSizes.labelLarge,
                                    fontWeight: NautermFontWeights.medium,
                                    letterSpacing: 0,
                                  ),
                                ),
                              ),
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                );
              },
            ),
          ),
        ),
      ),
    );

    if (!collapsed) {
      return button;
    }

    return Tooltip(
      message: label,
      waitDuration: const Duration(milliseconds: 250),
      decoration: BoxDecoration(
        color: const Color(0xff151927),
        borderRadius: BorderRadius.circular(6),
      ),
      textStyle: TextStyle(
        color: Colors.white,
        fontSize: NautermFontSizes.labelMedium,
        fontWeight: NautermFontWeights.medium,
        letterSpacing: 0,
      ),
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
      preferBelow: false,
      verticalOffset: 14,
      child: button,
    );
  }
}
