part of 'nauterm_workspace.dart';

class _SftpPane extends ConsumerStatefulWidget {
  const _SftpPane({
    super.key,
    required this.sessionId,
    required this.active,
    required this.groups,
    required this.hosts,
    this.tags = const [],
    required this.dataStore,
    required this.connectRequest,
    required this.onHostSelected,
    this.onRemoteConnected,
    this.manageFileDrop = true,
    this.remoteOnly = false,
    this.sshEditorController,
    this.sshWorkingDirectoryResolver,
    this.onSshEditorOpened,
    this.onSshSelected,
    this.compact = false,
    this.panelColors,
  });

  final String sessionId;
  final bool active;
  final List<_GroupItem> groups;
  final List<_HostItem> hosts;
  final List<TagEntry> tags;
  final NautermDataStore? dataStore;
  final _SftpConnectRequest? connectRequest;
  final ValueChanged<_HostItem> onHostSelected;
  final void Function(_HostItem host, _SftpRemoteAuth auth)? onRemoteConnected;
  final bool manageFileDrop;
  final bool remoteOnly;
  final TerminalController? sshEditorController;
  final String? Function()? sshWorkingDirectoryResolver;
  final VoidCallback? onSshEditorOpened;
  final VoidCallback? onSshSelected;
  final bool compact;
  final _AiAssistantColors? panelColors;

  @override
  ConsumerState<_SftpPane> createState() => _SftpPaneState();
}

final _sftpTaskManagerProvider = ChangeNotifierProvider<_SftpTaskManager>(
  (ref) => _SftpTaskManager(),
);

final _sftpPaneControllerProvider =
    ChangeNotifierProvider.family<_SftpPaneController, String>(
      (ref, sessionId) =>
          _SftpPaneController(sessionId, ref.read(_sftpTaskManagerProvider)),
    );

class _SftpTaskManager extends ChangeNotifier {
  _SftpTaskManager() : downloadsDirectory = _prepareSftpDownloadsDirectory();

  int nextTaskId = 1;
  int nextHistoryTaskId = -1;
  List<_SftpTask> _tasks = const [];
  final Map<int, _SftpPaneState> _owners = <int, _SftpPaneState>{};
  final Map<int, _QueuedSftpTaskExecution> taskExecutions =
      <int, _QueuedSftpTaskExecution>{};
  final Map<int, Completer<_SftpTask>> taskCompletions =
      <int, Completer<_SftpTask>>{};
  final Set<int> runningTaskIds = <int>{};
  final Set<Future<void>> taskCleanupFutures = <Future<void>>{};
  bool historyLoaded = false;
  final Future<io.Directory?> downloadsDirectory;

  List<_SftpTask> get tasks => _tasks;

  set tasks(List<_SftpTask> value) {
    _tasks = value;
    notifyListeners();
  }

  void registerOwner(int taskId, _SftpPaneState owner) {
    _owners[taskId] = owner;
  }

  _SftpPaneState? ownerOf(int taskId) => _owners[taskId];

  void forgetOwner(int taskId) {
    _owners.remove(taskId);
  }

  bool isOwnedBy(int taskId, _SftpPaneState owner) {
    return identical(_owners[taskId], owner);
  }

  bool canRetry(_SftpTask task) {
    // Relayed transfers and edits can depend on staged files cleaned up by
    // their original caller after the transfer ends.
    if (task.status != _SftpTaskStatus.failed ||
        (task.type != _SftpTaskType.upload &&
            task.type != _SftpTaskType.download) ||
        runningTaskIds.contains(task.id)) {
      return false;
    }
    final owner = _owners[task.id];
    final execution = taskExecutions[task.id];
    final connection = execution?.remoteSession?.connection;
    return owner != null &&
        owner.mounted &&
        !owner._shuttingDown &&
        execution != null &&
        execution.persistHistory &&
        connection?.phase == _SftpConnectionPhase.connected &&
        identical(connection?.auth, execution.auth);
  }

  void pumpQueues() {
    for (final owner in _owners.values.toSet()) {
      owner._pumpSftpTaskQueue();
    }
  }
}

class _SftpRemotePaneSession {
  _SftpRemotePaneSession(this.sessionId) {
    pathController = TextEditingController(text: path);
    filterController = TextEditingController();
    pathFocusNode = FocusNode();
  }

  final String sessionId;
  String path = '~';
  late final TextEditingController pathController;
  late final TextEditingController filterController;
  late final FocusNode pathFocusNode;
  final List<String> backStack = <String>[];
  final List<String> forwardStack = <String>[];
  List<_SftpFileEntry> entries = const [];
  String? selectedPath;
  final Set<String> selectedPaths = <String>{};
  String? selectionAnchorPath;
  _SftpSortColumn sortColumn = _SftpSortColumn.name;
  bool sortAscending = true;
  bool loading = false;
  bool changingDirectory = false;
  bool showHiddenFiles = sftpShowHiddenFiles;
  bool editingPath = false;
  bool selectingHost = false;
  bool followSshDirectory = false;
  String? followedSshDirectory;
  final List<String> favoritePaths = <String>[];
  int favoriteLoadGeneration = 0;
  bool loadingFavorites = false;
  Object? favoritesError;
  int loadGeneration = 0;
  int navigationRevision = 0;
  final Map<int, int> destinationChecks = <int, int>{};
  _SftpListingCancellation? listingCancellation;
  _SftpConnectionState? connection;
  _SftpConnectRequest? closedEndpoint;
  _SftpRemoteEditOpening? editOpening;
  int _sudoGeneration = 0;
  String get sudoSessionId => '$sessionId:with-sudo:$_sudoGeneration';
  bool sudoSessionAuthenticated = false;
  Object? loadError;

  void closeSudoSession() {
    final id = sudoSessionId;
    _sudoGeneration += 1;
    sudoSessionAuthenticated = false;
    FfiSftpTaskExecutor.closeSudoSession(id);
  }

  void dispose() {
    listingCancellation?.cancel();
    listingCancellation = null;
    closeSudoSession();
    pathController.dispose();
    filterController.dispose();
    pathFocusNode.dispose();
  }
}

class _SftpRemoteEditOpening {
  _SftpRemoteEditOpening(this.filePath);

  final String filePath;
  int? taskId;
  _SftpListingCancellation? verification;
}

enum _SftpNameConflictResolution { cancel, keepBoth, replace }

enum _SftpSudoPromptKind { explicit, permissionDenied, saveEdit }

class _SftpResolvedRemoteTarget {
  const _SftpResolvedRemoteTarget({
    required this.targetPath,
    required this.replaceExisting,
  });

  final String targetPath;
  final bool replaceExisting;
}

class _SftpLocalPaneSession {
  _SftpLocalPaneSession(String initialPath) : path = initialPath {
    pathController = TextEditingController(text: path);
    filterController = TextEditingController();
    pathFocusNode = FocusNode();
  }

  String path;
  int navigationRevision = 0;
  int loadGeneration = 0;
  late final TextEditingController pathController;
  late final TextEditingController filterController;
  late final FocusNode pathFocusNode;
  final List<String> backStack = <String>[];
  final List<String> forwardStack = <String>[];
  List<_SftpFileEntry> entries = const [];
  String? selectedPath;
  final Set<String> selectedPaths = <String>{};
  String? selectionAnchorPath;
  _SftpSortColumn sortColumn = _SftpSortColumn.name;
  bool sortAscending = true;
  bool loading = true;
  bool changingDirectory = false;
  bool showHiddenFiles = sftpShowHiddenFiles;
  bool editingPath = false;
  bool hasUnrepresentableNames = false;
  Object? loadError;

  void dispose() {
    pathController.dispose();
    filterController.dispose();
    pathFocusNode.dispose();
  }
}

class _SftpPaneController extends ChangeNotifier {
  _SftpPaneController(this.sessionId, this.taskManager) {
    final defaultLocalPath = _defaultSftpPath();
    hostSearchController = TextEditingController();
    leftLocal = _SftpLocalPaneSession(defaultLocalPath);
    rightLocal = _SftpLocalPaneSession(defaultLocalPath);
    leftRemote = _SftpRemotePaneSession('$sessionId:left');
    rightRemote = _SftpRemotePaneSession('$sessionId:right');
    hostSearchController.addListener(notifyListeners);
    leftLocal.filterController.addListener(notifyListeners);
    rightLocal.filterController.addListener(notifyListeners);
    leftRemote.filterController.addListener(notifyListeners);
    rightRemote.filterController.addListener(notifyListeners);
    taskManager.addListener(notifyListeners);
  }

  final String sessionId;
  final _SftpTaskManager taskManager;
  late final TextEditingController hostSearchController;
  late final _SftpLocalPaneSession leftLocal;
  late final _SftpLocalPaneSession rightLocal;
  late final _SftpRemotePaneSession leftRemote;
  late final _SftpRemotePaneSession rightRemote;
  int? handledConnectRequestId;
  _SftpConnectRequest? retainedConnectRequest;
  SshConnectionProfile? boundToolProfile;
  int? boundToolHostId;
  String? boundToolHostName;
  int get nextSftpTaskId => taskManager.nextTaskId;

  set nextSftpTaskId(int value) {
    taskManager.nextTaskId = value;
  }

  int get nextSftpHistoryTaskId => taskManager.nextHistoryTaskId;

  set nextSftpHistoryTaskId(int value) {
    taskManager.nextHistoryTaskId = value;
  }

  List<_SftpTask> get tasks => taskManager.tasks;

  set tasks(List<_SftpTask> value) {
    taskManager.tasks = value;
  }

  Map<int, _QueuedSftpTaskExecution> get taskExecutions =>
      taskManager.taskExecutions;

  Map<int, Completer<_SftpTask>> get taskCompletions =>
      taskManager.taskCompletions;

  Set<int> get runningTaskIds => taskManager.runningTaskIds;

  Set<Future<void>> get taskCleanupFutures => taskManager.taskCleanupFutures;
  bool taskListOpen = false;
  _SftpPaneSlot taskListSlot = _SftpPaneSlot.right;
  bool favoriteListOpen = false;
  _SftpPaneSlot favoriteListSlot = _SftpPaneSlot.right;
  _SftpPaneEndpoint leftPaneEndpoint = _SftpPaneEndpoint.local;
  _SftpPaneEndpoint rightPaneEndpoint = _SftpPaneEndpoint.remote;
  _SftpPaneSlot? pendingConnectSlot;

  void mutate(VoidCallback update) {
    update();
    notifyListeners();
  }

  @override
  void dispose() {
    taskManager.removeListener(notifyListeners);
    hostSearchController.removeListener(notifyListeners);
    leftLocal.filterController.removeListener(notifyListeners);
    rightLocal.filterController.removeListener(notifyListeners);
    leftRemote.filterController.removeListener(notifyListeners);
    rightRemote.filterController.removeListener(notifyListeners);
    hostSearchController.dispose();
    leftLocal.dispose();
    rightLocal.dispose();
    leftRemote.dispose();
    rightRemote.dispose();
    super.dispose();
  }
}

class _ExternalEditLocalSnapshot {
  const _ExternalEditLocalSnapshot({
    required this.modified,
    required this.size,
  });

  final DateTime modified;
  final int size;

  @override
  bool operator ==(Object other) {
    return other is _ExternalEditLocalSnapshot &&
        other.modified == modified &&
        other.size == size;
  }

  @override
  int get hashCode => Object.hash(modified, size);
}

class _ExternalEditRemoteVersion {
  const _ExternalEditRemoteVersion({
    required this.modified,
    required this.size,
  });

  final DateTime modified;
  final int size;

  bool matches(_ExternalEditRemoteVersion other) {
    return size == other.size &&
        modified.millisecondsSinceEpoch ~/ 1000 ==
            other.modified.millisecondsSinceEpoch ~/ 1000;
  }
}

typedef _VerifiedSudoEdit = ({
  _ExternalEditRemoteVersion? version,
  String sessionId,
});

class _ExternalSftpEditTarget {
  _ExternalSftpEditTarget({required this.remotePath, required this.version});

  String remotePath;
  _ExternalEditRemoteVersion version;
}

enum _ExternalEditConflictResolution {
  cancel,
  keepRemote,
  overwriteRemote,
  saveCopy,
}

enum _ExternalEditSaveResult {
  failed,
  uploaded,
  localReplaced,
  authorizationPaused,
}

class _ExternalSftpEditSession {
  _ExternalSftpEditSession({
    required this.localPath,
    required this.tempDirectory,
    required this.auth,
    required this.protectFile,
    required this.onSaved,
    required this.onAuthorizationChanged,
  });

  final String localPath;
  final io.Directory tempDirectory;
  final _SftpRemoteAuth auth;
  final Future<bool> Function(String path) protectFile;
  final Future<_ExternalEditSaveResult> Function(String uploadPath) onSaved;
  final VoidCallback onAuthorizationChanged;
  static const _sampleInterval = Duration(milliseconds: 350);
  StreamSubscription<io.FileSystemEvent>? _watcher;
  _ExternalEditLocalSnapshot? _lastUploaded;
  _ExternalEditLocalSnapshot? _pendingUpload;
  Future<void>? _uploadWorker;
  int _sampleGeneration = 0;
  bool _closed = false;
  bool authorizationPaused = false;
  Future<void>? _closeFuture;

  Future<void> startWatching() async {
    _lastUploaded = await _snapshot();
    _watcher = tempDirectory.watch().listen((event) {
      if (io.File(event.path).absolute.path ==
          io.File(localPath).absolute.path) {
        _scheduleStableUpload();
      }
    });
  }

  void _scheduleStableUpload() {
    if (_closed) return;
    final generation = ++_sampleGeneration;
    unawaited(_waitForStableFile(generation));
  }

  Future<void> _waitForStableFile(int generation) async {
    _ExternalEditLocalSnapshot? previous;
    var stableSamples = 0;
    while (!_closed && generation == _sampleGeneration) {
      await Future<void>.delayed(_sampleInterval);
      if (_closed || generation != _sampleGeneration) return;
      final current = await _snapshot();
      if (current == null) return;
      if (current == previous) {
        stableSamples += 1;
      } else {
        previous = current;
        stableSamples = 1;
      }
      if (stableSamples >= 2) {
        if (!await protectFile(localPath)) return;
        if (current != _lastUploaded) _queueUpload(current);
        return;
      }
    }
  }

  void _queueUpload(_ExternalEditLocalSnapshot snapshot) {
    if (authorizationPaused) return;
    _pendingUpload = snapshot;
    _uploadWorker ??= _drainUploads().whenComplete(() {
      _uploadWorker = null;
      if (_pendingUpload case final pending? when pending != _lastUploaded) {
        _queueUpload(pending);
      }
    });
  }

  Future<void> retryPausedSave() async {
    if (!authorizationPaused || _closed || _closeFuture != null) return;
    final latest = await _snapshot();
    if (latest != null && !await protectFile(localPath)) return;
    if (!authorizationPaused || _closed || _closeFuture != null) return;
    authorizationPaused = false;
    onAuthorizationChanged();
    if (latest != null && latest != _lastUploaded) {
      _queueUpload(latest);
    }
  }

  Future<void> _drainUploads() async {
    while (true) {
      final snapshot = _pendingUpload;
      if (snapshot == null) return;
      _pendingUpload = null;
      if (snapshot == _lastUploaded) continue;
      final uploadFile = io.File(
        '${tempDirectory.path}${io.Platform.pathSeparator}'
        '.nauterm-upload-${DateTime.now().microsecondsSinceEpoch}',
      );
      try {
        await io.File(localPath).copy(uploadFile.path);
        final afterCopy = await _snapshot();
        if (afterCopy != snapshot) {
          if (afterCopy != null) _scheduleStableUpload();
          continue;
        }
        final result = await onSaved(uploadFile.path);
        switch (result) {
          case _ExternalEditSaveResult.failed:
            break;
          case _ExternalEditSaveResult.uploaded:
            _lastUploaded = snapshot;
          case _ExternalEditSaveResult.localReplaced:
            _lastUploaded = await _snapshot();
          case _ExternalEditSaveResult.authorizationPaused:
            authorizationPaused = true;
            _pendingUpload = null;
            onAuthorizationChanged();
            return;
        }
      } finally {
        try {
          await uploadFile.delete();
        } on Object {
          // Snapshot cleanup is best-effort.
        }
      }
    }
  }

  Future<_ExternalEditLocalSnapshot?> _snapshot() async {
    try {
      final file = io.File(localPath);
      return _ExternalEditLocalSnapshot(
        modified: await file.lastModified(),
        size: await file.length(),
      );
    } on Object {
      return null;
    }
  }

  Future<void> close() {
    return _closeFuture ??= _performClose();
  }

  Future<void> _performClose() async {
    if (_closed) return;
    await _watcher?.cancel();
    _watcher = null;
    _sampleGeneration += 1;

    final first = await _snapshot();
    await Future<void>.delayed(_sampleInterval);
    final second = await _snapshot();
    if (first != null && first == second) {
      if (!await protectFile(localPath)) return;
      if (second != _lastUploaded) _queueUpload(second!);
    }
    while (_uploadWorker != null || _pendingUpload != null) {
      final worker = _uploadWorker;
      if (worker != null) await worker;
    }
    _closed = true;
    final latest = await _snapshot();
    if (latest != null && latest != _lastUploaded) {
      return;
    }
    try {
      await tempDirectory.delete(recursive: true);
    } on Object {
      // Temp cleanup is best-effort.
    }
  }

  void dispose() {
    unawaited(close());
  }
}

class _SftpPaneState extends ConsumerState<_SftpPane> {
  static const _sftpTaskHistoryRetention = Duration(days: 30);
  late final String _controllerKey;
  late final _SftpPaneController _controller;
  final GlobalKey _leftPaneKey = GlobalKey();
  final GlobalKey _rightPaneKey = GlobalKey();
  StreamSubscription<NautermFileDropEvent>? _fileDropSubscription;
  _SftpPaneSlot? _fileDropHoverSlot;
  String? _fileDropHoverLabelKey;
  final Map<String, _ExternalSftpEditSession> _externalEditSessions =
      <String, _ExternalSftpEditSession>{};
  final Map<String, Future<void>> _externalEditUploadLocks =
      <String, Future<void>>{};
  bool _shuttingDown = false;
  bool _closingSftp = false;
  Future<void>? _shutdownFuture;
  bool _sshDirectoryFollowScheduled = false;
  bool _sshDirectoryFollowStopped = false;

  TextEditingController get _hostSearchController =>
      _controller.hostSearchController;

  _SftpLocalPaneSession get _leftLocal => _controller.leftLocal;

  _SftpLocalPaneSession get _rightLocal => _controller.rightLocal;

  _SftpRemotePaneSession get _leftRemote => _controller.leftRemote;

  _SftpRemotePaneSession get _rightRemote => _controller.rightRemote;

  int? get _handledConnectRequestId => _controller.handledConnectRequestId;

  set _handledConnectRequestId(int? value) {
    _controller.handledConnectRequestId = value;
  }

  int get _nextSftpTaskId => _controller.nextSftpTaskId;

  set _nextSftpTaskId(int value) {
    _controller.nextSftpTaskId = value;
  }

  List<_SftpTask> get _tasks => _controller.tasks;

  set _tasks(List<_SftpTask> value) {
    _controller.tasks = value;
  }

  _SftpPaneEndpoint get _leftPaneEndpoint => _controller.leftPaneEndpoint;

  set _leftPaneEndpoint(_SftpPaneEndpoint value) {
    if (_leftPaneEndpoint != value) {
      _leftLocal.navigationRevision += 1;
      _leftRemote.navigationRevision += 1;
    }
    _controller.leftPaneEndpoint = value;
  }

  _SftpPaneEndpoint get _rightPaneEndpoint => _controller.rightPaneEndpoint;

  set _rightPaneEndpoint(_SftpPaneEndpoint value) {
    if (_rightPaneEndpoint != value) {
      _rightLocal.navigationRevision += 1;
      _rightRemote.navigationRevision += 1;
    }
    _controller.rightPaneEndpoint = value;
  }

  _SftpPaneSlot? get _pendingConnectSlot => _controller.pendingConnectSlot;

  set _pendingConnectSlot(_SftpPaneSlot? value) {
    _controller.pendingConnectSlot = value;
  }

  @override
  void initState() {
    super.initState();
    _workspaceSftpPanes.add(this);
    _controllerKey = widget.sessionId;
    _controller = ref.read(_sftpPaneControllerProvider(_controllerKey));
    widget.sshEditorController?.workingDirectoryListenable.addListener(
      _scheduleSshDirectoryFollow,
    );
    NautermFileDropChannel.instance.ensureInitialized();
    _syncManagedFileDrop();
    _fileDropSubscription = NautermFileDropChannel.instance.events.listen(
      _handleFileDropEvent,
    );
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      _loadInitialState();
      _scheduleSshDirectoryFollow();
    });
  }

  void _loadInitialState() {
    _loadPersistedSftpFavorites();
    _loadPersistedSftpTaskHistory();
    if (!widget.remoteOnly) {
      unawaited(
        _loadLocalPath(_leftLocal, _leftLocal.path, recordHistory: false),
      );
      unawaited(
        _loadLocalPath(_rightLocal, _rightLocal.path, recordHistory: false),
      );
    }
    _handleConnectRequest(widget.connectRequest);
  }

  @override
  void didUpdateWidget(covariant _SftpPane oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.sshEditorController != oldWidget.sshEditorController) {
      oldWidget.sshEditorController?.workingDirectoryListenable.removeListener(
        _scheduleSshDirectoryFollow,
      );
      widget.sshEditorController?.workingDirectoryListenable.addListener(
        _scheduleSshDirectoryFollow,
      );
      _leftRemote.followedSshDirectory = null;
      _rightRemote.followedSshDirectory = null;
    }
    _scheduleSshDirectoryFollow();
    if (widget.active != oldWidget.active ||
        widget.manageFileDrop != oldWidget.manageFileDrop) {
      _syncManagedFileDrop();
    }
    if (widget.connectRequest?.id != oldWidget.connectRequest?.id) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) {
          return;
        }
        _handleConnectRequest(widget.connectRequest);
      });
    }
    if (widget.dataStore != oldWidget.dataStore) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) {
          return;
        }
        _loadPersistedSftpFavorites();
        _loadPersistedSftpTaskHistory();
      });
    }
  }

  void _loadPersistedSftpTaskHistory() {
    final store = widget.dataStore;
    if (store == null || _controller.taskManager.historyLoaded) return;
    try {
      final entries = store.listSftpTaskHistory(
        cutoff: DateTime.now().subtract(_sftpTaskHistoryRetention),
      );
      final history = <_SftpTask>[];
      for (final entry in entries) {
        final slot = _sftpHistorySlot(entry);
        final type = _sftpTaskTypeFromHistory(entry.type);
        final status = _sftpTaskStatusFromHistory(entry.status);
        if (type == null || status == null || entry.id == null) continue;
        history.add(
          _SftpTask(
            id: _controller.nextSftpHistoryTaskId--,
            nativeTaskId: -1,
            slot: slot,
            type: type,
            status: status,
            displayName: entry.displayName,
            sourcePath: entry.sourcePath,
            targetPath: entry.targetPath,
            createdAt: entry.createdAt,
            finishedAt: entry.finishedAt,
            bytes: entry.bytes,
            totalBytes: entry.totalBytes,
            itemKind: entry.itemKind,
            error: entry.error,
            historyId: entry.id,
          ),
        );
      }
      _setSftpState(() {
        _tasks = [
          for (final task in _tasks)
            if (task.historyId == null) task,
          ...history,
        ];
      });
      _controller.taskManager.historyLoaded = true;
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'sftp',
        'Unable to load SFTP task history.',
        error: error,
        stackTrace: stackTrace,
      );
    }
  }

  @override
  void dispose() {
    _sshDirectoryFollowStopped = true;
    _leftRemote.editOpening?.verification?.cancel();
    _rightRemote.editOpening?.verification?.cancel();
    _leftRemote.editOpening = null;
    _rightRemote.editOpening = null;
    widget.sshEditorController?.workingDirectoryListenable.removeListener(
      _scheduleSshDirectoryFollow,
    );
    _workspaceSftpPanes.remove(this);
    unawaited(_fileDropSubscription?.cancel());
    _leftRemote.listingCancellation?.cancel();
    _leftRemote.listingCancellation = null;
    _leftRemote.loading = false;
    _rightRemote.listingCancellation?.cancel();
    _rightRemote.listingCancellation = null;
    _rightRemote.loading = false;
    for (final session in _externalEditSessions.values) {
      session.dispose();
    }
    _externalEditSessions.clear();
    if (widget.manageFileDrop) {
      unawaited(NautermFileDropChannel.instance.setEnabled(false));
    }
    super.dispose();
  }

  Future<void> _flushAndClose() {
    return _shutdownFuture ??= _performFlushAndClose();
  }

  bool get _hasPendingWork =>
      _leftRemote.loading ||
      _rightRemote.loading ||
      _leftRemote.editOpening != null ||
      _rightRemote.editOpening != null ||
      _tasks.any((task) => !_isFinishedSftpTask(task));

  Future<void> _performFlushAndClose() async {
    _closingSftp = true;
    _sshDirectoryFollowStopped = true;
    _cancelRemoteEditOpening(_leftRemote);
    _cancelRemoteEditOpening(_rightRemote);
    widget.sshEditorController?.workingDirectoryListenable.removeListener(
      _scheduleSshDirectoryFollow,
    );
    await _fileDropSubscription?.cancel();
    _fileDropSubscription = null;
    _leftRemote.listingCancellation?.cancel();
    _leftRemote.listingCancellation = null;
    _rightRemote.listingCancellation?.cancel();
    _rightRemote.listingCancellation = null;

    try {
      await Future.wait([
        for (final session in _externalEditSessions.values) session.close(),
      ]).timeout(const Duration(seconds: 10));
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'sftp',
        'Unable to finish saving SFTP external edits before shutdown.',
        error: error,
        stackTrace: stackTrace,
      );
    }
    _externalEditSessions.clear();
    _shuttingDown = true;

    final pendingTasks = <Future<_SftpTask>>[];
    for (final task in _tasks.toList(growable: false)) {
      if (!_controller.taskManager.isOwnedBy(task.id, this)) {
        continue;
      }
      if (task.status != _SftpTaskStatus.queued &&
          task.status != _SftpTaskStatus.running &&
          task.status != _SftpTaskStatus.paused) {
        continue;
      }
      final completion = _controller.taskCompletions[task.id];
      if (completion != null) {
        pendingTasks.add(completion.future);
      }
      _cancelSftpTask(task.id);
    }
    if (pendingTasks.isNotEmpty) {
      try {
        await Future.wait(pendingTasks).timeout(const Duration(seconds: 10));
      } on TimeoutException catch (error, stackTrace) {
        NautermLog.warning(
          'sftp',
          'Timed out waiting for SFTP tasks during shutdown.',
          error: error,
          stackTrace: stackTrace,
          fields: {'task_count': pendingTasks.length},
        );
      }
    }
    final pendingCleanups = _controller.taskCleanupFutures.toList(
      growable: false,
    );
    if (pendingCleanups.isNotEmpty) {
      try {
        await Future.wait(pendingCleanups).timeout(const Duration(seconds: 10));
      } on TimeoutException catch (error, stackTrace) {
        NautermLog.warning(
          'sftp',
          'Timed out cleaning up cancelled SFTP tasks during shutdown.',
          error: error,
          stackTrace: stackTrace,
          fields: {'cleanup_count': pendingCleanups.length},
        );
      }
    }
  }

  void _syncManagedFileDrop() {
    if (!widget.manageFileDrop) {
      return;
    }
    unawaited(NautermFileDropChannel.instance.setEnabled(widget.active));
  }

  void _loadPersistedSftpFavorites() {
    final leftConnection = _leftRemote.connection;
    if (leftConnection != null) {
      unawaited(_loadRemoteSftpFavorites(_leftRemote, leftConnection.host));
    }
    final rightConnection = _rightRemote.connection;
    if (rightConnection != null) {
      unawaited(_loadRemoteSftpFavorites(_rightRemote, rightConnection.host));
    }
  }

  Future<void> _loadRemoteSftpFavorites(
    _SftpRemotePaneSession session,
    _HostItem host,
  ) async {
    final generation = ++session.favoriteLoadGeneration;
    final store = widget.dataStore;
    if (host.id <= 0) return;
    if (store == null) {
      _setSftpState(() => _resetSftpFavorites(session));
      return;
    }
    _setSftpState(() {
      session.loadingFavorites = true;
      session.favoritesError = null;
    });
    bool isCurrent() =>
        mounted &&
        identical(store, widget.dataStore) &&
        generation == session.favoriteLoadGeneration &&
        session.connection?.host.id == host.id &&
        session.connection?.host.uuid == host.uuid;
    try {
      final entries = await loadNautermRemoteSftpFavorites(
        NautermPaths.resolve().databasePath,
        host.id,
      );
      if (!isCurrent()) return;
      final paths = _favoritePathsFromEntries(entries);
      _setSftpState(() {
        _replaceFavoritePaths(session.favoritePaths, paths);
        session.loadingFavorites = false;
      });
    } on Object catch (error) {
      if (!isCurrent()) return;
      _setSftpState(() {
        session.loadingFavorites = false;
        session.favoritesError = error;
      });
    }
  }

  void _resetSftpFavorites(_SftpRemotePaneSession session) {
    session.favoriteLoadGeneration += 1;
    session.loadingFavorites = false;
    session.favoritesError = null;
    session.favoritePaths.clear();
  }

  List<String> _favoritePathsFromEntries(List<SftpFavoritePathEntry> entries) {
    final seen = <String>{};
    return [
      for (final entry in entries)
        if (entry.path.trim().isNotEmpty && seen.add(entry.path.trim()))
          entry.path.trim(),
    ];
  }

  void _replaceFavoritePaths(List<String> favoritePaths, List<String> paths) {
    favoritePaths
      ..clear()
      ..addAll(paths);
  }

  void _clearLocalSelection(_SftpLocalPaneSession session) {
    session.selectedPath = null;
    session.selectedPaths.clear();
    session.selectionAnchorPath = null;
  }

  void _clearRemoteSelection(_SftpRemotePaneSession session) {
    session.selectedPath = null;
    session.selectedPaths.clear();
    session.selectionAnchorPath = null;
  }

  void _setLocalSelection(
    _SftpLocalPaneSession session,
    _SftpSelectionChange selection,
  ) {
    session.selectedPath = selection.primaryPath;
    session.selectionAnchorPath = selection.anchorPath;
    session.selectedPaths
      ..clear()
      ..addAll(selection.selectedPaths);
  }

  void _setRemoteSelection(
    _SftpRemotePaneSession session,
    _SftpSelectionChange selection,
  ) {
    if (session.editOpening case final opening?
        when opening.filePath != selection.primaryPath ||
            selection.selectedPaths.length != 1) {
      _cancelRemoteEditOpening(
        session,
        message: tr(
          'sftp.edit.selectionChanged.description',
          fallback: 'Open cancelled because the selected file changed.',
        ),
      );
    }
    session.selectedPath = selection.primaryPath;
    session.selectionAnchorPath = selection.anchorPath;
    session.selectedPaths
      ..clear()
      ..addAll(selection.selectedPaths);
  }

  void _selectLocalEntry(_SftpLocalPaneSession session, _SftpFileEntry entry) {
    session.selectedPath = entry.path;
    session.selectionAnchorPath = entry.path;
    session.selectedPaths
      ..clear()
      ..add(entry.path);
  }

  void _selectRemoteEntry(
    _SftpRemotePaneSession session,
    _SftpFileEntry entry,
  ) {
    if (session.editOpening case final opening?
        when opening.filePath != entry.path) {
      _cancelRemoteEditOpening(
        session,
        message: tr(
          'sftp.edit.selectionChanged.description',
          fallback: 'Open cancelled because the selected file changed.',
        ),
      );
    }
    session.selectedPath = entry.path;
    session.selectionAnchorPath = entry.path;
    session.selectedPaths
      ..clear()
      ..add(entry.path);
  }

  void _setSftpState(VoidCallback update) {
    if (!mounted) {
      return;
    }
    _controller.mutate(update);
  }

  void _showEndpointSelector(_SftpPaneSlot slot) {
    final session = _remoteSessionForSlot(slot);
    _cancelRemoteEditOpening(session);
    _setSftpState(() {
      session.navigationRevision += 1;
      switch (slot) {
        case _SftpPaneSlot.left:
          _leftPaneEndpoint = _SftpPaneEndpoint.remote;
          _clearLocalSelection(_leftLocal);
        case _SftpPaneSlot.right:
          _rightPaneEndpoint = _SftpPaneEndpoint.remote;
          _clearLocalSelection(_rightLocal);
      }
      session.selectingHost = true;
    });
  }

  void _useLocalEndpoint(_SftpPaneSlot slot) {
    final session = _remoteSessionForSlot(slot);
    _cancelRemoteEditOpening(session);
    _setSftpState(() {
      switch (slot) {
        case _SftpPaneSlot.left:
          _leftPaneEndpoint = _SftpPaneEndpoint.local;
        case _SftpPaneSlot.right:
          _rightPaneEndpoint = _SftpPaneEndpoint.local;
      }
      session.selectingHost = false;
      session.followSshDirectory = false;
      session.followedSshDirectory = null;
      _clearRemoteSelection(session);
    });
  }

  void _handleEndpointHostSelected(_SftpPaneSlot slot, _HostItem host) {
    final session = _remoteSessionForSlot(slot);
    _setSftpState(() {
      switch (slot) {
        case _SftpPaneSlot.left:
          _leftPaneEndpoint = _SftpPaneEndpoint.remote;
        case _SftpPaneSlot.right:
          _rightPaneEndpoint = _SftpPaneEndpoint.remote;
      }
      session.selectingHost = false;
      _pendingConnectSlot = slot;
    });
    widget.onHostSelected(host);
  }

  void _closeLocalEndpoint(_SftpPaneSlot slot) {
    final session = _localSessionForSlot(slot);
    _setSftpState(() {
      switch (slot) {
        case _SftpPaneSlot.left:
          _leftPaneEndpoint = _SftpPaneEndpoint.remote;
        case _SftpPaneSlot.right:
          _rightPaneEndpoint = _SftpPaneEndpoint.remote;
      }
      _clearLocalSelection(session);
    });
  }

  _SftpLocalPaneSession _localSessionForSlot(_SftpPaneSlot slot) {
    return switch (slot) {
      _SftpPaneSlot.left => _leftLocal,
      _SftpPaneSlot.right => _rightLocal,
    };
  }

  _SftpRemotePaneSession _remoteSessionForSlot(_SftpPaneSlot? slot) {
    return switch (slot) {
      _SftpPaneSlot.left => _leftRemote,
      _SftpPaneSlot.right || null => _rightRemote,
    };
  }

  _SftpPaneSlot _slotForRemoteSession(_SftpRemotePaneSession session) {
    return identical(session, _leftRemote)
        ? _SftpPaneSlot.left
        : _SftpPaneSlot.right;
  }

  _SftpPaneLocation _sftpPaneLocation(
    _SftpPaneSlot slot,
    _SftpPaneEndpoint endpoint,
  ) {
    final local = _localSessionForSlot(slot);
    final remote = _remoteSessionForSlot(slot);
    final isLocal = endpoint == _SftpPaneEndpoint.local;
    return (
      sessionId: widget.sessionId,
      slot: slot,
      endpoint: endpoint,
      path: isLocal ? local.path : remote.path,
      revision: isLocal ? local.navigationRevision : remote.navigationRevision,
    );
  }

  bool _sftpPaneLocationIsCurrent(_SftpPaneLocation location) {
    if (!mounted || !widget.active || _closingSftp) return false;
    final endpoint = location.slot == _SftpPaneSlot.left
        ? _leftPaneEndpoint
        : _rightPaneEndpoint;
    if (endpoint != location.endpoint ||
        location != _sftpPaneLocation(location.slot, endpoint)) {
      return false;
    }
    if (endpoint == _SftpPaneEndpoint.local) {
      return !_localSessionForSlot(location.slot).changingDirectory;
    }
    final session = _remoteSessionForSlot(location.slot);
    return !session.selectingHost &&
        !session.changingDirectory &&
        session.connection?.phase == _SftpConnectionPhase.connected;
  }

  bool _sftpDragSourceIsCurrent(_SftpPaneDragPayload payload) {
    if (!_sftpPaneLocationIsCurrent(payload.source)) return false;
    final selected = payload.sourceRemote
        ? _selectedRemoteEntries(_remoteSessionForSlot(payload.source.slot))
        : _selectedEntries(_localSessionForSlot(payload.source.slot));
    return setEquals(
      {for (final entry in selected) (entry.path, entry.isDirectory)},
      {for (final entry in payload.entries) (entry.path, entry.isDirectory)},
    );
  }

  bool _checkSftpTransferScope(bool isCurrent, String message) {
    if (!isCurrent && mounted && widget.active && !_shuttingDown) {
      _showSftpSnack(message);
    }
    return isCurrent;
  }

  String get _sftpTransferScopeChanged => tr(
    'sftp.transfer.scopeChanged.description',
    fallback: 'Transfer cancelled because a pane or selection changed.',
  );

  void _focusPathInput(FocusNode focusNode, TextEditingController controller) {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      focusNode.requestFocus();
      controller.selection = TextSelection(
        baseOffset: 0,
        extentOffset: controller.text.length,
      );
    });
  }

  void _handleConnectRequest(_SftpConnectRequest? request) {
    if (request == null || request.id == _handledConnectRequestId) {
      return;
    }
    _controller.retainedConnectRequest = request;
    _handledConnectRequestId = request.id;
    final session = _remoteSessionForSlot(_pendingConnectSlot);
    _pendingConnectSlot = null;
    _startSftpConnection(request, session);
  }

  void _startSftpConnection(
    _SftpConnectRequest request,
    _SftpRemotePaneSession session,
  ) {
    NautermLog.info('sftp', 'SFTP connection started.');
    _cancelRemoteEditOpening(session);
    _closeRemoteSudoSession(session);
    session.loadGeneration += 1;
    _setSftpState(() {
      session.navigationRevision += 1;
      _resetSftpFavorites(session);
      session.closedEndpoint = null;
      session.selectingHost = false;
      session.connection = _SftpConnectionState.connecting(
        request.host,
        auth: request.auth,
      );
      session.followSshDirectory = false;
      session.followedSshDirectory = null;
      session.path = request.initialPath;
      session.pathController.text = session.path;
      session.entries = const [];
      _clearRemoteSelection(session);
      session.loadError = null;
      session.loading = true;
      session.editingPath = false;
      session.backStack.clear();
      session.forwardStack.clear();
    });
    unawaited(_loadRemoteSftpFavorites(session, request.host));

    unawaited(
      _loadRemotePath(
        session,
        request.auth,
        session.path,
        host: request.host,
        recordHistory: false,
        initial: true,
      ),
    );
  }

  void _retrySftpConnection(_SftpRemotePaneSession session) {
    final connection = session.connection;
    final auth = connection?.auth;
    if (connection == null || auth == null) {
      return;
    }
    _startSftpConnection(
      _SftpConnectRequest(
        id: _handledConnectRequestId ?? 0,
        host: connection.host,
        auth: auth,
        initialPath: session.path,
      ),
      session,
    );
  }

  void _trustAndRetrySftpConnection(
    _SftpRemotePaneSession session,
    SshHostKeyTrustMode hostKeyTrustMode,
  ) {
    final connection = session.connection;
    final auth = connection?.auth;
    if (connection == null || auth == null) {
      return;
    }
    _startSftpConnection(
      _SftpConnectRequest(
        id: _handledConnectRequestId ?? 0,
        host: connection.host,
        auth: auth.copyWith(hostKeyTrustMode: hostKeyTrustMode),
        initialPath: session.path,
      ),
      session,
    );
  }

  void _showSftpHostSelector(_SftpRemotePaneSession session) {
    _closeSftpConnectionPage(session);
    _setSftpState(() {
      session.selectingHost = true;
    });
  }

  void _closeSftpConnectionPage(_SftpRemotePaneSession session) {
    final connection = session.connection;
    final auth = connection?.auth;
    _cancelRemoteEditOpening(session);
    _closeRemoteSudoSession(session);
    session.listingCancellation?.cancel();
    session.listingCancellation = null;
    session.loadGeneration += 1;
    _setSftpState(() {
      session.navigationRevision += 1;
      _resetSftpFavorites(session);
      if (widget.compact && connection != null && auth != null) {
        session.closedEndpoint = _SftpConnectRequest(
          id: _handledConnectRequestId ?? 0,
          host: connection.host,
          auth: auth.copyWith(hostKeyTrustMode: SshHostKeyTrustMode.strict),
          initialPath: session.path,
        );
      }
      session.connection = null;
      session.selectingHost = false;
      session.entries = const [];
      _clearRemoteSelection(session);
      session.loadError = null;
      session.loading = false;
      session.changingDirectory = false;
      session.followSshDirectory = false;
      session.followedSshDirectory = null;
      final slot = _slotForRemoteSession(session);
      if (_controller.favoriteListSlot == slot) {
        _controller.favoriteListOpen = false;
      }
      if (_controller.taskListSlot == slot) {
        _controller.taskListOpen = false;
      }
    });
  }

  void _closeRemoteEndpoint(_SftpRemotePaneSession session) {
    _closeSftpConnectionPage(session);
  }

  void _closeRemoteSudoSession(_SftpRemotePaneSession session) {
    session.closeSudoSession();
  }

  Future<void> _loadLocalPath(
    _SftpLocalPaneSession session,
    String path, {
    bool recordHistory = true,
    bool clearForward = true,
  }) async {
    final normalized = _normalizeSftpPath(path);
    final generation = ++session.loadGeneration;
    _setSftpState(() {
      session.changingDirectory = normalized != session.path;
      if (session.changingDirectory) session.navigationRevision += 1;
      session.loading = true;
      session.loadError = null;
      session.editingPath = false;
    });

    try {
      final directory = io.Directory(normalized);
      final listing = await _listSftpDirectory(session, directory);
      if (!mounted || generation != session.loadGeneration) {
        return;
      }
      final showNameWarning =
          listing.hasUnrepresentableNames &&
          (normalized != session.path || !session.hasUnrepresentableNames);
      _setSftpState(() {
        if (recordHistory && normalized != session.path) {
          session.backStack.add(session.path);
          if (clearForward) {
            session.forwardStack.clear();
          }
        }
        session.path = normalized;
        session.pathController.text = normalized;
        session.entries = listing.entries;
        session.hasUnrepresentableNames = listing.hasUnrepresentableNames;
        _clearLocalSelection(session);
        session.loading = false;
        session.changingDirectory = false;
      });
      if (showNameWarning) {
        _showSftpSnack(context.tr('sftp.error.unrepresentableLocalName'));
      }
    } on Object catch (error) {
      if (!mounted || generation != session.loadGeneration) {
        return;
      }
      _setSftpState(() {
        session.loadError = error;
        session.loading = false;
        session.changingDirectory = false;
      });
    }
  }

  Future<({List<_SftpFileEntry> entries, bool hasUnrepresentableNames})>
  _listSftpDirectory(
    _SftpLocalPaneSession session,
    io.Directory directory,
  ) async {
    final entities = await directory.list(followLinks: false).toList();
    final entries = <_SftpFileEntry>[];
    final replacementPaths = <String>{};
    var hasUnrepresentableNames = false;
    final parent = directory.parent.path;
    if (parent != directory.path) {
      entries.add(
        _SftpFileEntry.parent(
          path: parent,
          separator: io.Platform.pathSeparator,
        ),
      );
    }

    for (final entity in entities) {
      final name = _basename(entity.path);
      if (!session.showHiddenFiles && name.startsWith('.')) {
        continue;
      }
      // dart:io replaces malformed filename bytes in the displayed path.
      // Probe the resulting path before exposing it to any file action, and
      // list a valid file with the same spelling only once.
      final needsRoundTrip = name.contains('\uFFFD');
      if (needsRoundTrip && !replacementPaths.add(entity.path)) {
        hasUnrepresentableNames = true;
        continue;
      }
      final stat = await entity.stat();
      final type = stat.type;
      if (needsRoundTrip &&
          type == io.FileSystemEntityType.notFound &&
          await io.FileSystemEntity.type(entity.path, followLinks: false) ==
              io.FileSystemEntityType.notFound) {
        hasUnrepresentableNames = true;
        continue;
      }
      entries.add(
        _SftpFileEntry(
          path: entity.path,
          name: name,
          modified: stat.modified,
          size: stat.size,
          kind: _sftpKindForName(
            name,
            isDirectory: type == io.FileSystemEntityType.directory,
            isLink: type == io.FileSystemEntityType.link,
          ),
          permissions: stat.modeString(),
          isDirectory: type == io.FileSystemEntityType.directory,
          isParent: false,
        ),
      );
    }

    entries.sort((a, b) {
      if (a.isParent != b.isParent) {
        return a.isParent ? -1 : 1;
      }
      if (a.isDirectory != b.isDirectory) {
        return a.isDirectory ? -1 : 1;
      }
      return a.name.toLowerCase().compareTo(b.name.toLowerCase());
    });
    return (entries: entries, hasUnrepresentableNames: hasUnrepresentableNames);
  }

  Future<void> _loadRemotePath(
    _SftpRemotePaneSession session,
    _SftpRemoteAuth auth,
    String path, {
    required _HostItem host,
    bool recordHistory = true,
    bool clearForward = true,
    bool initial = false,
    bool fromSshFollow = false,
  }) async {
    if (_closingSftp) return;
    final generation = ++session.loadGeneration;
    session.listingCancellation?.cancel();
    final cancellation = _SftpListingCancellation(_allocateSftpNativeTaskId());
    session.listingCancellation = cancellation;
    final normalized = _normalizeRemoteSftpPath(path, base: session.path);
    _cancelRemoteEditOpening(session);
    _setSftpState(() {
      session.changingDirectory = normalized != session.path;
      if (session.changingDirectory) session.navigationRevision += 1;
      if (!initial && !fromSshFollow && normalized != session.path) {
        session.followSshDirectory = false;
        session.followedSshDirectory = null;
      }
      session.loading = true;
      session.loadError = null;
      session.editingPath = false;
      if (initial) {
        session.connection = _SftpConnectionState.connecting(host, auth: auth);
      }
    });

    try {
      final listing = await _listRemoteSftpDirectory(
        session,
        auth,
        normalized,
        cancellation: cancellation,
      );
      if (cancellation.isCancelled) {
        return;
      }
      if (!mounted || generation != session.loadGeneration) {
        return;
      }
      _setSftpState(() {
        if (recordHistory && listing.path != session.path) {
          session.backStack.add(session.path);
          if (clearForward) {
            session.forwardStack.clear();
          }
        }
        session.path = listing.path;
        session.pathController.text = listing.path;
        session.entries = listing.entries;
        _clearRemoteSelection(session);
        session.loading = false;
        session.changingDirectory = false;
        session.connection = _SftpConnectionState.connected(host, auth);
      });
      if (initial) {
        NautermLog.info('sftp', 'SFTP connection established.');
        widget.onRemoteConnected?.call(host, auth);
      }
    } on Object catch (error, stackTrace) {
      if (cancellation.isCancelled) {
        return;
      }
      if (!mounted || generation != session.loadGeneration) {
        return;
      }
      final listingError = error is _SftpRemoteListingException ? error : null;
      if (initial) {
        NautermLog.warning(
          'sftp',
          'SFTP connection failed.',
          error: error,
          stackTrace: stackTrace,
          fields: {
            'host_key_event':
                listingError != null &&
                _hasSftpHostKeyEvent(listingError.events),
          },
        );
      }
      _setSftpState(() {
        session.loading = false;
        session.changingDirectory = false;
        if (initial) {
          if (listingError != null &&
              _hasSftpHostKeyEvent(listingError.events)) {
            session.connection = _SftpConnectionState.hostKey(
              host,
              listingError.message,
              auth: auth,
              fingerprint: _sftpLatestFingerprint(listingError.events),
            );
          } else {
            session.connection = _SftpConnectionState.failed(
              host,
              listingError?.message ?? '$error',
              auth: auth,
            );
          }
        } else {
          session.loadError = error;
        }
      });
    } finally {
      if (identical(session.listingCancellation, cancellation)) {
        session.listingCancellation = null;
      }
      _scheduleSshDirectoryFollow();
    }
  }

  int _allocateSftpNativeTaskId() {
    final sequence = _nextSftpTaskId++;
    return DateTime.now().microsecondsSinceEpoch + sequence;
  }

  Future<_SftpRemoteListing> _listRemoteSftpDirectory(
    _SftpRemotePaneSession session,
    _SftpRemoteAuth auth,
    String directory, {
    bool includeHidden = false,
    _SftpListingCancellation? cancellation,
    String? sudoSessionId,
    String? sudoPassword,
  }) async {
    final result = await _spawnSftpDirectoryEntryListing(
      auth.toArguments(directory)
        ..['sudoSessionId'] = sudoSessionId
        ..['sudoPassword'] = sudoPassword,
      requestId: cancellation?.requestId ?? _allocateSftpNativeTaskId(),
      cancellation: cancellation,
    );
    if (result.isError) {
      throw _SftpRemoteListingException(
        result.error ?? 'Failed to list remote directory.',
        result.events,
      );
    }
    final resolved = _emptyToNull(result.resolvedDirectory) ?? directory;
    final entries = <_SftpFileEntry>[];
    final parent = _remoteParentPath(resolved);
    if (parent != null) {
      entries.add(
        _SftpFileEntry.parent(path: parent, separator: _remoteSftpSeparator),
      );
    }
    for (final entry in result.entries) {
      final name = entry.name.trim();
      if (name.isEmpty ||
          (!includeHidden &&
              !session.showHiddenFiles &&
              name.startsWith('.'))) {
        continue;
      }
      entries.add(
        _SftpFileEntry(
          path: _joinRemoteSftpPath(resolved, name),
          name: name,
          modified: entry.modified ?? DateTime.fromMillisecondsSinceEpoch(0),
          size: entry.isDirectory ? 0 : entry.size,
          kind: _sftpKindForName(name, isDirectory: entry.isDirectory),
          permissions: entry.isDirectory ? 'drwxr-xr-x' : '-rw-r--r--',
          isDirectory: entry.isDirectory,
          isParent: false,
        ),
      );
    }
    entries.sort((a, b) {
      if (a.isParent != b.isParent) {
        return a.isParent ? -1 : 1;
      }
      if (a.isDirectory != b.isDirectory) {
        return a.isDirectory ? -1 : 1;
      }
      return a.name.toLowerCase().compareTo(b.name.toLowerCase());
    });
    return _SftpRemoteListing(path: resolved, entries: entries);
  }

  void _goBack(_SftpLocalPaneSession session) {
    if (session.backStack.isEmpty) {
      return;
    }
    final target = session.backStack.removeLast();
    session.forwardStack.add(session.path);
    unawaited(_loadLocalPath(session, target, recordHistory: false));
  }

  void _goLocalHome(_SftpLocalPaneSession session) {
    unawaited(_loadLocalPath(session, _localSftpHomePath()));
  }

  void _goForward(_SftpLocalPaneSession session) {
    if (session.forwardStack.isEmpty) {
      return;
    }
    final target = session.forwardStack.removeLast();
    session.backStack.add(session.path);
    unawaited(_loadLocalPath(session, target, recordHistory: false));
  }

  void _goRemoteBack(_SftpRemotePaneSession session) {
    if (session.backStack.isEmpty) {
      return;
    }
    final connection = session.connection;
    final auth = connection?.auth;
    if (connection == null || auth == null) {
      return;
    }
    final target = session.backStack.removeLast();
    session.forwardStack.add(session.path);
    unawaited(
      _loadRemotePath(
        session,
        auth,
        target,
        host: connection.host,
        recordHistory: false,
      ),
    );
  }

  void _goRemoteHome(_SftpRemotePaneSession session) {
    final connection = session.connection;
    final auth = connection?.auth;
    if (connection == null || auth == null) {
      return;
    }
    unawaited(_loadRemotePath(session, auth, '~', host: connection.host));
  }

  void _goRemoteSshDirectory(_SftpRemotePaneSession session) {
    if (!_remoteSessionSharesSshEditor(session)) return;
    final directory = _emptyToNull(widget.sshWorkingDirectoryResolver?.call());
    final connection = session.connection;
    final auth = connection?.auth;
    if (connection == null || auth == null) return;
    if (directory == null) {
      _showSftpSnack(
        tr(
          'sftp.message.sshDirectoryUnavailable',
          fallback: 'SSH current directory is not available yet.',
        ),
      );
      return;
    }
    unawaited(_loadRemotePath(session, auth, directory, host: connection.host));
  }

  void _toggleSshDirectoryFollow(_SftpRemotePaneSession session) {
    if (!session.followSshDirectory &&
        !_remoteSessionSharesSshEditor(session)) {
      return;
    }
    _setSftpState(() {
      session.followSshDirectory = !session.followSshDirectory;
      session.followedSshDirectory = null;
    });
    _scheduleSshDirectoryFollow();
  }

  void _scheduleSshDirectoryFollow() {
    if (!mounted ||
        _shuttingDown ||
        _sshDirectoryFollowStopped ||
        !widget.active ||
        _sshDirectoryFollowScheduled ||
        !(_leftRemote.followSshDirectory || _rightRemote.followSshDirectory)) {
      return;
    }
    _sshDirectoryFollowScheduled = true;
    scheduleMicrotask(() {
      _sshDirectoryFollowScheduled = false;
      if (!mounted ||
          _shuttingDown ||
          _sshDirectoryFollowStopped ||
          !widget.active) {
        return;
      }
      for (final session in [_leftRemote, _rightRemote]) {
        if (!session.followSshDirectory ||
            session.loading ||
            session.editingPath ||
            session.selectingHost ||
            !_remoteSessionSharesSshEditor(session)) {
          continue;
        }
        final directory = _emptyToNull(
          widget.sshWorkingDirectoryResolver?.call(),
        );
        if (directory == null || directory == session.followedSshDirectory) {
          continue;
        }
        session.followedSshDirectory = directory;
        if (_normalizeRemoteSftpPath(directory, base: session.path) ==
            session.path) {
          continue;
        }
        final connection = session.connection!;
        unawaited(
          _loadRemotePath(
            session,
            connection.auth!,
            directory,
            host: connection.host,
            recordHistory: false,
            fromSshFollow: true,
          ),
        );
      }
    });
  }

  void _goRemoteForward(_SftpRemotePaneSession session) {
    if (session.forwardStack.isEmpty) {
      return;
    }
    final connection = session.connection;
    final auth = connection?.auth;
    if (connection == null || auth == null) {
      return;
    }
    final target = session.forwardStack.removeLast();
    session.backStack.add(session.path);
    unawaited(
      _loadRemotePath(
        session,
        auth,
        target,
        host: connection.host,
        recordHistory: false,
      ),
    );
  }

  void _openEntry(
    _SftpLocalPaneSession session,
    _SftpFileEntry entry, {
    bool openFile = false,
  }) {
    if (entry.isDirectory || entry.isParent) {
      unawaited(_loadLocalPath(session, entry.path));
    } else if (openFile &&
        canOpenFileWithSystemDefaultApplication(
          entry.name,
          permissions: entry.permissions,
        )) {
      unawaited(_openLocalFileWithApplication(entry));
    }
  }

  void _openRemoteEntry(
    _SftpRemotePaneSession session,
    _SftpFileEntry entry, {
    bool openFile = false,
    bool withSudo = false,
  }) {
    if (!entry.isDirectory && !entry.isParent) {
      if (openFile &&
          canOpenFileWithSystemDefaultApplication(
            entry.name,
            permissions: entry.permissions,
          )) {
        unawaited(
          _openRemoteFileWithApplication(session, entry, withSudo: withSudo),
        );
      }
      return;
    }
    final connection = session.connection;
    final auth = connection?.auth;
    if (connection == null || auth == null) {
      return;
    }
    unawaited(
      _loadRemotePath(session, auth, entry.path, host: connection.host),
    );
  }

  bool _remoteEditSelectionIsCurrent(
    _SftpRemotePaneSession session,
    _SftpConnectionState? connection,
    String filePath,
  ) {
    return mounted &&
        widget.active &&
        !_closingSftp &&
        !session.selectingHost &&
        !session.loading &&
        connection?.phase == _SftpConnectionPhase.connected &&
        identical(connection, session.connection) &&
        session.selectedPath == filePath &&
        session.selectedPaths.length == 1;
  }

  void _cancelRemoteEditOpening(
    _SftpRemotePaneSession session, {
    String? message,
  }) {
    final opening = session.editOpening;
    if (opening == null) return;
    _setSftpState(() => session.editOpening = null);
    opening.verification?.cancel();
    if (opening.taskId case final taskId?) {
      _cancelSftpTask(taskId);
    }
    if (message != null && mounted && widget.active) {
      _showSftpSnack(message);
    }
  }

  Future<void> _openRemoteFileWithApplication(
    _SftpRemotePaneSession session,
    _SftpFileEntry entry, {
    SftpExternalEditorCommand? application,
    bool withSudo = false,
  }) async {
    if (application == null &&
        !canOpenFileWithSystemDefaultApplication(
          entry.name,
          permissions: entry.permissions,
        )) {
      return;
    }
    final connection = session.connection;
    final auth = connection?.auth;
    if (auth == null ||
        session.editOpening != null ||
        !_remoteEditSelectionIsCurrent(session, connection, entry.path)) {
      return;
    }
    final opening = _SftpRemoteEditOpening(entry.path);
    _setSftpState(() => session.editOpening = opening);
    bool canLaunch() =>
        identical(session.editOpening, opening) &&
        _remoteEditSelectionIsCurrent(session, connection, entry.path);

    io.Directory? tempDir;
    var watching = false;
    try {
      final directory = await io.Directory.systemTemp.createTemp(
        'nauterm-edit-',
      );
      tempDir = directory;
      if (!canLaunch() ||
          !await _ensurePrivateExternalEditDirectory(directory.path) ||
          !canLaunch()) {
        return;
      }
      final localPath = _joinSftpPath(directory.path, entry.name);
      final task = await _enqueueSftpTask(
        type: _SftpTaskType.download,
        slot: _slotForRemoteSession(session),
        displayName: tr(
          'sftp.edit.opening.description',
          fallback: 'Opening remote file…',
        ),
        sourcePath: entry.path,
        targetPath: localPath,
        auth: auth,
        operation: {
          'op': _SftpTaskType.download.wireName,
          'remote_path': entry.path,
          'local_path': localPath,
        },
        remoteSession: session,
        persistHistory: false,
        withSudo: withSudo,
        onQueued: (taskId) => opening.taskId = taskId,
      );
      if (task.status == _SftpTaskStatus.cancelled && canLaunch()) {
        _showSftpSnack(
          tr(
            'sftp.edit.openCancelled.description',
            fallback: 'Remote file opening cancelled.',
          ),
        );
      }
      if (canLaunch()) {
        _setSftpState(() => opening.taskId = null);
      }
      if (task.status != _SftpTaskStatus.completed ||
          !canLaunch() ||
          !await _ensurePrivateExternalEditFile(localPath) ||
          !canLaunch()) {
        return;
      }
      var remoteVersion = _ExternalEditRemoteVersion(
        modified: entry.modified,
        size: entry.size,
      );
      {
        final verification = _SftpListingCancellation(
          _allocateSftpNativeTaskId(),
        );
        opening.verification = verification;
        try {
          final version = await _readRemoteExternalEditVersion(
            session,
            auth,
            entry.path,
            cancellation: verification,
            sudoSessionId: withSudo ? session.sudoSessionId : null,
          );
          if (withSudo && version == null) {
            if (canLaunch()) {
              _showSftpSnack(tr('sftp.error.remoteFileMissing'));
            }
            return;
          }
          remoteVersion = version ?? remoteVersion;
        } on Object catch (error) {
          if (canLaunch()) {
            _showSftpSnack(
              tr(
                'sftp.error.verifyFile',
                fallback: 'Unable to verify {name}: {error}',
                args: {'name': entry.name, 'error': error},
              ),
            );
          }
          return;
        } finally {
          opening.verification = null;
        }
      }
      if (!canLaunch()) return;
      // Launch has been handed to the OS, so the opening can no longer be cancelled.
      _setSftpState(() => session.editOpening = null);
      if (!await _startExternalApplication(
            localPath,
            application: application,
          ) ||
          !mounted) {
        return;
      }
      final target = _ExternalSftpEditTarget(
        remotePath: entry.path,
        version: remoteVersion,
      );
      final editSession = _ExternalSftpEditSession(
        localPath: localPath,
        tempDirectory: directory,
        auth: auth,
        protectFile: _ensurePrivateExternalEditFile,
        onAuthorizationChanged: () => _setSftpState(() {}),
        onSaved: (uploadPath) async {
          try {
            return await _uploadEditedRemoteFile(
              session,
              auth,
              localPath,
              uploadPath,
              entry,
              target,
              withSudo: withSudo,
            );
          } on Object catch (error) {
            if (mounted) {
              _showSftpSnack(
                tr(
                  'sftp.error.saveFile',
                  fallback: 'Failed to save {name}: {error}',
                  args: {'name': entry.name, 'error': error},
                ),
              );
            }
            return _ExternalEditSaveResult.failed;
          }
        },
      );
      _externalEditSessions[localPath]?.dispose();
      _externalEditSessions[localPath] = editSession;
      watching = true;
      await editSession.startWatching();
      if (mounted) {
        _showSftpSnack(
          context.tr(
            'sftp.edit.opened.description',
            fallback: 'Opened a private copy for editing.',
          ),
        );
      }
    } on io.FileSystemException catch (error) {
      if (canLaunch()) {
        _showSftpSnack(
          tr(
            'sftp.error.openFile',
            fallback: 'Failed to open {path}: {error}',
            args: {'path': entry.path, 'error': error},
          ),
        );
      }
    } finally {
      if (identical(session.editOpening, opening)) {
        _setSftpState(() => session.editOpening = null);
      }
      if (!watching && tempDir != null) {
        try {
          await tempDir.delete(recursive: true);
        } on io.FileSystemException {
          // Temp cleanup is best-effort.
        }
      }
    }
  }

  Future<bool> _ensurePrivateExternalEditFile(String path) async {
    if (io.Platform.isWindows) return true;
    try {
      final result = await io.Process.run('/bin/chmod', ['600', path]);
      if (result.exitCode != 0) {
        throw io.FileSystemException(
          'chmod failed: ${result.stderr.toString().trim()}',
          path,
        );
      }
      return true;
    } on Object catch (error) {
      if (mounted) {
        _showSftpSnack(
          tr(
            'sftp.error.protectEditFile',
            fallback: 'Unable to protect temporary edit file: {error}',
            args: {'error': error},
          ),
        );
      }
      return false;
    }
  }

  Future<bool> _ensurePrivateExternalEditDirectory(String path) async {
    if (io.Platform.isWindows) return true;
    try {
      final result = await io.Process.run('/bin/chmod', ['700', path]);
      if (result.exitCode != 0) {
        throw io.FileSystemException(
          'chmod failed: ${result.stderr.toString().trim()}',
          path,
        );
      }
      return true;
    } on Object catch (error) {
      if (mounted) {
        _showSftpSnack(
          tr(
            'sftp.error.protectEditDirectory',
            fallback: 'Unable to protect temporary edit directory: {error}',
            args: {'error': error},
          ),
        );
      }
      return false;
    }
  }

  Future<void> _openLocalFileWithApplication(
    _SftpFileEntry entry, {
    SftpExternalEditorCommand? application,
  }) async {
    if (application == null &&
        !canOpenFileWithSystemDefaultApplication(
          entry.name,
          permissions: entry.permissions,
        )) {
      return;
    }
    if (!await io.File(entry.path).exists()) {
      if (mounted) {
        _showSftpSnack(
          tr(
            'sftp.error.fileMissing',
            fallback: 'File no longer exists: {path}',
            args: {'path': entry.path},
          ),
        );
      }
      return;
    }
    await _startExternalApplication(entry.path, application: application);
  }

  Future<void> _openLocalFileWithOtherApplication(
    _SftpLocalPaneSession session,
    _SftpPaneSlot slot,
    _SftpFileEntry entry,
  ) async {
    final path = session.path;
    final application = await chooseSystemFileApplication();
    if (application == null || !mounted || !widget.active || _shuttingDown) {
      return;
    }
    final endpoint = slot == _SftpPaneSlot.left
        ? _leftPaneEndpoint
        : _rightPaneEndpoint;
    if (endpoint != _SftpPaneEndpoint.local ||
        session.loading ||
        session.path != path ||
        session.selectedPath != entry.path ||
        session.selectedPaths.length != 1) {
      _showSftpSnack(
        tr(
          'sftp.edit.selectionChanged.description',
          fallback: 'Open cancelled because the selected file changed.',
        ),
      );
      return;
    }
    await _openLocalFileWithApplication(
      entry,
      application: application.command,
    );
  }

  Future<void> _openRemoteFileWithOtherApplication(
    _SftpRemotePaneSession session,
    _SftpFileEntry entry, {
    bool withSudo = false,
  }) async {
    final connection = session.connection;
    final application = await chooseSystemFileApplication();
    if (application == null || !mounted || !widget.active || _shuttingDown) {
      return;
    }
    if (!_remoteEditSelectionIsCurrent(session, connection, entry.path)) {
      _showSftpSnack(
        tr(
          'sftp.edit.selectionChanged.description',
          fallback: 'Open cancelled because the selected file changed.',
        ),
      );
      return;
    }
    await _openRemoteFileWithApplication(
      session,
      entry,
      application: application.command,
      withSudo: withSudo,
    );
  }

  Future<bool> _startExternalApplication(
    String path, {
    SftpExternalEditorCommand? application,
  }) async {
    try {
      await openFileWithSystemApplication(path, application: application);
      return true;
    } on Object catch (error) {
      if (mounted) {
        final target = application?.label ?? 'the default application';
        _showSftpSnack(
          tr(
            'sftp.error.openFile',
            fallback: 'Failed to open {path}: {error}',
            args: {'path': target, 'error': error},
          ),
        );
      }
      return false;
    }
  }

  Future<_ExternalEditSaveResult> _uploadEditedRemoteFile(
    _SftpRemotePaneSession session,
    _SftpRemoteAuth auth,
    String localPath,
    String uploadSourcePath,
    _SftpFileEntry entry,
    _ExternalSftpEditTarget target, {
    bool withSudo = false,
  }) async {
    final lockKey =
        '${auth.username}@${auth.host}:${auth.port}:'
        '${_collapseRemoteSftpPath(target.remotePath)}';
    final previous = _externalEditUploadLocks[lockKey];
    final release = Completer<void>();
    final lock = release.future;
    _externalEditUploadLocks[lockKey] = lock;
    if (previous != null) {
      try {
        await previous;
      } on Object {
        // A prior edit failure must not leave the path permanently locked.
      }
    }
    try {
      return await _performUploadEditedRemoteFile(
        session,
        auth,
        localPath,
        uploadSourcePath,
        entry,
        target,
        withSudo: withSudo,
      );
    } finally {
      release.complete();
      if (identical(_externalEditUploadLocks[lockKey], lock)) {
        _externalEditUploadLocks.remove(lockKey);
      }
    }
  }

  Future<_ExternalEditSaveResult> _performUploadEditedRemoteFile(
    _SftpRemotePaneSession session,
    _SftpRemoteAuth auth,
    String localPath,
    String uploadSourcePath,
    _SftpFileEntry entry,
    _ExternalSftpEditTarget target, {
    bool withSudo = false,
    bool retrySudoAuthorization = true,
  }) async {
    if (!mounted || _shuttingDown || (_closingSftp && withSudo)) {
      return _ExternalEditSaveResult.failed;
    }
    if (!await _ensurePrivateExternalEditFile(uploadSourcePath)) {
      return _ExternalEditSaveResult.failed;
    }

    final _ExternalEditRemoteVersion? currentVersion;
    final String? verifiedSudoId;
    try {
      if (withSudo) {
        final verified = await _verifySudoEdit(
          session,
          auth,
          target.remotePath,
        );
        if (verified == null) {
          if (mounted && !_closingSftp) {
            _showSftpSnack(context.tr('sftp.edit.status.localChangesKept'));
          }
          return _ExternalEditSaveResult.authorizationPaused;
        }
        verifiedSudoId = verified.sessionId;
        currentVersion = verified.version;
      } else {
        verifiedSudoId = null;
        currentVersion = await _readRemoteExternalEditVersion(
          session,
          auth,
          target.remotePath,
        );
      }
    } on Object catch (error) {
      _showSftpSnack(
        tr(
          'sftp.error.checkBeforeUpload',
          fallback: 'Unable to check {name} before upload: {error}',
          args: {'name': entry.name, 'error': error},
        ),
      );
      return _ExternalEditSaveResult.failed;
    }

    var remoteUploadPath = target.remotePath;
    var replaceExisting = !withSudo || currentVersion != null;
    if (currentVersion == null || !target.version.matches(currentVersion)) {
      if (_closingSftp) return _ExternalEditSaveResult.failed;
      final resolution = await _promptExternalEditConflict(
        remotePath: target.remotePath,
        remoteMissing: currentVersion == null,
      );
      if (!mounted) return _ExternalEditSaveResult.failed;
      if (withSudo && resolution != _ExternalEditConflictResolution.cancel) {
        try {
          final latestVersion = await _readRemoteExternalEditVersion(
            session,
            auth,
            target.remotePath,
            sudoSessionId: verifiedSudoId,
          );
          final unchanged = currentVersion == null
              ? latestVersion == null
              : latestVersion != null && currentVersion.matches(latestVersion);
          if (!unchanged) {
            _showSftpSnack(tr('sftp.message.uploadPaused'));
            return _ExternalEditSaveResult.failed;
          }
        } on _SftpRemoteListingException catch (error) {
          if (_sftpSudoAuthenticationFailed(error.message)) {
            _showSftpSnack(tr('sftp.edit.status.localChangesKept'));
            return _ExternalEditSaveResult.authorizationPaused;
          }
          _showSftpSnack(
            tr(
              'sftp.error.checkBeforeUpload',
              args: {'name': entry.name, 'error': error},
            ),
          );
          return _ExternalEditSaveResult.failed;
        }
      }
      switch (resolution) {
        case _ExternalEditConflictResolution.cancel:
          _showSftpSnack(
            tr(
              'sftp.message.uploadPaused',
              fallback: 'Upload paused. The remote file is unchanged.',
            ),
          );
          return _ExternalEditSaveResult.failed;
        case _ExternalEditConflictResolution.keepRemote:
          if (currentVersion == null) {
            _showSftpSnack(
              tr(
                'sftp.error.remoteFileMissing',
                fallback: 'The remote file no longer exists.',
              ),
            );
            return _ExternalEditSaveResult.failed;
          }
          final kept = await _downloadRemoteEditVersion(
            session,
            auth,
            target.remotePath,
            localPath,
            entry.name,
            verifiedSudoSessionId: verifiedSudoId,
          );
          if (kept) {
            target.version = currentVersion;
            _showSftpSnack(
              tr(
                'sftp.message.keptRemoteVersion',
                fallback: 'Kept the remote version of {name}.',
                args: {'name': entry.name},
              ),
            );
          }
          return kept
              ? _ExternalEditSaveResult.localReplaced
              : _ExternalEditSaveResult.failed;
        case _ExternalEditConflictResolution.overwriteRemote:
          break;
        case _ExternalEditConflictResolution.saveCopy:
          final existingTargets = _remoteSftpSessionSiblingTargets(
            session,
            target.remotePath,
          );
          remoteUploadPath = _uniqueRemoteSftpPath(
            target.remotePath,
            existingTargets,
          );
          replaceExisting = false;
      }
    }

    final uploadName = _remoteSftpName(remoteUploadPath);
    final task = await _enqueueSftpTask(
      type: _SftpTaskType.edit,
      slot: _slotForRemoteSession(session),
      displayName: 'Save $uploadName',
      sourcePath: localPath,
      targetPath: remoteUploadPath,
      auth: auth,
      operation: {
        'op': _SftpTaskType.upload.wireName,
        'local_path': uploadSourcePath,
        'remote_path': remoteUploadPath,
        'replace_existing': replaceExisting,
        if (withSudo && replaceExisting && currentVersion != null)
          'expected_remote_version': {
            'modified': currentVersion.modified.millisecondsSinceEpoch ~/ 1000,
            'size': currentVersion.size,
          },
      },
      refreshRemote: true,
      remoteSession: session,
      withSudo: withSudo,
      verifiedSudoSessionId: verifiedSudoId,
    );
    if (!mounted) return _ExternalEditSaveResult.failed;
    if (task.status == _SftpTaskStatus.cancelled) {
      _showSftpSnack(context.tr('sftp.edit.status.localChangesKept'));
      return _ExternalEditSaveResult.failed;
    }
    if (task.status != _SftpTaskStatus.completed) {
      if (withSudo &&
          retrySudoAuthorization &&
          !_closingSftp &&
          _sftpSudoAuthenticationFailed(task.error)) {
        return _performUploadEditedRemoteFile(
          session,
          auth,
          localPath,
          uploadSourcePath,
          entry,
          target,
          withSudo: true,
          retrySudoAuthorization: false,
        );
      }
      _showSftpSnack(
        tr(
          'sftp.error.uploadFile',
          fallback: 'Failed to upload {name}: {error}',
          args: {
            'name': uploadName,
            'error':
                task.error ??
                tr('common.error.unknown', fallback: 'Unknown error'),
          },
        ),
      );
      return _ExternalEditSaveResult.failed;
    }

    try {
      final uploadedVersion = await _readRemoteExternalEditVersion(
        session,
        auth,
        remoteUploadPath,
        sudoSessionId: verifiedSudoId,
      );
      if (uploadedVersion != null) target.version = uploadedVersion;
    } on Object catch (error) {
      _showSftpSnack(
        tr(
          'sftp.error.verifyAfterSave',
          fallback: 'Saved {name}. Verification failed: {error}',
          args: {'name': uploadName, 'error': error},
        ),
      );
    }
    if (remoteUploadPath != target.remotePath) {
      target.remotePath = remoteUploadPath;
      _showSftpSnack(
        tr(
          'sftp.message.savedEditedFile',
          fallback: 'Saved the edited file as {name}.',
          args: {'name': uploadName},
        ),
      );
    }
    return _ExternalEditSaveResult.uploaded;
  }

  Future<_VerifiedSudoEdit?> _verifySudoEdit(
    _SftpRemotePaneSession session,
    _SftpRemoteAuth auth,
    String path,
  ) async {
    bool connectionIsCurrent() =>
        mounted && !_closingSftp && identical(session.connection?.auth, auth);
    if (!connectionIsCurrent()) return null;

    final cachedId = session.sudoSessionId;
    try {
      final version = await _readRemoteExternalEditVersion(
        session,
        auth,
        path,
        sudoSessionId: cachedId,
      );
      if (!connectionIsCurrent() || session.sudoSessionId != cachedId) {
        return null;
      }
      return (version: version, sessionId: cachedId);
    } on _SftpRemoteListingException catch (error) {
      if (!_sftpSudoAuthenticationFailed(error.message)) rethrow;
      if (!connectionIsCurrent() || session.sudoSessionId != cachedId) {
        return null;
      }
      session.closeSudoSession();
    }

    final revision = session.navigationRevision;
    final sudoId = session.sudoSessionId;
    bool paneIsCurrent() =>
        connectionIsCurrent() &&
        session.navigationRevision == revision &&
        session.sudoSessionId == sudoId;
    return _showSftpSudoPrompt<_VerifiedSudoEdit>(
      kind: _SftpSudoPromptKind.saveEdit,
      target: tr(
        'sftp.sudo.explicit.dialog.target',
        args: {'user': auth.username, 'host': auth.host},
      ),
      onSubmit: (password) async {
        if (!paneIsCurrent()) {
          _showSftpSnack(tr('sftp.sudo.error.paneChanged'));
          return null;
        }
        _ExternalEditRemoteVersion? version;
        try {
          version = await _readRemoteExternalEditVersion(
            session,
            auth,
            path,
            sudoSessionId: sudoId,
            sudoPassword: password,
          );
        } on Object {
          if (paneIsCurrent()) rethrow;
        }
        if (!paneIsCurrent()) {
          if (session.sudoSessionId == sudoId) {
            session.closeSudoSession();
          } else {
            FfiSftpTaskExecutor.closeSudoSession(sudoId);
          }
          _showSftpSnack(tr('sftp.sudo.error.paneChanged'));
          return null;
        }
        session.sudoSessionAuthenticated = true;
        return (version: version, sessionId: sudoId);
      },
    );
  }

  Future<_ExternalEditRemoteVersion?> _readRemoteExternalEditVersion(
    _SftpRemotePaneSession session,
    _SftpRemoteAuth auth,
    String remotePath, {
    _SftpListingCancellation? cancellation,
    String? sudoSessionId,
    String? sudoPassword,
  }) async {
    final parent = _remoteParentPath(remotePath) ?? session.path;
    final listing = await _listRemoteSftpDirectory(
      session,
      auth,
      parent,
      includeHidden: true,
      cancellation: cancellation,
      sudoSessionId: sudoSessionId,
      sudoPassword: sudoPassword,
    );
    final normalized = _collapseRemoteSftpPath(remotePath);
    final match = listing.entries.where((candidate) {
      return !candidate.isParent &&
          _collapseRemoteSftpPath(candidate.path) == normalized;
    }).firstOrNull;
    if (match == null || match.isDirectory) return null;
    return _ExternalEditRemoteVersion(
      modified: match.modified,
      size: match.size,
    );
  }

  Future<bool> _downloadRemoteEditVersion(
    _SftpRemotePaneSession session,
    _SftpRemoteAuth auth,
    String remotePath,
    String localPath,
    String displayName, {
    String? verifiedSudoSessionId,
  }) async {
    final task = await _enqueueSftpTask(
      type: _SftpTaskType.download,
      slot: _slotForRemoteSession(session),
      displayName: 'Keep remote $displayName',
      sourcePath: remotePath,
      targetPath: localPath,
      auth: auth,
      operation: {
        'op': _SftpTaskType.download.wireName,
        'remote_path': remotePath,
        'local_path': localPath,
      },
      refreshLocal: false,
      remoteSession: session,
      persistHistory: false,
      withSudo: verifiedSudoSessionId != null,
      verifiedSudoSessionId: verifiedSudoSessionId,
    );
    if (task.status != _SftpTaskStatus.completed) {
      if (mounted) {
        _showSftpSnack(
          tr(
            'sftp.error.keepRemoteVersion',
            fallback: 'Failed to keep the remote version: {error}',
            args: {
              'error':
                  task.error ??
                  tr('common.error.unknown', fallback: 'Unknown error'),
            },
          ),
        );
      }
      return false;
    }
    return _ensurePrivateExternalEditFile(localPath);
  }

  Future<_ExternalEditConflictResolution> _promptExternalEditConflict({
    required String remotePath,
    required bool remoteMissing,
  }) async {
    if (!mounted) return _ExternalEditConflictResolution.cancel;
    final result =
        await _showSftpWorkspaceDialog<_ExternalEditConflictResolution>(
          barrierDismissible: false,
          builder: (dialogContext) {
            final colors = widget.compact ? widget.panelColors : null;
            return _WorkspaceDialogFrame(
              width: 540,
              title: Text(
                tr(
                  'sftp.label.remoteFileChanged',
                  fallback: 'Remote file changed',
                ),
              ),
              content: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    remoteMissing
                        ? dialogContext.tr(
                            'sftp.edit.conflict.removed.description',
                            fallback: 'The file was removed from the server after editing began.',
                          )
                        : dialogContext.tr(
                            'sftp.edit.conflict.changed.description',
                            fallback: 'The file was changed on the server after editing began.',
                          ),
                    style: TextStyle(
                      color: colors?.foreground ?? _text,
                      fontSize: NautermFontSizes.labelLarge,
                      fontWeight: NautermFontWeights.regular,
                      height: 1.35,
                      letterSpacing: 0,
                    ),
                  ),
                  SizedBox(height: 10),
                  Text(
                    remotePath,
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: colors?.muted ?? _mutedText,
                      fontFamily: 'monospace',
                      fontSize: NautermFontSizes.labelMedium,
                      fontWeight: NautermFontWeights.regular,
                      letterSpacing: 0,
                    ),
                  ),
                  const SizedBox(height: 10),
                  Text(
                    dialogContext.tr(
                      'sftp.edit.conflict.localCopy.description',
                      fallback: 'Your local edits are retained until you choose a resolution.',
                    ),
                    style: TextStyle(
                      color: colors?.muted ?? _mutedText,
                      fontSize: NautermFontSizes.labelMedium,
                      letterSpacing: 0,
                    ),
                  ),
                ],
              ),
              actions: [
                _WorkspaceButton(
                  label: 'common.action.cancel',
                  variant: _WorkspaceButtonVariant.text,
                  onPressed: () =>
                      Navigator.of(dialogContext)
                          .pop(_ExternalEditConflictResolution.cancel),
                ),
                if (!remoteMissing)
                  _WorkspaceButton(
                    label: 'sftp.label.keepRemote',
                    variant: _WorkspaceButtonVariant.filled,
                    onPressed: () =>
                        Navigator.of(dialogContext)
                            .pop(_ExternalEditConflictResolution.keepRemote),
                  ),
                _WorkspaceButton(
                  label: 'sftp.label.saveACopy',
                  variant: _WorkspaceButtonVariant.filled,
                  onPressed: () =>
                      Navigator.of(dialogContext)
                          .pop(_ExternalEditConflictResolution.saveCopy),
                ),
                _WorkspaceButton(
                  label: 'sftp.label.overwriteRemote',
                  type: _WorkspaceButtonType.error,
                  variant: _WorkspaceButtonVariant.solid,
                  onPressed: () =>
                      Navigator.of(dialogContext)
                          .pop(_ExternalEditConflictResolution.overwriteRemote),
                ),
              ],
            );
          },
        );
    return result ?? _ExternalEditConflictResolution.cancel;
  }

  void _openRemoteFileWithSshEditor(
    _SftpRemotePaneSession session,
    _SftpFileEntry entry, {
    bool withSudo = false,
  }) {
    final controller = widget.sshEditorController;
    final editor = sftpSshEditor.trim();
    if (controller == null ||
        editor.isEmpty ||
        !_remoteSessionSharesSshEditor(session)) {
      return;
    }
    final command = withSudo ? 'sudo -- $editor' : editor;
    controller.sendInput('$command ${_shellQuoteRemotePath(entry.path)}\r');
    widget.onSshEditorOpened?.call();
  }

  bool _remoteSessionSharesSshEditor(_SftpRemotePaneSession session) {
    final controller = widget.sshEditorController;
    final profile = controller?.sshProfile;
    final connection = session.connection;
    final auth = connection?.auth;
    if (controller == null ||
        profile == null ||
        auth == null ||
        controller.connectionStatus.phase !=
            TerminalConnectionPhase.connected) {
      return false;
    }
    if (connection?.host.id != null &&
        profile.hostId != null &&
        connection!.host.id != profile.hostId) {
      return false;
    }
    return auth.host == profile.host &&
        auth.port == profile.port &&
        auth.username == profile.username;
  }

  String _shellQuoteRemotePath(String value) {
    return "'${value.replaceAll("'", "'\"'\"'")}'";
  }

  void _showSftpSnack(String message) {
    if (!mounted) return;
    ScaffoldMessenger.maybeOf(context)?.showSnackBar(
      SnackBar(content: Text(message), duration: const Duration(seconds: 3)),
    );
  }

  Widget? _sftpOperationBanner(
    _SftpRemotePaneSession session, {
    _AiAssistantColors? colors,
  }) {
    final opening = session.editOpening;
    if (opening == null &&
        !session.destinationChecks.containsKey(session.navigationRevision)) {
      final paused = _externalEditSessions.values
          .where(
            (edit) =>
                edit.authorizationPaused &&
                identical(edit.auth, session.connection?.auth),
          )
          .firstOrNull;
      if (paused == null || _closingSftp) return null;
      return Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10),
        child: Row(
          children: [
            Expanded(
              child: Tooltip(
                message: tr(
                  'sftp.edit.notice.localCopy',
                  args: {
                    'error': tr('sftp.edit.status.localChangesKept'),
                    'path': paused.localPath,
                  },
                ),
                child: Text(
                  tr(
                    'sftp.edit.status.savePaused',
                    args: {'host': paused.auth.host},
                  ),
                  style: TextStyle(
                    color: colors?.foreground ?? _text,
                    fontSize: NautermFontSizes.labelSmall,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ),
            TextButton(
              onPressed: () => unawaited(paused.retryPausedSave()),
              child: Text(tr('common.action.authenticate')),
            ),
          ],
        ),
      );
    }
    final label = switch (opening) {
      null => tr(
        'sftp.transfer.checkingDestination.description',
        fallback: 'Checking destination…',
      ),
      _SftpRemoteEditOpening(taskId: null) => tr(
        'sftp.edit.opening.description',
        fallback: 'Opening remote file…',
      ),
      _ => tr(
        'sftp.edit.downloading.description',
        fallback: 'Downloading a private copy for editing…',
      ),
    };
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 10),
      child: SizedBox(
        height: 32,
        child: Row(
          children: [
            SizedBox.square(
              dimension: 12,
              child: CircularProgressIndicator(
                strokeWidth: 1.5,
                color: colors?.accent,
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                label,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: colors?.foreground ?? _text,
                  fontSize: NautermFontSizes.labelSmall,
                ),
              ),
            ),
            if (opening != null)
              TextButton(
                style: TextButton.styleFrom(
                  foregroundColor: colors?.foreground ?? _text,
                  minimumSize: const Size(0, 32),
                  padding: const EdgeInsets.symmetric(horizontal: 8),
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                  textStyle: const TextStyle(
                    fontSize: NautermFontSizes.labelSmall,
                  ),
                ),
                onPressed: () => _cancelRemoteEditOpening(
                  session,
                  message: tr(
                    'sftp.edit.openCancelled.description',
                    fallback: 'Remote file opening cancelled.',
                  ),
                ),
                child: Text(tr('common.action.cancel', fallback: 'Cancel')),
              ),
          ],
        ),
      ),
    );
  }

  void _sortLocalEntriesBy(
    _SftpLocalPaneSession session,
    _SftpSortColumn column,
  ) {
    _setSftpState(() {
      if (session.sortColumn == column) {
        session.sortAscending = !session.sortAscending;
      } else {
        session.sortColumn = column;
        session.sortAscending = true;
      }
    });
  }

  void _sortRemoteEntriesBy(
    _SftpRemotePaneSession session,
    _SftpSortColumn column,
  ) {
    _setSftpState(() {
      if (session.sortColumn == column) {
        session.sortAscending = !session.sortAscending;
      } else {
        session.sortColumn = column;
        session.sortAscending = true;
      }
    });
  }

  void _selectAll(_SftpLocalPaneSession session) {
    final selectable = [
      for (final entry in session.entries)
        if (!entry.isParent) entry,
    ];
    if (selectable.isEmpty) {
      return;
    }
    _setSftpState(() {
      session.selectedPath = selectable.first.path;
      session.selectionAnchorPath = selectable.first.path;
      session.selectedPaths
        ..clear()
        ..addAll(selectable.map((entry) => entry.path));
    });
  }

  void _toggleHiddenFiles(_SftpLocalPaneSession session) {
    _setSftpState(() {
      session.showHiddenFiles = !session.showHiddenFiles;
    });
    unawaited(_loadLocalPath(session, session.path, recordHistory: false));
  }

  void _toggleRemoteHiddenFiles(_SftpRemotePaneSession session) {
    final connection = session.connection;
    final auth = connection?.auth;
    if (connection == null || auth == null) {
      return;
    }
    _setSftpState(() {
      session.showHiddenFiles = !session.showHiddenFiles;
    });
    unawaited(
      _loadRemotePath(
        session,
        auth,
        session.path,
        host: connection.host,
        recordHistory: false,
      ),
    );
  }

  Future<void> _createFolder(_SftpLocalPaneSession session) async {
    final defaultName = tr('sftp.action.newFolder', fallback: 'New Folder');
    var name = defaultName;
    var candidate = _joinSftpPath(session.path, name);
    var suffix = 2;
    while (await io.Directory(candidate).exists()) {
      name = '$defaultName $suffix';
      candidate = _joinSftpPath(session.path, name);
      suffix += 1;
    }
    await io.Directory(candidate).create();
    await _loadLocalPath(session, session.path, recordHistory: false);
    _setSftpState(() {
      _selectLocalEntry(
        session,
        _SftpFileEntry(
          path: candidate,
          name: _basename(candidate),
          modified: DateTime.fromMillisecondsSinceEpoch(0),
          size: 0,
          kind: _sftpKindForName(_basename(candidate), isDirectory: true),
          permissions: 'drwxr-xr-x',
          isDirectory: true,
          isParent: false,
        ),
      );
    });
  }

  Future<_SftpTask> _enqueueSftpTask({
    required _SftpTaskType type,
    required _SftpPaneSlot slot,
    required String displayName,
    required String sourcePath,
    required String targetPath,
    required Map<String, Object?> operation,
    required _SftpRemoteAuth auth,
    bool refreshLocal = false,
    bool refreshRemote = false,
    _SftpLocalPaneSession? localSession,
    _SftpRemotePaneSession? remoteSession,
    bool persistHistory = true,
    bool withSudo = false,
    String? verifiedSudoSessionId,
    ValueChanged<int>? onQueued,
  }) {
    if (_shuttingDown || (_closingSftp && type != _SftpTaskType.edit)) {
      return Future.value(
        _SftpTask(
          id: -1,
          nativeTaskId: -1,
          slot: slot,
          type: type,
          status: _SftpTaskStatus.cancelled,
          displayName: displayName,
          sourcePath: sourcePath,
          targetPath: targetPath,
          createdAt: DateTime.now(),
          error: 'Nauterm is shutting down.',
        ),
      );
    }
    final id = _nextSftpTaskId++;
    final nativeTaskId = DateTime.now().microsecondsSinceEpoch + id;
    final task = _SftpTask(
      id: id,
      nativeTaskId: nativeTaskId,
      slot: slot,
      type: type,
      status: _SftpTaskStatus.queued,
      displayName: displayName,
      sourcePath: sourcePath,
      targetPath: targetPath,
      createdAt: DateTime.now(),
    );
    _setSftpState(() {
      _tasks = [..._tasks, task];
    });
    final completer = Completer<_SftpTask>();
    final host = remoteSession?.connection?.host;
    final hostUuid = host != null && host.id > 0 ? host.uuid : null;
    _controller.taskCompletions[id] = completer;
    _controller.taskExecutions[id] = _QueuedSftpTaskExecution(
      nativeTaskId: nativeTaskId,
      auth: auth,
      operation: {...operation, 'transfer_threads': sftpTransferThreads},
      refreshLocal: refreshLocal,
      refreshRemote: refreshRemote,
      localSession: localSession,
      remoteSession: remoteSession,
      persistHistory: persistHistory,
      hostUuid: hostUuid,
      historyUuid: persistHistory && hostUuid != null
          ? const Uuid().v7()
          : null,
      withSudo: withSudo,
      verifiedSudoSessionId: verifiedSudoSessionId,
    );
    _controller.taskManager.registerOwner(id, this);
    onQueued?.call(id);
    _controller.taskManager.pumpQueues();
    return completer.future;
  }

  void _pumpSftpTaskQueue() {
    if (_shuttingDown) {
      return;
    }
    while (_controller.runningTaskIds.length < sftpConcurrentTasks) {
      final nextTask = _tasks
          .where(
            (task) =>
                task.status == _SftpTaskStatus.queued &&
                !task.cancelRequested &&
                _controller.taskManager.isOwnedBy(task.id, this) &&
                !_controller.runningTaskIds.contains(task.id),
          )
          .firstOrNull;
      if (nextTask == null) {
        return;
      }
      final execution = _controller.taskExecutions[nextTask.id];
      if (execution == null) {
        _replaceSftpTask(
          nextTask.id,
          (task) => task.copyWith(
            status: _SftpTaskStatus.failed,
            error: 'SFTP task queue entry was missing.',
            finishedAt: DateTime.now(),
          ),
        );
        _completeSftpTask(nextTask.id);
        continue;
      }

      final runningTaskId = nextTask.id;
      _controller.runningTaskIds.add(runningTaskId);
      unawaited(
        _runQueuedSftpTask(runningTaskId, execution).whenComplete(() {
          _controller.runningTaskIds.remove(runningTaskId);
          final finishedTask = _tasks
              .where((task) => task.id == runningTaskId)
              .firstOrNull;
          final retainExecution =
              finishedTask?.status == _SftpTaskStatus.paused ||
              (execution.verifiedSudoSessionId == null &&
                  finishedTask?.status == _SftpTaskStatus.failed &&
                  _isPausableSftpTask(finishedTask!));
          if (!retainExecution) {
            _controller.taskExecutions.remove(runningTaskId);
            _controller.taskManager.forgetOwner(runningTaskId);
          }
          _controller.taskManager.pumpQueues();
        }),
      );
    }
  }

  Future<void> _runQueuedSftpTask(
    int taskId,
    _QueuedSftpTaskExecution execution,
  ) async {
    final currentTask = _tasks.where((task) => task.id == taskId).firstOrNull;
    if (currentTask == null ||
        currentTask.status != _SftpTaskStatus.queued ||
        currentTask.cancelRequested) {
      return;
    }
    final logOperation = NautermLog.begin(
      'sftp',
      'SFTP task',
      fields: {'task_type': currentTask.type.name},
    );
    _replaceSftpTask(
      taskId,
      (task) => task.copyWith(status: _SftpTaskStatus.running),
    );
    final session = execution.remoteSession ?? _rightRemote;
    final verifiedSudoId = execution.verifiedSudoSessionId;
    var activeSudoId = verifiedSudoId ?? session.sudoSessionId;
    bool currentSudoScope() =>
        identical(session.connection?.auth, execution.auth) &&
        session.sudoSessionId == activeSudoId;
    final arguments = execution.auth.toArguments(session.path)
      ..['taskId'] = execution.nativeTaskId;
    var lastProgressAt = DateTime.fromMillisecondsSinceEpoch(0);
    var lastProgressBytes = -1;
    var sudoPaneChanged = false;

    void updateProgress(_SftpTaskProgressUpdate progress) {
      final now = DateTime.now();
      final finishedProgress =
          progress.totalBytes > 0 &&
          progress.transferredBytes >= progress.totalBytes;
      final shouldUpdate =
          finishedProgress ||
          progress.transferredBytes == 0 ||
          progress.transferredBytes - lastProgressBytes >= 1024 * 1024 ||
          now.difference(lastProgressAt) >= const Duration(milliseconds: 100);
      if (!shouldUpdate) {
        return;
      }
      lastProgressAt = now;
      lastProgressBytes = progress.transferredBytes;
      _replaceSftpTask(
        taskId,
        (task) => task.copyWith(
          bytes: progress.transferredBytes,
          totalBytes: progress.totalBytes,
          currentPath: progress.currentPath,
        ),
      );
    }

    Future<FfiSftpTaskResult> run(Map<String, Object?> operation) {
      if ((execution.withSudo || operation.containsKey('sudo_password')) &&
          !currentSudoScope()) {
        sudoPaneChanged = true;
        return Future.value(
          const FfiSftpTaskResult(
            ok: false,
            bytes: 0,
            itemKind: 'unknown',
            error: 'SFTP connection changed before the task could run.',
          ),
        );
      }
      arguments['operation'] = operation;
      return _spawnSftpTask(arguments, onProgress: updateProgress);
    }

    Future<String?> promptForCurrentPane(
      _SftpSudoPromptKind kind, {
      String? error,
    }) async {
      if (!currentSudoScope()) {
        sudoPaneChanged = true;
        return null;
      }
      final revision = session.navigationRevision;
      final password = await _promptSftpSudoPassword(
        kind: kind,
        error: error,
        paths: kind == _SftpSudoPromptKind.permissionDenied
            ? (
                source: currentTask.sourcePath,
                destination: currentTask.targetPath,
              )
            : null,
      );
      if (password != null &&
          (revision != session.navigationRevision || !currentSudoScope())) {
        sudoPaneChanged = true;
        return null;
      }
      return password;
    }

    var operation = execution.operation;
    String? initialSudoPassword;
    final usedCachedChannel =
        execution.withSudo &&
        (verifiedSudoId != null || session.sudoSessionAuthenticated);
    if (execution.withSudo) {
      if (!usedCachedChannel && !_closingSftp) {
        initialSudoPassword = await promptForCurrentPane(
          _SftpSudoPromptKind.explicit,
        );
      }
      if (usedCachedChannel || initialSudoPassword != null) {
        operation = {...execution.operation, 'sudo_session_id': activeSudoId};
        if (initialSudoPassword != null) {
          operation['sudo_password'] = initialSudoPassword;
        }
      }
    }

    var result =
        execution.withSudo && !usedCachedChannel && initialSudoPassword == null
        ? const FfiSftpTaskResult(
            ok: false,
            bytes: 0,
            itemKind: 'unknown',
            error: 'Sudo authorization was cancelled.',
          )
        : await run(operation);
    if (execution.withSudo) {
      if (!currentSudoScope()) {
        FfiSftpTaskExecutor.closeSudoSession(activeSudoId);
      } else if (result.ok) {
        session.sudoSessionAuthenticated = true;
      } else if (_sftpSudoAuthenticationFailed(result.error)) {
        session.closeSudoSession();
        activeSudoId = session.sudoSessionId;
        if (usedCachedChannel && verifiedSudoId == null && !_closingSftp) {
          final replacementPassword = await promptForCurrentPane(
            _SftpSudoPromptKind.explicit,
            error: result.error,
          );
          if (replacementPassword != null) {
            result = await run({
              ...execution.operation,
              'sudo_session_id': activeSudoId,
              'sudo_password': replacementPassword,
            });
            if (!currentSudoScope()) {
              FfiSftpTaskExecutor.closeSudoSession(activeSudoId);
            } else if (result.ok) {
              session.sudoSessionAuthenticated = true;
            }
          }
        }
      }
    } else if (!result.ok &&
        !_closingSftp &&
        _sftpPermissionDenied(result.error)) {
      final oneShotPassword = await promptForCurrentPane(
        _SftpSudoPromptKind.permissionDenied,
        error: result.error,
      );
      if (oneShotPassword != null) {
        result = await run({
          ...execution.operation,
          'sudo_password': oneShotPassword,
        });
      }
    }
    final cancelled =
        sudoPaneChanged ||
        result.error?.toLowerCase().contains('cancelled') == true;
    final latestTask = _tasks.where((task) => task.id == taskId).firstOrNull;
    final paused =
        !result.ok &&
        cancelled &&
        !sudoPaneChanged &&
        latestTask?.pauseRequested == true;
    if (sudoPaneChanged && mounted && !_closingSftp) {
      _showSftpSnack(context.tr('sftp.sudo.error.paneChanged'));
    } else if (cancelled && !paused && !sudoPaneChanged) {
      await _discardCancelledSftpTaskParts(operation, arguments);
    }
    if (result.ok && mounted) {
      logOperation.succeed(
        fields: {'bytes': result.bytes, 'item_kind': result.itemKind},
      );
    } else {
      logOperation.warn(
        fields: {
          'status': paused
              ? 'paused'
              : cancelled
              ? 'cancelled'
              : 'failed',
        },
      );
    }
    _replaceSftpTask(
      taskId,
      (task) => task.copyWith(
        status: result.ok
            ? _SftpTaskStatus.completed
            : paused
            ? _SftpTaskStatus.paused
            : cancelled
            ? _SftpTaskStatus.cancelled
            : _SftpTaskStatus.failed,
        bytes: result.ok ? result.bytes : null,
        itemKind: result.ok ? result.itemKind : null,
        pauseRequested: false,
        error: result.ok || paused ? null : result.error ?? 'SFTP task failed.',
        finishedAt: paused ? null : DateTime.now(),
      ),
    );
    if (!paused && execution.persistHistory) {
      _persistSftpTaskHistory(taskId, execution);
    }
    if (!paused) {
      _completeSftpTask(taskId);
    }
    if (!paused && !execution.persistHistory && result.ok) {
      _removeSftpTaskFromMemory(taskId);
    }
    if (result.ok) {
      final sessionToRefresh = execution.localSession ?? _leftLocal;
      if (execution.refreshLocal && !sessionToRefresh.changingDirectory) {
        unawaited(
          _loadLocalPath(
            sessionToRefresh,
            sessionToRefresh.path,
            recordHistory: false,
          ),
        );
      }
      if (execution.refreshRemote &&
          !session.changingDirectory &&
          !session.selectingHost) {
        final connection = session.connection;
        if (connection?.phase == _SftpConnectionPhase.connected &&
            identical(connection?.auth, execution.auth)) {
          unawaited(
            _loadRemotePath(
              session,
              execution.auth,
              session.path,
              host: connection!.host,
              recordHistory: false,
            ),
          );
        }
      }
    }
  }

  void _replaceSftpTask(int taskId, _SftpTask Function(_SftpTask task) update) {
    _tasks = [
      for (final task in _tasks) task.id == taskId ? update(task) : task,
    ];
  }

  Future<String?> _promptSftpSudoPassword({
    required _SftpSudoPromptKind kind,
    String? error,
    ({String source, String destination})? paths,
  }) => _showSftpSudoPrompt<String>(
    kind: kind,
    error: error,
    paths: paths,
    onSubmit: (password) => password,
  );

  Future<T?> _showSftpSudoPrompt<T>({
    required _SftpSudoPromptKind kind,
    required FutureOr<T?> Function(String password) onSubmit,
    String? error,
    String? target,
    ({String source, String destination})? paths,
  }) async {
    if (!mounted) return null;
    final controller = TextEditingController();
    final permissionDenied = kind == _SftpSudoPromptKind.permissionDenied;
    final descriptionKey = switch (kind) {
      _SftpSudoPromptKind.explicit => 'sftp.sudo.explicit.dialog.description',
      _SftpSudoPromptKind.permissionDenied =>
        'sftp.sudo.permissionDenied.dialog.description',
      _SftpSudoPromptKind.saveEdit => 'sftp.sudo.edit.dialog.description',
    };
    final actionKey = switch (kind) {
      _SftpSudoPromptKind.explicit => 'common.action.authenticate',
      _SftpSudoPromptKind.permissionDenied =>
        'sftp.sudo.permissionDenied.action.retry',
      _SftpSudoPromptKind.saveEdit =>
        'sftp.sudo.edit.action.authenticateAndSave',
    };
    var authenticating = false;
    try {
      return await _showSftpWorkspaceDialog<T>(
        barrierDismissible: false,
        builder: (_) => StatefulBuilder(
          builder: (dialogContext, setDialogState) {
            Future<void> submit() async {
              if (authenticating) return;
              final password = controller.text;
              if (password.isEmpty) return;
              controller.clear();
              setDialogState(() {
                authenticating = true;
                error = null;
              });
              try {
                final result = await onSubmit(password);
                if (dialogContext.mounted) {
                  Navigator.of(dialogContext).pop(result);
                }
              } on Object catch (failure) {
                if (dialogContext.mounted) {
                  setDialogState(() {
                    authenticating = false;
                    error = failure.toString();
                  });
                }
              }
            }

            return _WorkspaceDialogFrame(
              title: Text(
                tr(
                  permissionDenied
                      ? 'sftp.sudo.permissionDenied.dialog.title'
                      : 'sftp.sudo.explicit.dialog.title',
                ),
              ),
              content: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(tr(descriptionKey)),
                  if (target != null) ...[
                    const SizedBox(height: 8),
                    Text(target),
                  ],
                  if (paths != null) ...[
                    const SizedBox(height: 8),
                    if (paths.source.isNotEmpty)
                      Text(
                        dialogContext.tr(
                          'sftp.label.sourcePath',
                          args: {'path': paths.source},
                        ),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                    if (paths.destination.isNotEmpty)
                      Text(
                        dialogContext.tr(
                          'sftp.label.destinationPath',
                          args: {'path': paths.destination},
                        ),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                  ],
                  if (error case final message?
                      when message.trim().isNotEmpty) ...[
                    const SizedBox(height: 10),
                    Text(
                      message,
                      maxLines: 3,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color:
                            widget.panelColors?.muted ??
                            context.nautermPalette.faintText,
                        fontSize: 11,
                      ),
                    ),
                  ],
                  const SizedBox(height: 16),
                  AbsorbPointer(
                    absorbing: authenticating,
                    child: _WorkspaceInput(
                      controller: controller,
                      label: tr('common.label.password', fallback: 'Password'),
                      autofocus: true,
                      obscureText: true,
                      clearable: false,
                      textInputAction: TextInputAction.done,
                      onSubmitted: (_) => unawaited(submit()),
                    ),
                  ),
                  if (authenticating) ...[
                    const SizedBox(height: 8),
                    Text(tr('common.status.authenticating')),
                  ],
                ],
              ),
              actions: [
                _WorkspaceButton(
                  label: tr('common.action.cancel', fallback: 'Cancel'),
                  variant: _WorkspaceButtonVariant.text,
                  onPressed: authenticating
                      ? null
                      : () => Navigator.of(dialogContext).pop(),
                ),
                _WorkspaceButton(
                  label: tr(actionKey),
                  type: _WorkspaceButtonType.primary,
                  variant: _WorkspaceButtonVariant.solid,
                  onPressed: authenticating ? null : () => unawaited(submit()),
                ),
              ],
            );
          },
        ),
      );
    } finally {
      controller.dispose();
    }
  }

  void _completeSftpTask(int taskId) {
    final completer = _controller.taskCompletions.remove(taskId);
    if (completer == null || completer.isCompleted) {
      return;
    }
    final task = _tasks.where((task) => task.id == taskId).firstOrNull;
    if (task != null) {
      completer.complete(task);
    }
  }

  _SftpPaneSlot _sftpHistorySlot(SftpTaskHistoryEntry entry) {
    final hostUuid = entry.hostUuid;
    if (hostUuid != null && _leftRemote.connection?.host.uuid == hostUuid) {
      return _SftpPaneSlot.left;
    }
    if (hostUuid != null && _rightRemote.connection?.host.uuid == hostUuid) {
      return _SftpPaneSlot.right;
    }
    return _SftpPaneSlot.right;
  }

  void _persistSftpTaskHistory(int taskId, _QueuedSftpTaskExecution execution) {
    final store = widget.dataStore;
    if (store == null) return;
    final task = _tasks.where((task) => task.id == taskId).firstOrNull;
    final finishedAt = task?.finishedAt;
    if (task == null || finishedAt == null || execution.historyUuid == null) {
      return;
    }
    final auth = execution.auth;
    try {
      final historyId = store.saveSftpTaskHistory(
        SftpTaskHistoryEntry(
          uuid: execution.historyUuid,
          hostUuid: execution.hostUuid,
          type: _persistedSftpTaskType(task.type),
          host: auth.host,
          username: auth.username,
          port: auth.port,
          status: task.status.name,
          displayName: task.displayName,
          sourcePath: task.sourcePath,
          targetPath: task.targetPath,
          createdAt: task.createdAt,
          finishedAt: finishedAt,
          bytes: task.bytes,
          totalBytes: task.totalBytes,
          itemKind: task.itemKind,
          error: task.error,
        ),
      );
      _replaceSftpTask(
        taskId,
        (current) => current.copyWith(historyId: historyId),
      );
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'sftp',
        'Unable to save SFTP task history.',
        error: error,
        stackTrace: stackTrace,
      );
    }
  }

  void _removeSftpTaskFromMemory(int taskId) {
    _tasks = [
      for (final task in _tasks)
        if (task.id != taskId) task,
    ];
    _controller.taskManager.forgetOwner(taskId);
  }

  List<_SftpTask> _tasksForSlot(_SftpPaneSlot slot) {
    final cutoff = DateTime.now().subtract(_sftpTaskHistoryRetention);
    return [
      for (final task in _tasks)
        if ((task.slot == slot || widget.remoteOnly || widget.compact) &&
            (!_isFinishedSftpTask(task) ||
                (task.finishedAt ?? task.createdAt).isAfter(cutoff)))
          task,
    ];
  }

  void _clearCompletedSftpTasks() {
    final removedTasks = [
      for (final task in _tasks)
        if (task.status != _SftpTaskStatus.queued &&
            task.status != _SftpTaskStatus.running &&
            task.status != _SftpTaskStatus.paused)
          task,
    ];
    for (final task in removedTasks) {
      final execution = _controller.taskExecutions[task.id];
      if (task.status != _SftpTaskStatus.completed && execution != null) {
        _trackSftpTaskCleanup(_discardSftpExecutionParts(execution));
      } else if (task.status != _SftpTaskStatus.completed &&
          task.type.wireName == _SftpTaskType.download.wireName) {
        _trackSftpTaskCleanup(_discardLocalDownloadParts(task.targetPath));
      }
      _controller.taskManager.forgetOwner(task.id);
    }
    try {
      widget.dataStore?.clearSftpTaskHistory();
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'sftp',
        'Unable to clear SFTP task history.',
        error: error,
        stackTrace: stackTrace,
      );
    }
    _setSftpState(() {
      _tasks = [
        for (final task in _tasks)
          if (task.status == _SftpTaskStatus.queued ||
              task.status == _SftpTaskStatus.running ||
              task.status == _SftpTaskStatus.paused)
            task,
      ];
    });
    final activeTaskIds = _tasks.map((task) => task.id).toSet();
    _controller.taskExecutions.removeWhere(
      (taskId, _) => !activeTaskIds.contains(taskId),
    );
  }

  void _dismissSftpTask(int taskId) {
    final owner = _controller.taskManager.ownerOf(taskId);
    if (owner != null && !identical(owner, this)) {
      owner._dismissSftpTask(taskId);
      return;
    }
    final task = _tasks.where((task) => task.id == taskId).firstOrNull;
    final historyId = task?.historyId;
    final execution = _controller.taskExecutions[taskId];
    if (task != null && task.status != _SftpTaskStatus.completed) {
      if (execution != null) {
        _trackSftpTaskCleanup(_discardSftpExecutionParts(execution));
      } else if (task.type.wireName == _SftpTaskType.download.wireName) {
        _trackSftpTaskCleanup(_discardLocalDownloadParts(task.targetPath));
      }
    }
    if (historyId != null) {
      try {
        widget.dataStore?.deleteSftpTaskHistory(historyId);
      } on Object catch (error, stackTrace) {
        NautermLog.warning(
          'sftp',
          'Unable to delete SFTP task history.',
          error: error,
          stackTrace: stackTrace,
        );
      }
    }
    _controller.taskExecutions.remove(taskId);
    _controller.taskManager.forgetOwner(taskId);
    _setSftpState(() {
      _tasks = [
        for (final task in _tasks)
          if (task.id != taskId) task,
      ];
    });
  }

  void _cancelSftpTask(int taskId) {
    final owner = _controller.taskManager.ownerOf(taskId);
    if (owner != null && !identical(owner, this)) {
      owner._cancelSftpTask(taskId);
      return;
    }
    _SftpTask? selectedTask;
    for (final task in _tasks) {
      if (task.id == taskId) {
        selectedTask = task;
        break;
      }
    }
    if (selectedTask == null ||
        (selectedTask.status != _SftpTaskStatus.queued &&
            selectedTask.status != _SftpTaskStatus.running &&
            selectedTask.status != _SftpTaskStatus.paused)) {
      return;
    }
    if (selectedTask.status == _SftpTaskStatus.queued ||
        selectedTask.status == _SftpTaskStatus.paused) {
      final execution = _controller.taskExecutions[selectedTask.id];
      if (execution != null) {
        _trackSftpTaskCleanup(_discardSftpExecutionParts(execution));
      }
      _controller.taskExecutions.remove(taskId);
      _replaceSftpTask(
        taskId,
        (task) => task.copyWith(
          status: _SftpTaskStatus.cancelled,
          cancelRequested: true,
          finishedAt: DateTime.now(),
        ),
      );
      if (execution != null && execution.persistHistory) {
        _persistSftpTaskHistory(taskId, execution);
      }
      _completeSftpTask(taskId);
      _controller.taskManager.forgetOwner(taskId);
      _controller.taskManager.pumpQueues();
      return;
    }
    FfiSftpTaskExecutor.cancel(selectedTask.nativeTaskId);
    _replaceSftpTask(
      taskId,
      (task) => task.copyWith(cancelRequested: true, pauseRequested: false),
    );
  }

  void _pauseSftpTask(int taskId) {
    final owner = _controller.taskManager.ownerOf(taskId);
    if (owner != null && !identical(owner, this)) {
      owner._pauseSftpTask(taskId);
      return;
    }
    final task = _tasks.where((task) => task.id == taskId).firstOrNull;
    if (task == null ||
        !_isPausableSftpTask(task) ||
        (task.status != _SftpTaskStatus.queued &&
            task.status != _SftpTaskStatus.running)) {
      return;
    }
    if (task.status == _SftpTaskStatus.queued) {
      _replaceSftpTask(
        taskId,
        (task) => task.copyWith(status: _SftpTaskStatus.paused),
      );
      _controller.taskManager.pumpQueues();
      return;
    }
    FfiSftpTaskExecutor.cancel(task.nativeTaskId);
    _replaceSftpTask(taskId, (task) => task.copyWith(pauseRequested: true));
  }

  void _resumeSftpTask(int taskId) {
    final owner = _controller.taskManager.ownerOf(taskId);
    if (owner != null && !identical(owner, this)) {
      owner._resumeSftpTask(taskId);
      return;
    }
    final task = _tasks.where((task) => task.id == taskId).firstOrNull;
    final execution = _controller.taskExecutions[taskId];
    if (task == null ||
        execution == null ||
        _shuttingDown ||
        _controller.runningTaskIds.contains(taskId) ||
        (task.status != _SftpTaskStatus.paused &&
            !_controller.taskManager.canRetry(task))) {
      return;
    }
    final nativeTaskId = DateTime.now().microsecondsSinceEpoch + taskId;
    _controller.taskExecutions[taskId] = execution.withNativeTaskId(
      nativeTaskId,
    );
    _controller.taskCompletions.putIfAbsent(taskId, Completer<_SftpTask>.new);
    _replaceSftpTask(
      taskId,
      (task) => task.copyWith(
        nativeTaskId: nativeTaskId,
        status: _SftpTaskStatus.queued,
        cancelRequested: false,
        pauseRequested: false,
        error: null,
        finishedAt: null,
      ),
    );
    _controller.taskManager.pumpQueues();
  }

  void _toggleSftpTaskPause(int taskId) {
    final task = _tasks.where((task) => task.id == taskId).firstOrNull;
    if (task?.status == _SftpTaskStatus.paused ||
        task?.status == _SftpTaskStatus.failed) {
      _resumeSftpTask(taskId);
    } else {
      _pauseSftpTask(taskId);
    }
  }

  Future<void> _discardCancelledSftpTaskParts(
    Map<String, Object?> operation,
    Map<String, Object?> arguments,
  ) async {
    final operationName = operation['op'];
    if (operationName == _SftpTaskType.download.wireName) {
      final localPath = operation['local_path'] as String?;
      if (localPath != null && localPath.isNotEmpty) {
        await _discardLocalDownloadParts(localPath);
      }
      return;
    }
    if (operationName != _SftpTaskType.upload.wireName) {
      return;
    }
    final remotePath = operation['remote_path'] as String?;
    if (remotePath == null || remotePath.isEmpty) {
      return;
    }
    final cleanupOperation = <String, Object?>{
      'op': 'cleanup_upload',
      'remote_path': remotePath,
      if (operation['sudo_session_id'] case final String sessionId)
        'sudo_session_id': sessionId,
      if (operation['sudo_password'] case final String password)
        'sudo_password': password,
    };
    final cleanupArguments = <String, Object?>{
      ...arguments,
      'taskId': DateTime.now().microsecondsSinceEpoch,
      'operation': cleanupOperation,
    };
    final result = await _spawnSftpTask(cleanupArguments, onProgress: (_) {});
    if (!result.ok) {
      NautermLog.warning(
        'sftp',
        'Unable to discard cancelled SFTP upload data.',
        fields: {'remote_path': remotePath, 'error': result.error},
      );
    }
  }

  Future<void> _discardSftpExecutionParts(
    _QueuedSftpTaskExecution execution,
  ) async {
    final session = execution.remoteSession ?? _rightRemote;
    final operation = <String, Object?>{
      ...execution.operation,
      if (execution.withSudo) 'sudo_session_id': session.sudoSessionId,
    };
    final arguments = execution.auth.toArguments(session.path)
      ..['taskId'] = DateTime.now().microsecondsSinceEpoch;
    await _discardCancelledSftpTaskParts(operation, arguments);
  }

  void _trackSftpTaskCleanup(Future<void> cleanup) {
    final tracked = cleanup.then<void>(
      (_) {},
      onError: (Object error, StackTrace stackTrace) {
        NautermLog.warning(
          'sftp',
          'Unable to clean up cancelled SFTP task data.',
          error: error,
          stackTrace: stackTrace,
        );
      },
    );
    _controller.taskCleanupFutures.add(tracked);
    unawaited(
      tracked.whenComplete(() {
        _controller.taskCleanupFutures.remove(tracked);
      }),
    );
  }

  Future<void> _discardLocalDownloadParts(String localPath) async {
    final target = io.File(localPath);
    final partPath = _joinSftpPath(
      target.parent.path,
      '.${_basename(localPath)}.nauterm-download.part',
    );
    final metadataPath = '$partPath.meta';
    try {
      final metadata = await io.File(metadataPath).readAsString();
      if (!metadata.startsWith('nauterm-sftp-part-v1\n') &&
          !metadata.startsWith('nauterm-sftp-download-v1\n')) {
        NautermLog.warning(
          'sftp',
          'Refusing to remove an unowned SFTP download staging path.',
          fields: {'staging_path': partPath},
        );
        return;
      }
    } on Object {
      return;
    }
    final paths = <String>[
      partPath,
      for (var index = 0; index < 32; index++) '$partPath.chunk-$index',
    ];
    var removedOwnedData = true;
    for (final path in paths) {
      try {
        final type = await io.FileSystemEntity.type(path, followLinks: false);
        if (type == io.FileSystemEntityType.directory) {
          await io.Directory(path).delete(recursive: true);
        } else if (type != io.FileSystemEntityType.notFound) {
          await io.File(path).delete();
        }
      } on Object catch (error, stackTrace) {
        removedOwnedData = false;
        NautermLog.warning(
          'sftp',
          'Unable to discard cancelled SFTP download data.',
          error: error,
          stackTrace: stackTrace,
        );
      }
    }
    if (removedOwnedData) {
      try {
        await io.File(metadataPath).delete();
      } on Object catch (error, stackTrace) {
        NautermLog.warning(
          'sftp',
          'Unable to discard cancelled SFTP download metadata.',
          error: error,
          stackTrace: stackTrace,
        );
      }
    }
  }

  void _toggleSftpTaskList(_SftpPaneSlot slot) {
    _setSftpState(() {
      if (_controller.taskListOpen && _controller.taskListSlot == slot) {
        _controller.taskListOpen = false;
        return;
      }
      _controller.taskListSlot = slot;
      _controller.taskListOpen = true;
      _controller.favoriteListOpen = false;
    });
  }

  void _toggleSftpFavoriteList(_SftpPaneSlot slot) {
    _setSftpState(() {
      if (_controller.favoriteListOpen &&
          _controller.favoriteListSlot == slot) {
        _controller.favoriteListOpen = false;
        return;
      }
      _controller.favoriteListSlot = slot;
      _controller.favoriteListOpen = true;
      _controller.taskListOpen = false;
    });
  }

  void _dismissSftpPathPanels() {
    _setSftpState(() {
      _controller.taskListOpen = false;
      _controller.favoriteListOpen = false;
    });
  }

  void _toggleRemoteSftpFavoritePath(
    _SftpRemotePaneSession session,
    _HostItem host,
  ) {
    final normalizedPath = session.path.trim();
    if (normalizedPath.isEmpty || session.loadingFavorites) {
      return;
    }
    final remove = session.favoritePaths.contains(normalizedPath);
    if (host.id > 0) {
      try {
        _persistSftpFavoritePathChange(
          scope: SftpFavoriteScope.remote,
          hostId: host.id,
          path: normalizedPath,
          remove: remove,
        );
      } on Object catch (error) {
        _showSftpSnack(error.toString());
        return;
      }
    }
    _setSftpState(
      () => _setFavoritePath(session.favoritePaths, normalizedPath, remove),
    );
  }

  void _persistSftpFavoritePathChange({
    required SftpFavoriteScope scope,
    int? hostId,
    required String path,
    required bool remove,
  }) {
    final store = widget.dataStore;
    if (store == null) {
      return;
    }
    if (remove) {
      store.deleteSftpFavoritePathByTarget(
        scope: scope,
        hostId: hostId,
        path: path,
      );
    } else {
      store.saveSftpFavoritePath(
        SftpFavoritePathEntry(scope: scope, hostId: hostId, path: path),
      );
    }
  }

  void _setFavoritePath(
    List<String> favoritePaths,
    String normalizedPath,
    bool remove,
  ) {
    favoritePaths.remove(normalizedPath);
    if (!remove) {
      favoritePaths.insert(0, normalizedPath);
    }
  }

  Future<_SftpResolvedRemoteTarget?> _prepareRemoteSftpTarget(
    _SftpRemotePaneSession session,
    _SftpRemoteAuth auth,
    String targetPath, {
    Set<String>? reservedTargets,
    Map<String, Set<String>>? siblingCache,
    required bool Function() isCurrent,
  }) async {
    if (!isCurrent()) return null;
    final normalizedTarget = _collapseRemoteSftpPath(targetPath);
    final revision = session.navigationRevision;
    _setSftpState(() {
      session.destinationChecks.update(
        revision,
        (count) => count + 1,
        ifAbsent: () => 1,
      );
    });
    final Set<String> siblingTargets;
    try {
      siblingTargets = await _remoteSftpSiblingTargets(
        session,
        auth,
        normalizedTarget,
        cache: siblingCache,
      );
    } finally {
      final count = session.destinationChecks[revision] ?? 0;
      if (count <= 1) {
        session.destinationChecks.remove(revision);
      } else {
        session.destinationChecks[revision] = count - 1;
      }
      _setSftpState(() {});
    }
    if (!isCurrent()) return null;
    if (reservedTargets != null) {
      siblingTargets.addAll(reservedTargets);
    }
    if (!siblingTargets.contains(normalizedTarget)) {
      return _SftpResolvedRemoteTarget(
        targetPath: normalizedTarget,
        replaceExisting: false,
      );
    }

    final keepBothPath = _uniqueRemoteSftpPath(
      normalizedTarget,
      siblingTargets,
    );
    final resolution = await _promptRemoteNameConflict(
      name: _remoteSftpName(normalizedTarget),
      targetPath: normalizedTarget,
      keepBothPath: keepBothPath,
    );
    if (!isCurrent() || resolution == _SftpNameConflictResolution.cancel) {
      return null;
    }
    if (resolution == _SftpNameConflictResolution.replace) {
      return _SftpResolvedRemoteTarget(
        targetPath: normalizedTarget,
        replaceExisting: true,
      );
    }
    return _SftpResolvedRemoteTarget(
      targetPath: keepBothPath,
      replaceExisting: false,
    );
  }

  Future<Set<String>> _remoteSftpSiblingTargets(
    _SftpRemotePaneSession session,
    _SftpRemoteAuth auth,
    String targetPath, {
    Map<String, Set<String>>? cache,
  }) async {
    final parent = _remoteParentPath(targetPath);
    final cacheKey = parent == null ? '' : _collapseRemoteSftpPath(parent);
    final cached = cache?[cacheKey];
    if (cached != null) {
      return {...cached};
    }

    final targets = _remoteSftpSessionSiblingTargets(session, targetPath);
    if (parent != null) {
      try {
        final listing = await _listRemoteSftpDirectory(
          session,
          auth,
          parent,
          includeHidden: true,
        );
        for (final entry in listing.entries) {
          if (!entry.isParent) {
            targets.add(_collapseRemoteSftpPath(entry.path));
          }
        }
      } catch (_) {
        // Fall back to the current pane listing; the task runner will still
        // reject unexpected overwrites when replace_existing is false.
      }
    }
    if (cache != null) {
      cache[cacheKey] = {...targets};
    }
    return targets;
  }

  Set<String> _remoteSftpSessionSiblingTargets(
    _SftpRemotePaneSession session,
    String targetPath,
  ) {
    final parent = _remoteParentPath(targetPath);
    return {
      for (final entry in session.entries)
        if (!entry.isParent && _sameRemoteSftpParent(entry.path, parent))
          _collapseRemoteSftpPath(entry.path),
    };
  }

  bool _sameRemoteSftpParent(String path, String? parent) {
    final entryParent = _remoteParentPath(path);
    if (entryParent == null || parent == null) {
      return entryParent == parent;
    }
    return _collapseRemoteSftpPath(entryParent) ==
        _collapseRemoteSftpPath(parent);
  }

  String _uniqueRemoteSftpPath(String targetPath, Set<String> existingTargets) {
    final parent = _remoteParentPath(targetPath) ?? '~';
    final name = _remoteSftpName(targetPath);
    var candidate = _collapseRemoteSftpPath(targetPath);
    var copyIndex = 1;
    while (existingTargets.contains(candidate)) {
      candidate = _joinRemoteSftpPath(
        parent,
        _remoteSftpCopyName(name, copyIndex),
      );
      copyIndex += 1;
    }
    return candidate;
  }

  String _remoteSftpName(String path) {
    final collapsed = _collapseRemoteSftpPath(path);
    final index = collapsed.lastIndexOf('/');
    return index < 0 ? collapsed : collapsed.substring(index + 1);
  }

  String _remoteSftpCopyName(String name, int copyIndex) {
    final suffix = copyIndex == 1 ? ' copy' : ' copy $copyIndex';
    final dotIndex = name.lastIndexOf('.');
    final hasExtension = dotIndex > 0 && dotIndex < name.length - 1;
    if (!hasExtension) {
      return '$name$suffix';
    }
    final stem = name.substring(0, dotIndex);
    final extension = name.substring(dotIndex);
    return '$stem$suffix$extension';
  }

  Future<_SftpNameConflictResolution> _promptRemoteNameConflict({
    required String name,
    required String targetPath,
    required String keepBothPath,
  }) async {
    if (!mounted) {
      return _SftpNameConflictResolution.cancel;
    }
    final result = await _showSftpWorkspaceDialog<_SftpNameConflictResolution>(
      builder: (dialogContext) {
        final colors = widget.compact ? widget.panelColors : null;
        return _WorkspaceDialogFrame(
          width: 460,
          title: Text(
            dialogContext.tr(
              'sftp.conflict.destinationExists.title',
              fallback: 'Destination already exists',
            ),
          ),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                tr(
                  'sftp.conflict.itemExists.description',
                  fallback: 'An item named "{name}" already exists in the destination.',
                  args: {'name': name},
                ),
                style: TextStyle(
                  color: colors?.foreground ?? _text,
                  fontSize: NautermFontSizes.labelLarge,
                  fontWeight: NautermFontWeights.regular,
                  height: 1.35,
                  letterSpacing: 0,
                ),
              ),
              const SizedBox(height: 8),
              Text(
                dialogContext.tr(
                  'sftp.conflict.replacement.description',
                  fallback: 'Keep both items with different names, or replace the existing destination. Replacement cannot be undone.',
                ),
                style: TextStyle(
                  color: colors?.muted ?? _mutedText,
                  fontSize: NautermFontSizes.labelMedium,
                  letterSpacing: 0,
                ),
              ),
              SizedBox(height: 10),
              DecoratedBox(
                decoration: BoxDecoration(
                  color: colors?.inputBackground ?? _sidebarHover,
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: colors?.border ?? _sidebarDivider),
                ),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 10,
                    vertical: 8,
                  ),
                  child: Text(
                    dialogContext.tr(
                      'sftp.label.destinationPath',
                      args: {'path': targetPath},
                    ),
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: colors?.muted ?? Color(0xff5f737b),
                      fontFamily: 'monospace',
                      fontSize: NautermFontSizes.labelMedium,
                      fontWeight: NautermFontWeights.regular,
                      height: 1.35,
                      letterSpacing: 0,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 8),
              Text(
                dialogContext.tr(
                  'sftp.conflict.keepBoth.path',
                  args: {'path': keepBothPath},
                ),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: colors?.muted ?? _mutedText,
                  fontSize: NautermFontSizes.labelMedium,
                ),
              ),
            ],
          ),
          actions: [
            _WorkspaceButton(
              label: 'common.action.cancel',
              variant: _WorkspaceButtonVariant.text,
              onPressed: () =>
                  Navigator.of(dialogContext)
                      .pop(_SftpNameConflictResolution.cancel),
            ),
            _WorkspaceButton(
              label: 'sftp.label.keepBoth',
              variant: _WorkspaceButtonVariant.filled,
              onPressed: () =>
                  Navigator.of(dialogContext)
                      .pop(_SftpNameConflictResolution.keepBoth),
            ),
            _WorkspaceButton(
              label: 'sftp.label.replace',
              type: _WorkspaceButtonType.primary,
              variant: _WorkspaceButtonVariant.solid,
              onPressed: () =>
                  Navigator.of(dialogContext)
                      .pop(_SftpNameConflictResolution.replace),
            ),
          ],
        );
      },
    );
    return result ?? _SftpNameConflictResolution.cancel;
  }

  Future<void> _downloadSelectedRemoteToDownloadsDirectory(
    _SftpRemotePaneSession session, {
    bool withSudo = false,
  }) async {
    final entries = _selectedRemoteEntries(session);
    if (entries.isEmpty) return;
    final source = _sftpPaneLocation(
      _slotForRemoteSession(session),
      _SftpPaneEndpoint.remote,
    );
    final cancellation = tr(
      'sftp.transfer.sourceChanged.description',
      fallback: 'Download cancelled because the source changed.',
    );
    bool isCurrent() => _sftpPaneLocationIsCurrent(source);

    final downloadsDirectory = await _controller.taskManager.downloadsDirectory;
    if (!_checkSftpTransferScope(isCurrent(), cancellation)) return;
    if (downloadsDirectory == null) {
      if (mounted) {
        _showSftpSnack(
          tr(
            'sftp.error.useDownloadsDirectory',
            fallback: 'Unable to use the system Downloads directory.',
          ),
        );
      }
      return;
    }
    await _downloadRemoteEntriesToDirectory(
      entries,
      session,
      _normalizeLocalSftpSeparators(downloadsDirectory.path),
      isCurrent: isCurrent,
      cancellation: cancellation,
      withSudo: withSudo,
    );
  }

  Future<void> _downloadRemoteEntriesToDirectory(
    List<_SftpFileEntry> entries,
    _SftpRemotePaneSession session,
    String targetDirectory, {
    required bool Function() isCurrent,
    required String cancellation,
    _SftpLocalPaneSession? localSession,
    bool withSudo = false,
  }) async {
    final connection = session.connection;
    final auth = connection?.auth;
    if (entries.isEmpty || connection == null || auth == null) {
      return;
    }
    final reservedTargets = <String>{};
    for (final entry in entries) {
      final String targetPath;
      try {
        targetPath = await _uniqueLocalSftpPathWithReserved(
          targetDirectory,
          entry.name,
          reservedTargets,
        );
      } on Object catch (error) {
        if (isCurrent()) {
          _showSftpSnack(tr('sftp.error.download', args: {'error': error}));
        }
        return;
      }
      if (!_checkSftpTransferScope(isCurrent(), cancellation)) return;
      reservedTargets.add(targetPath);
      _enqueueSftpTask(
        type: _SftpTaskType.download,
        slot: _slotForRemoteSession(session),
        displayName: entry.name,
        sourcePath: entry.path,
        targetPath: targetPath,
        auth: auth,
        operation: {
          'op': _SftpTaskType.download.wireName,
          'remote_path': entry.path,
          'local_path': targetPath,
        },
        refreshLocal: localSession != null,
        localSession: localSession,
        remoteSession: session,
        withSudo: withSudo,
      );
    }
  }

  Future<void> _copyRemoteEntriesToRemote(
    _SftpPaneDragPayload drag,
    _SftpRemotePaneSession sourceSession,
    _SftpRemotePaneSession targetSession,
    _SftpPaneLocation destination,
  ) async {
    final entries = drag.entries;
    final sourceAuth = sourceSession.connection?.auth;
    final targetAuth = targetSession.connection?.auth;
    if (entries.isEmpty || sourceAuth == null || targetAuth == null) {
      return;
    }
    bool isCurrent() =>
        _sftpDragSourceIsCurrent(drag) &&
        _sftpPaneLocationIsCurrent(destination);
    final cancellation = _sftpTransferScopeChanged;

    final reservedTargets = <String>{};
    final siblingCache = <String, Set<String>>{};
    io.Directory? relayDirectory;
    try {
      relayDirectory = await io.Directory.systemTemp.createTemp(
        'nauterm-sftp-transfer-',
      );
      if (!await _ensurePrivateSftpTransferDirectory(relayDirectory.path)) {
        return;
      }

      for (final entry in entries) {
        final originalTargetPath = _joinRemoteSftpPath(
          destination.path,
          entry.name,
        );
        final target = await _prepareRemoteSftpTarget(
          targetSession,
          targetAuth,
          originalTargetPath,
          reservedTargets: reservedTargets,
          siblingCache: siblingCache,
          isCurrent: isCurrent,
        );
        if (!_checkSftpTransferScope(isCurrent(), cancellation)) return;
        if (target == null) continue;

        final targetPath = target.targetPath;
        reservedTargets.add(_collapseRemoteSftpPath(targetPath));
        final displayName = targetPath == originalTargetPath
            ? entry.name
            : '${entry.name} -> ${_remoteSftpName(targetPath)}';

        final localPath = _joinSftpPath(relayDirectory.path, entry.name);
        final downloadTask = await _enqueueSftpTask(
          type: _SftpTaskType.transferDownload,
          slot: _slotForRemoteSession(sourceSession),
          displayName: entry.name,
          sourcePath: entry.path,
          targetPath: localPath,
          auth: sourceAuth,
          operation: {
            'op': _SftpTaskType.download.wireName,
            'remote_path': entry.path,
            'local_path': localPath,
          },
          remoteSession: sourceSession,
        );
        if (!_checkSftpTransferScope(isCurrent(), cancellation)) return;
        if (downloadTask.status != _SftpTaskStatus.completed) {
          continue;
        }

        await _enqueueSftpTask(
          type: _SftpTaskType.transferUpload,
          slot: _slotForRemoteSession(targetSession),
          displayName: displayName,
          sourcePath: entry.path,
          targetPath: targetPath,
          auth: targetAuth,
          operation: {
            'op': _SftpTaskType.upload.wireName,
            'local_path': localPath,
            'remote_path': targetPath,
            'replace_existing': target.replaceExisting,
          },
          refreshRemote: true,
          remoteSession: targetSession,
        );
        try {
          final localType = await io.FileSystemEntity.type(
            localPath,
            followLinks: false,
          );
          if (localType == io.FileSystemEntityType.directory) {
            await io.Directory(localPath).delete(recursive: true);
          } else if (localType != io.FileSystemEntityType.notFound) {
            await io.File(localPath).delete();
          }
        } on Object {
          // The containing private relay directory is removed in finally.
        }
      }
    } finally {
      if (relayDirectory != null) {
        try {
          await relayDirectory.delete(recursive: true);
        } on Object {
          // Temporary transfer cleanup is best-effort.
        }
      }
    }
  }

  Future<bool> _ensurePrivateSftpTransferDirectory(String path) async {
    if (io.Platform.isWindows) return true;
    try {
      final result = await io.Process.run('/bin/chmod', ['700', path]);
      if (result.exitCode != 0) {
        throw io.FileSystemException(
          'chmod failed: ${result.stderr.toString().trim()}',
          path,
        );
      }
      return true;
    } on Object catch (error) {
      if (mounted) {
        _showSftpSnack(
          tr(
            'sftp.error.protectTransferData',
            fallback: 'Unable to protect temporary transfer data: {error}',
            args: {'error': error},
          ),
        );
      }
      return false;
    }
  }

  bool _canAcceptSftpPaneDrag(
    _SftpPaneSlot targetSlot,
    _SftpPaneDragPayload payload,
  ) {
    if (payload.entries.isEmpty ||
        payload.source.slot == targetSlot ||
        !_sftpDragSourceIsCurrent(payload)) {
      return false;
    }
    final targetEndpoint = switch (targetSlot) {
      _SftpPaneSlot.left => _leftPaneEndpoint,
      _SftpPaneSlot.right => _rightPaneEndpoint,
    };
    return (payload.sourceRemote ||
            targetEndpoint == _SftpPaneEndpoint.remote) &&
        _sftpPaneLocationIsCurrent(
          _sftpPaneLocation(targetSlot, targetEndpoint),
        );
  }

  void _updateSftpPaneDragHover(
    _SftpPaneSlot targetSlot,
    _SftpPaneDragPayload payload,
  ) {
    if (!_canAcceptSftpPaneDrag(targetSlot, payload)) return;
    final targetEndpoint = switch (targetSlot) {
      _SftpPaneSlot.left => _leftPaneEndpoint,
      _SftpPaneSlot.right => _rightPaneEndpoint,
    };
    final label = payload.sourceRemote
        ? targetEndpoint == _SftpPaneEndpoint.remote
              ? 'sftp.action.uploadHere'
              : 'sftp.action.downloadHere'
        : 'sftp.action.uploadHere';
    if (_fileDropHoverSlot == targetSlot && _fileDropHoverLabelKey == label) {
      return;
    }
    setState(() {
      _fileDropHoverSlot = targetSlot;
      _fileDropHoverLabelKey = label;
    });
  }

  void _acceptSftpPaneDrag(
    _SftpPaneSlot targetSlot,
    _SftpPaneDragPayload payload,
  ) {
    _clearFileDropHover();
    if (!_checkSftpTransferScope(
      _canAcceptSftpPaneDrag(targetSlot, payload),
      _sftpTransferScopeChanged,
    )) {
      return;
    }
    final targetEndpoint = targetSlot == _SftpPaneSlot.left
        ? _leftPaneEndpoint
        : _rightPaneEndpoint;
    final destination = _sftpPaneLocation(targetSlot, targetEndpoint);
    if (payload.sourceRemote) {
      final sourceSession = _remoteSessionForSlot(payload.source.slot);
      if (targetEndpoint == _SftpPaneEndpoint.remote) {
        unawaited(
          _copyRemoteEntriesToRemote(
            payload,
            sourceSession,
            _remoteSessionForSlot(targetSlot),
            destination,
          ),
        );
      } else {
        unawaited(
          _downloadRemoteEntriesToDirectory(
            payload.entries,
            sourceSession,
            destination.path,
            localSession: _localSessionForSlot(targetSlot),
            isCurrent: () =>
                _sftpDragSourceIsCurrent(payload) &&
                _sftpPaneLocationIsCurrent(destination),
            cancellation: _sftpTransferScopeChanged,
          ),
        );
      }
      return;
    }
    final remoteSession = _remoteSessionForSlot(targetSlot);
    unawaited(
      _uploadSftpFiles(
        payload.entries.map((entry) => (path: entry.path, name: entry.name)),
        remoteSession,
        destination,
        drag: payload,
      ),
    );
  }

  _SftpPaneSlot? _fileDropTargetSlot(NautermFileDropEvent event) {
    final x = event.x;
    final y = event.y;
    if (x == null || y == null) {
      return null;
    }
    final position = Offset(x, y);
    for (final (slot, key) in [
      (_SftpPaneSlot.left, _leftPaneKey),
      (_SftpPaneSlot.right, _rightPaneKey),
    ]) {
      final context = key.currentContext;
      final renderObject = context?.findRenderObject();
      if (renderObject is! RenderBox || !renderObject.hasSize) {
        continue;
      }
      final origin = renderObject.localToGlobal(Offset.zero);
      final bounds = origin & renderObject.size;
      if (bounds.contains(position)) {
        return slot;
      }
    }
    return null;
  }

  _SftpRemotePaneSession? _fileDropRemoteSessionForSlot(
    _SftpPaneSlot targetSlot,
  ) {
    final endpoint = switch (targetSlot) {
      _SftpPaneSlot.left => _leftPaneEndpoint,
      _SftpPaneSlot.right => _rightPaneEndpoint,
    };
    if (widget.remoteOnly || endpoint == _SftpPaneEndpoint.remote) {
      final session = _remoteSessionForSlot(targetSlot);
      if (_sftpPaneLocationIsCurrent(
        _sftpPaneLocation(targetSlot, _SftpPaneEndpoint.remote),
      )) {
        return session;
      }
    }
    return null;
  }

  _SftpRemotePaneSession? _fileDropTargetRemoteSession(
    NautermFileDropEvent event,
  ) {
    final targetSlot = _fileDropTargetSlot(event);
    if (targetSlot != null) {
      return _fileDropRemoteSessionForSlot(targetSlot);
    }

    if (event.x != null && event.y != null) {
      return null;
    }

    return _fileDropRemoteSessionForSlot(_SftpPaneSlot.right) ??
        _fileDropRemoteSessionForSlot(_SftpPaneSlot.left);
  }

  void _updateFileDropHover(NautermFileDropEvent event) {
    final slot = _fileDropTargetSlot(event);
    final targetSlot =
        slot != null && _fileDropRemoteSessionForSlot(slot) != null
        ? slot
        : null;
    if (_fileDropHoverSlot == targetSlot &&
        _fileDropHoverLabelKey ==
            (targetSlot == null ? null : 'sftp.action.uploadHere')) {
      return;
    }
    setState(() {
      _fileDropHoverSlot = targetSlot;
      _fileDropHoverLabelKey = targetSlot == null
          ? null
          : 'sftp.action.uploadHere';
    });
  }

  void _clearFileDropHover() {
    if (_fileDropHoverSlot == null && _fileDropHoverLabelKey == null) {
      return;
    }
    setState(() {
      _fileDropHoverSlot = null;
      _fileDropHoverLabelKey = null;
    });
  }

  void _handleFileDropEvent(NautermFileDropEvent event) {
    switch (event.type) {
      case NautermFileDropEventType.dragging:
        if (mounted && widget.active) {
          _updateFileDropHover(event);
        }
        return;
      case NautermFileDropEventType.exited:
        if (mounted) {
          _clearFileDropHover();
        }
        return;
      case NautermFileDropEventType.dropped:
        if (mounted) {
          _clearFileDropHover();
        }
    }
    final session = _fileDropTargetRemoteSession(event);
    final connection = session?.connection;
    final auth = connection?.auth;
    if (!mounted ||
        !widget.active ||
        session == null ||
        connection == null ||
        auth == null ||
        connection.phase != _SftpConnectionPhase.connected) {
      return;
    }
    unawaited(
      _uploadSftpFiles(
        _sftpUploadFiles(event.paths),
        session,
        _sftpPaneLocation(
          _slotForRemoteSession(session),
          _SftpPaneEndpoint.remote,
        ),
      ),
    );
  }

  Iterable<({String path, String name})> _sftpUploadFiles(
    Iterable<String> paths,
  ) sync* {
    final uniquePaths = <String>{};
    for (final rawPath in paths) {
      final path = rawPath.trim();
      if (path.isEmpty || !uniquePaths.add(path)) continue;
      final name = _basename(path);
      if (name.isEmpty ||
          io.FileSystemEntity.typeSync(path, followLinks: false) ==
              io.FileSystemEntityType.notFound) {
        continue;
      }
      yield (path: path, name: name);
    }
  }

  Future<void> _uploadSftpFiles(
    Iterable<({String path, String name})> files,
    _SftpRemotePaneSession session,
    _SftpPaneLocation destination, {
    bool withSudo = false,
    _SftpPaneDragPayload? drag,
  }) async {
    final auth = session.connection?.auth;
    if (auth == null) return;
    bool isCurrent() =>
        _sftpPaneLocationIsCurrent(destination) &&
        (drag == null || _sftpDragSourceIsCurrent(drag));

    final cancellation = drag == null
        ? tr(
            'sftp.transfer.destinationChanged.description',
            fallback: 'Upload cancelled because the destination changed.',
          )
        : _sftpTransferScopeChanged;
    final reservedTargets = <String>{};
    final siblingCache = <String, Set<String>>{};
    for (final file in files) {
      final originalTargetPath = _joinRemoteSftpPath(
        destination.path,
        file.name,
      );
      final target = await _prepareRemoteSftpTarget(
        session,
        auth,
        originalTargetPath,
        reservedTargets: reservedTargets,
        siblingCache: siblingCache,
        isCurrent: isCurrent,
      );
      if (!_checkSftpTransferScope(isCurrent(), cancellation)) return;
      if (target == null) continue;
      final targetPath = target.targetPath;
      reservedTargets.add(_collapseRemoteSftpPath(targetPath));
      unawaited(
        _enqueueSftpTask(
          type: _SftpTaskType.upload,
          slot: destination.slot,
          displayName: targetPath == originalTargetPath
              ? file.name
              : '${file.name} -> ${_remoteSftpName(targetPath)}',
          sourcePath: file.path,
          targetPath: targetPath,
          auth: auth,
          operation: {
            'op': _SftpTaskType.upload.wireName,
            'local_path': file.path,
            'remote_path': targetPath,
            'replace_existing': target.replaceExisting,
          },
          refreshRemote: true,
          remoteSession: session,
          withSudo: withSudo,
        ),
      );
    }
  }

  Future<void> _pickLocalFilesForRemoteUpload(
    _SftpRemotePaneSession session, {
    bool withSudo = false,
  }) async {
    final destination = _sftpPaneLocation(
      _slotForRemoteSession(session),
      _SftpPaneEndpoint.remote,
    );
    if (!_sftpPaneLocationIsCurrent(destination) ||
        session.connection?.auth == null) {
      return;
    }
    final List<XFile> files;
    try {
      files = await openFiles();
    } on Object catch (error) {
      if (mounted) {
        _showSftpSnack(
          tr(
            'sftp.error.chooseFiles',
            fallback: 'Unable to choose files: {error}',
            args: {'error': error},
          ),
        );
      }
      return;
    }
    if (!mounted || files.isEmpty) {
      return;
    }
    if (!_checkSftpTransferScope(
      _sftpPaneLocationIsCurrent(destination),
      tr(
        'sftp.transfer.destinationChanged.description',
        fallback: 'Upload cancelled because the destination changed.',
      ),
    )) {
      return;
    }
    await _uploadSftpFiles(
      _sftpUploadFiles(files.map((file) => file.path)),
      session,
      destination,
      withSudo: withSudo,
    );
  }

  Future<void> _renameSelectedRemote(
    _SftpRemotePaneSession session, {
    bool withSudo = false,
  }) async {
    final entry = _selectedRemoteEntry(session);
    final connection = session.connection;
    final auth = connection?.auth;
    if (entry == null || entry.isParent || connection == null || auth == null) {
      return;
    }
    final newName = await _promptSftpInput(
      titleKey: 'common.action.rename',
      label: 'common.label.name',
      initialValue: entry.name,
      confirmLabel: 'common.action.rename',
      withSudo: withSudo,
    );
    if (newName == null || !mounted || newName == entry.name) {
      return;
    }
    final parent = _remoteParentPath(entry.path) ?? session.path;
    final targetPath = _joinRemoteSftpPath(parent, newName);
    _enqueueSftpTask(
      type: _SftpTaskType.move,
      slot: _slotForRemoteSession(session),
      displayName: '${entry.name} -> $newName',
      sourcePath: entry.path,
      targetPath: targetPath,
      auth: auth,
      operation: {
        'op': _SftpTaskType.move.wireName,
        'source_path': entry.path,
        'target_path': targetPath,
        'replace_existing': false,
      },
      refreshRemote: true,
      remoteSession: session,
      withSudo: withSudo,
    );
  }

  Future<void> _moveSelectedRemoteTo(
    _SftpRemotePaneSession session, {
    bool withSudo = false,
  }) async {
    final entry = _selectedRemoteEntry(session);
    final connection = session.connection;
    final auth = connection?.auth;
    if (entry == null || entry.isParent || connection == null || auth == null) {
      return;
    }
    final location = _sftpPaneLocation(
      _slotForRemoteSession(session),
      _SftpPaneEndpoint.remote,
    );
    bool isCurrent() =>
        _sftpPaneLocationIsCurrent(location) &&
        _selectedRemoteEntry(session)?.path == entry.path;
    final input = await _promptSftpInput(
      titleKey: 'sftp.label.moveTo',
      label: 'sftp.field.destinationPath',
      initialValue: entry.path,
      confirmLabel: 'sftp.task.move',
      withSudo: withSudo,
    );
    if (input == null || !mounted) {
      return;
    }
    if (!_checkSftpTransferScope(isCurrent(), _sftpTransferScopeChanged)) {
      return;
    }
    final targetPath = _normalizeRemoteSftpPath(input, base: location.path);
    if (targetPath == entry.path) {
      return;
    }
    final target = await _prepareRemoteSftpTarget(
      session,
      auth,
      targetPath,
      isCurrent: isCurrent,
    );
    if (!_checkSftpTransferScope(isCurrent(), _sftpTransferScopeChanged) ||
        target == null) {
      return;
    }
    _enqueueSftpTask(
      type: _SftpTaskType.move,
      slot: _slotForRemoteSession(session),
      displayName: '${entry.name} -> ${target.targetPath}',
      sourcePath: entry.path,
      targetPath: target.targetPath,
      auth: auth,
      operation: {
        'op': _SftpTaskType.move.wireName,
        'source_path': entry.path,
        'target_path': target.targetPath,
        'replace_existing': target.replaceExisting,
      },
      refreshRemote: true,
      remoteSession: session,
      withSudo: withSudo,
    );
  }

  Future<void> _copySelectedRemoteTo(
    _SftpRemotePaneSession session, {
    bool withSudo = false,
  }) async {
    final entry = _selectedRemoteEntry(session);
    final connection = session.connection;
    final auth = connection?.auth;
    if (entry == null || entry.isParent || connection == null || auth == null) {
      return;
    }
    final location = _sftpPaneLocation(
      _slotForRemoteSession(session),
      _SftpPaneEndpoint.remote,
    );
    bool isCurrent() =>
        _sftpPaneLocationIsCurrent(location) &&
        _selectedRemoteEntry(session)?.path == entry.path;
    final input = await _promptSftpInput(
      titleKey: 'sftp.label.copyTo',
      label: 'sftp.field.destinationPath',
      initialValue: _defaultRemoteCopyPath(session, entry),
      confirmLabel: 'common.action.copy',
      withSudo: withSudo,
    );
    if (input == null || !mounted) {
      return;
    }
    if (!_checkSftpTransferScope(isCurrent(), _sftpTransferScopeChanged)) {
      return;
    }
    final targetPath = _normalizeRemoteSftpPath(input, base: location.path);
    if (targetPath == entry.path) {
      return;
    }
    final target = await _prepareRemoteSftpTarget(
      session,
      auth,
      targetPath,
      isCurrent: isCurrent,
    );
    if (!_checkSftpTransferScope(isCurrent(), _sftpTransferScopeChanged) ||
        target == null) {
      return;
    }
    _enqueueSftpTask(
      type: _SftpTaskType.copy,
      slot: _slotForRemoteSession(session),
      displayName: '${entry.name} -> ${target.targetPath}',
      sourcePath: entry.path,
      targetPath: target.targetPath,
      auth: auth,
      operation: {
        'op': _SftpTaskType.copy.wireName,
        'source_path': entry.path,
        'target_path': target.targetPath,
        'replace_existing': target.replaceExisting,
      },
      refreshRemote: true,
      remoteSession: session,
      withSudo: withSudo,
    );
  }

  Future<void> _deleteSelectedRemote(
    _SftpRemotePaneSession session, {
    bool withSudo = false,
  }) async {
    final entries = _selectedRemoteEntries(session);
    final connection = session.connection;
    final auth = connection?.auth;
    if (entries.isEmpty || connection == null || auth == null) {
      return;
    }
    final confirmed = await _confirmDeleteRemoteEntries(entries);
    if (!confirmed || !mounted) {
      return;
    }
    for (final entry in entries) {
      _enqueueSftpTask(
        type: _SftpTaskType.delete,
        slot: _slotForRemoteSession(session),
        displayName: entry.name,
        sourcePath: entry.path,
        targetPath: entry.path,
        auth: auth,
        operation: {
          'op': _SftpTaskType.delete.wireName,
          'target_path': entry.path,
        },
        refreshRemote: true,
        remoteSession: session,
        withSudo: withSudo,
      );
    }
  }

  void _createRemoteFolder(
    _SftpRemotePaneSession session, {
    bool withSudo = false,
  }) {
    final connection = session.connection;
    final auth = connection?.auth;
    if (connection == null || auth == null) {
      return;
    }
    final defaultName = tr('sftp.action.newFolder', fallback: 'New Folder');
    var name = defaultName;
    var suffix = 2;
    final existingNames = {for (final entry in session.entries) entry.name};
    while (existingNames.contains(name)) {
      name = '$defaultName $suffix';
      suffix += 1;
    }
    final targetPath = _joinRemoteSftpPath(session.path, name);
    _enqueueSftpTask(
      type: _SftpTaskType.newFolder,
      slot: _slotForRemoteSession(session),
      displayName: name,
      sourcePath: session.path,
      targetPath: targetPath,
      auth: auth,
      operation: {
        'op': _SftpTaskType.newFolder.wireName,
        'target_path': targetPath,
      },
      refreshRemote: true,
      remoteSession: session,
      persistHistory: false,
      withSudo: withSudo,
    );
  }

  String _defaultRemoteCopyPath(
    _SftpRemotePaneSession session,
    _SftpFileEntry entry,
  ) {
    final parent = _remoteParentPath(entry.path) ?? session.path;
    final existingTargets = _remoteSftpSessionSiblingTargets(
      session,
      entry.path,
    );
    var copyIndex = 1;
    while (true) {
      final targetPath = _joinRemoteSftpPath(
        parent,
        _remoteSftpCopyName(entry.name, copyIndex),
      );
      if (!existingTargets.contains(_collapseRemoteSftpPath(targetPath))) {
        return targetPath;
      }
      copyIndex += 1;
    }
  }

  Future<String?> _promptSftpInput({
    required String titleKey,
    required String label,
    required String initialValue,
    required String confirmLabel,
    required bool withSudo,
  }) async {
    if (!mounted) {
      return null;
    }
    final controller = TextEditingController(text: initialValue);
    try {
      return await _showSftpWorkspaceDialog<String>(
        builder: (dialogContext) {
          void submit() {
            final value = controller.text.trim();
            Navigator.of(dialogContext).pop(value.isEmpty ? null : value);
          }

          final action = dialogContext.tr(titleKey);
          return _WorkspaceDialogFrame(
            title: Text(
              withSudo
                  ? dialogContext.tr(
                      'sftp.sudo.action.title',
                      args: {'action': action},
                    )
                  : action,
            ),
            content: _WorkspaceInput(
              controller: controller,
              label: label,
              autofocus: true,
              textInputAction: TextInputAction.done,
              onSubmitted: (_) => submit(),
            ),
            actions: [
              _WorkspaceButton(
                label: 'common.action.cancel',
                variant: _WorkspaceButtonVariant.text,
                onPressed: () => Navigator.of(dialogContext).pop(),
              ),
              _WorkspaceButton(
                label: confirmLabel,
                type: _WorkspaceButtonType.primary,
                variant: _WorkspaceButtonVariant.solid,
                onPressed: submit,
              ),
            ],
          );
        },
      );
    } finally {
      controller.dispose();
    }
  }

  Future<bool> _confirmDeleteRemoteEntries(List<_SftpFileEntry> entries) async {
    if (!mounted || entries.isEmpty) {
      return false;
    }
    final singleEntry = entries.length == 1 ? entries.single : null;
    final result = await _showSftpWorkspaceDialog<bool>(
      builder: (dialogContext) {
        return _WorkspaceConfirmDialog(
          title: Text(
            dialogContext.tr(
              'sftp.delete.dialog.title',
              fallback: 'Delete remote items?',
            ),
          ),
          details:
              singleEntry?.name ??
              dialogContext.tr(
                'sftp.delete.selectedCount',
                fallback: '{count} remote items',
                args: {'count': entries.length},
              ),
          message: 'sftp.delete.confirmation.description',
          confirmLabel: 'common.action.delete',
        );
      },
    );
    return result ?? false;
  }

  void _handleAction(
    _SftpLocalPaneSession session,
    _SftpPaneSlot slot,
    _SftpAction action, {
    SftpExternalEditorCommand? application,
  }) {
    final selected = _selectedEntry(session);
    switch (action) {
      case _SftpAction.copyName:
      case _SftpAction.copyAbsolutePath:
        unawaited(
          copySftpEntryText(
            _sortSftpEntries(
              _selectedEntries(session),
              session.sortColumn,
              ascending: session.sortAscending,
            ).map((entry) => (name: entry.name, path: entry.path)),
            absolutePath: action == _SftpAction.copyAbsolutePath,
          ),
        );
      case _SftpAction.open:
        if (selected != null) {
          _openEntry(session, selected, openFile: true);
        }
      case _SftpAction.refresh:
        unawaited(_loadLocalPath(session, session.path, recordHistory: false));
      case _SftpAction.newFolder:
        unawaited(_createFolder(session));
      case _SftpAction.showHiddenFiles:
        _toggleHiddenFiles(session);
      case _SftpAction.selectAll:
        if (session.entries.isNotEmpty) {
          _selectAll(session);
        }
      case _SftpAction.close:
        _closeLocalEndpoint(slot);
      case _SftpAction.openWithExternalEditor:
        if (selected != null &&
            !selected.isDirectory &&
            !selected.isParent &&
            application != null) {
          unawaited(
            _openLocalFileWithApplication(selected, application: application),
          );
        }
      case _SftpAction.openWithOtherApplication:
        if (selected != null && !selected.isDirectory && !selected.isParent) {
          unawaited(
            _openLocalFileWithOtherApplication(session, slot, selected),
          );
        }
      case _SftpAction.moveTo:
      case _SftpAction.copyTo:
      case _SftpAction.uploadFiles:
      case _SftpAction.openWith:
      case _SftpAction.openWithSshEditor:
      case _SftpAction.copyToTarget:
      case _SftpAction.rename:
      case _SftpAction.delete:
      case _SftpAction.withSudo:
      case _SftpAction.sudoRename:
      case _SftpAction.sudoMoveTo:
      case _SftpAction.sudoCopyTo:
      case _SftpAction.sudoDelete:
      case _SftpAction.sudoNewFolder:
      case _SftpAction.sudoOpen:
      case _SftpAction.sudoDownload:
      case _SftpAction.sudoOpenWith:
      case _SftpAction.sudoOpenWithSshEditor:
      case _SftpAction.sudoUpload:
        break;
    }
  }

  void _handleRemoteAction(
    _SftpRemotePaneSession session,
    _SftpAction action, {
    SftpExternalEditorCommand? application,
    bool withSudo = false,
  }) {
    final connection = session.connection;
    final auth = connection?.auth;
    final selected = _selectedRemoteEntry(session);
    switch (action) {
      case _SftpAction.copyName:
      case _SftpAction.copyAbsolutePath:
        unawaited(
          copySftpEntryText(
            _sortSftpEntries(
              _selectedRemoteEntries(session),
              session.sortColumn,
              ascending: session.sortAscending,
            ).map((entry) => (name: entry.name, path: entry.path)),
            absolutePath: action == _SftpAction.copyAbsolutePath,
          ),
        );
      case _SftpAction.open:
        if (selected != null) {
          _openRemoteEntry(
            session,
            selected,
            openFile: true,
            withSudo: withSudo,
          );
        }
      case _SftpAction.sudoOpen:
        if (selected != null) {
          _openRemoteEntry(session, selected, openFile: true, withSudo: true);
        }
      case _SftpAction.refresh:
        if (connection != null && auth != null) {
          unawaited(
            _loadRemotePath(
              session,
              auth,
              session.path,
              host: connection.host,
              recordHistory: false,
            ),
          );
        }
      case _SftpAction.showHiddenFiles:
        _toggleRemoteHiddenFiles(session);
      case _SftpAction.selectAll:
        final selectable = [
          for (final entry in session.entries)
            if (!entry.isParent) entry,
        ];
        if (selectable.isNotEmpty) {
          _setSftpState(() {
            session.selectedPath = selectable.first.path;
            session.selectionAnchorPath = selectable.first.path;
            session.selectedPaths
              ..clear()
              ..addAll(selectable.map((entry) => entry.path));
          });
        }
      case _SftpAction.close:
        _closeRemoteEndpoint(session);
      case _SftpAction.copyToTarget:
        unawaited(
          _downloadSelectedRemoteToDownloadsDirectory(
            session,
            withSudo: withSudo,
          ),
        );
      case _SftpAction.sudoDownload:
        unawaited(
          _downloadSelectedRemoteToDownloadsDirectory(session, withSudo: true),
        );
      case _SftpAction.uploadFiles:
        unawaited(_pickLocalFilesForRemoteUpload(session, withSudo: withSudo));
      case _SftpAction.sudoUpload:
        unawaited(_pickLocalFilesForRemoteUpload(session, withSudo: true));
      case _SftpAction.rename:
        unawaited(_renameSelectedRemote(session, withSudo: withSudo));
      case _SftpAction.sudoRename:
        unawaited(_renameSelectedRemote(session, withSudo: true));
      case _SftpAction.moveTo:
        unawaited(_moveSelectedRemoteTo(session, withSudo: withSudo));
      case _SftpAction.sudoMoveTo:
        unawaited(_moveSelectedRemoteTo(session, withSudo: true));
      case _SftpAction.copyTo:
        unawaited(_copySelectedRemoteTo(session, withSudo: withSudo));
      case _SftpAction.sudoCopyTo:
        unawaited(_copySelectedRemoteTo(session, withSudo: true));
      case _SftpAction.delete:
        unawaited(_deleteSelectedRemote(session, withSudo: withSudo));
      case _SftpAction.sudoDelete:
        unawaited(_deleteSelectedRemote(session, withSudo: true));
      case _SftpAction.newFolder:
        _createRemoteFolder(session, withSudo: withSudo);
      case _SftpAction.sudoNewFolder:
        _createRemoteFolder(session, withSudo: true);
      case _SftpAction.openWithExternalEditor:
        if (selected != null &&
            !selected.isDirectory &&
            !selected.isParent &&
            application != null) {
          unawaited(
            _openRemoteFileWithApplication(
              session,
              selected,
              application: application,
              withSudo: withSudo,
            ),
          );
        }
      case _SftpAction.openWithOtherApplication:
        if (selected != null && !selected.isDirectory && !selected.isParent) {
          unawaited(
            _openRemoteFileWithOtherApplication(
              session,
              selected,
              withSudo: withSudo,
            ),
          );
        }
      case _SftpAction.openWithSshEditor:
      case _SftpAction.sudoOpenWithSshEditor:
        if (selected != null && !selected.isDirectory && !selected.isParent) {
          _openRemoteFileWithSshEditor(
            session,
            selected,
            withSudo: withSudo || action == _SftpAction.sudoOpenWithSshEditor,
          );
        }
      case _SftpAction.openWith:
      case _SftpAction.withSudo:
      case _SftpAction.sudoOpenWith:
        break;
    }
  }

  _SftpFileEntry? _selectedEntry(_SftpLocalPaneSession session) {
    final selectedPath = session.selectedPath;
    if (selectedPath == null) {
      return null;
    }
    for (final entry in session.entries) {
      if (entry.path == selectedPath) {
        return entry;
      }
    }
    return null;
  }

  List<_SftpFileEntry> _selectedEntries(_SftpLocalPaneSession session) {
    final selectedPaths = session.selectedPaths.isEmpty
        ? {?session.selectedPath}
        : session.selectedPaths;
    return [
      for (final entry in session.entries)
        if (!entry.isParent && selectedPaths.contains(entry.path)) entry,
    ];
  }

  _SftpFileEntry? _selectedRemoteEntry(_SftpRemotePaneSession session) {
    final selectedPath = session.selectedPath;
    if (selectedPath == null) {
      return null;
    }
    for (final entry in session.entries) {
      if (entry.path == selectedPath) {
        return entry;
      }
    }
    return null;
  }

  List<_SftpFileEntry> _selectedRemoteEntries(_SftpRemotePaneSession session) {
    final selectedPaths = session.selectedPaths.isEmpty
        ? {?session.selectedPath}
        : session.selectedPaths;
    return [
      for (final entry in session.entries)
        if (!entry.isParent && selectedPaths.contains(entry.path)) entry,
    ];
  }

  Future<void> _dispatchSftpMenuAction(VoidCallback action) async {
    await WidgetsBinding.instance.endOfFrame;
    if (!mounted) {
      return;
    }
    action();
  }

  Future<T?> _showSftpWorkspaceDialog<T>({
    required WidgetBuilder builder,
    bool barrierDismissible = true,
  }) async {
    NautermOverlayScope.find(context)?.dismissTransientOverlays();
    FocusManager.instance.primaryFocus?.unfocus();
    requestMainWindowFocus();
    return showNautermDialog<T>(
      context: context,
      barrierDismissible: barrierDismissible,
      builder: (dialogContext) {
        final dialog = builder(dialogContext);
        final colors = widget.compact ? widget.panelColors : null;
        return colors == null
            ? dialog
            : _WorkspaceDialogThemeScope(colors: colors, child: dialog);
      },
    );
  }

  void _showEntryMenu(
    _SftpLocalPaneSession session,
    _SftpPaneSlot slot,
    TapDownDetails details,
    _SftpFileEntry entry,
  ) {
    unawaited(_showLocalEntryMenu(session, slot, details, entry));
  }

  Future<List<SystemFileApplication>> _loadSftpFileApplications(
    String fileName,
  ) async {
    final applications = await loadSystemApplicationsForFileName(fileName);
    if (!sftpExternalEditorSupportsFileName(fileName)) {
      return applications;
    }
    final configuredEditor = sftpExternalEditor;
    if (configuredEditor == null ||
        applications.any(
          (application) => application.command.id == configuredEditor.id,
        )) {
      return applications;
    }
    final configuredApplication = await loadSystemApplication(configuredEditor);
    if (configuredApplication == null) {
      return applications;
    }
    return [...applications, configuredApplication];
  }

  Future<_SftpMenuCommand?> _showSftpFileMenu({
    required Offset position,
    required _SftpFileEntry entry,
    required bool remote,
    bool sshEditorAvailable = false,
    _AiAssistantColors? colors,
  }) async {
    final menuGroups = _sftpRowMenuGroups(
      entry,
      remote: remote,
      sshEditorAvailable: sshEditorAvailable,
    );
    List<NautermContextMenuEntry<_SftpMenuCommand>> buildEntries(
      List<SystemFileApplication> applications, {
      String? catalogStatus,
    }) => _sftpFileContextMenuEntries(
      menuGroups,
      fileName: entry.name,
      applications: applications,
      sshEditorAvailable: sshEditorAvailable,
      catalogStatus: catalogStatus,
    );

    final loading =
        io.Platform.isMacOS && !entry.isDirectory && !entry.isParent;
    final initialEntries = buildEntries(
      const [],
      catalogStatus: loading
          ? tr(
              'applications.catalog.loading',
              fallback: 'Loading applications…',
            )
          : null,
    );
    final updates = loading ? ValueNotifier(initialEntries) : null;
    var closed = false;
    if (updates != null) {
      unawaited(() async {
        try {
          final applications = await _loadSftpFileApplications(entry.name);
          if (!closed) updates.value = buildEntries(applications);
        } on Object catch (error, stackTrace) {
          NautermLog.warning(
            'sftp',
            'Unable to load applications',
            error: error,
            stackTrace: stackTrace,
          );
          if (!closed) {
            updates.value = buildEntries(
              const [],
              catalogStatus: tr(
                'applications.catalog.error',
                fallback: 'Unable to load applications',
              ),
            );
          }
        }
      }());
    }
    try {
      return await showNautermContextMenu<_SftpMenuCommand>(
        context: context,
        position: position,
        width: colors == null ? 220 : 210,
        style: colors == null ? null : _terminalSftpMenuStyle(colors),
        scaleAnimation: false,
        entries: initialEntries,
        updatedEntries: updates,
      );
    } finally {
      closed = true;
      updates?.dispose();
    }
  }

  Future<void> _showLocalEntryMenu(
    _SftpLocalPaneSession session,
    _SftpPaneSlot slot,
    TapDownDetails details,
    _SftpFileEntry entry,
  ) async {
    _setSftpState(() {
      if (session.selectedPaths.contains(entry.path)) {
        session.selectedPath = entry.path;
      } else {
        _selectLocalEntry(session, entry);
      }
    });
    final location = _sftpPaneLocation(slot, _SftpPaneEndpoint.local);
    final command = await _showSftpFileMenu(
      position: details.globalPosition,
      entry: entry,
      remote: false,
    );
    if (command == null) return;
    await _dispatchSftpMenuAction(() {
      if (!_sftpPaneLocationIsCurrent(location) ||
          session.selectedPath != entry.path) {
        return;
      }
      _handleAction(
        session,
        slot,
        command.action,
        application: command.application,
      );
    });
  }

  void _showBlankMenu(
    _SftpLocalPaneSession session,
    _SftpPaneSlot slot,
    TapDownDetails details,
  ) {
    _setSftpState(() {
      _clearLocalSelection(session);
    });
    unawaited(
      showNautermContextMenu<_SftpAction>(
        context: context,
        position: details.globalPosition,
        scaleAnimation: false,
        entries: _sftpContextMenuEntries(
          _sftpActionMenuGroups(
            hasSelection: false,
            showHiddenFiles: session.showHiddenFiles,
            remote: false,
            showCloseAction: true,
            sshEditorAvailable: false,
          ),
        ),
      ).then((action) {
        if (action != null) {
          unawaited(
            _dispatchSftpMenuAction(() => _handleAction(session, slot, action)),
          );
        }
      }),
    );
  }

  Future<void> _showRemoteEntryMenu(
    _SftpRemotePaneSession session,
    TapDownDetails details,
    _SftpFileEntry entry, {
    _AiAssistantColors? colors,
  }) async {
    _setSftpState(() {
      if (session.selectedPaths.contains(entry.path)) {
        session.selectedPath = entry.path;
      } else {
        _selectRemoteEntry(session, entry);
      }
    });
    final location = _sftpPaneLocation(
      _slotForRemoteSession(session),
      _SftpPaneEndpoint.remote,
    );
    final command = await _showSftpFileMenu(
      position: details.globalPosition,
      entry: entry,
      remote: true,
      sshEditorAvailable: _remoteSessionSharesSshEditor(session),
      colors: colors,
    );
    if (command == null) return;
    await _dispatchSftpMenuAction(() {
      if (!_sftpPaneLocationIsCurrent(location) ||
          session.selectedPath != entry.path) {
        return;
      }
      _handleRemoteAction(
        session,
        command.action,
        application: command.application,
        withSudo: command.withSudo,
      );
    });
  }

  void _showRemoteBlankMenu(
    _SftpRemotePaneSession session,
    TapDownDetails details,
  ) {
    _setSftpState(() {
      _clearRemoteSelection(session);
    });
    unawaited(
      showNautermContextMenu<_SftpAction>(
        context: context,
        position: details.globalPosition,
        scaleAnimation: false,
        entries: _sftpContextMenuEntries(
          _sftpActionMenuGroups(
            hasSelection: false,
            showHiddenFiles: session.showHiddenFiles,
            remote: true,
            showCloseAction: !widget.remoteOnly,
            sshEditorAvailable: _remoteSessionSharesSshEditor(session),
          ),
        ),
      ).then((action) {
        if (action != null) {
          unawaited(
            _dispatchSftpMenuAction(() => _handleRemoteAction(session, action)),
          );
        }
      }),
    );
  }

  @override
  Widget build(BuildContext context) {
    ref.watch(_sftpPaneControllerProvider(_controllerKey));
    return ColoredBox(
      color: _surface,
      child: widget.remoteOnly
          ? _buildFileDropPane(_SftpPaneSlot.right, _buildRemotePane())
          : LayoutBuilder(
              builder: (context, constraints) {
                final leftWidth = constraints.maxWidth * 0.5;
                return Row(
                  children: [
                    SizedBox(
                      width: leftWidth,
                      child: _buildFileDropPane(
                        _SftpPaneSlot.left,
                        _buildLeftPane(),
                      ),
                    ),
                    VerticalDivider(
                      width: 1,
                      thickness: 1,
                      color: _sidebarDivider,
                    ),
                    Expanded(
                      child: _buildFileDropPane(
                        _SftpPaneSlot.right,
                        _buildPaneForSlot(_SftpPaneSlot.right),
                      ),
                    ),
                  ],
                );
              },
            ),
    );
  }

  Widget _buildFileDropPane(_SftpPaneSlot slot, Widget child) {
    final key = switch (slot) {
      _SftpPaneSlot.left => _leftPaneKey,
      _SftpPaneSlot.right => _rightPaneKey,
    };
    final highlighted = _fileDropHoverSlot == slot;
    return KeyedSubtree(
      key: key,
      child: DragTarget<_SftpPaneDragPayload>(
        key: ValueKey('sftp-transfer-target:${slot.name}'),
        onWillAcceptWithDetails: (details) {
          final accepted = _canAcceptSftpPaneDrag(slot, details.data);
          if (accepted) _updateSftpPaneDragHover(slot, details.data);
          return accepted;
        },
        onMove: (details) => _updateSftpPaneDragHover(slot, details.data),
        onLeave: (_) {
          if (_fileDropHoverSlot == slot) _clearFileDropHover();
        },
        onAcceptWithDetails: (details) =>
            _acceptSftpPaneDrag(slot, details.data),
        builder: (context, candidateData, rejectedData) => Stack(
          fit: StackFit.expand,
          children: [
            Positioned.fill(child: child),
            if (highlighted)
              Positioned.fill(
                child: IgnorePointer(
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      color: const Color(0x26298df2),
                      border: Border.all(
                        color: const Color(0xaa298df2),
                        width: 2,
                      ),
                    ),
                    child: Center(
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          color: _workspaceMenuBackground.withValues(
                            alpha: 0.94,
                          ),
                          borderRadius: BorderRadius.circular(7),
                          border: Border.all(
                            color: _blue.withValues(alpha: 0.55),
                          ),
                        ),
                        child: Padding(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 7,
                          ),
                          child: Text(
                            context.tr(
                              _fileDropHoverLabelKey ??
                                  'sftp.action.uploadHere',
                            ),
                            style: TextStyle(
                              color: _text,
                              fontSize: NautermFontSizes.labelMedium,
                              fontWeight: NautermFontWeights.semibold,
                              letterSpacing: 0,
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }

  Widget _buildLeftPane() {
    return _buildPaneForSlot(_SftpPaneSlot.left);
  }

  Widget _buildPaneForSlot(_SftpPaneSlot slot) {
    final endpoint = switch (slot) {
      _SftpPaneSlot.left => _leftPaneEndpoint,
      _SftpPaneSlot.right => _rightPaneEndpoint,
    };
    if (endpoint == _SftpPaneEndpoint.remote) {
      return _buildRemotePane(slot: slot);
    }
    final session = _localSessionForSlot(slot);
    return _SftpLocalPane(
      title: tr('common.label.local', fallback: 'Local'),
      icon: Icons.drive_folder_upload_rounded,
      iconColor: const Color(0xff075e92),
      remote: false,
      onEndpointTap: () => _showEndpointSelector(slot),
      path: session.path,
      pathController: session.pathController,
      pathFocusNode: session.pathFocusNode,
      filterController: session.filterController,
      entries: session.entries,
      selectedPath: session.selectedPath,
      selectedPaths: session.selectedPaths,
      selectionAnchorPath: session.selectionAnchorPath,
      sortColumn: session.sortColumn,
      sortAscending: session.sortAscending,
      loading: session.loading,
      loadError: session.loadError,
      editingPath: session.editingPath,
      canGoBack: session.backStack.isNotEmpty,
      canGoForward: session.forwardStack.isNotEmpty,
      showHiddenFiles: session.showHiddenFiles,
      hasSelection: _selectedEntries(session).isNotEmpty,
      showCloseAction: true,
      sshEditorAvailable: false,
      tasks: const [],
      taskListOpen: false,
      favoriteListOpen: false,
      favoritePaths: const [],
      onHome: () => _goLocalHome(session),
      onBack: () => _goBack(session),
      onForward: () => _goForward(session),
      onPathEditRequested: () {
        _setSftpState(() {
          session.editingPath = true;
        });
        _focusPathInput(session.pathFocusNode, session.pathController);
      },
      onPathSubmitted: (value) => unawaited(_loadLocalPath(session, value)),
      onPathEditCancelled: () {
        _setSftpState(() {
          session.editingPath = false;
          session.pathController.text = session.path;
        });
      },
      onSelectionChanged: (selection) {
        _setSftpState(() {
          _setLocalSelection(session, selection);
        });
      },
      onEntryDoubleTap: (entry) => _openEntry(session, entry),
      onEntrySecondaryTapDown: (details, entry) =>
          _showEntryMenu(session, slot, details, entry),
      onBlankSecondaryTapDown: (details) =>
          _showBlankMenu(session, slot, details),
      onTaskListToggle: () {},
      onTaskClearCompleted: () {},
      onTaskDismiss: _dismissSftpTask,
      onTaskCancel: _cancelSftpTask,
      onTaskPauseToggle: _toggleSftpTaskPause,
      canRetryTask: _controller.taskManager.canRetry,
      onPathPanelDismiss: () {},
      onPathFavoriteToggle: () {},
      onFavoriteListToggle: () {},
      onFavoritePathSelected: (_) {},
      onSortChanged: (column) => _sortLocalEntriesBy(session, column),
      onAction: (action) => _handleAction(session, slot, action),
      createDragPayload: (entries) => _SftpPaneDragPayload(
        source: _sftpPaneLocation(slot, _SftpPaneEndpoint.local),
        entries: entries,
      ),
    );
  }

  Widget _buildRemotePane({_SftpPaneSlot? slot}) {
    final session = _remoteSessionForSlot(slot);
    final selectingHost = session.selectingHost;
    final connection = session.connection;
    final compactColors = widget.compact ? widget.panelColors : null;
    if (connection != null &&
        connection.phase != _SftpConnectionPhase.connected &&
        !selectingHost) {
      if (compactColors != null) {
        return _TerminalSftpConnectionView(
          colors: compactColors,
          state: connection,
          onClose: () => _closeRemoteEndpoint(session),
          onRetry: () => _retrySftpConnection(session),
          onTrustOnceAndRetry: () => _trustAndRetrySftpConnection(
            session,
            SshHostKeyTrustMode.acceptOnce,
          ),
          onTrustAndRetry: () => _trustAndRetrySftpConnection(
            session,
            SshHostKeyTrustMode.acceptAndSave,
          ),
          onChangeHost: () => _showSftpHostSelector(session),
        );
      }
      return _SftpConnectionPage(
        state: connection,
        onRetry: () => _retrySftpConnection(session),
        onTrustOnceAndRetry: () => _trustAndRetrySftpConnection(
          session,
          SshHostKeyTrustMode.acceptOnce,
        ),
        onTrustAndRetry: () => _trustAndRetrySftpConnection(
          session,
          SshHostKeyTrustMode.acceptAndSave,
        ),
        onChangeHost: () => _showSftpHostSelector(session),
        onClose: slot == null ? null : () => _closeRemoteEndpoint(session),
        onSshSelected: widget.onSshSelected,
      );
    }

    if (connection != null && !selectingHost) {
      final auth = connection.auth;
      final paneSlot = slot ?? _SftpPaneSlot.right;
      final favoritesStatus =
          session.loadingFavorites || session.favoritesError != null
          ? _SftpFavoritesStatus(
              error: session.favoritesError,
              color: compactColors?.muted ?? _mutedText,
              onRetry: () =>
                  unawaited(_loadRemoteSftpFavorites(session, connection.host)),
            )
          : null;
      final toggleFavorite =
          session.loadingFavorites || session.favoritesError != null
          ? null
          : () => _toggleRemoteSftpFavoritePath(session, connection.host);
      if (compactColors != null) {
        return _TerminalSftpBrowser(
          colors: compactColors,
          operationBanner: _sftpOperationBanner(session, colors: compactColors),
          title: connection.host.name,
          path: session.path,
          pathController: session.pathController,
          pathFocusNode: session.pathFocusNode,
          filterController: session.filterController,
          entries: session.entries,
          selectedPath: session.selectedPath,
          selectedPaths: session.selectedPaths,
          selectionAnchorPath: session.selectionAnchorPath,
          sortColumn: session.sortColumn,
          sortAscending: session.sortAscending,
          loading: session.loading,
          loadError: session.loadError,
          editingPath: session.editingPath,
          canGoBack: session.backStack.isNotEmpty,
          canGoForward: session.forwardStack.isNotEmpty,
          showHiddenFiles: session.showHiddenFiles,
          sshEditorAvailable: _remoteSessionSharesSshEditor(session),
          tasks: _tasksForSlot(paneSlot),
          taskListOpen:
              _controller.taskListOpen && _controller.taskListSlot == paneSlot,
          favoriteListOpen:
              _controller.favoriteListOpen &&
              _controller.favoriteListSlot == paneSlot,
          favoritePaths: session.favoritePaths,
          favoritesStatus: favoritesStatus,
          onHome: () => _goRemoteHome(session),
          onSshDirectory:
              _remoteSessionSharesSshEditor(session) &&
                  widget.sshWorkingDirectoryResolver != null
              ? () => _goRemoteSshDirectory(session)
              : null,
          followSshDirectory: session.followSshDirectory,
          onFollowSshDirectory:
              session.followSshDirectory ||
                  _remoteSessionSharesSshEditor(session)
              ? () => _toggleSshDirectoryFollow(session)
              : null,
          onBack: () => _goRemoteBack(session),
          onForward: () => _goRemoteForward(session),
          onPathEditRequested: () {
            _setSftpState(() => session.editingPath = true);
            _focusPathInput(session.pathFocusNode, session.pathController);
          },
          onPathSubmitted: auth == null
              ? (_) {}
              : (value) => unawaited(
                  _loadRemotePath(session, auth, value, host: connection.host),
                ),
          onPathEditCancelled: () {
            _setSftpState(() {
              session.editingPath = false;
              session.pathController.text = session.path;
            });
            _scheduleSshDirectoryFollow();
          },
          onSelectionChanged: (selection) {
            _setSftpState(() => _setRemoteSelection(session, selection));
          },
          onEntryDoubleTap: (entry) => _openRemoteEntry(session, entry),
          onEntrySecondaryTapDown: (details, entry) => unawaited(
            _showRemoteEntryMenu(
              session,
              details,
              entry,
              colors: compactColors,
            ),
          ),
          onRefresh: auth == null
              ? () {}
              : () => unawaited(
                  _loadRemotePath(
                    session,
                    auth,
                    session.path,
                    host: connection.host,
                    recordHistory: false,
                  ),
                ),
          onTaskListToggle: () => _toggleSftpTaskList(paneSlot),
          onTaskClearCompleted: _clearCompletedSftpTasks,
          onTaskDismiss: _dismissSftpTask,
          onTaskCancel: _cancelSftpTask,
          onTaskPauseToggle: _toggleSftpTaskPause,
          canRetryTask: _controller.taskManager.canRetry,
          onPanelsDismiss: _dismissSftpPathPanels,
          onPathFavoriteToggle: toggleFavorite,
          onFavoriteListToggle: () => _toggleSftpFavoriteList(paneSlot),
          onFavoritePathSelected: auth == null
              ? (_) {}
              : (path) {
                  _setSftpState(() => _controller.favoriteListOpen = false);
                  unawaited(
                    _loadRemotePath(session, auth, path, host: connection.host),
                  );
                },
          onSortChanged: (column) => _sortRemoteEntriesBy(session, column),
          onAction: (action) => _handleRemoteAction(session, action),
        );
      }
      return _SftpLocalPane(
        title: connection.host.name,
        operationBanner: _sftpOperationBanner(session),
        icon: Icons.cloud_sync_rounded,
        iconColor: connection.host.color,
        remote: true,
        onEndpointTap: slot == null ? null : () => _showEndpointSelector(slot),
        path: session.path,
        pathController: session.pathController,
        pathFocusNode: session.pathFocusNode,
        filterController: session.filterController,
        entries: session.entries,
        selectedPath: session.selectedPath,
        selectedPaths: session.selectedPaths,
        selectionAnchorPath: session.selectionAnchorPath,
        sortColumn: session.sortColumn,
        sortAscending: session.sortAscending,
        loading: session.loading,
        loadError: session.loadError,
        editingPath: session.editingPath,
        canGoBack: session.backStack.isNotEmpty,
        canGoForward: session.forwardStack.isNotEmpty,
        showHiddenFiles: session.showHiddenFiles,
        hasSelection: _selectedRemoteEntries(session).isNotEmpty,
        showCloseAction: slot != null,
        sshEditorAvailable: _remoteSessionSharesSshEditor(session),
        onSshSelected: widget.onSshSelected,
        showSshDirectory: widget.sshWorkingDirectoryResolver != null,
        followSshDirectory: session.followSshDirectory,
        onFollowSshDirectory:
            session.followSshDirectory || _remoteSessionSharesSshEditor(session)
            ? () => _toggleSshDirectoryFollow(session)
            : null,
        onSshDirectory:
            !session.loading && _remoteSessionSharesSshEditor(session)
            ? () => _goRemoteSshDirectory(session)
            : null,
        tasks: _tasksForSlot(paneSlot),
        taskListOpen:
            _controller.taskListOpen && _controller.taskListSlot == paneSlot,
        favoriteListOpen:
            _controller.favoriteListOpen &&
            _controller.favoriteListSlot == paneSlot,
        favoritePaths: session.favoritePaths,
        favoritesStatus: favoritesStatus,
        onHome: () => _goRemoteHome(session),
        onBack: () => _goRemoteBack(session),
        onForward: () => _goRemoteForward(session),
        onPathEditRequested: () {
          _setSftpState(() {
            session.editingPath = true;
          });
          _focusPathInput(session.pathFocusNode, session.pathController);
        },
        onPathSubmitted: auth == null
            ? (_) {}
            : (value) => unawaited(
                _loadRemotePath(session, auth, value, host: connection.host),
              ),
        onPathEditCancelled: () {
          _setSftpState(() {
            session.editingPath = false;
            session.pathController.text = session.path;
          });
          _scheduleSshDirectoryFollow();
        },
        onSelectionChanged: (selection) {
          _setSftpState(() {
            _setRemoteSelection(session, selection);
          });
        },
        onEntryDoubleTap: (entry) => _openRemoteEntry(session, entry),
        onEntrySecondaryTapDown: (details, entry) =>
            unawaited(_showRemoteEntryMenu(session, details, entry)),
        onBlankSecondaryTapDown: (details) =>
            _showRemoteBlankMenu(session, details),
        onTaskListToggle: () => _toggleSftpTaskList(paneSlot),
        onTaskClearCompleted: _clearCompletedSftpTasks,
        onTaskDismiss: _dismissSftpTask,
        onTaskCancel: _cancelSftpTask,
        onTaskPauseToggle: _toggleSftpTaskPause,
        canRetryTask: _controller.taskManager.canRetry,
        onPathPanelDismiss: _dismissSftpPathPanels,
        onPathFavoriteToggle: toggleFavorite,
        onFavoriteListToggle: () => _toggleSftpFavoriteList(paneSlot),
        onFavoritePathSelected: auth == null
            ? (_) {}
            : (path) {
                _setSftpState(() => _controller.favoriteListOpen = false);
                unawaited(
                  _loadRemotePath(session, auth, path, host: connection.host),
                );
              },
        onSortChanged: (column) => _sortRemoteEntriesBy(session, column),
        onAction: (action) => _handleRemoteAction(session, action),
        createDragPayload: (entries) => _SftpPaneDragPayload(
          source: _sftpPaneLocation(paneSlot, _SftpPaneEndpoint.remote),
          entries: entries,
        ),
      );
    }

    if (selectingHost) {
      if (compactColors != null) {
        return _TerminalSftpHostSelector(
          colors: compactColors,
          groups: widget.groups,
          hosts: widget.hosts,
          tags: widget.tags,
          searchController: _hostSearchController,
          onBack: () => _setSftpState(() => session.selectingHost = false),
          onHostSelected: (host) {
            _setSftpState(() {
              session.selectingHost = false;
              _pendingConnectSlot = null;
            });
            widget.onHostSelected(host);
          },
        );
      }
      return _SftpHostSelectorPane(
        groups: widget.groups,
        hosts: widget.hosts,
        tags: widget.tags,
        searchController: _hostSearchController,
        onBack: () {
          if (slot == null) {
            _setSftpState(() => session.selectingHost = false);
          } else if (connection == null) {
            _useLocalEndpoint(slot);
          } else {
            _setSftpState(() {
              session.selectingHost = false;
            });
          }
        },
        onUseLocal: slot == null
            ? () => _setSftpState(() => session.selectingHost = false)
            : () => _useLocalEndpoint(slot),
        onHostSelected: slot == null
            ? (host) {
                _setSftpState(() {
                  session.selectingHost = false;
                  _pendingConnectSlot = null;
                });
                widget.onHostSelected(host);
              }
            : (host) => _handleEndpointHostSelected(slot, host),
      );
    }

    if (compactColors != null) {
      final closed = session.closedEndpoint;
      if (closed != null) {
        return _TerminalToolEmptyState(
          icon: LucideIcons.folderOpen,
          title: context.tr(
            'sftp.connection.closed',
            fallback: 'SFTP connection closed',
          ),
          description: _sftpConnectionTarget(closed.host),
          colors: compactColors,
          actionLabel: context.tr(
            'common.label.reconnect',
            fallback: 'Reconnect',
          ),
          onAction: () => _startSftpConnection(closed, session),
        );
      }
      return _TerminalToolEmptyState(
        icon: LucideIcons.folderOpen,
        title: 'No SFTP connection',
        description: 'Choose an SSH host to browse its files.',
        colors: compactColors,
        actionLabel: 'Select host',
        onAction: () => _setSftpState(() => session.selectingHost = true),
      );
    }
    return _SftpRemoteEmptyState(
      onUseLocal: slot == null ? null : () => _useLocalEndpoint(slot),
      onSelectHost: () {
        if (slot == null) {
          _setSftpState(() {
            session.selectingHost = true;
          });
        } else {
          _showEndpointSelector(slot);
        }
      },
    );
  }
}
