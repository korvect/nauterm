part of 'nauterm_workspace.dart';

typedef _SaveGroup = Future<void> Function(HostGroup group);
typedef _SaveHost = Future<void> Function(HostEntry host);
typedef _CreateTag = TagEntry? Function(String name);
typedef _SaveHostEnvironment = void Function(
  List<HostEnvironmentVariable> variables,
);
typedef _EditHostEnvironment = void Function(
  String hostLabel,
  List<HostEnvironmentVariable> variables,
  ValueChanged<List<HostEnvironmentVariable>> onSaved,
);
typedef _SaveKey = Future<void> Function(KeyEntry key);
typedef _SaveGeneratedKey = Future<void> Function(KeyEntry key);
typedef _ExportKey = Future<void> Function(KeyEntry key, _KeyExportDraft draft);
typedef _ShowWorkspaceNotification = void Function(
  String message, {
  _WorkspaceNotificationType type,
});
typedef _SaveIdentity = Future<void> Function(IdentityEntry identity);
typedef _SavePortForward = Future<void> Function(PortForwardEntry portForward);
typedef _SaveProxy = Future<void> Function(ProxyEntry proxy);
typedef _SaveSnippetPackage = Future<void> Function(
  SnippetPackageEntry package,
);
typedef _SaveSnippet = Future<void> Function(
  _SnippetItem? initial,
  _SnippetDraft draft,
);
typedef _CreateRelatedEntry = void Function(
  String initialName,
  ValueChanged<int> onCreated,
);
typedef _CreateGroupFromProtocol = void Function(HostGroup template);
typedef _CreateRelatedCredential = void Function(
  String initialName, {
  required bool certificate,
  required ValueChanged<int> onCreated,
});

class _WorkspaceEditorStackEntry {
  const _WorkspaceEditorStackEntry({required this.request, this.onSaved});

  final _WorkspaceEditorRequest request;
  final ValueChanged<Object>? onSaved;
}

sealed class _WorkspaceEditorRequest {
  const _WorkspaceEditorRequest();
}

class _GroupEditorRequest extends _WorkspaceEditorRequest {
  const _GroupEditorRequest({
    this.initial,
    this.template,
    this.initialParentId,
    this.initialName,
  });

  final HostGroup? initial;
  final HostGroup? template;
  final int? initialParentId;
  final String? initialName;
}

class _HostEditorRequest extends _WorkspaceEditorRequest {
  const _HostEditorRequest({this.initial, this.initialGroupId});

  final HostEntry? initial;
  final int? initialGroupId;
}

class _HostEnvironmentEditorRequest extends _WorkspaceEditorRequest {
  _HostEnvironmentEditorRequest({
    required this.hostLabel,
    required List<HostEnvironmentVariable> variables,
  }) : variables = List<HostEnvironmentVariable>.unmodifiable(variables);

  final String hostLabel;
  final List<HostEnvironmentVariable> variables;
}

class _KeyEditorRequest extends _WorkspaceEditorRequest {
  const _KeyEditorRequest({
    this.initial,
    this.generate = false,
    this.initialName,
    this.certificateMode = false,
    this.credentialCreation = false,
  });

  final KeyEntry? initial;
  final bool generate;
  final String? initialName;
  final bool certificateMode;

  /// Related editors (Host, Group, and Identity) use the consolidated
  /// Paste / Import / Generate creation flow. The Keychain toolbar keeps its
  /// existing, direct New Key and Generate Key routes.
  final bool credentialCreation;
}

class _KeyExportEditorRequest extends _WorkspaceEditorRequest {
  const _KeyExportEditorRequest({required this.key});

  final KeyEntry key;
}

class _IdentityEditorRequest extends _WorkspaceEditorRequest {
  const _IdentityEditorRequest({this.initial, this.initialName});

  final IdentityEntry? initial;
  final String? initialName;
}

class _PortForwardEditorRequest extends _WorkspaceEditorRequest {
  const _PortForwardEditorRequest({this.initial, this.initialType = 'local'});

  final PortForwardEntry? initial;
  final String initialType;
}

class _ProxyEditorRequest extends _WorkspaceEditorRequest {
  const _ProxyEditorRequest({this.initial, this.initialName});

  final ProxyEntry? initial;
  final String? initialName;
}

class _SnippetEditorRequest extends _WorkspaceEditorRequest {
  const _SnippetEditorRequest({
    this.initial,
    this.initialPackageId,
    this.initialDescription,
    this.initialScript,
  });

  final _SnippetItem? initial;
  final int? initialPackageId;
  final String? initialDescription;
  final String? initialScript;
}

class _SnippetPackageEditorRequest extends _WorkspaceEditorRequest {
  const _SnippetPackageEditorRequest({this.initial});

  final _SnippetPackageItem? initial;
}

class _ShellHistoryDrawerRequest extends _WorkspaceEditorRequest {
  const _ShellHistoryDrawerRequest();
}

class _SnippetDraft {
  const _SnippetDraft({
    this.packageId,
    required this.scope,
    required this.description,
    required this.script,
    required this.targetGroupIds,
    required this.targetHostIds,
  });

  final int? packageId;
  final SnippetScope scope;
  final String description;
  final String script;
  final List<int> targetGroupIds;
  final List<int> targetHostIds;
}

class _WorkspaceEditorDrawer extends StatelessWidget {
  const _WorkspaceEditorDrawer({
    required this.request,
    required this.groups,
    required this.hosts,
    required this.keys,
    required this.identities,
    required this.tags,
    required this.proxies,
    required this.snippetPackages,
    required this.snippets,
    required this.shellHistory,
    required this.terminalThemeCatalog,
    required this.onClose,
    required this.onCreateGroup,
    required this.onCreateGroupFromProtocol,
    required this.onCreateCredential,
    required this.onCreateIdentity,
    required this.onCreateProxy,
    required this.onCreateTag,
    required this.onCreateSnippet,
    required this.onCreateSnippetPackage,
    required this.onEditHostEnvironment,
    required this.onSaveGeneratedKey,
    required this.onExportKey,
    required this.onShowNotification,
    required this.onSaveGroup,
    required this.onDuplicateGroup,
    required this.onDeleteGroup,
    required this.onSaveHost,
    required this.onConnectHost,
    required this.onDuplicateHost,
    required this.onDeleteHost,
    required this.onSaveHostEnvironment,
    required this.onSaveKey,
    required this.onDuplicateKey,
    required this.onExportKeyToHost,
    required this.onExportKeyToFile,
    required this.onDeleteKey,
    required this.onSaveIdentity,
    required this.onDuplicateIdentity,
    required this.onDeleteIdentity,
    required this.onSavePortForward,
    required this.onDeletePortForward,
    required this.onSaveProxy,
    required this.onDuplicateProxy,
    required this.onDeleteProxy,
    required this.onSaveSnippetPackage,
    required this.onDeleteSnippetPackage,
    required this.onSaveSnippet,
    required this.onDuplicateSnippet,
    required this.onDeleteSnippet,
    required this.onCreateSnippetFromShellHistory,
    required this.onClearShellHistory,
  });

  final _WorkspaceEditorRequest? request;
  final List<HostGroup> groups;
  final List<HostEntry> hosts;
  final List<KeyEntry> keys;
  final List<IdentityEntry> identities;
  final List<TagEntry> tags;
  final List<ProxyEntry> proxies;
  final List<_SnippetPackageItem> snippetPackages;
  final List<_SnippetItem> snippets;
  final List<ShellHistoryEntry> shellHistory;
  final TerminalThemeCatalog? terminalThemeCatalog;
  final VoidCallback onClose;
  final _CreateRelatedEntry onCreateGroup;
  final _CreateGroupFromProtocol onCreateGroupFromProtocol;
  final _CreateRelatedCredential onCreateCredential;
  final _CreateRelatedEntry onCreateIdentity;
  final _CreateRelatedEntry onCreateProxy;
  final _CreateTag onCreateTag;
  final _CreateRelatedEntry onCreateSnippet;
  final _CreateRelatedEntry onCreateSnippetPackage;
  final _EditHostEnvironment onEditHostEnvironment;
  final _SaveGeneratedKey onSaveGeneratedKey;
  final _ExportKey onExportKey;
  final _ShowWorkspaceNotification onShowNotification;
  final _SaveGroup onSaveGroup;
  final ValueChanged<HostGroup> onDuplicateGroup;
  final ValueChanged<HostGroup> onDeleteGroup;
  final _SaveHost onSaveHost;
  final ValueChanged<HostEntry> onConnectHost;
  final ValueChanged<HostEntry> onDuplicateHost;
  final ValueChanged<HostEntry> onDeleteHost;
  final _SaveHostEnvironment onSaveHostEnvironment;
  final _SaveKey onSaveKey;
  final ValueChanged<KeyEntry> onDuplicateKey;
  final ValueChanged<KeyEntry> onExportKeyToHost;
  final ValueChanged<KeyEntry> onExportKeyToFile;
  final ValueChanged<KeyEntry> onDeleteKey;
  final _SaveIdentity onSaveIdentity;
  final ValueChanged<IdentityEntry> onDuplicateIdentity;
  final ValueChanged<IdentityEntry> onDeleteIdentity;
  final _SavePortForward onSavePortForward;
  final ValueChanged<PortForwardEntry> onDeletePortForward;
  final _SaveProxy onSaveProxy;
  final ValueChanged<ProxyEntry> onDuplicateProxy;
  final ValueChanged<ProxyEntry> onDeleteProxy;
  final _SaveSnippetPackage onSaveSnippetPackage;
  final ValueChanged<_SnippetPackageItem> onDeleteSnippetPackage;
  final _SaveSnippet onSaveSnippet;
  final ValueChanged<_SnippetItem> onDuplicateSnippet;
  final ValueChanged<_SnippetItem> onDeleteSnippet;
  final ValueChanged<String> onCreateSnippetFromShellHistory;
  final VoidCallback onClearShellHistory;

  @override
  Widget build(BuildContext context) {
    final request = this.request;
    if (request == null) {
      return const SizedBox.shrink();
    }

    final content = switch (request) {
      _GroupEditorRequest() => _GroupEditorContent(
        request: request,
        groups: groups,
        snippets: snippets,
        keys: keys,
        identities: identities,
        proxies: proxies,
        terminalThemeCatalog: terminalThemeCatalog,
        onCreateCredential: onCreateCredential,
        onCreateIdentity: onCreateIdentity,
        onCreateSnippet: onCreateSnippet,
        onEditEnvironment: onEditHostEnvironment,
        onClose: onClose,
        onSave: onSaveGroup,
        onDuplicate: onDuplicateGroup,
        onDelete: onDeleteGroup,
      ),
      _HostEditorRequest() => _HostEditorContent(
        request: request,
        groups: groups,
        snippets: snippets,
        keys: keys,
        identities: identities,
        tags: tags,
        proxies: proxies,
        terminalThemeCatalog: terminalThemeCatalog,
        onClose: onClose,
        onCreateGroup: onCreateGroup,
        onCreateGroupFromProtocol: onCreateGroupFromProtocol,
        onCreateCredential: onCreateCredential,
        onCreateIdentity: onCreateIdentity,
        onCreateProxy: onCreateProxy,
        onCreateTag: onCreateTag,
        onCreateSnippet: onCreateSnippet,
        onEditEnvironment: onEditHostEnvironment,
        onSave: onSaveHost,
        onConnect: onConnectHost,
        onDuplicate: onDuplicateHost,
        onDelete: onDeleteHost,
      ),
      _HostEnvironmentEditorRequest() => _HostEnvironmentEditorContent(
        request: request,
        onClose: onClose,
        onSave: onSaveHostEnvironment,
      ),
      _KeyEditorRequest(generate: true) => _GenerateKeyEditorContent(
        onClose: onClose,
        onSave: onSaveGeneratedKey,
      ),
      _KeyEditorRequest() => _KeyEditorContent(
        request: request,
        onClose: onClose,
        onSave: onSaveKey,
        onDuplicate: onDuplicateKey,
        onExportToHost: onExportKeyToHost,
        onExportToFile: onExportKeyToFile,
        onDelete: onDeleteKey,
      ),
      _KeyExportEditorRequest() => _KeyExportEditorContent(
        request: request,
        hosts: hosts,
        onClose: onClose,
        onExport: onExportKey,
        onShowNotification: onShowNotification,
      ),
      _IdentityEditorRequest() => _IdentityEditorContent(
        request: request,
        keys: keys,
        onClose: onClose,
        onCreateCredential: onCreateCredential,
        onSave: onSaveIdentity,
        onDuplicate: onDuplicateIdentity,
        onDelete: onDeleteIdentity,
      ),
      _PortForwardEditorRequest() => _PortForwardEditorContent(
        request: request,
        hosts: hosts,
        onClose: onClose,
        onSave: onSavePortForward,
        onDelete: onDeletePortForward,
      ),
      _ProxyEditorRequest() => _ProxyEditorContent(
        request: request,
        identities: identities,
        onClose: onClose,
        onCreateIdentity: onCreateIdentity,
        onSave: onSaveProxy,
        onDuplicate: onDuplicateProxy,
        onDelete: onDeleteProxy,
      ),
      _SnippetPackageEditorRequest() => _SnippetPackageEditorContent(
        request: request,
        onClose: onClose,
        onSave: onSaveSnippetPackage,
        onDelete: onDeleteSnippetPackage,
      ),
      _SnippetEditorRequest() => _SnippetEditorContent(
        request: request,
        packages: snippetPackages,
        groups: groups,
        hosts: hosts,
        onClose: onClose,
        onCreatePackage: onCreateSnippetPackage,
        onSave: onSaveSnippet,
        onDuplicate: onDuplicateSnippet,
        onDelete: onDeleteSnippet,
      ),
      _ShellHistoryDrawerRequest() => _ShellHistoryDrawerContent(
        entries: shellHistory,
        onClose: onClose,
        onSaveCommand: onCreateSnippetFromShellHistory,
        onClearAll: onClearShellHistory,
      ),
    };

    return Material(
      color: _surface,
      elevation: 18,
      shadowColor: const Color(0x26000000),
      shape: Border(left: BorderSide(color: _sidebarDivider)),
      child: KeyedSubtree(
        key: ValueKey(_workspaceEditorRequestKey(request)),
        child: content,
      ),
    );
  }
}

Object _workspaceEditorRequestKey(_WorkspaceEditorRequest request) {
  return switch (request) {
    _GroupEditorRequest(
      initial: final initial,
      initialParentId: final parentId,
    ) =>
      'group:${initial?.id ?? 'new:${parentId ?? 'root'}'}',
    _HostEditorRequest(initial: final initial, initialGroupId: final groupId) =>
      'host:${initial?.id ?? 'new:${groupId ?? 'root'}'}',
    _HostEnvironmentEditorRequest() => request,
    _KeyEditorRequest(
      initial: final initial,
      generate: final generate,
      certificateMode: final certificateMode,
      credentialCreation: final credentialCreation,
    ) =>
      'key:${initial?.id ?? (generate ? 'generate' : '${certificateMode ? 'certificate' : 'key'}:${credentialCreation ? 'credential' : 'direct'}')}',
    _KeyExportEditorRequest(key: final key) => 'key-export:${key.id}',
    _IdentityEditorRequest(initial: final initial) =>
      'identity:${initial?.id ?? 'new'}',
    _PortForwardEditorRequest(
      initial: final initial,
      initialType: final type,
    ) =>
      'forward:${initial?.id ?? 'new:$type'}',
    _ProxyEditorRequest(initial: final initial) =>
      'proxy:${initial?.id ?? 'new'}',
    _SnippetPackageEditorRequest(initial: final initial) =>
      'snippet-package:${initial?.id ?? 'new'}',
    _SnippetEditorRequest(
      initial: final initial,
      initialPackageId: final packageId,
      initialScript: final initialScript,
    ) =>
      'snippet:${initial?.id ?? 'new:${packageId ?? 'default'}:${initialScript ?? ''}'}',
    _ShellHistoryDrawerRequest() => 'shell-history',
  };
}

class _ShellHistoryDrawerContent extends StatelessWidget {
  const _ShellHistoryDrawerContent({
    required this.entries,
    required this.onClose,
    required this.onSaveCommand,
    required this.onClearAll,
  });

  final List<ShellHistoryEntry> entries;
  final VoidCallback onClose;
  final ValueChanged<String> onSaveCommand;
  final VoidCallback onClearAll;

  @override
  Widget build(BuildContext context) {
    final history = entries.toList(growable: false)
      ..sort(
        (a, b) => (b.createdAt ?? DateTime.fromMillisecondsSinceEpoch(0))
            .compareTo(a.createdAt ?? DateTime.fromMillisecondsSinceEpoch(0)),
      );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Container(
          height: _workspaceDrawerHeaderHeight,
          padding: const EdgeInsets.fromLTRB(20, 8, 12, 8),
          decoration: BoxDecoration(
            color: _card,
            border: Border(bottom: BorderSide(color: _sidebarDivider)),
          ),
          child: LayoutBuilder(
            builder: (context, constraints) {
              final showClose = constraints.maxWidth >= 28;
              final showMore = constraints.maxWidth >= 56;
              return Row(
                children: [
                  Expanded(
                    child: Text(
                      tr(
                        'workspace.label.shellHistory',
                        fallback: 'Shell History',
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: _text,
                        fontSize: NautermFontSizes.titleSmall,
                        fontWeight: NautermFontWeights.semibold,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                  if (showMore)
                    _WorkspaceDropdown<_ShellHistoryAction>(
                      width: 170,
                      entries: const [
                        NautermContextMenuAction<_ShellHistoryAction>(
                          value: _ShellHistoryAction.clearAll,
                          label: 'Clear all',
                          icon: LucideIcons.trash2,
                          destructive: true,
                        ),
                      ],
                      onSelected: (action) {
                        if (action == _ShellHistoryAction.clearAll) {
                          onClearAll();
                        }
                      },
                      triggerBuilder: (openMenu) =>
                          _WorkspaceDrawerHeaderButton(
                            icon: Icons.more_horiz_rounded,
                            tooltip: tr(
                              'workspace.label.more',
                              fallback: 'More',
                            ),
                            onPressed: openMenu,
                          ),
                    ),
                  if (showClose)
                    _WorkspaceDrawerHeaderButton(
                      icon: Icons.keyboard_tab_rounded,
                      tooltip: tr('common.action.close', fallback: 'Close'),
                      onPressed: onClose,
                    ),
                ],
              );
            },
          ),
        ),
        Expanded(
          child: ColoredBox(
            color: _card,
            child: history.isEmpty
                ? Center(
                    child: Padding(
                      padding: EdgeInsets.all(24),
                      child: Text(
                        tr(
                          'workspace.description.noShellHistoryRecordedYet',
                          fallback: 'No shell history recorded yet.',
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
                  )
                : ListView.builder(
                    padding: const EdgeInsets.symmetric(vertical: 10),
                    itemCount: history.length,
                    itemBuilder: (context, index) {
                      final entry = history[index];
                      return _ShellHistoryEntryRow(
                        command: entry.command,
                        onSave: () => onSaveCommand(entry.command),
                      );
                    },
                  ),
          ),
        ),
      ],
    );
  }
}

class _ShellHistoryEntryRow extends StatefulWidget {
  const _ShellHistoryEntryRow({required this.command, required this.onSave});

  final String command;
  final VoidCallback onSave;

  @override
  State<_ShellHistoryEntryRow> createState() => _ShellHistoryEntryRowState();
}

class _ShellHistoryEntryRowState extends State<_ShellHistoryEntryRow> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final hoverColor = _workspaceDark
        ? const Color(0xff27313c)
        : const Color(0xffedf4f5);
    return MouseRegion(
      onEnter: (_) {
        if (!_hovered) {
          setState(() => _hovered = true);
        }
      },
      onExit: (_) {
        if (_hovered) {
          setState(() => _hovered = false);
        }
      },
      child: Container(
        constraints: const BoxConstraints(minHeight: 48),
        color: _hovered ? hoverColor : _card,
        padding: const EdgeInsets.fromLTRB(20, 6, 20, 6),
        child: LayoutBuilder(
          builder: (context, constraints) {
            final command = Text(
              widget.command,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: _text,
                fontSize: NautermFontSizes.labelLarge,
                fontWeight: NautermFontWeights.medium,
                letterSpacing: 0,
              ),
            );
            if (constraints.maxWidth < 80) {
              return command;
            }
            return Stack(
              alignment: Alignment.centerRight,
              children: [
                SizedBox(width: double.infinity, child: command),
                if (_hovered)
                  ColoredBox(
                    color: hoverColor,
                    child: Padding(
                      padding: const EdgeInsets.only(left: 8),
                      child: _WorkspaceButton(
                        label: 'SAVE',
                        type: _WorkspaceButtonType.primary,
                        variant: _WorkspaceButtonVariant.solid,
                        size: _WorkspaceControlSize.tiny,
                        height: 26,
                        minWidth: 58,
                        horizontalPadding: 10,
                        onPressed: widget.onSave,
                      ),
                    ),
                  ),
              ],
            );
          },
        ),
      ),
    );
  }
}

bool _canUseGroupAsParent(List<HostGroup> groups, int? groupId) {
  if (groupId == null) {
    return false;
  }
  return _hostGroupDepth(groups, groupId) < 2;
}

bool _isDescendantGroup(
  List<HostGroup> groups,
  int? candidateId,
  int? ancestorId,
) {
  if (candidateId == null || ancestorId == null) {
    return false;
  }
  final byId = {for (final group in groups) group.id: group};
  final seen = <int>{};
  var current = byId[candidateId];
  while (current != null && current.id != null && seen.add(current.id!)) {
    if (current.parentId == ancestorId) {
      return true;
    }
    current = byId[current.parentId];
  }
  return false;
}

int _hostGroupDepth(List<HostGroup> groups, int groupId) {
  final byId = {for (final group in groups) group.id: group};
  final seen = <int>{};
  var depth = 0;
  HostGroup? current = byId[groupId];
  while (current != null && current.id != null && seen.add(current.id!)) {
    depth++;
    current = byId[current.parentId];
  }
  return depth;
}

String _hostGroupPathLabel(List<HostGroup> groups, HostGroup group) {
  final byId = {
    for (final candidate in groups)
      if (candidate.id != null) candidate.id!: candidate,
  };
  final names = <String>[];
  final seen = <int>{};
  HostGroup? current = group;
  while (current != null) {
    names.insert(0, current.name);
    final parentId = current.parentId;
    if (parentId == null || !seen.add(parentId)) {
      break;
    }
    current = byId[parentId];
  }
  return names.join(' > ');
}

class _EditorShell extends StatelessWidget {
  const _EditorShell({
    required this.title,
    required this.onClose,
    required this.onSave,
    required this.children,
    // Retained for drawers that need secondary header context in the future.
    // ignore: unused_element_parameter
    this.subtitle,
    this.saving = false,
    this.error,
    this.saveLabel = 'Save',
    this.savingLabel = 'Saving...',
    this.headerActions = const [],
  });

  final String title;
  final String? subtitle;
  final VoidCallback onClose;
  final VoidCallback onSave;
  final List<Widget> children;
  final bool saving;
  final String? error;
  final String saveLabel;
  final String savingLabel;
  final List<_EditorShellMenuAction> headerActions;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Container(
          height: _workspaceDrawerHeaderHeight,
          padding: const EdgeInsets.fromLTRB(16, 5, 8, 5),
          decoration: BoxDecoration(
            color: _card,
            border: Border(bottom: BorderSide(color: _sidebarDivider)),
          ),
          child: LayoutBuilder(
            builder: (context, constraints) {
              final showClose = constraints.maxWidth >= 28;
              final showMore =
                  headerActions.isNotEmpty && constraints.maxWidth >= 56;
              return Row(
                children: [
                  Expanded(
                    child: _WorkspaceDrawerHeaderTitle(
                      title: title,
                      subtitle: subtitle,
                    ),
                  ),
                  if (showMore)
                    _WorkspaceDropdown<_EditorShellMenuAction>(
                      width: 170,
                      entries: [
                        for (final action in headerActions)
                          NautermContextMenuAction<_EditorShellMenuAction>(
                            value: action,
                            label: action.label,
                            icon: action.icon,
                            destructive: action.destructive,
                          ),
                      ],
                      onSelected: (action) => action.onSelected(),
                      triggerBuilder: (openMenu) =>
                          _WorkspaceDrawerHeaderButton(
                            icon: Icons.more_horiz_rounded,
                            tooltip: tr(
                              'workspace.label.more',
                              fallback: 'More',
                            ),
                            onPressed: saving ? null : openMenu,
                          ),
                    ),
                  if (showClose)
                    _WorkspaceDrawerHeaderButton(
                      tooltip: tr('common.action.close', fallback: 'Close'),
                      onPressed: saving ? null : onClose,
                      icon: Icons.keyboard_tab_rounded,
                    ),
                ],
              );
            },
          ),
        ),
        Expanded(
          child: _WorkspaceControlSizeScope(
            size: _WorkspaceControlSize.large,
            child: ListView(
              key: const ValueKey('workspace-editor-scroll-view'),
              padding: const EdgeInsets.all(16),
              children: [
                ...children,
                if (error != null) ...[
                  SizedBox(height: 12),
                  Text(
                    error!,
                    style: TextStyle(
                      color: Color(0xffe5453d),
                      fontSize: NautermFontSizes.labelMedium,
                      fontWeight: NautermFontWeights.medium,
                      letterSpacing: 0,
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
        Container(
          padding: const EdgeInsets.fromLTRB(16, 10, 16, 16),
          decoration: BoxDecoration(
            color: _surface,
            border: Border(top: BorderSide(color: _sidebarDivider)),
          ),
          child: SizedBox(
            height: 38,
            child: FilledButton(
              style: FilledButton.styleFrom(
                backgroundColor: _blue,
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(8),
                ),
              ),
              onPressed: saving ? null : onSave,
              child: Text(
                tr(saving ? savingLabel : saveLabel),
                style: TextStyle(
                  fontSize: NautermFontSizes.labelLarge,
                  fontWeight: NautermFontWeights.medium,
                  letterSpacing: 0,
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _EditorShellMenuAction {
  const _EditorShellMenuAction({
    required this.label,
    required this.icon,
    required this.onSelected,
    this.destructive = false,
  });

  factory _EditorShellMenuAction.connect(VoidCallback onSelected) {
    return _EditorShellMenuAction(
      label: 'Connect',
      icon: Icons.play_arrow_rounded,
      onSelected: onSelected,
    );
  }

  factory _EditorShellMenuAction.duplicate(VoidCallback onSelected) {
    return _EditorShellMenuAction(
      label: 'Duplicate',
      icon: Icons.copy_rounded,
      onSelected: onSelected,
    );
  }

  factory _EditorShellMenuAction.exportToHost(VoidCallback onSelected) {
    return _EditorShellMenuAction(
      label: 'Export to Host',
      icon: Icons.upload_rounded,
      onSelected: onSelected,
    );
  }

  factory _EditorShellMenuAction.exportToFile(VoidCallback onSelected) {
    return _EditorShellMenuAction(
      label: 'Export to File',
      icon: Icons.save_alt_rounded,
      onSelected: onSelected,
    );
  }

  factory _EditorShellMenuAction.delete(VoidCallback onSelected) {
    return _EditorShellMenuAction(
      label: 'Delete',
      icon: LucideIcons.trash2,
      onSelected: onSelected,
      destructive: true,
    );
  }

  final String label;
  final IconData icon;
  final VoidCallback onSelected;
  final bool destructive;
}

enum _ShellHistoryAction { clearAll }

class _WorkspaceDrawerHeaderButton extends StatelessWidget {
  const _WorkspaceDrawerHeaderButton({
    required this.icon,
    required this.tooltip,
    required this.onPressed,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceButton(
      icon: icon,
      tooltip: tooltip,
      size: _WorkspaceControlSize.tiny,
      variant: _WorkspaceButtonVariant.text,
      minWidth: 28,
      height: 28,
      onPressed: onPressed,
    );
  }
}

class _WorkspaceDrawerHeaderTitle extends StatelessWidget {
  const _WorkspaceDrawerHeaderTitle({required this.title, this.subtitle});

  final String title;
  final String? subtitle;

  @override
  Widget build(BuildContext context) {
    final subtitle = this.subtitle?.trim();
    final subtitleText = subtitle == null || subtitle.isEmpty
        ? null
        : Text(
            tr(subtitle),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            strutStyle: const StrutStyle(
              fontSize: NautermFontSizes.labelSmall,
              height: 1,
              forceStrutHeight: true,
            ),
            style: TextStyle(
              color: _mutedText,
              fontSize: NautermFontSizes.labelSmall,
              height: 1,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          );

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          tr(title),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          strutStyle: const StrutStyle(
            fontSize: NautermFontSizes.titleSmall,
            height: 1,
            forceStrutHeight: true,
          ),
          style: TextStyle(
            color: _text,
            fontSize: NautermFontSizes.titleSmall,
            height: 1,
            fontWeight: NautermFontWeights.semibold,
            letterSpacing: 0,
          ),
        ),
        ?subtitleText,
      ],
    );
  }
}
