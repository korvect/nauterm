part of 'nauterm_workspace.dart';

class _IdentityEditorContent extends StatefulWidget {
  const _IdentityEditorContent({
    required this.request,
    required this.keys,
    required this.onClose,
    required this.onCreateCredential,
    required this.onSave,
    required this.onDuplicate,
    required this.onDelete,
  });

  final _IdentityEditorRequest request;
  final List<KeyEntry> keys;
  final VoidCallback onClose;
  final _CreateRelatedCredential onCreateCredential;
  final _SaveIdentity onSave;
  final ValueChanged<IdentityEntry> onDuplicate;
  final ValueChanged<IdentityEntry> onDelete;

  @override
  State<_IdentityEditorContent> createState() => _IdentityEditorContentState();
}

class _IdentityEditorContentState extends State<_IdentityEditorContent> {
  late final TextEditingController _nameController;
  late final TextEditingController _usernameController;
  late final TextEditingController _passwordController;
  int? _keyId;
  _SshCredentialKind? _credentialKind;
  bool _saving = false;
  String? _error;
  String? _usernameError;
  late bool _nameIsAutomatic;
  bool _settingAutomaticName = false;

  @override
  void initState() {
    super.initState();
    final initial = widget.request.initial;
    _nameController = TextEditingController(
      text: initial?.name ?? widget.request.initialName ?? '',
    );
    _usernameController = TextEditingController(text: initial?.username ?? '');
    _passwordController = TextEditingController(text: initial?.password ?? '');
    _keyId = initial?.keyId;
    _credentialKind = _credentialKindForKeyId(widget.keys, _keyId);
    _nameIsAutomatic = _nameController.text.trim().isEmpty;
    _nameController.addListener(_handleNameChanged);
    _usernameController.addListener(_handleUsernameChanged);
    if (_keyId != null && !widget.keys.any((key) => key.id == _keyId)) {
      _keyId = null;
    }
    _refreshAutomaticName();
  }

  @override
  void dispose() {
    _nameController.removeListener(_handleNameChanged);
    _usernameController.removeListener(_handleUsernameChanged);
    _nameController.dispose();
    _usernameController.dispose();
    _passwordController.dispose();
    super.dispose();
  }

  @override
  void didUpdateWidget(covariant _IdentityEditorContent oldWidget) {
    super.didUpdateWidget(oldWidget);
    _refreshAutomaticName();
  }

  void _handleNameChanged() {
    if (_settingAutomaticName) return;
    _nameIsAutomatic = _nameController.text.trim().isEmpty;
  }

  void _handleUsernameChanged() {
    _refreshAutomaticName();
    if (_usernameError != null && _usernameController.text.trim().isNotEmpty) {
      setState(() => _usernameError = null);
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
    final username = _usernameController.text.trim();
    final keyName = widget.keys
        .where((key) => key.id == _keyId)
        .firstOrNull
        ?.name
        .trim();
    return switch ((username.isEmpty, keyName?.isEmpty ?? true)) {
      (false, false) => '$username:$keyName',
      (false, true) => username,
      (true, false) => keyName!,
      (true, true) => '',
    };
  }

  void _selectCredentialKind(_SshCredentialKind kind) {
    setState(() {
      if (_credentialKind != kind) {
        _keyId = null;
      }
      _credentialKind = kind;
    });
    _refreshAutomaticName();
  }

  void _createCredential(String initialName) {
    final kind = _credentialKind;
    if (kind == null) {
      return;
    }
    widget.onCreateCredential(
      initialName,
      certificate: kind == _SshCredentialKind.certificate,
      onCreated: (id) {
        if (mounted) {
          setState(() {
            _keyId = id;
            _credentialKind = _SshCredentialKind.key;
          });
          _refreshAutomaticName();
        }
      },
    );
  }

  Future<void> _save() async {
    final username = _usernameController.text.trim();
    if (username.isEmpty) {
      setState(() {
        _usernameError = 'Username is required.';
        _error = null;
      });
      return;
    }
    final name = _nameController.text.trim().isNotEmpty
        ? _nameController.text.trim()
        : _suggestedName();
    if (name.isEmpty) {
      setState(() => _error = 'Name is required.');
      return;
    }

    setState(() {
      _saving = true;
      _error = null;
    });
    await widget.onSave(
      IdentityEntry(
        id: widget.request.initial?.id,
        name: name,
        username: username,
        password: _emptyToNull(_passwordController.text),
        keyId: _keyId,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final initial = widget.request.initial;

    return _EditorShell(
      title: initial == null ? 'New Identity' : 'Edit Identity',
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
          title: 'Identity',
          children: [
            _WorkspaceInput(
              controller: _nameController,
              label: 'Name',
              autofocus: true,
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _WorkspaceInput(
              controller: _usernameController,
              label: 'Username',
              isRequired: true,
              errorText: _usernameError,
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _WorkspaceInput(
              controller: _passwordController,
              label: 'Password',
              obscureText: true,
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _SshCredentialControl(
              kind: _credentialKind,
              keyId: _keyId,
              keys: widget.keys,
              onKindSelected: _selectCredentialKind,
              onChanged: (value) {
                setState(() => _keyId = value);
                _refreshAutomaticName();
              },
              onCreate: _createCredential,
              onCleared: () {
                setState(() {
                  _keyId = null;
                  _credentialKind = null;
                });
                _refreshAutomaticName();
              },
            ),
          ],
        ),
      ],
    );
  }
}

class _PortForwardEditorContent extends StatefulWidget {
  const _PortForwardEditorContent({
    required this.request,
    required this.hosts,
    required this.onClose,
    required this.onSave,
    required this.onDelete,
  });

  final _PortForwardEditorRequest request;
  final List<HostEntry> hosts;
  final VoidCallback onClose;
  final _SavePortForward onSave;
  final ValueChanged<PortForwardEntry> onDelete;

  @override
  State<_PortForwardEditorContent> createState() =>
      _PortForwardEditorContentState();
}

class _WorkspaceAddressPortFields extends StatelessWidget {
  const _WorkspaceAddressPortFields({
    required this.address,
    required this.port,
  });

  final Widget address;
  final Widget port;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        address,
        SizedBox(height: _workspaceFormFieldGap),
        port,
      ],
    );
  }
}

class _PortForwardEditorContentState extends State<_PortForwardEditorContent> {
  late final TextEditingController _nameController;
  late final TextEditingController _bindAddressController;
  late final TextEditingController _bindPortController;
  late final TextEditingController _destinationHostController;
  late final TextEditingController _destinationPortController;
  late String _type;
  late int? _hostId;
  bool _saving = false;
  String? _error;
  String? _nameError;
  String? _bindPortError;
  String? _hostError;
  String? _destinationHostError;
  String? _destinationPortError;

  @override
  void initState() {
    super.initState();
    final initial = widget.request.initial;
    _type = _normalizePortForwardType(
      initial?.type ?? widget.request.initialType,
    );
    _hostId = initial?.connectionId;
    _nameController = TextEditingController(text: initial?.name ?? '');
    _bindAddressController = TextEditingController(
      text: initial?.bindAddress ?? '127.0.0.1',
    );
    _bindPortController = TextEditingController(
      text: initial == null || initial.bindPort <= 0
          ? ''
          : initial.bindPort.toString(),
    );
    _destinationHostController = TextEditingController(
      text: initial?.destinationHost ?? '',
    );
    _destinationPortController = TextEditingController(
      text: initial == null || initial.destinationPort <= 0
          ? ''
          : initial.destinationPort.toString(),
    );
    _nameController.addListener(_clearNameError);
    _bindPortController.addListener(_clearBindPortError);
    _destinationHostController.addListener(_clearDestinationHostError);
    _destinationPortController.addListener(_clearDestinationPortError);
  }

  @override
  void dispose() {
    _nameController.removeListener(_clearNameError);
    _bindPortController.removeListener(_clearBindPortError);
    _destinationHostController.removeListener(_clearDestinationHostError);
    _destinationPortController.removeListener(_clearDestinationPortError);
    _nameController.dispose();
    _bindAddressController.dispose();
    _bindPortController.dispose();
    _destinationHostController.dispose();
    _destinationPortController.dispose();
    super.dispose();
  }

  void _clearNameError() {
    if (_nameError != null && _nameController.text.trim().isNotEmpty) {
      setState(() => _nameError = null);
    }
  }

  void _clearBindPortError() {
    final port = int.tryParse(_bindPortController.text.trim()) ?? 0;
    if (_bindPortError != null && _isValidPort(port)) {
      setState(() => _bindPortError = null);
    }
  }

  void _clearDestinationHostError() {
    if (_destinationHostError != null &&
        _destinationHostController.text.trim().isNotEmpty) {
      setState(() => _destinationHostError = null);
    }
  }

  void _clearDestinationPortError() {
    final port = int.tryParse(_destinationPortController.text.trim()) ?? 0;
    if (_destinationPortError != null && _isValidPort(port)) {
      setState(() => _destinationPortError = null);
    }
  }

  Future<void> _save() async {
    final name = _nameController.text.trim();
    final bindAddress =
        _emptyToNull(_bindAddressController.text) ?? '127.0.0.1';
    final bindPort = int.tryParse(_bindPortController.text.trim()) ?? 0;
    final destinationHost = _destinationHostController.text.trim();
    final destinationPort =
        int.tryParse(_destinationPortController.text.trim()) ?? 0;
    final hostId = _hostId;

    final nameError = name.isEmpty
        ? tr(
            'workspace.portForward.validation.nameRequired',
            fallback: 'Rule name is required.',
          )
        : null;
    final bindPortError = _isValidPort(bindPort)
        ? null
        : _type == 'remote'
        ? tr(
            'workspace.portForward.validation.remotePortRequired',
            fallback: 'Remote port number is required.',
          )
        : tr(
            'workspace.portForward.validation.localPortRequired',
            fallback: 'Local port number is required.',
          );
    final hostError = hostId != null && hostId > 0
        ? null
        : _type == 'remote'
        ? tr(
            'workspace.portForward.validation.remoteHostRequired',
            fallback: 'Select a remote host.',
          )
        : tr(
            'workspace.portForward.validation.intermediateHostRequired',
            fallback: 'Select an intermediate host.',
          );
    final destinationHostError =
        _type == 'dynamic' || destinationHost.isNotEmpty
        ? null
        : tr(
            'workspace.portForward.validation.destinationAddressRequired',
            fallback: 'Destination address is required.',
          );
    final destinationPortError =
        _type == 'dynamic' || _isValidPort(destinationPort)
        ? null
        : tr(
            'workspace.portForward.validation.destinationPortRequired',
            fallback: 'Destination port number is required.',
          );
    if (nameError != null ||
        bindPortError != null ||
        hostError != null ||
        destinationHostError != null ||
        destinationPortError != null) {
      setState(() {
        _nameError = nameError;
        _bindPortError = bindPortError;
        _hostError = hostError;
        _destinationHostError = destinationHostError;
        _destinationPortError = destinationPortError;
        _error = null;
      });
      return;
    }

    setState(() {
      _saving = true;
      _error = null;
    });

    try {
      final initial = widget.request.initial;
      await widget.onSave(
        PortForwardEntry(
          id: initial?.id,
          name: name,
          type: _type,
          bindAddress: bindAddress,
          bindPort: bindPort,
          destinationHost: _type == 'dynamic' ? '' : destinationHost,
          destinationPort: _type == 'dynamic' ? 0 : destinationPort,
          connectionId: hostId!,
        ),
      );
    } catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _saving = false;
        _error = 'Failed to save forward: $error';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final initial = widget.request.initial;
    final isDynamic = _type == 'dynamic';
    final isRemote = _type == 'remote';
    final bindPortLabel = isRemote
        ? tr(
            'workspace.portForward.field.remotePort',
            fallback: 'Remote port number',
          )
        : tr(
            'workspace.portForward.field.localPort',
            fallback: 'Local port number',
          );
    final hostLabel = isRemote
        ? tr('workspace.portForward.field.remoteHost', fallback: 'Remote host')
        : tr(
            'workspace.portForward.field.intermediateHost',
            fallback: 'Intermediate host',
          );

    return _EditorShell(
      title: initial == null
          ? tr('workspace.label.newForwarding', fallback: 'New forwarding')
          : tr('workspace.label.editForward', fallback: 'Edit Forward'),
      saving: _saving,
      error: _error,
      onClose: widget.onClose,
      onSave: _save,
      headerActions: initial == null || _saving
          ? const []
          : [_EditorShellMenuAction.delete(() => widget.onDelete(initial))],
      children: [
        _WorkspaceFormSection(
          title: tr('common.label.rule', fallback: 'Rule'),
          children: [
            _WorkspaceInput(
              controller: _nameController,
              label: tr('workspace.label.ruleName', fallback: 'Rule name'),
              isRequired: true,
              errorText: _nameError,
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _WorkspaceSelect<String>(
              label: tr('common.label.type', fallback: 'Type'),
              value: _type,
              onChanged: (type) {
                if (type == null) return;
                setState(() {
                  _type = type;
                  if (_type == 'dynamic') {
                    _destinationHostController.clear();
                    _destinationPortController.clear();
                  }
                });
              },
              items: [
                DropdownMenuItem(
                  value: 'local',
                  child: Text(tr('common.label.local', fallback: 'Local')),
                ),
                DropdownMenuItem(
                  value: 'remote',
                  child: Text(tr('common.label.remote', fallback: 'Remote')),
                ),
                DropdownMenuItem(
                  value: 'dynamic',
                  child: Text(tr('common.label.dynamic', fallback: 'Dynamic')),
                ),
              ],
            ),
          ],
        ),
        SizedBox(height: 14),
        _WorkspaceFormSection(
          title: isRemote
              ? tr(
                  'workspace.portForward.section.remotePortAddress',
                  fallback: 'Remote Port Address',
                )
              : tr(
                  'workspace.portForward.section.localPortAddress',
                  fallback: 'Local Port Address',
                ),
          children: [
            _WorkspaceAddressPortFields(
              address: _WorkspaceInput(
                controller: _bindAddressController,
                label: tr(
                  'workspace.label.bindAddress',
                  fallback: 'Bind Address',
                ),
              ),
              port: _WorkspaceInput(
                controller: _bindPortController,
                label: bindPortLabel,
                isRequired: true,
                errorText: _bindPortError,
                keyboardType: TextInputType.number,
                inputFormatters: [FilteringTextInputFormatter.digitsOnly],
              ),
            ),
          ],
        ),
        SizedBox(height: 14),
        _WorkspaceFormSection(
          title: isRemote
              ? tr(
                  'workspace.portForward.section.remoteHost',
                  fallback: 'Remote Host',
                )
              : tr(
                  'workspace.portForward.section.intermediateHost',
                  fallback: 'Intermediate Host',
                ),
          children: [
            _WorkspaceSelect<int?>(
              label: hostLabel,
              isRequired: true,
              value: _hostId,
              errorText: _hostError,
              editable: true,
              searchable: true,
              clearable: true,
              onChanged: (value) => setState(() {
                _hostId = value;
                if (value != null) {
                  _hostError = null;
                }
              }),
              items: [
                for (final host in widget.hosts)
                  if (host.id != null && host.type == NautermHostType.remote)
                    DropdownMenuItem<int?>(
                      value: host.id,
                      child: Text(_portForwardHostLabel(host)),
                    ),
              ],
            ),
          ],
        ),
        if (!isDynamic) ...[
          SizedBox(height: 14),
          _WorkspaceFormSection(
            title: tr('common.label.destination', fallback: 'Destination'),
            children: [
              _WorkspaceAddressPortFields(
                address: _WorkspaceInput(
                  controller: _destinationHostController,
                  label: tr(
                    'workspace.portForward.field.destinationAddress',
                    fallback: 'Destination address',
                  ),
                  isRequired: true,
                  errorText: _destinationHostError,
                ),
                port: _WorkspaceInput(
                  controller: _destinationPortController,
                  label: tr(
                    'workspace.portForward.field.destinationPort',
                    fallback: 'Destination port number',
                  ),
                  isRequired: true,
                  errorText: _destinationPortError,
                  keyboardType: TextInputType.number,
                  inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                ),
              ),
            ],
          ),
        ],
      ],
    );
  }
}

String _normalizePortForwardType(String? value) {
  final normalized = value?.trim().toLowerCase();
  return switch (normalized) {
    'remote' => 'remote',
    'dynamic' => 'dynamic',
    _ => 'local',
  };
}

String _portForwardHostLabel(HostEntry host) {
  final address = _emptyToNull(host.host);
  if (address == null) {
    return host.name;
  }
  final port = host.port ?? 22;
  return '${host.name} ($address:$port)';
}

bool _isValidPort(int port) => port >= 1 && port <= 65535;

class _ProxyEditorContent extends StatefulWidget {
  const _ProxyEditorContent({
    required this.request,
    required this.identities,
    required this.onClose,
    required this.onCreateIdentity,
    required this.onSave,
    required this.onDuplicate,
    required this.onDelete,
  });

  final _ProxyEditorRequest request;
  final List<IdentityEntry> identities;
  final VoidCallback onClose;
  final _CreateRelatedEntry onCreateIdentity;
  final _SaveProxy onSave;
  final ValueChanged<ProxyEntry> onDuplicate;
  final ValueChanged<ProxyEntry> onDelete;

  @override
  State<_ProxyEditorContent> createState() => _ProxyEditorContentState();
}

class _ProxyEditorContentState extends State<_ProxyEditorContent> {
  late final TextEditingController _nameController;
  late final TextEditingController _hostController;
  late final TextEditingController _portController;
  late final TextEditingController _usernameController;
  late final TextEditingController _passwordController;
  late String _type;
  late int? _identityId;
  bool _saving = false;
  String? _error;
  String? _nameError;
  String? _hostError;
  String? _portError;

  @override
  void initState() {
    super.initState();
    final initial = widget.request.initial;
    _type = _normalizeProxyType(initial?.type);
    _identityId = initial?.identityId;
    _nameController = TextEditingController(
      text: initial?.name ?? widget.request.initialName ?? '',
    );
    _hostController = TextEditingController(text: initial?.host ?? '');
    _portController = TextEditingController(
      text: initial == null || initial.port <= 0 ? '' : initial.port.toString(),
    );
    _usernameController = TextEditingController(text: initial?.username ?? '');
    _passwordController = TextEditingController(text: initial?.password ?? '');
    _nameController.addListener(_clearNameError);
    _hostController.addListener(_clearHostError);
    _portController.addListener(_clearPortError);
    if (_identityId != null &&
        !widget.identities.any((identity) => identity.id == _identityId)) {
      _identityId = null;
    }
    _showSelectedIdentityName();
  }

  @override
  void dispose() {
    _nameController.removeListener(_clearNameError);
    _hostController.removeListener(_clearHostError);
    _portController.removeListener(_clearPortError);
    _nameController.dispose();
    _hostController.dispose();
    _portController.dispose();
    _usernameController.dispose();
    _passwordController.dispose();
    super.dispose();
  }

  void _clearNameError() {
    if (_nameError != null && _nameController.text.trim().isNotEmpty) {
      setState(() => _nameError = null);
    }
  }

  void _clearHostError() {
    if (_hostError != null && _hostController.text.trim().isNotEmpty) {
      setState(() => _hostError = null);
    }
  }

  void _clearPortError() {
    final port = int.tryParse(_portController.text.trim()) ?? 0;
    if (_portError != null && _isValidPort(port)) {
      setState(() => _portError = null);
    }
  }

  IdentityEntry? get _selectedIdentity =>
      _identityById(widget.identities, _identityId);

  void _showSelectedIdentityName() {
    final identity = _selectedIdentity;
    if (identity == null) return;
    _usernameController.text = identity.name;
    _passwordController.clear();
  }

  void _selectIdentity(int? identityId) {
    setState(() {
      _identityId = identityId;
      if (identityId == null) {
        _usernameController.clear();
        return;
      }
      _showSelectedIdentityName();
    });
  }

  void _handleIdentityTextChanged(String value) {
    final selectedName = _selectedIdentity?.name ?? '';
    if (_identityId != null && value != selectedName) {
      setState(() => _identityId = null);
    }
  }

  void _createIdentity(String initialName) {
    widget.onCreateIdentity(initialName, (id) {
      if (mounted) {
        setState(() {
          _identityId = id;
          _showSelectedIdentityName();
        });
      }
    });
  }

  Future<void> _save() async {
    final name = _nameController.text.trim();
    final host = _hostController.text.trim();
    final port = int.tryParse(_portController.text.trim()) ?? 0;

    final nameError = name.isEmpty
        ? tr(
            'workspace.proxy.validation.nameRequired',
            fallback: 'Proxy name is required.',
          )
        : null;
    final hostError = host.isEmpty
        ? tr(
            'workspace.proxy.validation.hostRequired',
            fallback: 'Proxy host is required.',
          )
        : null;
    final portError = _isValidPort(port)
        ? null
        : tr(
            'workspace.proxy.validation.portRequired',
            fallback: 'Proxy port is required.',
          );
    if (nameError != null || hostError != null || portError != null) {
      setState(() {
        _nameError = nameError;
        _hostError = hostError;
        _portError = portError;
        _error = null;
      });
      return;
    }

    setState(() {
      _saving = true;
      _error = null;
    });

    try {
      final initial = widget.request.initial;
      await widget.onSave(
        ProxyEntry(
          id: initial?.id,
          name: name,
          type: _type,
          host: host,
          port: port,
          identityId: _identityId,
          username: _identityId == null
              ? _emptyToNull(_usernameController.text)
              : null,
          password: _identityId == null
              ? _emptyToNull(_passwordController.text)
              : null,
        ),
      );
    } catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _saving = false;
        _error = 'Failed to save proxy: $error';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final initial = widget.request.initial;
    return _EditorShell(
      title: initial == null
          ? tr('workspace.label.newProxy', fallback: 'New Proxy')
          : tr('workspace.label.editProxy', fallback: 'Edit Proxy'),
      saving: _saving,
      error: _error,
      onClose: widget.onClose,
      onSave: _save,
      headerActions: initial == null || _saving
          ? const []
          : [
              _EditorShellMenuAction.duplicate(
                () => widget.onDuplicate(initial),
              ),
              _EditorShellMenuAction.delete(() => widget.onDelete(initial)),
            ],
      children: [
        _WorkspaceFormSection(
          title: tr('common.label.proxy', fallback: 'Proxy'),
          children: [
            _WorkspaceInput(
              controller: _nameController,
              label: tr('workspace.label.proxyName', fallback: 'Proxy name'),
              isRequired: true,
              errorText: _nameError,
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _WorkspaceSelect<String>(
              label: tr('common.label.type', fallback: 'Type'),
              value: _type,
              items: [
                DropdownMenuItem(value: 'http', child: Text('HTTP')),
                DropdownMenuItem(value: 'socks5', child: Text('SOCKS5')),
              ],
              onChanged: (value) {
                if (value != null) {
                  setState(() => _type = _normalizeProxyType(value));
                }
              },
            ),
          ],
        ),
        SizedBox(height: 14),
        _WorkspaceFormSection(
          title: tr('common.label.endpoint', fallback: 'Endpoint'),
          children: [
            _WorkspaceAddressPortFields(
              address: _WorkspaceInput(
                controller: _hostController,
                label: tr('common.label.host', fallback: 'Host'),
                isRequired: true,
                errorText: _hostError,
              ),
              port: _WorkspaceInput(
                controller: _portController,
                label: tr('common.label.port', fallback: 'Port'),
                isRequired: true,
                errorText: _portError,
                keyboardType: TextInputType.number,
                inputFormatters: [FilteringTextInputFormatter.digitsOnly],
              ),
            ),
          ],
        ),
        SizedBox(height: 14),
        _WorkspaceFormSection(
          title: tr('common.label.authentication', fallback: 'Authentication'),
          children: [
            _WorkspaceSelect<int?>(
              label: _identityId == null
                  ? tr('common.label.username', fallback: 'Username')
                  : tr('common.label.identity', fallback: 'Identity'),
              value: _identityId,
              editable: true,
              searchable: true,
              clearable: true,
              inputController: _usernameController,
              onTextChanged: _handleIdentityTextChanged,
              createLabel: tr(
                'workspace.label.createIdentity',
                fallback: 'Create identity',
              ),
              onCreate: _createIdentity,
              items: [
                for (final identity in widget.identities)
                  DropdownMenuItem<int?>(
                    value: identity.id,
                    child: Text(identity.name),
                  ),
              ],
              onChanged: _selectIdentity,
            ),
            if (_identityId == null) ...[
              SizedBox(height: _workspaceFormFieldGap),
              _WorkspaceSelect<int?>(
                label: tr('common.label.password', fallback: 'Password'),
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
                onChanged: _selectIdentity,
              ),
            ],
          ],
        ),
      ],
    );
  }
}

class _SnippetPackageEditorContent extends StatefulWidget {
  const _SnippetPackageEditorContent({
    required this.request,
    required this.onClose,
    required this.onSave,
    required this.onDelete,
  });

  final _SnippetPackageEditorRequest request;
  final VoidCallback onClose;
  final _SaveSnippetPackage onSave;
  final ValueChanged<_SnippetPackageItem> onDelete;

  @override
  State<_SnippetPackageEditorContent> createState() =>
      _SnippetPackageEditorContentState();
}

class _SnippetPackageEditorContentState
    extends State<_SnippetPackageEditorContent> {
  late final TextEditingController _nameController;
  bool _saving = false;
  String? _error;
  String? _nameError;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.request.initial?.name);
    _nameController.addListener(_clearNameError);
  }

  @override
  void dispose() {
    _nameController.removeListener(_clearNameError);
    _nameController.dispose();
    super.dispose();
  }

  void _clearNameError() {
    if (_nameError != null && _nameController.text.trim().isNotEmpty) {
      setState(() => _nameError = null);
    }
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

    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await widget.onSave(
        SnippetPackageEntry(id: widget.request.initial?.id, name: name),
      );
    } catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _saving = false;
        _error = 'Failed to save snippet package: $error';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return _EditorShell(
      title: widget.request.initial == null
          ? 'New Snippet Package'
          : 'Rename Snippet Package',
      saving: _saving,
      error: _error,
      onClose: widget.onClose,
      onSave: _save,
      headerActions: widget.request.initial == null || _saving
          ? const []
          : [
              _EditorShellMenuAction.delete(
                () => widget.onDelete(widget.request.initial!),
              ),
            ],
      children: [
        _WorkspaceFormSection(
          title: 'Package',
          children: [
            _WorkspaceInput(
              controller: _nameController,
              label: 'Name',
              isRequired: true,
              errorText: _nameError,
              autofocus: true,
            ),
          ],
        ),
      ],
    );
  }
}

class _SnippetEditorContent extends StatefulWidget {
  const _SnippetEditorContent({
    required this.request,
    required this.packages,
    required this.groups,
    required this.hosts,
    required this.onClose,
    required this.onCreatePackage,
    required this.onSave,
    required this.onDuplicate,
    required this.onDelete,
  });

  final _SnippetEditorRequest request;
  final List<_SnippetPackageItem> packages;
  final List<HostGroup> groups;
  final List<HostEntry> hosts;
  final VoidCallback onClose;
  final _CreateRelatedEntry onCreatePackage;
  final _SaveSnippet onSave;
  final ValueChanged<_SnippetItem> onDuplicate;
  final ValueChanged<_SnippetItem> onDelete;

  @override
  State<_SnippetEditorContent> createState() => _SnippetEditorContentState();
}

class _SnippetEditorContentState extends State<_SnippetEditorContent> {
  late final TextEditingController _descriptionController;
  late final TextEditingController _scriptController;
  late int? _packageId;
  late Set<int> _targetGroupIds;
  late Set<int> _targetHostIds;
  bool _saving = false;
  String? _error;
  String? _scriptError;

  @override
  void initState() {
    super.initState();
    final initial = widget.request.initial;
    _packageId =
        initial?.packageId ??
        widget.request.initialPackageId ??
        widget.packages.firstOrNull?.id;
    _targetGroupIds = {...initial?.targetGroupIds ?? const <int>[]};
    _targetHostIds = {...initial?.targetHostIds ?? const <int>[]};
    _descriptionController = TextEditingController(
      text: initial?.description ?? widget.request.initialDescription ?? '',
    );
    _scriptController = TextEditingController(
      text: initial?.script ?? widget.request.initialScript ?? '',
    );
    _scriptController.addListener(_clearScriptError);
  }

  @override
  void dispose() {
    _descriptionController.dispose();
    _scriptController.removeListener(_clearScriptError);
    _scriptController.dispose();
    super.dispose();
  }

  void _clearScriptError() {
    if (_scriptError != null && _scriptController.text.trim().isNotEmpty) {
      setState(() => _scriptError = null);
    }
  }

  void _createPackage(String initialName) {
    widget.onCreatePackage(initialName, (id) {
      if (mounted) {
        setState(() => _packageId = id);
      }
    });
  }

  Future<void> _save() async {
    final script = _scriptController.text.trim();
    if (script.isEmpty) {
      setState(() {
        _scriptError = 'Script is required.';
        _error = null;
      });
      return;
    }
    final description = _descriptionController.text.trim();

    setState(() {
      _saving = true;
      _error = null;
    });

    try {
      final scope = _targetGroupIds.isEmpty && _targetHostIds.isEmpty
          ? SnippetScope.global
          : SnippetScope.targeted;
      await widget.onSave(
        widget.request.initial,
        _SnippetDraft(
          packageId: _packageId,
          scope: scope,
          description: description.isEmpty ? 'Untitled snippet' : description,
          script: script,
          targetGroupIds: scope == SnippetScope.targeted
              ? _targetGroupIds.toList(growable: false)
              : const [],
          targetHostIds: scope == SnippetScope.targeted
              ? _targetHostIds.toList(growable: false)
              : const [],
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
    final initial = widget.request.initial;
    return _EditorShell(
      title: initial == null
          ? tr('workspace.label.newSnippet', fallback: 'New Snippet')
          : tr('workspace.label.editSnippet', fallback: 'Edit Snippet'),
      saving: _saving,
      error: _error,
      onClose: widget.onClose,
      onSave: _save,
      headerActions: initial == null || _saving
          ? const []
          : [
              _EditorShellMenuAction.duplicate(
                () => widget.onDuplicate(initial),
              ),
              _EditorShellMenuAction.delete(() => widget.onDelete(initial)),
            ],
      children: [
        _WorkspaceFormSection(
          title: 'Snippet',
          children: [
            _WorkspaceSelect<int?>(
              label: 'Package',
              value: _packageId,
              editable: true,
              searchable: true,
              clearable: true,
              createLabel: 'Create package',
              onCreate: _createPackage,
              onChanged: (value) {
                setState(() => _packageId = value);
              },
              items: [
                for (final package in widget.packages)
                  DropdownMenuItem<int?>(
                    value: package.id,
                    child: Text(package.name),
                  ),
              ],
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _WorkspaceInput(
              controller: _descriptionController,
              label: 'Description',
              autofocus: initial == null,
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _WorkspaceInput(
              controller: _scriptController,
              label: tr('common.label.script', fallback: 'Script'),
              isRequired: true,
              errorText: _scriptError,
              minLines: 6,
              maxLines: 10,
            ),
          ],
        ),
        SizedBox(height: 14),
        _WorkspaceFormSection(
          title: 'Targets',
          children: [
            _SnippetTargetList<HostGroup>(
              title: 'Groups',
              emptyLabel: 'No groups',
              items: widget.groups,
              selectedIds: _targetGroupIds,
              idOf: (group) => group.id,
              labelOf: (group) => group.name,
              onChanged: (id, selected) {
                setState(() {
                  if (selected) {
                    _targetGroupIds.add(id);
                  } else {
                    _targetGroupIds.remove(id);
                  }
                });
              },
            ),
            SizedBox(height: 12),
            _SnippetTargetList<HostEntry>(
              title: 'Hosts',
              emptyLabel: 'No hosts',
              items: widget.hosts,
              selectedIds: _targetHostIds,
              idOf: (host) => host.id,
              labelOf: (host) => host.name,
              subtitleOf: _snippetHostSubtitle,
              onChanged: (id, selected) {
                setState(() {
                  if (selected) {
                    _targetHostIds.add(id);
                  } else {
                    _targetHostIds.remove(id);
                  }
                });
              },
            ),
          ],
        ),
      ],
    );
  }
}

class _SnippetTargetList<T> extends StatelessWidget {
  const _SnippetTargetList({
    required this.title,
    required this.emptyLabel,
    required this.items,
    required this.selectedIds,
    required this.idOf,
    required this.labelOf,
    required this.onChanged,
    this.subtitleOf,
  });

  final String title;
  final String emptyLabel;
  final List<T> items;
  final Set<int> selectedIds;
  final int? Function(T item) idOf;
  final String Function(T item) labelOf;
  final String? Function(T item)? subtitleOf;
  final void Function(int id, bool selected) onChanged;

  @override
  Widget build(BuildContext context) {
    final availableItems = [
      for (final item in items)
        if (idOf(item) != null) item,
    ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title,
          style: TextStyle(
            color: _text,
            fontSize: NautermFontSizes.labelMedium,
            fontWeight: NautermFontWeights.semibold,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 8),
        DecoratedBox(
          decoration: BoxDecoration(
            border: Border.all(color: _sidebarDivider),
            borderRadius: BorderRadius.circular(8),
          ),
          child: availableItems.isEmpty
              ? Padding(
                  padding: const EdgeInsets.all(12),
                  child: Text(
                    emptyLabel,
                    style: TextStyle(
                      color: _mutedText,
                      fontSize: NautermFontSizes.labelMedium,
                      letterSpacing: 0,
                    ),
                  ),
                )
              : Column(
                  children: [
                    for (var index = 0; index < availableItems.length; index++)
                      _SnippetTargetRow(
                        id: idOf(availableItems[index])!,
                        label: labelOf(availableItems[index]),
                        subtitle: subtitleOf?.call(availableItems[index]),
                        selected: selectedIds.contains(
                          idOf(availableItems[index]),
                        ),
                        showDivider: index < availableItems.length - 1,
                        onChanged: onChanged,
                      ),
                  ],
                ),
        ),
      ],
    );
  }
}

class _SnippetTargetRow extends StatelessWidget {
  const _SnippetTargetRow({
    required this.id,
    required this.label,
    required this.selected,
    required this.showDivider,
    required this.onChanged,
    this.subtitle,
  });

  final int id;
  final String label;
  final String? subtitle;
  final bool selected;
  final bool showDivider;
  final void Function(int id, bool selected) onChanged;

  @override
  Widget build(BuildContext context) {
    Widget checkbox() {
      return Checkbox(
        value: selected,
        onChanged: (value) => onChanged(id, value ?? false),
        visualDensity: VisualDensity.compact,
        activeColor: _workspaceDark ? _blue : null,
        checkColor: _workspaceDark ? Colors.white : null,
        side: _workspaceDark ? BorderSide(color: _sidebarDivider) : null,
        fillColor: _workspaceDark
            ? WidgetStateProperty.resolveWith((states) {
                if (states.contains(WidgetState.selected)) {
                  return _blue;
                }
                return Colors.transparent;
              })
            : null,
        overlayColor: _workspaceDark
            ? WidgetStatePropertyAll(_blue.withValues(alpha: 0.10))
            : null,
      );
    }

    return DecoratedBox(
      decoration: BoxDecoration(
        border: showDivider
            ? Border(bottom: BorderSide(color: _sidebarDivider))
            : null,
      ),
      child: InkWell(
        onTap: () => onChanged(id, !selected),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
          child: LayoutBuilder(
            builder: (context, constraints) {
              if (constraints.maxWidth < 40) {
                return SizedBox(
                  height: 32,
                  width: constraints.maxWidth,
                  child: FittedBox(
                    alignment: Alignment.centerLeft,
                    fit: BoxFit.scaleDown,
                    child: checkbox(),
                  ),
                );
              }
              return Row(
                children: [
                  checkbox(),
                  SizedBox(width: 4),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
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
                        if (_emptyToNull(subtitle) != null)
                          Text(
                            subtitle!,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: _mutedText,
                              fontSize: NautermFontSizes.labelSmall,
                              letterSpacing: 0,
                            ),
                          ),
                      ],
                    ),
                  ),
                ],
              );
            },
          ),
        ),
      ),
    );
  }
}

String? _snippetHostSubtitle(HostEntry host) {
  if (host.type == NautermHostType.local) {
    return 'Local';
  }
  final address = _emptyToNull(host.host);
  if (address == null) {
    return 'SSH';
  }
  return '${host.username ?? 'ssh'}@$address:${host.port ?? 22}';
}
