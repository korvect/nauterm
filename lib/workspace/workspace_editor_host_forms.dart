part of 'nauterm_workspace.dart';

class _GroupEditorContent extends StatefulWidget {
  const _GroupEditorContent({
    required this.request,
    required this.groups,
    required this.snippets,
    required this.keys,
    required this.identities,
    required this.proxies,
    required this.terminalThemeCatalog,
    required this.onCreateCredential,
    required this.onCreateIdentity,
    required this.onCreateSnippet,
    required this.onEditEnvironment,
    required this.onClose,
    required this.onSave,
    required this.onDuplicate,
    required this.onDelete,
  });

  final _GroupEditorRequest request;
  final List<HostGroup> groups;
  final List<_SnippetItem> snippets;
  final List<KeyEntry> keys;
  final List<IdentityEntry> identities;
  final List<ProxyEntry> proxies;
  final TerminalThemeCatalog? terminalThemeCatalog;
  final _CreateRelatedCredential onCreateCredential;
  final _CreateRelatedEntry onCreateIdentity;
  final _CreateRelatedEntry onCreateSnippet;
  final _EditHostEnvironment onEditEnvironment;
  final VoidCallback onClose;
  final _SaveGroup onSave;
  final ValueChanged<HostGroup> onDuplicate;
  final ValueChanged<HostGroup> onDelete;

  @override
  State<_GroupEditorContent> createState() => _GroupEditorContentState();
}

class _GroupEditorContentState extends State<_GroupEditorContent> {
  late final TextEditingController _nameController;
  late final TextEditingController _portController;
  late final TextEditingController _usernameController;
  late final TextEditingController _passwordController;
  late final TextEditingController _moshCommandController;
  late final TextEditingController _telnetPortController;
  late final TextEditingController _telnetUsernameController;
  late final TextEditingController _telnetPasswordController;
  int? _parentId;
  int? _startupSnippetId;
  int? _identityId;
  int? _proxyId;
  int? _keyId;
  int? _telnetIdentityId;
  _SshCredentialKind? _sshCredentialKind;
  bool _sshEnabled = false;
  bool _moshEnabled = false;
  bool _telnetEnabled = false;
  bool _sshConfigured = false;
  bool _moshConfigured = false;
  bool _telnetConfigured = false;
  bool _portConfigured = false;
  bool _telnetPortConfigured = false;
  bool _encodingConfigured = false;
  bool _telnetEncodingConfigured = false;
  bool _sshShowMore = false;
  String _encoding = defaultHostEncoding;
  String _telnetEncoding = defaultHostEncoding;
  String? _themeId;
  String? _telnetThemeId;
  late List<HostEnvironmentVariable> _environmentVariables;
  StoredTerminalTheme? _selectedTheme;
  StoredTerminalTheme? _selectedTelnetTheme;
  Future<List<StoredTerminalTheme>>? _themeGalleryFuture;
  bool _themeGalleryOpen = false;
  _HostThemeTarget _themeGalleryTarget = _HostThemeTarget.ssh;
  bool _saving = false;
  String? _error;
  String? _nameError;

  @override
  void initState() {
    super.initState();
    final initial = widget.request.initial ?? widget.request.template;
    _nameController = TextEditingController(
      text: initial?.name ?? widget.request.initialName ?? '',
    );
    _nameController.addListener(_clearNameError);
    _portController = TextEditingController(text: '${initial?.port ?? 22}');
    _usernameController = TextEditingController(text: initial?.username ?? '');
    _passwordController = TextEditingController(text: initial?.password ?? '');
    _moshCommandController = TextEditingController(
      text: initial?.moshServerCommand ?? defaultMoshServerCommand,
    );
    _telnetPortController = TextEditingController(
      text: '${initial?.telnetPort ?? 23}',
    );
    _telnetUsernameController = TextEditingController(
      text: initial?.telnetUsername ?? '',
    );
    _telnetPasswordController = TextEditingController(
      text: initial?.telnetPassword ?? '',
    );
    _parentId = initial?.parentId ?? widget.request.initialParentId;
    _startupSnippetId = initial?.startupSnippetId;
    _identityId = initial?.identityId;
    _proxyId = initial?.proxyId;
    _keyId = initial?.keyId;
    _telnetIdentityId = initial?.telnetIdentityId;
    _sshEnabled = initial?.sshEnabled ?? false;
    _moshEnabled = initial?.moshEnabled ?? false;
    _telnetEnabled = initial?.telnetEnabled ?? false;
    _sshConfigured = initial?.sshEnabled != null;
    _moshConfigured = initial?.moshEnabled != null;
    _telnetConfigured = initial?.telnetEnabled != null;
    _portConfigured = initial?.port != null;
    _telnetPortConfigured = initial?.telnetPort != null;
    _encodingConfigured = initial?.encoding?.trim().isNotEmpty == true;
    _telnetEncodingConfigured =
        initial?.telnetEncoding?.trim().isNotEmpty == true;
    _encoding = _normalizeHostEncoding(initial?.encoding);
    _telnetEncoding = _normalizeHostEncoding(initial?.telnetEncoding);
    _themeId = normalizeTerminalThemeId(initial?.themeId);
    _telnetThemeId = normalizeTerminalThemeId(initial?.telnetThemeId);
    _environmentVariables = List.of(initial?.environmentVariables ?? const []);
    _sshCredentialKind = _credentialKindForKeyId(widget.keys, _keyId);
    if (_parentId != null && !_canUseGroupAsParent(widget.groups, _parentId)) {
      _parentId = null;
    }
    if (initial == null && _parentId != null) {
      final parent = widget.groups
          .where((group) => group.id == _parentId)
          .firstOrNull;
      if (parent != null) {
        _mergeSettingsFromGroup(parent);
      }
    }
    _showSelectedIdentityName();
    _loadSelectedTheme();
    _loadSelectedTelnetTheme();
  }

  Future<void> _loadSelectedTheme() async {
    final themeId = _themeId;
    if (themeId == null) return;
    final theme = await widget.terminalThemeCatalog?.loadTheme(themeId);
    if (!mounted || themeId != _themeId) return;
    setState(() {
      if (theme == null) {
        _themeId = null;
        _selectedTheme = null;
      } else {
        _selectedTheme = StoredTerminalTheme(id: themeId, theme: theme);
      }
    });
  }

  Future<void> _loadSelectedTelnetTheme() async {
    final themeId = _telnetThemeId;
    if (themeId == null) return;
    final theme = await widget.terminalThemeCatalog?.loadTheme(themeId);
    if (!mounted || themeId != _telnetThemeId) return;
    setState(() {
      if (theme == null) {
        _telnetThemeId = null;
        _selectedTelnetTheme = null;
      } else {
        _selectedTelnetTheme = StoredTerminalTheme(id: themeId, theme: theme);
      }
    });
  }

  void _openThemeGallery() {
    _openThemeGalleryFor(_HostThemeTarget.ssh);
  }

  void _openTelnetThemeGallery() {
    _openThemeGalleryFor(_HostThemeTarget.telnet);
  }

  void _openThemeGalleryFor(_HostThemeTarget target) {
    setState(() {
      _themeGalleryFuture ??=
          widget.terminalThemeCatalog?.loadThemes() ??
          Future.value(const <StoredTerminalTheme>[]);
      _themeGalleryTarget = target;
      _themeGalleryOpen = true;
    });
  }

  @override
  void dispose() {
    _nameController.removeListener(_clearNameError);
    _nameController.dispose();
    _portController.dispose();
    _usernameController.dispose();
    _passwordController.dispose();
    _moshCommandController.dispose();
    _telnetPortController.dispose();
    _telnetUsernameController.dispose();
    _telnetPasswordController.dispose();
    super.dispose();
  }

  void _clearNameError() {
    if (_nameError != null && _nameController.text.trim().isNotEmpty) {
      setState(() => _nameError = null);
    }
  }

  void _mergeSettingsFromGroup(HostGroup group) {
    _proxyId ??= group.proxyId;
    _keyId ??= group.keyId;
    _sshCredentialKind ??= _credentialKindForKeyId(widget.keys, _keyId);
    if (!_sshConfigured && group.sshEnabled == true) {
      _sshEnabled = true;
      _sshConfigured = true;
    }
    if (_sshEnabled) {
      _startupSnippetId ??= group.startupSnippetId;
    }
    if (!_moshConfigured && _sshEnabled && group.moshEnabled == true) {
      _moshEnabled = true;
      _moshConfigured = true;
    }
    if (!_telnetConfigured && group.telnetEnabled == true) {
      _telnetEnabled = true;
      _telnetConfigured = true;
    }
    if (!_portConfigured &&
        group.port != null &&
        (_portController.text.trim().isEmpty ||
            _portController.text.trim() == '22')) {
      _portController.text = '${group.port}';
      _portConfigured = true;
    }
    final sshIdentityIsEmpty =
        _identityId == null &&
        _usernameController.text.trim().isEmpty &&
        _passwordController.text.isEmpty;
    if (sshIdentityIsEmpty) {
      if (group.identityId != null) {
        _identityId = group.identityId;
        _showSelectedIdentityName();
      } else {
        if (group.username?.trim().isNotEmpty == true) {
          _usernameController.text = group.username!;
        }
        if (group.password?.isNotEmpty == true) {
          _passwordController.text = group.password!;
        }
      }
    }
    if ((_moshCommandController.text.trim().isEmpty ||
            _moshCommandController.text.trim() == defaultMoshServerCommand) &&
        group.moshServerCommand?.trim().isNotEmpty == true) {
      _moshCommandController.text = group.moshServerCommand!;
    }
    if (!_telnetPortConfigured &&
        group.telnetPort != null &&
        (_telnetPortController.text.trim().isEmpty ||
            _telnetPortController.text.trim() == '23')) {
      _telnetPortController.text = '${group.telnetPort}';
      _telnetPortConfigured = true;
    }
    final telnetIdentityIsEmpty =
        _telnetIdentityId == null &&
        _telnetUsernameController.text.trim().isEmpty &&
        _telnetPasswordController.text.isEmpty;
    if (telnetIdentityIsEmpty) {
      if (group.telnetIdentityId != null) {
        _telnetIdentityId = group.telnetIdentityId;
        _showSelectedIdentityName();
      } else {
        if (group.telnetUsername?.trim().isNotEmpty == true) {
          _telnetUsernameController.text = group.telnetUsername!;
        }
        if (group.telnetPassword?.isNotEmpty == true) {
          _telnetPasswordController.text = group.telnetPassword!;
        }
      }
    }
    if (!_encodingConfigured && group.encoding?.trim().isNotEmpty == true) {
      _encoding = _normalizeHostEncoding(group.encoding);
      _encodingConfigured = true;
    }
    if (!_telnetEncodingConfigured &&
        group.telnetEncoding?.trim().isNotEmpty == true) {
      _telnetEncoding = _normalizeHostEncoding(group.telnetEncoding);
      _telnetEncodingConfigured = true;
    }
    _themeId ??= normalizeTerminalThemeId(group.themeId);
    _telnetThemeId ??= normalizeTerminalThemeId(group.telnetThemeId);
    if (_sshEnabled) {
      final existingVariables = {
        for (final variable in _environmentVariables) variable.variable,
      };
      _environmentVariables = [
        ..._environmentVariables,
        for (final variable in group.environmentVariables)
          if (variable.variable.trim().isNotEmpty &&
              existingVariables.add(variable.variable))
            variable,
      ];
    }
  }

  void _selectParentGroup(int? parentId) {
    setState(() {
      _parentId = parentId;
      final parent = widget.groups
          .where((group) => group.id == parentId)
          .firstOrNull;
      if (parent != null) {
        _mergeSettingsFromGroup(parent);
      }
    });
    _loadSelectedTheme();
    _loadSelectedTelnetTheme();
  }

  void _showSelectedIdentityName() {
    final identity = _identityById(widget.identities, _identityId);
    if (identity != null) {
      _usernameController.text = identity.name;
      _passwordController.clear();
    }
    final telnetIdentity = _identityById(widget.identities, _telnetIdentityId);
    if (telnetIdentity != null) {
      _telnetUsernameController.text = telnetIdentity.name;
      _telnetPasswordController.clear();
    }
  }

  void _selectSshIdentity(int? id) {
    setState(() {
      _identityId = id;
      if (id == null) {
        _usernameController.clear();
        return;
      }
      _usernameController.text =
          _identityById(widget.identities, id)?.name ?? '';
      _passwordController.clear();
      _keyId = null;
      _sshCredentialKind = null;
    });
  }

  void _handleSshIdentityTextChanged(String value) {
    if (_identityId == null) return;
    final selectedName =
        _identityById(widget.identities, _identityId)?.name ?? '';
    if (value != selectedName) {
      setState(() => _identityId = null);
    }
  }

  void _createSshIdentity(String initialName) {
    widget.onCreateIdentity(initialName, (id) {
      if (!mounted) return;
      setState(() {
        _identityId = id;
        _usernameController.text =
            _identityById(widget.identities, id)?.name ?? initialName;
        _passwordController.clear();
        _keyId = null;
        _sshCredentialKind = null;
      });
    });
  }

  void _selectTelnetIdentity(int? id) {
    setState(() {
      _telnetIdentityId = id;
      if (id == null) {
        _telnetUsernameController.clear();
        return;
      }
      _telnetUsernameController.text =
          _identityById(widget.identities, id)?.name ?? '';
      _telnetPasswordController.clear();
    });
  }

  void _handleTelnetIdentityTextChanged(String value) {
    if (_telnetIdentityId == null) return;
    final selectedName =
        _identityById(widget.identities, _telnetIdentityId)?.name ?? '';
    if (value != selectedName) {
      setState(() => _telnetIdentityId = null);
    }
  }

  void _createTelnetIdentity(String initialName) {
    widget.onCreateIdentity(initialName, (id) {
      if (!mounted) return;
      setState(() {
        _telnetIdentityId = id;
        _telnetUsernameController.text =
            _identityById(widget.identities, id)?.name ?? initialName;
        _telnetPasswordController.clear();
      });
    });
  }

  void _createStartupSnippet(String initialName) {
    widget.onCreateSnippet(initialName, (id) {
      if (mounted) {
        setState(() => _startupSnippetId = id);
      }
    });
  }

  void _selectSshCredentialKind(_SshCredentialKind kind) {
    setState(() {
      if (_sshCredentialKind != kind) {
        _keyId = null;
      }
      _sshCredentialKind = kind;
    });
  }

  void _createSshCredential(String initialName) {
    final kind = _sshCredentialKind;
    if (kind == null) return;
    widget.onCreateCredential(
      initialName,
      certificate: kind == _SshCredentialKind.certificate,
      onCreated: (id) {
        if (mounted) {
          setState(() {
            _keyId = id;
            _sshCredentialKind = kind;
          });
        }
      },
    );
  }

  Future<void> _save() async {
    final name = _nameController.text.trim();
    if (name.isEmpty) {
      setState(() {
        _nameError = 'Name is required.';
        _error = null;
      });
      return;
    }
    final port = _intFromText(_portController.text);
    final telnetPort = _intFromText(_telnetPortController.text);
    if (_sshEnabled && (port == null || port < 1 || port > 65535)) {
      setState(() => _error = 'SSH port must be between 1 and 65535.');
      return;
    }
    if (_telnetEnabled &&
        (telnetPort == null || telnetPort < 1 || telnetPort > 65535)) {
      setState(() => _error = 'Telnet port must be between 1 and 65535.');
      return;
    }

    setState(() {
      _saving = true;
      _error = null;
    });
    await widget.onSave(
      HostGroup(
        id: widget.request.initial?.id,
        name: name,
        parentId: _parentId,
        startupSnippetId: _sshEnabled ? _startupSnippetId : null,
        identityId: _sshEnabled ? _identityId : null,
        proxyId: _sshEnabled ? _proxyId : null,
        port: _sshEnabled ? port : null,
        username: _sshEnabled && _identityId == null
            ? _emptyToNull(_usernameController.text)
            : null,
        password: _sshEnabled && _identityId == null
            ? _emptyToNull(_passwordController.text)
            : null,
        sshEnabled: _sshEnabled,
        moshEnabled: _sshEnabled ? _moshEnabled : false,
        moshServerCommand: _sshEnabled && _moshEnabled
            ? _emptyToNull(_moshCommandController.text)
            : null,
        telnetEnabled: _telnetEnabled,
        telnetIdentityId: _telnetEnabled ? _telnetIdentityId : null,
        telnetUsername: _telnetEnabled && _telnetIdentityId == null
            ? _emptyToNull(_telnetUsernameController.text)
            : null,
        telnetPassword: _telnetEnabled && _telnetIdentityId == null
            ? _emptyToNull(_telnetPasswordController.text)
            : null,
        telnetPort: _telnetEnabled ? telnetPort : null,
        themeId: _sshEnabled ? _themeId : null,
        telnetThemeId: _telnetEnabled ? _telnetThemeId : null,
        environmentVariables: _sshEnabled ? _environmentVariables : const [],
        encoding: _sshEnabled ? _encoding : null,
        telnetEncoding: _telnetEnabled ? _telnetEncoding : null,
        keyId: _sshEnabled && _identityId == null ? _keyId : null,
      ),
    );
  }

  void _editEnvironment() {
    widget.onEditEnvironment(
      _nameController.text.trim().isEmpty
          ? 'this group'
          : _nameController.text.trim(),
      _environmentVariables,
      (variables) {
        if (mounted) {
          setState(() => _environmentVariables = List.of(variables));
        }
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final initial = widget.request.initial;
    final groups = widget.groups
        .where(
          (group) =>
              group.id != initial?.id &&
              _canUseGroupAsParent(widget.groups, group.id) &&
              !_isDescendantGroup(widget.groups, group.id, initial?.id),
        )
        .toList(growable: false);

    final editor = _EditorShell(
      title: initial == null ? 'New Group' : 'Edit Group',
      onClose: widget.onClose,
      onSave: _save,
      saving: _saving,
      error: _error,
      headerActions: initial?.id == null || _saving
          ? const []
          : [
              _EditorShellMenuAction.duplicate(
                () => widget.onDuplicate(initial!),
              ),
              _EditorShellMenuAction.delete(() => widget.onDelete(initial!)),
            ],
      children: [
        _WorkspaceFormSection(
          title: 'General',
          children: [
            LayoutBuilder(
              builder: (context, constraints) {
                final compact = constraints.maxWidth < 120;
                final icon = _BrandIcon(
                  icon: Icons.dashboard_customize_rounded,
                  color: const Color(0xff075e92),
                );
                final input = _WorkspaceInput(
                  controller: _nameController,
                  label: 'Name',
                  isRequired: true,
                  errorText: _nameError,
                  autofocus: true,
                );

                if (compact) {
                  return Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      icon,
                      SizedBox(height: _workspaceFormFieldGap),
                      input,
                    ],
                  );
                }

                return Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    icon,
                    SizedBox(width: 12),
                    Expanded(child: input),
                  ],
                );
              },
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _WorkspaceSelect<int?>(
              label: 'Parent group',
              value: _parentId,
              items: [
                DropdownMenuItem<int?>(
                  value: null,
                  child: Text(
                    tr('workspace.label.noGroup', fallback: 'No group'),
                  ),
                ),
                for (final group in groups)
                  DropdownMenuItem<int?>(
                    value: group.id,
                    child: Text(_hostGroupPathLabel(widget.groups, group)),
                  ),
              ],
              onChanged: _selectParentGroup,
            ),
          ],
        ),
        if (_sshEnabled) ...[
          SizedBox(height: 14),
          _ProtocolFormSection(
            protocol: 'SSH',
            portController: _portController,
            onRemove: () => setState(() {
              _sshEnabled = false;
              _moshEnabled = false;
              _startupSnippetId = null;
              _environmentVariables = [];
              _sshConfigured = true;
              _moshConfigured = true;
            }),
            credentials: [
              _WorkspaceSelect<int?>(
                label: _identityId == null ? 'Username' : 'Identity',
                value: _identityId,
                editable: true,
                searchable: true,
                clearable: true,
                inputController: _usernameController,
                onTextChanged: _handleSshIdentityTextChanged,
                createLabel: 'Create identity',
                onCreate: _createSshIdentity,
                items: [
                  for (final identity in widget.identities)
                    DropdownMenuItem<int?>(
                      value: identity.id,
                      child: Text(identity.name),
                    ),
                ],
                onChanged: _selectSshIdentity,
              ),
              if (_identityId == null) ...[
                SizedBox(height: _workspaceFormFieldGap),
                _WorkspaceSelect<int?>(
                  label: 'Password',
                  value: _identityId,
                  editable: true,
                  searchable: true,
                  clearable: true,
                  obscureText: true,
                  inputController: _passwordController,
                  items: [
                    for (final identity in widget.identities)
                      DropdownMenuItem<int?>(
                        value: identity.id,
                        child: Text(identity.name),
                      ),
                  ],
                  onChanged: _selectSshIdentity,
                ),
                SizedBox(height: _workspaceFormFieldGap),
                _SshCredentialControl(
                  kind: _sshCredentialKind,
                  keyId: _keyId,
                  keys: widget.keys,
                  onKindSelected: _selectSshCredentialKind,
                  onChanged: (value) => setState(() => _keyId = value),
                  onCreate: _createSshCredential,
                  onCleared: () => setState(() {
                    _keyId = null;
                    _sshCredentialKind = null;
                  }),
                ),
              ],
            ],
            theme: _HostThemeSelector(
              selectedTheme: _selectedTheme,
              onOpenGallery: _openThemeGallery,
            ),
            showMore: _sshShowMore,
            onShowMoreChanged: (value) => setState(() => _sshShowMore = value),
            moreChildren: [
              _WorkspaceSelect<int?>(
                label: 'Startup snippet',
                value: _startupSnippetId,
                editable: true,
                clearable: true,
                searchable: true,
                createLabel: 'Create snippet',
                onCreate: _createStartupSnippet,
                items: [
                  for (final snippet in widget.snippets)
                    DropdownMenuItem<int?>(
                      value: snippet.id,
                      child: Text(snippet.description),
                    ),
                ],
                onChanged: (value) => setState(() => _startupSnippetId = value),
              ),
              SizedBox(height: _workspaceFormFieldGap),
              _WorkspaceSelect<int?>(
                label: 'Proxy',
                value: _proxyId,
                editable: true,
                clearable: true,
                items: [
                  for (final proxy in widget.proxies)
                    DropdownMenuItem<int?>(
                      value: proxy.id,
                      child: Text(proxy.name),
                    ),
                ],
                onChanged: (value) => setState(() => _proxyId = value),
              ),
              SizedBox(height: _workspaceFormFieldGap),
              _HostEnvironmentNavigationField(
                variables: _environmentVariables,
                onPressed: _editEnvironment,
              ),
              SizedBox(height: _workspaceFormFieldGap),
              _WorkspaceSelect<String>(
                label: 'Encoding',
                value: _encoding,
                items: [
                  for (final encoding in _hostEncodingOptions)
                    DropdownMenuItem(value: encoding, child: Text(encoding)),
                ],
                onChanged: (value) => setState(() {
                  _encoding = value ?? _encoding;
                  _encodingConfigured = true;
                }),
              ),
              SizedBox(height: _workspaceFormFieldGap),
              _WorkspaceSelect<bool>(
                label: 'Mosh',
                value: _moshEnabled,
                items: [
                  DropdownMenuItem(
                    value: false,
                    child: Text(
                      tr('common.label.disabled', fallback: 'Disabled'),
                    ),
                  ),
                  DropdownMenuItem(
                    value: true,
                    child: Text(
                      tr('common.label.enabled', fallback: 'Enabled'),
                    ),
                  ),
                ],
                onChanged: (value) {
                  if (value == null) return;
                  setState(() {
                    _moshEnabled = value;
                    _moshConfigured = true;
                  });
                },
              ),
              if (_moshEnabled) ...[
                SizedBox(height: _workspaceFormFieldGap),
                _WorkspaceInput(
                  controller: _moshCommandController,
                  label: 'Mosh server command',
                ),
              ],
            ],
          ),
        ],
        if (_telnetEnabled) ...[
          SizedBox(height: 14),
          _ProtocolFormSection(
            protocol: 'Telnet',
            portController: _telnetPortController,
            onRemove: () => setState(() {
              _telnetEnabled = false;
              _telnetConfigured = true;
            }),
            credentials: [
              _WorkspaceSelect<int?>(
                label: _telnetIdentityId == null ? 'Username' : 'Identity',
                value: _telnetIdentityId,
                editable: true,
                searchable: true,
                clearable: true,
                inputController: _telnetUsernameController,
                onTextChanged: _handleTelnetIdentityTextChanged,
                createLabel: 'Create identity',
                onCreate: _createTelnetIdentity,
                items: [
                  for (final identity in widget.identities)
                    DropdownMenuItem<int?>(
                      value: identity.id,
                      child: Text(identity.name),
                    ),
                ],
                onChanged: _selectTelnetIdentity,
              ),
              if (_telnetIdentityId == null) ...[
                SizedBox(height: _workspaceFormFieldGap),
                _WorkspaceSelect<int?>(
                  label: 'Password',
                  value: _telnetIdentityId,
                  editable: true,
                  searchable: true,
                  clearable: true,
                  obscureText: true,
                  inputController: _telnetPasswordController,
                  items: [
                    for (final identity in widget.identities)
                      DropdownMenuItem<int?>(
                        value: identity.id,
                        child: Text(identity.name),
                      ),
                  ],
                  onChanged: _selectTelnetIdentity,
                ),
              ],
              SizedBox(height: _workspaceFormFieldGap),
              _WorkspaceSelect<String>(
                label: 'Encoding',
                value: _telnetEncoding,
                items: [
                  for (final encoding in _hostEncodingOptions)
                    DropdownMenuItem(value: encoding, child: Text(encoding)),
                ],
                onChanged: (value) => setState(() {
                  _telnetEncoding = value ?? _telnetEncoding;
                  _telnetEncodingConfigured = true;
                }),
              ),
            ],
            theme: _HostThemeSelector(
              selectedTheme: _selectedTelnetTheme,
              onOpenGallery: _openTelnetThemeGallery,
            ),
          ),
        ],
        if (!_sshEnabled || !_telnetEnabled) ...[
          SizedBox(height: 14),
          _AddProtocolControl(
            sshAvailable: !_sshEnabled,
            telnetAvailable: !_telnetEnabled,
            onSelected: (value) => setState(() {
              if (value == 'ssh') {
                _sshEnabled = true;
                _sshConfigured = true;
              }
              if (value == 'telnet') {
                _telnetEnabled = true;
                _telnetConfigured = true;
              }
            }),
          ),
        ],
      ],
    );

    return Stack(
      children: [
        editor,
        Positioned.fill(
          child: IgnorePointer(
            ignoring: !_themeGalleryOpen,
            child: AnimatedOpacity(
              opacity: _themeGalleryOpen ? 1 : 0,
              duration: const Duration(milliseconds: 130),
              curve: Curves.easeOutCubic,
              child: AnimatedSlide(
                offset: _themeGalleryOpen ? Offset.zero : const Offset(0.08, 0),
                duration: const Duration(milliseconds: 150),
                curve: Curves.easeOutCubic,
                child: _ThemeGalleryDrawer(
                  themesFuture:
                      _themeGalleryFuture ??
                      Future.value(const <StoredTerminalTheme>[]),
                  selectedThemeId:
                      _themeGalleryTarget == _HostThemeTarget.telnet
                      ? _telnetThemeId
                      : _themeId,
                  onClose: () => setState(() => _themeGalleryOpen = false),
                  onSelected: (themeId, selectedTheme) {
                    setState(() {
                      if (_themeGalleryTarget == _HostThemeTarget.telnet) {
                        _telnetThemeId = normalizeTerminalThemeId(themeId);
                        _selectedTelnetTheme = selectedTheme;
                      } else {
                        _themeId = normalizeTerminalThemeId(themeId);
                        _selectedTheme = selectedTheme;
                      }
                      _themeGalleryOpen = false;
                    });
                  },
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _HostEditorContent extends StatefulWidget {
  const _HostEditorContent({
    required this.request,
    required this.groups,
    required this.snippets,
    required this.keys,
    required this.identities,
    required this.tags,
    required this.proxies,
    required this.terminalThemeCatalog,
    required this.onClose,
    required this.onCreateGroup,
    required this.onCreateGroupFromProtocol,
    required this.onCreateCredential,
    required this.onCreateIdentity,
    required this.onCreateProxy,
    required this.onCreateTag,
    required this.onCreateSnippet,
    required this.onEditEnvironment,
    required this.onSave,
    required this.onConnect,
    required this.onDuplicate,
    required this.onDelete,
  });

  final _HostEditorRequest request;
  final List<HostGroup> groups;
  final List<_SnippetItem> snippets;
  final List<KeyEntry> keys;
  final List<IdentityEntry> identities;
  final List<TagEntry> tags;
  final List<ProxyEntry> proxies;
  final TerminalThemeCatalog? terminalThemeCatalog;
  final VoidCallback onClose;
  final _CreateRelatedEntry onCreateGroup;
  final _CreateGroupFromProtocol onCreateGroupFromProtocol;
  final _CreateRelatedCredential onCreateCredential;
  final _CreateRelatedEntry onCreateIdentity;
  final _CreateRelatedEntry onCreateProxy;
  final _CreateTag onCreateTag;
  final _CreateRelatedEntry onCreateSnippet;
  final _EditHostEnvironment onEditEnvironment;
  final _SaveHost onSave;
  final ValueChanged<HostEntry> onConnect;
  final ValueChanged<HostEntry> onDuplicate;
  final ValueChanged<HostEntry> onDelete;

  @override
  State<_HostEditorContent> createState() => _HostEditorContentState();
}

class _HostTagSelect extends StatefulWidget {
  const _HostTagSelect({
    required this.tags,
    required this.selectedUuids,
    required this.onChanged,
    required this.onCreate,
  });

  final List<TagEntry> tags;
  final Set<String> selectedUuids;
  final ValueChanged<Set<String>> onChanged;
  final _CreateTag onCreate;

  @override
  State<_HostTagSelect> createState() => _HostTagSelectState();
}

class _HostTagSelectState extends State<_HostTagSelect> {
  final TextEditingController _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final selectedTags = [
      for (final tag in widget.tags)
        if (tag.uuid != null && widget.selectedUuids.contains(tag.uuid)) tag,
    ];
    final availableTags = [
      for (final tag in widget.tags)
        if (tag.uuid != null && !widget.selectedUuids.contains(tag.uuid)) tag,
    ];
    return _WorkspaceSelect<String?>(
      label: 'Tags',
      value: null,
      editable: true,
      searchable: true,
      createConflictLabels: widget.tags.map((tag) => tag.name),
      inputController: _controller,
      createLabel: 'Create tag',
      onCreate: _createAndSelect,
      leading: selectedTags.isEmpty
          ? null
          : SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  for (final tag in selectedTags) ...[
                    _HostTagChip(tag: tag, onDeleted: () => _remove(tag)),
                    if (tag != selectedTags.last) const SizedBox(width: 4),
                  ],
                ],
              ),
            ),
      items: [
        for (final tag in availableTags)
          DropdownMenuItem<String?>(value: tag.uuid, child: Text(tag.name)),
      ],
      onChanged: (uuid) {
        if (uuid == null) {
          return;
        }
        widget.onChanged({...widget.selectedUuids, uuid});
        _controller.clear();
      },
    );
  }

  void _createAndSelect(String name) {
    final tag = widget.onCreate(name);
    final uuid = tag?.uuid;
    if (uuid == null) {
      return;
    }
    widget.onChanged({...widget.selectedUuids, uuid});
    _controller.clear();
  }

  void _remove(TagEntry tag) {
    final uuid = tag.uuid;
    if (uuid == null) {
      return;
    }
    widget.onChanged({...widget.selectedUuids}..remove(uuid));
  }
}

class _HostTagChip extends StatelessWidget {
  const _HostTagChip({required this.tag, required this.onDeleted});

  final TagEntry tag;
  final VoidCallback onDeleted;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 24,
      padding: const EdgeInsets.only(left: 8, right: 2),
      decoration: BoxDecoration(
        color: _sidebarHover,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: _sidebarDivider),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            tag.name,
            style: TextStyle(
              color: _text,
              fontSize: NautermFontSizes.labelSmall,
              fontWeight: NautermFontWeights.medium,
            ),
          ),
          SizedBox(
            width: 20,
            height: 20,
            child: IconButton(
              tooltip: tr('Remove ${tag.name}'),
              padding: EdgeInsets.zero,
              visualDensity: VisualDensity.compact,
              icon: Icon(Icons.close_rounded, size: 13, color: _mutedText),
              onPressed: onDeleted,
            ),
          ),
        ],
      ),
    );
  }
}

class _HostEditorContentState extends State<_HostEditorContent> {
  late final TextEditingController _nameController;
  late final TextEditingController _hostController;
  late final TextEditingController _portController;
  late final TextEditingController _usernameController;
  late final TextEditingController _passwordController;
  late final TextEditingController _moshServerCommandController;
  late final TextEditingController _telnetPortController;
  late final TextEditingController _telnetUsernameController;
  late final TextEditingController _telnetPasswordController;
  late final TextEditingController _shellController;
  late final TextEditingController _workDirController;
  late final List<String> _shellOptions;
  late List<HostEnvironmentVariable> _environmentVariables;
  late NautermHostType _type;
  late bool _sshEnabled;
  late bool _moshEnabled;
  late bool _telnetEnabled;
  late bool _sshConfigured;
  late bool _moshConfigured;
  late bool _telnetConfigured;
  late bool _portConfigured;
  late bool _telnetPortConfigured;
  late bool _encodingConfigured;
  late bool _telnetEncodingConfigured;
  int? _groupId;
  int? _identityId;
  int? _proxyId;
  int? _telnetIdentityId;
  int? _keyId;
  int? _startupSnippetId;
  _SshCredentialKind? _sshCredentialKind;
  String? _themeId;
  String? _telnetThemeId;
  String _encoding = defaultHostEncoding;
  String _telnetEncoding = defaultHostEncoding;
  late Set<String> _tagUuids;
  StoredTerminalTheme? _selectedTheme;
  StoredTerminalTheme? _selectedTelnetTheme;
  Future<List<StoredTerminalTheme>>? _themeGalleryFuture;
  bool _themeGalleryOpen = false;
  bool _sshShowMore = false;
  _HostThemeTarget _themeGalleryTarget = _HostThemeTarget.ssh;
  bool _saving = false;
  String? _error;
  String? _nameError;
  late bool _nameIsAutomatic;
  bool _settingAutomaticName = false;

  @override
  void initState() {
    super.initState();
    final initial = widget.request.initial;
    _nameController = TextEditingController(text: initial?.name ?? '');
    _hostController = TextEditingController(text: initial?.host ?? '');
    _portController = TextEditingController(text: '${initial?.port ?? 22}');
    _usernameController = TextEditingController(text: initial?.username ?? '');
    _passwordController = TextEditingController(text: initial?.password ?? '');
    _startupSnippetId = initial?.startupSnippetId;
    _moshServerCommandController = TextEditingController(
      text: initial?.moshServerCommand ?? defaultMoshServerCommand,
    );
    _telnetPortController = TextEditingController(
      text: '${initial?.telnetPort ?? 23}',
    );
    _telnetUsernameController = TextEditingController(
      text: initial?.telnetUsername ?? '',
    );
    _telnetPasswordController = TextEditingController(
      text: initial?.telnetPassword ?? '',
    );
    _shellController = TextEditingController(text: initial?.shellPath ?? '');
    _workDirController = TextEditingController(text: initial?.workDir ?? '');
    _shellOptions = _availableShellOptions(initial?.shellPath);
    _environmentVariables = List<HostEnvironmentVariable>.of(
      initial?.environmentVariables ?? const <HostEnvironmentVariable>[],
    );
    _type = initial?.type ?? NautermHostType.remote;
    _sshEnabled =
        initial?.sshEnabledOverride ??
        (initial == null || initial.type == NautermHostType.remote);
    _moshEnabled = initial?.moshEnabled ?? false;
    _telnetEnabled = initial?.telnetEnabled ?? false;
    _sshConfigured = initial?.sshEnabledOverride != null;
    _moshConfigured = initial?.moshEnabledOverride != null;
    _telnetConfigured = initial?.telnetEnabledOverride != null;
    _portConfigured = initial?.port != null;
    _telnetPortConfigured = initial?.telnetPort != null;
    _encodingConfigured = initial?.encodingOverride?.trim().isNotEmpty == true;
    _telnetEncodingConfigured =
        initial?.telnetEncodingOverride?.trim().isNotEmpty == true;
    _groupId = initial?.groupId ?? widget.request.initialGroupId;
    _identityId = initial?.identityId;
    _proxyId = initial?.proxyId;
    _telnetIdentityId = initial?.telnetIdentityId;
    _keyId = initial?.keyId;
    _themeId = normalizeTerminalThemeId(initial?.themeId);
    _telnetThemeId = normalizeTerminalThemeId(initial?.telnetThemeId);
    _encoding = _normalizeHostEncoding(initial?.encoding);
    _telnetEncoding = _normalizeHostEncoding(initial?.telnetEncoding);
    _tagUuids = {...?initial?.tagUuids};
    _nameIsAutomatic = _nameController.text.trim().isEmpty;
    _nameController.addListener(_handleNameChanged);
    _hostController.addListener(_refreshAutomaticName);
    _portController.addListener(_refreshAutomaticName);
    _usernameController.addListener(_refreshAutomaticName);
    _telnetPortController.addListener(_refreshAutomaticName);
    _telnetUsernameController.addListener(_refreshAutomaticName);
    if (_groupId != null &&
        !widget.groups.any((group) => group.id == _groupId)) {
      _groupId = null;
    }
    if (_identityId != null &&
        !widget.identities.any((identity) => identity.id == _identityId)) {
      _identityId = null;
    }
    if (_proxyId != null &&
        !widget.proxies.any((proxy) => proxy.id == _proxyId)) {
      _proxyId = null;
    }
    if (_telnetIdentityId != null &&
        !widget.identities.any(
          (identity) => identity.id == _telnetIdentityId,
        )) {
      _telnetIdentityId = null;
    }
    if (_keyId != null && !widget.keys.any((key) => key.id == _keyId)) {
      _keyId = null;
    }
    if (initial == null && _groupId != null) {
      final group = widget.groups
          .where((entry) => entry.id == _groupId)
          .firstOrNull;
      if (group != null) {
        _mergeSettingsFromGroup(group);
      }
    }
    _sshCredentialKind = _credentialKindForKeyId(widget.keys, _keyId);
    _showSelectedIdentityName();
    _refreshAutomaticName();
    _loadSelectedTheme();
    _loadSelectedTelnetTheme();
  }

  Future<void> _loadSelectedTheme() async {
    final themeId = _themeId;
    if (themeId == null) {
      return;
    }

    final theme = await widget.terminalThemeCatalog?.loadTheme(themeId);
    if (!mounted || themeId != _themeId) {
      return;
    }

    setState(() {
      if (theme == null) {
        _themeId = null;
        _selectedTheme = null;
      } else {
        _selectedTheme = StoredTerminalTheme(id: themeId, theme: theme);
      }
    });
  }

  @override
  void didUpdateWidget(covariant _HostEditorContent oldWidget) {
    super.didUpdateWidget(oldWidget);
    _refreshAutomaticName();
  }

  void _openThemeGallery() {
    _openThemeGalleryFor(_HostThemeTarget.ssh);
  }

  void _openTelnetThemeGallery() {
    _openThemeGalleryFor(_HostThemeTarget.telnet);
  }

  void _openThemeGalleryFor(_HostThemeTarget target) {
    setState(() {
      _themeGalleryFuture ??=
          widget.terminalThemeCatalog?.loadThemes() ??
          Future.value(const <StoredTerminalTheme>[]);
      _themeGalleryTarget = target;
      _themeGalleryOpen = true;
    });
  }

  Future<void> _loadSelectedTelnetTheme() async {
    final themeId = _telnetThemeId;
    if (themeId == null) {
      return;
    }

    final theme = await widget.terminalThemeCatalog?.loadTheme(themeId);
    if (!mounted || themeId != _telnetThemeId) {
      return;
    }

    setState(() {
      if (theme == null) {
        _telnetThemeId = null;
        _selectedTelnetTheme = null;
      } else {
        _selectedTelnetTheme = StoredTerminalTheme(id: themeId, theme: theme);
      }
    });
  }

  @override
  void dispose() {
    _nameController.removeListener(_handleNameChanged);
    _hostController.removeListener(_refreshAutomaticName);
    _portController.removeListener(_refreshAutomaticName);
    _usernameController.removeListener(_refreshAutomaticName);
    _telnetPortController.removeListener(_refreshAutomaticName);
    _telnetUsernameController.removeListener(_refreshAutomaticName);
    _nameController.dispose();
    _hostController.dispose();
    _portController.dispose();
    _usernameController.dispose();
    _passwordController.dispose();
    _moshServerCommandController.dispose();
    _telnetPortController.dispose();
    _telnetUsernameController.dispose();
    _telnetPasswordController.dispose();
    _shellController.dispose();
    _workDirController.dispose();
    super.dispose();
  }

  void _handleNameChanged() {
    if (_settingAutomaticName) return;
    _nameIsAutomatic = _nameController.text.trim().isEmpty;
    if (_nameError != null && _nameController.text.trim().isNotEmpty) {
      setState(() => _nameError = null);
    }
  }

  void _refreshAutomaticName() {
    if (!_nameIsAutomatic) return;
    final suggested = _suggestedName();
    if (_nameController.text == suggested) return;
    _settingAutomaticName = true;
    _nameController.value = TextEditingValue(
      text: suggested,
      selection: TextSelection.collapsed(offset: suggested.length),
    );
    _settingAutomaticName = false;
  }

  String _suggestedName() {
    return _suggestHostName(
      type: _type,
      host: _hostController.text,
      sshEnabled: _sshEnabled,
      sshUsername: _identityUsername(_identityId) ?? _usernameController.text,
      sshPort: _portController.text,
      telnetEnabled: _telnetEnabled,
      telnetUsername:
          _identityUsername(_telnetIdentityId) ??
          _telnetUsernameController.text,
      telnetPort: _telnetPortController.text,
    );
  }

  String? _identityUsername(int? identityId) {
    return _identityById(widget.identities, identityId)?.username?.trim();
  }

  void _createGroup(String initialName) {
    widget.onCreateGroup(initialName, (id) {
      if (mounted) {
        _selectGroup(id);
      }
    });
  }

  void _mergeSettingsFromGroup(HostGroup group) {
    _proxyId ??= group.proxyId;
    _keyId ??= group.keyId;
    _sshCredentialKind ??= _credentialKindForKeyId(widget.keys, _keyId);
    if (!_sshConfigured && group.sshEnabled == true) {
      _sshEnabled = true;
      _sshConfigured = true;
    }
    if (_sshEnabled) {
      _startupSnippetId ??= group.startupSnippetId;
    }
    if (!_moshConfigured && _sshEnabled && group.moshEnabled == true) {
      _moshEnabled = true;
      _moshConfigured = true;
    }
    if (!_telnetConfigured && group.telnetEnabled == true) {
      _telnetEnabled = true;
      _telnetConfigured = true;
    }
    if (!_portConfigured &&
        group.port != null &&
        (_portController.text.trim().isEmpty ||
            _portController.text.trim() == '22')) {
      _portController.text = '${group.port}';
      _portConfigured = true;
    }
    final sshIdentityIsEmpty =
        _identityId == null &&
        _usernameController.text.trim().isEmpty &&
        _passwordController.text.isEmpty;
    if (sshIdentityIsEmpty) {
      if (group.identityId != null) {
        _identityId = group.identityId;
        _showSelectedIdentityName();
      } else {
        if (group.username?.trim().isNotEmpty == true) {
          _usernameController.text = group.username!;
        }
        if (group.password?.isNotEmpty == true) {
          _passwordController.text = group.password!;
        }
      }
    }
    if ((_moshServerCommandController.text.trim().isEmpty ||
            _moshServerCommandController.text.trim() ==
                defaultMoshServerCommand) &&
        group.moshServerCommand?.trim().isNotEmpty == true) {
      _moshServerCommandController.text = group.moshServerCommand!;
    }
    if (!_telnetPortConfigured &&
        group.telnetPort != null &&
        (_telnetPortController.text.trim().isEmpty ||
            _telnetPortController.text.trim() == '23')) {
      _telnetPortController.text = '${group.telnetPort}';
      _telnetPortConfigured = true;
    }
    final telnetIdentityIsEmpty =
        _telnetIdentityId == null &&
        _telnetUsernameController.text.trim().isEmpty &&
        _telnetPasswordController.text.isEmpty;
    if (telnetIdentityIsEmpty) {
      if (group.telnetIdentityId != null) {
        _telnetIdentityId = group.telnetIdentityId;
        _showSelectedIdentityName();
      } else {
        if (group.telnetUsername?.trim().isNotEmpty == true) {
          _telnetUsernameController.text = group.telnetUsername!;
        }
        if (group.telnetPassword?.isNotEmpty == true) {
          _telnetPasswordController.text = group.telnetPassword!;
        }
      }
    }
    if (!_encodingConfigured && group.encoding?.trim().isNotEmpty == true) {
      _encoding = _normalizeHostEncoding(group.encoding);
      _encodingConfigured = true;
    }
    if (!_telnetEncodingConfigured &&
        group.telnetEncoding?.trim().isNotEmpty == true) {
      _telnetEncoding = _normalizeHostEncoding(group.telnetEncoding);
      _telnetEncodingConfigured = true;
    }
    _themeId ??= normalizeTerminalThemeId(group.themeId);
    _telnetThemeId ??= normalizeTerminalThemeId(group.telnetThemeId);
    if (_sshEnabled) {
      final existingVariables = {
        for (final variable in _environmentVariables) variable.variable,
      };
      _environmentVariables = [
        ..._environmentVariables,
        for (final variable in group.environmentVariables)
          if (variable.variable.trim().isNotEmpty &&
              existingVariables.add(variable.variable))
            variable,
      ];
    }
  }

  void _selectGroup(int? groupId) {
    setState(() {
      _groupId = groupId;
      final group = widget.groups
          .where((entry) => entry.id == groupId)
          .firstOrNull;
      if (group != null) {
        _mergeSettingsFromGroup(group);
      }
    });
    _loadSelectedTheme();
    _loadSelectedTelnetTheme();
  }

  void _showSelectedIdentityName() {
    final identity = _identityById(widget.identities, _identityId);
    if (identity != null) {
      _usernameController.text = identity.name;
      _passwordController.clear();
    }
    final telnetIdentity = _identityById(widget.identities, _telnetIdentityId);
    if (telnetIdentity != null) {
      _telnetUsernameController.text = telnetIdentity.name;
      _telnetPasswordController.clear();
    }
  }

  void _selectSshIdentity(int? id) {
    setState(() {
      _identityId = id;
      if (id == null) {
        _usernameController.clear();
        return;
      }
      _usernameController.text =
          _identityById(widget.identities, id)?.name ?? '';
      _passwordController.clear();
      _keyId = null;
      _sshCredentialKind = null;
    });
  }

  void _handleSshIdentityTextChanged(String value) {
    if (_identityId == null) return;
    final selectedName =
        _identityById(widget.identities, _identityId)?.name ?? '';
    if (value != selectedName) {
      setState(() => _identityId = null);
      _refreshAutomaticName();
    }
  }

  void _createSshIdentity(String initialName) {
    widget.onCreateIdentity(initialName, (id) {
      if (!mounted) return;
      setState(() {
        _identityId = id;
        _usernameController.text =
            _identityById(widget.identities, id)?.name ?? initialName;
        _passwordController.clear();
        _keyId = null;
        _sshCredentialKind = null;
      });
    });
  }

  void _selectTelnetIdentity(int? id) {
    setState(() {
      _telnetIdentityId = id;
      if (id == null) {
        _telnetUsernameController.clear();
        return;
      }
      _telnetUsernameController.text =
          _identityById(widget.identities, id)?.name ?? '';
      _telnetPasswordController.clear();
    });
  }

  void _handleTelnetIdentityTextChanged(String value) {
    if (_telnetIdentityId == null) return;
    final selectedName =
        _identityById(widget.identities, _telnetIdentityId)?.name ?? '';
    if (value != selectedName) {
      setState(() => _telnetIdentityId = null);
      _refreshAutomaticName();
    }
  }

  void _createTelnetIdentity(String initialName) {
    widget.onCreateIdentity(initialName, (id) {
      if (!mounted) return;
      setState(() {
        _telnetIdentityId = id;
        _telnetUsernameController.text =
            _identityById(widget.identities, id)?.name ?? initialName;
        _telnetPasswordController.clear();
      });
    });
  }

  void _createProxy(String initialName) {
    widget.onCreateProxy(initialName, (id) {
      if (mounted) {
        setState(() => _proxyId = id);
      }
    });
  }

  void _createStartupSnippet(String initialName) {
    widget.onCreateSnippet(initialName, (id) {
      if (mounted) {
        setState(() => _startupSnippetId = id);
      }
    });
  }

  void _selectSshCredentialKind(_SshCredentialKind kind) {
    setState(() {
      if (_sshCredentialKind != kind) {
        _keyId = null;
      }
      _sshCredentialKind = kind;
    });
  }

  void _createSshCredential(String initialName) {
    final kind = _sshCredentialKind;
    if (kind == null) return;
    widget.onCreateCredential(
      initialName,
      certificate: kind == _SshCredentialKind.certificate,
      onCreated: (id) {
        if (mounted) {
          setState(() {
            _keyId = id;
            _sshCredentialKind = kind;
          });
        }
      },
    );
  }

  void _createGroupFromSsh() {
    widget.onCreateGroupFromProtocol(
      HostGroup(
        name: '',
        startupSnippetId: _startupSnippetId,
        identityId: _identityId,
        proxyId: _proxyId,
        port: _intFromText(_portController.text),
        username: _identityId == null
            ? _emptyToNull(_usernameController.text)
            : null,
        password: _identityId == null
            ? _emptyToNull(_passwordController.text)
            : null,
        themeId: normalizeTerminalThemeId(_themeId),
        sshEnabled: true,
        moshEnabled: _moshEnabled,
        moshServerCommand: _moshEnabled
            ? _emptyToNull(_moshServerCommandController.text)
            : null,
        telnetEnabled: false,
        environmentVariables: List.of(_environmentVariables),
        encoding: _encoding,
        keyId: _identityId == null ? _keyId : null,
      ),
    );
  }

  void _createGroupFromTelnet() {
    widget.onCreateGroupFromProtocol(
      HostGroup(
        name: '',
        sshEnabled: false,
        moshEnabled: false,
        telnetEnabled: true,
        telnetIdentityId: _telnetIdentityId,
        telnetUsername: _telnetIdentityId == null
            ? _emptyToNull(_telnetUsernameController.text)
            : null,
        telnetPassword: _telnetIdentityId == null
            ? _emptyToNull(_telnetPasswordController.text)
            : null,
        telnetPort: _intFromText(_telnetPortController.text),
        telnetThemeId: normalizeTerminalThemeId(_telnetThemeId),
        telnetEncoding: _telnetEncoding,
      ),
    );
  }

  Future<void> _save() async {
    final name = _nameController.text.trim().isNotEmpty
        ? _nameController.text.trim()
        : _suggestedName();
    if (name.isEmpty) {
      setState(() {
        _nameError = 'Name is required.';
        _error = null;
      });
      return;
    }

    final isRemote = _type == NautermHostType.remote;
    final sshEnabled = isRemote && _sshEnabled;
    final moshEnabled = sshEnabled && _moshEnabled;
    final telnetEnabled = isRemote && _telnetEnabled;
    final port = _intFromText(_portController.text);
    if (sshEnabled && (port == null || port < 1 || port > 65535)) {
      setState(() => _error = 'SSH port must be between 1 and 65535.');
      return;
    }
    final telnetPort = _intFromText(_telnetPortController.text);
    if (telnetEnabled &&
        (telnetPort == null || telnetPort < 1 || telnetPort > 65535)) {
      setState(() => _error = 'Telnet port must be between 1 and 65535.');
      return;
    }

    setState(() {
      _saving = true;
      _error = null;
    });

    final initial = widget.request.initial;
    await widget.onSave(
      HostEntry(
        id: initial?.id,
        name: name,
        groupId: _groupId,
        identityId: sshEnabled ? _identityId : null,
        proxyId: sshEnabled ? _proxyId : null,
        host: isRemote ? _emptyToNull(_hostController.text) : null,
        port: sshEnabled ? port : null,
        username: sshEnabled && _identityId == null
            ? _emptyToNull(_usernameController.text)
            : null,
        password: sshEnabled && _identityId == null
            ? _emptyToNull(_passwordController.text)
            : null,
        themeId: normalizeTerminalThemeId(_themeId),
        startupSnippetId: sshEnabled ? _startupSnippetId : null,
        sshEnabled: sshEnabled,
        moshEnabled: moshEnabled,
        moshServerCommand:
            _emptyToNull(_moshServerCommandController.text) ??
            defaultMoshServerCommand,
        telnetEnabled: telnetEnabled,
        telnetIdentityId: telnetEnabled ? _telnetIdentityId : null,
        telnetUsername: telnetEnabled && _telnetIdentityId == null
            ? _emptyToNull(_telnetUsernameController.text)
            : null,
        telnetPassword: telnetEnabled && _telnetIdentityId == null
            ? _emptyToNull(_telnetPasswordController.text)
            : null,
        telnetPort: telnetEnabled ? telnetPort : null,
        telnetThemeId: telnetEnabled
            ? normalizeTerminalThemeId(_telnetThemeId)
            : null,
        environmentVariables: sshEnabled ? _environmentVariables : const [],
        encoding: _encoding,
        telnetEncoding: _telnetEncoding,
        type: _type,
        keyId: sshEnabled && _identityId == null ? _keyId : null,
        shellPath: _type == NautermHostType.local
            ? _emptyToNull(_shellController.text)
            : null,
        workDir: _type == NautermHostType.local
            ? _emptyToNull(_workDirController.text)
            : null,
        os: initial?.os,
        distro: initial?.distro,
        tagUuids: _tagUuids.toList(growable: false),
      ),
    );
  }

  void _editEnvironmentVariables() {
    final hostLabel =
        _emptyToNull(_hostController.text) ??
        _emptyToNull(_nameController.text) ??
        'this host';
    widget.onEditEnvironment(hostLabel, _environmentVariables, (variables) {
      if (!mounted) {
        return;
      }
      setState(() {
        _environmentVariables = List<HostEnvironmentVariable>.of(variables);
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    final initial = widget.request.initial;
    final isLocal = _type == NautermHostType.local;

    final editor = _EditorShell(
      title: initial?.id == null
          ? tr('workspace.label.newHost', fallback: 'New Host')
          : tr('workspace.label.editHost', fallback: 'Edit Host'),
      onClose: widget.onClose,
      onSave: _save,
      saving: _saving,
      error: _error,
      headerActions: initial?.id == null || _saving
          ? const []
          : [
              _EditorShellMenuAction.connect(() => widget.onConnect(initial!)),
              _EditorShellMenuAction.duplicate(
                () => widget.onDuplicate(initial!),
              ),
              _EditorShellMenuAction.delete(() => widget.onDelete(initial!)),
            ],
      children: [
        _WorkspaceFormSection(
          title: isLocal ? 'Shell' : 'Address',
          children: [
            LayoutBuilder(
              builder: (context, constraints) {
                final compact = constraints.maxWidth < 120;
                final icon = _BrandIcon(
                  icon: isLocal
                      ? LucideIcons.squareTerminal
                      : Icons.public_rounded,
                  color: isLocal ? const Color(0xff075e92) : _orange,
                );
                final input = isLocal
                    ? _WorkspaceSelect<String?>(
                        label: 'Shell',
                        value: _emptyToNull(_shellController.text),
                        items: [
                          DropdownMenuItem<String?>(
                            value: null,
                            child: Text(
                              tr(
                                'workspace.label.systemDefaultShell',
                                fallback: 'System default shell',
                              ),
                            ),
                          ),
                          for (final shell in _shellOptions)
                            DropdownMenuItem<String?>(
                              value: shell,
                              child: Text(shell),
                            ),
                        ],
                        onChanged: (value) {
                          setState(() {
                            _shellController.text = value ?? '';
                          });
                        },
                      )
                    : _WorkspaceInput(
                        controller: _hostController,
                        label: 'Host',
                        autofocus: true,
                      );

                if (compact) {
                  return Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      icon,
                      SizedBox(height: _workspaceFormFieldGap),
                      input,
                    ],
                  );
                }

                return Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    icon,
                    SizedBox(width: 12),
                    Expanded(child: input),
                  ],
                );
              },
            ),
          ],
        ),
        SizedBox(height: 14),
        _WorkspaceFormSection(
          title: 'General',
          children: [
            _WorkspaceInput(
              controller: _nameController,
              label: tr('common.label.name', fallback: 'Name'),
              isRequired: true,
              errorText: _nameError,
              autofocus: isLocal,
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _WorkspaceSelect<NautermHostType>(
              label: tr('common.label.type', fallback: 'Type'),
              value: _type,
              items: [
                DropdownMenuItem(
                  value: NautermHostType.remote,
                  child: Text(tr('common.label.remote', fallback: 'Remote')),
                ),
                DropdownMenuItem(
                  value: NautermHostType.local,
                  child: Text(tr('common.label.local', fallback: 'Local')),
                ),
              ],
              onChanged: (value) {
                if (value != null) {
                  setState(() {
                    _type = value;
                    if (value == NautermHostType.local) {
                      _startupSnippetId = null;
                      _environmentVariables = [];
                    }
                  });
                  _refreshAutomaticName();
                }
              },
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _WorkspaceSelect<int?>(
              label: 'Parent group',
              value: _groupId,
              editable: true,
              searchable: true,
              clearable: true,
              createLabel: 'Create group',
              onCreate: _createGroup,
              items: [
                for (final group in widget.groups)
                  DropdownMenuItem<int?>(
                    value: group.id,
                    child: Text(_hostGroupPathLabel(widget.groups, group)),
                  ),
              ],
              onChanged: _selectGroup,
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _HostTagSelect(
              tags: widget.tags,
              selectedUuids: _tagUuids,
              onChanged: (value) => setState(() => _tagUuids = value),
              onCreate: widget.onCreateTag,
            ),
          ],
        ),
        if (isLocal) ...[
          SizedBox(height: 14),
          _WorkspaceFormSection(
            title: 'Theme',
            children: [
              _HostThemeSelector(
                selectedTheme: _selectedTheme,
                onOpenGallery: _openThemeGallery,
              ),
            ],
          ),
          SizedBox(height: 14),
          _WorkspaceFormSection(
            title: 'Local',
            children: [
              _WorkspaceInput(
                controller: _workDirController,
                label: 'Work dir',
              ),
            ],
          ),
        ],
        if (!isLocal && _sshEnabled) ...[
          SizedBox(height: 14),
          _ProtocolFormSection(
            protocol: 'SSH',
            portController: _portController,
            onCreateGroup: _createGroupFromSsh,
            onRemove: () {
              setState(() {
                _sshEnabled = false;
                _moshEnabled = false;
                _startupSnippetId = null;
                _environmentVariables = [];
                _sshConfigured = true;
                _moshConfigured = true;
              });
              _refreshAutomaticName();
            },
            credentials: [
              _WorkspaceSelect<int?>(
                label: _identityId == null ? 'Username' : 'Identity',
                value: _identityId,
                editable: true,
                searchable: true,
                clearable: true,
                inputController: _usernameController,
                onTextChanged: _handleSshIdentityTextChanged,
                createLabel: 'Create identity',
                onCreate: _createSshIdentity,
                items: [
                  for (final identity in widget.identities)
                    DropdownMenuItem<int?>(
                      value: identity.id,
                      child: Text(identity.name),
                    ),
                ],
                onChanged: _selectSshIdentity,
              ),
              if (_identityId == null) ...[
                SizedBox(height: _workspaceFormFieldGap),
                _WorkspaceSelect<int?>(
                  label: 'Password',
                  value: _identityId,
                  editable: true,
                  searchable: true,
                  clearable: true,
                  obscureText: true,
                  inputController: _passwordController,
                  items: [
                    for (final identity in widget.identities)
                      DropdownMenuItem<int?>(
                        value: identity.id,
                        child: Text(identity.name),
                      ),
                  ],
                  onChanged: _selectSshIdentity,
                ),
                SizedBox(height: _workspaceFormFieldGap),
                _SshCredentialControl(
                  kind: _sshCredentialKind,
                  keyId: _keyId,
                  keys: widget.keys,
                  onKindSelected: _selectSshCredentialKind,
                  onChanged: (value) => setState(() => _keyId = value),
                  onCreate: _createSshCredential,
                  onCleared: () => setState(() {
                    _keyId = null;
                    _sshCredentialKind = null;
                  }),
                ),
              ],
            ],
            theme: _HostThemeSelector(
              selectedTheme: _selectedTheme,
              onOpenGallery: _openThemeGallery,
            ),
            showMore: _sshShowMore,
            onShowMoreChanged: (value) => setState(() => _sshShowMore = value),
            moreChildren: [
              _WorkspaceSelect<int?>(
                label: 'Startup snippet',
                value: _startupSnippetId,
                editable: true,
                clearable: true,
                searchable: true,
                createLabel: 'Create snippet',
                onCreate: _createStartupSnippet,
                items: [
                  for (final snippet in widget.snippets)
                    DropdownMenuItem<int?>(
                      value: snippet.id,
                      child: Text(snippet.description),
                    ),
                ],
                onChanged: (value) => setState(() => _startupSnippetId = value),
              ),
              SizedBox(height: _workspaceFormFieldGap),
              _WorkspaceSelect<int?>(
                label: 'Proxy',
                value: _proxyId,
                editable: true,
                searchable: true,
                clearable: true,
                createLabel: 'Create proxy',
                onCreate: _createProxy,
                items: [
                  for (final proxy in widget.proxies)
                    DropdownMenuItem<int?>(
                      value: proxy.id,
                      child: Text(
                        tr(
                          '${_proxyTypeLabel(proxy.type)} ${proxy.name} (${proxy.host}:${proxy.port})',
                        ),
                      ),
                    ),
                ],
                onChanged: (value) => setState(() => _proxyId = value),
              ),
              SizedBox(height: _workspaceFormFieldGap),
              _HostEnvironmentNavigationField(
                variables: _environmentVariables,
                onPressed: _editEnvironmentVariables,
              ),
              SizedBox(height: _workspaceFormFieldGap),
              _WorkspaceSelect<String>(
                label: 'Encoding',
                value: _encoding,
                items: [
                  for (final encoding in _hostEncodingOptions)
                    DropdownMenuItem(value: encoding, child: Text(encoding)),
                ],
                onChanged: (value) => setState(() {
                  _encoding = value ?? _encoding;
                  _encodingConfigured = true;
                }),
              ),
              SizedBox(height: _workspaceFormFieldGap),
              _WorkspaceSelect<bool>(
                label: 'Mosh',
                value: _moshEnabled,
                items: [
                  DropdownMenuItem(
                    value: false,
                    child: Text(
                      tr('common.label.disabled', fallback: 'Disabled'),
                    ),
                  ),
                  DropdownMenuItem(
                    value: true,
                    child: Text(
                      tr('common.label.enabled', fallback: 'Enabled'),
                    ),
                  ),
                ],
                onChanged: (value) {
                  if (value == null) return;
                  setState(() {
                    _moshEnabled = value;
                    _moshConfigured = true;
                  });
                },
              ),
              if (_moshEnabled) ...[
                SizedBox(height: _workspaceFormFieldGap),
                _WorkspaceInput(
                  controller: _moshServerCommandController,
                  label: 'Mosh server command',
                  hintText: defaultMoshServerCommand,
                ),
              ],
            ],
          ),
        ],
        if (!isLocal && _telnetEnabled) ...[
          SizedBox(height: 14),
          _ProtocolFormSection(
            protocol: 'Telnet',
            portController: _telnetPortController,
            onCreateGroup: _createGroupFromTelnet,
            onRemove: () {
              setState(() {
                _telnetEnabled = false;
                _telnetConfigured = true;
              });
              _refreshAutomaticName();
            },
            credentials: [
              _WorkspaceSelect<int?>(
                label: _telnetIdentityId == null ? 'Username' : 'Identity',
                value: _telnetIdentityId,
                editable: true,
                searchable: true,
                clearable: true,
                inputController: _telnetUsernameController,
                onTextChanged: _handleTelnetIdentityTextChanged,
                createLabel: 'Create identity',
                onCreate: _createTelnetIdentity,
                items: [
                  for (final identity in widget.identities)
                    DropdownMenuItem<int?>(
                      value: identity.id,
                      child: Text(identity.name),
                    ),
                ],
                onChanged: _selectTelnetIdentity,
              ),
              if (_telnetIdentityId == null) ...[
                SizedBox(height: _workspaceFormFieldGap),
                _WorkspaceSelect<int?>(
                  label: 'Password',
                  value: _telnetIdentityId,
                  editable: true,
                  searchable: true,
                  clearable: true,
                  obscureText: true,
                  inputController: _telnetPasswordController,
                  items: [
                    for (final identity in widget.identities)
                      DropdownMenuItem<int?>(
                        value: identity.id,
                        child: Text(identity.name),
                      ),
                  ],
                  onChanged: _selectTelnetIdentity,
                ),
              ],
              SizedBox(height: _workspaceFormFieldGap),
              _WorkspaceSelect<String>(
                label: 'Encoding',
                value: _telnetEncoding,
                items: [
                  for (final encoding in _hostEncodingOptions)
                    DropdownMenuItem(value: encoding, child: Text(encoding)),
                ],
                onChanged: (value) => setState(() {
                  _telnetEncoding = value ?? _telnetEncoding;
                  _telnetEncodingConfigured = true;
                }),
              ),
            ],
            theme: _HostThemeSelector(
              selectedTheme: _selectedTelnetTheme,
              onOpenGallery: _openTelnetThemeGallery,
            ),
          ),
        ],
        if (!isLocal && (!_sshEnabled || !_telnetEnabled)) ...[
          SizedBox(height: 14),
          _AddProtocolControl(
            sshAvailable: !_sshEnabled,
            telnetAvailable: !_telnetEnabled,
            onSelected: (value) {
              setState(() {
                if (value == 'ssh') {
                  _sshEnabled = true;
                  _sshConfigured = true;
                }
                if (value == 'telnet') {
                  _telnetEnabled = true;
                  _telnetConfigured = true;
                }
              });
              _refreshAutomaticName();
            },
          ),
        ],
      ],
    );

    return Stack(
      children: [
        editor,
        Positioned.fill(
          child: IgnorePointer(
            ignoring: !_themeGalleryOpen,
            child: AnimatedOpacity(
              opacity: _themeGalleryOpen ? 1 : 0,
              duration: const Duration(milliseconds: 130),
              curve: Curves.easeOutCubic,
              child: AnimatedSlide(
                offset: _themeGalleryOpen ? Offset.zero : const Offset(0.08, 0),
                duration: const Duration(milliseconds: 150),
                curve: Curves.easeOutCubic,
                child: _ThemeGalleryDrawer(
                  themesFuture:
                      _themeGalleryFuture ??
                      Future.value(const <StoredTerminalTheme>[]),
                  selectedThemeId:
                      _themeGalleryTarget == _HostThemeTarget.telnet
                      ? _telnetThemeId
                      : _themeId,
                  onClose: () => setState(() => _themeGalleryOpen = false),
                  onSelected: (themeId, selectedTheme) {
                    setState(() {
                      if (_themeGalleryTarget == _HostThemeTarget.telnet) {
                        _telnetThemeId = normalizeTerminalThemeId(themeId);
                        _selectedTelnetTheme = selectedTheme;
                      } else {
                        _themeId = normalizeTerminalThemeId(themeId);
                        _selectedTheme = selectedTheme;
                      }
                      _themeGalleryOpen = false;
                    });
                  },
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

enum _SshCredentialKind { key, certificate }

extension on _SshCredentialKind {
  String get label => switch (this) {
    _SshCredentialKind.key => tr('common.label.key', fallback: 'Key'),
    _SshCredentialKind.certificate => tr(
      'workspace.label.certificate',
      fallback: 'Certificate',
    ),
  };

  String get createLabel => switch (this) {
    _SshCredentialKind.key => tr(
      'workspace.credentials.createKey',
      fallback: 'Create key',
    ),
    _SshCredentialKind.certificate => tr(
      'workspace.credentials.createCertificate',
      fallback: 'Create certificate',
    ),
  };

  IconData get icon => switch (this) {
    _SshCredentialKind.key => Icons.key_rounded,
    _SshCredentialKind.certificate => Icons.workspace_premium_rounded,
  };
}

class _SshCredentialControl extends StatefulWidget {
  const _SshCredentialControl({
    required this.kind,
    required this.keyId,
    required this.keys,
    required this.onKindSelected,
    required this.onChanged,
    required this.onCreate,
    required this.onCleared,
  });

  final _SshCredentialKind? kind;
  final int? keyId;
  final List<KeyEntry> keys;
  final ValueChanged<_SshCredentialKind> onKindSelected;
  final ValueChanged<int?> onChanged;
  final ValueChanged<String> onCreate;
  final VoidCallback onCleared;

  @override
  State<_SshCredentialControl> createState() => _SshCredentialControlState();
}

class _SshCredentialControlState extends State<_SshCredentialControl> {
  int _activationToken = 0;

  void _handleKindSelected(_SshCredentialKind value) {
    setState(() => _activationToken++);
    widget.onKindSelected(value);
  }

  @override
  Widget build(BuildContext context) {
    final selectedKind = widget.kind;
    final kindPicker = Align(
      alignment: Alignment.centerLeft,
      child: _WorkspaceDropdown<_SshCredentialKind>(
        width: 190,
        entries: [
          for (final value in _SshCredentialKind.values)
            if (selectedKind == null || value != selectedKind)
              NautermContextMenuAction(
                value: value,
                label: value.label,
                icon: value.icon,
              ),
        ],
        onSelected: _handleKindSelected,
        triggerBuilder: (open) => Material(
          key: const ValueKey('ssh-add-credential'),
          color: Colors.transparent,
          child: InkWell(
            borderRadius: BorderRadius.circular(5),
            onTap: open,
            child: FittedBox(
              fit: BoxFit.scaleDown,
              alignment: Alignment.centerLeft,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 2, vertical: 4),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(Icons.add_rounded, size: 15, color: _mutedText),
                    SizedBox(width: 5),
                    Text(
                      tr(
                        'workspace.credentials.keyOrCertificate',
                        fallback: 'Key or Certificate',
                      ),
                      style: TextStyle(
                        color: _mutedText,
                        fontSize: NautermFontSizes.labelMedium,
                        fontWeight: NautermFontWeights.medium,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
    if (selectedKind == null) {
      return kindPicker;
    }

    // Key is the complete credential catalog; Certificate is a filtered view
    // for credentials that carry OpenSSH certificate material.
    final matchingKeys = filterSshCredentialKeysForTesting(
      widget.keys,
      certificatesOnly: selectedKind == _SshCredentialKind.certificate,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _WorkspaceSelect<int?>(
          label: selectedKind.label,
          value: widget.keyId,
          editable: true,
          searchable: true,
          clearable: true,
          createLabel: selectedKind.createLabel,
          onCreate: widget.onCreate,
          onCleared: widget.onCleared,
          activationToken: _activationToken,
          createConflictLabels: matchingKeys.map((entry) => entry.name),
          items: [
            for (final key in matchingKeys)
              DropdownMenuItem<int?>(value: key.id, child: Text(key.name)),
          ],
          onChanged: (value) {
            widget.onChanged(value);
          },
        ),
        SizedBox(height: _workspaceFormFieldGap),
        kindPicker,
      ],
    );
  }
}

IdentityEntry? _identityById(
  Iterable<IdentityEntry> identities,
  int? identityId,
) {
  if (identityId == null) return null;
  return identities.where((entry) => entry.id == identityId).firstOrNull;
}

String _suggestHostName({
  required NautermHostType type,
  required String host,
  required bool sshEnabled,
  required String sshUsername,
  required String sshPort,
  required bool telnetEnabled,
  required String telnetUsername,
  required String telnetPort,
}) {
  if (type != NautermHostType.remote) return '';
  final address = host.trim();
  if (address.isEmpty) return '';

  String compose(String username, String port) {
    final user = username.trim();
    final normalizedPort = port.trim();
    final target = user.isEmpty ? address : '$user@$address';
    return normalizedPort.isEmpty ? target : '$target:$normalizedPort';
  }

  if (sshEnabled) {
    final suggested = compose(sshUsername, sshPort);
    if (suggested.isNotEmpty) return suggested;
  }
  if (telnetEnabled) {
    return compose(telnetUsername, telnetPort);
  }
  return '';
}

_SshCredentialKind? _credentialKindForKeyId(
  Iterable<KeyEntry> keys,
  int? keyId,
) {
  if (keyId == null) return null;
  final key = keys.where((entry) => entry.id == keyId).firstOrNull;
  if (key == null) return null;
  return sshCredentialUsesCertificateForTesting(key)
      ? _SshCredentialKind.certificate
      : _SshCredentialKind.key;
}

@visibleForTesting
List<KeyEntry> filterSshCredentialKeysForTesting(
  Iterable<KeyEntry> keys, {
  required bool certificatesOnly,
}) {
  return [
    for (final key in keys)
      if (!certificatesOnly || sshCredentialUsesCertificateForTesting(key)) key,
  ];
}

@visibleForTesting
bool sshCredentialUsesCertificateForTesting(KeyEntry key) {
  return key.certificate != null;
}

enum _HostThemeTarget { ssh, telnet }

enum _ProtocolMenuAction { createGroup, remove }

class _ProtocolFormSection extends StatelessWidget {
  const _ProtocolFormSection({
    required this.protocol,
    required this.portController,
    required this.onRemove,
    required this.credentials,
    required this.theme,
    this.onCreateGroup,
    this.moreChildren = const [],
    this.showMore = false,
    this.onShowMoreChanged,
  });

  final String protocol;
  final TextEditingController portController;
  final VoidCallback onRemove;
  final List<Widget> credentials;
  final Widget theme;
  final VoidCallback? onCreateGroup;
  final List<Widget> moreChildren;
  final bool showMore;
  final ValueChanged<bool>? onShowMoreChanged;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: _card,
        borderRadius: BorderRadius.circular(10),
      ),
      child: Padding(
        padding: EdgeInsets.fromLTRB(
          16,
          16,
          16,
          moreChildren.isNotEmpty && !showMore ? 12 : 16,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            LayoutBuilder(
              builder: (context, constraints) {
                final protocolControls = <Widget>[
                  Text(
                    '$protocol on',
                    style: TextStyle(
                      color: _text,
                      fontSize: NautermFontSizes.labelLarge,
                      fontWeight: NautermFontWeights.semibold,
                    ),
                  ),
                  SizedBox(
                    key: ValueKey('protocol-port:$protocol'),
                    width: 64,
                    child: _WorkspaceInput(
                      size: _WorkspaceControlSize.small,
                      controller: portController,
                      label: 'Port',
                      floatingLabel: false,
                      clearable: false,
                      keyboardType: TextInputType.number,
                      inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                    ),
                  ),
                  Text(
                    'port',
                    style: TextStyle(
                      color: _text,
                      fontSize: NautermFontSizes.labelLarge,
                      fontWeight: NautermFontWeights.semibold,
                    ),
                  ),
                ];
                final actionsButton = _WorkspaceDropdown<_ProtocolMenuAction>(
                  width: 190,
                  entries: [
                    if (onCreateGroup != null) ...[
                      const NautermContextMenuAction<_ProtocolMenuAction>(
                        value: _ProtocolMenuAction.createGroup,
                        label: 'Create group',
                        icon: Icons.dashboard_customize_rounded,
                      ),
                      const NautermContextMenuDivider<_ProtocolMenuAction>(),
                    ],
                    NautermContextMenuAction<_ProtocolMenuAction>(
                      value: _ProtocolMenuAction.remove,
                      label: tr(
                        'workspace.host.protocol.remove',
                        fallback: 'Remove protocol',
                      ),
                      icon: LucideIcons.trash2,
                      destructive: true,
                    ),
                  ],
                  onSelected: (action) {
                    switch (action) {
                      case _ProtocolMenuAction.createGroup:
                        onCreateGroup?.call();
                      case _ProtocolMenuAction.remove:
                        onRemove();
                    }
                  },
                  triggerBuilder: (openMenu) => KeyedSubtree(
                    key: ValueKey('protocol-actions:$protocol'),
                    child: _WorkspaceDrawerHeaderButton(
                      icon: LucideIcons.ellipsis,
                      tooltip: '$protocol actions',
                      onPressed: openMenu,
                    ),
                  ),
                );

                if (constraints.maxWidth < 210) {
                  return Wrap(
                    spacing: 6,
                    runSpacing: 6,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [...protocolControls, actionsButton],
                  );
                }

                return Row(
                  children: [
                    Expanded(
                      child: Wrap(
                        spacing: 6,
                        runSpacing: 6,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: protocolControls,
                      ),
                    ),
                    SizedBox(width: 6),
                    actionsButton,
                  ],
                );
              },
            ),
            Divider(height: 25, color: _sidebarDivider),
            ...credentials,
            Divider(height: 25, color: _sidebarDivider),
            if (moreChildren.isNotEmpty && showMore) ...[
              ...moreChildren,
              SizedBox(height: 12),
              Divider(height: 1, color: _sidebarDivider),
              SizedBox(height: 12),
              theme,
            ] else ...[
              theme,
              if (moreChildren.isNotEmpty) ...[
                SizedBox(height: 8),
                _ProtocolShowMoreButton(
                  onPressed: () => onShowMoreChanged?.call(true),
                ),
              ],
            ],
          ],
        ),
      ),
    );
  }
}

class _ProtocolShowMoreButton extends StatelessWidget {
  const _ProtocolShowMoreButton({required this.onPressed});

  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: Alignment.centerLeft,
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(5),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          key: const ValueKey('protocol-show-more'),
          onTap: onPressed,
          hoverColor: _workspaceDark ? _sidebarHover : const Color(0xfff1f5f6),
          splashColor: _workspaceDark
              ? _workspaceMenuPressed
              : const Color(0xffdbe8eb).withValues(alpha: 0.42),
          highlightColor: Colors.transparent,
          child: FittedBox(
            fit: BoxFit.scaleDown,
            alignment: Alignment.centerLeft,
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  tr('workspace.host.protocol.showMore', fallback: 'Show more'),
                  style: TextStyle(
                    color: _mutedText,
                    fontSize: NautermFontSizes.labelMedium,
                    fontWeight: NautermFontWeights.medium,
                  ),
                ),
                SizedBox(width: 3),
                Icon(
                  Icons.keyboard_arrow_down_rounded,
                  size: 17,
                  color: _mutedText,
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _AddProtocolControl extends StatefulWidget {
  const _AddProtocolControl({
    required this.sshAvailable,
    required this.telnetAvailable,
    required this.onSelected,
  });

  final bool sshAvailable;
  final bool telnetAvailable;
  final ValueChanged<String> onSelected;

  @override
  State<_AddProtocolControl> createState() => _AddProtocolControlState();
}

class _AddProtocolControlState extends State<_AddProtocolControl> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    return _WorkspaceDropdown<String>(
      width: 180,
      entries: [
        if (widget.sshAvailable)
          const NautermContextMenuAction<String>(
            value: 'ssh',
            label: 'SSH',
            icon: Icons.terminal_rounded,
          ),
        if (widget.telnetAvailable)
          const NautermContextMenuAction<String>(
            value: 'telnet',
            label: 'Telnet',
            icon: Icons.settings_ethernet_rounded,
          ),
      ],
      onSelected: widget.onSelected,
      triggerBuilder: (openMenu) => MouseRegion(
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: Material(
          color: _hovered ? _cardHover : _card,
          borderRadius: BorderRadius.circular(10),
          animationDuration: const Duration(milliseconds: 120),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            key: const ValueKey('add-protocol'),
            onTap: openMenu,
            borderRadius: BorderRadius.circular(10),
            hoverColor: Colors.transparent,
            splashColor: _workspaceDark
                ? _workspaceMenuPressed
                : const Color(0xffdbe8eb).withValues(alpha: 0.42),
            highlightColor: _workspaceDark
                ? _sidebarHover.withValues(alpha: 0.72)
                : const Color(0xffe7eff1).withValues(alpha: 0.36),
            child: SizedBox(
              width: double.infinity,
              height: 42,
              child: Wrap(
                alignment: WrapAlignment.center,
                runAlignment: WrapAlignment.center,
                crossAxisAlignment: WrapCrossAlignment.center,
                spacing: 8,
                children: [
                  Container(
                    width: 24,
                    height: 24,
                    decoration: BoxDecoration(
                      color: _blue.withValues(alpha: 0.1),
                      borderRadius: BorderRadius.circular(7),
                    ),
                    alignment: Alignment.center,
                    child: Icon(Icons.add_rounded, size: 16, color: _blue),
                  ),
                  Text(
                    tr('workspace.host.protocol.add', fallback: 'Add protocol'),
                    style: TextStyle(
                      color: _text,
                      fontSize: NautermFontSizes.labelMedium,
                      fontWeight: NautermFontWeights.semibold,
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
}

const List<String> _hostEncodingOptions = [
  defaultHostEncoding,
  'GBK',
  'GB18030',
  'Big5',
  'ISO-8859-1',
];

String _normalizeHostEncoding(String? encoding) {
  final normalized = _emptyToNull(encoding);
  if (normalized == null) {
    return defaultHostEncoding;
  }
  return _hostEncodingOptions.contains(normalized)
      ? normalized
      : defaultHostEncoding;
}

class _HostThemeSelector extends StatelessWidget {
  const _HostThemeSelector({
    required this.selectedTheme,
    required this.onOpenGallery,
  });

  final StoredTerminalTheme? selectedTheme;
  final VoidCallback onOpenGallery;

  @override
  Widget build(BuildContext context) {
    final selected = selectedTheme;
    final previewTheme = selected?.theme ?? defaultTerminalTheme;
    final title = selected == null ? 'Default' : selected.theme.name;

    return Material(
      color: Colors.transparent,
      child: InkWell(
        borderRadius: BorderRadius.circular(9),
        onTap: onOpenGallery,
        child: TerminalThemePreviewCard(
          title: title,
          theme: previewTheme,
          selected: true,
          compact: true,
        ),
      ),
    );
  }
}
