part of 'nauterm_workspace.dart';

class _SerialConnectionDraft {
  const _SerialConnectionDraft({required this.config});

  final SerialConnectionConfig config;

  String get serialPort => config.serialPort;
  int get baudRate => config.baudRate;
}

enum _WorkspaceNotificationType { info, error }

class _WorkspaceNotification {
  const _WorkspaceNotification({required this.message, required this.type});

  final String message;
  final _WorkspaceNotificationType type;
}

class _WorkspaceNotificationToast extends StatelessWidget {
  const _WorkspaceNotificationToast({
    required this.notification,
    required this.onDismissed,
  });

  final _WorkspaceNotification notification;
  final VoidCallback onDismissed;

  @override
  Widget build(BuildContext context) {
    final error = notification.type == _WorkspaceNotificationType.error;
    final color = error ? const Color(0xffef4444) : const Color(0xff075e92);
    final textStyle = TextStyle(
      color: _text,
      fontSize: NautermFontSizes.labelLarge,
      fontWeight: NautermFontWeights.medium,
      height: 1.35,
      letterSpacing: 0,
    );
    final textPainter = TextPainter(
      text: TextSpan(text: notification.message, style: textStyle),
      maxLines: 1,
      textDirection: Directionality.of(context),
      textScaler: MediaQuery.textScalerOf(context),
    )..layout(maxWidth: 338);
    final singleLine =
        !notification.message.contains('\n') && !textPainter.didExceedMaxLines;

    return Positioned(
      right: 18,
      top: _topBarHeight + 14,
      child: Material(
        color: Colors.transparent,
        child: Container(
          constraints: const BoxConstraints(maxWidth: 420),
          decoration: BoxDecoration(
            color: _card,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: color.withValues(alpha: 0.36)),
            boxShadow: const [
              BoxShadow(
                color: Color(0x1f000000),
                blurRadius: 18,
                offset: Offset(0, 8),
              ),
            ],
          ),
          padding: singleLine
              ? const EdgeInsets.fromLTRB(12, 8, 6, 8)
              : const EdgeInsets.fromLTRB(14, 12, 8, 12),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: singleLine
                ? CrossAxisAlignment.center
                : CrossAxisAlignment.start,
            children: [
              Padding(
                padding: EdgeInsets.only(top: singleLine ? 0 : 1),
                child: Icon(
                  error
                      ? Icons.error_outline_rounded
                      : Icons.info_outline_rounded,
                  color: color,
                  size: 18,
                ),
              ),
              SizedBox(width: singleLine ? 8 : 10),
              Flexible(child: Text(notification.message, style: textStyle)),
              SizedBox(width: singleLine ? 6 : 8),
              _WorkspaceButton(
                icon: Icons.close_rounded,
                tooltip: tr('common.action.dismiss', fallback: 'Dismiss'),
                size: _WorkspaceControlSize.tiny,
                variant: _WorkspaceButtonVariant.text,
                height: 24,
                minWidth: 24,
                onPressed: onDismissed,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _WorkspaceUpdateToast extends StatelessWidget {
  const _WorkspaceUpdateToast({required this.notice});

  final StartupUpdateNotice notice;

  @override
  Widget build(BuildContext context) {
    final palette = context.nautermPalette;
    final busy =
        notice.phase == StartupUpdatePhase.downloading ||
        notice.phase == StartupUpdatePhase.installing;
    final restart = notice.phase == StartupUpdatePhase.restartRequired;
    final error = notice.phase == StartupUpdatePhase.error;
    final message = switch (notice.phase) {
      StartupUpdatePhase.available => tr(
        'update.startup.available.description',
        fallback: 'Nauterm {version} is available.',
        args: {'version': notice.version},
      ),
      StartupUpdatePhase.downloading => tr(
        'update.startup.downloading',
        fallback: 'Downloading Nauterm {version}...',
        args: {'version': notice.version},
      ),
      StartupUpdatePhase.installing => tr(
        'update.startup.installing',
        fallback: 'Installing Nauterm {version}...',
        args: {'version': notice.version},
      ),
      StartupUpdatePhase.restartRequired => tr(
        'update.startup.restartRequired',
        fallback: 'Nauterm {version} is ready. Restart to finish updating.',
        args: {'version': notice.version},
      ),
      StartupUpdatePhase.installerLaunched => tr(
        'update.startup.installerLaunched',
        fallback: 'The Nauterm {version} installer is open.',
        args: {'version': notice.version},
      ),
      StartupUpdatePhase.error => tr(
        'update.startup.error',
        fallback: 'Unable to update Nauterm {version}.',
        args: {'version': notice.version},
      ),
    };

    return Positioned(
      right: 18,
      bottom: 66,
      child: Material(
        color: Colors.transparent,
        child: Container(
          width: 390,
          constraints: const BoxConstraints(maxWidth: 390),
          padding: const EdgeInsets.all(14),
          decoration: BoxDecoration(
            color: _card,
            borderRadius: BorderRadius.circular(9),
            border: Border.all(
              color: (error ? const Color(0xffef4444) : palette.primary)
                  .withValues(alpha: 0.34),
            ),
            boxShadow: const [
              BoxShadow(
                color: Color(0x26000000),
                blurRadius: 22,
                offset: Offset(0, 10),
              ),
            ],
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Icon(
                    error ? Icons.error_outline_rounded : Icons.system_update,
                    color: error ? const Color(0xffef4444) : palette.primary,
                    size: 19,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          tr(
                            'update.startup.title',
                            fallback: 'Nauterm update available',
                          ),
                          style: TextStyle(
                            color: _text,
                            fontSize: NautermFontSizes.labelLarge,
                            fontWeight: NautermFontWeights.semibold,
                          ),
                        ),
                        const SizedBox(height: 4),
                        Text(
                          message,
                          style: TextStyle(
                            color: _mutedText,
                            fontSize: NautermFontSizes.labelMedium,
                            height: 1.4,
                          ),
                        ),
                      ],
                    ),
                  ),
                  if (!busy)
                    _WorkspaceButton(
                      icon: LucideIcons.x,
                      tooltip: tr(
                        'settings.update.action.later',
                        fallback: 'Later',
                      ),
                      size: _WorkspaceControlSize.tiny,
                      variant: _WorkspaceButtonVariant.text,
                      height: 24,
                      minWidth: 24,
                      onPressed: notice.onDismiss,
                    ),
                ],
              ),
              if (notice.progress != null) ...[
                const SizedBox(height: 12),
                LinearProgressIndicator(
                  value: notice.progress,
                  minHeight: 3,
                  color: palette.primary,
                  backgroundColor: palette.softOutline,
                  borderRadius: BorderRadius.circular(3),
                ),
              ],
              if (!busy) ...[
                const SizedBox(height: 12),
                Wrap(
                  alignment: WrapAlignment.end,
                  spacing: 8,
                  children: [
                    if (notice.phase == StartupUpdatePhase.available)
                      _WorkspaceTextButton(
                        label: tr(
                          'update.startup.action.skip',
                          fallback: 'Skip this version',
                        ),
                        onPressed: notice.onSkip,
                      ),
                    if (notice.phase != StartupUpdatePhase.installerLaunched)
                      _WorkspaceTextButton(
                        label: tr(
                          restart
                              ? 'settings.update.action.restart'
                              : 'update.startup.action.update',
                          fallback: restart ? 'Restart Nauterm' : 'Update',
                        ),
                        primary: true,
                        onPressed: notice.onUpdate,
                      ),
                  ],
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _WorkspaceTextButton extends StatelessWidget {
  const _WorkspaceTextButton({
    required this.label,
    required this.onPressed,
    this.primary = false,
  });

  final String label;
  final VoidCallback onPressed;
  final bool primary;

  @override
  Widget build(BuildContext context) {
    final palette = context.nautermPalette;
    return TextButton(
      onPressed: onPressed,
      style: TextButton.styleFrom(
        foregroundColor: primary ? Colors.white : _mutedText,
        backgroundColor: primary ? palette.primary : Colors.transparent,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        minimumSize: const Size(0, 32),
        tapTargetSize: MaterialTapTargetSize.shrinkWrap,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(7),
          side: primary
              ? BorderSide.none
              : BorderSide(color: palette.softOutline),
        ),
      ),
      child: Text(
        label,
        style: const TextStyle(
          fontSize: NautermFontSizes.labelMedium,
          fontWeight: NautermFontWeights.medium,
        ),
      ),
    );
  }
}

class _SerialConnectionDialog extends StatefulWidget {
  const _SerialConnectionDialog();

  @override
  State<_SerialConnectionDialog> createState() =>
      _SerialConnectionDialogState();
}

class _SerialConnectionDialogState extends State<_SerialConnectionDialog> {
  late final TextEditingController _portController;
  late final TextEditingController _baudController;
  List<SerialPortInfo> _serialPorts = const [];
  bool _loadingPorts = false;
  String? _portListError;
  String? _error;
  int _dataBits = 8;
  SerialParity _parity = SerialParity.none;
  int _stopBits = 1;
  SerialFlowControl _flowControl = SerialFlowControl.none;

  @override
  void initState() {
    super.initState();
    _portController = TextEditingController(text: _defaultSerialPort());
    _baudController = TextEditingController(text: '115200');
    _portController.addListener(_handleInputChanged);
    _baudController.addListener(_handleInputChanged);
    _refreshSerialPorts();
  }

  @override
  void dispose() {
    _portController.removeListener(_handleInputChanged);
    _baudController.removeListener(_handleInputChanged);
    _portController.dispose();
    _baudController.dispose();
    super.dispose();
  }

  void _handleInputChanged() {
    if (mounted) {
      setState(() {});
    }
  }

  Future<void> _refreshSerialPorts() async {
    if (_loadingPorts) {
      return;
    }

    setState(() {
      _loadingPorts = true;
      _portListError = null;
    });

    try {
      final ports = await Future<List<SerialPortInfo>>(
        NativeTerminalDriver.listSerialPorts,
      );
      if (!mounted) {
        return;
      }

      final currentPort = _portController.text.trim();
      final shouldUseDetectedPort =
          ports.isNotEmpty &&
          (currentPort.isEmpty || currentPort == _defaultSerialPort());
      setState(() {
        _serialPorts = ports;
        _loadingPorts = false;
      });
      if (shouldUseDetectedPort) {
        _portController.text = ports.first.path;
      }
    } on Object catch (error) {
      if (!mounted) {
        return;
      }
      setState(() {
        _serialPorts = const [];
        _loadingPorts = false;
        _portListError = 'Unable to enumerate serial devices: $error';
      });
    }
  }

  void _submit() {
    final serialPort = _portController.text.trim();
    final baudRate = int.tryParse(_baudController.text.trim());

    if (serialPort.isEmpty) {
      setState(() => _error = 'Serial port is required.');
      return;
    }
    if (baudRate == null || baudRate <= 0) {
      setState(() => _error = 'Baud rate must be a positive number.');
      return;
    }
    if (_dataBits < 5 || _dataBits > 8) {
      setState(() => _error = 'Data bits must be between 5 and 8.');
      return;
    }
    if (_stopBits != 1 && _stopBits != 2) {
      setState(() => _error = 'Stop bits must be 1 or 2.');
      return;
    }

    Navigator.of(context).pop(
      _SerialConnectionDraft(
        config: SerialConnectionConfig(
          serialPort: serialPort,
          baudRate: baudRate,
          dataBits: _dataBits,
          parity: _parity,
          stopBits: _stopBits,
          flowControl: _flowControl,
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return _WorkspaceDialogFrame(
      width: 560,
      title: Text(tr('common.label.serial', fallback: 'Serial')),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: _WorkspaceSelect<String>(
                  label: 'Serial Port',
                  value: _portController.text.trim(),
                  size: _WorkspaceControlSize.large,
                  editable: true,
                  clearable: true,
                  inputController: _portController,
                  items: [
                    for (final port in _serialPorts)
                      DropdownMenuItem<String>(
                        value: port.path,
                        child: Text(port.label),
                      ),
                  ],
                  onChanged: (value) {
                    if (value == null) {
                      return;
                    }
                    _portController.text = value;
                  },
                ),
              ),
              SizedBox(width: 8),
              _WorkspaceButton(
                icon: Icons.refresh_rounded,
                size: _WorkspaceControlSize.large,
                height: _WorkspaceControlSize.large.inputHeight,
                minWidth: _WorkspaceControlSize.large.inputHeight,
                tooltip: tr(
                  'workspace.label.refreshSerialDevices',
                  fallback: 'Refresh serial devices',
                ),
                onPressed: _loadingPorts ? null : _refreshSerialPorts,
              ),
            ],
          ),
          if (_loadingPorts || _portListError != null || _serialPorts.isEmpty)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                _serialPortStatusText,
                style: TextStyle(
                  color: _portListError == null
                      ? _mutedText
                      : const Color(0xffef4444),
                  fontSize: NautermFontSizes.labelMedium,
                  fontWeight: NautermFontWeights.medium,
                  letterSpacing: 0,
                ),
              ),
            ),
          SizedBox(height: 12),
          _WorkspaceSelect<int>(
            label: 'Baud Rate',
            value: int.tryParse(_baudController.text.trim()) ?? 0,
            size: _WorkspaceControlSize.large,
            editable: true,
            clearable: true,
            inputController: _baudController,
            items: [
              for (final rate in _commonSerialBaudRates)
                DropdownMenuItem<int>(value: rate, child: Text(tr('$rate'))),
            ],
            onChanged: (value) {
              if (value == null) {
                return;
              }
              _baudController.text = '$value';
            },
          ),
          SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: _WorkspaceSelect<int>(
                  label: 'Data Bits',
                  value: _dataBits,
                  size: _WorkspaceControlSize.large,
                  items: const [
                    DropdownMenuItem<int>(value: 5, child: Text('5')),
                    DropdownMenuItem<int>(value: 6, child: Text('6')),
                    DropdownMenuItem<int>(value: 7, child: Text('7')),
                    DropdownMenuItem<int>(value: 8, child: Text('8')),
                  ],
                  onChanged: (value) {
                    if (value != null) {
                      setState(() => _dataBits = value);
                    }
                  },
                ),
              ),
              SizedBox(width: 10),
              Expanded(
                child: _WorkspaceSelect<int>(
                  label: 'Stop Bits',
                  value: _stopBits,
                  size: _WorkspaceControlSize.large,
                  items: const [
                    DropdownMenuItem<int>(value: 1, child: Text('1')),
                    DropdownMenuItem<int>(value: 2, child: Text('2')),
                  ],
                  onChanged: (value) {
                    if (value != null) {
                      setState(() => _stopBits = value);
                    }
                  },
                ),
              ),
            ],
          ),
          SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: _WorkspaceSelect<SerialParity>(
                  label: 'Parity',
                  value: _parity,
                  size: _WorkspaceControlSize.large,
                  items: [
                    for (final parity in SerialParity.values)
                      DropdownMenuItem<SerialParity>(
                        value: parity,
                        child: Text(tr(_serialParityLabel(parity))),
                      ),
                  ],
                  onChanged: (value) {
                    if (value != null) {
                      setState(() => _parity = value);
                    }
                  },
                ),
              ),
              SizedBox(width: 10),
              Expanded(
                child: _WorkspaceSelect<SerialFlowControl>(
                  label: 'Flow Control',
                  value: _flowControl,
                  size: _WorkspaceControlSize.large,
                  items: [
                    for (final flowControl in SerialFlowControl.values)
                      DropdownMenuItem<SerialFlowControl>(
                        value: flowControl,
                        child: Text(tr(_serialFlowControlLabel(flowControl))),
                      ),
                  ],
                  onChanged: (value) {
                    if (value != null) {
                      setState(() => _flowControl = value);
                    }
                  },
                ),
              ),
            ],
          ),
          if (_error != null) ...[
            SizedBox(height: 12),
            Text(
              _error!,
              style: TextStyle(
                color: Color(0xffef4444),
                fontSize: NautermFontSizes.labelMedium,
                fontWeight: NautermFontWeights.medium,
                letterSpacing: 0,
              ),
            ),
          ],
        ],
      ),
      actions: [
        _WorkspaceButton(
          label: 'Cancel',
          variant: _WorkspaceButtonVariant.text,
          onPressed: () => Navigator.of(context).pop(),
        ),
        _WorkspaceButton(
          label: 'Connect',
          type: _WorkspaceButtonType.primary,
          variant: _WorkspaceButtonVariant.solid,
          onPressed: _submit,
        ),
      ],
    );
  }

  String get _serialPortStatusText {
    if (_loadingPorts) {
      return 'Scanning serial devices...';
    }
    final error = _portListError;
    if (error != null) {
      return error;
    }
    return 'No serial devices found. Enter a path manually.';
  }
}

const List<int> _commonSerialBaudRates = [
  1200,
  2400,
  4800,
  9600,
  19200,
  38400,
  57600,
  115200,
  230400,
];

String _serialParityLabel(SerialParity parity) {
  return switch (parity) {
    SerialParity.none => 'None',
    SerialParity.even => 'Even',
    SerialParity.odd => 'Odd',
  };
}

String _serialFlowControlLabel(SerialFlowControl flowControl) {
  return switch (flowControl) {
    SerialFlowControl.none => 'None',
    SerialFlowControl.software => 'Software',
    SerialFlowControl.hardware => 'Hardware',
  };
}

String _defaultSerialPort() {
  if (io.Platform.isWindows) {
    return 'COM1';
  }
  if (io.Platform.isMacOS) {
    return '/dev/cu.usbserial';
  }
  return '/dev/ttyUSB0';
}

String _workspaceDataLoadNotification(Object error) {
  final text = error.toString();
  if (text.contains('unsupported') && text.contains('database schema')) {
    return 'Workspace database uses an unsupported schema. Reset the local database.';
  }
  return 'Failed to load workspace data.';
}
