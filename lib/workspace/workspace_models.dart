part of 'nauterm_workspace.dart';

enum _WorkspaceTab {
  vaults('Vaults', Icons.security_rounded),
  sftp('SFTP', Icons.folder_rounded),
  sessions('Sessions', Icons.dashboard_rounded);

  const _WorkspaceTab(this.label, this.icon);

  final String label;
  final IconData icon;
}

enum _TerminalTabPageMode { ssh, sftp }

class _PendingHostConnection {
  const _PendingHostConnection({required this.host, required this.item});

  final HostEntry host;
  final _HostItem item;
}

class _TerminalTab {
  _TerminalTab({
    required this.id,
    required this.title,
    required TerminalController controller,
    required TerminalTheme theme,
    _PendingHostConnection? pendingConnection,
    TerminalFontConfig? font,
    _TerminalViewLayout? rootLayout,
    this.replay = false,
    this.replayLoading = false,
    this.pageMode = _TerminalTabPageMode.ssh,
    this.sftpPaneMounted = false,
    this.sftpConnectRequestId = 0,
    this.sftpConnectRequest,
    this.aiAssistantOpen = false,
    this.aiAssistantWidth = 360,
    this.toolPanelMode = _TerminalToolPanelMode.ai,
    AiConversationController? aiConversation,
  }) : font = font ?? terminalFontConfig,
       rootLayout =
           rootLayout ??
           _TerminalViewLeaf(
             _TerminalViewEntry(
               id: id,
               title: title,
               controller: controller,
               theme: theme,
               pendingConnection: pendingConnection,
               composerVisible:
                   !replay && currentTerminalConfig().composer.enabled,
             ),
           ),
       aiConversation = aiConversation ?? AiConversationController();

  final int id;
  final String title;
  final _TerminalViewLayout rootLayout;
  final bool replay;
  bool replayLoading;
  TerminalFontConfig font;
  _TerminalTabPageMode pageMode;
  bool sftpPaneMounted;
  int sftpConnectRequestId;
  _SftpConnectRequest? sftpConnectRequest;
  bool aiAssistantOpen;
  double aiAssistantWidth;
  _TerminalToolPanelMode toolPanelMode;
  final AiConversationController aiConversation;

  _TerminalViewEntry get primaryView => rootLayout.views.first;

  TerminalController get controller => primaryView.controller;

  TerminalTheme get theme => primaryView.theme;

  String displayTitle({int? selectedViewId}) {
    final selectedView = selectedViewId == null
        ? null
        : rootLayout.viewFor(selectedViewId);
    return _terminalViewDisplayTitle(selectedView ?? primaryView);
  }

  Iterable<TerminalController> get controllers sync* {
    for (final view in rootLayout.views) {
      yield* view.controllers;
    }
  }

  _TerminalTab copyWith({String? title, _TerminalViewLayout? rootLayout}) {
    return _TerminalTab(
      id: id,
      title: title ?? this.title,
      controller: controller,
      theme: theme,
      pendingConnection: primaryView.activeTab.pendingConnection,
      font: font,
      rootLayout: rootLayout ?? this.rootLayout,
      replay: replay,
      replayLoading: replayLoading,
      pageMode: pageMode,
      sftpPaneMounted: sftpPaneMounted,
      sftpConnectRequestId: sftpConnectRequestId,
      sftpConnectRequest: sftpConnectRequest,
      aiAssistantOpen: aiAssistantOpen,
      aiAssistantWidth: aiAssistantWidth,
      toolPanelMode: toolPanelMode,
      aiConversation: aiConversation,
    );
  }
}

class _WorkspaceRuntimeState {
  _WorkspaceRuntimeState({
    required this.id,
    required this.name,
    required this.icon,
    required this.color,
    List<_TerminalTab>? terminalTabs,
    AiConversationController? aiConversation,
  }) : terminalTabs = terminalTabs ?? [],
       aiConversation = aiConversation ?? AiConversationController();

  final int id;
  String name;
  final IconData icon;
  final Color color;
  final List<_TerminalTab> terminalTabs;
  int? selectedTerminalId;
  int? selectedTerminalViewId;
  bool aiAssistantOpen = false;
  double aiAssistantWidth = 360;
  final AiConversationController aiConversation;

  int get sessionCount {
    return terminalTabs.fold<int>(
      0,
      (count, tab) => count + tab.controllers.length,
    );
  }
}

class _TerminalViewEntry {
  _TerminalViewEntry({
    required this.id,
    required String title,
    required TerminalController controller,
    required TerminalTheme theme,
    _PendingHostConnection? pendingConnection,
    this.composerVisible = true,
  }) : tabs = [
         _TerminalViewTabEntry(
           id: id,
           title: title,
           controller: controller,
           theme: theme,
           pendingConnection: pendingConnection,
         ),
       ],
       selectedTabId = id;

  final int id;
  final List<_TerminalViewTabEntry> tabs;
  int? selectedTabId;
  bool composerVisible;

  _TerminalViewTabEntry get activeTab {
    return tabs.where((tab) => tab.id == selectedTabId).firstOrNull ??
        tabs.first;
  }

  String get title => _terminalViewTabDisplayTitle(activeTab);

  TerminalController get controller => activeTab.controller;

  TerminalTheme get theme => activeTab.theme;

  Iterable<TerminalController> get controllers sync* {
    for (final tab in tabs) {
      yield tab.controller;
    }
  }
}

class _TerminalViewTabEntry {
  _TerminalViewTabEntry({
    required this.id,
    required this.title,
    required this.controller,
    required this.theme,
    this.pendingConnection,
  }) : connectionPageVisible = false;

  final int id;
  final String title;
  final TerminalController controller;
  TerminalTheme theme;
  final _PendingHostConnection? pendingConnection;
  bool connectionPageVisible;
}

String _terminalViewDisplayTitle(_TerminalViewEntry view) {
  return _terminalViewTabDisplayTitle(view.activeTab);
}

String _terminalViewTabDisplayTitle(_TerminalViewTabEntry tab) {
  return _terminalControllerDisplayTitle(tab.controller, tab.title);
}

String _terminalViewSessionTitle(_TerminalViewTabEntry tab) {
  final shellTitle = tab.controller.snapshot.title.trim();
  return shellTitle.isEmpty ? _terminalViewTabDisplayTitle(tab) : shellTitle;
}

String _terminalControllerDisplayTitle(
  TerminalController controller,
  String fallback,
) {
  final sshProfile = controller.sshProfile;
  if (sshProfile != null) {
    return _remoteConnectionDisplayTitle(
      host: sshProfile.host,
      hostId: sshProfile.hostId,
      label: sshProfile.label,
    );
  }
  final telnetProfile = controller.telnetProfile;
  if (telnetProfile != null) {
    return _remoteConnectionDisplayTitle(
      host: telnetProfile.host,
      hostId: telnetProfile.hostId,
      label: telnetProfile.label,
    );
  }
  final title = controller.snapshot.title.trim();
  return title.isEmpty ? fallback : title;
}

String _remoteConnectionDisplayTitle({
  required String host,
  required int? hostId,
  required String? label,
}) {
  final savedHostName = hostId == null ? null : _emptyToNull(label);
  return savedHostName ?? host;
}

sealed class _TerminalViewLayout {
  const _TerminalViewLayout();

  Iterable<_TerminalViewEntry> get views;

  TerminalTheme get theme => views.first.theme;

  bool containsView(int viewId) => viewFor(viewId) != null;

  _TerminalViewEntry? viewFor(int viewId);

  _TerminalViewLayout splitView({
    required int targetViewId,
    required _TerminalViewEntry newView,
    required Axis axis,
    required int splitId,
  });

  _TerminalViewLayout? removeView(int viewId);
}

final class _TerminalViewLeaf extends _TerminalViewLayout {
  const _TerminalViewLeaf(this.view);

  final _TerminalViewEntry view;

  @override
  Iterable<_TerminalViewEntry> get views sync* {
    yield view;
  }

  @override
  _TerminalViewEntry? viewFor(int viewId) {
    return view.id == viewId ? view : null;
  }

  @override
  _TerminalViewLayout splitView({
    required int targetViewId,
    required _TerminalViewEntry newView,
    required Axis axis,
    required int splitId,
  }) {
    if (view.id != targetViewId) {
      return this;
    }

    return _TerminalSplitLayout(
      id: splitId,
      axis: axis,
      children: [this, _TerminalViewLeaf(newView)],
    );
  }

  @override
  _TerminalViewLayout? removeView(int viewId) {
    return view.id == viewId ? null : this;
  }
}

final class _TerminalSplitLayout extends _TerminalViewLayout {
  const _TerminalSplitLayout({
    required this.id,
    required this.axis,
    required this.children,
  });

  final int id;
  final Axis axis;
  final List<_TerminalViewLayout> children;

  @override
  TerminalTheme get theme => children.first.theme;

  @override
  Iterable<_TerminalViewEntry> get views sync* {
    for (final child in children) {
      yield* child.views;
    }
  }

  @override
  _TerminalViewEntry? viewFor(int viewId) {
    for (final child in children) {
      final view = child.viewFor(viewId);
      if (view != null) {
        return view;
      }
    }
    return null;
  }

  @override
  _TerminalViewLayout splitView({
    required int targetViewId,
    required _TerminalViewEntry newView,
    required Axis axis,
    required int splitId,
  }) {
    var replaced = false;
    final nextChildren = <_TerminalViewLayout>[];
    for (final child in children) {
      if (replaced || !child.containsView(targetViewId)) {
        nextChildren.add(child);
        continue;
      }

      final nextChild = child.splitView(
        targetViewId: targetViewId,
        newView: newView,
        axis: axis,
        splitId: splitId,
      );
      nextChildren.add(nextChild);
      replaced = nextChild.containsView(newView.id);
    }

    if (!replaced) {
      return this;
    }

    return _TerminalSplitLayout(
      id: id,
      axis: this.axis,
      children: nextChildren,
    );
  }

  @override
  _TerminalViewLayout? removeView(int viewId) {
    var removed = false;
    final nextChildren = <_TerminalViewLayout>[];
    for (final child in children) {
      if (!child.containsView(viewId)) {
        nextChildren.add(child);
        continue;
      }

      final nextChild = child.removeView(viewId);
      if (nextChild != null) {
        nextChildren.add(nextChild);
      }
      removed = true;
    }

    if (!removed) {
      return this;
    }
    if (nextChildren.isEmpty) {
      return null;
    }
    if (nextChildren.length == 1) {
      return nextChildren.single;
    }

    return _TerminalSplitLayout(id: id, axis: axis, children: nextChildren);
  }
}

class _CloseTabIntent extends Intent {
  const _CloseTabIntent();
}

class _QuickConnectIntent extends Intent {
  const _QuickConnectIntent();
}

class _PreviousTabIntent extends Intent {
  const _PreviousTabIntent();
}

class _NextTabIntent extends Intent {
  const _NextTabIntent();
}

class _ShowTerminalSshIntent extends Intent {
  const _ShowTerminalSshIntent();
}

class _ShowTerminalSftpIntent extends Intent {
  const _ShowTerminalSftpIntent();
}

class _SelectTabIntent extends Intent {
  const _SelectTabIntent(this.index);

  final int index;
}

class _SplitRightIntent extends Intent {
  const _SplitRightIntent();
}

class _SplitDownIntent extends Intent {
  const _SplitDownIntent();
}

class _NewLocalTerminalIntent extends Intent {
  const _NewLocalTerminalIntent();
}

class _OpenSettingsIntent extends Intent {
  const _OpenSettingsIntent();
}

Map<ShortcutActivator, Intent> _buildWorkspaceShortcuts(
  TerminalShortcutConfig config,
) {
  final isMacOS = defaultTargetPlatform == TargetPlatform.macOS;
  final shortcuts = <ShortcutActivator, Intent>{};

  Iterable<SingleActivator> a(String value) =>
      _parseShortcutActivators(value, isMacOS: isMacOS);

  final mapping = <(String, Intent)>[
    (config.quickConnect, const _QuickConnectIntent()),
    (config.commandPalette, const _QuickConnectIntent()),
    (config.switchToSsh, const _ShowTerminalSshIntent()),
    (config.switchToSftp, const _ShowTerminalSftpIntent()),
    (config.previousTab, const _PreviousTabIntent()),
    (config.nextTab, const _NextTabIntent()),
    (config.splitRight, const _SplitRightIntent()),
    (config.splitDown, const _SplitDownIntent()),
    (config.newLocalTerminal, const _NewLocalTerminalIntent()),
    (config.openSettings, const _OpenSettingsIntent()),
    (config.closeTab, const _CloseTabIntent()),
  ];

  for (final (value, intent) in mapping) {
    for (final activator in a(value)) {
      shortcuts[activator] = intent;
    }
  }

  for (var index = 0; index < config.tabSwitches.length; index++) {
    for (final activator in a(config.tabSwitches[index])) {
      shortcuts[activator] = _SelectTabIntent(index);
    }
  }

  return shortcuts;
}

bool _isQuickConnectKeyEvent(KeyEvent event, TerminalShortcutConfig config) {
  final keyboard = HardwareKeyboard.instance;
  final key = event.logicalKey;
  final isMacOS = defaultTargetPlatform == TargetPlatform.macOS;

  bool matches(String shortcut) {
    final parts = shortcut.toLowerCase().split('+');
    final keyName = parts.last.trim();
    final needsShift = parts.contains('shift');
    if (!shortcutKeyMatches(key, keyName, shift: needsShift)) return false;
    if (needsShift != keyboard.isShiftPressed) return false;
    return true;
  }

  if (isMacOS) {
    if (!keyboard.isMetaPressed || keyboard.isControlPressed) return false;
  } else {
    if (!keyboard.isControlPressed || keyboard.isMetaPressed) return false;
  }
  return matches(config.quickConnect) || matches(config.commandPalette);
}

Iterable<SingleActivator> _parseShortcutActivators(
  String value, {
  required bool isMacOS,
}) sync* {
  final parts = value.toLowerCase().split('+');
  final keyName = parts.last.trim();
  final hasShift = parts.contains('shift');
  final hasAlt = parts.contains('alt');
  for (final key in shortcutKeysFromName(keyName, shift: hasShift)) {
    yield SingleActivator(
      key,
      meta: isMacOS,
      control: !isMacOS,
      shift: hasShift,
      alt: hasAlt,
    );
  }
}

enum _SidebarSection {
  hosts('workspace.sidebar.hosts.label', 'Hosts', LucideIcons.server),
  keychain('workspace.sidebar.keychain.label', 'Keychain', LucideIcons.key),
  proxies('workspace.sidebar.proxies.label', 'Proxies', LucideIcons.network),
  portForwarding(
    'workspace.sidebar.portForwarding.label',
    'Port Forwarding',
    LucideIcons.arrowRightLeft,
  ),
  snippets('workspace.sidebar.snippets.label', 'Snippets', LucideIcons.code),
  knownHosts(
    'workspace.sidebar.knownHosts.label',
    'Known Hosts',
    LucideIcons.fingerprint,
  ),
  logs('workspace.sidebar.logs.label', 'Logs', LucideIcons.clock);

  const _SidebarSection(this.localizationKey, this.label, this.icon);

  final String localizationKey;
  final String label;
  final IconData icon;
}

enum _WorkspaceViewMode { grid, list }

abstract class _WorkspaceItemData {
  const _WorkspaceItemData({
    required this.name,
    required this.subtitle,
    required this.icon,
    required this.color,
    this.createdAt,
    this.updatedAt,
  });

  final String name;
  final String subtitle;
  final IconData icon;
  final Color color;
  final DateTime? createdAt;
  final DateTime? updatedAt;
}

class _GroupItem extends _WorkspaceItemData {
  const _GroupItem({
    required this.id,
    required this.parentId,
    required super.name,
    required super.subtitle,
    required super.icon,
    required super.color,
    super.createdAt,
    super.updatedAt,
  });

  final int id;
  final int? parentId;
}

class _KeyItem extends _WorkspaceItemData {
  const _KeyItem({
    required this.id,
    required super.name,
    required super.subtitle,
    required super.icon,
    required super.color,
    super.createdAt,
    super.updatedAt,
    this.privateKey,
    this.publicKey,
    this.certificate,
  });

  final int id;
  final String? privateKey;
  final String? publicKey;
  final String? certificate;
}

class _IdentityItem extends _WorkspaceItemData {
  const _IdentityItem({
    required this.id,
    required super.name,
    required super.subtitle,
    required super.icon,
    required super.color,
    super.createdAt,
    super.updatedAt,
    this.username,
    this.keyId,
  });

  final int id;
  final String? username;
  final int? keyId;
}

const String _connectionTypeRemote = 'remote';

class _HostItem extends _WorkspaceItemData {
  const _HostItem({
    required this.id,
    this.uuid,
    required super.name,
    required super.subtitle,
    required super.icon,
    required super.color,
    super.createdAt,
    super.updatedAt,
    required this.type,
    this.groupId,
    this.host,
    this.port,
    this.username,
    this.os,
    this.distro,
    this.sshEnabled = false,
    this.moshEnabled = false,
    this.moshServerCommand = defaultMoshServerCommand,
    this.telnetEnabled = false,
    this.tagUuids = const [],
  });

  final int id;
  final String? uuid;
  final String type;
  final int? groupId;
  final String? host;
  final int? port;
  final String? username;
  final String? os;
  final String? distro;
  final bool sshEnabled;
  final bool moshEnabled;
  final String moshServerCommand;
  final bool telnetEnabled;
  final List<String> tagUuids;
}

class _KnownHostItem extends _WorkspaceItemData {
  const _KnownHostItem({
    required this.lineIndex,
    required this.line,
    required this.hostPattern,
    required this.host,
    required this.port,
    required super.name,
    required super.subtitle,
    required super.icon,
    required super.color,
  });

  final int lineIndex;
  final String line;
  final String hostPattern;
  final String? host;
  final int? port;

  bool get canConvertToHost => host != null && host!.trim().isNotEmpty;
}

class _SnippetPackageItem extends _WorkspaceItemData {
  const _SnippetPackageItem({
    required this.id,
    required super.name,
    required super.icon,
    required super.color,
  }) : super(subtitle: '');

  final int id;
}

class _PortForwardItem extends _WorkspaceItemData {
  const _PortForwardItem({
    required this.id,
    required this.type,
    required this.enabled,
    required this.bindAddress,
    required this.bindPort,
    required this.destinationHost,
    required this.destinationPort,
    required this.connectionId,
    required this.intermediateHostName,
    this.activeConnections = 0,
    this.statusError,
    required super.name,
    required super.subtitle,
    required super.icon,
    required super.color,
    super.createdAt,
    super.updatedAt,
  });

  final int id;
  final String type;
  final bool enabled;
  final String bindAddress;
  final int bindPort;
  final String destinationHost;
  final int destinationPort;
  final int connectionId;
  final String intermediateHostName;
  final int activeConnections;
  final String? statusError;
}

class _ProxyItem extends _WorkspaceItemData {
  const _ProxyItem({
    required this.id,
    required this.type,
    required this.host,
    required this.port,
    this.identityId,
    this.identityName,
    this.username,
    required super.name,
    required super.subtitle,
    required super.icon,
    required super.color,
    super.createdAt,
    super.updatedAt,
  });

  final int id;
  final String type;
  final String host;
  final int port;
  final int? identityId;
  final String? identityName;
  final String? username;
}

const Object _preserveSnippetPackageId = Object();

class _SnippetItem extends _WorkspaceItemData {
  const _SnippetItem({
    required this.id,
    this.packageId,
    required this.scope,
    required this.description,
    required this.script,
    required this.targetGroupIds,
    required this.targetHostIds,
    required super.icon,
    required super.color,
    super.createdAt,
    super.updatedAt,
  }) : super(name: description, subtitle: script);

  final int id;
  final int? packageId;
  final SnippetScope scope;
  final String description;
  final String script;
  final List<int> targetGroupIds;
  final List<int> targetHostIds;

  _SnippetItem copyWith({
    Object? packageId = _preserveSnippetPackageId,
    SnippetScope? scope,
    String? description,
    String? script,
    List<int>? targetGroupIds,
    List<int>? targetHostIds,
  }) {
    return _SnippetItem(
      id: id,
      packageId: identical(packageId, _preserveSnippetPackageId)
          ? this.packageId
          : packageId as int?,
      scope: scope ?? this.scope,
      description: description ?? this.description,
      script: script ?? this.script,
      targetGroupIds: targetGroupIds ?? this.targetGroupIds,
      targetHostIds: targetHostIds ?? this.targetHostIds,
      icon: icon,
      color: color,
      createdAt: createdAt,
      updatedAt: updatedAt,
    );
  }
}

class _ContextMenuAction {
  const _ContextMenuAction({
    required this.icon,
    required this.label,
    required this.localizationKey,
    this.id,
    this.shortcut,
    this.submenuActions = const [],
    this.destructive = false,
    this.localizationArgs = const {},
  });

  final IconData icon;
  final String label;
  final String localizationKey;
  final Map<String, String> localizationArgs;
  final _ContextMenuActionId? id;
  final String? shortcut;
  final List<_ContextMenuAction> submenuActions;
  final bool destructive;

  bool get hasSubmenu => submenuActions.isNotEmpty;

  String get localizedLabel =>
      tr(localizationKey, fallback: label, args: localizationArgs);

  String? get displayShortcut {
    final configuredShortcut = switch (id) {
      _ContextMenuActionId.edit => terminalShortcutConfig.editWorkspaceItem,
      _ContextMenuActionId.duplicate =>
        terminalShortcutConfig.duplicateWorkspaceItem,
      _ => null,
    };
    if (configuredShortcut != null) {
      return formatShortcutForPlatform(configuredShortcut);
    }
    return shortcut;
  }
}

enum _ContextMenuActionId {
  open,
  run,
  hostWithSsh,
  hostWithMosh,
  hostWithTelnet,
  hostWithSftp,
  openInCurrentWorkspace,
  openInNewWorkspace,
  convertToHost,
  edit,
  duplicate,
  newHostInGroup,
  copySshCommand,
  exportToHost,
  exportToFile,
  delete,
}

class _MenuDivider {
  const _MenuDivider._();

  static const instance = _MenuDivider._();
}

List<Object> _contextMenuRowsFor(
  _WorkspaceItemData item, {
  String? currentWorkspaceName,
}) {
  if (item is _HostItem) {
    return _hostMenuRowsFor(item, currentWorkspaceName: currentWorkspaceName);
  }
  if (item is _GroupItem) {
    return _groupMenuRows;
  }
  if (item is _KeyItem) {
    return _keyMenuRows;
  }
  if (item is _IdentityItem) {
    return _identityMenuRows;
  }
  if (item is _ProxyItem) {
    return _proxyMenuRows;
  }
  if (item is _KnownHostItem) {
    return _knownHostMenuRows;
  }
  if (item is _SnippetPackageItem) {
    return _snippetPackageMenuRows;
  }
  if (item is _SnippetItem) {
    return _snippetMenuRows;
  }
  return const [];
}

List<Object> _contextMenuRowsForSelection(
  List<_WorkspaceItemData> items, {
  String? currentWorkspaceName,
}) {
  if (items.isEmpty) {
    return const [];
  }
  if (items.length == 1) {
    return _contextMenuRowsFor(
      items.single,
      currentWorkspaceName: currentWorkspaceName,
    );
  }

  final availableActions = _contextMenuActionIds(
    _contextMenuRowsFor(
      items.first,
      currentWorkspaceName: currentWorkspaceName,
    ),
  );
  for (final item in items.skip(1)) {
    availableActions.retainAll(
      _contextMenuActionIds(
        _contextMenuRowsFor(item, currentWorkspaceName: currentWorkspaceName),
      ),
    );
  }
  availableActions.retainAll(_multiSelectionContextActionsFor(items.first));
  return _filterContextMenuRows(
    _contextMenuRowsFor(
      items.first,
      currentWorkspaceName: currentWorkspaceName,
    ),
    availableActions,
  );
}

Set<_ContextMenuActionId> _contextMenuActionIds(Iterable<Object> rows) {
  final ids = <_ContextMenuActionId>{};
  for (final row in rows) {
    if (row is! _ContextMenuAction) {
      continue;
    }
    final id = row.id;
    if (id != null) {
      ids.add(id);
    }
    ids.addAll(_contextMenuActionIds(row.submenuActions));
  }
  return ids;
}

Set<_ContextMenuActionId> _multiSelectionContextActionsFor(
  _WorkspaceItemData item,
) => switch (item) {
  _HostItem() => {
    _ContextMenuActionId.open,
    _ContextMenuActionId.hostWithSsh,
    _ContextMenuActionId.hostWithMosh,
    _ContextMenuActionId.hostWithTelnet,
    _ContextMenuActionId.hostWithSftp,
    _ContextMenuActionId.openInCurrentWorkspace,
    _ContextMenuActionId.openInNewWorkspace,
    _ContextMenuActionId.duplicate,
    _ContextMenuActionId.delete,
  },
  _GroupItem() ||
  _KeyItem() ||
  _IdentityItem() ||
  _ProxyItem() ||
  _SnippetItem() => {
    _ContextMenuActionId.duplicate,
    _ContextMenuActionId.delete,
  },
  _KnownHostItem() || _SnippetPackageItem() => {_ContextMenuActionId.delete},
  _ => const <_ContextMenuActionId>{},
};

List<Object> _filterContextMenuRows(
  Iterable<Object> rows,
  Set<_ContextMenuActionId> availableActions,
) {
  final filtered = <Object>[];
  for (final row in rows) {
    if (row == _MenuDivider.instance) {
      filtered.add(row);
      continue;
    }
    if (row is! _ContextMenuAction) {
      continue;
    }
    if (row.hasSubmenu) {
      final submenuActions = _filterContextMenuRows(
        row.submenuActions,
        availableActions,
      ).whereType<_ContextMenuAction>().toList(growable: false);
      if (submenuActions.isNotEmpty) {
        filtered.add(
          _ContextMenuAction(
            icon: row.icon,
            label: row.label,
            localizationKey: row.localizationKey,
            localizationArgs: row.localizationArgs,
            id: row.id,
            shortcut: row.shortcut,
            submenuActions: submenuActions,
            destructive: row.destructive,
          ),
        );
      }
      continue;
    }
    if (row.id case final id? when availableActions.contains(id)) {
      filtered.add(row);
    }
  }

  while (filtered.firstOrNull == _MenuDivider.instance) {
    filtered.removeAt(0);
  }
  while (filtered.lastOrNull == _MenuDivider.instance) {
    filtered.removeLast();
  }
  return [
    for (var index = 0; index < filtered.length; index++)
      if (filtered[index] != _MenuDivider.instance ||
          filtered[index - 1] != _MenuDivider.instance)
        filtered[index],
  ];
}

List<Object> _hostMenuRowsFor(_HostItem host, {String? currentWorkspaceName}) {
  final isRemote = host.type == _connectionTypeRemote;
  final hasSsh = isRemote && host.sshEnabled;
  final hasMosh = host.moshEnabled;
  final hasTelnet = host.telnetEnabled;
  final workspaceName = _emptyToNull(currentWorkspaceName) ?? 'Current';

  return [
    const _ContextMenuAction(
      icon: Icons.open_in_new_rounded,
      label: 'Open',
      localizationKey: 'common.action.open',
      id: _ContextMenuActionId.open,
      shortcut: '↵',
    ),
    if (workspacePageEnabled)
      _ContextMenuAction(
        icon: Icons.dashboard_rounded,
        label: 'Open in $workspaceName Workspace',
        localizationKey: 'workspace.contextMenu.host.openInWorkspace',
        localizationArgs: {'workspace': workspaceName},
        id: _ContextMenuActionId.openInCurrentWorkspace,
      ),
    if (workspacePageEnabled)
      const _ContextMenuAction(
        icon: Icons.add_to_photos_rounded,
        label: 'Open in New Workspace',
        localizationKey: 'workspace.label.openInNewWorkspace',
        id: _ContextMenuActionId.openInNewWorkspace,
      ),
    if (isRemote)
      _ContextMenuAction(
        icon: Icons.dns_rounded,
        label: 'Host',
        localizationKey: 'workspace.contextMenu.host.connectionMethods',
        submenuActions: [
          if (hasSsh)
            _ContextMenuAction(
              icon: LucideIcons.squareTerminal,
              label: 'With SSH',
              localizationKey: 'workspace.label.withSsh',
              id: _ContextMenuActionId.hostWithSsh,
            ),
          if (hasMosh)
            _ContextMenuAction(
              icon: LucideIcons.radioTower,
              label: 'With Mosh',
              localizationKey: 'workspace.label.withMosh',
              id: _ContextMenuActionId.hostWithMosh,
            ),
          if (hasSsh)
            _ContextMenuAction(
              icon: Icons.folder_rounded,
              label: 'With SFTP',
              localizationKey: 'workspace.label.withSftp',
              id: _ContextMenuActionId.hostWithSftp,
            ),
          if (hasTelnet)
            _ContextMenuAction(
              icon: Icons.settings_ethernet_rounded,
              label: 'With Telnet',
              localizationKey: 'workspace.label.withTelnet',
              id: _ContextMenuActionId.hostWithTelnet,
            ),
        ],
      ),
    const _ContextMenuAction(
      icon: LucideIcons.pencil,
      label: 'Edit Host',
      localizationKey: 'workspace.label.editHost',
      id: _ContextMenuActionId.edit,
    ),
    _MenuDivider.instance,
    const _ContextMenuAction(
      icon: Icons.copy_rounded,
      label: 'Duplicate',
      localizationKey: 'workspace.action.duplicate',
      id: _ContextMenuActionId.duplicate,
    ),
    const _ContextMenuAction(
      icon: Icons.key_rounded,
      label: 'Copy SSH Command',
      localizationKey: 'workspace.label.copySshCommand',
      id: _ContextMenuActionId.copySshCommand,
    ),
    _MenuDivider.instance,
    const _ContextMenuAction(
      icon: LucideIcons.trash2,
      label: 'Delete Host',
      localizationKey: 'workspace.label.deleteHost',
      id: _ContextMenuActionId.delete,
      destructive: true,
    ),
  ];
}

const List<Object> _groupMenuRows = [
  _ContextMenuAction(
    icon: Icons.folder_open_rounded,
    label: 'Open Group',
    localizationKey: 'workspace.label.openGroup',
    id: _ContextMenuActionId.open,
    shortcut: '↵',
  ),
  _ContextMenuAction(
    icon: LucideIcons.pencil,
    label: 'Edit Group',
    localizationKey: 'workspace.label.editGroup',
    id: _ContextMenuActionId.edit,
  ),
  _MenuDivider.instance,
  _ContextMenuAction(
    icon: Icons.copy_rounded,
    label: 'Duplicate Group',
    localizationKey: 'workspace.label.duplicateGroup',
    id: _ContextMenuActionId.duplicate,
  ),
  _ContextMenuAction(
    icon: Icons.add_circle_outline_rounded,
    label: 'New Host in Group',
    localizationKey: 'workspace.label.newHostInGroup',
    id: _ContextMenuActionId.newHostInGroup,
  ),
  _MenuDivider.instance,
  _ContextMenuAction(
    icon: LucideIcons.trash2,
    label: 'Delete Group',
    localizationKey: 'workspace.label.deleteGroup',
    id: _ContextMenuActionId.delete,
    destructive: true,
  ),
];

const List<Object> _keyMenuRows = [
  _ContextMenuAction(
    icon: Icons.ios_share_rounded,
    label: 'Export to Host',
    localizationKey: 'workspace.label.exportToHost',
    id: _ContextMenuActionId.exportToHost,
  ),
  _ContextMenuAction(
    icon: Icons.save_alt_rounded,
    label: 'Export to File',
    localizationKey: 'workspace.label.exportToFile',
    id: _ContextMenuActionId.exportToFile,
  ),
  _ContextMenuAction(
    icon: LucideIcons.pencil,
    label: 'Edit Key',
    localizationKey: 'workspace.label.editKey',
    id: _ContextMenuActionId.edit,
  ),
  _ContextMenuAction(
    icon: Icons.copy_rounded,
    label: 'Duplicate',
    localizationKey: 'workspace.action.duplicate',
    id: _ContextMenuActionId.duplicate,
  ),
  _MenuDivider.instance,
  _ContextMenuAction(
    icon: LucideIcons.trash2,
    label: 'Delete Key',
    localizationKey: 'workspace.label.deleteKey',
    id: _ContextMenuActionId.delete,
    destructive: true,
  ),
];

const List<Object> _identityMenuRows = [
  _ContextMenuAction(
    icon: LucideIcons.pencil,
    label: 'Edit Identity',
    localizationKey: 'workspace.label.editIdentity',
    id: _ContextMenuActionId.edit,
  ),
  _ContextMenuAction(
    icon: Icons.copy_rounded,
    label: 'Duplicate',
    localizationKey: 'workspace.action.duplicate',
    id: _ContextMenuActionId.duplicate,
  ),
  _MenuDivider.instance,
  _ContextMenuAction(
    icon: LucideIcons.trash2,
    label: 'Delete Identity',
    localizationKey: 'workspace.label.deleteIdentity',
    id: _ContextMenuActionId.delete,
    destructive: true,
  ),
];

const List<Object> _proxyMenuRows = [
  _ContextMenuAction(
    icon: LucideIcons.pencil,
    label: 'Edit Proxy',
    localizationKey: 'workspace.label.editProxy',
    id: _ContextMenuActionId.edit,
  ),
  _ContextMenuAction(
    icon: Icons.copy_rounded,
    label: 'Duplicate',
    localizationKey: 'workspace.action.duplicate',
    id: _ContextMenuActionId.duplicate,
  ),
  _MenuDivider.instance,
  _ContextMenuAction(
    icon: LucideIcons.trash2,
    label: 'Delete Proxy',
    localizationKey: 'workspace.label.deleteProxy',
    id: _ContextMenuActionId.delete,
    destructive: true,
  ),
];

const List<Object> _knownHostMenuRows = [
  _ContextMenuAction(
    icon: Icons.public_rounded,
    label: 'Convert to Host',
    localizationKey: 'workspace.label.convertToHost',
    id: _ContextMenuActionId.convertToHost,
  ),
  _MenuDivider.instance,
  _ContextMenuAction(
    icon: LucideIcons.trash2,
    label: 'Delete',
    localizationKey: 'common.action.delete',
    id: _ContextMenuActionId.delete,
    destructive: true,
  ),
];

const List<Object> _snippetMenuRows = [
  _ContextMenuAction(
    icon: Icons.play_arrow_rounded,
    label: 'Run',
    localizationKey: 'common.action.run',
    id: _ContextMenuActionId.run,
    shortcut: '↵',
  ),
  _ContextMenuAction(
    icon: LucideIcons.pencil,
    label: 'Edit',
    localizationKey: 'common.action.edit',
    id: _ContextMenuActionId.edit,
  ),
  _MenuDivider.instance,
  _ContextMenuAction(
    icon: Icons.copy_rounded,
    label: 'Duplicate',
    localizationKey: 'workspace.action.duplicate',
    id: _ContextMenuActionId.duplicate,
  ),
  _MenuDivider.instance,
  _ContextMenuAction(
    icon: LucideIcons.trash2,
    label: 'Delete',
    localizationKey: 'common.action.delete',
    id: _ContextMenuActionId.delete,
    destructive: true,
  ),
];

const List<Object> _snippetPackageMenuRows = [
  _ContextMenuAction(
    icon: Icons.folder_open_rounded,
    label: 'Open',
    localizationKey: 'common.action.open',
    id: _ContextMenuActionId.open,
    shortcut: '↵',
  ),
  _ContextMenuAction(
    icon: LucideIcons.pencil,
    label: 'Rename',
    localizationKey: 'common.action.rename',
    id: _ContextMenuActionId.edit,
  ),
  _MenuDivider.instance,
  _ContextMenuAction(
    icon: LucideIcons.trash2,
    label: 'Delete',
    localizationKey: 'common.action.delete',
    id: _ContextMenuActionId.delete,
    destructive: true,
  ),
];
