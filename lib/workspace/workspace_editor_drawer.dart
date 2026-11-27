part of 'nauterm_workspace.dart';

typedef _BuildDrawerHostSelector = Widget Function(
  TextEditingController search,
  VoidCallback onBack,
  ValueChanged<int> onSelected,
);

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
typedef _ExportKey = Future<void> Function(
  KeyEntry key,
  _KeyExportDraft draft,
  TerminalController connection,
);
typedef _CreateKeyExportConnection = TerminalController Function(int hostId);
typedef _BuildKeyExportConnectionPage = Widget Function(
  TerminalController connection,
  VoidCallback onClose,
);
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
typedef _CreateRelatedGroup = void Function(
  String initialName,
  ValueChanged<HostGroup> onCreated, {
  required bool rootOnly,
});
typedef _CreateGroupFromProtocol = void Function(HostGroup template);
typedef _CreateRelatedCredential = void Function(
  String initialName, {
  required bool certificate,
  required ValueChanged<int> onCreated,
});

class _WorkspaceEditorStackEntry {
  _WorkspaceEditorStackEntry({
    required this.request,
    this.onSaved,
    _WorkspaceEditorDraft? draft,
  }) : draft = draft ?? _WorkspaceEditorDraft();

  final _WorkspaceEditorRequest request;
  final ValueChanged<Object>? onSaved;
  final _WorkspaceEditorDraft draft;
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
    this.rootOnly = false,
  });

  final HostGroup? initial;
  final HostGroup? template;
  final int? initialParentId;
  final String? initialName;
  final bool rootOnly;
}

class _HostEditorRequest extends _WorkspaceEditorRequest {
  const _HostEditorRequest({this.initial, this.initialGroupId});

  final HostEntry? initial;
  final int? initialGroupId;
}

class _LoadHostEditorRequest extends _WorkspaceEditorRequest {
  _LoadHostEditorRequest({required this.host, required this.databasePath});

  final _HostItem host;
  final String databasePath;
  Future<HostEntry?>? _pending;
  HostEntry? details;

  Future<HostEntry?> load({bool retry = false}) {
    final pending = _pending;
    if (!retry && pending != null) return pending;
    late final Future<HostEntry?> request;
    request = loadNautermHostDetails(databasePath, host.id).then((value) {
      final match = host.uuid == null || value?.uuid == host.uuid
          ? value
          : null;
      if (identical(_pending, request)) details = match;
      return match;
    });
    return _pending = request;
  }
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

class _Fido2KeyEditorRequest extends _WorkspaceEditorRequest {
  const _Fido2KeyEditorRequest();
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
    required this.draft,
    required this.onDiscard,
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
    required this.createKeyExportConnection,
    required this.buildKeyExportConnectionPage,
    required this.buildHostSelector,
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
    required this.onReloadShellHistory,
  });

  final _WorkspaceEditorRequest? request;
  final _WorkspaceEditorDraft draft;
  final VoidCallback onDiscard;
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
  final _CreateRelatedGroup onCreateGroup;
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
  final _CreateKeyExportConnection createKeyExportConnection;
  final _BuildKeyExportConnectionPage buildKeyExportConnectionPage;
  final _BuildDrawerHostSelector buildHostSelector;
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
  final Future<void> Function() onReloadShellHistory;

  Widget _buildHostEditor(_HostEditorRequest request) => _HostEditorContent(
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
    onSave: (value) async {
      await onSaveHost(value);
      draft.discard();
    },
    onConnect: onConnectHost,
    onDuplicate: onDuplicateHost,
    onDelete: onDeleteHost,
  );

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
        onCreateGroup: onCreateGroup,
        onCreateCredential: onCreateCredential,
        onCreateIdentity: onCreateIdentity,
        onCreateSnippet: onCreateSnippet,
        onEditEnvironment: onEditHostEnvironment,
        onClose: onClose,
        onSave: (value) async {
          await onSaveGroup(value);
          draft.discard();
        },
        onDuplicate: onDuplicateGroup,
        onDelete: onDeleteGroup,
      ),
      _HostEditorRequest() => _buildHostEditor(request),
      _LoadHostEditorRequest() => _HostEditorLoader(
        request: request,
        onClose: onClose,
        builder: (host) => _buildHostEditor(_HostEditorRequest(initial: host)),
      ),
      _HostEnvironmentEditorRequest() => _HostEnvironmentEditorContent(
        request: request,
        onClose: onClose,
        onSave: (value) {
          onSaveHostEnvironment(value);
          draft.discard();
        },
      ),
      _KeyEditorRequest(generate: true) => _GenerateKeyEditorContent(
        onClose: onClose,
        onSave: (value) async {
          await onSaveGeneratedKey(value);
          draft.discard();
        },
      ),
      _KeyEditorRequest() => _KeyEditorContent(
        request: request,
        onClose: onClose,
        onSave: (value) async {
          await onSaveKey(value);
          draft.discard();
        },
        onDuplicate: onDuplicateKey,
        onExportToHost: onExportKeyToHost,
        onExportToFile: onExportKeyToFile,
        onDelete: onDeleteKey,
      ),
      _Fido2KeyEditorRequest() => _Fido2KeyEditorContent(
        generation: draft.fido2Key ??= _Fido2KeyDraft(),
        onClose: onClose,
        onSave: (value) async {
          await onSaveGeneratedKey(value);
          draft.discard();
        },
      ),
      _KeyExportEditorRequest() => _KeyExportEditorContent(
        request: request,
        hosts: hosts,
        onClose: onClose,
        onExport: onExportKey,
        createConnection: createKeyExportConnection,
        buildConnectionPage: buildKeyExportConnectionPage,
        buildHostSelector: buildHostSelector,
        onShowNotification: onShowNotification,
      ),
      _IdentityEditorRequest() => _IdentityEditorContent(
        request: request,
        keys: keys,
        onClose: onClose,
        onCreateCredential: onCreateCredential,
        onSave: (value) async {
          await onSaveIdentity(value);
          draft.discard();
        },
        onDuplicate: onDuplicateIdentity,
        onDelete: onDeleteIdentity,
      ),
      _PortForwardEditorRequest() => _PortForwardEditorContent(
        request: request,
        hosts: hosts,
        onClose: onClose,
        onSave: (value) async {
          await onSavePortForward(value);
          draft.discard();
        },
        onDelete: onDeletePortForward,
      ),
      _ProxyEditorRequest() => _ProxyEditorContent(
        request: request,
        identities: identities,
        onClose: onClose,
        onCreateIdentity: onCreateIdentity,
        onSave: (value) async {
          await onSaveProxy(value);
          draft.discard();
        },
        onDuplicate: onDuplicateProxy,
        onDelete: onDeleteProxy,
      ),
      _SnippetPackageEditorRequest() => _SnippetPackageEditorContent(
        request: request,
        onClose: onClose,
        onSave: (value) async {
          await onSaveSnippetPackage(value);
          draft.discard();
        },
        onDelete: onDeleteSnippetPackage,
      ),
      _SnippetEditorRequest() => _SnippetEditorContent(
        request: request,
        packages: snippetPackages,
        groups: groups,
        hosts: hosts,
        onClose: onClose,
        onCreatePackage: onCreateSnippetPackage,
        onSave: (initial, value) async {
          await onSaveSnippet(initial, value);
          draft.discard();
        },
        onDuplicate: onDuplicateSnippet,
        onDelete: onDeleteSnippet,
      ),
      _ShellHistoryDrawerRequest() => _ShellHistoryDrawerContent(
        entries: shellHistory,
        onClose: onClose,
        onSaveCommand: onCreateSnippetFromShellHistory,
        onClearAll: onClearShellHistory,
        onReload: onReloadShellHistory,
      ),
    };

    return _WorkspaceEditorDraftScope(
      key: ObjectKey(draft),
      draft: draft,
      onDiscard: onDiscard,
      child: Material(
        color: _surface,
        elevation: 18,
        shadowColor: const Color(0x26000000),
        shape: Border(left: BorderSide(color: _sidebarDivider)),
        child: KeyedSubtree(
          key: ValueKey(_workspaceEditorRequestKey(request)),
          child: content,
        ),
      ),
    );
  }
}

Object _workspaceEditorRequestKey(_WorkspaceEditorRequest request) {
  return switch (request) {
    _GroupEditorRequest(
      initial: final initial,
      initialParentId: final parentId,
      rootOnly: final rootOnly,
    ) =>
      rootOnly
          ? 'group:new:parent'
          : 'group:${initial?.id ?? 'new:${parentId ?? 'root'}'}',
    _HostEditorRequest(initial: final initial, initialGroupId: final groupId) =>
      'host:${initial?.id ?? 'new:${groupId ?? 'root'}'}',
    _LoadHostEditorRequest(host: final host) => 'host:${host.id}',
    _HostEnvironmentEditorRequest() => request,
    _KeyEditorRequest(
      initial: final initial,
      generate: final generate,
      certificateMode: final certificateMode,
      credentialCreation: final credentialCreation,
    ) =>
      'key:${initial?.id ?? (generate ? 'generate' : '${certificateMode ? 'certificate' : 'key'}:${credentialCreation ? 'credential' : 'direct'}')}',
    _KeyExportEditorRequest(key: final key) => 'key-export:${key.id}',
    _Fido2KeyEditorRequest() => 'fido2-key:new',
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

class _HostEditorLoader extends StatefulWidget {
  const _HostEditorLoader({
    required this.request,
    required this.onClose,
    required this.builder,
  });

  final _LoadHostEditorRequest request;
  final VoidCallback onClose;
  final Widget Function(HostEntry) builder;

  @override
  State<_HostEditorLoader> createState() => _HostEditorLoaderState();
}

class _HostEditorLoaderState extends State<_HostEditorLoader> {
  late Future<HostEntry?> _host = widget.request.load();

  @override
  void didUpdateWidget(covariant _HostEditorLoader oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(oldWidget.request, widget.request)) {
      _host = widget.request.load();
    }
  }

  @override
  Widget build(BuildContext context) {
    if (widget.request.details case final details?) {
      return widget.builder(details);
    }
    return FutureBuilder<HostEntry?>(
      future: _host,
      builder: (context, snapshot) {
        final host = snapshot.data;
        if (snapshot.connectionState == ConnectionState.done && host != null) {
          return widget.builder(host);
        }
        final loading = snapshot.connectionState != ConnectionState.done;
        final item = widget.request.host;
        final message = loading
            ? item.sshEnabled || item.telnetEnabled || item.moshEnabled
                  ? context.tr(
                      'workspace.host.credentials.loading',
                      fallback: 'Loading password…',
                    )
                  : context.tr('common.status.loading', fallback: 'Loading…')
            : snapshot.error?.toString() ??
                  context.tr(
                    'workspace.host.error.unavailable',
                    fallback: 'Host is not available.',
                  );
        return _EditorShell(
          title: context.tr('workspace.label.editHost', fallback: 'Edit Host'),
          onClose: widget.onClose,
          onSave: null,
          body: Center(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(20),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  if (loading) ...[
                    const SizedBox.square(
                      dimension: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                    const SizedBox(height: 12),
                  ],
                  Text(
                    message,
                    textAlign: TextAlign.center,
                    style: TextStyle(color: _mutedText),
                  ),
                  if (!loading)
                    TextButton(
                      onPressed: () => setState(
                        () => _host = widget.request.load(retry: true),
                      ),
                      child: Text(
                        context.tr('common.action.retry', fallback: 'Retry'),
                      ),
                    ),
                ],
              ),
            ),
          ),
          children: const [],
        );
      },
    );
  }
}

class _ShellHistoryDrawerContent extends StatefulWidget {
  const _ShellHistoryDrawerContent({
    required this.entries,
    required this.onClose,
    required this.onSaveCommand,
    required this.onClearAll,
    required this.onReload,
  });

  final List<ShellHistoryEntry> entries;
  final VoidCallback onClose;
  final ValueChanged<String> onSaveCommand;
  final VoidCallback onClearAll;
  final Future<void> Function() onReload;

  @override
  State<_ShellHistoryDrawerContent> createState() =>
      _ShellHistoryDrawerContentState();
}

class _ShellHistoryDrawerContentState
    extends State<_ShellHistoryDrawerContent> {
  late Future<void> _load = widget.onReload();

  @override
  Widget build(BuildContext context) =>
      FutureBuilder<void>(future: _load, builder: _buildHistory);

  Widget _buildHistory(BuildContext context, AsyncSnapshot<void> snapshot) {
    final loading = snapshot.connectionState != ConnectionState.done;
    final error = loading ? null : snapshot.error?.toString();
    final errorColor = Theme.of(context).colorScheme.error;
    final history = widget.entries.toList(growable: false)
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
              final showRetry = error != null && constraints.maxWidth >= 84;
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
                  if (showRetry)
                    _WorkspaceDrawerHeaderButton(
                      icon: LucideIcons.refreshCw,
                      tooltip: context.tr(
                        'common.action.retry',
                        fallback: 'Retry',
                      ),
                      onPressed: () =>
                          setState(() => _load = widget.onReload()),
                    ),
                  if (showMore)
                    _WorkspaceDropdown<_ShellHistoryAction>(
                      width: 170,
                      entries: const [
                        NautermContextMenuAction<_ShellHistoryAction>(
                          value: _ShellHistoryAction.clearAll,
                          label: 'workspace.label.clearAll',
                          icon: LucideIcons.trash2,
                          destructive: true,
                        ),
                      ],
                      onSelected: (action) {
                        if (action == _ShellHistoryAction.clearAll) {
                          widget.onClearAll();
                        }
                      },
                      triggerBuilder: (openMenu) =>
                          _WorkspaceDrawerHeaderButton(
                            icon: Icons.more_horiz_rounded,
                            tooltip: tr(
                              'workspace.label.more',
                              fallback: 'More',
                            ),
                            onPressed: loading ? null : openMenu,
                          ),
                    ),
                  if (showClose)
                    _WorkspaceDrawerHeaderButton(
                      icon: Icons.keyboard_tab_rounded,
                      tooltip: tr('common.action.close', fallback: 'Close'),
                      onPressed: widget.onClose,
                    ),
                ],
              );
            },
          ),
        ),
        if (error != null && history.isNotEmpty)
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 12, 20, 0),
            child: Text(
              error,
              maxLines: 3,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: errorColor),
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
                        error ??
                            (loading
                                ? context.tr(
                                    'workspace.history.loading',
                                    fallback: 'Loading shell history…',
                                  )
                                : context.tr(
                                    'workspace.description.noShellHistoryRecordedYet',
                                    fallback: 'No shell history recorded yet.',
                                  )),
                        textAlign: TextAlign.center,
                        style: TextStyle(
                          color: error == null ? _mutedText : errorColor,
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
                        onSave: () => widget.onSaveCommand(entry.command),
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
    this.body,
    // Retained for drawers that need secondary header context in the future.
    // ignore: unused_element_parameter
    this.subtitle,
    this.saving = false,
    this.error,
    this.saveLabel = 'Save',
    this.savingLabel = 'Saving…',
    this.headerActions = const [],
  });

  final String title;
  final String? subtitle;
  final VoidCallback onClose;
  final VoidCallback? onSave;
  final List<Widget> children;
  final Widget? body;
  final bool saving;
  final String? error;
  final String saveLabel;
  final String savingLabel;
  final List<_EditorShellMenuAction> headerActions;

  @override
  Widget build(BuildContext context) {
    final draftScope = _WorkspaceEditorDraftScope.maybeOf(context);
    final discard = draftScope == null
        ? null
        : _EditorShellMenuAction(
            label: tr(
              'workspace.action.discardChanges',
              fallback: 'Discard changes',
            ),
            icon: Icons.undo_rounded,
            onSelected: () {
              if (draftScope.draft.hasChanges) draftScope.onDiscard();
            },
          );
    final actions = [...headerActions, ?discard];

    Widget buildMore() {
      return _WorkspaceDropdown<_EditorShellMenuAction>(
        width: 170,
        entries: [
          for (final action in actions)
            NautermContextMenuAction<_EditorShellMenuAction>(
              value: action,
              label: action.label,
              icon: action.icon,
              destructive: action.destructive,
              enabled:
                  action != discard || (draftScope?.draft.hasChanges ?? false),
            ),
        ],
        onSelected: (action) => action.onSelected(),
        triggerBuilder: (openMenu) => _WorkspaceDrawerHeaderButton(
          icon: Icons.more_horiz_rounded,
          tooltip: tr('workspace.label.more', fallback: 'More'),
          onPressed: saving ? null : openMenu,
        ),
      );
    }

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
              final showMore = actions.isNotEmpty && constraints.maxWidth >= 56;
              return Row(
                children: [
                  Expanded(
                    child: _WorkspaceDrawerHeaderTitle(
                      title: title,
                      subtitle: subtitle,
                    ),
                  ),
                  if (showMore)
                    if (draftScope == null)
                      buildMore()
                    else
                      ListenableBuilder(
                        listenable: draftScope.draft,
                        builder: (context, _) => buildMore(),
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
          child:
              body ??
              _WorkspaceControlSizeScope(
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
        if (onSave != null)
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
      label: 'workspace.label.exportToFile',
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
    final draft = _WorkspaceEditorDraftScope.maybeOf(context)?.draft;
    if (draft == null) return _buildTitle(context, unsaved: false);
    return ListenableBuilder(
      listenable: draft,
      builder: (context, _) => _buildTitle(context, unsaved: draft.hasChanges),
    );
  }

  Widget _buildTitle(BuildContext context, {required bool unsaved}) {
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
        Row(
          children: [
            Flexible(
              flex: 3,
              child: Text(
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
            ),
            if (unsaved) ...[
              const SizedBox(width: 8),
              Flexible(
                child: Text(
                  tr('workspace.label.unsaved', fallback: 'Unsaved'),
                  key: const ValueKey('workspace-editor-unsaved'),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: _mutedText,
                    fontSize: 10,
                    fontWeight: NautermFontWeights.medium,
                    height: 1,
                    letterSpacing: 0,
                  ),
                ),
              ),
            ],
          ],
        ),
        ?subtitleText,
      ],
    );
  }
}
