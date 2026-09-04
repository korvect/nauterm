part of 'nauterm_workspace.dart';

enum _Fido2EditorStep { device, pin, options }

class _Fido2KeyEditorContent extends StatefulWidget {
  const _Fido2KeyEditorContent({required this.onClose, required this.onSave});

  final VoidCallback onClose;
  final _SaveGeneratedKey onSave;

  @override
  State<_Fido2KeyEditorContent> createState() => _Fido2KeyEditorContentState();
}

class _Fido2KeyEditorContentState extends State<_Fido2KeyEditorContent> {
  final _labelController = TextEditingController(text: 'FIDO2 key');
  final _pinController = TextEditingController();
  final _requirePinController = TextEditingController();
  final _passphraseController = TextEditingController();
  final _roundsController = TextEditingController(text: '100');
  var _step = _Fido2EditorStep.device;
  var _devices = const <Fido2DeviceInfo>[];
  Fido2DeviceInfo? _device;
  var _loading = true;
  var _working = false;
  var _keyType = 'ecdsa';
  var _requireUserPresence = true;
  var _requirePin = false;
  var _pinVisible = false;
  var _requirePinVisible = false;
  var _passphraseVisible = false;
  var _cipher = 'aes256-ctr';
  var _savePassphrase = false;
  var _passphraseWasEmpty = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _pinController.addListener(_handlePinChanged);
    _requirePinController.addListener(_handleRequirePinChanged);
    _passphraseController.addListener(_handlePassphraseChanged);
    _refreshDevices();
  }

  void _handlePinChanged() {
    if (mounted && _step == _Fido2EditorStep.pin) {
      setState(() {});
    }
  }

  void _handleRequirePinChanged() {
    if (mounted && _step == _Fido2EditorStep.options && _requirePin) {
      setState(() {});
    }
  }

  void _handlePassphraseChanged() {
    if (!mounted) return;
    final isEmpty = _passphraseController.text.isEmpty;
    setState(() {
      if (_passphraseWasEmpty && !isEmpty) {
        _savePassphrase = true;
      } else if (isEmpty) {
        _savePassphrase = false;
      }
      _passphraseWasEmpty = isEmpty;
    });
  }

  @override
  void dispose() {
    _labelController.dispose();
    _pinController.removeListener(_handlePinChanged);
    _pinController.dispose();
    _requirePinController.removeListener(_handleRequirePinChanged);
    _requirePinController.dispose();
    _passphraseController.removeListener(_handlePassphraseChanged);
    _passphraseController.dispose();
    _roundsController.dispose();
    super.dispose();
  }

  Future<void> _refreshDevices() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final devices = await listFido2Devices();
      if (!mounted) return;
      setState(() {
        _devices = devices;
        _device = devices.length == 1 ? devices.single : null;
        _loading = false;
      });
    } on Object catch (error) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _error = '$error';
      });
    }
  }

  Future<void> _primaryAction() async {
    switch (_step) {
      case _Fido2EditorStep.device:
        if (_device == null) return;
        setState(() {
          _step = _device!.hasPin
              ? _Fido2EditorStep.pin
              : _Fido2EditorStep.options;
          _error = null;
        });
      case _Fido2EditorStep.pin:
        await _verifyPin();
      case _Fido2EditorStep.options:
        await _generate();
    }
  }

  Future<void> _verifyPin() async {
    if (_pinController.text.isEmpty) {
      setState(() => _error = 'Enter the security key PIN.');
      return;
    }
    setState(() {
      _working = true;
      _error = null;
    });
    try {
      await verifyFido2Pin(deviceId: _device!.id, pin: _pinController.text);
      if (!mounted) return;
      _pinController.clear();
      setState(() {
        _working = false;
        _step = _Fido2EditorStep.options;
      });
    } on Object catch (error) {
      if (!mounted) return;
      setState(() {
        _working = false;
        _error = '$error'.replaceFirst('Bad state: ', '');
      });
    }
  }

  Future<void> _generate() async {
    final label = _labelController.text.trim();
    if (label.isEmpty) {
      setState(() => _error = 'Label is required.');
      return;
    }
    if (_requirePin && _requirePinController.text.isEmpty) {
      setState(() => _error = 'Enter the security key PIN.');
      return;
    }
    final passphrase = _passphraseController.text;
    final rounds = int.tryParse(_roundsController.text);
    if (passphrase.isNotEmpty &&
        (rounds == null || rounds < 1 || rounds > 1000000)) {
      setState(() => _error = 'Rounds must be between 1 and 1000000.');
      return;
    }
    setState(() {
      _working = true;
      _error = null;
    });
    try {
      final generated = await generateFido2Key(
        deviceId: _device!.id,
        label: label,
        keyType: _keyType,
        pin: _requirePin ? _requirePinController.text : '',
        requireUserPresence: _requireUserPresence,
        requireUserVerification: _requirePin,
        resident: false,
        passphrase: passphrase,
        cipher: _cipher,
        rounds: rounds ?? 100,
      );
      if (!mounted) return;
      _pinController.clear();
      _requirePinController.clear();
      await widget.onSave(
        KeyEntry(
          name: label,
          privateKey: generated.privateKey,
          publicKey: generated.publicKey,
          passphrase: _savePassphrase && passphrase.isNotEmpty
              ? passphrase
              : null,
        ),
      );
    } on Object catch (error) {
      if (!mounted) return;
      setState(() {
        _working = false;
        _error = '$error'.replaceFirst('Bad state: ', '');
      });
    }
  }

  void _back() {
    setState(() {
      _error = null;
      _step = switch (_step) {
        _Fido2EditorStep.device => _Fido2EditorStep.device,
        _Fido2EditorStep.pin => _Fido2EditorStep.device,
        _Fido2EditorStep.options =>
          _device?.hasPin == true
              ? _Fido2EditorStep.pin
              : _Fido2EditorStep.device,
      };
    });
  }

  @override
  Widget build(BuildContext context) {
    final canContinue = switch (_step) {
      _Fido2EditorStep.device => _device != null && !_loading,
      _Fido2EditorStep.pin => _pinController.text.isNotEmpty && !_working,
      _Fido2EditorStep.options =>
        !_working && (!_requirePin || _requirePinController.text.isNotEmpty),
    };
    final showFooter = _step != _Fido2EditorStep.device || _devices.isNotEmpty;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _fidoHeader(),
        Expanded(
          child: _WorkspaceControlSizeScope(
            size: _WorkspaceControlSize.large,
            child:
                _step == _Fido2EditorStep.device &&
                    !_loading &&
                    _devices.isEmpty
                ? _emptyDeviceStep()
                : ListView(
                    padding: const EdgeInsets.all(16),
                    children: [
                      switch (_step) {
                        _Fido2EditorStep.device => _deviceStep(),
                        _Fido2EditorStep.pin => _pinStep(),
                        _Fido2EditorStep.options => _optionsStep(),
                      },
                      if (_error != null) ...[
                        const SizedBox(height: 12),
                        Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Icon(
                              Icons.error_outline_rounded,
                              size: 17,
                              color: Color(0xffe5453d),
                            ),
                            const SizedBox(width: 7),
                            Expanded(
                              child: Text(
                                _error!,
                                style: const TextStyle(
                                  color: Color(0xffe5453d),
                                  fontSize: NautermFontSizes.labelMedium,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ],
                    ],
                  ),
          ),
        ),
        if (showFooter)
          Container(
            padding: const EdgeInsets.fromLTRB(16, 10, 16, 16),
            decoration: BoxDecoration(
              color: _surface,
              border: Border(top: BorderSide(color: _sidebarDivider)),
            ),
            child: Row(
              children: [
                if (_step != _Fido2EditorStep.device) ...[
                  Expanded(
                    key: const ValueKey('fido2-editor-back'),
                    child: _WorkspaceButton(
                      label: 'Back',
                      fullWidth: true,
                      height: 38,
                      onPressed: _working ? null : _back,
                    ),
                  ),
                  const SizedBox(width: 10),
                ],
                Expanded(
                  key: const ValueKey('fido2-editor-primary'),
                  child: _WorkspaceButton(
                    label: _working
                        ? (_step == _Fido2EditorStep.pin
                              ? 'Unlocking...'
                              : 'Waiting for security key...')
                        : switch (_step) {
                            _Fido2EditorStep.device => 'Continue',
                            _Fido2EditorStep.pin => 'Continue',
                            _Fido2EditorStep.options => 'Generate',
                          },
                    type: _WorkspaceButtonType.primary,
                    variant: _WorkspaceButtonVariant.solid,
                    fullWidth: true,
                    height: 38,
                    onPressed: canContinue ? _primaryAction : null,
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }

  Widget _fidoHeader() {
    return Container(
      height: _workspaceDrawerHeaderHeight,
      padding: const EdgeInsets.fromLTRB(16, 5, 8, 5),
      decoration: BoxDecoration(
        color: _card,
        border: Border(bottom: BorderSide(color: _sidebarDivider)),
      ),
      child: Row(
        children: [
          const Expanded(
            child: _WorkspaceDrawerHeaderTitle(title: 'Generate FIDO2 key'),
          ),
          _WorkspaceDrawerHeaderButton(
            icon: Icons.keyboard_tab_rounded,
            tooltip: 'Close',
            onPressed: _working ? null : widget.onClose,
          ),
        ],
      ),
    );
  }

  Widget _deviceStep() {
    return _WorkspaceFormSection(
      title: 'Security key',
      children: [
        Text(
          'Select the FIDO2 authenticator that will hold this key.',
          style: TextStyle(
            color: _mutedText,
            fontSize: NautermFontSizes.labelMedium,
          ),
        ),
        const SizedBox(height: 12),
        if (_loading)
          const Center(
            child: Padding(
              padding: EdgeInsets.all(24),
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
          )
        else
          for (final device in _devices)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: _Fido2DeviceTile(
                device: device,
                selected: _device?.id == device.id,
                onTap: () => setState(() {
                  _device = device;
                  _pinController.clear();
                }),
              ),
            ),
      ],
    );
  }

  Widget _emptyDeviceStep() {
    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(28),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: 60,
              height: 60,
              decoration: BoxDecoration(
                color: _sidebarHover,
                border: Border.all(color: _sidebarDivider),
                borderRadius: BorderRadius.circular(16),
              ),
              child: Icon(Icons.usb_rounded, size: 22, color: _text),
            ),
            const SizedBox(height: 28),
            Text(
              'Insert FIDO2 device',
              textAlign: TextAlign.center,
              style: TextStyle(
                color: _text,
                fontSize: 18,
                fontWeight: NautermFontWeights.bold,
              ),
            ),
            const SizedBox(height: 10),
            Text(
              'Connect your FIDO2 device to show here.',
              textAlign: TextAlign.center,
              style: TextStyle(
                color: _mutedText,
                fontSize: NautermFontSizes.labelMedium,
              ),
            ),
            if (_error != null) ...[
              const SizedBox(height: 12),
              Text(
                _error!,
                textAlign: TextAlign.center,
                style: const TextStyle(
                  color: Color(0xffe5453d),
                  fontSize: NautermFontSizes.labelSmall,
                ),
              ),
            ],
            const SizedBox(height: 20),
            _WorkspaceButton(
              label: 'Refresh',
              icon: Icons.refresh_rounded,
              onPressed: _refreshDevices,
            ),
          ],
        ),
      ),
    );
  }

  Widget _pinStep() {
    return _WorkspaceFormSection(
      title: 'Security key PIN',
      children: [
        Text(
          'Enter the PIN to unlock ${_device!.name}.',
          style: TextStyle(
            color: _mutedText,
            fontSize: NautermFontSizes.labelMedium,
          ),
        ),
        const SizedBox(height: 14),
        _WorkspaceInput(
          controller: _pinController,
          label: 'PIN',
          autofocus: true,
          obscureText: !_pinVisible,
          onSubmitted: (_) {
            if (!_working && _pinController.text.isNotEmpty) {
              unawaited(_verifyPin());
            }
          },
          trailing: _WorkspaceButton(
            icon: _pinVisible
                ? Icons.visibility_rounded
                : Icons.visibility_off_rounded,
            tooltip: _pinVisible ? 'Hide PIN' : 'Show PIN',
            variant: _WorkspaceButtonVariant.text,
            size: _WorkspaceControlSize.tiny,
            height: 28,
            minWidth: 28,
            onPressed: () => setState(() => _pinVisible = !_pinVisible),
          ),
        ),
      ],
    );
  }

  Widget _optionsStep() {
    return Column(
      children: [
        _WorkspaceFormSection(
          title: 'Key',
          children: [
            _WorkspaceInput(
              controller: _labelController,
              label: 'Label',
              isRequired: true,
              autofocus: true,
            ),
            const SizedBox(height: 14),
            const _GenerateKeyFieldLabel('Key type'),
            const SizedBox(height: 8),
            _GenerateKeySegmentedControl<String>(
              value: _keyType,
              values: const ['ecdsa', 'ed25519'],
              labels: const {'ecdsa': 'ECDSA 256', 'ed25519': 'ED25519'},
              onChanged: (value) => setState(() => _keyType = value),
            ),
          ],
        ),
        const SizedBox(height: 14),
        _WorkspaceFormSection(
          title: 'Security',
          children: [
            _WorkspaceToggleRow(
              label: 'Require user presence',
              description: 'Touch the security key for every SSH signature.',
              value: _requireUserPresence,
              onChanged: (value) =>
                  setState(() => _requireUserPresence = value),
            ),
            const SizedBox(height: 12),
            if (_device?.hasPin == true)
              _WorkspaceToggleRow(
                label: 'Require PIN',
                description:
                    'Require the authenticator PIN for every SSH signature.',
                value: _requirePin,
                onChanged: (value) {
                  setState(() {
                    _requirePin = value;
                    if (!value) {
                      _requirePinController.clear();
                    }
                  });
                },
              ),
            if (_requirePin) ...[
              const SizedBox(height: 14),
              _WorkspaceInput(
                controller: _requirePinController,
                label: 'PIN',
                autofocus: true,
                obscureText: !_requirePinVisible,
                onSubmitted: (_) {
                  if (!_working && _requirePinController.text.isNotEmpty) {
                    unawaited(_generate());
                  }
                },
                trailing: _WorkspaceButton(
                  icon: _requirePinVisible
                      ? Icons.visibility_rounded
                      : Icons.visibility_off_rounded,
                  tooltip: _requirePinVisible ? 'Hide PIN' : 'Show PIN',
                  variant: _WorkspaceButtonVariant.text,
                  size: _WorkspaceControlSize.tiny,
                  height: 28,
                  minWidth: 28,
                  onPressed: () =>
                      setState(() => _requirePinVisible = !_requirePinVisible),
                ),
              ),
            ],
          ],
        ),
        const SizedBox(height: 14),
        _WorkspaceFormSection(
          title: 'Private key protection',
          children: [
            _WorkspaceInput(
              controller: _passphraseController,
              label: 'Passphrase',
              obscureText: !_passphraseVisible,
              trailing: _WorkspaceButton(
                icon: _passphraseVisible
                    ? Icons.visibility_rounded
                    : Icons.visibility_off_rounded,
                tooltip: _passphraseVisible
                    ? 'Hide passphrase'
                    : 'Show passphrase',
                variant: _WorkspaceButtonVariant.text,
                size: _WorkspaceControlSize.tiny,
                height: 28,
                minWidth: 28,
                onPressed: () =>
                    setState(() => _passphraseVisible = !_passphraseVisible),
              ),
            ),
            if (_passphraseController.text.isNotEmpty) ...[
              const SizedBox(height: 14),
              const _GenerateKeyFieldLabel('Cipher'),
              const SizedBox(height: 8),
              _GenerateKeySegmentedControl<String>(
                value: _cipher,
                values: const ['aes256-ctr', 'aes128-ctr'],
                labels: const {
                  'aes256-ctr': 'AES-256',
                  'aes128-ctr': 'AES-128',
                },
                onChanged: (value) => setState(() => _cipher = value),
              ),
              const SizedBox(height: 14),
              _WorkspaceInput(
                controller: _roundsController,
                label: 'Rounds',
                keyboardType: TextInputType.number,
                inputFormatters: [FilteringTextInputFormatter.digitsOnly],
              ),
              const SizedBox(height: 8),
              Text(
                'Higher values protect the private key more strongly but take longer to unlock.',
                style: TextStyle(
                  color: _mutedText,
                  fontSize: NautermFontSizes.labelSmall,
                ),
              ),
              const SizedBox(height: 12),
              _WorkspaceToggleRow(
                label: 'Save passphrase',
                value: _savePassphrase,
                onChanged: (value) => setState(() => _savePassphrase = value),
              ),
            ],
          ],
        ),
        if (_working) ...[
          const SizedBox(height: 14),
          _WorkspaceFormSection(
            title: 'Authenticator',
            children: [
              Row(
                children: [
                  const SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      'Touch your security key when it starts blinking.',
                      style: TextStyle(color: _text),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ],
      ],
    );
  }
}

class _Fido2DeviceTile extends StatelessWidget {
  const _Fido2DeviceTile({
    required this.device,
    required this.selected,
    required this.onTap,
  });

  final Fido2DeviceInfo device;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: selected ? _blue.withValues(alpha: 0.10) : _surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(9),
        side: BorderSide(color: selected ? _blue : _sidebarDivider),
      ),
      child: InkWell(
        borderRadius: BorderRadius.circular(9),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Row(
            children: [
              Container(
                width: 38,
                height: 38,
                decoration: BoxDecoration(
                  color: _blue,
                  borderRadius: BorderRadius.circular(9),
                ),
                child: const Icon(
                  Icons.usb_rounded,
                  color: Colors.white,
                  size: 21,
                ),
              ),
              const SizedBox(width: 11),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      device.name,
                      style: TextStyle(
                        color: _text,
                        fontWeight: NautermFontWeights.semibold,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      '${device.vendorId.toRadixString(16).padLeft(4, '0')}:${device.productId.toRadixString(16).padLeft(4, '0')}',
                      style: TextStyle(
                        color: _mutedText,
                        fontSize: NautermFontSizes.labelSmall,
                      ),
                    ),
                  ],
                ),
              ),
              if (selected)
                Icon(Icons.check_circle_rounded, color: _blue, size: 19),
            ],
          ),
        ),
      ),
    );
  }
}
