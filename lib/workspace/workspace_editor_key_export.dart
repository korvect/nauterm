part of 'nauterm_workspace.dart';

@visibleForTesting
Widget buildKeyExportDrawerForTesting({
  required TerminalController Function() createConnection,
  required Future<void> Function(TerminalController) onExport,
}) => _KeyExportEditorContent(
  request: const _KeyExportEditorRequest(
    key: KeyEntry(name: 'Test key', publicKey: 'ssh-ed25519 AAAATEST'),
  ),
  hosts: const [
    HostEntry(
      id: 1,
      name: 'Test server',
      host: 'example.test',
      type: NautermHostType.remote,
    ),
  ],
  initialHostId: 1,
  onClose: () {},
  onShowNotification: (message, {type = _WorkspaceNotificationType.info}) {},
  createConnection: (_) => createConnection(),
  buildConnectionPage: (connection, onClose) => _TerminalConnectionPage(
    inDrawer: true,
    controller: connection,
    keys: const [],
    identities: const [],
    onCloseRequested: onClose,
  ),
  onExport: (key, draft, connection) => onExport(connection),
  buildHostSelector: (search, onBack, onSelected) => _SftpHostSelectorPane(
    groups: const [],
    tags: const [],
    searchController: search,
    onBack: onBack,
    hosts: const [
      _HostItem(
        id: 1,
        name: 'Test server',
        subtitle: 'example.test',
        icon: Icons.dns,
        color: Colors.blue,
        type: 'remote',
      ),
    ],
    onHostSelected: (host) => onSelected(host.id),
  ),
);

const String _defaultKeyExportScript = r'''if test ! -e "$1"; then
  mkdir -p "$1"
  chmod 700 "$1"
fi
if test ! -e "$1/$2"; then
  touch "$1/$2"
  chmod 600 "$1/$2"
fi
printf '%s\n' "$3" >> "$1/$2"
''';

class _KeyExportDraft {
  const _KeyExportDraft({
    required this.hostId,
    required this.location,
    required this.filename,
    required this.script,
  });

  final int hostId;
  final String location;
  final String filename;
  final String script;
}

class _KeyExportEditorContent extends StatefulWidget {
  const _KeyExportEditorContent({
    required this.request,
    required this.hosts,
    required this.onClose,
    required this.onExport,
    required this.createConnection,
    required this.buildConnectionPage,
    required this.buildHostSelector,
    required this.onShowNotification,
    this.initialHostId,
  });

  final _KeyExportEditorRequest request;
  final List<HostEntry> hosts;
  final int? initialHostId;
  final VoidCallback onClose;
  final _ExportKey onExport;
  final _CreateKeyExportConnection createConnection;
  final _BuildKeyExportConnectionPage buildConnectionPage;
  final _BuildDrawerHostSelector buildHostSelector;
  final _ShowWorkspaceNotification onShowNotification;

  @override
  State<_KeyExportEditorContent> createState() =>
      _KeyExportEditorContentState();
}

class _KeyExportEditorContentState extends State<_KeyExportEditorContent> {
  late final TextEditingController _locationController;
  late final TextEditingController _filenameController;
  late final TextEditingController _scriptController;
  int? _hostId;
  bool _selectingHost = false;
  final _hostSearchController = TextEditingController();
  bool _exporting = false;
  TerminalController? _connection;
  _KeyExportDraft? _draft;
  bool _commandStarted = false;
  Timer? _completionTimer;
  bool _exported = false;
  String? _exportError;
  bool _advancedExpanded = false;
  bool _advancedHovered = false;
  String? _hostError;
  String? _locationError;
  String? _filenameError;
  String? _scriptError;

  @override
  void initState() {
    super.initState();
    _hostId = widget.initialHostId;
    _hostSearchController.addListener(_refreshHostSearch);
    _locationController = TextEditingController(text: '.ssh');
    _filenameController = TextEditingController(text: 'authorized_keys');
    _scriptController = TextEditingController(text: _defaultKeyExportScript);
    _locationController.addListener(_clearLocationError);
    _filenameController.addListener(_clearFilenameError);
    _scriptController.addListener(_clearScriptError);
  }

  @override
  void dispose() {
    _hostSearchController.dispose();
    _completionTimer?.cancel();
    _connection?.removeListener(_handleConnection);
    _connection?.dispose();
    _locationController.removeListener(_clearLocationError);
    _filenameController.removeListener(_clearFilenameError);
    _scriptController.removeListener(_clearScriptError);
    _locationController.dispose();
    _filenameController.dispose();
    _scriptController.dispose();
    super.dispose();
  }

  void _clearLocationError() {
    if (_locationError != null && _locationController.text.trim().isNotEmpty) {
      setState(() => _locationError = null);
    }
  }

  void _refreshHostSearch() => setState(() {});

  void _clearFilenameError() {
    if (_filenameError != null &&
        _isValidKeyExportFilename(_filenameController.text)) {
      setState(() => _filenameError = null);
    }
  }

  void _clearScriptError() {
    if (_scriptError != null && _scriptController.text.trim().isNotEmpty) {
      setState(() => _scriptError = null);
    }
  }

  Future<void> _export() async {
    final publicKey = _emptyToNull(widget.request.key.publicKey);
    if (publicKey == null || !_looksLikeSshPublicKey(publicKey)) {
      _showError('A valid public key is required.');
      return;
    }
    final hostId = _hostId;
    final location = _locationController.text.trim();
    final filename = _filenameController.text.trim();
    final script = _scriptController.text;
    final hostError = hostId == null ? 'Select a host.' : null;
    final locationError = location.isEmpty ? 'Location is required.' : null;
    final filenameError = _isValidKeyExportFilename(filename)
        ? null
        : 'Enter a valid filename.';
    final scriptError = script.trim().isEmpty ? 'Script is required.' : null;
    if (hostError != null ||
        locationError != null ||
        filenameError != null ||
        scriptError != null) {
      setState(() {
        _hostError = hostError;
        _locationError = locationError;
        _filenameError = filenameError;
        _scriptError = scriptError;
        if (scriptError != null) {
          _advancedExpanded = true;
        }
      });
      return;
    }

    try {
      _draft = _KeyExportDraft(
        hostId: hostId!,
        location: location,
        filename: filename,
        script: script,
      );
      final connection = widget.createConnection(hostId);
      setState(() {
        _connection = connection;
        _exporting = true;
        _commandStarted = false;
        _exportError = null;
        _exported = false;
      });
      connection.addListener(_handleConnection);
      _handleConnection();
    } catch (error) {
      _showError('Failed to start key export: $error');
    }
  }

  void _handleConnection() {
    if (!mounted || _commandStarted) return;
    if (_connection?.connectionStatus.phase !=
        TerminalConnectionPhase.connected) {
      _completionTimer?.cancel();
      _completionTimer = null;
      return;
    }
    _completionTimer ??= Timer(_connectionCompletionHoldDuration, () {
      _completionTimer = null;
      if (!mounted ||
          _connection?.connectionStatus.phase !=
              TerminalConnectionPhase.connected) {
        return;
      }
      setState(() => _commandStarted = true);
      unawaited(_runExport());
    });
  }

  Future<void> _runExport() async {
    final connection = _connection!;
    try {
      await widget.onExport(widget.request.key, _draft!, connection);
      if (!mounted || !identical(connection, _connection)) return;
      setState(() {
        _exported = true;
        _exporting = false;
      });
    } catch (error) {
      if (!mounted || !identical(connection, _connection)) return;
      setState(() {
        _exportError = '$error';
        _exporting = false;
      });
    }
  }

  void _backToExportForm() {
    _completionTimer?.cancel();
    _completionTimer = null;
    _connection?.removeListener(_handleConnection);
    _connection?.dispose();
    setState(() {
      _connection = null;
      _exporting = false;
      _commandStarted = false;
      _exported = false;
      _exportError = null;
    });
  }

  void _showError(String message) {
    widget.onShowNotification(message, type: _WorkspaceNotificationType.error);
  }

  Widget _buildResultPage() {
    final statusColor = _exported
        ? const Color(0xff34a853)
        : const Color(0xffe5453d);
    final host = widget.hosts
        .where((host) => host.id == _draft?.hostId)
        .firstOrNull;
    return LayoutBuilder(
      builder: (context, constraints) => SingleChildScrollView(
        key: const ValueKey('key-export-result-page'),
        padding: const EdgeInsets.all(24),
        child: ConstrainedBox(
          constraints: BoxConstraints(
            minHeight: math.max(0, constraints.maxHeight - 48),
          ),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              if (_exporting)
                const SizedBox(
                  width: 40,
                  height: 40,
                  child: CircularProgressIndicator(),
                )
              else
                Container(
                  width: 64,
                  height: 64,
                  decoration: BoxDecoration(
                    color: statusColor.withValues(alpha: 0.12),
                    shape: BoxShape.circle,
                  ),
                  child: Icon(
                    _exported
                        ? Icons.check_rounded
                        : Icons.error_outline_rounded,
                    color: statusColor,
                    size: 32,
                  ),
                ),
              const SizedBox(height: 20),
              Text(
                _exporting
                    ? 'Exporting key...'
                    : _exported
                    ? 'Key exported successfully.'
                    : 'Key export failed',
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: _text,
                  fontSize: 18,
                  fontWeight: FontWeight.w600,
                ),
              ),
              const SizedBox(height: 12),
              Text(
                '${widget.request.key.name} → ${host?.name ?? 'Host'}',
                textAlign: TextAlign.center,
                style: TextStyle(color: _mutedText),
              ),
              const SizedBox(height: 8),
              Text(
                '${_draft!.location}/${_draft!.filename}',
                textAlign: TextAlign.center,
                style: TextStyle(color: _mutedText),
              ),
              if (_exportError != null) ...[
                const SizedBox(height: 24),
                Container(
                  width: double.infinity,
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: _card,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: SelectableText(
                    _exportError!,
                    style: TextStyle(color: _text),
                  ),
                ),
                const SizedBox(height: 12),
                Text(
                  'Check the remote file before retrying. The command may have partially completed.',
                  textAlign: TextAlign.center,
                  style: TextStyle(color: _mutedText),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_selectingHost) {
      return widget.buildHostSelector(
        _hostSearchController,
        () => setState(() => _selectingHost = false),
        (hostId) => setState(() {
          _hostId = hostId;
          _hostError = null;
          _selectingHost = false;
        }),
      );
    }
    final connection = _connection;
    if (connection != null) {
      return _EditorShell(
        title: 'Export Key',
        onClose: widget.onClose,
        onSave: _exported ? widget.onClose : _backToExportForm,
        saveLabel: _exported ? 'Done' : 'Back',
        saving: _commandStarted && _exporting,
        savingLabel: 'Exporting...',
        body: !_commandStarted
            ? widget.buildConnectionPage(connection, _backToExportForm)
            : _buildResultPage(),
        children: const [],
      );
    }
    final key = widget.request.key;
    final selectedHost = widget.hosts
        .where((host) => host.id == _hostId)
        .firstOrNull;

    return _EditorShell(
      title: 'Export Key',
      onClose: widget.onClose,
      onSave: _export,
      saving: _exporting,
      saveLabel: 'Export to host',
      savingLabel: 'Exporting...',
      children: [
        _WorkspaceFormSection(
          title: 'Key',
          children: [
            _KeyExportSummary(
              name: key.name,
              type: _keyTypeLabel(
                publicKey: key.publicKey,
                privateKey: key.privateKey,
                certificate: key.certificate,
              ),
            ),
          ],
        ),
        SizedBox(height: 14),
        _WorkspaceFormSection(
          title: 'Destination',
          children: [
            _WorkspaceFieldFrame(
              key: const ValueKey('key-export-host-selector'),
              label: 'Host',
              isRequired: true,
              size: _WorkspaceControlSize.large,
              focused: false,
              floatingLabel: true,
              hasContent: selectedHost != null,
              errorText: _hostError,
              onTap: () => setState(() => _selectingHost = true),
              child: LayoutBuilder(
                builder: (context, constraints) => Row(
                  children: [
                    Expanded(
                      child: Text(
                        selectedHost == null
                            ? ''
                            : _portForwardHostLabel(selectedHost),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    if (constraints.maxWidth >= 18)
                      Icon(
                        Icons.chevron_right_rounded,
                        size: 18,
                        color: _mutedText,
                      ),
                  ],
                ),
              ),
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _WorkspaceInput(
              controller: _locationController,
              label: 'Location (\$1)',
              isRequired: true,
              errorText: _locationError,
            ),
            SizedBox(height: _workspaceFormFieldGap),
            _WorkspaceInput(
              controller: _filenameController,
              label: 'Filename (\$2)',
              isRequired: true,
              errorText: _filenameError,
            ),
            SizedBox(height: 12),
            const _KeyExportNotice(),
          ],
        ),
        SizedBox(height: 14),
        _KeyExportAdvancedSection(
          expanded: _advancedExpanded,
          hovered: _advancedHovered,
          scriptController: _scriptController,
          scriptError: _scriptError,
          onHovered: (value) => setState(() => _advancedHovered = value),
          onExpandedChanged: () =>
              setState(() => _advancedExpanded = !_advancedExpanded),
        ),
      ],
    );
  }
}

bool _isValidKeyExportFilename(String value) {
  final filename = value.trim();
  return filename.isNotEmpty &&
      filename != '.' &&
      filename != '..' &&
      !filename.contains('/') &&
      !filename.contains(r'\');
}

class _KeyExportAdvancedSection extends StatelessWidget {
  const _KeyExportAdvancedSection({
    required this.expanded,
    required this.hovered,
    required this.scriptController,
    required this.scriptError,
    required this.onHovered,
    required this.onExpandedChanged,
  });

  final bool expanded;
  final bool hovered;
  final TextEditingController scriptController;
  final String? scriptError;
  final ValueChanged<bool> onHovered;
  final VoidCallback onExpandedChanged;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        MouseRegion(
          cursor: SystemMouseCursors.click,
          onEnter: (_) => onHovered(true),
          onExit: (_) => onHovered(false),
          child: Semantics(
            button: true,
            expanded: expanded,
            child: Material(
              color: Colors.transparent,
              borderRadius: BorderRadius.circular(6),
              clipBehavior: Clip.antiAlias,
              child: InkWell(
                key: const ValueKey('key-export-advanced-toggle'),
                onTap: onExpandedChanged,
                hoverColor: Colors.transparent,
                splashColor: _workspaceDark
                    ? _workspaceMenuPressed
                    : const Color(0xffdbe8eb).withValues(alpha: 0.5),
                highlightColor: Colors.transparent,
                child: SizedBox(
                  height: 30,
                  child: Row(
                    children: [
                      AnimatedRotation(
                        turns: expanded ? 0.25 : 0,
                        duration: const Duration(milliseconds: 150),
                        curve: Curves.easeOutCubic,
                        child: Icon(
                          LucideIcons.chevronRight,
                          size: 15,
                          color: hovered
                              ? _mutedText
                              : _mutedText.withValues(alpha: 0.68),
                        ),
                      ),
                      SizedBox(width: 5),
                      Text(
                        'Advanced',
                        style: TextStyle(
                          color: hovered ? _text : _mutedText,
                          fontSize: NautermFontSizes.labelMedium,
                          fontWeight: NautermFontWeights.medium,
                          letterSpacing: 0,
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ),
        AnimatedSize(
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOutCubic,
          alignment: Alignment.topCenter,
          child: expanded
              ? Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      color: _card,
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Padding(
                      padding: const EdgeInsets.all(16),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            r'Use $1 for location, $2 for filename, and $3 for the public key.',
                            style: TextStyle(
                              color: _mutedText,
                              fontSize: NautermFontSizes.labelMedium,
                              fontWeight: NautermFontWeights.regular,
                              height: 1.35,
                            ),
                          ),
                          SizedBox(height: 10),
                          _WorkspaceInput(
                            controller: scriptController,
                            label: 'Script',
                            errorText: scriptError,
                            minLines: 8,
                            maxLines: 12,
                          ),
                        ],
                      ),
                    ),
                  ),
                )
              : const SizedBox.shrink(),
        ),
      ],
    );
  }
}

class _KeyExportSummary extends StatelessWidget {
  const _KeyExportSummary({required this.name, required this.type});

  final String name;
  final String type;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 40,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final details = _CardText(title: name, subtitle: type);
          if (constraints.maxWidth < 64) {
            return details;
          }
          return Row(
            children: [
              const _BrandIcon(
                icon: Icons.key_rounded,
                color: Color(0xff075e92),
              ),
              SizedBox(width: 10),
              Expanded(child: details),
            ],
          );
        },
      ),
    );
  }
}

class _KeyExportNotice extends StatelessWidget {
  const _KeyExportNotice();

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: _blue.withValues(alpha: _workspaceDark ? 0.12 : 0.07),
        borderRadius: BorderRadius.circular(9),
      ),
      child: Padding(
        padding: const EdgeInsets.all(11),
        child: LayoutBuilder(
          builder: (context, constraints) {
            final message = Text(
              'Exports the public key to a UNIX host. The target directory '
              'and file are created with secure permissions.',
              style: TextStyle(
                color: _mutedText,
                fontSize: NautermFontSizes.labelMedium,
                fontWeight: NautermFontWeights.regular,
                height: 1.35,
              ),
            );
            if (constraints.maxWidth < 32) {
              return message;
            }
            return Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Icon(Icons.info_outline_rounded, size: 17, color: _blue),
                SizedBox(width: 9),
                Expanded(child: message),
              ],
            );
          },
        ),
      ),
    );
  }
}
