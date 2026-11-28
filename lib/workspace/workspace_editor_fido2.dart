part of 'nauterm_workspace.dart';

enum _Fido2EditorStep { device, pin, options }

typedef _GenerateFido2Key = Future<Fido2GeneratedKey> Function({
  required String deviceId,
  required String label,
  required String keyType,
  required String pin,
  required bool requireUserPresence,
  required bool requireUserVerification,
  required String passphrase,
  required String cipher,
  required int rounds,
});

@visibleForTesting
Widget buildFido2KeyEditorForTesting({
  required Future<List<Fido2DeviceInfo>> Function() listDevices,
  required Future<Fido2GeneratedKey> Function() generateKey,
  required Future<void> Function(KeyEntry) onSave,
  required VoidCallback onClose,
}) => _Fido2KeyEditorContent(
  generation: _Fido2KeyDraft(),
  listDevices: listDevices,
  generateKey: ({
    required deviceId,
    required label,
    required keyType,
    required pin,
    required requireUserPresence,
    required requireUserVerification,
    required passphrase,
    required cipher,
    required rounds,
  }) => generateKey(),
  onSave: onSave,
  onClose: onClose,
);

class _Fido2KeyDraft extends ChangeNotifier {
  KeyEntry? pending;
  String? error;
  bool busy = false;

  void start() {
    busy = true;
    error = null;
    notifyListeners();
  }

  void finish({required KeyEntry? key, String? failure}) {
    pending = key;
    error = failure;
    busy = false;
    notifyListeners();
  }
}

class _Fido2KeyEditorContent extends StatefulWidget {
  const _Fido2KeyEditorContent({
    required this.generation,
    required this.onClose,
    required this.onSave,
    this.listDevices = listFido2Devices,
    this.generateKey = generateFido2Key,
  });

  final _Fido2KeyDraft generation;
  final VoidCallback onClose;
  final _SaveGeneratedKey onSave;
  final Future<List<Fido2DeviceInfo>> Function() listDevices;
  final _GenerateFido2Key generateKey;

  @override
  State<_Fido2KeyEditorContent> createState() => _Fido2KeyEditorContentState();
}

class _Fido2KeyEditorContentState extends State<_Fido2KeyEditorContent>
    with _WorkspaceEditorDraftState<_Fido2KeyEditorContent> {
  @override
  Map<String, _WorkspaceDraftField> get draftFields => {
    'label': _WorkspaceDraftField.text(_labelController),
    'passphrase': _WorkspaceDraftField.text(_passphraseController),
    'rounds': _WorkspaceDraftField.text(_roundsController),
    'keyType': _draftValue(() => _keyType, (value) => _keyType = value),
    'requireUserPresence': _draftValue(
      () => _requireUserPresence,
      (value) => _requireUserPresence = value,
    ),
    'requirePin': _draftValue(
      () => _requirePin,
      (value) => _requirePin = value,
    ),
    'cipher': _draftValue(() => _cipher, (value) => _cipher = value),
    'savePassphrase': _draftValue(
      () => _savePassphrase,
      (value) => _savePassphrase = value,
    ),
    'passphraseWasEmpty': _draftViewValue(
      () => _passphraseWasEmpty,
      (value) => _passphraseWasEmpty = value,
    ),
  };

  final _labelController = TextEditingController(text: 'FIDO2 key');
  final _pinController = TextEditingController();
  final _requirePinController = TextEditingController();
  final _passphraseController = TextEditingController();
  final _roundsController = TextEditingController(text: '100');
  var _step = _Fido2EditorStep.device;
  var _devices = const <Fido2DeviceInfo>[];
  Fido2DeviceInfo? _device;
  var _loading = false;
  var _verifyingPin = false;
  bool get _working => _verifyingPin || widget.generation.busy;
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
    widget.generation.addListener(_generationChanged);
    if (widget.generation.pending == null && !widget.generation.busy) {
      _refreshDevices();
    }
  }

  @override
  void didUpdateWidget(covariant _Fido2KeyEditorContent oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (identical(oldWidget.generation, widget.generation)) return;
    oldWidget.generation.removeListener(_generationChanged);
    widget.generation.addListener(_generationChanged);
  }

  void _generationChanged() => setState(() {});

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
    if (restoringEditorDraft) return;
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
    widget.generation.removeListener(_generationChanged);
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
      widget.generation.error = null;
    });
    try {
      final devices = await widget.listDevices();
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
    if (_working) return;
    if (widget.generation.pending != null) {
      await _savePendingKey();
      return;
    }
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
      setState(() => _error = tr('workspace.keys.fido2.pinRequired'));
      return;
    }
    setState(() {
      _verifyingPin = true;
      _error = null;
    });
    try {
      await verifyFido2Pin(deviceId: _device!.id, pin: _pinController.text);
      if (!mounted) return;
      _pinController.clear();
      setState(() {
        _verifyingPin = false;
        _step = _Fido2EditorStep.options;
      });
    } on Object catch (error) {
      if (!mounted) return;
      setState(() {
        _verifyingPin = false;
        _error = '$error'.replaceFirst('Bad state: ', '');
      });
    }
  }

  Future<void> _generate() async {
    if (_working || widget.generation.pending != null) return;
    final label = _labelController.text.trim();
    if (label.isEmpty) {
      setState(
        () => _error = tr('workspace.keys.fido2.validation.labelRequired'),
      );
      return;
    }
    if (_requirePin && _requirePinController.text.isEmpty) {
      setState(() => _error = tr('workspace.keys.fido2.pinRequired'));
      return;
    }
    final passphrase = _passphraseController.text;
    final rounds = int.tryParse(_roundsController.text);
    if (passphrase.isNotEmpty &&
        (rounds == null || rounds < 1 || rounds > 1000000)) {
      setState(() => _error = tr('workspace.keys.fido2.roundsRange'));
      return;
    }
    final savePassphrase = _savePassphrase;
    final generation = widget.generation;
    _error = null;
    generation.start();
    try {
      final generated = await widget.generateKey(
        deviceId: _device!.id,
        label: label,
        keyType: _keyType,
        pin: _requirePin ? _requirePinController.text : '',
        requireUserPresence: _requireUserPresence,
        requireUserVerification: _requirePin,
        passphrase: passphrase,
        cipher: _cipher,
        rounds: rounds ?? 100,
      );
      generation.finish(
        key: KeyEntry(
          name: label,
          privateKey: generated.privateKey,
          publicKey: generated.publicKey,
          passphrase: savePassphrase && passphrase.isNotEmpty
              ? passphrase
              : null,
        ),
      );
    } on Object catch (error) {
      generation.finish(key: null, failure: error.toString());
      return;
    }
    if (!mounted) return;
    _pinController.clear();
    _requirePinController.clear();
    _passphraseController.clear();
    await _savePendingKey();
  }

  Future<void> _savePendingKey() async {
    final generation = widget.generation;
    final key = generation.pending;
    if (key == null || generation.busy) return;
    generation.start();
    try {
      await widget.onSave(key);
      generation.finish(key: null);
    } on Object catch (error) {
      generation.finish(key: key, failure: error.toString());
    }
  }

  void _back() {
    setState(() {
      _error = null;
      widget.generation.error = null;
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
    final generation = widget.generation;
    final pending = generation.pending != null;
    final canContinue =
        !_working &&
        (pending ||
            switch (_step) {
              _Fido2EditorStep.device => _device != null && !_loading,
              _Fido2EditorStep.pin => _pinController.text.isNotEmpty,
              _Fido2EditorStep.options =>
                !_requirePin || _requirePinController.text.isNotEmpty,
            });
    final showFooter =
        pending ||
        generation.busy ||
        _step != _Fido2EditorStep.device ||
        _devices.isNotEmpty;
    final error = generation.error == null
        ? _error
        : !pending
        ? generation.error
        : context.tr(
            'workspace.keys.fido2.pending.saveError',
            args: {'error': generation.error},
          );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _fidoHeader(),
        Expanded(
          child: _WorkspaceControlSizeScope(
            size: _WorkspaceControlSize.large,
            child:
                _step == _Fido2EditorStep.device &&
                    !pending &&
                    !generation.busy &&
                    !_loading &&
                    _devices.isEmpty
                ? _emptyDeviceStep(error)
                : ListView(
                    padding: const EdgeInsets.all(16),
                    children: [
                      if (pending)
                        _WorkspaceFormSection(
                          title: context.tr(
                            'workspace.keys.fido2.pending.title',
                          ),
                          children: [
                            if (!generation.busy)
                              Text(
                                context.tr(
                                  'workspace.keys.fido2.pending.description',
                                ),
                              ),
                          ],
                        )
                      else if (generation.busy)
                        Text(context.tr('workspace.keys.fido2.confirmDevice'))
                      else
                        switch (_step) {
                          _Fido2EditorStep.device => _deviceStep(),
                          _Fido2EditorStep.pin => _pinStep(),
                          _Fido2EditorStep.options => _optionsStep(),
                        },
                      if (error != null) ...[
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
                                error,
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
                if (_step != _Fido2EditorStep.device && !pending) ...[
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
                    label: context.tr(switch ((
                      pending,
                      generation.busy,
                      _verifyingPin,
                      _step,
                    )) {
                      (true, true, _, _) => 'common.label.saving',
                      (true, false, _, _) => 'common.action.retrySave',
                      (_, true, _, _) =>
                        'workspace.keys.fido2.waitingForDevice',
                      (_, _, true, _) => 'workspace.keys.fido2.unlocking',
                      (_, _, _, _Fido2EditorStep.options) =>
                        'workspace.keys.generate',
                      _ => 'common.action.continue',
                    }),
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
          Expanded(
            child: _WorkspaceDrawerHeaderTitle(
              title: tr(
                'workspace.keys.fido2.generate',
                fallback: 'Generate FIDO2 key',
              ),
            ),
          ),
          _WorkspaceDrawerHeaderButton(
            icon: Icons.keyboard_tab_rounded,
            tooltip: tr('common.action.close'),
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
          context.tr('workspace.keys.fido2.selectAuthenticator'),
          style: TextStyle(
            color: _mutedText,
            fontSize: NautermFontSizes.labelMedium,
          ),
        ),
        const SizedBox(height: 12),
        if (_loading)
          Center(
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: CircularProgressIndicator(
                strokeWidth: 2,
                semanticsLabel: context.tr(
                  'workspace.keys.fido2.scanningDevices',
                ),
              ),
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

  Widget _emptyDeviceStep(String? error) {
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
              context.tr('workspace.keys.fido2.insertDevice'),
              textAlign: TextAlign.center,
              style: TextStyle(
                color: _text,
                fontSize: 18,
                fontWeight: NautermFontWeights.bold,
              ),
            ),
            const SizedBox(height: 10),
            Text(
              context.tr('workspace.keys.fido2.empty.description'),
              textAlign: TextAlign.center,
              style: TextStyle(
                color: _mutedText,
                fontSize: NautermFontSizes.labelMedium,
              ),
            ),
            if (error != null) ...[
              const SizedBox(height: 12),
              Text(
                error,
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
      title: 'connection.auth.securityKeyPin.title',
      children: [
        Text(
          context.tr(
            'workspace.keys.fido2.unlockDevice.description',
            fallback: 'Enter the PIN to unlock {device}.',
            args: {'device': _device!.name},
          ),
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
          title: 'common.label.key',
          children: [
            _WorkspaceInput(
              controller: _labelController,
              label: 'workspace.keys.fido2.field.label',
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
          title: 'workspace.keys.section.security',
          children: [
            _WorkspaceToggleRow(
              label: tr(
                'workspace.keys.fido2.requireUserPresence',
                fallback: 'Require user presence',
              ),
              description:
                  'workspace.keys.fido2.requireUserPresenceDescription',
              value: _requireUserPresence,
              onChanged: (value) =>
                  setState(() => _requireUserPresence = value),
            ),
            const SizedBox(height: 12),
            if (_device?.hasPin == true)
              _WorkspaceToggleRow(
                label: tr(
                  'workspace.keys.fido2.requirePin',
                  fallback: 'Require PIN',
                ),
                description: 'workspace.keys.fido2.requirePinDescription',
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
          title: 'workspace.keys.fido2.privateKeyProtection',
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
              _GenerateKeyFieldLabel(tr('workspace.keys.fido2.cipher.label')),
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
                context.tr(
                  'workspace.keys.fido2.privateKeyProtectionDescription',
                ),
                style: TextStyle(
                  color: _mutedText,
                  fontSize: NautermFontSizes.labelSmall,
                ),
              ),
              const SizedBox(height: 12),
              _WorkspaceToggleRow(
                label: tr(
                  'workspace.keys.label.savePassphrase',
                  fallback: 'Save passphrase',
                ),
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
