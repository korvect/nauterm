// ignore_for_file: invalid_use_of_internal_member, implementation_imports

import 'dart:async';
import 'dart:convert';
import 'dart:io' as io;
import 'dart:isolate';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/src/widgets/_window.dart' hide WindowManager;
import 'package:flutter/services.dart';
import 'package:file_selector/file_selector.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/legacy.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';
import 'package:nativeapi/nativeapi.dart' hide Dialog;
import 'package:path_provider/path_provider.dart';

import '../ai/ai_client.dart';
import '../ai/ai_attachment.dart';
import '../ai/ai_config.dart';
import '../ai/ai_conversation.dart';
import '../ai/ai_context.dart';
import '../data/ai_conversation_repository.dart';
import '../data/ai_provider_store.dart';
import '../data/known_hosts_store.dart';
import '../data/host_import.dart';
import '../data/nauterm_config_store.dart';
import '../data/nauterm_data_store.dart';
import '../data/nauterm_paths.dart';
import '../data/recording_service.dart';
import '../data/shell_history_reader.dart';
import '../data/shell_history_file_store.dart';
import '../data/terminal_recording_store.dart';
import '../data/terminal_retention_policy.dart';
import '../data/terminal_theme_store.dart';
import '../app/nauterm_localizations.dart';
import '../app/nauterm_log.dart';
import '../app/nauterm_theme.dart';
import '../ui/nauterm_context_menu.dart';
import '../ui/terminal_theme_preview.dart';
import '../ui/nauterm_overlay.dart';
import '../ui/nauterm_typography.dart';
import '../terminal/terminal_config.dart';
import '../terminal/terminal_controller.dart';
import '../terminal/terminal_driver.dart';
import '../terminal/external_editor_catalog.dart';
import '../terminal/terminal_ffi.dart';
import '../terminal/terminal_key_encoder.dart';
import '../terminal/terminal_models.dart';
import '../terminal/terminal_recording.dart';
import '../terminal/terminal_recording_config.dart';
import '../terminal/terminal_replay_sanitizer.dart';
import '../terminal/system_font_catalog.dart';
import '../terminal/system_shells.dart';
import '../terminal/terminal_theme.dart';
import '../update/startup_update.dart';
import '../terminal/terminal_widget.dart';
import '../window/file_drop_channel.dart';
import '../window/native_windowing.dart';
import 'workspace_composer_completion.dart';
import 'terminal_lifecycle_service.dart';

part 'workspace_style.dart';
part 'workspace_models.dart';
part 'workspace_top_bar.dart';
part 'vault_workspace.dart';
part 'hosts_pane.dart';
part 'workspace_toolbar.dart';
part 'workspace_sorting.dart';
part 'workspace_item_grid.dart';
part 'workspace_context_menu.dart';
part 'workspace_form_controls.dart';
part 'workspace_editor_drawer.dart';
part 'keychain_pane.dart';
part 'workspace_connection_page.dart';
part 'workspace_panes.dart';
part 'workspace_quick_connect.dart';
part 'workspace_sessions.dart';
part 'workspace_sftp_tasks.dart';
part 'workspace_sftp.dart';
part 'workspace_dialogs.dart';
part 'workspace_serial_dialog.dart';
part 'workspace_item_mapping.dart';
part 'workspace_completion_isolates.dart';
part 'workspace_helpers.dart';
part 'workspace_sftp_task_bar.dart';
part 'workspace_sftp_file_panes.dart';
part 'workspace_sftp_connection_view.dart';
part 'workspace_sftp_models.dart';
part 'workspace_editor_host_forms.dart';
part 'workspace_editor_host_environment.dart';
part 'workspace_editor_theme_gallery.dart';
part 'workspace_editor_key_generation.dart';
part 'workspace_editor_key_export.dart';
part 'workspace_editor_identity_forms.dart';
part 'workspace_completion_methods.dart';
part 'workspace_session_actions.dart';
part 'workspace_editor_actions.dart';
part 'workspace_host_import.dart';
part 'workspace_terminal_actions.dart';
part 'workspace_rendering.dart';
part 'workspace_terminal_tool_panel.dart';
part 'workspace_terminal_controls.dart';
part 'workspace_terminal_sftp_tool.dart';
part 'workspace_terminal_system_tool.dart';
part 'workspace_terminal_command_library.dart';
part 'workspace_terminal_theme_gallery.dart';
part 'workspace_terminal_reconnect.dart';
part 'workspace_ai_controls.dart';
part 'workspace_ai_conversation.dart';

final Set<Future<void> Function()> _workspaceShutdownHooks = {};

class NautermWorkspaceController extends ChangeNotifier {
  _NautermWorkspaceState? _state;
  StartupUpdateNotice? _updateNotice;
  final Completer<void> _initialDataReady = Completer<void>();
  Future<void>? _flushAndCloseFuture;

  bool get hasTerminalTabs => _state?._allTerminalTabs.isNotEmpty ?? false;
  Future<void> get initialDataReady => _initialDataReady.future;
  Set<String> get selectedWorkspaceItemIds =>
      _state?._selectedWorkspaceItemIds ?? const <String>{};

  void closeSelectedTerminalTab() {
    _state?._closeSelectedTerminalTab();
  }

  Future<bool> confirmQuitIfNeeded() async {
    return _state?._confirmQuitIfNeeded() ?? true;
  }

  Future<bool> confirmCloseWindowIfNeeded() async {
    return _state?._confirmCloseWindowIfNeeded() ?? true;
  }

  Future<void> reloadData() async {
    await _state?._loadWorkspaceData();
  }

  Future<String?> loadSkippedUpdateVersion() async {
    await initialDataReady;
    return _state?._dataStore?.getAppMetadata('skipped_update_version');
  }

  Future<void> saveSkippedUpdateVersion(String version) async {
    await initialDataReady;
    _state?._dataStore?.setAppMetadata('skipped_update_version', version);
  }

  void showUpdateNotice(StartupUpdateNotice? notice) {
    _updateNotice = notice;
    _state?._showUpdateNotice(notice);
  }

  Future<void> flushAndClose() {
    return _flushAndCloseFuture ??=
        _state?._flushAndClose() ?? Future<void>.value();
  }

  void _attach(_NautermWorkspaceState state) {
    _state = state;
    state._showUpdateNotice(_updateNotice);
    notifyListeners();
  }

  void _detach(_NautermWorkspaceState state) {
    if (identical(_state, state)) {
      _state = null;
      notifyListeners();
    }
  }

  void _markInitialDataReady() {
    if (!_initialDataReady.isCompleted) {
      _initialDataReady.complete();
    }
  }

  @override
  void dispose() {
    _markInitialDataReady();
    super.dispose();
  }

  void _notifyTabsChanged() {
    notifyListeners();
  }

  void _notifyWorkspaceItemSelectionChanged() {
    notifyListeners();
  }
}

final _nautermWorkspaceModelProvider = ChangeNotifierProvider.autoDispose
    .family<_NautermWorkspaceModel, Object>(
      (ref, _) => _NautermWorkspaceModel(),
    );

class _NautermWorkspaceModel extends ChangeNotifier {
  _WorkspaceTab tab = _WorkspaceTab.vaults;
  _SidebarSection section = _SidebarSection.hosts;
  List<HostGroup> groupEntries = const [];
  List<HostEntry> hostEntries = const [];
  List<KeyEntry> keyEntries = const [];
  List<IdentityEntry> identityEntries = const [];
  List<TagEntry> tagEntries = const [];
  List<PortForwardEntry> portForwardEntries = const [];
  List<ProxyEntry> proxyEntries = const [];
  List<SnippetPackageEntry> snippetPackageEntries = const [];
  List<_GroupItem> groups = const [];
  List<_HostItem> hosts = const [];
  List<_KeyItem> keys = const [];
  List<_IdentityItem> identities = const [];
  List<_PortForwardItem> portForwards = const [];
  List<_ProxyItem> proxies = const [];
  final Set<int> runningPortForwardIds = <int>{};
  final Map<int, FfiPortForwardStatus> portForwardStatuses =
      <int, FfiPortForwardStatus>{};
  List<_SnippetItem> snippets = const [];
  final Map<_SidebarSection, Set<String>> selectedWorkspaceItemIds = {
    for (final section in _SidebarSection.values) section: <String>{},
  };
  String knownHostsText = '';
  bool loadingData = true;
  bool workspaceOverviewActive = false;
  _WorkspaceNotification? notification;
  StartupUpdateNotice? updateNotice;
  int sftpConnectRequestId = 0;
  _SftpConnectRequest? sftpConnectRequest;
  bool sftpPaneMounted = false;
  bool sftpFileDropEnabled = false;
  List<TerminalLogEntry> terminalLogs = const [];
  bool terminalLogsHasMore = true;
  bool terminalLogsLoading = false;
  List<ShellHistoryEntry> shellHistory = const [];
  final Map<TerminalController, List<ShellHistoryEntry>>
  shellHistoryByController = {};
  final Set<TerminalController> shellHistoryLoading = {};
  final Map<TerminalController, DateTime> shellHistoryLastRead = {};
  Future<void> shellHistoryPersistence = Future<void>.value();
  String? selectedLogId;
  final List<_WorkspaceRuntimeState> workspaces = [
    _WorkspaceRuntimeState(
      id: 1,
      name: 'Default',
      icon: Icons.dashboard_rounded,
      color: const Color(0xff075e92),
    ),
  ];
  int selectedWorkspaceId = 1;
  int nextTerminalId = 1;
  int nextTerminalSplitId = 1;
  int nextWorkspaceId = 2;
  final Map<_SidebarSection, List<_WorkspaceEditorStackEntry>> editorStacks = {
    for (final section in _SidebarSection.values)
      section: <_WorkspaceEditorStackEntry>[],
  };

  void mutate(VoidCallback update) {
    update();
    notifyListeners();
  }
}

class NautermWorkspace extends ConsumerStatefulWidget {
  const NautermWorkspace({
    super.key,
    required this.onOpenSettings,
    required this.onOpenTerminalSettings,
    this.controller,
    this.onStartWindowDrag,
    this.onToggleWindowMaximized,
  });

  final VoidCallback onOpenSettings;
  final VoidCallback onOpenTerminalSettings;
  final NautermWorkspaceController? controller;
  final VoidCallback? onStartWindowDrag;
  final VoidCallback? onToggleWindowMaximized;

  @override
  ConsumerState<NautermWorkspace> createState() => _NautermWorkspaceState();
}

class _NautermWorkspaceState extends ConsumerState<NautermWorkspace> {
  late final Object _workspaceModelKey;
  late final _NautermWorkspaceModel _workspaceModel;
  late final AiConversationRepository _aiConversationRepository;
  late final RecordingService _recordingService;
  NautermDataStore? _dataStore;
  Timer? _notificationTimer;
  Timer? _portForwardStatusTimer;
  final NautermOverlayController _overlayController =
      NautermOverlayController();
  final FocusNode _workspaceFocusNode = FocusNode(
    debugLabel: 'nauterm workspace',
  );
  late final Map<_SidebarSection, _WorkspaceItemSelectionController>
  _itemSelectionControllers;
  TerminalThemeCatalog? _terminalThemeCatalog;
  TerminalLogCaptureStore? _terminalLogCaptureStore;
  int _terminalCaptureDiskUsage = 0;
  NautermRecordingConfig _lastRecordingConfig = terminalRecordingConfig;
  final Map<String, List<String>> _localZshCompletionCache = {};
  final Set<String> _localZshCompletionRequests = {};
  final Map<String, Timer> _localZshCompletionDebounceTimers = {};
  final Map<String, String> _activeLocalZshCompletionKeys = {};
  static const Duration _completionDebounceDuration = Duration(
    milliseconds: 160,
  );
  final Map<String, List<String>> _localShellCommandCompletionCache = {};
  final Set<String> _localShellCommandCompletionRequests = {};
  final Set<String> _localShellCommandCompletionErrorsShown = {};
  final Map<String, List<String>> _sshDirectoryCompletionCache = {};
  final Set<String> _sshDirectoryCompletionRequests = {};
  final Set<String> _sshDirectoryCompletionErrorsShown = {};
  final Map<int, Timer> _sshDirectoryCompletionDebounceTimers = {};
  final Map<int, String> _activeSshDirectoryCompletionKeys = {};
  final Map<String, List<WorkspacePathCompletionEntry>>
  _sshPathCompletionCache = {};
  final Set<String> _sshPathCompletionRequests = {};
  final Set<String> _sshPathCompletionErrorsShown = {};
  final Map<int, Timer> _sshPathCompletionDebounceTimers = {};
  final Map<int, String> _activeSshPathCompletionKeys = {};
  final Map<int, String> _sshWorkingDirectories = {};
  final Map<int, String> _pendingSshWorkingDirectories = {};
  final Map<int, String> _sshInputBuffers = {};
  final Set<int> _hostOsDetectionRequests = {};
  bool _filePickerInFlight = false;
  bool _quickConnectDialogOpen = false;
  bool _aiAssistantResizing = false;
  bool _isClosing = false;
  Future<void>? _flushAndCloseFuture;
  late final TerminalLifecycleService _terminalLifecycleService;

  _WorkspaceTab get _tab => _workspaceModel.tab;

  set _tab(_WorkspaceTab value) {
    _workspaceModel.tab = value;
  }

  _SidebarSection get _section => _workspaceModel.section;

  set _section(_SidebarSection value) {
    _workspaceModel.section = value;
  }

  Set<String> get _selectedWorkspaceItemIds => Set.unmodifiable(
    _workspaceModel.selectedWorkspaceItemIds[_section] ?? const <String>{},
  );

  List<HostGroup> get _groupEntries => _workspaceModel.groupEntries;

  set _groupEntries(List<HostGroup> value) {
    _workspaceModel.groupEntries = value;
  }

  List<HostEntry> get _hostEntries => _workspaceModel.hostEntries;

  set _hostEntries(List<HostEntry> value) {
    _workspaceModel.hostEntries = value;
  }

  List<KeyEntry> get _keyEntries => _workspaceModel.keyEntries;

  set _keyEntries(List<KeyEntry> value) {
    _workspaceModel.keyEntries = value;
  }

  List<IdentityEntry> get _identityEntries => _workspaceModel.identityEntries;

  set _identityEntries(List<IdentityEntry> value) {
    _workspaceModel.identityEntries = value;
  }

  List<TagEntry> get _tagEntries => _workspaceModel.tagEntries;

  set _tagEntries(List<TagEntry> value) {
    _workspaceModel.tagEntries = value;
  }

  List<PortForwardEntry> get _portForwardEntries =>
      _workspaceModel.portForwardEntries;

  set _portForwardEntries(List<PortForwardEntry> value) {
    _workspaceModel.portForwardEntries = value;
  }

  List<ProxyEntry> get _proxyEntries => _workspaceModel.proxyEntries;

  set _proxyEntries(List<ProxyEntry> value) {
    _workspaceModel.proxyEntries = value;
  }

  List<SnippetPackageEntry> get _snippetPackageEntries =>
      _workspaceModel.snippetPackageEntries;

  set _snippetPackageEntries(List<SnippetPackageEntry> value) {
    _workspaceModel.snippetPackageEntries = value;
  }

  List<_GroupItem> get _groups => _workspaceModel.groups;

  set _groups(List<_GroupItem> value) {
    _workspaceModel.groups = value;
  }

  List<_HostItem> get _hosts => _workspaceModel.hosts;

  set _hosts(List<_HostItem> value) {
    _workspaceModel.hosts = value;
  }

  List<_KeyItem> get _keys => _workspaceModel.keys;

  set _keys(List<_KeyItem> value) {
    _workspaceModel.keys = value;
  }

  List<_IdentityItem> get _identities => _workspaceModel.identities;

  set _identities(List<_IdentityItem> value) {
    _workspaceModel.identities = value;
  }

  List<_PortForwardItem> get _portForwards => _workspaceModel.portForwards;

  set _portForwards(List<_PortForwardItem> value) {
    _workspaceModel.portForwards = value;
  }

  List<_ProxyItem> get _proxies => _workspaceModel.proxies;

  set _proxies(List<_ProxyItem> value) {
    _workspaceModel.proxies = value;
  }

  Set<int> get _runningPortForwardIds => _workspaceModel.runningPortForwardIds;
  Map<int, FfiPortForwardStatus> get _portForwardStatuses =>
      _workspaceModel.portForwardStatuses;

  List<_SnippetItem> get _snippets => _workspaceModel.snippets;

  set _snippets(List<_SnippetItem> value) {
    _workspaceModel.snippets = value;
  }

  String get _knownHostsText => _workspaceModel.knownHostsText;

  set _knownHostsText(String value) {
    _workspaceModel.knownHostsText = value;
  }

  bool get _loadingData => _workspaceModel.loadingData;

  set _loadingData(bool value) {
    _workspaceModel.loadingData = value;
  }

  bool get _workspaceOverviewActive => _workspaceModel.workspaceOverviewActive;

  set _workspaceOverviewActive(bool value) {
    _workspaceModel.workspaceOverviewActive = value;
  }

  _WorkspaceNotification? get _notification => _workspaceModel.notification;

  set _notification(_WorkspaceNotification? value) {
    _workspaceModel.notification = value;
  }

  StartupUpdateNotice? get _updateNotice => _workspaceModel.updateNotice;

  set _updateNotice(StartupUpdateNotice? value) {
    _workspaceModel.updateNotice = value;
  }

  int get _sftpConnectRequestId => _workspaceModel.sftpConnectRequestId;

  set _sftpConnectRequestId(int value) {
    _workspaceModel.sftpConnectRequestId = value;
  }

  _SftpConnectRequest? get _sftpConnectRequest =>
      _workspaceModel.sftpConnectRequest;

  set _sftpConnectRequest(_SftpConnectRequest? value) {
    _workspaceModel.sftpConnectRequest = value;
  }

  bool get _sftpPaneMounted => _workspaceModel.sftpPaneMounted;

  set _sftpPaneMounted(bool value) {
    _workspaceModel.sftpPaneMounted = value;
  }

  bool get _sftpFileDropEnabled => _workspaceModel.sftpFileDropEnabled;

  set _sftpFileDropEnabled(bool value) {
    _workspaceModel.sftpFileDropEnabled = value;
  }

  List<TerminalSessionRecorder> get _terminalSessionRecorders =>
      _recordingService.recorders;

  List<TerminalLogEntry> get _terminalLogs => _workspaceModel.terminalLogs;

  set _terminalLogs(List<TerminalLogEntry> value) {
    _workspaceModel.terminalLogs = value;
  }

  bool get _terminalLogsHasMore => _workspaceModel.terminalLogsHasMore;

  set _terminalLogsHasMore(bool value) {
    _workspaceModel.terminalLogsHasMore = value;
  }

  bool get _terminalLogsLoading => _workspaceModel.terminalLogsLoading;

  set _terminalLogsLoading(bool value) {
    _workspaceModel.terminalLogsLoading = value;
  }

  List<ShellHistoryEntry> get _shellHistory => _workspaceModel.shellHistory;

  set _shellHistory(List<ShellHistoryEntry> value) {
    _workspaceModel.shellHistory = value;
  }

  Map<TerminalController, List<ShellHistoryEntry>>
  get _shellHistoryByController => _workspaceModel.shellHistoryByController;

  Set<TerminalController> get _shellHistoryLoading =>
      _workspaceModel.shellHistoryLoading;

  Map<TerminalController, DateTime> get _shellHistoryLastRead =>
      _workspaceModel.shellHistoryLastRead;

  Future<void> get _shellHistoryPersistence =>
      _workspaceModel.shellHistoryPersistence;

  set _shellHistoryPersistence(Future<void> value) {
    _workspaceModel.shellHistoryPersistence = value;
  }

  String? get _selectedLogId => _workspaceModel.selectedLogId;

  set _selectedLogId(String? value) {
    _workspaceModel.selectedLogId = value;
  }

  List<_WorkspaceRuntimeState> get _workspaces => _workspaceModel.workspaces;

  int get _selectedWorkspaceId => _workspaceModel.selectedWorkspaceId;

  set _selectedWorkspaceId(int value) {
    _workspaceModel.selectedWorkspaceId = value;
  }

  int get _nextTerminalId => _workspaceModel.nextTerminalId;

  set _nextTerminalId(int value) {
    _workspaceModel.nextTerminalId = value;
  }

  int get _nextTerminalSplitId => _workspaceModel.nextTerminalSplitId;

  set _nextTerminalSplitId(int value) {
    _workspaceModel.nextTerminalSplitId = value;
  }

  int get _nextWorkspaceId => _workspaceModel.nextWorkspaceId;

  set _nextWorkspaceId(int value) {
    _workspaceModel.nextWorkspaceId = value;
  }

  List<_WorkspaceEditorStackEntry> _editorStackFor(_SidebarSection section) =>
      _workspaceModel.editorStacks[section]!;

  List<_WorkspaceEditorStackEntry> get _editorStack =>
      _editorStackFor(_section);

  _WorkspaceEditorRequest? get _editorRequest =>
      _editorStack.isEmpty ? null : _editorStack.last.request;

  set _editorRequest(_WorkspaceEditorRequest? value) {
    _editorStack.clear();
    if (value != null) {
      _editorStack.add(_WorkspaceEditorStackEntry(request: value));
    }
  }

  _WorkspaceRuntimeState get _selectedWorkspace {
    return _workspaces.firstWhere(
      (workspace) => workspace.id == _selectedWorkspaceId,
      orElse: () => _workspaces.first,
    );
  }

  _WorkspaceRuntimeState get _activeSessionWorkspace {
    return _selectedWorkspace;
  }

  List<_TerminalTab> get _terminalTabs => _activeSessionWorkspace.terminalTabs;

  Iterable<_TerminalTab> get _allTerminalTabs sync* {
    for (final workspace in _workspaces) {
      yield* workspace.terminalTabs;
    }
  }

  int? get _selectedTerminalId => _activeSessionWorkspace.selectedTerminalId;

  set _selectedTerminalId(int? value) {
    _activeSessionWorkspace.selectedTerminalId = value;
  }

  int? get _selectedTerminalViewId =>
      _activeSessionWorkspace.selectedTerminalViewId;

  set _selectedTerminalViewId(int? value) {
    _activeSessionWorkspace.selectedTerminalViewId = value;
  }

  List<_SnippetPackageItem> get _snippetPackages {
    return [
      for (final package in _snippetPackageEntries)
        if (package.id != null)
          _SnippetPackageItem(
            id: package.id!,
            name: package.name,
            icon: Icons.inventory_2_rounded,
            color: const Color(0xff075e92),
          ),
    ];
  }

  List<TerminalConnectionKeyOption> get _terminalConnectionKeys {
    return [
      for (final key in _keyEntries)
        if (key.id != null)
          TerminalConnectionKeyOption(
            id: key.id!,
            name: key.name,
            subtitle: _emptyToNull(key.publicKey) == null
                ? 'Key'
                : 'Public key stored',
          ),
    ];
  }

  List<TerminalConnectionIdentityOption> get _terminalConnectionIdentities {
    return [
      for (final identity in _identityEntries)
        if (identity.id != null)
          TerminalConnectionIdentityOption(
            id: identity.id!,
            name: identity.name,
            username: _emptyToNull(identity.username),
            keyId: identity.keyId,
            keyName: identity.keyId == null
                ? null
                : _keyEntries
                      .where((key) => key.id == identity.keyId)
                      .firstOrNull
                      ?.name,
          ),
    ];
  }

  List<String> _snippetComposerSuggestionsForController(
    TerminalController controller,
  ) {
    return [
      for (final snippet in _snippets)
        if (_snippetMatchesController(snippet, controller) &&
            snippet.script.trim().isNotEmpty)
          snippet.script.trim(),
    ];
  }

  bool _snippetMatchesController(
    _SnippetItem snippet,
    TerminalController controller,
  ) {
    if (snippet.scope == SnippetScope.global) {
      return true;
    }

    final hostId = controller.sshProfile?.hostId;
    if (hostId == null) {
      return false;
    }
    if (snippet.targetHostIds.contains(hostId)) {
      return true;
    }

    final host = _hostEntries.where((host) => host.id == hostId).firstOrNull;
    if (host == null) {
      return false;
    }
    final groupIds = _groupScopeIdsForHost(host);
    return snippet.targetGroupIds.any(groupIds.contains);
  }

  Set<int> _groupScopeIdsForHost(HostEntry host) {
    final groupIds = <int>{};
    var groupId = host.groupId;
    while (groupId != null && groupIds.add(groupId)) {
      groupId = _groupEntries
          .where((group) => group.id == groupId)
          .firstOrNull
          ?.parentId;
    }
    return groupIds;
  }

  bool _handleGlobalWorkspaceKeyEvent(KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return false;
    }
    if (!_isQuickConnectKeyEvent(event, terminalShortcutConfig)) {
      return false;
    }
    if (event is KeyRepeatEvent) {
      return true;
    }

    unawaited(_openQuickConnectDialog());
    return true;
  }

  @override
  void initState() {
    super.initState();
    _workspaceModelKey = Object();
    _workspaceModel = ref.read(
      _nautermWorkspaceModelProvider(_workspaceModelKey),
    );
    _itemSelectionControllers = {
      for (final section in _SidebarSection.values)
        section: _WorkspaceItemSelectionController(
          onSelectionChanged: (identities) =>
              _handleWorkspaceItemSelectionChanged(section, identities),
        ),
    };
    _aiConversationRepository = AiConversationRepository(
      dataStore: () => _dataStore,
    );
    _terminalLifecycleService = TerminalLifecycleService(
      releaseConversation: _aiConversationRepository.release,
    );
    _recordingService = RecordingService();
    widget.controller?._attach(this);
    _portForwardStatusTimer = Timer.periodic(
      const Duration(seconds: 2),
      (_) => _syncPortForwardStatuses(),
    );
    HardwareKeyboard.instance.addHandler(_handleGlobalWorkspaceKeyEvent);
    sftpTabEnabledListenable.addListener(_handleSftpTabEnabledChanged);
    workspacePageEnabledListenable.addListener(
      _handleWorkspacePageEnabledChanged,
    );
    aiAssistantConfigListenable.addListener(_handleAiConfigChanged);
    terminalConfigNotifier.addListener(_handleTerminalConfigChanged);
    fullscreenNotifier.addListener(_handleFullscreenChanged);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      unawaited(_loadWorkspaceDataAfterUiPresentation());
    });
  }

  Future<void> _loadWorkspaceDataAfterUiPresentation() async {
    await waitForNautermUiPresentation();
    try {
      if (mounted) {
        await _loadWorkspaceData();
      }
    } finally {
      widget.controller?._markInitialDataReady();
    }
  }

  @override
  void didUpdateWidget(covariant NautermWorkspace oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!identical(widget.controller, oldWidget.controller)) {
      oldWidget.controller?._detach(this);
      widget.controller?._attach(this);
    }
  }

  void _setWorkspaceState(VoidCallback fn) {
    if (!mounted) {
      return;
    }
    _workspaceModel.mutate(fn);
  }

  void _handleWorkspaceItemSelectionChanged(
    _SidebarSection section,
    Set<Object> identities,
  ) {
    final selectedIds = {
      for (final identity in identities)
        if (identity is String) identity,
    };
    _setWorkspaceState(() {
      _workspaceModel.selectedWorkspaceItemIds[section] = selectedIds;
    });
    widget.controller?._notifyWorkspaceItemSelectionChanged();
  }

  void _handleSftpTabEnabledChanged() {
    _setWorkspaceState(() {
      if (!sftpTabEnabled && _tab == _WorkspaceTab.sftp) {
        _tab = _WorkspaceTab.sessions;
        _workspaceOverviewActive = true;
        _editorRequest = null;
      }
    });
  }

  void _handleWorkspacePageEnabledChanged() {
    _setWorkspaceState(() {
      if (!workspacePageEnabled &&
          _tab == _WorkspaceTab.sessions &&
          _workspaceOverviewActive) {
        _tab = _WorkspaceTab.vaults;
        _workspaceOverviewActive = false;
        _editorRequest = null;
      }
    });
  }

  void _handleAiConfigChanged() {
    final config = aiAssistantConfig;
    for (final tab in _allTerminalTabs) {
      tab.aiConversation.updateConfig(config);
    }
    for (final workspace in _workspaces) {
      workspace.aiConversation.updateConfig(config);
    }
  }

  void _handleTerminalConfigChanged() {
    final current = terminalRecordingConfig;
    final previous = _lastRecordingConfig;
    if (current.enabled == previous.enabled &&
        current.captureEnabled == previous.captureEnabled &&
        current.retentionDays == previous.retentionDays &&
        current.maxSessionBytes == previous.maxSessionBytes &&
        current.maxTotalBytes == previous.maxTotalBytes) {
      return;
    }
    _lastRecordingConfig = current;
    if (_dataStore != null && !_isClosing) {
      unawaited(_saveTerminalLogs());
    }
  }

  void _handleFullscreenChanged() {
    if (mounted) setState(() {});
  }

  Future<void> _flushAndClose() {
    return _flushAndCloseFuture ??= _performFlushAndClose();
  }

  Future<void> _performFlushAndClose() async {
    _isClosing = true;
    _cancelShutdownTimers();

    await Future.wait([
      for (final hook in _workspaceShutdownHooks.toList(growable: false))
        _runShutdownHook(hook),
    ]);

    final conversations = <AiConversationController>{
      for (final tab in _allTerminalTabs) tab.aiConversation,
      for (final workspace in _workspaces) workspace.aiConversation,
      ..._aiConversationRepository.watchedConversations,
    };
    for (final conversation in conversations) {
      _disposeAiConversation(conversation);
    }

    for (final controller in <TerminalController>{
      for (final tab in _allTerminalTabs) ...tab.controllers,
    }) {
      if (!controller.isDisposed) {
        try {
          controller.poll();
        } on Object catch (error, stackTrace) {
          NautermLog.warning(
            'shutdown',
            'Unable to drain terminal output.',
            error: error,
            stackTrace: stackTrace,
          );
        }
      }
      _disposeTerminalController(controller);
    }

    await _saveTerminalLogs(updateState: false);
    await _shellHistoryPersistence;

    try {
      FfiPortForwarding.stopAll();
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'shutdown',
        'Unable to stop port forwards.',
        error: error,
        stackTrace: stackTrace,
      );
    }
    _runningPortForwardIds.clear();
    _portForwardStatuses.clear();

    shutdownNativeTerminalRuntime();
    try {
      await NautermFileDropChannel.instance.setEnabled(false);
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'shutdown',
        'Unable to disable file drop.',
        error: error,
        stackTrace: stackTrace,
      );
    }

    final dataStore = _dataStore;
    _dataStore = null;
    dataStore?.dispose();
    _recordingService.dispose();
    NautermLog.info('application', 'Application shutdown completed.');
    await NautermLog.flush();
  }

  Future<void> _runShutdownHook(Future<void> Function() hook) async {
    try {
      await hook();
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'shutdown',
        'Unable to close workspace resource.',
        error: error,
        stackTrace: stackTrace,
      );
    }
  }

  void _cancelShutdownTimers() {
    _recordingService.cancelScheduledSave();
    _notificationTimer?.cancel();
    _portForwardStatusTimer?.cancel();
    for (final timer in _localZshCompletionDebounceTimers.values) {
      timer.cancel();
    }
    for (final timer in _sshDirectoryCompletionDebounceTimers.values) {
      timer.cancel();
    }
    for (final timer in _sshPathCompletionDebounceTimers.values) {
      timer.cancel();
    }
  }

  void _disposeAiConversation(AiConversationController conversation) {
    _terminalLifecycleService.disposeConversation(conversation);
  }

  void _disposeTerminalController(TerminalController controller) {
    _terminalLifecycleService.disposeTerminal(controller);
  }

  @override
  void dispose() {
    widget.controller?._markInitialDataReady();
    HardwareKeyboard.instance.removeHandler(_handleGlobalWorkspaceKeyEvent);
    sftpTabEnabledListenable.removeListener(_handleSftpTabEnabledChanged);
    workspacePageEnabledListenable.removeListener(
      _handleWorkspacePageEnabledChanged,
    );
    aiAssistantConfigListenable.removeListener(_handleAiConfigChanged);
    terminalConfigNotifier.removeListener(_handleTerminalConfigChanged);
    fullscreenNotifier.removeListener(_handleFullscreenChanged);
    widget.controller?._detach(this);
    for (final tab in _allTerminalTabs) {
      _disposeAiConversation(tab.aiConversation);
      for (final controller in tab.controllers) {
        _disposeTerminalController(controller);
      }
    }
    for (final workspace in _workspaces) {
      _disposeAiConversation(workspace.aiConversation);
    }
    for (final conversation
        in _aiConversationRepository.watchedConversations.toList()) {
      _disposeAiConversation(conversation);
    }
    _aiConversationRepository.dispose();
    _cancelShutdownTimers();
    try {
      FfiPortForwarding.stopAll();
    } on Object {
      // App shutdown should continue even if the native library is unavailable.
    }
    _runningPortForwardIds.clear();
    _portForwardStatuses.clear();
    final dataStore = _dataStore;
    unawaited(
      _saveTerminalLogs(updateState: false).whenComplete(() {
        dataStore?.dispose();
        _recordingService.dispose();
      }),
    );
    unawaited(NautermFileDropChannel.instance.setEnabled(false));
    for (final controller in _itemSelectionControllers.values) {
      controller.dispose();
    }
    _workspaceFocusNode.dispose();
    _overlayController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    ref.watch(_nautermWorkspaceModelProvider(_workspaceModelKey));
    final selectedTerminalTab = _selectedTerminalTab;
    final selectedTerminalPageActive =
        _tab == _WorkspaceTab.sessions &&
        !_workspaceOverviewActive &&
        selectedTerminalTab != null;
    final chromeTerminalTab = selectedTerminalPageActive
        ? selectedTerminalTab
        : null;
    return NautermOverlaySafeAreaScope(
      padding: const EdgeInsets.only(top: _topBarHeight),
      child: NautermOverlayScope(
        controller: _overlayController,
        child: Shortcuts(
          shortcuts: _buildWorkspaceShortcuts(terminalShortcutConfig),
          child: Actions(
            actions: {
              _CloseTabIntent: CallbackAction<_CloseTabIntent>(
                onInvoke: (_) {
                  _closeSelectedTerminalTab();
                  return null;
                },
              ),
              _QuickConnectIntent: CallbackAction<_QuickConnectIntent>(
                onInvoke: (_) {
                  unawaited(_openQuickConnectDialog());
                  return null;
                },
              ),
              _PreviousTabIntent: CallbackAction<_PreviousTabIntent>(
                onInvoke: (_) {
                  _selectRelativeTopBarTab(-1);
                  return null;
                },
              ),
              _NextTabIntent: CallbackAction<_NextTabIntent>(
                onInvoke: (_) {
                  _selectRelativeTopBarTab(1);
                  return null;
                },
              ),
              _ShowTerminalSshIntent: CallbackAction<_ShowTerminalSshIntent>(
                onInvoke: (_) {
                  _showSelectedTerminalSsh();
                  return null;
                },
              ),
              _ShowTerminalSftpIntent: CallbackAction<_ShowTerminalSftpIntent>(
                onInvoke: (_) {
                  _showSelectedTerminalSftp();
                  return null;
                },
              ),
              _SelectTabIntent: CallbackAction<_SelectTabIntent>(
                onInvoke: (intent) {
                  _selectTopBarTabIndex(intent.index);
                  return null;
                },
              ),
              _SplitRightIntent: CallbackAction<_SplitRightIntent>(
                onInvoke: (_) {
                  _splitSelectedTerminalTab(TerminalSplitDirection.right);
                  return null;
                },
              ),
              _SplitDownIntent: CallbackAction<_SplitDownIntent>(
                onInvoke: (_) {
                  _splitSelectedTerminalTab(TerminalSplitDirection.down);
                  return null;
                },
              ),
              _NewLocalTerminalIntent: CallbackAction<_NewLocalTerminalIntent>(
                onInvoke: (_) {
                  _openLocalTerminalTab();
                  return null;
                },
              ),
              _OpenSettingsIntent: CallbackAction<_OpenSettingsIntent>(
                onInvoke: (_) {
                  if (selectedTerminalPageActive) {
                    widget.onOpenTerminalSettings();
                  } else {
                    widget.onOpenSettings();
                  }
                  return null;
                },
              ),
            },
            child: Focus(
              focusNode: _workspaceFocusNode,
              autofocus: true,
              child: Scaffold(
                backgroundColor: context.nautermPalette.background,
                body: Stack(
                  children: [
                    Column(
                      children: [
                        _buildTopBar(chromeTerminalTab),
                        Expanded(
                          child: _buildWorkspaceContent(selectedTerminalTab),
                        ),
                      ],
                    ),
                    if (_notification != null)
                      _WorkspaceNotificationToast(
                        notification: _notification!,
                        onDismissed: _dismissWorkspaceNotification,
                      ),
                    if (_updateNotice != null)
                      _WorkspaceUpdateToast(notice: _updateNotice!),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
