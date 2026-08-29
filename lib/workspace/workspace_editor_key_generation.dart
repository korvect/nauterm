part of 'nauterm_workspace.dart';

class _GenerateKeyEditorContent extends StatefulWidget {
  const _GenerateKeyEditorContent({
    required this.onClose,
    required this.onSave,
  });

  final VoidCallback onClose;
  final _SaveGeneratedKey onSave;

  @override
  State<_GenerateKeyEditorContent> createState() =>
      _GenerateKeyEditorContentState();
}

class _GenerateKeyEditorContentState extends State<_GenerateKeyEditorContent> {
  late final TextEditingController _nameController;
  late final TextEditingController _passphraseController;
  String _keyType = 'ecdsa';
  int _ecdsaBits = 521;
  int _rsaBits = 4096;
  bool _passphraseVisible = false;
  bool _generating = false;
  String? _error;
  late bool _nameIsAutomatic;
  bool _settingAutomaticName = false;

  int? get _selectedKeySize {
    return switch (_keyType) {
      'ecdsa' => _ecdsaBits,
      'rsa' => _rsaBits,
      _ => null,
    };
  }

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController();
    _nameIsAutomatic = true;
    _nameController.addListener(_handleNameChanged);
    _passphraseController = TextEditingController();
    _refreshAutomaticName();
  }

  @override
  void dispose() {
    _nameController.removeListener(_handleNameChanged);
    _nameController.dispose();
    _passphraseController.dispose();
    super.dispose();
  }

  void _handleNameChanged() {
    if (_settingAutomaticName) return;
    _nameIsAutomatic = _nameController.text.trim().isEmpty;
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

  String _suggestedName() => '${_generatedKeyTypeLabel(_keyType)} key';

  Future<void> _generate() async {
    final name = _nameController.text.trim().isNotEmpty
        ? _nameController.text.trim()
        : _suggestedName();
    if (name.isEmpty) {
      setState(() => _error = 'Name is required.');
      return;
    }

    setState(() {
      _generating = true;
      _error = null;
    });

    try {
      final generated = await _generateSshKeyPair(
        type: _keyType,
        comment: name,
        passphrase: _passphraseController.text,
        bits: _selectedKeySize,
      );
      await widget.onSave(
        KeyEntry(
          name: name,
          privateKey: generated.privateKey,
          publicKey: generated.publicKey,
        ),
      );
      if (mounted) {
        setState(() => _generating = false);
      }
    } on Object catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _generating = false;
        _error = '$error';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return _EditorShell(
      title: tr('workspace.label.generateKey', fallback: 'Generate Key'),
      onClose: widget.onClose,
      onSave: _generate,
      saving: _generating,
      error: _error,
      saveLabel: tr(
        'workspace.keys.generateAndSave',
        fallback: 'Generate & save',
      ),
      savingLabel: tr('workspace.keys.generating', fallback: 'Generating...'),
      children: [
        _WorkspaceFormSection(
          title: tr('common.label.key', fallback: 'Key'),
          children: [
            _WorkspaceInput(
              controller: _nameController,
              label: tr('common.label.name', fallback: 'Name'),
              autofocus: true,
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _GenerateKeyFieldLabel(
              tr('workspace.keys.field.type', fallback: 'Key type'),
            ),
            SizedBox(height: 8),
            _GenerateKeySegmentedControl<String>(
              value: _keyType,
              // OpenSSH-current only defines the composite
              // mldsa44-ed25519 key, which the current russh backend cannot
              // decode or authenticate with yet.
              values: const ['ed25519', 'ecdsa', 'rsa'],
              labels: const {
                'ed25519': 'ED25519',
                'ecdsa': 'ECDSA',
                'rsa': 'RSA',
              },
              onChanged: (value) {
                setState(() => _keyType = value);
                _refreshAutomaticName();
              },
            ),
            if (_keyType == 'ecdsa') ...[
              SizedBox(height: _workspaceFormFieldGap),
              _GenerateKeyFieldLabel(
                tr(
                  'workspace.keys.field.curveSize',
                  fallback: 'Elliptic curve size (bits)',
                ),
              ),
              SizedBox(height: 8),
              _GenerateKeySegmentedControl<int>(
                value: _ecdsaBits,
                values: const [521, 384, 256],
                labels: const {521: '521', 384: '384', 256: '256'},
                onChanged: (value) => setState(() => _ecdsaBits = value),
              ),
            ] else if (_keyType == 'rsa') ...[
              SizedBox(height: _workspaceFormFieldGap),
              _GenerateKeyFieldLabel(
                tr('workspace.keys.field.size', fallback: 'Key size (bits)'),
              ),
              SizedBox(height: 8),
              _GenerateKeySegmentedControl<int>(
                value: _rsaBits,
                values: const [4096, 2048, 1024],
                labels: const {4096: '4096', 2048: '2048', 1024: '1024'},
                onChanged: (value) => setState(() => _rsaBits = value),
              ),
            ],
          ],
        ),
        SizedBox(height: 14),
        _WorkspaceFormSection(
          title: tr('workspace.keys.section.security', fallback: 'Security'),
          children: [
            _WorkspaceInput(
              controller: _passphraseController,
              label: tr('common.label.passphrase', fallback: 'Passphrase'),
              obscureText: !_passphraseVisible,
              trailing: _WorkspaceButton(
                icon: _passphraseVisible
                    ? Icons.visibility_rounded
                    : Icons.visibility_off_rounded,
                tooltip: _passphraseVisible
                    ? tr(
                        'workspace.keys.action.hidePassphrase',
                        fallback: 'Hide passphrase',
                      )
                    : tr(
                        'workspace.keys.action.showPassphrase',
                        fallback: 'Show passphrase',
                      ),
                onPressed: () =>
                    setState(() => _passphraseVisible = !_passphraseVisible),
                variant: _WorkspaceButtonVariant.text,
                size: _WorkspaceControlSize.tiny,
                height: 28,
                minWidth: 28,
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _GenerateKeyFieldLabel extends StatelessWidget {
  const _GenerateKeyFieldLabel(this.label);

  final String label;

  @override
  Widget build(BuildContext context) {
    return Text(
      label,
      style: TextStyle(
        color: _text,
        fontSize: NautermFontSizes.labelMedium,
        fontWeight: NautermFontWeights.medium,
        letterSpacing: 0,
      ),
    );
  }
}

class _GenerateKeySegmentedControl<T> extends StatelessWidget {
  const _GenerateKeySegmentedControl({
    required this.value,
    required this.values,
    required this.labels,
    required this.onChanged,
  }) : assert(values.length > 0);

  final T value;
  final List<T> values;
  final Map<T, String> labels;
  final ValueChanged<T> onChanged;

  @override
  Widget build(BuildContext context) {
    final selectedIndex = values.indexOf(value).clamp(0, values.length - 1);
    final selectedAlignment = values.length == 1
        ? Alignment.center
        : Alignment(-1 + (selectedIndex / (values.length - 1)) * 2, 0);
    return Container(
      height: 26,
      decoration: BoxDecoration(
        color: _sidebarHover,
        border: Border.all(color: _sidebarDivider),
        borderRadius: BorderRadius.circular(7),
      ),
      clipBehavior: Clip.antiAlias,
      child: Stack(
        fit: StackFit.expand,
        children: [
          Row(
            children: [
              for (var index = 0; index < values.length; index++)
                Expanded(
                  child: Align(
                    alignment: Alignment.centerRight,
                    child: index == values.length - 1
                        ? null
                        : Container(
                            width: 1,
                            height: 14,
                            color: _sidebarDivider,
                          ),
                  ),
                ),
            ],
          ),
          AnimatedAlign(
            alignment: selectedAlignment,
            duration: const Duration(milliseconds: 180),
            curve: Curves.easeOutCubic,
            child: FractionallySizedBox(
              widthFactor: 1 / values.length,
              heightFactor: 1,
              child: Padding(
                padding: const EdgeInsets.all(1),
                child: DecoratedBox(
                  key: const ValueKey('generate-key-segment-indicator'),
                  decoration: BoxDecoration(
                    color: _blue,
                    borderRadius: BorderRadius.circular(6),
                  ),
                ),
              ),
            ),
          ),
          Row(
            children: [
              for (final item in values)
                Expanded(
                  child: _GenerateKeySegment<T>(
                    value: item,
                    label: labels[item] ?? '$item',
                    selected: item == value,
                    onChanged: onChanged,
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

class _GenerateKeySegment<T> extends StatefulWidget {
  const _GenerateKeySegment({
    required this.value,
    required this.label,
    required this.selected,
    required this.onChanged,
  });

  final T value;
  final String label;
  final bool selected;
  final ValueChanged<T> onChanged;

  @override
  State<_GenerateKeySegment<T>> createState() => _GenerateKeySegmentState<T>();
}

class _GenerateKeySegmentState<T> extends State<_GenerateKeySegment<T>> {
  bool _hovered = false;
  bool _pressed = false;

  Color get _background {
    if (widget.selected) {
      if (_pressed) {
        return Colors.black.withValues(alpha: 0.08);
      }
      if (_hovered) {
        return Colors.white.withValues(alpha: 0.06);
      }
      return Colors.transparent;
    }
    if (_pressed) {
      return _workspaceMenuPressed;
    }
    if (_hovered) {
      return _workspaceMenuHover;
    }
    return Colors.transparent;
  }

  @override
  Widget build(BuildContext context) {
    return Semantics(
      button: true,
      selected: widget.selected,
      label: widget.label,
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() {
          _hovered = false;
          _pressed = false;
        }),
        child: Material(
          color: _background,
          animationDuration: const Duration(milliseconds: 90),
          child: InkWell(
            hoverColor: Colors.transparent,
            highlightColor: Colors.transparent,
            splashColor: widget.selected
                ? Colors.white.withValues(alpha: 0.14)
                : _blue.withValues(alpha: 0.14),
            onHighlightChanged: (pressed) {
              if (_pressed != pressed) {
                setState(() => _pressed = pressed);
              }
            },
            onTap: () => widget.onChanged(widget.value),
            child: Center(
              child: AnimatedDefaultTextStyle(
                duration: const Duration(milliseconds: 120),
                curve: Curves.easeOutCubic,
                style: TextStyle(
                  color: widget.selected ? Colors.white : _mutedText,
                  fontSize: NautermFontSizes.labelMedium,
                  fontWeight: NautermFontWeights.medium,
                  letterSpacing: 0,
                ),
                child: Text(
                  widget.label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  textAlign: TextAlign.center,
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

enum _CredentialCreationMethod { paste, import, generate }

extension on _CredentialCreationMethod {
  String get label => switch (this) {
    _CredentialCreationMethod.paste => tr(
      'common.label.paste',
      fallback: 'Paste',
    ),
    _CredentialCreationMethod.import => tr(
      'common.action.import',
      fallback: 'Import',
    ),
    _CredentialCreationMethod.generate => tr(
      'workspace.keys.generate',
      fallback: 'Generate',
    ),
  };
}

class _KeyEditorContent extends StatefulWidget {
  const _KeyEditorContent({
    required this.request,
    required this.onClose,
    required this.onSave,
    required this.onDuplicate,
    required this.onExportToHost,
    required this.onExportToFile,
    required this.onDelete,
  });

  final _KeyEditorRequest request;
  final VoidCallback onClose;
  final _SaveKey onSave;
  final ValueChanged<KeyEntry> onDuplicate;
  final ValueChanged<KeyEntry> onExportToHost;
  final ValueChanged<KeyEntry> onExportToFile;
  final ValueChanged<KeyEntry> onDelete;

  @override
  State<_KeyEditorContent> createState() => _KeyEditorContentState();
}

class _KeyEditorContentState extends State<_KeyEditorContent> {
  final GlobalKey _importDropKey = GlobalKey();
  late final TextEditingController _nameController;
  late final TextEditingController _privateController;
  late final TextEditingController _publicController;
  late final TextEditingController _certificateController;
  late final TextEditingController _passphraseController;
  StreamSubscription<NautermFileDropEvent>? _fileDropSubscription;
  bool _saving = false;
  bool _generating = false;
  bool _importing = false;
  bool _pickerLaunchPending = false;
  bool _dropHovered = false;
  bool _passphraseVisible = false;
  _CredentialCreationMethod _creationMethod = _CredentialCreationMethod.paste;
  String _generatedKeyType = 'ecdsa';
  int _ecdsaBits = 521;
  int _rsaBits = 4096;
  String? _error;
  String? _privateKeyError;
  String? _certificateError;
  late bool _nameIsAutomatic;
  bool _settingAutomaticName = false;

  @override
  void initState() {
    super.initState();
    final initial = widget.request.initial;
    _nameController = TextEditingController(
      text: initial?.name ?? widget.request.initialName ?? '',
    );
    _privateController = TextEditingController(text: initial?.privateKey ?? '');
    _publicController = TextEditingController(text: initial?.publicKey ?? '');
    _certificateController = TextEditingController(
      text: initial?.certificate ?? '',
    );
    _passphraseController = TextEditingController();
    _nameIsAutomatic = _nameController.text.trim().isEmpty;
    _nameController.addListener(_handleNameChanged);
    _privateController.addListener(_handlePrivateKeyChanged);
    _publicController.addListener(_refreshAutomaticName);
    _certificateController.addListener(_refreshAutomaticName);
    _certificateController.addListener(_handleCertificateChanged);
    _refreshAutomaticName();
    if (initial == null) {
      NautermFileDropChannel.instance.ensureInitialized();
      _fileDropSubscription = NautermFileDropChannel.instance.events.listen(
        _handleFileDropEvent,
      );
      unawaited(NautermFileDropChannel.instance.setEnabled(true));
    }
  }

  @override
  void dispose() {
    unawaited(_fileDropSubscription?.cancel());
    if (widget.request.initial == null) {
      unawaited(NautermFileDropChannel.instance.setEnabled(false));
    }
    _nameController.removeListener(_handleNameChanged);
    _privateController.removeListener(_handlePrivateKeyChanged);
    _publicController.removeListener(_refreshAutomaticName);
    _certificateController.removeListener(_refreshAutomaticName);
    _certificateController.removeListener(_handleCertificateChanged);
    _nameController.dispose();
    _privateController.dispose();
    _publicController.dispose();
    _certificateController.dispose();
    _passphraseController.dispose();
    super.dispose();
  }

  void _handleNameChanged() {
    if (_settingAutomaticName) return;
    _nameIsAutomatic = _nameController.text.trim().isEmpty;
  }

  void _handlePrivateKeyChanged() {
    _refreshAutomaticName();
    if (_privateKeyError != null && _privateController.text.trim().isNotEmpty) {
      setState(() => _privateKeyError = null);
    }
  }

  void _handleCertificateChanged() {
    if (_certificateError != null &&
        _certificateController.text.trim().isNotEmpty) {
      setState(() => _certificateError = null);
    }
  }

  bool get _usesCredentialCreationFlow =>
      widget.request.initial == null && widget.request.credentialCreation;

  bool get _isGenerating =>
      _usesCredentialCreationFlow &&
      _creationMethod == _CredentialCreationMethod.generate;

  bool get _canImportFiles =>
      widget.request.initial == null &&
      (!_usesCredentialCreationFlow ||
          _creationMethod == _CredentialCreationMethod.import);

  int? get _selectedGeneratedKeySize {
    return switch (_generatedKeyType) {
      'ecdsa' => _ecdsaBits,
      'rsa' => _rsaBits,
      _ => null,
    };
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
    if (_isGenerating) {
      return '${_generatedKeyTypeLabel(_generatedKeyType)} key';
    }
    return _sshCredentialDisplayType(
      privateKey: _privateController.text,
      publicKey: _publicController.text,
      certificate: _certificateController.text,
    );
  }

  bool _dropEventInsideTarget(NautermFileDropEvent event) {
    final x = event.x;
    final y = event.y;
    if (x == null || y == null) {
      return true;
    }
    final renderObject = _importDropKey.currentContext?.findRenderObject();
    if (renderObject is! RenderBox || !renderObject.hasSize) {
      return false;
    }
    final bounds = renderObject.localToGlobal(Offset.zero) & renderObject.size;
    return bounds.contains(Offset(x, y));
  }

  void _handleFileDropEvent(NautermFileDropEvent event) {
    if (!mounted || !_canImportFiles) {
      return;
    }
    switch (event.type) {
      case NautermFileDropEventType.dragging:
        final hovered = _dropEventInsideTarget(event);
        if (_dropHovered != hovered) {
          setState(() => _dropHovered = hovered);
        }
      case NautermFileDropEventType.exited:
        if (_dropHovered) {
          setState(() => _dropHovered = false);
        }
      case NautermFileDropEventType.dropped:
        final inside = _dropEventInsideTarget(event);
        if (_dropHovered) {
          setState(() => _dropHovered = false);
        }
        if (inside && event.paths.isNotEmpty) {
          unawaited(_importCredentialFile(io.File(event.paths.first)));
        }
    }
  }

  Future<void> _pickCredentialFile() async {
    if (_importing || _pickerLaunchPending) {
      return;
    }
    _pickerLaunchPending = true;
    await WidgetsBinding.instance.endOfFrame;
    await Future<void>.delayed(const Duration(milliseconds: 240));
    if (!mounted) {
      _pickerLaunchPending = false;
      return;
    }
    final XFile? file;
    try {
      file = await openFile(
        initialDirectory: _defaultSshKeyDirectoryPath(),
        confirmButtonText: 'Import',
      );
    } finally {
      _pickerLaunchPending = false;
    }
    if (!mounted) {
      return;
    }
    if (file == null || file.path.trim().isEmpty) {
      return;
    }
    await _importCredentialFile(io.File(file.path));
  }

  Future<void> _importCredentialFile(io.File file) async {
    if (_importing || _pickerLaunchPending) {
      return;
    }
    setState(() {
      _importing = true;
      _error = null;
    });
    try {
      final imported = await _readSshKeyFile(file);
      if (!mounted) {
        return;
      }
      setState(() {
        _privateController.text = imported.privateKey;
        _publicController.text = imported.publicKey ?? '';
        _certificateController.text = imported.certificate ?? '';
        _importing = false;
      });
    } on Object catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _importing = false;
        _error = 'Unable to import private key: $error';
      });
    }
  }

  Future<void> _save() async {
    if (_isGenerating) {
      await _generate();
      return;
    }
    final name = _nameController.text.trim().isNotEmpty
        ? _nameController.text.trim()
        : _suggestedName();
    if (name.isEmpty) {
      setState(() => _error = 'Name is required.');
      return;
    }
    if (_privateController.text.trim().isEmpty) {
      setState(() {
        _privateKeyError = tr(
          'workspace.keys.validation.privateKeyRequired',
          fallback: 'Private key is required.',
        );
        _error = null;
      });
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    await widget.onSave(
      KeyEntry(
        id: widget.request.initial?.id,
        name: name,
        privateKey: _emptyToNull(_privateController.text),
        publicKey: _emptyToNull(_publicController.text),
        certificate: _emptyToNull(_certificateController.text),
      ),
    );
  }

  Future<void> _generate() async {
    final name = _nameController.text.trim().isNotEmpty
        ? _nameController.text.trim()
        : _suggestedName();
    if (name.isEmpty) {
      setState(() => _error = 'Name is required.');
      return;
    }

    setState(() {
      _generating = true;
      _error = null;
    });
    try {
      final generated = await _generateSshKeyPair(
        type: _generatedKeyType,
        comment: name,
        passphrase: _passphraseController.text,
        bits: _selectedGeneratedKeySize,
      );
      await widget.onSave(
        KeyEntry(
          name: name,
          privateKey: generated.privateKey,
          publicKey: generated.publicKey,
          certificate: _emptyToNull(_certificateController.text),
        ),
      );
      if (mounted) {
        setState(() => _generating = false);
      }
    } on Object catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _generating = false;
        _error = '$error';
      });
    }
  }

  void _selectCreationMethod(_CredentialCreationMethod method) {
    if (method == _creationMethod) {
      return;
    }
    setState(() {
      _creationMethod = method;
      _dropHovered = false;
      _error = null;
    });
    _refreshAutomaticName();
  }

  Widget _buildCredentialNameInput() {
    final input = _WorkspaceInput(
      controller: _nameController,
      label: tr('common.label.name', fallback: 'Name'),
      autofocus: true,
    );
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < 96) {
          return input;
        }
        return Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              width: 40,
              height: 40,
              decoration: BoxDecoration(
                color: const Color(0xff075e92),
                borderRadius: BorderRadius.circular(8),
              ),
              child: Icon(Icons.key_rounded, color: Colors.white, size: 20),
            ),
            SizedBox(width: 10),
            Expanded(child: input),
          ],
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final initial = widget.request.initial;
    final credentialCreation = _usesCredentialCreationFlow;
    // Key and Certificate differ only while choosing a related creation
    // flow. Once a record exists, it is always a Key and exposes the
    // optional certificate material alongside its private/public key.
    final showCertificateField =
        initial != null || widget.request.certificateMode;
    final methods = [
      _CredentialCreationMethod.paste,
      _CredentialCreationMethod.import,
      _CredentialCreationMethod.generate,
    ];

    return _EditorShell(
      title: initial == null
          ? tr('workspace.label.newKey', fallback: 'New Key')
          : tr('workspace.label.editKey', fallback: 'Edit Key'),
      onClose: widget.onClose,
      onSave: _save,
      saving: _saving || _generating,
      error: _error,
      saveLabel: _isGenerating
          ? tr('workspace.keys.generateAndSave', fallback: 'Generate & save')
          : tr('common.action.save', fallback: 'Save'),
      savingLabel: _isGenerating
          ? tr('workspace.keys.generating', fallback: 'Generating...')
          : tr('common.label.saving', fallback: 'Saving...'),
      headerActions: initial?.id == null || _saving || _generating
          ? const []
          : [
              _EditorShellMenuAction.exportToHost(
                () => widget.onExportToHost(initial!),
              ),
              _EditorShellMenuAction.exportToFile(
                () => widget.onExportToFile(initial!),
              ),
              _EditorShellMenuAction.duplicate(
                () => widget.onDuplicate(initial!),
              ),
              _EditorShellMenuAction.delete(() => widget.onDelete(initial!)),
            ],
      children: [
        if (credentialCreation) ...[
          _GenerateKeySegmentedControl<_CredentialCreationMethod>(
            value: _creationMethod,
            values: methods,
            labels: {for (final method in methods) method: method.label},
            onChanged: _selectCreationMethod,
          ),
          SizedBox(height: 14),
        ],
        if (!credentialCreation ||
            _creationMethod == _CredentialCreationMethod.paste)
          _WorkspaceFormSection(
            title: tr('common.label.key', fallback: 'Key'),
            children: [
              _buildCredentialNameInput(),
              SizedBox(height: _workspaceFormFieldGap),
              _WorkspaceInput(
                controller: _privateController,
                label: tr('common.label.privateKey', fallback: 'Private key'),
                isRequired: true,
                errorText: _privateKeyError,
                minLines: 4,
                maxLines: 8,
                growable: true,
              ),
              if (!credentialCreation || !widget.request.certificateMode) ...[
                SizedBox(height: _workspaceFormFieldGap),
                _WorkspaceInput(
                  controller: _publicController,
                  label: tr('common.label.publicKey', fallback: 'Public key'),
                  minLines: 4,
                  maxLines: 8,
                  growable: true,
                ),
              ],
              if (showCertificateField) ...[
                SizedBox(height: _workspaceFormFieldGap),
                _WorkspaceInput(
                  controller: _certificateController,
                  label: tr(
                    'workspace.label.certificate',
                    fallback: 'Certificate',
                  ),
                  errorText: _certificateError,
                  minLines: 4,
                  maxLines: 8,
                  growable: true,
                ),
              ],
            ],
          ),
        if (credentialCreation &&
            _creationMethod == _CredentialCreationMethod.import) ...[
          _WorkspaceFormSection(
            title: tr('common.label.key', fallback: 'Key'),
            children: [
              _buildCredentialNameInput(),
              SizedBox(height: _workspaceFormFieldGap),
              if (widget.request.certificateMode) ...[
                _WorkspaceInput(
                  controller: _certificateController,
                  label: tr(
                    'workspace.label.certificate',
                    fallback: 'Certificate',
                  ),
                  errorText: _certificateError,
                  minLines: 4,
                  maxLines: 8,
                  growable: true,
                ),
                SizedBox(height: _workspaceFormFieldGap),
              ],
              if (_privateController.text.trim().isEmpty)
                _KeyFileImportPanel(
                  dropKey: _importDropKey,
                  hovered: _dropHovered,
                  importing: _importing,
                  errorText: _privateKeyError,
                  onPressed: _pickCredentialFile,
                )
              else
                _WorkspaceInput(
                  controller: _privateController,
                  label: tr('common.label.privateKey', fallback: 'Private key'),
                  isRequired: true,
                  errorText: _privateKeyError,
                  minLines: 4,
                  maxLines: 8,
                  growable: true,
                ),
              if (widget.request.certificateMode &&
                  _privateController.text.trim().isNotEmpty) ...[
                SizedBox(height: _workspaceFormFieldGap),
                _WorkspaceInput(
                  controller: _publicController,
                  label: tr('common.label.publicKey', fallback: 'Public key'),
                  minLines: 4,
                  maxLines: 8,
                  growable: true,
                ),
              ],
            ],
          ),
        ],
        if (credentialCreation &&
            _creationMethod == _CredentialCreationMethod.generate) ...[
          _WorkspaceFormSection(
            title: tr('common.label.key', fallback: 'Key'),
            children: [
              _buildCredentialNameInput(),
              SizedBox(height: _workspaceFormFieldGap),
              _GenerateKeyFieldLabel(
                tr('workspace.keys.field.type', fallback: 'Key type'),
              ),
              SizedBox(height: 8),
              _GenerateKeySegmentedControl<String>(
                value: _generatedKeyType,
                values: const ['ed25519', 'ecdsa', 'rsa'],
                labels: const {
                  'ed25519': 'ED25519',
                  'ecdsa': 'ECDSA',
                  'rsa': 'RSA',
                },
                onChanged: (value) {
                  setState(() => _generatedKeyType = value);
                  _refreshAutomaticName();
                },
              ),
              if (_generatedKeyType == 'ecdsa') ...[
                SizedBox(height: _workspaceFormFieldGap),
                _GenerateKeyFieldLabel(
                  tr(
                    'workspace.keys.field.curveSize',
                    fallback: 'Elliptic curve size (bits)',
                  ),
                ),
                SizedBox(height: 8),
                _GenerateKeySegmentedControl<int>(
                  value: _ecdsaBits,
                  values: const [521, 384, 256],
                  labels: const {521: '521', 384: '384', 256: '256'},
                  onChanged: (value) => setState(() => _ecdsaBits = value),
                ),
              ] else if (_generatedKeyType == 'rsa') ...[
                SizedBox(height: _workspaceFormFieldGap),
                _GenerateKeyFieldLabel(
                  tr('workspace.keys.field.size', fallback: 'Key size (bits)'),
                ),
                SizedBox(height: 8),
                _GenerateKeySegmentedControl<int>(
                  value: _rsaBits,
                  values: const [4096, 2048, 1024],
                  labels: const {4096: '4096', 2048: '2048', 1024: '1024'},
                  onChanged: (value) => setState(() => _rsaBits = value),
                ),
              ],
            ],
          ),
          SizedBox(height: 14),
          _WorkspaceFormSection(
            title: tr('workspace.keys.section.security', fallback: 'Security'),
            children: [
              _WorkspaceInput(
                controller: _passphraseController,
                label: tr('common.label.passphrase', fallback: 'Passphrase'),
                obscureText: !_passphraseVisible,
                trailing: _WorkspaceButton(
                  icon: _passphraseVisible
                      ? Icons.visibility_rounded
                      : Icons.visibility_off_rounded,
                  tooltip: _passphraseVisible
                      ? tr(
                          'workspace.keys.action.hidePassphrase',
                          fallback: 'Hide passphrase',
                        )
                      : tr(
                          'workspace.keys.action.showPassphrase',
                          fallback: 'Show passphrase',
                        ),
                  onPressed: () =>
                      setState(() => _passphraseVisible = !_passphraseVisible),
                  variant: _WorkspaceButtonVariant.text,
                  size: _WorkspaceControlSize.tiny,
                  height: 28,
                  minWidth: 28,
                ),
              ),
            ],
          ),
        ],
        if (initial == null && !credentialCreation) ...[
          SizedBox(height: 14),
          _WorkspaceFormSection(
            title: tr('common.action.import', fallback: 'Import'),
            children: [
              _KeyFileImportPanel(
                dropKey: _importDropKey,
                hovered: _dropHovered,
                importing: _importing,
                errorText: null,
                onPressed: _pickCredentialFile,
              ),
            ],
          ),
        ],
      ],
    );
  }
}

class _KeyFileImportPanel extends StatelessWidget {
  const _KeyFileImportPanel({
    required this.dropKey,
    required this.hovered,
    required this.importing,
    required this.errorText,
    required this.onPressed,
  });

  final GlobalKey dropKey;
  final bool hovered;
  final bool importing;
  final String? errorText;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final outline = hovered ? _blue : _sidebarDivider;
    final shape = RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(9),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        CustomPaint(
          key: dropKey,
          foregroundPainter: _DashedShapeBorderPainter(
            shape: shape,
            color: outline,
          ),
          child: Material(
            color: hovered
                ? _blue.withValues(alpha: _workspaceDark ? 0.14 : 0.07)
                : _surface.withValues(alpha: 0.48),
            shape: shape,
            clipBehavior: Clip.antiAlias,
            child: InkWell(
              onTap: importing ? null : onPressed,
              hoverColor: _sidebarHover,
              splashColor: _blue.withValues(alpha: 0.12),
              child: SizedBox(
                height: 112,
                child: LayoutBuilder(
                  builder: (context, constraints) {
                    final icon = SizedBox(
                      width: math.min(28, constraints.maxWidth),
                      height: 28,
                      child: FittedBox(
                        fit: BoxFit.scaleDown,
                        child: Icon(
                          Icons.upload_file_rounded,
                          size: 28,
                          color: hovered ? _blue : _mutedText,
                        ),
                      ),
                    );
                    return Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        icon,
                        SizedBox(height: 9),
                        Text(
                          hovered
                              ? tr(
                                  'workspace.keys.import.drop',
                                  fallback: 'Drop the private key to import',
                                )
                              : tr(
                                  'workspace.keys.import.drag',
                                  fallback: 'Drag and drop a private key file',
                                ),
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          textAlign: TextAlign.center,
                          style: TextStyle(
                            color: hovered ? _blue : _text,
                            fontSize: NautermFontSizes.labelLarge,
                            fontWeight: NautermFontWeights.medium,
                            letterSpacing: 0,
                          ),
                        ),
                      ],
                    );
                  },
                ),
              ),
            ),
          ),
        ),
        SizedBox(height: _workspaceFormFieldGap),
        _WorkspaceButton(
          label: importing
              ? tr('workspace.keys.import.importing', fallback: 'Importing...')
              : tr(
                  'workspace.keys.import.fromFile',
                  fallback: 'Import from key file',
                ),
          icon: Icons.file_open_rounded,
          onPressed: importing ? null : onPressed,
          variant: _WorkspaceButtonVariant.solid,
          type: _WorkspaceButtonType.primary,
          size: _WorkspaceControlSize.large,
          fullWidth: true,
        ),
        if (errorText != null) ...[
          const SizedBox(height: 6),
          Text(
            errorText!,
            style: const TextStyle(
              color: Color(0xffe5453d),
              fontSize: NautermFontSizes.labelSmall,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
        ],
      ],
    );
  }
}

class _ImportedSshKeyFile {
  const _ImportedSshKeyFile({
    required this.privateKey,
    this.publicKey,
    this.certificate,
  });

  final String privateKey;
  final String? publicKey;
  final String? certificate;
}

Future<_ImportedSshKeyFile> _readSshKeyFile(io.File file) async {
  const maximumKeyFileBytes = 4 * 1024 * 1024;
  final selectedText = await _readSshCredentialFile(
    file,
    maximumBytes: maximumKeyFileBytes,
  );
  final selectedValue = selectedText.trim();
  final selectedCertificate =
      _sshCertificateBaseWireType(selectedValue) != null;
  final io.File privateFile;
  final String? certificate;
  if (selectedCertificate) {
    if (!file.path.endsWith('-cert.pub')) {
      throw const FormatException(
        'The certificate filename must end with -cert.pub so its private key can be found.',
      );
    }
    privateFile = io.File(
      file.path.substring(0, file.path.length - '-cert.pub'.length),
    );
    if (!await privateFile.exists()) {
      throw const FormatException(
        'The matching private key was not found next to the certificate.',
      );
    }
    certificate = selectedValue;
  } else {
    privateFile = file;
    final certificateFile = io.File('${file.path}-cert.pub');
    if (await certificateFile.exists()) {
      final candidate = (await _readSshCredentialFile(
        certificateFile,
        maximumBytes: maximumKeyFileBytes,
      )).trim();
      certificate = _sshCertificateBaseWireType(candidate) == null
          ? null
          : candidate;
    } else {
      certificate = null;
    }
  }

  final privateKey = selectedCertificate
      ? await _readSshCredentialFile(
          privateFile,
          maximumBytes: maximumKeyFileBytes,
        )
      : selectedText;
  final normalized = privateKey.trim();
  if (_looksLikeSshPublicKey(normalized)) {
    throw const FormatException('Select a private key file, not a public key.');
  }
  if (!_looksLikeSshPrivateKey(normalized)) {
    throw const FormatException('The selected file is not a private key.');
  }

  String? publicKey;
  final publicFile = io.File('${privateFile.path}.pub');
  if (await publicFile.exists()) {
    final candidate = (await _readSshCredentialFile(
      publicFile,
      maximumBytes: maximumKeyFileBytes,
    )).trim();
    if (candidate.isNotEmpty &&
        _sshCertificateBaseWireType(candidate) == null) {
      publicKey = candidate;
    }
  }

  return _ImportedSshKeyFile(
    privateKey: privateKey,
    publicKey: publicKey,
    certificate: certificate,
  );
}

Future<String> _readSshCredentialFile(
  io.File file, {
  required int maximumBytes,
}) async {
  final size = await file.length();
  if (size <= 0) {
    throw const FormatException('The selected file is empty.');
  }
  if (size > maximumBytes) {
    throw const FormatException('The selected file is too large.');
  }
  final value = await file.readAsString();
  if (value.trim().isEmpty) {
    throw const FormatException('The selected file is empty.');
  }
  return value;
}

bool _looksLikeSshPublicKey(String value) {
  final firstLine = value.split(RegExp(r'\r?\n')).first.trim();
  return firstLine.startsWith('ssh-') ||
      firstLine.startsWith('ecdsa-') ||
      firstLine.startsWith('sk-ssh-') ||
      firstLine.startsWith('sk-ecdsa-');
}

bool _looksLikeSshPrivateKey(String value) {
  final firstLine = value.split(RegExp(r'\r?\n')).first.trim();
  return (firstLine.startsWith('-----BEGIN ') &&
          firstLine.endsWith(' PRIVATE KEY-----')) ||
      firstLine.startsWith('PuTTY-User-Key-File-');
}

String _generatedKeyTypeLabel(String type) {
  return switch (type) {
    'ed25519' => 'ED25519',
    'ecdsa' => 'ECDSA',
    'rsa' => 'RSA',
    _ => 'SSH',
  };
}

String _sshCredentialDisplayType({
  required String privateKey,
  required String publicKey,
  required String certificate,
}) {
  final material = '$certificate\n$publicKey\n$privateKey'.toLowerCase();
  final certificateType = _sshCertificateBaseWireType(certificate);
  if (certificateType != null) {
    return '${_keyTypeLabelFromWireName(certificateType)} SSH Certificate';
  }
  final type = switch (true) {
    _ when material.contains('sk-ssh-ed25519') => 'FIDO2 ED25519',
    _ when material.contains('sk-ecdsa') => 'FIDO2 ECDSA',
    _ when material.contains('ed25519') => 'ED25519',
    _ when material.contains('ecdsa') || material.contains('begin ec ') =>
      'ECDSA',
    _ when material.contains('ssh-rsa') || material.contains('begin rsa ') =>
      'RSA',
    _ when material.contains('dsa') => 'DSA',
    _ => 'SSH',
  };
  return '$type key';
}

String? _defaultSshKeyDirectoryPath() {
  final home = io.Platform.isWindows
      ? io.Platform.environment['USERPROFILE']
      : io.Platform.environment['HOME'];
  if (home == null || home.trim().isEmpty) {
    return null;
  }
  final sshDirectory = '$home${io.Platform.pathSeparator}.ssh';
  return io.Directory(sshDirectory).existsSync() ? sshDirectory : home;
}

class _GeneratedSshKeyPair {
  const _GeneratedSshKeyPair({
    required this.privateKey,
    required this.publicKey,
  });

  final String privateKey;
  final String publicKey;
}

Future<_GeneratedSshKeyPair> _generateSshKeyPair({
  required String type,
  required String comment,
  required String passphrase,
  int? bits,
}) async {
  final tempDir = await io.Directory.systemTemp.createTemp('nauterm-key-');
  final keyPath = '${tempDir.path}${io.Platform.pathSeparator}id_$type';

  try {
    final args = <String>[
      '-q',
      '-t',
      type,
      if (bits != null) ...['-b', '$bits'],
      '-N',
      passphrase,
      '-C',
      comment,
      '-f',
      keyPath,
    ];
    final result = await io.Process.run('ssh-keygen', args);
    if (result.exitCode != 0) {
      final stderr = result.stderr.toString().trim();
      final stdout = result.stdout.toString().trim();
      final message = [
        stderr,
        stdout,
      ].where((part) => part.isNotEmpty).join('\n').trim();
      throw Exception(
        message.isEmpty ? 'ssh-keygen failed for key type "$type".' : message,
      );
    }

    final privateKey = await io.File(keyPath).readAsString();
    final publicKey = await io.File('$keyPath.pub').readAsString();
    return _GeneratedSshKeyPair(
      privateKey: privateKey.trimRight(),
      publicKey: publicKey.trimRight(),
    );
  } on io.ProcessException catch (error) {
    throw Exception('ssh-keygen is not available: ${error.message}');
  } finally {
    try {
      await tempDir.delete(recursive: true);
    } on Object {
      // Best effort cleanup for temporary key files.
    }
  }
}
