part of 'nauterm_workspace.dart';

@immutable
class TerminalConnectionKeyOption {
  const TerminalConnectionKeyOption({
    required this.id,
    required this.name,
    this.privateKey,
    this.certificate,
    this.subtitle,
  });

  final int id;
  final String name;
  final String? privateKey;
  final String? certificate;
  final String? subtitle;
}

@immutable
class TerminalConnectionIdentityOption {
  const TerminalConnectionIdentityOption({
    required this.id,
    required this.name,
    this.username,
    this.password,
    this.keyId,
    this.keyName,
    this.privateKey,
  });

  final int id;
  final String name;
  final String? username;
  final String? password;
  final int? keyId;
  final String? keyName;
  final String? privateKey;

  String get subtitle {
    final parts = <String>[
      if (username != null && username!.trim().isNotEmpty) username!.trim(),
      if (keyName != null && keyName!.trim().isNotEmpty) keyName!.trim(),
    ];
    return parts.isEmpty ? 'Identity' : parts.join(', ');
  }
}

typedef TerminalConnectionAuthSaver = Future<void> Function(
  SshConnectionProfile profile, {
  String? username,
  String? password,
  TerminalConnectionKeyOption? key,
});

class _ReloadedTerminalConnection {
  const _ReloadedTerminalConnection({
    required this.profile,
    this.moshServerCommand,
  });

  final SshConnectionProfile profile;
  final String? moshServerCommand;
}

@visibleForTesting
SshConnectionProfile refreshSavedHostSshProfile({
  required SshConnectionProfile current,
  required HostEntry host,
  required String address,
  required int port,
  required String username,
  required Map<String, String> environment,
  String? password,
  String? privateKey,
  String? certificate,
  String? passphrase,
  TerminalProxyConfig? proxy,
}) {
  return SshConnectionProfile(
    host: address,
    port: port,
    username: username,
    knownHostsPath: current.knownHostsPath,
    hostId: host.id,
    identityId: host.identityId,
    label: host.name,
    password: password,
    privateKey: privateKey,
    certificate: certificate,
    passphrase: passphrase,
    proxy: proxy,
    shellPath: _emptyToNull(host.shellPath),
    environment: environment,
    encoding: host.encoding,
  );
}

@visibleForTesting
Widget buildTerminalConnectionPageForTesting(
  TerminalController controller, {
  bool inDrawer = false,
  List<TerminalConnectionKeyOption> keys = const [],
  SshConnectionProfile? Function()? reloadProfile,
}) {
  return _TerminalConnectionPage(
    controller: controller,
    inDrawer: inDrawer,
    keys: keys,
    identities: const [],
    onReloadConnection: reloadProfile == null
        ? null
        : () {
            final profile = reloadProfile();
            return profile == null
                ? null
                : _ReloadedTerminalConnection(profile: profile);
          },
  );
}

@visibleForTesting
Widget buildPendingHostConnectionPageForTesting(HostEntry host) =>
    _PendingHostConnectionPage(
      pending: _PendingHostConnection(
        host: host,
        item: _mapHost(host, const [], const {}),
      ),
      onConnect: (_) {},
      onCloseRequested: () {},
    );

class _PendingHostConnectionPage extends StatefulWidget {
  const _PendingHostConnectionPage({
    super.key,
    required this.pending,
    required this.onConnect,
    required this.onCloseRequested,
  });

  final _PendingHostConnection pending;
  final ValueChanged<_HostProtocolSelection> onConnect;
  final VoidCallback onCloseRequested;

  @override
  State<_PendingHostConnectionPage> createState() =>
      _PendingHostConnectionPageState();
}

class _PendingHostConnectionPageState
    extends State<_PendingHostConnectionPage> {
  late _HostConnectProtocol _selected;
  bool _starting = false;
  bool _invalidPort = false;
  late final TextEditingController _sshPortController;
  late final TextEditingController _moshPortController;
  late final TextEditingController _telnetPortController;
  late final TextEditingController _moshServerCommandController;

  @override
  void initState() {
    super.initState();
    final host = widget.pending.host;
    _selected = host.sshEnabled
        ? _HostConnectProtocol.ssh
        : host.moshEnabled
        ? _HostConnectProtocol.mosh
        : _HostConnectProtocol.telnet;
    _sshPortController = TextEditingController(
      text: (host.port ?? 22).toString(),
    );
    _moshPortController = TextEditingController(
      text: (host.port ?? 22).toString(),
    );
    _telnetPortController = TextEditingController(
      text: (host.telnetPort ?? 23).toString(),
    );
    _moshServerCommandController = TextEditingController(
      text: host.moshServerCommand,
    );
  }

  @override
  void dispose() {
    _sshPortController.dispose();
    _moshPortController.dispose();
    _telnetPortController.dispose();
    _moshServerCommandController.dispose();
    super.dispose();
  }

  TextEditingController _portControllerFor(_HostConnectProtocol protocol) {
    return switch (protocol) {
      _HostConnectProtocol.ssh => _sshPortController,
      _HostConnectProtocol.mosh => _moshPortController,
      _HostConnectProtocol.telnet => _telnetPortController,
    };
  }

  void _selectProtocol(_HostConnectProtocol protocol) {
    if (_starting) {
      return;
    }
    setState(() {
      _selected = protocol;
      _invalidPort = false;
    });
  }

  void _connect() {
    final port = int.tryParse(_portControllerFor(_selected).text.trim());
    if (port == null || port < 1 || port > 65535) {
      setState(() => _invalidPort = true);
      return;
    }
    setState(() {
      _starting = true;
      _invalidPort = false;
    });
    widget.onConnect(
      _HostProtocolSelection(
        protocol: _selected,
        port: port,
        moshServerCommand:
            _emptyToNull(_moshServerCommandController.text) ??
            defaultMoshServerCommand,
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final host = widget.pending.host;
    final item = widget.pending.item;
    final address = _emptyToNull(host.host) ?? item.host ?? host.name;
    return _ConnectionPageViewport(
      child: _ConnectionPagePanel(
        title: host.name,
        subtitle: address,
        hostOs: host.os,
        hostDistro: host.distro,
        progress: _ConnectionProgress(
          icons: const [Icons.power_rounded, LucideIcons.squareTerminal],
          reachedStep: 0,
          spinningStep: _starting ? 0 : null,
        ),
        body: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              tr('common.label.protocol', fallback: 'Protocol'),
              style: TextStyle(
                color: _text,
                fontSize: NautermFontSizes.titleSmall,
                fontWeight: NautermFontWeights.semibold,
                letterSpacing: 0,
              ),
            ),
            SizedBox(height: 14),
            Flexible(
              fit: FlexFit.loose,
              child: SingleChildScrollView(
                key: const ValueKey('connection-protocol-scrollable'),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    if (host.sshEnabled)
                      _HostProtocolCard(
                        title: 'SSH',
                        command: 'ssh $address',
                        portController: _sshPortController,
                        color: _green,
                        selected: _selected == _HostConnectProtocol.ssh,
                        enabled: !_starting,
                        onTap: () => _selectProtocol(_HostConnectProtocol.ssh),
                      ),
                    if (host.moshEnabled) ...[
                      SizedBox(height: 10),
                      _HostProtocolCard(
                        title: 'Mosh',
                        command: 'mosh $address',
                        portController: _moshPortController,
                        color: const Color(0xff0ea5a8),
                        selected: _selected == _HostConnectProtocol.mosh,
                        enabled: !_starting,
                        commandController: _moshServerCommandController,
                        onTap: () => _selectProtocol(_HostConnectProtocol.mosh),
                      ),
                    ],
                    if (host.telnetEnabled) ...[
                      SizedBox(height: 10),
                      _HostProtocolCard(
                        title: 'Telnet',
                        command: 'telnet $address',
                        portController: _telnetPortController,
                        color: const Color(0xff7b61ff),
                        selected: _selected == _HostConnectProtocol.telnet,
                        enabled: !_starting,
                        onTap: () =>
                            _selectProtocol(_HostConnectProtocol.telnet),
                      ),
                    ],
                    if (_invalidPort) ...[
                      const SizedBox(height: 8),
                      Text(
                        'Port must be between 1 and 65535.',
                        style: TextStyle(
                          color: const Color(0xffef4444),
                          fontSize: NautermFontSizes.labelMedium,
                          fontWeight: NautermFontWeights.regular,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ],
        ),
        footer: Row(
          children: [
            _ConnectionButton(
              label: tr('common.action.close', fallback: 'Close'),
              onPressed: _starting ? null : widget.onCloseRequested,
            ),
            const Spacer(),
            _ConnectionButton(
              label: tr('common.action.connect', fallback: 'Connect'),
              primary: true,
              onPressed: _starting ? null : _connect,
            ),
          ],
        ),
      ),
    );
  }
}

class _TerminalConnectionPage extends StatefulWidget {
  const _TerminalConnectionPage({
    required this.controller,
    required this.keys,
    required this.identities,
    this.inDrawer = false,
    this.onSaveAuth,
    this.onAddKeyRequested,
    this.onEditHostRequested,
    this.onReloadConnection,
    this.onCloseRequested,
    this.dataStore,
  });

  final TerminalController controller;
  final bool inDrawer;
  final List<TerminalConnectionKeyOption> keys;
  final List<TerminalConnectionIdentityOption> identities;
  final TerminalConnectionAuthSaver? onSaveAuth;
  final VoidCallback? onAddKeyRequested;
  final VoidCallback? onEditHostRequested;
  final _ReloadedTerminalConnection? Function()? onReloadConnection;
  final VoidCallback? onCloseRequested;
  final NautermDataStore? dataStore;

  @override
  State<_TerminalConnectionPage> createState() =>
      _TerminalConnectionPageState();
}

enum _ConnectionPageMode {
  username,
  connecting,
  completing,
  hostKey,
  passphrase,
  authentication,
  connected,
  failed,
  exited,
}

enum _ConnectionIntervention {
  username,
  hostKey,
  passphrase,
  securityKeyPin,
  password,
  publicKey,
  verificationCode,
}

enum _ConnectionAuthTab { password, publicKey }

bool _shouldShowConnectionPage(
  TerminalController controller,
  TerminalConnectionStatus status,
  SshConnectionProfile? profile,
  SerialConnectionProfile? serialProfile,
  TelnetConnectionProfile? telnetProfile,
) {
  if (profile != null && controller.hasConnectedOnce) {
    final events = controller.connectionEvents;
    return status.phase == TerminalConnectionPhase.authentication ||
        _hasLatestActionableHostKey(events) ||
        _hasLatestActionableAuth(events);
  }
  return (profile != null || serialProfile != null || telnetProfile != null) &&
      status.phase != TerminalConnectionPhase.connected &&
      status.phase != TerminalConnectionPhase.idle;
}

class _TerminalConnectionPageState extends State<_TerminalConnectionPage> {
  static const _pipeAnimationDuration = Duration(milliseconds: 300);
  final TextEditingController _hostController = TextEditingController();
  final TextEditingController _portController = TextEditingController();
  final TextEditingController _usernameController = TextEditingController();
  final TextEditingController _passwordController = TextEditingController();
  final TextEditingController _passphraseController = TextEditingController();
  final TextEditingController _fido2PinController = TextEditingController();
  GlobalKey<_KeyboardInteractiveFormState> _challengeFormKey = GlobalKey();
  SshAuthenticationChallenge? _challenge;
  TerminalConnectionPhase? _lastPhase;
  SshConnectionProfile? _lastProfile;
  bool _showLogs = false;
  bool _obscurePassword = true;
  bool _obscurePassphrase = true;
  ({String privateKey, String passphrase})? _sessionKeyUnlock;
  ({String host, int port, int? identityId, String value})? _sessionUsername;
  bool _obscureFido2Pin = true;
  bool _saveAuthenticationChoice = false;
  _ConnectionAuthTab _authTab = _ConnectionAuthTab.password;
  int? _selectedKeyIndex;
  int? _selectedIdentityId;
  SshHostKeyTrustMode _sessionHostKeyTrustMode = SshHostKeyTrustMode.strict;
  final List<_ConnectionIntervention> _interventions =
      <_ConnectionIntervention>[];
  ({_ConnectionIntervention stage, int? challengeId, int? requestId})?
  _lastProgressPage;
  DateTime _connectionSegmentStartedAt = DateTime.now();
  _ConnectionPageMode? _lastActualMode;
  Timer? _connectedRevealTimer;
  bool _revealConnected = false;
  bool _syncingControllers = false;

  HostEntry? get _resolvedHost {
    final hostId =
        widget.controller.sshProfile?.hostId ??
        widget.controller.telnetProfile?.hostId ??
        widget.controller.serialProfile?.hostId;
    if (hostId == null || widget.dataStore == null) return null;
    return widget.dataStore!.getHost(hostId);
  }

  @override
  void initState() {
    super.initState();
    _usernameController.addListener(_handleUsernameChanged);
    _passwordController.addListener(_handlePasswordChanged);
    _passphraseController.addListener(_handlePassphraseChanged);
    _fido2PinController.addListener(_handleFido2PinChanged);
  }

  @override
  void dispose() {
    _connectedRevealTimer?.cancel();
    _usernameController.removeListener(_handleUsernameChanged);
    _passwordController.removeListener(_handlePasswordChanged);
    _passphraseController.removeListener(_handlePassphraseChanged);
    _fido2PinController.removeListener(_handleFido2PinChanged);
    _hostController.dispose();
    _portController.dispose();
    _usernameController.dispose();
    _passwordController.dispose();
    _passphraseController.dispose();
    _fido2PinController.dispose();
    super.dispose();
  }

  _ConnectionAuthTab get _effectiveAuthTab {
    final methods = widget.controller.authenticationMethods;
    if (methods != null) {
      final password = methods.contains('password');
      final publicKey = methods.contains('publickey');
      if (password && !publicKey) return _ConnectionAuthTab.password;
      if (publicKey && !password) return _ConnectionAuthTab.publicKey;
    }
    return _authTab;
  }

  bool get _showAuthTabs {
    final methods = widget.controller.authenticationMethods;
    return methods == null ||
        (methods.contains('password') && methods.contains('publickey'));
  }

  void _handlePasswordChanged() {
    if (_syncingControllers) return;
    if (mounted && _effectiveAuthTab == _ConnectionAuthTab.password) {
      setState(() {});
    }
  }

  bool get _requiresUsername =>
      widget.controller.sshProfile?.username.trim().isEmpty == true;

  void _handleUsernameChanged() {
    if (!_syncingControllers && mounted && _requiresUsername) {
      setState(() {});
    }
  }

  void _handleFido2PinChanged() {
    if (mounted && _requiresFido2Pin) {
      setState(() {});
    }
  }

  void _handlePassphraseChanged() {
    if (mounted &&
        _hasLatestPrivateKeyPassphraseRequest(
          widget.controller.connectionEvents,
        )) {
      setState(() {});
    }
  }

  bool get _requiresFido2Pin =>
      _hasLatestFido2PinRequest(widget.controller.connectionEvents);

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, _) {
        final status = widget.controller.connectionStatus;
        final profile = widget.controller.sshProfile;
        final serialProfile = widget.controller.serialProfile;
        _syncControllers(profile, status.phase);
        _syncAuthenticationChallenge();

        return _ConnectionPageViewport(
          inDrawer: widget.inDrawer,
          child: _buildPage(status, profile, serialProfile),
        );
      },
    );
  }

  Widget _buildPage(
    TerminalConnectionStatus status,
    SshConnectionProfile? profile,
    SerialConnectionProfile? serialProfile,
  ) {
    final events = widget.controller.connectionEvents;
    final challenge = _challenge;
    final telnetProfile = widget.controller.telnetProfile;
    final actualMode = _requiresUsername
        ? _ConnectionPageMode.username
        : _connectionMode(status, events);
    final mode = _displayMode(actualMode);
    _syncConnectionProgress(actualMode);
    final logs = _connectionLogEntries(status, profile, serialProfile, events);
    final diagnostics = _connectionDiagnostics(
      status,
      profile,
      serialProfile,
      events,
      logs,
    );

    return _ConnectionPagePanel(
      title: profile == null
          ? serialProfile?.label ??
                serialProfile?.serialPort ??
                telnetProfile?.label ??
                telnetProfile?.host ??
                'Connection'
          : _remoteConnectionDisplayTitle(
              host: profile.host,
              hostId: profile.hostId,
              label: profile.label,
            ),
      subtitle: serialProfile != null
          ? 'Serial ${serialProfile.serialPort} @ ${serialProfile.config.summary}'
          : telnetProfile != null
          ? 'Telnet ${telnetProfile.host}:${telnetProfile.port}'
          : profile == null
          ? 'Connection'
          : '${widget.controller.isMoshSession ? 'Mosh' : 'SSH'} ${profile.host}:${profile.port}',
      actionLabel: mode == _ConnectionPageMode.failed
          ? 'Copy logs'
          : _showLogs
          ? 'Hide logs'
          : 'Show logs',
      onAction: mode == _ConnectionPageMode.failed
          ? () => _copyDiagnostics(diagnostics)
          : () => setState(() => _showLogs = !_showLogs),
      hostOs: _resolvedHost?.os,
      hostDistro: _resolvedHost?.distro,
      progress: _ConnectionProgress.forConnection(
        mode: mode,
        interventions: _interventions,
        protocolIcon: _connectionProtocolIcon(widget.controller),
      ),
      body: mode == _ConnectionPageMode.failed
          ? _ConnectionFailureDetails(
              key: const ValueKey('failure-details'),
              logs: logs,
              diagnostics: diagnostics,
              onCopyDiagnostics: _copyDiagnostics,
            )
          : challenge != null
          ? _KeyboardInteractiveForm(
              key: _challengeFormKey,
              challenge: challenge,
              onSubmit: (responses) => widget.controller
                  .respondToAuthentication(challenge.id, responses),
              logs: _showLogs
                  ? _ConnectionLogsCard(
                      logs: logs,
                      diagnostics: diagnostics,
                      onCopyDiagnostics: _copyDiagnostics,
                    )
                  : null,
            )
          : _showLogs
          ? _ConnectionLogsCard(
              key: const ValueKey('logs'),
              logs: logs,
              diagnostics: diagnostics,
              onCopyDiagnostics: _copyDiagnostics,
            )
          : _buildModeBody(mode, status, profile, serialProfile, events),
      footer: _buildFooter(mode, status),
    );
  }

  _ConnectionIntervention _challengeProgressStage(
    SshAuthenticationChallenge challenge,
  ) {
    final prompts = challenge.prompts.map((prompt) => prompt.prompt).join('\n');
    final asksForPassword = RegExp(
      r'\bpassword\b|密码|口令',
      caseSensitive: false,
    ).hasMatch(prompts);
    final asksForCode = RegExp(
      r'\b(code|otp|token)\b|verification|one[ -]?time|验证码|认证码|动态|一次性',
      caseSensitive: false,
    ).hasMatch(prompts);
    return asksForPassword && !asksForCode
        ? _ConnectionIntervention.password
        : _ConnectionIntervention.verificationCode;
  }

  void _syncConnectionProgress(_ConnectionPageMode mode) {
    final challenge = _challenge;
    final stage = switch (mode) {
      _ConnectionPageMode.username => _ConnectionIntervention.username,
      _ConnectionPageMode.hostKey => _ConnectionIntervention.hostKey,
      _ConnectionPageMode.passphrase => _ConnectionIntervention.passphrase,
      _ConnectionPageMode.authentication =>
        challenge != null
            ? _challengeProgressStage(challenge)
            : _requiresFido2Pin
            ? _ConnectionIntervention.securityKeyPin
            : _effectiveAuthTab == _ConnectionAuthTab.publicKey
            ? _ConnectionIntervention.publicKey
            : _ConnectionIntervention.password,
      _ => null,
    };
    // Connection activity between forms stays on the last visible step.
    // Background key/agent attempts never create steps of their own.
    if (stage == null) return;
    final requestId = widget.controller.authenticationRequestId;
    final page = (
      stage: stage,
      challengeId: challenge?.id,
      requestId: requestId,
    );
    final previousPage = _lastProgressPage;
    if (page == previousPage) return;
    if (mode == _ConnectionPageMode.authentication &&
        _showAuthTabs &&
        requestId != null &&
        _interventions.isNotEmpty &&
        previousPage?.requestId == requestId) {
      _interventions[_interventions.length - 1] = stage;
      _lastProgressPage = page;
      return;
    }
    _lastProgressPage = page;
    _interventions.add(stage);
  }

  _ConnectionPageMode _displayMode(_ConnectionPageMode actualMode) {
    final previousMode = _lastActualMode;
    if (actualMode == _ConnectionPageMode.connecting &&
        previousMode != _ConnectionPageMode.connecting) {
      _connectionSegmentStartedAt = DateTime.now();
      _revealConnected = false;
      _connectedRevealTimer?.cancel();
      _connectedRevealTimer = null;
    } else if (actualMode == _ConnectionPageMode.connected &&
        previousMode != _ConnectionPageMode.connected) {
      _connectionSegmentStartedAt = DateTime.now();
      _revealConnected = false;
      _connectedRevealTimer?.cancel();
      _connectedRevealTimer = null;
    }
    _lastActualMode = actualMode;
    if (actualMode != _ConnectionPageMode.connected || _revealConnected) {
      return actualMode;
    }

    final elapsed = DateTime.now().difference(_connectionSegmentStartedAt);
    final remaining = _pipeAnimationDuration - elapsed;
    if (remaining <= Duration.zero) {
      _revealConnected = true;
      return actualMode;
    }
    _connectedRevealTimer ??= Timer(remaining, () {
      if (!mounted) return;
      setState(() {
        _revealConnected = true;
        _connectedRevealTimer = null;
      });
    });
    return _ConnectionPageMode.completing;
  }

  Future<void> _copyDiagnostics(String diagnostics) async {
    if (diagnostics.trim().isEmpty) {
      return;
    }
    await Clipboard.setData(ClipboardData(text: diagnostics));
    if (!mounted) {
      return;
    }
    ScaffoldMessenger.maybeOf(context)?.showSnackBar(
      SnackBar(
        content: Text(
          tr(
            'workspace.description.diagnosticsCopied',
            fallback: 'Diagnostics copied.',
          ),
        ),
        duration: const Duration(milliseconds: 1300),
      ),
    );
  }

  Widget _buildModeBody(
    _ConnectionPageMode mode,
    TerminalConnectionStatus status,
    SshConnectionProfile? profile,
    SerialConnectionProfile? serialProfile,
    List<TerminalConnectionEvent> events,
  ) {
    final body = switch (mode) {
      _ConnectionPageMode.username => _buildUsernameBody(),
      _ConnectionPageMode.hostKey => _HostKeyPrompt(
        key: const ValueKey('host-key'),
        profile: profile,
        fingerprint: _latestFingerprint(events),
      ),
      _ConnectionPageMode.authentication => _buildAuthenticationBody(),
      _ConnectionPageMode.passphrase => _buildPassphraseBody(),
      _ConnectionPageMode.failed => _ConnectionFailureBody(
        key: const ValueKey('failed'),
        message: status.message ?? 'Connection failed.',
      ),
      _ConnectionPageMode.exited => _ConnectionFailureBody(
        key: const ValueKey('exited'),
        message:
            status.message ??
            (serialProfile == null
                ? 'SSH session exited.'
                : 'Serial session closed.'),
        icon: Icons.logout_rounded,
        iconColor: _mutedText,
      ),
      _ConnectionPageMode.connecting => SizedBox(
        key: ValueKey('connecting'),
        height: 1,
      ),
      _ConnectionPageMode.completing => SizedBox(
        key: const ValueKey('completing'),
        height: 1,
      ),
      _ConnectionPageMode.connected => SizedBox(
        key: const ValueKey('connected'),
        height: 1,
      ),
    };
    if (mode == _ConnectionPageMode.authentication && !_requiresFido2Pin) {
      return body;
    }
    return SingleChildScrollView(
      key: const ValueKey('connection-mode-scrollable'),
      child: body,
    );
  }

  void _syncAuthenticationChallenge() {
    final challenge = widget.controller.authenticationChallenge;
    if (identical(challenge, _challenge)) return;
    _challenge = challenge;
    _challengeFormKey = GlobalKey();
  }

  void _submitAuthenticationChallenge() {
    _challengeFormKey.currentState?.submit();
  }

  Widget _buildAuthenticationBody() {
    if (_requiresFido2Pin) {
      return Column(
        key: const ValueKey('fido2-pin'),
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Unlock FIDO2 security key',
            style: TextStyle(
              color: _text,
              fontSize: 18,
              fontWeight: NautermFontWeights.bold,
            ),
          ),
          const SizedBox(height: 10),
          Text(
            'Enter the device PIN to continue SSH authentication.',
            style: TextStyle(color: _mutedText),
          ),
          const SizedBox(height: 20),
          _WorkspaceInput(
            controller: _fido2PinController,
            label: 'PIN',
            size: _WorkspaceControlSize.large,
            autofocus: true,
            obscureText: _obscureFido2Pin,
            onSubmitted: (_) {
              if (_fido2PinController.text.isNotEmpty) {
                _continueFido2Authentication();
              }
            },
            trailing: _WorkspaceButton(
              icon: _obscureFido2Pin
                  ? Icons.visibility_off_rounded
                  : Icons.visibility_rounded,
              size: widget.inDrawer
                  ? _WorkspaceControlSize.tiny
                  : _WorkspaceControlSize.medium,
              variant: _WorkspaceButtonVariant.text,
              height: widget.inDrawer
                  ? 24
                  : _WorkspaceControlSize.medium.inputHeight,
              minWidth: widget.inDrawer
                  ? 24
                  : _WorkspaceControlSize.medium.inputHeight,
              tooltip: _obscureFido2Pin ? 'Show PIN' : 'Hide PIN',
              onPressed: () {
                setState(() => _obscureFido2Pin = !_obscureFido2Pin);
              },
            ),
          ),
        ],
      );
    }
    final availableKeys = widget.keys;
    final selectedKeyIndex = _selectedKeyIndex;
    final selectedIndex =
        selectedKeyIndex != null &&
            selectedKeyIndex >= 0 &&
            selectedKeyIndex < availableKeys.length
        ? selectedKeyIndex
        : null;

    return Column(
      key: const ValueKey('auth'),
      mainAxisSize: MainAxisSize.min,
      children: [
        if (_showAuthTabs) ...[
          _AuthTabs(
            selected: _effectiveAuthTab,
            onChanged: (tab) => setState(() => _authTab = tab),
          ),
          SizedBox(height: 20),
        ] else if (_effectiveAuthTab == _ConnectionAuthTab.publicKey) ...[
          Align(
            alignment: Alignment.centerLeft,
            child: Text(
              tr('Public Key'),
              style: _connectionBodyStyle.copyWith(fontWeight: FontWeight.w500),
            ),
          ),
          SizedBox(height: 20),
        ],
        Flexible(
          child: SingleChildScrollView(
            key: const ValueKey('connection-auth-scrollable'),
            child: _effectiveAuthTab == _ConnectionAuthTab.password
                ? _WorkspaceInput(
                    controller: _passwordController,
                    label: 'Password',
                    size: _WorkspaceControlSize.large,
                    obscureText: _obscurePassword,
                    trailing: _WorkspaceButton(
                      icon: _obscurePassword
                          ? Icons.visibility_off_rounded
                          : Icons.visibility_rounded,
                      size: widget.inDrawer
                          ? _WorkspaceControlSize.tiny
                          : _WorkspaceControlSize.medium,
                      variant: _WorkspaceButtonVariant.text,
                      height: widget.inDrawer
                          ? 24
                          : _WorkspaceControlSize.medium.inputHeight,
                      minWidth: widget.inDrawer
                          ? 24
                          : _WorkspaceControlSize.medium.inputHeight,
                      tooltip: _obscurePassword
                          ? 'Show password'
                          : 'Hide password',
                      onPressed: () {
                        setState(() => _obscurePassword = !_obscurePassword);
                      },
                    ),
                  )
                : availableKeys.isEmpty
                ? const _EmptyKeyState()
                : Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      for (
                        var index = 0;
                        index < availableKeys.length;
                        index++
                      ) ...[
                        _ConnectionKeyCard(
                          keyOption: availableKeys[index],
                          selected: index == selectedIndex,
                          onTap: () =>
                              setState(() => _selectedKeyIndex = index),
                        ),
                        if (index != availableKeys.length - 1)
                          SizedBox(height: 14),
                      ],
                    ],
                  ),
          ),
        ),
      ],
    );
  }

  Widget _buildUsernameBody() {
    return Column(
      key: const ValueKey('connection-username'),
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          tr(
            'workspace.label.enterUsername',
            fallback: 'Please enter username',
          ),
          style: _connectionBodyStyle.copyWith(fontWeight: FontWeight.w500),
        ),
        const SizedBox(height: 20),
        _WorkspaceInput(
          controller: _usernameController,
          label: tr('common.label.username', fallback: 'Username'),
          size: _WorkspaceControlSize.large,
          autofocus: true,
          textInputAction: TextInputAction.go,
          onSubmitted: (_) =>
              _continueWithUsername(save: _saveAuthenticationChoice),
        ),
      ],
    );
  }

  Widget _buildPassphraseBody() {
    return Column(
      key: const ValueKey('private-key-passphrase'),
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          tr(
            'workspace.label.enterKeyPassphrase',
            fallback: 'Please Enter passphrase for key',
          ),
          style: _connectionBodyStyle.copyWith(fontWeight: FontWeight.w500),
        ),
        const SizedBox(height: 20),
        _WorkspaceInput(
          controller: _passphraseController,
          label: tr('common.label.passphrase', fallback: 'Passphrase'),
          size: _WorkspaceControlSize.large,
          autofocus: true,
          obscureText: _obscurePassphrase,
          onSubmitted: (_) => _continuePrivateKeyAuthentication(),
          trailing: _WorkspaceButton(
            icon: _obscurePassphrase
                ? Icons.visibility_off_rounded
                : Icons.visibility_rounded,
            size: widget.inDrawer
                ? _WorkspaceControlSize.tiny
                : _WorkspaceControlSize.medium,
            variant: _WorkspaceButtonVariant.text,
            height: widget.inDrawer
                ? 24
                : _WorkspaceControlSize.medium.inputHeight,
            minWidth: widget.inDrawer
                ? 24
                : _WorkspaceControlSize.medium.inputHeight,
            tooltip: _obscurePassphrase
                ? tr(
                    'workspace.keys.action.showPassphrase',
                    fallback: 'Show passphrase',
                  )
                : tr(
                    'workspace.keys.action.hidePassphrase',
                    fallback: 'Hide passphrase',
                  ),
            onPressed: () =>
                setState(() => _obscurePassphrase = !_obscurePassphrase),
          ),
        ),
      ],
    );
  }

  TerminalConnectionIdentityOption? get _selectedIdentity {
    final selectedIdentityId = _selectedIdentityId;
    if (selectedIdentityId == null) {
      return null;
    }

    for (final identity in widget.identities) {
      if (identity.id == selectedIdentityId) {
        return identity;
      }
    }

    return null;
  }

  Widget _buildFooter(
    _ConnectionPageMode mode,
    TerminalConnectionStatus status,
  ) {
    final selectedKeyIndex = _selectedKeyIndex;
    final hasSelectedKey =
        selectedKeyIndex != null &&
        selectedKeyIndex >= 0 &&
        selectedKeyIndex < widget.keys.length;
    final hasSshProfile = widget.controller.sshProfile != null;
    final hasSavedSshHost = widget.controller.sshProfile?.hostId != null;
    final requiresFido2Pin = _requiresFido2Pin;
    final canContinue = mode == _ConnectionPageMode.username
        ? _usernameController.text.trim().isNotEmpty
        : mode == _ConnectionPageMode.passphrase
        ? _passphraseController.text.isNotEmpty
        : mode == _ConnectionPageMode.authentication
        ? requiresFido2Pin
              ? _fido2PinController.text.isNotEmpty
              : (_effectiveAuthTab == _ConnectionAuthTab.password
                    ? _passwordController.text.isNotEmpty
                    : hasSelectedKey)
        : true;

    final secondaryActions = <Widget>[
      _ConnectionButton(
        label: tr('common.action.close', fallback: 'Close'),
        onPressed: _closePage,
      ),
      if (mode == _ConnectionPageMode.authentication &&
          !requiresFido2Pin &&
          _challenge == null &&
          _effectiveAuthTab == _ConnectionAuthTab.publicKey) ...[
        SizedBox(width: 10),
        _ConnectionButton(
          label: tr('workspace.label.addKey', fallback: 'Add key'),
          onPressed: widget.onAddKeyRequested,
        ),
      ],
      if (mode == _ConnectionPageMode.failed &&
          widget.onEditHostRequested != null) ...[
        SizedBox(width: 10),
        _ConnectionButton(
          label: tr('workspace.label.editHost', fallback: 'Edit Host'),
          onPressed: widget.onEditHostRequested,
        ),
      ],
    ];
    final primaryActions = <Widget>[
      if (mode == _ConnectionPageMode.username) ...[
        _ConnectionButton(
          label: _saveAuthenticationChoice
              ? tr('workspace.label.continueSave', fallback: 'Continue & Save')
              : tr('common.action.continue', fallback: 'Continue'),
          primary: true,
          shape: hasSavedSshHost
              ? const RoundedRectangleBorder(
                  borderRadius: BorderRadius.horizontal(
                    left: Radius.circular(8),
                  ),
                )
              : null,
          onPressed: canContinue
              ? () => _continueWithUsername(save: _saveAuthenticationChoice)
              : null,
        ),
        if (hasSavedSshHost) ...[
          SizedBox(width: 2),
          _AuthenticationContinueMenuButton(
            enabled: canContinue,
            saveIsSelected: _saveAuthenticationChoice,
            onSelected: (save) => _continueWithUsername(save: save),
          ),
        ],
      ] else if (mode == _ConnectionPageMode.hostKey) ...[
        _ConnectionButton(
          label: tr('common.action.continue', fallback: 'Continue'),
          onPressed: _trustHostKeyForSession,
        ),
        SizedBox(width: 10),
        _ConnectionButton(
          label: tr(
            'common.label.addAndContinue',
            fallback: 'Add and continue',
          ),
          primary: true,
          onPressed: _trustHostKey,
        ),
      ] else if (mode == _ConnectionPageMode.failed && hasSshProfile) ...[
        _ConnectionButton(
          label: tr('workspace.label.startOver', fallback: 'Start over'),
          primary: true,
          onPressed: () => _reconnect(resetProgress: true),
        ),
      ] else if ((mode == _ConnectionPageMode.failed ||
              mode == _ConnectionPageMode.exited) &&
          widget.controller.serialProfile != null) ...[
        _ConnectionButton(
          label: tr('workspace.label.startOver', fallback: 'Start over'),
          primary: true,
          onPressed: () {
            final host = _resolvedHost;
            widget.controller.reconnectSerial(
              serialConfig: host?.serialConnectionConfig,
            );
          },
        ),
      ] else if (mode == _ConnectionPageMode.passphrase) ...[
        _ConnectionButton(
          label: tr('common.action.continue', fallback: 'Continue'),
          primary: true,
          onPressed: canContinue ? _continuePrivateKeyAuthentication : null,
        ),
      ] else if (mode == _ConnectionPageMode.authentication &&
          _challenge != null) ...[
        _ConnectionButton(
          label: tr('common.action.continue', fallback: 'Continue'),
          primary: true,
          onPressed: _submitAuthenticationChallenge,
        ),
      ] else if (mode == _ConnectionPageMode.authentication &&
          requiresFido2Pin) ...[
        _ConnectionButton(
          label: tr('common.action.continue', fallback: 'Continue'),
          primary: true,
          onPressed: canContinue ? _continueFido2Authentication : null,
        ),
      ] else if (mode == _ConnectionPageMode.authentication) ...[
        _ConnectionButton(
          label: _saveAuthenticationChoice
              ? tr('workspace.label.continueSave', fallback: 'Continue & Save')
              : tr('common.action.continue', fallback: 'Continue'),
          primary: true,
          shape: hasSavedSshHost
              ? const RoundedRectangleBorder(
                  borderRadius: BorderRadius.horizontal(
                    left: Radius.circular(8),
                  ),
                )
              : null,
          onPressed: canContinue
              ? () => _continueAuthentication(save: _saveAuthenticationChoice)
              : null,
        ),
        if (hasSavedSshHost) ...[
          SizedBox(width: 2),
          _AuthenticationContinueMenuButton(
            enabled: canContinue,
            saveIsSelected: _saveAuthenticationChoice,
            onSelected: (save) => _continueAuthentication(save: save),
          ),
        ],
      ] else if (_showLogs &&
          status.phase == TerminalConnectionPhase.connecting &&
          hasSshProfile) ...[
        _ConnectionButton(
          label: tr('workspace.label.startOver', fallback: 'Start over'),
          primary: true,
          onPressed: () => _reconnect(resetProgress: true),
        ),
      ],
    ];
    return OverflowBar(
      alignment: MainAxisAlignment.spaceBetween,
      spacing: 8,
      overflowSpacing: 8,
      overflowAlignment: OverflowBarAlignment.end,
      children: [
        Wrap(runSpacing: 8, children: secondaryActions),
        if (primaryActions.isNotEmpty)
          Wrap(
            alignment: WrapAlignment.end,
            runSpacing: 8,
            children: primaryActions,
          ),
      ],
    );
  }

  void _trustHostKey() {
    _sessionHostKeyTrustMode = SshHostKeyTrustMode.strict;
    _reconnect(hostKeyTrustMode: SshHostKeyTrustMode.acceptAndSave);
  }

  void _trustHostKeyForSession() {
    _sessionHostKeyTrustMode = SshHostKeyTrustMode.acceptOnce;
    _reconnect(hostKeyTrustMode: SshHostKeyTrustMode.acceptOnce);
  }

  void _syncControllers(
    SshConnectionProfile? profile,
    TerminalConnectionPhase phase,
  ) {
    if (profile == null || (_lastProfile == profile && _lastPhase == phase)) {
      return;
    }

    _lastProfile = profile;
    _lastPhase = phase;
    _syncingControllers = true;
    _hostController.text = profile.host;
    _portController.text = profile.port.toString();
    _usernameController.text = profile.username;
    _passwordController.text = profile.password ?? '';
    _syncingControllers = false;
    _selectedIdentityId = profile.identityId;
  }

  void _closePage() {
    widget.controller.cancelAuthenticationRequest();
    final challenge = widget.controller.authenticationChallenge;
    if (challenge != null) {
      widget.controller.respondToAuthentication(challenge.id, null);
    }
    final onCloseRequested = widget.onCloseRequested;
    if (onCloseRequested != null) {
      onCloseRequested();
      return;
    }
    widget.controller.dismissConnectionStatus();
  }

  Future<void> _continueAuthentication({required bool save}) async {
    if (_saveAuthenticationChoice != save) {
      setState(() => _saveAuthenticationChoice = save);
    }
    final profile = widget.controller.sshProfile;
    if (profile == null) {
      return;
    }

    final requestId = widget.controller.authenticationRequestId;
    if (_effectiveAuthTab == _ConnectionAuthTab.publicKey) {
      final availableKeys = widget.keys;
      final selectedKeyIndex = _selectedKeyIndex;
      if (selectedKeyIndex == null ||
          selectedKeyIndex < 0 ||
          selectedKeyIndex >= availableKeys.length) {
        return;
      }
      final selectedKey = availableKeys[selectedKeyIndex];
      final keyDetail = widget.dataStore?.getKey(selectedKey.id);
      final privateKey =
          (keyDetail?.privateKey ??
                  (widget.dataStore == null ? selectedKey.privateKey : null))
              ?.trim();
      if (privateKey == null || privateKey.isEmpty) {
        return;
      }
      final certificate = _emptyToNull(
        keyDetail?.certificate ?? selectedKey.certificate,
      );
      final resolvedKey = TerminalConnectionKeyOption(
        id: selectedKey.id,
        name: selectedKey.name,
        subtitle: selectedKey.subtitle,
        privateKey: privateKey,
        certificate: certificate,
      );
      if (save) {
        await widget.onSaveAuth?.call(profile, key: resolvedKey);
      }
      if (requestId != null) {
        widget.controller.respondToAuthenticationCredentials(
          requestId,
          SshAuthenticationCredentials.publicKey(
            privateKey: privateKey,
            certificate: certificate,
            passphrase: keyDetail?.passphrase,
          ),
        );
        return;
      }
      _reconnect(
        privateKey: privateKey,
        certificate: certificate,
        passphrase: keyDetail?.passphrase,
        password: null,
      );
      return;
    }

    if (_passwordController.text.isEmpty) {
      return;
    }
    if (save) {
      await widget.onSaveAuth?.call(
        profile,
        password: _passwordController.text,
      );
    }
    if (requestId != null) {
      widget.controller.respondToAuthenticationCredentials(
        requestId,
        SshAuthenticationCredentials.password(_passwordController.text),
      );
      return;
    }
    _reconnect(
      password: _passwordController.text,
      privateKey: null,
      certificate: null,
    );
  }

  void _continueFido2Authentication() {
    final pin = _fido2PinController.text;
    if (pin.isEmpty) {
      return;
    }
    final requestId = widget.controller.authenticationRequestId;
    final profile = widget.controller.sshProfile;
    if (requestId != null && profile?.privateKey != null) {
      widget.controller.respondToAuthenticationCredentials(
        requestId,
        SshAuthenticationCredentials.publicKey(
          privateKey: profile!.privateKey,
          certificate: profile.certificate,
          passphrase:
              'nauterm-auth-secrets:v1:${jsonEncode({'keyPassphrase': profile.passphrase, 'fido2Pin': pin})}',
        ),
        fido2Pin: pin,
      );
    } else {
      _reconnect(fido2Pin: pin, password: null);
    }
    _fido2PinController.clear();
  }

  Future<void> _continueWithUsername({required bool save}) async {
    if (_saveAuthenticationChoice != save) {
      setState(() => _saveAuthenticationChoice = save);
    }
    final profile = widget.controller.sshProfile;
    final username = _usernameController.text.trim();
    if (profile == null || username.isEmpty) return;
    if (save) {
      await widget.onSaveAuth?.call(profile, username: username);
    }
    _sessionUsername = (
      host: profile.host,
      port: profile.port,
      identityId: profile.identityId,
      value: username,
    );
    _reconnect(username: username);
  }

  void _continuePrivateKeyAuthentication() {
    final profile = widget.controller.sshProfile;
    final passphrase = _passphraseController.text;
    final privateKey = profile?.privateKey;
    if (profile == null || privateKey == null || passphrase.isEmpty) return;
    _sessionKeyUnlock = (privateKey: privateKey, passphrase: passphrase);
    final requestId = widget.controller.authenticationRequestId;
    if (requestId != null) {
      widget.controller.respondToAuthenticationCredentials(
        requestId,
        SshAuthenticationCredentials.publicKey(
          privateKey: privateKey,
          certificate: profile.certificate,
          passphrase: passphrase,
        ),
      );
      _passphraseController.clear();
      return;
    }
    _reconnect(
      privateKey: privateKey,
      certificate: profile.certificate,
      passphrase: passphrase,
    );
    _passphraseController.clear();
  }

  void _reconnect({
    bool resetProgress = false,
    String? username,
    Object? password = _preserveReconnectValue,
    Object? privateKey = _preserveReconnectValue,
    Object? certificate = _preserveReconnectValue,
    Object? passphrase = _preserveReconnectValue,
    Object? fido2Pin = _preserveReconnectValue,
    SshHostKeyTrustMode? hostKeyTrustMode,
  }) {
    final reloader = widget.onReloadConnection;
    final reloaded = reloader?.call();
    if (reloader != null && reloaded == null) {
      return;
    }
    var profile = reloaded?.profile ?? widget.controller.sshProfile;
    if (profile == null) {
      return;
    }
    final sessionUsername = _sessionUsername;
    if (sessionUsername != null) {
      if (profile.host == sessionUsername.host &&
          profile.port == sessionUsername.port &&
          profile.identityId == sessionUsername.identityId) {
        if (profile.username.trim().isEmpty) {
          profile = profile.copyWith(username: sessionUsername.value);
        }
      } else {
        _sessionUsername = null;
      }
    }
    _syncControllers(profile, widget.controller.connectionStatus.phase);
    if (username != null) _usernameController.text = username;
    final port = int.tryParse(_portController.text.trim());
    if (port == null || port < 1 || port > 65535) {
      return;
    }

    setState(() {
      _showLogs = false;
    });

    final identity = _selectedIdentity;
    var nextPassword = password;
    var nextPrivateKey = privateKey;
    var nextCertificate = certificate;
    var nextPassphrase = passphrase;
    var nextFido2Pin = fido2Pin;
    if (identity != null &&
        identical(password, _preserveReconnectValue) &&
        identical(privateKey, _preserveReconnectValue)) {
      final detail = widget.dataStore?.getIdentity(identity.id);
      final identityPassword = detail?.password?.trim();
      final identityKeyEntry = detail?.keyId == null
          ? null
          : widget.dataStore?.getKey(detail!.keyId!);
      final identityKey = identityKeyEntry?.privateKey?.trim();
      final identityCertificate = identityKeyEntry?.certificate?.trim();
      final identityPassphrase = identityKeyEntry?.passphrase;
      if (identityKey != null && identityKey.isNotEmpty) {
        nextPrivateKey = identityKey;
        nextCertificate = identityCertificate?.isEmpty == true
            ? null
            : identityCertificate;
        nextPassword = null;
        nextPassphrase = identityPassphrase?.isEmpty == true
            ? null
            : identityPassphrase;
        nextFido2Pin = null;
      } else if (identityPassword != null && identityPassword.isNotEmpty) {
        nextPassword = identityPassword;
        nextPrivateKey = null;
        nextCertificate = null;
        nextPassphrase = null;
        nextFido2Pin = null;
      }
    }

    final effectivePrivateKey =
        identical(nextPrivateKey, _preserveReconnectValue)
        ? profile.privateKey
        : nextPrivateKey;
    final sessionKeyUnlock = _sessionKeyUnlock;
    if (sessionKeyUnlock != null) {
      if (effectivePrivateKey == sessionKeyUnlock.privateKey) {
        if (identical(passphrase, _preserveReconnectValue)) {
          nextPassphrase = sessionKeyUnlock.passphrase;
        }
      } else {
        _sessionKeyUnlock = null;
      }
    }

    if (resetProgress) {
      _interventions.clear();
      _lastProgressPage = null;
    }
    widget.controller.reconnectSsh(
      profile: profile,
      host: _hostController.text.trim(),
      port: port,
      username: _usernameController.text.trim(),
      identityId: _selectedIdentityId,
      password: nextPassword,
      privateKey: nextPrivateKey,
      certificate: nextCertificate,
      passphrase: nextPassphrase,
      fido2Pin: nextFido2Pin,
      moshServerCommand: reloaded?.moshServerCommand,
      hostKeyTrustMode: hostKeyTrustMode ?? _sessionHostKeyTrustMode,
    );
  }
}

const Object _preserveReconnectValue = Object();

class _ConnectionPageHeader extends StatelessWidget {
  const _ConnectionPageHeader({
    required this.title,
    required this.subtitle,
    this.actionLabel,
    this.onAction,
    this.hostOs,
    this.hostDistro,
  });

  final String title;
  final String subtitle;
  final String? actionLabel;
  final VoidCallback? onAction;
  final String? hostOs;
  final String? hostDistro;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        _BrandIcon(
          icon: Icons.dns_rounded,
          color: const Color(0xffff552f),
          name: title,
          os: hostOs,
          distro: hostDistro,
        ),
        SizedBox(width: 16),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                tr(title),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: _text,
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  letterSpacing: 0,
                ),
              ),
              SizedBox(height: 2),
              Text(
                tr(subtitle),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: _mutedText,
                  fontSize: 11,
                  fontWeight: FontWeight.w500,
                  letterSpacing: 0,
                ),
              ),
            ],
          ),
        ),
        if (onAction != null && actionLabel != null)
          _ConnectionButton(label: actionLabel!, onPressed: onAction),
      ],
    );
  }
}

class _KeyboardInteractiveForm extends StatefulWidget {
  const _KeyboardInteractiveForm({
    super.key,
    required this.challenge,
    required this.onSubmit,
    this.logs,
  });

  final SshAuthenticationChallenge challenge;
  final ValueChanged<List<String>> onSubmit;
  final Widget? logs;

  @override
  State<_KeyboardInteractiveForm> createState() =>
      _KeyboardInteractiveFormState();
}

class _KeyboardInteractiveFormState extends State<_KeyboardInteractiveForm> {
  late final List<TextEditingController> _controllers = [
    for (final _ in widget.challenge.prompts) TextEditingController(),
  ];

  @override
  void dispose() {
    for (final controller in _controllers) {
      controller.dispose();
    }
    super.dispose();
  }

  void submit() {
    final responses = [for (final controller in _controllers) controller.text];
    for (final controller in _controllers) {
      controller.clear();
    }
    widget.onSubmit(responses);
  }

  @override
  Widget build(BuildContext context) {
    final challenge = widget.challenge;
    if (widget.logs != null) return widget.logs!;
    return SingleChildScrollView(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (challenge.name.isNotEmpty)
            Text(
              challenge.name,
              style: TextStyle(
                color: _text,
                fontSize: 16,
                fontWeight: NautermFontWeights.bold,
              ),
            ),
          if (challenge.instructions.isNotEmpty) ...[
            if (challenge.name.isNotEmpty) const SizedBox(height: 10),
            Text(challenge.instructions, style: _connectionBodyStyle),
          ],
          for (var index = 0; index < challenge.prompts.length; index++) ...[
            if (index > 0 ||
                challenge.name.isNotEmpty ||
                challenge.instructions.isNotEmpty)
              const SizedBox(height: 16),
            _WorkspaceInput(
              controller: _controllers[index],
              label: challenge.prompts[index].prompt,
              size: _WorkspaceControlSize.large,
              autofocus: index == 0,
              obscureText: !challenge.prompts[index].echo,
              textInputAction: index == challenge.prompts.length - 1
                  ? TextInputAction.go
                  : TextInputAction.next,
              onSubmitted: (_) {
                if (index == challenge.prompts.length - 1) {
                  submit();
                } else {
                  FocusScope.of(context).nextFocus();
                }
              },
            ),
          ],
        ],
      ),
    );
  }
}

class _ConnectionPageViewport extends StatelessWidget {
  const _ConnectionPageViewport({required this.child, this.inDrawer = false});

  final Widget child;
  final bool inDrawer;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        const compactHeight = 420.0;
        final short = constraints.maxHeight < compactHeight;
        final compact = inDrawer || short;
        final topPadding = short
            ? 16.0
            : math.min(
                inDrawer ? 72.0 : 80.0,
                24.0 + constraints.maxHeight - compactHeight,
              );
        return _ConnectionPageLayoutScope(
          short: short,
          child: _WorkspaceControlSizeScope(
            size: compact
                ? _WorkspaceControlSize.tiny
                : _WorkspaceControlSize.small,
            child: ColoredBox(
              color: _surface,
              child: Padding(
                padding: EdgeInsets.fromLTRB(
                  compact ? 16 : 24,
                  topPadding,
                  compact ? 16 : 24,
                  compact ? 16 : 24,
                ),
                child: Align(alignment: Alignment.topCenter, child: child),
              ),
            ),
          ),
        );
      },
    );
  }
}

class _ConnectionPageLayoutScope extends InheritedWidget {
  const _ConnectionPageLayoutScope({required this.short, required super.child});

  final bool short;

  static bool isShort(BuildContext context) =>
      context
          .dependOnInheritedWidgetOfExactType<_ConnectionPageLayoutScope>()
          ?.short ??
      false;

  @override
  bool updateShouldNotify(_ConnectionPageLayoutScope oldWidget) =>
      short != oldWidget.short;
}

class _ConnectionPagePanel extends StatelessWidget {
  const _ConnectionPagePanel({
    required this.title,
    required this.subtitle,
    required this.progress,
    required this.body,
    required this.footer,
    this.actionLabel,
    this.onAction,
    this.hostOs,
    this.hostDistro,
  });

  final String title;
  final String subtitle;
  final Widget progress;
  final Widget body;
  final Widget footer;
  final String? actionLabel;
  final VoidCallback? onAction;
  final String? hostOs;
  final String? hostDistro;

  @override
  Widget build(BuildContext context) {
    final short = _ConnectionPageLayoutScope.isShort(context);
    final compact =
        _WorkspaceControlSizeScope.maybeOf(context) ==
        _WorkspaceControlSize.tiny;
    return Material(
      color: Colors.transparent,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 510),
        child: Padding(
          padding: EdgeInsets.symmetric(horizontal: compact ? 0 : 22),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              KeyedSubtree(
                key: const ValueKey('connection-page-header'),
                child: _ConnectionPageHeader(
                  title: title,
                  subtitle: subtitle,
                  actionLabel: actionLabel,
                  onAction: onAction,
                  hostOs: hostOs,
                  hostDistro: hostDistro,
                ),
              ),
              SizedBox(height: short ? 12 : 22),
              KeyedSubtree(
                key: const ValueKey('connection-page-progress'),
                child: progress,
              ),
              SizedBox(height: short ? 12 : 28),
              Flexible(
                fit: FlexFit.loose,
                child: AnimatedSwitcher(
                  duration: const Duration(milliseconds: 140),
                  switchInCurve: Curves.easeOutCubic,
                  switchOutCurve: Curves.easeOutCubic,
                  child: body,
                ),
              ),
              SizedBox(height: short ? 12 : 26),
              KeyedSubtree(
                key: const ValueKey('connection-page-footer'),
                child: footer,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ConnectionProgress extends StatelessWidget {
  const _ConnectionProgress({
    required this.icons,
    required this.reachedStep,
    this.spinningStep,
    this.movingLine,
    this.travelingStep,
    this.failed = false,
    this.labels = const [],
  });

  factory _ConnectionProgress.forConnection({
    required _ConnectionPageMode mode,
    required List<_ConnectionIntervention> interventions,
    required IconData protocolIcon,
    IconData destinationIcon = LucideIcons.squareTerminal,
  }) {
    final icons = <IconData>[
      protocolIcon,
      for (final intervention in interventions)
        switch (intervention) {
          _ConnectionIntervention.username => Icons.person_outline_rounded,
          _ConnectionIntervention.hostKey => Icons.fingerprint_rounded,
          _ConnectionIntervention.passphrase => Icons.lock_open_rounded,
          _ConnectionIntervention.securityKeyPin => Icons.pin_rounded,
          _ConnectionIntervention.password => Icons.lock_outline_rounded,
          _ConnectionIntervention.publicKey => Icons.key_rounded,
          _ConnectionIntervention.verificationCode => Icons.password_rounded,
        },
      destinationIcon,
    ];
    final labels = [
      for (final stage in interventions)
        switch (stage) {
          _ConnectionIntervention.username => tr(
            'common.label.username',
            fallback: 'Username',
          ),
          _ConnectionIntervention.hostKey => tr(
            'common.label.hostKey',
            fallback: 'Host key',
          ),
          _ConnectionIntervention.passphrase => tr(
            'common.label.passphrase',
            fallback: 'Passphrase',
          ),
          _ConnectionIntervention.securityKeyPin => 'PIN',
          _ConnectionIntervention.password => tr(
            'common.label.password',
            fallback: 'Password',
          ),
          _ConnectionIntervention.publicKey => tr(
            'workspace.label.publicKey',
            fallback: 'Public Key',
          ),
          _ConnectionIntervention.verificationCode => tr(
            'common.label.verificationCode',
            fallback: 'Verification code',
          ),
        },
    ];
    final last = icons.length - 1;
    final lastInterventionStep = interventions.length;
    return switch (mode) {
      _ConnectionPageMode.connecting => _ConnectionProgress(
        icons: icons,
        labels: labels,
        reachedStep: lastInterventionStep,
        spinningStep: lastInterventionStep,
      ),
      _ConnectionPageMode.completing => _ConnectionProgress(
        icons: icons,
        labels: labels,
        reachedStep: lastInterventionStep,
        movingLine: lastInterventionStep,
        travelingStep: last,
      ),
      _ConnectionPageMode.username ||
      _ConnectionPageMode.hostKey ||
      _ConnectionPageMode.passphrase ||
      _ConnectionPageMode.authentication => _ConnectionProgress(
        icons: icons,
        labels: labels,
        reachedStep: lastInterventionStep,
        spinningStep: lastInterventionStep,
      ),
      _ConnectionPageMode.connected => _ConnectionProgress(
        icons: icons,
        labels: labels,
        reachedStep: last,
        spinningStep: last,
      ),
      _ConnectionPageMode.failed ||
      _ConnectionPageMode.exited => _ConnectionProgress(
        icons: icons,
        labels: labels,
        reachedStep: lastInterventionStep,
        failed: true,
      ),
    };
  }

  final List<IconData> icons;
  final int reachedStep;
  final int? spinningStep;
  final int? movingLine;
  final int? travelingStep;
  final bool failed;
  final List<String> labels;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      label: labels.isEmpty ? null : labels.join(' → '),
      child: _ConnectionPipeline(
        icons: icons,
        reachedStep: reachedStep,
        spinningStep: spinningStep,
        movingLine: movingLine,
        travelingStep: travelingStep,
        failed: failed,
      ),
    );
  }
}

class _ConnectionStepDot extends StatelessWidget {
  const _ConnectionStepDot({
    required this.icon,
    required this.active,
    required this.spinning,
    this.visible = true,
    this.colorOverride,
  });

  final IconData icon;
  final bool active;
  final bool spinning;
  final bool visible;
  final Color? colorOverride;

  @override
  Widget build(BuildContext context) {
    final color = colorOverride ?? (active ? _blue : _mutedText);
    return SizedBox(
      width: 36,
      height: 36,
      child: visible
          ? Stack(
              alignment: Alignment.center,
              children: [
                if (spinning)
                  SizedBox(
                    width: 36,
                    height: 36,
                    child: CircularProgressIndicator(
                      strokeWidth: 3,
                      valueColor: AlwaysStoppedAnimation<Color>(color),
                    ),
                  ),
                Container(
                  width: 24,
                  height: 24,
                  decoration: BoxDecoration(
                    color: color,
                    shape: BoxShape.circle,
                  ),
                  child: Icon(icon, size: 14, color: Colors.white),
                ),
              ],
            )
          : null,
    );
  }
}

class _ConnectionPipeline extends StatefulWidget {
  const _ConnectionPipeline({
    required this.icons,
    required this.reachedStep,
    required this.spinningStep,
    required this.movingLine,
    required this.travelingStep,
    required this.failed,
  });

  final List<IconData> icons;
  final int reachedStep;
  final int? spinningStep;
  final int? movingLine;
  final int? travelingStep;
  final bool failed;

  @override
  State<_ConnectionPipeline> createState() => _ConnectionPipelineState();
}

class _ConnectionPipelineState extends State<_ConnectionPipeline>
    with TickerProviderStateMixin {
  static const Curve _stageCurve = Cubic(0.55, 0, 0.30, 1);

  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: _TerminalConnectionPageState._pipeAnimationDuration,
  );
  late final AnimationController _layoutController = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 320),
    value: 1,
  );
  late List<IconData> _previousIcons = List<IconData>.of(widget.icons);

  @override
  void initState() {
    super.initState();
    if (widget.movingLine != null) _controller.forward();
  }

  @override
  void didUpdateWidget(covariant _ConnectionPipeline oldWidget) {
    super.didUpdateWidget(oldWidget);
    final iconsChanged = !_sameIcons(widget.icons, oldWidget.icons);
    if (iconsChanged) {
      _previousIcons = List<IconData>.of(oldWidget.icons);
      _controller
        ..stop()
        ..value = widget.movingLine == null ? 0 : 1;
      _layoutController.forward(from: 0);
      return;
    }
    final transitionChanged =
        widget.movingLine != oldWidget.movingLine ||
        widget.travelingStep != oldWidget.travelingStep ||
        widget.icons.length != oldWidget.icons.length;
    if (!transitionChanged) return;
    if (_layoutController.isAnimating) {
      _controller.stop();
      return;
    }
    if (widget.movingLine != null) {
      _controller.forward(from: 0);
    } else {
      _controller.stop();
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    _layoutController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: Listenable.merge([_controller, _layoutController]),
      builder: (context, _) => LayoutBuilder(
        builder: (context, constraints) {
          final availableWidth = math.max(0.0, constraints.maxWidth - 36);
          final layoutProgress = _stageCurve.transform(_layoutController.value);
          final travelProgress = _stageCurve.transform(_controller.value);
          double finalPosition(int index) {
            final divisor = math.max(1, widget.icons.length - 1);
            return availableWidth * index / divisor;
          }

          double nodePosition(int index) {
            final destination = finalPosition(index);
            if (_layoutController.isCompleted) return destination;
            final originIndex = index == widget.icons.length - 1
                ? _previousIcons.length - 1
                : math.min(index, math.max(0, _previousIcons.length - 2));
            final oldDivisor = math.max(1, _previousIcons.length - 1);
            final origin = availableWidth * originIndex / oldDivisor;
            return origin + (destination - origin) * layoutProgress;
          }

          final movingLine = widget.movingLine;
          final progressPosition = widget.failed
              ? availableWidth
              : movingLine == null
              ? nodePosition(
                  widget.reachedStep.clamp(0, widget.icons.length - 1).toInt(),
                )
              : nodePosition(movingLine) +
                    (nodePosition(widget.travelingStep ?? movingLine) -
                            nodePosition(movingLine)) *
                        travelProgress;
          final progress = availableWidth == 0
              ? 0.0
              : progressPosition / availableWidth;
          final progressColor = widget.failed ? const Color(0xffef4444) : _blue;
          final travelingStep = widget.travelingStep;
          return SizedBox(
            height: 36,
            child: Stack(
              clipBehavior: Clip.none,
              children: [
                Positioned(
                  left: 18,
                  right: 18,
                  top: 16,
                  height: 4,
                  child: Stack(
                    children: [
                      Positioned.fill(
                        child: ColoredBox(color: _sidebarDivider),
                      ),
                      Positioned.fill(
                        child: FractionallySizedBox(
                          alignment: Alignment.centerLeft,
                          widthFactor: progress.clamp(0.0, 1.0),
                          child: ColoredBox(color: progressColor),
                        ),
                      ),
                    ],
                  ),
                ),
                for (var index = 0; index < widget.icons.length; index++)
                  Positioned(
                    left: nodePosition(index),
                    top: 0,
                    child: _ConnectionStepDot(
                      icon: widget.icons[index],
                      active:
                          widget.failed ||
                          index <= widget.reachedStep ||
                          index == widget.spinningStep ||
                          (index == travelingStep &&
                              !_layoutController.isCompleted),
                      spinning:
                          index == widget.spinningStep ||
                          (index == travelingStep &&
                              !_layoutController.isCompleted),
                      visible:
                          index != travelingStep ||
                          !_layoutController.isCompleted,
                      colorOverride: widget.failed ? progressColor : null,
                    ),
                  ),
                if (travelingStep != null && _layoutController.isCompleted)
                  Positioned(
                    left: progress.clamp(0.0, 1.0) * availableWidth,
                    top: 0,
                    child: _ConnectionStepDot(
                      icon: widget.icons[travelingStep],
                      active: true,
                      spinning: true,
                    ),
                  ),
              ],
            ),
          );
        },
      ),
    );
  }

  bool _sameIcons(List<IconData> left, List<IconData> right) {
    if (left.length != right.length) return false;
    for (var index = 0; index < left.length; index++) {
      if (left[index] != right[index]) return false;
    }
    return true;
  }
}

class _ConnectionFailureDetails extends StatelessWidget {
  const _ConnectionFailureDetails({
    super.key,
    required this.logs,
    required this.diagnostics,
    required this.onCopyDiagnostics,
  });

  final List<_ConnectionLogEntry> logs;
  final String diagnostics;
  final ValueChanged<String> onCopyDiagnostics;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          tr(
            'workspace.label.connectionFailedWithConnectionLog',
            fallback: 'Connection failed with connection log:',
          ),
          maxLines: _ConnectionPageLayoutScope.isShort(context) ? 1 : null,
          overflow: TextOverflow.ellipsis,
          style: TextStyle(
            color: const Color(0xffef4444),
            fontSize: 14,
            fontWeight: FontWeight.w700,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 16),
        Flexible(
          fit: FlexFit.loose,
          child: _ConnectionLogsCard(
            logs: logs,
            diagnostics: diagnostics,
            onCopyDiagnostics: onCopyDiagnostics,
          ),
        ),
      ],
    );
  }
}

class _ConnectionLogsCopyButton extends StatelessWidget {
  const _ConnectionLogsCopyButton({required this.onPressed});

  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final small =
        _WorkspaceControlSizeScope.maybeOf(context) ==
        _WorkspaceControlSize.tiny;
    return Tooltip(
      message: tr(
        'workspace.label.copyDiagnostics',
        fallback: 'Copy diagnostics',
      ),
      child: IconButton(
        key: const ValueKey('connection-logs-copy'),
        onPressed: onPressed,
        icon: Icon(Icons.content_copy_rounded),
        color: _blue,
        iconSize: small ? 16 : 18,
        splashRadius: 20,
        padding: EdgeInsets.zero,
        style: IconButton.styleFrom(
          tapTargetSize: small ? MaterialTapTargetSize.shrinkWrap : null,
          hoverColor: _workspaceDark ? _blue.withValues(alpha: 0.12) : null,
          highlightColor: _workspaceDark ? _blue.withValues(alpha: 0.16) : null,
        ),
        constraints: BoxConstraints.tightFor(
          width: small ? 24 : 34,
          height: small ? 24 : 34,
        ),
      ),
    );
  }
}

class _ConnectionLogsCard extends StatelessWidget {
  const _ConnectionLogsCard({
    super.key,
    required this.logs,
    required this.diagnostics,
    required this.onCopyDiagnostics,
  });

  final List<_ConnectionLogEntry> logs;
  final String diagnostics;
  final ValueChanged<String> onCopyDiagnostics;

  @override
  Widget build(BuildContext context) {
    final compact =
        _WorkspaceControlSizeScope.maybeOf(context) ==
        _WorkspaceControlSize.tiny;
    return Container(
      key: const ValueKey('connection-logs-card'),
      constraints: const BoxConstraints(maxHeight: 280),
      padding: compact
          ? const EdgeInsets.all(12)
          : const EdgeInsets.fromLTRB(18, 16, 18, 16),
      decoration: BoxDecoration(
        color: _card.withValues(alpha: _workspaceDark ? 0.86 : 0.92),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  tr(
                    'workspace.label.connectionDiagnostics',
                    fallback: 'Connection diagnostics',
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: _text,
                    fontSize: 13,
                    fontWeight: FontWeight.w700,
                    letterSpacing: 0,
                  ),
                ),
              ),
              _ConnectionLogsCopyButton(
                onPressed: () => onCopyDiagnostics(diagnostics),
              ),
            ],
          ),
          SizedBox(height: 8),
          Expanded(
            child: logs.isEmpty
                ? Align(
                    alignment: Alignment.topLeft,
                    child: Text(
                      tr(
                        'workspace.description.waitingForConnectionEvents',
                        fallback: 'Waiting for connection events.',
                      ),
                      style: TextStyle(
                        color: _mutedText,
                        fontSize: 13,
                        height: 1.35,
                        letterSpacing: 0,
                      ),
                    ),
                  )
                : SelectionArea(
                    child: SingleChildScrollView(
                      key: const ValueKey('connection-logs-scrollable'),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          for (var index = 0; index < logs.length; index++)
                            _ConnectionLogRow(
                              log: logs[index],
                              prependLineBreak: index > 0,
                            ),
                        ],
                      ),
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}

class _ConnectionLogRow extends StatelessWidget {
  const _ConnectionLogRow({required this.log, required this.prependLineBreak});

  final _ConnectionLogEntry log;
  final bool prependLineBreak;

  @override
  Widget build(BuildContext context) {
    final color = _logSeverityColor(log.severity);
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 68,
            child: Text.rich(
              TextSpan(
                children: [
                  if (prependLineBreak)
                    TextSpan(
                      text: '\n',
                      style: TextStyle(fontSize: 0.1, height: 0.1),
                    ),
                  TextSpan(
                    text:
                        '${log.timestamp == null ? '--:--:--' : _formatConnectionLogTime(log.timestamp!)}  ',
                  ),
                ],
              ),
              style: TextStyle(
                color: _mutedText,
                fontSize: 11,
                fontFeatures: [FontFeature.tabularFigures()],
                height: 1.35,
                letterSpacing: 0,
              ),
            ),
          ),
          Container(
            width: 7,
            height: 7,
            margin: const EdgeInsets.only(top: 5, right: 9),
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
          Expanded(
            child: Text.rich(
              TextSpan(
                style: TextStyle(
                  color: _text,
                  fontSize: 12,
                  height: 1.35,
                  letterSpacing: 0,
                ),
                children: [
                  TextSpan(
                    text: '${log.category}  ',
                    style: TextStyle(color: color, fontWeight: FontWeight.w700),
                  ),
                  TextSpan(text: tr(log.message)),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _HostKeyPrompt extends StatelessWidget {
  const _HostKeyPrompt({super.key, this.profile, this.target, this.fingerprint})
    : assert(profile != null || target != null);

  final SshConnectionProfile? profile;
  final String? target;
  final String? fingerprint;

  @override
  Widget build(BuildContext context) {
    final host = target ?? '${profile!.host}:${profile!.port}';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          tr(
            'common.label.checkingHostAuthenticity',
            fallback: 'Checking host authenticity',
          ),
          style: TextStyle(
            color: _text,
            fontSize: 18,
            fontWeight: FontWeight.w700,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 28),
        Text(
          tr(
            'The authenticity of $host is not in Nauterm known_hosts yet. Trust it for this connection only, or save the fingerprint and reconnect.',
          ),
          style: _connectionBodyStyle,
        ),
        if (fingerprint != null) ...[
          SizedBox(height: 18),
          Text(
            tr(
              'workspace.label.serverFingerprint',
              fallback: 'Server fingerprint:',
            ),
            style: _connectionBodyStyle,
          ),
          SizedBox(height: 18),
          Text(fingerprint!, style: _connectionBodyStyle),
        ],
      ],
    );
  }
}

class _ConnectionFailureBody extends StatelessWidget {
  const _ConnectionFailureBody({
    super.key,
    required this.message,
    this.icon = Icons.error_outline_rounded,
    this.iconColor = const Color(0xffd24135),
  });

  final String message;
  final IconData icon;
  final Color iconColor;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(icon, size: 18, color: iconColor),
            SizedBox(width: 10),
            Expanded(
              child: Text(
                tr(message),
                style: TextStyle(
                  color: _text,
                  fontSize: 14,
                  height: 1.4,
                  fontWeight: FontWeight.w500,
                  letterSpacing: 0,
                ),
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _AuthTabs extends StatelessWidget {
  const _AuthTabs({required this.selected, required this.onChanged});

  final _ConnectionAuthTab selected;
  final ValueChanged<_ConnectionAuthTab> onChanged;

  @override
  Widget build(BuildContext context) {
    final small =
        _WorkspaceControlSizeScope.maybeOf(context) ==
        _WorkspaceControlSize.tiny;
    return Container(
      height: small ? 28 : 36,
      padding: const EdgeInsets.all(1),
      decoration: BoxDecoration(
        color: _workspaceDark ? _sidebarHover : _sidebarDivider,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Stack(
        fit: StackFit.expand,
        children: [
          AnimatedAlign(
            duration: const Duration(milliseconds: 180),
            curve: Curves.easeOutCubic,
            alignment: selected == _ConnectionAuthTab.password
                ? Alignment.centerLeft
                : Alignment.centerRight,
            child: FractionallySizedBox(
              widthFactor: 0.5,
              heightFactor: 1,
              child: DecoratedBox(
                decoration: BoxDecoration(
                  color: _card,
                  borderRadius: BorderRadius.circular(6),
                  border: Border.all(
                    color: _workspaceDark
                        ? _sidebarDivider
                        : const Color(0xffd3dfe2),
                  ),
                  boxShadow: [
                    BoxShadow(
                      color: Colors.black.withValues(
                        alpha: _workspaceDark ? 0.18 : 0.07,
                      ),
                      blurRadius: 3,
                      offset: const Offset(0, 1),
                    ),
                  ],
                ),
              ),
            ),
          ),
          Row(
            children: [
              _AuthTabButton(
                selected: selected == _ConnectionAuthTab.password,
                icon: Icons.lock_rounded,
                label: 'Password',
                onTap: () => onChanged(_ConnectionAuthTab.password),
              ),
              _AuthTabButton(
                selected: selected == _ConnectionAuthTab.publicKey,
                icon: Icons.key_rounded,
                label: 'Public Key',
                onTap: () => onChanged(_ConnectionAuthTab.publicKey),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _AuthTabButton extends StatelessWidget {
  const _AuthTabButton({
    required this.selected,
    required this.icon,
    required this.label,
    required this.onTap,
  });

  final bool selected;
  final IconData icon;
  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final foreground = selected ? _blue : _mutedText;
    return Expanded(
      child: Material(
        color: Colors.transparent,
        borderRadius: BorderRadius.circular(6),
        child: InkWell(
          splashFactory: InkRipple.splashFactory,
          borderRadius: BorderRadius.circular(6),
          hoverColor: selected
              ? Colors.transparent
              : _blue.withValues(alpha: _workspaceDark ? 0.10 : 0.055),
          splashColor: _blue.withValues(alpha: _workspaceDark ? 0.18 : 0.12),
          highlightColor: Colors.transparent,
          onTap: onTap,
          child: Center(
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(icon, size: 14, color: foreground),
                SizedBox(width: 6),
                Text(
                  tr(label),
                  style: TextStyle(
                    color: foreground,
                    fontSize: 12,
                    fontWeight: FontWeight.w700,
                    letterSpacing: 0,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _ConnectionKeyCard extends StatelessWidget {
  const _ConnectionKeyCard({
    required this.keyOption,
    required this.selected,
    required this.onTap,
  });

  final TerminalConnectionKeyOption keyOption;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: selected
          ? _card
          : _card.withValues(alpha: _workspaceDark ? 0.72 : 0.86),
      borderRadius: BorderRadius.circular(13),
      child: InkWell(
        borderRadius: BorderRadius.circular(13),
        hoverColor: _workspaceDark ? _sidebarHover : null,
        splashColor: _workspaceDark ? _workspaceMenuPressed : null,
        highlightColor: _workspaceDark ? _workspaceMenuPressed : null,
        onTap: onTap,
        child: Container(
          height: 64,
          padding: const EdgeInsets.symmetric(horizontal: 14),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(13),
            border: Border.all(
              color: selected
                  ? _blue.withValues(alpha: 0.78)
                  : Colors.transparent,
            ),
          ),
          child: Row(
            children: [
              Container(
                width: 40,
                height: 40,
                decoration: BoxDecoration(
                  color: const Color(0xff00649d),
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Icon(Icons.key_rounded, color: Colors.white),
              ),
              SizedBox(width: 14),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      keyOption.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: _text,
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                        letterSpacing: 0,
                      ),
                    ),
                    SizedBox(height: 2),
                    Text(
                      keyOption.subtitle ?? 'Private key',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: _mutedText,
                        fontSize: 11,
                        fontWeight: FontWeight.w500,
                        letterSpacing: 0,
                      ),
                    ),
                  ],
                ),
              ),
              if (selected)
                Icon(Icons.check_circle_rounded, color: _blue, size: 18),
            ],
          ),
        ),
      ),
    );
  }
}

class _EmptyKeyState extends StatelessWidget {
  const _EmptyKeyState();

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 64,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: _card.withValues(alpha: _workspaceDark ? 0.72 : 0.86),
        borderRadius: BorderRadius.circular(13),
      ),
      child: Text(
        tr(
          'workspace.description.noPublicKeysAvailable',
          fallback: 'No public keys available.',
        ),
        style: TextStyle(
          color: _mutedText,
          fontSize: 13,
          fontWeight: FontWeight.w500,
          letterSpacing: 0,
        ),
      ),
    );
  }
}

class _ConnectionButton extends StatelessWidget {
  const _ConnectionButton({
    super.key,
    required this.label,
    required this.onPressed,
    this.primary = false,
    this.compact = false,
    this.icon,
    this.shape,
  });

  final String label;
  final VoidCallback? onPressed;
  final bool primary;
  final bool compact;
  final IconData? icon;
  final OutlinedBorder? shape;

  @override
  Widget build(BuildContext context) {
    final small =
        _WorkspaceControlSizeScope.maybeOf(context) ==
        _WorkspaceControlSize.tiny;
    final height = small ? 24.0 : 36.0;
    return _WorkspaceButton(
      label: compact && icon != null ? null : label,
      icon: icon,
      onPressed: onPressed,
      size: small ? _WorkspaceControlSize.tiny : _WorkspaceControlSize.small,
      height: height,
      minWidth: compact ? height : null,
      horizontalPadding: compact
          ? 0
          : small
          ? 8
          : 16,
      variant: primary
          ? _WorkspaceButtonVariant.solid
          : _WorkspaceButtonVariant.filled,
      type: primary
          ? _WorkspaceButtonType.primary
          : _WorkspaceButtonType.defaultType,
      shape:
          shape ??
          RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
    );
  }
}

class _AuthenticationContinueMenuButton extends StatefulWidget {
  const _AuthenticationContinueMenuButton({
    required this.enabled,
    required this.saveIsSelected,
    required this.onSelected,
  });

  final bool enabled;
  final bool saveIsSelected;
  final ValueChanged<bool> onSelected;

  @override
  State<_AuthenticationContinueMenuButton> createState() =>
      _AuthenticationContinueMenuButtonState();
}

class _AuthenticationContinueMenuButtonState
    extends State<_AuthenticationContinueMenuButton> {
  final GlobalKey _buttonKey = GlobalKey();

  Future<void> _showMenu() async {
    final buttonContext = _buttonKey.currentContext;
    final buttonBox = buttonContext?.findRenderObject();
    final overlayBox = Overlay.of(context).context.findRenderObject();
    if (buttonBox is! RenderBox || overlayBox is! RenderBox) return;
    final anchor =
        buttonBox.localToGlobal(Offset.zero, ancestor: overlayBox) &
        buttonBox.size;
    final save = !widget.saveIsSelected;
    final action = await showNautermDropdownMenu<bool>(
      context: context,
      anchor: anchor,
      width: 190,
      entries: [
        NautermContextMenuAction<bool>(
          value: save,
          label: save
              ? tr('workspace.label.continueSave', fallback: 'Continue & Save')
              : tr('common.action.continue', fallback: 'Continue'),
          icon: save ? Icons.save_rounded : Icons.arrow_forward_rounded,
        ),
      ],
    );
    if (action != null && mounted) widget.onSelected(action);
  }

  @override
  Widget build(BuildContext context) {
    return _ConnectionButton(
      key: _buttonKey,
      label: '',
      primary: true,
      compact: true,
      onPressed: widget.enabled ? _showMenu : null,
      icon: Icons.keyboard_arrow_down_rounded,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.horizontal(right: Radius.circular(8)),
      ),
    );
  }
}

TextStyle get _connectionBodyStyle => TextStyle(
  color: _text,
  fontSize: 14,
  fontWeight: FontWeight.w400,
  height: 1.5,
  letterSpacing: 0,
);

_ConnectionPageMode _connectionMode(
  TerminalConnectionStatus status,
  List<TerminalConnectionEvent> events,
) {
  if (status.phase == TerminalConnectionPhase.failed) {
    return _ConnectionPageMode.failed;
  }
  if (_hasLatestActionableHostKey(events)) {
    return _ConnectionPageMode.hostKey;
  }
  if (_hasLatestActionableAuth(events)) {
    if (_hasLatestPrivateKeyPassphraseRequest(events)) {
      return _ConnectionPageMode.passphrase;
    }
    return _ConnectionPageMode.authentication;
  }
  if (status.phase == TerminalConnectionPhase.exited) {
    return _ConnectionPageMode.exited;
  }
  if (status.phase == TerminalConnectionPhase.hostKey) {
    return _ConnectionPageMode.hostKey;
  }
  if (status.phase == TerminalConnectionPhase.connected) {
    return _ConnectionPageMode.connected;
  }
  return _ConnectionPageMode.connecting;
}

IconData _connectionProtocolIcon(TerminalController controller) {
  if (controller.serialProfile != null) return Icons.usb_rounded;
  if (controller.telnetProfile != null) return Icons.lan_rounded;
  if (controller.isMoshSession) return Icons.bolt_rounded;
  return Icons.power_rounded;
}

enum _ConnectionLogSeverity { info, success, warning, error }

@immutable
class _ConnectionLogEntry {
  const _ConnectionLogEntry({
    required this.message,
    required this.category,
    required this.severity,
    this.timestamp,
  });

  final String message;
  final String category;
  final _ConnectionLogSeverity severity;
  final DateTime? timestamp;
}

List<_ConnectionLogEntry> _connectionLogEntries(
  TerminalConnectionStatus status,
  SshConnectionProfile? profile,
  SerialConnectionProfile? serialProfile,
  List<TerminalConnectionEvent> events,
) {
  if (profile?.username.trim().isEmpty == true) return const [];
  final logs = <_ConnectionLogEntry>[];
  if (profile != null) {
    logs.add(
      _ConnectionLogEntry(
        message:
            'Opening SSH connection to ${profile.username}@${profile.host}:${profile.port}.',
        category: 'transport',
        severity: _ConnectionLogSeverity.info,
        timestamp: events.firstOrNull?.timestamp,
      ),
    );
  } else if (serialProfile != null) {
    logs.add(
      _ConnectionLogEntry(
        message:
            'Opening serial port ${serialProfile.serialPort} at ${serialProfile.config.summary}.',
        category: 'transport',
        severity: _ConnectionLogSeverity.info,
        timestamp: events.firstOrNull?.timestamp,
      ),
    );
  }
  for (final event in events) {
    final line = event.logLine.trim();
    if (line.isNotEmpty) {
      logs.add(
        _ConnectionLogEntry(
          message: line,
          category: _connectionEventCategory(event.kind),
          severity: _connectionEventSeverity(event.kind),
          timestamp: event.timestamp,
        ),
      );
    }
  }
  final message = status.message;
  if (message != null && !logs.any((log) => log.message.contains(message))) {
    logs.add(
      _ConnectionLogEntry(
        message: message,
        category: 'status',
        severity: status.phase == TerminalConnectionPhase.failed
            ? _ConnectionLogSeverity.error
            : _ConnectionLogSeverity.info,
      ),
    );
  }
  return logs;
}

String _connectionDiagnostics(
  TerminalConnectionStatus status,
  SshConnectionProfile? profile,
  SerialConnectionProfile? serialProfile,
  List<TerminalConnectionEvent> events,
  List<_ConnectionLogEntry> logs,
) {
  final lines = <String>[
    'Nauterm connection diagnostics',
    'Generated: ${DateTime.now().toLocal().toIso8601String()}',
    'Phase: ${status.phase.name}',
  ];
  final message = status.message?.trim();
  if (message != null && message.isNotEmpty) {
    lines.add('Status: $message');
  }
  if (profile != null) {
    lines.add('Protocol: SSH');
    lines.add('Target: ${profile.username}@${profile.host}:${profile.port}');
    lines.add('Known hosts: ${profile.knownHostsPath}');
    if (profile.label?.trim().isNotEmpty == true) {
      lines.add('Label: ${profile.label!.trim()}');
    }
    if (profile.hostId != null) {
      lines.add('Host id: ${profile.hostId}');
    }
    if (profile.identityId != null) {
      lines.add('Identity id: ${profile.identityId}');
    }
    if (profile.privateKey?.trim().isNotEmpty == true) {
      lines.add('Private key: configured');
    }
    if (profile.password?.isNotEmpty == true) {
      lines.add('Password: configured');
    }
    if (profile.environment.isNotEmpty) {
      lines.add(
        'Environment: ${profile.environment.keys.toList(growable: false)..sort()}',
      );
    }
  } else if (serialProfile != null) {
    lines.add('Protocol: Serial');
    lines.add('Target: ${serialProfile.serialPort}');
    lines.add('Serial: ${serialProfile.config.summary}');
  }
  lines.add('Events: ${events.length}');
  lines.add('');
  lines.add('Log:');
  if (logs.isEmpty) {
    lines.add('  (no events)');
  } else {
    for (final log in logs) {
      final time = log.timestamp == null
          ? '--:--:--.---'
          : _formatConnectionDiagnosticTime(log.timestamp!);
      lines.add(
        '  $time [${log.severity.name.toUpperCase()}] [${log.category}] ${log.message}',
      );
    }
  }
  return lines.join('\n');
}

String _connectionEventCategory(TerminalConnectionEventKind kind) {
  return switch (kind) {
    TerminalConnectionEventKind.knownHostCheck ||
    TerminalConnectionEventKind.knownHostVerified ||
    TerminalConnectionEventKind.knownHostStoreMissing ||
    TerminalConnectionEventKind.hostKeyUnknown ||
    TerminalConnectionEventKind.hostKeyAccepted ||
    TerminalConnectionEventKind.hostKeyAcceptedForSession ||
    TerminalConnectionEventKind.hostKeyChanged ||
    TerminalConnectionEventKind.hostKeyRejected ||
    TerminalConnectionEventKind.hostKeySaveFailed => 'host-key',
    TerminalConnectionEventKind.authCredentialsRequired ||
    TerminalConnectionEventKind.authCredentialsSubmitted ||
    TerminalConnectionEventKind.authPartialSuccess ||
    TerminalConnectionEventKind.authInteractiveStart ||
    TerminalConnectionEventKind.authInteractiveRequest ||
    TerminalConnectionEventKind.authInteractiveSubmitted ||
    TerminalConnectionEventKind.authInteractiveFailed ||
    TerminalConnectionEventKind.authNoneStart ||
    TerminalConnectionEventKind.authNoneRejected ||
    TerminalConnectionEventKind.authNoneFailed ||
    TerminalConnectionEventKind.authPasswordStart ||
    TerminalConnectionEventKind.authPasswordRejected ||
    TerminalConnectionEventKind.authPasswordFailed ||
    TerminalConnectionEventKind.authKeyStart ||
    TerminalConnectionEventKind.authKeyRejected ||
    TerminalConnectionEventKind.authKeyFailed ||
    TerminalConnectionEventKind.authPassphraseRequired ||
    TerminalConnectionEventKind.authSuccess ||
    TerminalConnectionEventKind.authFailed => 'auth',
    TerminalConnectionEventKind.authAgentStart ||
    TerminalConnectionEventKind.authAgentIdentities ||
    TerminalConnectionEventKind.authAgentIdentityStart ||
    TerminalConnectionEventKind.authAgentIdentityRejected ||
    TerminalConnectionEventKind.authAgentUnavailable ||
    TerminalConnectionEventKind.authAgentFailed => 'agent',
    TerminalConnectionEventKind.connected ||
    TerminalConnectionEventKind.sshShellProbeSkipped ||
    TerminalConnectionEventKind.hostOsInferred ||
    TerminalConnectionEventKind.sshLatencyUpdated ||
    TerminalConnectionEventKind.exitStatus ||
    TerminalConnectionEventKind.sessionClosed => 'session',
    TerminalConnectionEventKind.connectStart ||
    TerminalConnectionEventKind.retry ||
    TerminalConnectionEventKind.sshLatencyUpdated ||
    TerminalConnectionEventKind.moshEchoEnabled ||
    TerminalConnectionEventKind.moshEchoDisabled ||
    TerminalConnectionEventKind.moshPredictionConfirmed ||
    TerminalConnectionEventKind.moshInputStateQueued ||
    TerminalConnectionEventKind.moshScreenCommitted ||
    TerminalConnectionEventKind.moshLatencyUpdated ||
    TerminalConnectionEventKind.moshUdpPeerConfirmed ||
    TerminalConnectionEventKind.moshNetworkSwitching ||
    TerminalConnectionEventKind.moshNetworkDegraded ||
    TerminalConnectionEventKind.moshNetworkRestored ||
    TerminalConnectionEventKind.connectionLost ||
    TerminalConnectionEventKind.error ||
    TerminalConnectionEventKind.unknown => 'transport',
  };
}

_ConnectionLogSeverity _connectionEventSeverity(
  TerminalConnectionEventKind kind,
) {
  return switch (kind) {
    TerminalConnectionEventKind.knownHostVerified ||
    TerminalConnectionEventKind.hostKeyAccepted ||
    TerminalConnectionEventKind.hostKeyAcceptedForSession ||
    TerminalConnectionEventKind.authSuccess ||
    TerminalConnectionEventKind.connected ||
    TerminalConnectionEventKind.moshNetworkRestored =>
      _ConnectionLogSeverity.success,
    TerminalConnectionEventKind.hostKeyUnknown ||
    TerminalConnectionEventKind.knownHostStoreMissing ||
    TerminalConnectionEventKind.authNoneRejected ||
    TerminalConnectionEventKind.authPasswordRejected ||
    TerminalConnectionEventKind.authKeyRejected ||
    TerminalConnectionEventKind.authPassphraseRequired ||
    TerminalConnectionEventKind.authAgentIdentityRejected ||
    TerminalConnectionEventKind.authAgentUnavailable ||
    TerminalConnectionEventKind.retry ||
    TerminalConnectionEventKind.moshNetworkSwitching ||
    TerminalConnectionEventKind.moshNetworkDegraded =>
      _ConnectionLogSeverity.warning,
    TerminalConnectionEventKind.hostKeyChanged ||
    TerminalConnectionEventKind.hostKeyRejected ||
    TerminalConnectionEventKind.hostKeySaveFailed ||
    TerminalConnectionEventKind.authNoneFailed ||
    TerminalConnectionEventKind.authPasswordFailed ||
    TerminalConnectionEventKind.authKeyFailed ||
    TerminalConnectionEventKind.authAgentFailed ||
    TerminalConnectionEventKind.authFailed ||
    TerminalConnectionEventKind.authInteractiveFailed ||
    TerminalConnectionEventKind.connectionLost ||
    TerminalConnectionEventKind.error => _ConnectionLogSeverity.error,
    TerminalConnectionEventKind.authCredentialsRequired ||
    TerminalConnectionEventKind.authCredentialsSubmitted ||
    TerminalConnectionEventKind.authPartialSuccess ||
    TerminalConnectionEventKind.authInteractiveStart ||
    TerminalConnectionEventKind.authInteractiveRequest ||
    TerminalConnectionEventKind.authInteractiveSubmitted ||
    TerminalConnectionEventKind.connectStart ||
    TerminalConnectionEventKind.knownHostCheck ||
    TerminalConnectionEventKind.authNoneStart ||
    TerminalConnectionEventKind.authPasswordStart ||
    TerminalConnectionEventKind.authKeyStart ||
    TerminalConnectionEventKind.authAgentStart ||
    TerminalConnectionEventKind.authAgentIdentities ||
    TerminalConnectionEventKind.authAgentIdentityStart ||
    TerminalConnectionEventKind.exitStatus ||
    TerminalConnectionEventKind.sessionClosed ||
    TerminalConnectionEventKind.sshLatencyUpdated ||
    TerminalConnectionEventKind.moshEchoEnabled ||
    TerminalConnectionEventKind.moshEchoDisabled ||
    TerminalConnectionEventKind.moshPredictionConfirmed ||
    TerminalConnectionEventKind.moshInputStateQueued ||
    TerminalConnectionEventKind.moshScreenCommitted ||
    TerminalConnectionEventKind.moshLatencyUpdated ||
    TerminalConnectionEventKind.moshUdpPeerConfirmed ||
    TerminalConnectionEventKind.sshShellProbeSkipped ||
    TerminalConnectionEventKind.hostOsInferred ||
    TerminalConnectionEventKind.unknown => _ConnectionLogSeverity.info,
  };
}

Color _logSeverityColor(_ConnectionLogSeverity severity) {
  return switch (severity) {
    _ConnectionLogSeverity.info => _mutedText,
    _ConnectionLogSeverity.success => const Color(0xff14a765),
    _ConnectionLogSeverity.warning => const Color(0xffbf7b12),
    _ConnectionLogSeverity.error => const Color(0xffd54a45),
  };
}

String _formatConnectionLogTime(DateTime timestamp) {
  final local = timestamp.toLocal();
  return '${_twoDigits(local.hour)}:${_twoDigits(local.minute)}:${_twoDigits(local.second)}';
}

String _formatConnectionDiagnosticTime(DateTime timestamp) {
  final local = timestamp.toLocal();
  return '${_twoDigits(local.hour)}:${_twoDigits(local.minute)}:'
      '${_twoDigits(local.second)}.${_threeDigits(local.millisecond)}';
}

String _twoDigits(int value) => value.toString().padLeft(2, '0');

String _threeDigits(int value) => value.toString().padLeft(3, '0');

bool _hasLatestActionableHostKey(List<TerminalConnectionEvent> events) {
  for (final event in events.reversed) {
    switch (event.kind) {
      case TerminalConnectionEventKind.hostKeyAccepted:
      case TerminalConnectionEventKind.hostKeyAcceptedForSession:
      case TerminalConnectionEventKind.connected:
        return false;
      case TerminalConnectionEventKind.hostKeyUnknown:
      case TerminalConnectionEventKind.knownHostStoreMissing:
        return true;
      default:
        break;
    }
  }
  return false;
}

bool _hasLatestActionableAuth(List<TerminalConnectionEvent> events) {
  for (final event in events.reversed) {
    switch (event.kind) {
      case TerminalConnectionEventKind.authCredentialsSubmitted:
      case TerminalConnectionEventKind.authPartialSuccess:
      case TerminalConnectionEventKind.authInteractiveStart:
      case TerminalConnectionEventKind.authInteractiveSubmitted:
      case TerminalConnectionEventKind.authInteractiveFailed:
      case TerminalConnectionEventKind.authSuccess:
      case TerminalConnectionEventKind.connected:
        return false;
      case TerminalConnectionEventKind.authCredentialsRequired:
      case TerminalConnectionEventKind.authInteractiveRequest:
      case TerminalConnectionEventKind.authFailed:
      case TerminalConnectionEventKind.authPassphraseRequired:
      case TerminalConnectionEventKind.authPasswordRejected:
      case TerminalConnectionEventKind.authKeyRejected:
      case TerminalConnectionEventKind.authAgentUnavailable:
        return true;
      default:
        break;
    }
  }
  return false;
}

bool _hasLatestFido2PinRequest(List<TerminalConnectionEvent> events) {
  for (final event in events.reversed) {
    if (event.kind == TerminalConnectionEventKind.authPassphraseRequired) {
      return event.method == 'fido2';
    }
    if (event.kind == TerminalConnectionEventKind.authCredentialsSubmitted ||
        event.kind == TerminalConnectionEventKind.authCredentialsRequired ||
        event.kind == TerminalConnectionEventKind.authPartialSuccess ||
        event.kind == TerminalConnectionEventKind.authInteractiveStart ||
        event.kind == TerminalConnectionEventKind.connectStart ||
        event.kind == TerminalConnectionEventKind.authSuccess ||
        event.kind == TerminalConnectionEventKind.connected) {
      return false;
    }
  }
  return false;
}

bool _hasLatestPrivateKeyPassphraseRequest(
  List<TerminalConnectionEvent> events,
) {
  for (final event in events.reversed) {
    if (event.kind == TerminalConnectionEventKind.authPassphraseRequired) {
      return event.method != 'fido2';
    }
    if (event.kind == TerminalConnectionEventKind.authCredentialsSubmitted ||
        event.kind == TerminalConnectionEventKind.authCredentialsRequired ||
        event.kind == TerminalConnectionEventKind.authPartialSuccess ||
        event.kind == TerminalConnectionEventKind.authInteractiveStart ||
        event.kind == TerminalConnectionEventKind.connectStart ||
        event.kind == TerminalConnectionEventKind.authSuccess ||
        event.kind == TerminalConnectionEventKind.connected) {
      return false;
    }
  }
  return false;
}

String? _latestFingerprint(List<TerminalConnectionEvent> events) {
  for (final event in events.reversed) {
    final fingerprint = event.fingerprint;
    if (fingerprint != null && fingerprint.trim().isNotEmpty) {
      return fingerprint;
    }
  }
  return null;
}
