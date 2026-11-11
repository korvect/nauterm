part of 'nauterm_workspace.dart';

class _SerialConnectionDraft {
  const _SerialConnectionDraft({required this.config, this.saveAsHost = false});

  final SerialConnectionConfig config;
  final bool saveAsHost;

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
  const _SerialConnectionDialog({this.allowSave = false});

  final bool allowSave;

  @override
  State<_SerialConnectionDialog> createState() =>
      _SerialConnectionDialogState();
}

class _SerialConnectionDialogState extends State<_SerialConnectionDialog> {
  late final _SerialConnectionFormController _serialForm;

  @override
  void initState() {
    super.initState();
    _serialForm = _SerialConnectionFormController();
    _serialForm.refreshPorts();
  }

  @override
  void dispose() {
    _serialForm.dispose();
    super.dispose();
  }

  void _submit({bool saveAsHost = false}) {
    final config = _serialForm.readConfig();
    if (config == null) return;
    Navigator.of(context)
        .pop(_SerialConnectionDraft(config: config, saveAsHost: saveAsHost));
  }

  @override
  Widget build(BuildContext context) => _WorkspaceDialogFrame(
    width: 560,
    title: Text(tr('common.label.serial', fallback: 'Serial')),
    content: _SerialConnectionFields(controller: _serialForm),
    actions: [
      _WorkspaceButton(
        label: tr('common.action.cancel', fallback: 'Cancel'),
        variant: _WorkspaceButtonVariant.text,
        onPressed: () => Navigator.of(context).pop(),
      ),
      if (widget.allowSave)
        _WorkspaceButton(
          label: tr('workspace.label.connectSave', fallback: 'Connect & Save'),
          onPressed: () => _submit(saveAsHost: true),
        ),
      _WorkspaceButton(
        label: tr('common.action.connect', fallback: 'Connect'),
        type: _WorkspaceButtonType.primary,
        variant: _WorkspaceButtonVariant.solid,
        onPressed: _submit,
      ),
    ],
  );
}

class _SerialConnectionFormController extends ChangeNotifier {
  _SerialConnectionFormController({SerialConnectionConfig? initialConfig})
    : portController = TextEditingController(
        text: initialConfig?.serialPort ?? _defaultSerialPort(),
      ),
      baudController = TextEditingController(
        text: '${initialConfig?.baudRate ?? 115200}',
      ),
      dataBits = initialConfig?.dataBits ?? 8,
      parity = initialConfig?.parity ?? SerialParity.none,
      stopBits = initialConfig?.stopBits ?? 1,
      flowControl = initialConfig?.flowControl ?? SerialFlowControl.none,
      _hasSavedPort = initialConfig != null {
    portController.addListener(notifyListeners);
    baudController.addListener(notifyListeners);
  }

  final TextEditingController portController;
  final TextEditingController baudController;
  final bool _hasSavedPort;
  List<SerialPortInfo> ports = const [];
  bool loadingPorts = false;
  String? portListError;
  String? error;
  int dataBits;
  SerialParity parity;
  int stopBits;
  SerialFlowControl flowControl;
  bool _disposed = false;

  void updateOptions({
    int? dataBits,
    SerialParity? parity,
    int? stopBits,
    SerialFlowControl? flowControl,
  }) {
    this.dataBits = dataBits ?? this.dataBits;
    this.parity = parity ?? this.parity;
    this.stopBits = stopBits ?? this.stopBits;
    this.flowControl = flowControl ?? this.flowControl;
    notifyListeners();
  }

  Future<void> refreshPorts() async {
    if (loadingPorts || _disposed) return;
    loadingPorts = true;
    portListError = null;
    notifyListeners();
    try {
      final detected = await Future<List<SerialPortInfo>>(
        NativeTerminalDriver.listSerialPorts,
      );
      if (_disposed) return;
      ports = detected;
      loadingPorts = false;
      final currentPort = portController.text.trim();
      if (!_hasSavedPort &&
          detected.isNotEmpty &&
          (currentPort.isEmpty || currentPort == _defaultSerialPort())) {
        portController.text = detected.first.path;
      }
    } on Object catch (error) {
      if (_disposed) return;
      ports = const [];
      loadingPorts = false;
      portListError = 'Unable to enumerate serial devices: $error';
    }
    notifyListeners();
  }

  SerialConnectionConfig? readConfig() {
    try {
      final config = SerialConnectionConfig.fromHostConfig(
        serialPort: portController.text.trim(),
        baudRate: int.tryParse(baudController.text.trim()) ?? 0,
        config: {
          'serial': {
            'data_bits': dataBits,
            'stop_bits': stopBits,
            'parity': parity.name,
            'flow_control': flowControl.name,
          },
        },
      );
      error = null;
      notifyListeners();
      return config;
    } on FormatException {
      error = tr(
        'workspace.serial.invalidSettings',
        fallback: 'Enter a serial port and a valid positive baud rate.',
      );
      notifyListeners();
      return null;
    }
  }

  String get portStatusText {
    if (loadingPorts) return 'Scanning serial devices...';
    return portListError ?? 'No serial devices found. Enter a path manually.';
  }

  @override
  void dispose() {
    _disposed = true;
    portController.removeListener(notifyListeners);
    baudController.removeListener(notifyListeners);
    portController.dispose();
    baudController.dispose();
    super.dispose();
  }
}

class _SerialPortField extends StatelessWidget {
  const _SerialPortField({required this.controller});

  final _SerialConnectionFormController controller;
  static const _size = _WorkspaceControlSize.large;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: controller,
    builder: (context, _) => Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Expanded(
          child: _WorkspaceSelect<String>(
            label: tr('workspace.label.serialPort', fallback: 'Serial Port'),
            value: controller.portController.text.trim(),
            size: _size,
            editable: true,
            clearable: true,
            inputController: controller.portController,
            items: [
              for (final port in controller.ports)
                DropdownMenuItem<String>(
                  value: port.path,
                  child: Text(port.label),
                ),
            ],
            onChanged: (value) {
              if (value != null) controller.portController.text = value;
            },
          ),
        ),
        SizedBox(width: 8),
        _WorkspaceButton(
          icon: Icons.refresh_rounded,
          size: _size,
          height: _size.inputHeight,
          minWidth: _size.inputHeight,
          tooltip: tr(
            'workspace.label.refreshSerialDevices',
            fallback: 'Refresh serial devices',
          ),
          onPressed: controller.loadingPorts ? null : controller.refreshPorts,
        ),
      ],
    ),
  );
}

class _SerialConnectionFields extends StatelessWidget {
  const _SerialConnectionFields({
    required this.controller,
    this.includePort = true,
    this.fieldGap = 12,
  });

  final _SerialConnectionFormController controller;
  final bool includePort;
  final double fieldGap;
  static const _size = _WorkspaceControlSize.large;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: controller,
    builder: (context, _) {
      final dataBits = _WorkspaceSelect<int>(
        label: tr('workspace.label.dataBits', fallback: 'Data Bits'),
        value: controller.dataBits,
        size: _size,
        items: [
          for (final bits in [5, 6, 7, 8])
            DropdownMenuItem<int>(value: bits, child: Text('$bits')),
        ],
        onChanged: (value) => controller.updateOptions(dataBits: value),
      );
      final stopBits = _WorkspaceSelect<int>(
        label: tr('workspace.label.stopBits', fallback: 'Stop Bits'),
        value: controller.stopBits,
        size: _size,
        items: [
          for (final bits in [1, 2])
            DropdownMenuItem<int>(value: bits, child: Text('$bits')),
        ],
        onChanged: (value) => controller.updateOptions(stopBits: value),
      );
      final parity = _WorkspaceSelect<SerialParity>(
        label: tr('common.label.parity', fallback: 'Parity'),
        value: controller.parity,
        size: _size,
        items: [
          for (final value in SerialParity.values)
            DropdownMenuItem<SerialParity>(
              value: value,
              child: Text(tr(_serialParityLabel(value))),
            ),
        ],
        onChanged: (value) => controller.updateOptions(parity: value),
      );
      final flowControl = _WorkspaceSelect<SerialFlowControl>(
        label: tr('workspace.label.flowControl', fallback: 'Flow Control'),
        value: controller.flowControl,
        size: _size,
        items: [
          for (final value in SerialFlowControl.values)
            DropdownMenuItem<SerialFlowControl>(
              value: value,
              child: Text(tr(_serialFlowControlLabel(value))),
            ),
        ],
        onChanged: (value) => controller.updateOptions(flowControl: value),
      );
      return Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (includePort) ...[
            _SerialPortField(controller: controller),
            SizedBox(height: fieldGap),
          ],
          if (controller.loadingPorts ||
              controller.portListError != null ||
              controller.ports.isEmpty) ...[
            Text(
              controller.portStatusText,
              style: TextStyle(
                color: controller.portListError == null
                    ? _mutedText
                    : const Color(0xffef4444),
                fontSize: NautermFontSizes.labelMedium,
                fontWeight: NautermFontWeights.medium,
                letterSpacing: 0,
              ),
            ),
            SizedBox(height: fieldGap),
          ],
          _WorkspaceSelect<int>(
            label: tr('workspace.label.baudRate', fallback: 'Baud Rate'),
            value: int.tryParse(controller.baudController.text.trim()) ?? 0,
            size: _size,
            editable: true,
            clearable: true,
            inputController: controller.baudController,
            items: [
              for (final rate in _commonSerialBaudRates)
                DropdownMenuItem<int>(value: rate, child: Text('$rate')),
            ],
            onChanged: (value) {
              if (value != null) controller.baudController.text = '$value';
            },
          ),
          SizedBox(height: fieldGap),
          Row(
            children: [
              Expanded(child: dataBits),
              SizedBox(width: 10),
              Expanded(child: parity),
            ],
          ),
          SizedBox(height: fieldGap),
          Row(
            children: [
              Expanded(child: stopBits),
              SizedBox(width: 10),
              Expanded(child: flowControl),
            ],
          ),
          if (controller.error != null) ...[
            SizedBox(height: fieldGap),
            Text(
              controller.error!,
              style: TextStyle(
                color: Color(0xffef4444),
                fontSize: NautermFontSizes.labelMedium,
                fontWeight: NautermFontWeights.medium,
                letterSpacing: 0,
              ),
            ),
          ],
        ],
      );
    },
  );
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
