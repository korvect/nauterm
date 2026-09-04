part of 'nauterm_workspace.dart';

class _KeychainPane extends StatefulWidget {
  const _KeychainPane({
    required this.keys,
    required this.identities,
    required this.onCreateKey,
    required this.onCreateCertificate,
    required this.onCreateFido2Key,
    required this.onGenerateKey,
    required this.onCreateIdentity,
    required this.onKeyContextAction,
    required this.onKeyContextActions,
    required this.onIdentityContextAction,
    required this.onIdentityContextActions,
  });

  final List<_KeyItem> keys;
  final List<_IdentityItem> identities;
  final VoidCallback onCreateKey;
  final VoidCallback onCreateCertificate;
  final VoidCallback onCreateFido2Key;
  final VoidCallback onGenerateKey;
  final VoidCallback onCreateIdentity;
  final _WorkspaceContextAction<_KeyItem> onKeyContextAction;
  final _WorkspaceContextActions<_KeyItem> onKeyContextActions;
  final _WorkspaceContextAction<_IdentityItem> onIdentityContextAction;
  final _WorkspaceContextActions<_IdentityItem> onIdentityContextActions;

  @override
  State<_KeychainPane> createState() => _KeychainPaneState();
}

class _KeychainPaneState extends State<_KeychainPane> {
  final _keyNavigation = _WorkspaceItemCollectionNavigationController();
  final _identityNavigation = _WorkspaceItemCollectionNavigationController();
  _WorkspaceSortOrder _sortOrder = _WorkspaceSortOrder.nameAscending;
  _WorkspaceViewMode _viewMode = _WorkspaceViewMode.grid;
  String _searchQuery = '';

  @override
  Widget build(BuildContext context) {
    final keys = _sortWorkspaceItems(
      _filterWorkspaceItems(
        widget.keys,
        _searchQuery,
        extraText: (key) => [
          key.privateKey ?? '',
          key.publicKey ?? '',
          key.certificate ?? '',
        ],
      ),
      _sortOrder,
      ordinal: (key) => key.id,
    );
    final identities = _sortWorkspaceItems(
      _filterWorkspaceItems(
        widget.identities,
        _searchQuery,
        extraText: (identity) => [
          identity.username ?? '',
          if (identity.keyId != null) '${identity.keyId}',
        ],
      ),
      _sortOrder,
      ordinal: (identity) => identity.id,
    );
    final hasStoredItems =
        widget.keys.isNotEmpty || widget.identities.isNotEmpty;
    final showEmptyState = !hasStoredItems && _searchQuery.trim().isEmpty;
    final sharedGridItemCount = math.max(keys.length, identities.length);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _KeychainToolbar(
          onCreateKey: widget.onCreateKey,
          onCreateCertificate: widget.onCreateCertificate,
          onCreateFido2Key: widget.onCreateFido2Key,
          onGenerateKey: widget.onGenerateKey,
          onCreateIdentity: widget.onCreateIdentity,
          sortOrder: _sortOrder,
          onSortOrderChanged: (value) => setState(() => _sortOrder = value),
          searchQuery: _searchQuery,
          onSearchQueryChanged: (value) => setState(() => _searchQuery = value),
          viewMode: _viewMode,
          onViewModeChanged: (value) => setState(() => _viewMode = value),
        ),
        Expanded(
          child: showEmptyState
              ? _WorkspaceEmptyState(
                  icon: LucideIcons.key,
                  title: 'No keychain items yet',
                  description: 'Add keys and identities once, then reuse them across your saved hosts.',
                  actionLabel: 'Add key',
                  onAction: widget.onCreateKey,
                )
              : SingleChildScrollView(
                  padding: _workspacePanePadding,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      if (keys.isNotEmpty) ...[
                        const _SectionTitle('Keys'),
                        SizedBox(height: 12),
                        _WorkspaceItemCollection(
                          items: keys,
                          viewMode: _viewMode,
                          maxColumns: 2,
                          layoutItemCount: sharedGridItemCount,
                          navigationController: _keyNavigation,
                          onNavigateDown: identities.isEmpty
                              ? null
                              : _identityNavigation.focusFirst,
                          onContextAction: widget.onKeyContextAction,
                          onContextActions: widget.onKeyContextActions,
                        ),
                      ],
                      if (keys.isNotEmpty && identities.isNotEmpty)
                        SizedBox(height: 28),
                      if (identities.isNotEmpty) ...[
                        const _SectionTitle('Identities'),
                        SizedBox(height: 12),
                        _WorkspaceItemCollection(
                          items: identities,
                          viewMode: _viewMode,
                          maxColumns: 2,
                          layoutItemCount: sharedGridItemCount,
                          navigationController: _identityNavigation,
                          onNavigateUp: keys.isEmpty
                              ? null
                              : _keyNavigation.focusLast,
                          onContextAction: widget.onIdentityContextAction,
                          onContextActions: widget.onIdentityContextActions,
                        ),
                      ],
                      if (keys.isEmpty && identities.isEmpty)
                        const _WorkspaceInlineMessage(
                          'No keychain items found.',
                        ),
                    ],
                  ),
                ),
        ),
      ],
    );
  }
}

class _KeychainToolbar extends StatelessWidget {
  const _KeychainToolbar({
    required this.onCreateKey,
    required this.onCreateCertificate,
    required this.onCreateFido2Key,
    required this.onGenerateKey,
    required this.onCreateIdentity,
    required this.sortOrder,
    required this.onSortOrderChanged,
    required this.searchQuery,
    required this.onSearchQueryChanged,
    required this.viewMode,
    required this.onViewModeChanged,
  });

  final VoidCallback onCreateKey;
  final VoidCallback onCreateCertificate;
  final VoidCallback onCreateFido2Key;
  final VoidCallback onGenerateKey;
  final VoidCallback onCreateIdentity;
  final _WorkspaceSortOrder sortOrder;
  final ValueChanged<_WorkspaceSortOrder> onSortOrderChanged;
  final String searchQuery;
  final ValueChanged<String> onSearchQueryChanged;
  final _WorkspaceViewMode viewMode;
  final ValueChanged<_WorkspaceViewMode> onViewModeChanged;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceToolbarFrame(
      children: [
        _KeychainCreateButton(
          onCreateKey: onCreateKey,
          onGenerateKey: onGenerateKey,
          onCreateIdentity: onCreateIdentity,
        ),
        SizedBox(width: 10),
        _ModeButton(
          icon: Icons.workspace_premium_rounded,
          label: 'Certificate',
          onTap: onCreateCertificate,
        ),
        SizedBox(width: 10),
        _ModeButton(
          icon: Icons.usb_rounded,
          label: 'FIDO2',
          onTap: onCreateFido2Key,
        ),
        Expanded(
          child: _ToolbarTrailingActions(
            children: _defaultToolbarTrailingActions(
              sortOrder: sortOrder,
              onSortOrderChanged: onSortOrderChanged,
              searchQuery: searchQuery,
              onSearchQueryChanged: onSearchQueryChanged,
              viewMode: viewMode,
              onViewModeChanged: onViewModeChanged,
            ),
          ),
        ),
      ],
    );
  }
}

enum _KeychainCreateAction { generateKey, identity }

class _KeychainCreateButton extends StatelessWidget {
  const _KeychainCreateButton({
    required this.onCreateKey,
    required this.onGenerateKey,
    required this.onCreateIdentity,
  });

  final VoidCallback onCreateKey;
  final VoidCallback onGenerateKey;
  final VoidCallback onCreateIdentity;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceDropdown<_KeychainCreateAction>(
      width: 190,
      entries: [
        NautermContextMenuAction<_KeychainCreateAction>(
          value: _KeychainCreateAction.generateKey,
          icon: Icons.auto_fix_high_rounded,
          label: tr('workspace.label.generateKey', fallback: 'Generate key'),
        ),
        NautermContextMenuAction<_KeychainCreateAction>(
          value: _KeychainCreateAction.identity,
          icon: Icons.badge_rounded,
          label: tr('workspace.label.newIdentity', fallback: 'New identity'),
        ),
      ],
      onSelected: (selected) {
        switch (selected) {
          case _KeychainCreateAction.generateKey:
            onGenerateKey();
          case _KeychainCreateAction.identity:
            onCreateIdentity();
        }
      },
      triggerBuilder: (openMenu) => _ToolbarSplitButton(
        icon: Icons.add_rounded,
        label: tr('workspace.label.newKey', fallback: 'New key'),
        onPrimaryPressed: onCreateKey,
        onSecondaryPressed: (_) => openMenu(),
      ),
    );
  }
}
