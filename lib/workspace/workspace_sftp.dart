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
  bool showHiddenFiles = sftpShowHiddenFiles;
  bool editingPath = false;
  bool selectingHost = false;
  final List<String> favoritePaths = <String>[];
  int loadGeneration = 0;
  _SftpListingCancellation? listingCancellation;
  _SftpConnectionState? connection;
  String get sudoSessionId => '$sessionId:with-sudo';
  bool sudoSessionAuthenticated = false;
  Object? loadError;

  void dispose() {
    listingCancellation?.cancel();
    listingCancellation = null;
    FfiSftpTaskExecutor.closeSudoSession(sudoSessionId);
    sudoSessionAuthenticated = false;
    pathController.dispose();
    filterController.dispose();
    pathFocusNode.dispose();
  }
}

enum _SftpNameConflictResolution { cancel, keepBoth, replace }

enum _SftpSudoPromptKind { explicit, permissionDenied }

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
  bool showHiddenFiles = sftpShowHiddenFiles;
  bool editingPath = false;
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

enum _ExternalEditSaveResult { failed, uploaded, localReplaced }

class _ExternalSftpEditSession {
  _ExternalSftpEditSession({
    required this.localPath,
    required this.tempDirectory,
    required this.protectFile,
    required this.onSaved,
  });

  final String localPath;
  final io.Directory tempDirectory;
  final Future<bool> Function(String path) protectFile;
  final Future<_ExternalEditSaveResult> Function(String uploadPath) onSaved;
  static const _sampleInterval = Duration(milliseconds: 350);
  StreamSubscription<io.FileSystemEvent>? _watcher;
  _ExternalEditLocalSnapshot? _lastUploaded;
  _ExternalEditLocalSnapshot? _pendingUpload;
  Future<void>? _uploadWorker;
  int _sampleGeneration = 0;
  bool _closed = false;
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
    _pendingUpload = snapshot;
    _uploadWorker ??= _drainUploads().whenComplete(() {
      _uploadWorker = null;
      if (_pendingUpload case final pending? when pending != _lastUploaded) {
        _queueUpload(pending);
      }
    });
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
  String? _fileDropHoverLabel;
  final Map<String, _ExternalSftpEditSession> _externalEditSessions =
      <String, _ExternalSftpEditSession>{};
  final Map<String, Future<void>> _externalEditUploadLocks =
      <String, Future<void>>{};
  late final Future<void> Function() _shutdownHook;
  bool _shuttingDown = false;
  Future<void>? _shutdownFuture;

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
    _controller.leftPaneEndpoint = value;
  }

  _SftpPaneEndpoint get _rightPaneEndpoint => _controller.rightPaneEndpoint;

  set _rightPaneEndpoint(_SftpPaneEndpoint value) {
    _controller.rightPaneEndpoint = value;
  }

  _SftpPaneSlot? get _pendingConnectSlot => _controller.pendingConnectSlot;

  set _pendingConnectSlot(_SftpPaneSlot? value) {
    _controller.pendingConnectSlot = value;
  }

  @override
  void initState() {
    super.initState();
    _shutdownHook = _flushAndClose;
    _workspaceShutdownHooks.add(_shutdownHook);
    _controllerKey = widget.sessionId;
    _controller = ref.read(_sftpPaneControllerProvider(_controllerKey));
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
    _workspaceShutdownHooks.remove(_shutdownHook);
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

  Future<void> _performFlushAndClose() async {
    await _fileDropSubscription?.cancel();
    _fileDropSubscription = null;
    _leftRemote.listingCancellation?.cancel();
    _leftRemote.listingCancellation = null;
    _rightRemote.listingCancellation?.cancel();
    _rightRemote.listingCancellation = null;

    await Future.wait([
      for (final session in _externalEditSessions.values) session.close(),
    ]);
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
      _loadRemoteSftpFavorites(_leftRemote, leftConnection.host);
    }
    final rightConnection = _rightRemote.connection;
    if (rightConnection != null) {
      _loadRemoteSftpFavorites(_rightRemote, rightConnection.host);
    }
  }

  void _loadRemoteSftpFavorites(
    _SftpRemotePaneSession session,
    _HostItem host,
  ) {
    if (host.id <= 0) {
      _setSftpState(session.favoritePaths.clear);
      return;
    }
    final store = widget.dataStore;
    if (store == null) {
      _setSftpState(session.favoritePaths.clear);
      return;
    }
    try {
      final paths = _favoritePathsFromEntries(
        store.listSftpFavoritePaths(
          scope: SftpFavoriteScope.remote,
          hostId: host.id,
        ),
      );
      _setSftpState(() => _replaceFavoritePaths(session.favoritePaths, paths));
    } catch (_) {
      return;
    }
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
    _setSftpState(() {
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
    _setSftpState(() {
      switch (slot) {
        case _SftpPaneSlot.left:
          _leftPaneEndpoint = _SftpPaneEndpoint.local;
        case _SftpPaneSlot.right:
          _rightPaneEndpoint = _SftpPaneEndpoint.local;
      }
      session.selectingHost = false;
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
    _closeRemoteSudoSession(session);
    session.loadGeneration += 1;
    _setSftpState(() {
      session.selectingHost = false;
      session.connection = _SftpConnectionState.connecting(request.host);
      session.path = '~';
      session.pathController.text = session.path;
      session.entries = const [];
      _clearRemoteSelection(session);
      session.loadError = null;
      session.loading = true;
      session.editingPath = false;
      session.backStack.clear();
      session.forwardStack.clear();
    });
    _loadRemoteSftpFavorites(session, request.host);

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
    final retryRequestId = (_handledConnectRequestId ?? 0) + 1;
    _handledConnectRequestId = retryRequestId;
    _startSftpConnection(
      _SftpConnectRequest(
        id: retryRequestId,
        host: connection.host,
        auth: auth,
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
    final retryRequestId = (_handledConnectRequestId ?? 0) + 1;
    _handledConnectRequestId = retryRequestId;
    _startSftpConnection(
      _SftpConnectRequest(
        id: retryRequestId,
        host: connection.host,
        auth: auth.copyWith(hostKeyTrustMode: hostKeyTrustMode),
      ),
      session,
    );
  }

  void _showSftpHostSelector(_SftpRemotePaneSession session) {
    _closeRemoteSudoSession(session);
    _setSftpState(() {
      session.connection = null;
      session.selectingHost = true;
    });
  }

  void _closeSftpConnectionPage(_SftpRemotePaneSession session) {
    _closeRemoteSudoSession(session);
    session.listingCancellation?.cancel();
    session.listingCancellation = null;
    session.loadGeneration += 1;
    _setSftpState(() {
      session.connection = null;
      session.selectingHost = false;
      session.entries = const [];
      _clearRemoteSelection(session);
      session.loadError = null;
      session.loading = false;
    });
  }

  void _closeRemoteEndpoint(_SftpRemotePaneSession session) {
    _closeSftpConnectionPage(session);
  }

  void _closeRemoteSudoSession(_SftpRemotePaneSession session) {
    FfiSftpTaskExecutor.closeSudoSession(session.sudoSessionId);
    session.sudoSessionAuthenticated = false;
  }

  Future<void> _loadLocalPath(
    _SftpLocalPaneSession session,
    String path, {
    bool recordHistory = true,
    bool clearForward = true,
  }) async {
    final normalized = _normalizeSftpPath(path);
    _setSftpState(() {
      session.loading = true;
      session.loadError = null;
      session.editingPath = false;
    });

    try {
      final directory = io.Directory(normalized);
      final entries = await _listSftpDirectory(session, directory);
      if (!mounted) {
        return;
      }
      _setSftpState(() {
        if (recordHistory && normalized != session.path) {
          session.backStack.add(session.path);
          if (clearForward) {
            session.forwardStack.clear();
          }
        }
        session.path = normalized;
        session.pathController.text = normalized;
        session.entries = entries;
        _clearLocalSelection(session);
        session.loading = false;
      });
    } on Object catch (error) {
      if (!mounted) {
        return;
      }
      _setSftpState(() {
        session.loadError = error;
        session.loading = false;
      });
    }
  }

  Future<List<_SftpFileEntry>> _listSftpDirectory(
    _SftpLocalPaneSession session,
    io.Directory directory,
  ) async {
    final entities = await directory.list(followLinks: false).toList();
    final entries = <_SftpFileEntry>[];
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
      final stat = await entity.stat();
      final type = stat.type;
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
    return entries;
  }

  Future<void> _loadRemotePath(
    _SftpRemotePaneSession session,
    _SftpRemoteAuth auth,
    String path, {
    required _HostItem host,
    bool recordHistory = true,
    bool clearForward = true,
    bool initial = false,
  }) async {
    final generation = ++session.loadGeneration;
    session.listingCancellation?.cancel();
    final cancellation = _SftpListingCancellation(_allocateSftpNativeTaskId());
    session.listingCancellation = cancellation;
    final normalized = _normalizeRemoteSftpPath(path, base: session.path);
    _setSftpState(() {
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
  }) async {
    final result = await _spawnSftpDirectoryEntryListing(
      auth.toArguments(directory),
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
    if (auth == null) return;
    final localSession = identical(session, _rightRemote)
        ? _leftLocal
        : _rightLocal;
    final tempDir = await io.Directory.systemTemp.createTemp('nauterm-edit-');
    if (!await _ensurePrivateExternalEditDirectory(tempDir.path)) {
      try {
        await tempDir.delete(recursive: true);
      } on Object {
        // Temp cleanup is best-effort.
      }
      return;
    }
    final localPath = _joinSftpPath(tempDir.path, entry.name);
    final task = await _enqueueSftpTask(
      type: _SftpTaskType.download,
      slot: _slotForRemoteSession(session),
      displayName: application == null
          ? 'Open ${entry.name}'
          : 'Open ${entry.name} with ${application.label}',
      sourcePath: entry.path,
      targetPath: localPath,
      auth: auth,
      operation: {
        'op': _SftpTaskType.download.wireName,
        'remote_path': entry.path,
        'local_path': localPath,
      },
      refreshLocal: false,
      localSession: localSession,
      remoteSession: session,
      persistHistory: false,
      withSudo: withSudo,
    );
    if (!mounted || task.status != _SftpTaskStatus.completed) {
      try {
        await tempDir.delete(recursive: true);
      } on Object {
        // Temp cleanup is best-effort.
      }
      return;
    }
    if (!await _ensurePrivateExternalEditFile(localPath)) {
      try {
        await tempDir.delete(recursive: true);
      } on Object {
        // Temp cleanup is best-effort.
      }
      return;
    }
    _ExternalEditRemoteVersion remoteVersion;
    try {
      remoteVersion = withSudo
          ? _ExternalEditRemoteVersion(
              modified: entry.modified,
              size: entry.size,
            )
          : await _readRemoteExternalEditVersion(session, auth, entry.path) ??
                _ExternalEditRemoteVersion(
                  modified: entry.modified,
                  size: entry.size,
                );
    } on Object catch (error) {
      _showSftpSnack('Unable to verify ${entry.name}: $error');
      try {
        await tempDir.delete(recursive: true);
      } on Object {
        // Temp cleanup is best-effort.
      }
      return;
    }
    final started = await _startExternalApplication(
      localPath,
      application: application,
    );
    if (!started) {
      try {
        await tempDir.delete(recursive: true);
      } on Object {
        // Temp cleanup is best-effort.
      }
      return;
    }
    if (!mounted) return;
    final target = _ExternalSftpEditTarget(
      remotePath: entry.path,
      version: remoteVersion,
    );
    final editSession = _ExternalSftpEditSession(
      localPath: localPath,
      tempDirectory: tempDir,
      protectFile: _ensurePrivateExternalEditFile,
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
            _showSftpSnack('Failed to save ${entry.name}: $error');
          }
          return _ExternalEditSaveResult.failed;
        }
      },
    );
    _externalEditSessions[localPath]?.dispose();
    _externalEditSessions[localPath] = editSession;
    await editSession.startWatching();
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
        _showSftpSnack('Unable to protect temporary edit file: $error');
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
        _showSftpSnack('Unable to protect temporary edit directory: $error');
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
        _showSftpSnack('File no longer exists: ${entry.path}');
      }
      return;
    }
    await _startExternalApplication(entry.path, application: application);
  }

  Future<void> _openLocalFileWithOtherApplication(_SftpFileEntry entry) async {
    final application = await chooseSystemFileApplication();
    if (application == null) return;
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
    final application = await chooseSystemFileApplication();
    if (application == null) return;
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
        _showSftpSnack('Failed to open $target: $error');
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
  }) async {
    if (!mounted || _shuttingDown) return _ExternalEditSaveResult.failed;
    if (!await _ensurePrivateExternalEditFile(uploadSourcePath)) {
      return _ExternalEditSaveResult.failed;
    }

    final _ExternalEditRemoteVersion? currentVersion;
    try {
      currentVersion = withSudo
          ? target.version
          : await _readRemoteExternalEditVersion(
              session,
              auth,
              target.remotePath,
            );
    } on Object catch (error) {
      _showSftpSnack('Unable to check ${entry.name} before upload: $error');
      return _ExternalEditSaveResult.failed;
    }

    var remoteUploadPath = target.remotePath;
    var replaceExisting = true;
    if (currentVersion == null || !target.version.matches(currentVersion)) {
      final resolution = await _promptExternalEditConflict(
        entryName: entry.name,
        remotePath: target.remotePath,
        remoteMissing: currentVersion == null,
      );
      if (!mounted) return _ExternalEditSaveResult.failed;
      switch (resolution) {
        case _ExternalEditConflictResolution.cancel:
          _showSftpSnack('Upload paused; the remote file was not changed.');
          return _ExternalEditSaveResult.failed;
        case _ExternalEditConflictResolution.keepRemote:
          if (currentVersion == null) {
            _showSftpSnack('The remote file no longer exists.');
            return _ExternalEditSaveResult.failed;
          }
          final kept = await _downloadRemoteEditVersion(
            session,
            auth,
            target.remotePath,
            localPath,
            entry.name,
          );
          if (kept) {
            target.version = currentVersion;
            _showSftpSnack('Kept the remote version of ${entry.name}.');
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
      },
      refreshRemote: true,
      remoteSession: session,
      withSudo: withSudo,
    );
    if (!mounted) return _ExternalEditSaveResult.failed;
    if (task.status != _SftpTaskStatus.completed) {
      _showSftpSnack(
        'Failed to upload $uploadName: ${task.error ?? 'Unknown error'}',
      );
      return _ExternalEditSaveResult.failed;
    }

    try {
      final uploadedVersion = withSudo
          ? _ExternalEditRemoteVersion(
              modified: DateTime.now(),
              size: await io.File(uploadSourcePath).length(),
            )
          : await _readRemoteExternalEditVersion(
              session,
              auth,
              remoteUploadPath,
            );
      if (uploadedVersion != null) target.version = uploadedVersion;
    } on Object catch (error) {
      _showSftpSnack('Saved $uploadName, but verification failed: $error');
    }
    if (remoteUploadPath != target.remotePath) {
      target.remotePath = remoteUploadPath;
      _showSftpSnack('Saved the edited file as $uploadName.');
    }
    return _ExternalEditSaveResult.uploaded;
  }

  Future<_ExternalEditRemoteVersion?> _readRemoteExternalEditVersion(
    _SftpRemotePaneSession session,
    _SftpRemoteAuth auth,
    String remotePath,
  ) async {
    final parent = _remoteParentPath(remotePath) ?? session.path;
    final listing = await _listRemoteSftpDirectory(
      session,
      auth,
      parent,
      includeHidden: true,
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
    String displayName,
  ) async {
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
    );
    if (task.status != _SftpTaskStatus.completed) {
      if (mounted) {
        _showSftpSnack(
          'Failed to keep the remote version: '
          '${task.error ?? 'Unknown error'}',
        );
      }
      return false;
    }
    return _ensurePrivateExternalEditFile(localPath);
  }

  Future<_ExternalEditConflictResolution> _promptExternalEditConflict({
    required String entryName,
    required String remotePath,
    required bool remoteMissing,
  }) async {
    if (!mounted) return _ExternalEditConflictResolution.cancel;
    final result = await _showSftpWorkspaceDialog<_ExternalEditConflictResolution>(
      barrierDismissible: false,
      builder: (dialogContext) {
        final colors = widget.compact ? widget.panelColors : null;
        return _WorkspaceDialogFrame(
          width: 540,
          title: Text(
            tr('sftp.label.remoteFileChanged', fallback: 'Remote file changed'),
          ),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                remoteMissing
                    ? '$entryName was removed from the server after editing began.'
                    : '$entryName was changed on the server after editing began.',
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
            ],
          ),
          actions: [
            _WorkspaceButton(
              label: 'Cancel',
              variant: _WorkspaceButtonVariant.text,
              onPressed: () => Navigator.of(
                dialogContext,
              ).pop(_ExternalEditConflictResolution.cancel),
            ),
            if (!remoteMissing)
              _WorkspaceButton(
                label: 'Keep Remote',
                variant: _WorkspaceButtonVariant.filled,
                onPressed: () => Navigator.of(
                  dialogContext,
                ).pop(_ExternalEditConflictResolution.keepRemote),
              ),
            _WorkspaceButton(
              label: 'Save a Copy',
              variant: _WorkspaceButtonVariant.filled,
              onPressed: () => Navigator.of(
                dialogContext,
              ).pop(_ExternalEditConflictResolution.saveCopy),
            ),
            _WorkspaceButton(
              label: 'Overwrite Remote',
              type: _WorkspaceButtonType.error,
              variant: _WorkspaceButtonVariant.solid,
              onPressed: () => Navigator.of(
                dialogContext,
              ).pop(_ExternalEditConflictResolution.overwriteRemote),
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
    ScaffoldMessenger.maybeOf(context)?.showSnackBar(
      SnackBar(
        content: Text(tr(message)),
        duration: const Duration(seconds: 3),
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
  }) {
    if (_shuttingDown) {
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
      withSudo: withSudo,
    );
    _controller.taskManager.registerOwner(id, this);
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
        _persistSftpTaskHistory(nextTask.id);
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
              (finishedTask?.status == _SftpTaskStatus.failed &&
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
    final arguments = execution.auth.toArguments(session.path)
      ..['taskId'] = execution.nativeTaskId;
    var lastProgressAt = DateTime.fromMillisecondsSinceEpoch(0);
    var lastProgressBytes = -1;

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
      arguments['operation'] = operation;
      return _spawnSftpTask(arguments, onProgress: updateProgress);
    }

    var operation = execution.operation;
    String? initialSudoPassword;
    final usedCachedChannel =
        execution.withSudo && session.sudoSessionAuthenticated;
    if (execution.withSudo) {
      if (!usedCachedChannel) {
        initialSudoPassword = await _promptSftpSudoPassword(
          kind: _SftpSudoPromptKind.explicit,
        );
      }
      if (usedCachedChannel || initialSudoPassword != null) {
        operation = {
          ...execution.operation,
          'sudo_session_id': session.sudoSessionId,
        };
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
      if (result.ok) {
        session.sudoSessionAuthenticated = true;
      } else if (_sftpSudoAuthenticationFailed(result.error)) {
        FfiSftpTaskExecutor.closeSudoSession(session.sudoSessionId);
        session.sudoSessionAuthenticated = false;
        if (usedCachedChannel) {
          final replacementPassword = await _promptSftpSudoPassword(
            kind: _SftpSudoPromptKind.explicit,
            error: result.error,
          );
          if (replacementPassword != null) {
            result = await run({
              ...execution.operation,
              'sudo_session_id': session.sudoSessionId,
              'sudo_password': replacementPassword,
            });
            if (result.ok) {
              session.sudoSessionAuthenticated = true;
            }
          }
        }
      }
    } else if (!result.ok && _sftpPermissionDenied(result.error)) {
      final oneShotPassword = await _promptSftpSudoPassword(
        kind: _SftpSudoPromptKind.permissionDenied,
        error: result.error,
      );
      if (oneShotPassword != null) {
        result = await run({
          ...execution.operation,
          'sudo_password': oneShotPassword,
        });
      }
    }
    final cancelled = result.error?.toLowerCase().contains('cancelled') == true;
    final latestTask = _tasks.where((task) => task.id == taskId).firstOrNull;
    final paused =
        !result.ok && cancelled && latestTask?.pauseRequested == true;
    if (cancelled && !paused) {
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
      _persistSftpTaskHistory(taskId);
    }
    if (!paused) {
      _completeSftpTask(taskId);
    }
    if (!paused && !execution.persistHistory && result.ok) {
      _removeSftpTaskFromMemory(taskId);
    }
    if (result.ok) {
      if (execution.refreshLocal) {
        final sessionToRefresh = execution.localSession ?? _leftLocal;
        unawaited(
          _loadLocalPath(
            sessionToRefresh,
            sessionToRefresh.path,
            recordHistory: false,
          ),
        );
      }
      if (execution.refreshRemote) {
        final connection = session.connection;
        final currentAuth = connection?.auth;
        if (connection != null && currentAuth != null) {
          unawaited(
            _loadRemotePath(
              session,
              currentAuth,
              session.path,
              host: connection.host,
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
  }) async {
    if (!mounted) return null;
    final controller = TextEditingController();
    final explicit = kind == _SftpSudoPromptKind.explicit;
    try {
      return await _showSftpWorkspaceDialog<String>(
        barrierDismissible: false,
        builder: (dialogContext) {
          void submit() {
            final password = controller.text;
            if (password.isEmpty) return;
            Navigator.of(dialogContext).pop(password);
          }

          return _WorkspaceDialogFrame(
            title: Text(
              tr(
                explicit
                    ? 'sftp.sudo.explicit.dialog.title'
                    : 'sftp.sudo.permissionDenied.dialog.title',
                fallback: explicit
                    ? 'Authenticate with sudo'
                    : 'Permission denied',
              ),
            ),
            content: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  tr(
                    explicit
                        ? 'sftp.sudo.explicit.dialog.description'
                        : 'sftp.sudo.permissionDenied.dialog.description',
                    fallback: explicit
                        ? 'Enter the sudo password to authorize this and future With sudo actions for 15 minutes.'
                        : 'The server denied this operation. Enter the sudo password to retry this task once.',
                  ),
                ),
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
                _WorkspaceInput(
                  controller: controller,
                  label: tr('common.label.password', fallback: 'Password'),
                  autofocus: true,
                  obscureText: true,
                  clearable: false,
                  textInputAction: TextInputAction.done,
                  onSubmitted: (_) => submit(),
                ),
              ],
            ),
            actions: [
              _WorkspaceButton(
                label: tr('common.action.cancel', fallback: 'Cancel'),
                variant: _WorkspaceButtonVariant.text,
                onPressed: () => Navigator.of(dialogContext).pop(),
              ),
              _WorkspaceButton(
                label: tr(
                  explicit
                      ? 'sftp.sudo.explicit.action.authenticate'
                      : 'sftp.sudo.permissionDenied.action.retry',
                  fallback: explicit ? 'Authenticate' : 'Retry with sudo',
                ),
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

  void _persistSftpTaskHistory(int taskId) {
    final store = widget.dataStore;
    if (store == null) return;
    final task = _tasks.where((task) => task.id == taskId).firstOrNull;
    final finishedAt = task?.finishedAt;
    if (task == null || finishedAt == null || task.historyId != null) return;
    final execution = _controller.taskExecutions[taskId];
    final auth = execution?.auth;
    final connection = execution?.remoteSession?.connection;
    if (connection == null ||
        connection.host.id <= 0 ||
        connection.host.uuid == null) {
      return;
    }
    try {
      final historyId = store.saveSftpTaskHistory(
        SftpTaskHistoryEntry(
          hostUuid: connection.host.uuid,
          type: _persistedSftpTaskType(task.type),
          host: auth?.host ?? '',
          username: auth?.username ?? '',
          port: auth?.port ?? 22,
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
      if (execution?.persistHistory ?? true) {
        _persistSftpTaskHistory(taskId);
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
        task.status != _SftpTaskStatus.paused) {
      return;
    }
    final nativeTaskId = DateTime.now().microsecondsSinceEpoch + taskId;
    _controller.taskExecutions[taskId] = execution.withNativeTaskId(
      nativeTaskId,
    );
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
    if (task?.status == _SftpTaskStatus.paused) {
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
    if (normalizedPath.isEmpty) {
      return;
    }
    final remove = session.favoritePaths.contains(normalizedPath);
    if (host.id > 0) {
      _persistSftpFavoritePathChange(
        scope: SftpFavoriteScope.remote,
        hostId: host.id,
        path: normalizedPath,
        remove: remove,
      );
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
    try {
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
    } catch (_) {
      return;
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
    required String actionLabel,
    Set<String>? reservedTargets,
    Map<String, Set<String>>? siblingCache,
  }) async {
    final normalizedTarget = _collapseRemoteSftpPath(targetPath);
    final siblingTargets = await _remoteSftpSiblingTargets(
      session,
      auth,
      normalizedTarget,
      cache: siblingCache,
    );
    if (reservedTargets != null) {
      siblingTargets.addAll(reservedTargets);
    }
    if (!siblingTargets.contains(normalizedTarget)) {
      return _SftpResolvedRemoteTarget(
        targetPath: normalizedTarget,
        replaceExisting: false,
      );
    }

    final resolution = await _promptRemoteNameConflict(
      actionLabel: actionLabel,
      name: _remoteSftpName(normalizedTarget),
      targetPath: normalizedTarget,
    );
    if (!mounted || resolution == _SftpNameConflictResolution.cancel) {
      return null;
    }
    if (resolution == _SftpNameConflictResolution.replace) {
      return _SftpResolvedRemoteTarget(
        targetPath: normalizedTarget,
        replaceExisting: true,
      );
    }
    return _SftpResolvedRemoteTarget(
      targetPath: _uniqueRemoteSftpPath(normalizedTarget, siblingTargets),
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
    required String actionLabel,
    required String name,
    required String targetPath,
  }) async {
    if (!mounted) {
      return _SftpNameConflictResolution.cancel;
    }
    final result = await _showSftpWorkspaceDialog<_SftpNameConflictResolution>(
      builder: (dialogContext) {
        final colors = widget.compact ? widget.panelColors : null;
        return _WorkspaceDialogFrame(
          width: 460,
          title: Text(tr('$actionLabel name conflict')),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                tr('An item named "$name" already exists in the destination.'),
                style: TextStyle(
                  color: colors?.foreground ?? _text,
                  fontSize: NautermFontSizes.labelLarge,
                  fontWeight: NautermFontWeights.regular,
                  height: 1.35,
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
                    targetPath,
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
            ],
          ),
          actions: [
            _WorkspaceButton(
              label: 'Cancel',
              variant: _WorkspaceButtonVariant.text,
              onPressed: () => Navigator.of(
                dialogContext,
              ).pop(_SftpNameConflictResolution.cancel),
            ),
            _WorkspaceButton(
              label: 'Keep Both',
              variant: _WorkspaceButtonVariant.filled,
              onPressed: () => Navigator.of(
                dialogContext,
              ).pop(_SftpNameConflictResolution.keepBoth),
            ),
            _WorkspaceButton(
              label: 'Replace',
              type: _WorkspaceButtonType.primary,
              variant: _WorkspaceButtonVariant.solid,
              onPressed: () => Navigator.of(
                dialogContext,
              ).pop(_SftpNameConflictResolution.replace),
            ),
          ],
        );
      },
    );
    return result ?? _SftpNameConflictResolution.cancel;
  }

  Future<void> _uploadLocalEntriesToRemote(
    List<_SftpFileEntry> entries,
    _SftpRemotePaneSession remoteSession, {
    bool withSudo = false,
  }) async {
    final connection = remoteSession.connection;
    final auth = connection?.auth;
    if (entries.isEmpty || connection == null || auth == null) {
      return;
    }
    final reservedTargets = <String>{};
    final siblingCache = <String, Set<String>>{};
    for (final entry in entries) {
      final originalTargetPath = _joinRemoteSftpPath(
        remoteSession.path,
        entry.name,
      );
      final target = await _prepareRemoteSftpTarget(
        remoteSession,
        auth,
        originalTargetPath,
        actionLabel: 'Upload',
        reservedTargets: reservedTargets,
        siblingCache: siblingCache,
      );
      if (!mounted) {
        return;
      }
      if (target == null) {
        continue;
      }
      final targetPath = target.targetPath;
      reservedTargets.add(_collapseRemoteSftpPath(targetPath));
      _enqueueSftpTask(
        type: _SftpTaskType.upload,
        slot: _slotForRemoteSession(remoteSession),
        displayName: targetPath == originalTargetPath
            ? entry.name
            : '${entry.name} -> ${_remoteSftpName(targetPath)}',
        sourcePath: entry.path,
        targetPath: targetPath,
        auth: auth,
        operation: {
          'op': _SftpTaskType.upload.wireName,
          'local_path': entry.path,
          'remote_path': targetPath,
          'replace_existing': target.replaceExisting,
        },
        refreshRemote: true,
        remoteSession: remoteSession,
        withSudo: withSudo,
      );
    }
  }

  Future<void> _downloadSelectedRemoteToDownloadsDirectory(
    _SftpRemotePaneSession session, {
    bool withSudo = false,
  }) async {
    final entries = _selectedRemoteEntries(session);
    if (entries.isEmpty) return;
    final downloadsDirectory = await _controller.taskManager.downloadsDirectory;
    if (downloadsDirectory == null) {
      if (mounted) {
        _showSftpSnack('Unable to use the system Downloads directory.');
      }
      return;
    }
    if (!mounted) return;
    await _downloadRemoteEntriesToDirectory(
      entries,
      session,
      _normalizeLocalSftpSeparators(downloadsDirectory.path),
      withSudo: withSudo,
    );
  }

  void _downloadRemoteEntriesToLocal(
    List<_SftpFileEntry> entries,
    _SftpRemotePaneSession session,
    _SftpLocalPaneSession localSession,
  ) {
    unawaited(
      _downloadRemoteEntriesToDirectory(
        entries,
        session,
        localSession.path,
        localSession: localSession,
      ),
    );
  }

  Future<void> _downloadRemoteEntriesToDirectory(
    List<_SftpFileEntry> entries,
    _SftpRemotePaneSession session,
    String targetDirectory, {
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
      final targetPath = entries.length == 1
          ? (await _uniqueLocalSftpPath(targetDirectory, entry.name))
          : (await _uniqueLocalSftpPathWithReserved(
              targetDirectory,
              entry.name,
              reservedTargets,
            ));
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
    List<_SftpFileEntry> entries,
    _SftpRemotePaneSession sourceSession,
    _SftpRemotePaneSession targetSession,
  ) async {
    final sourceAuth = sourceSession.connection?.auth;
    final targetAuth = targetSession.connection?.auth;
    if (entries.isEmpty || sourceAuth == null || targetAuth == null) {
      return;
    }

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
          targetSession.path,
          entry.name,
        );
        final target = await _prepareRemoteSftpTarget(
          targetSession,
          targetAuth,
          originalTargetPath,
          actionLabel: 'Upload',
          reservedTargets: reservedTargets,
          siblingCache: siblingCache,
        );
        if (!mounted) return;
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
        if (!mounted || downloadTask.status != _SftpTaskStatus.completed) {
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
        _showSftpSnack('Unable to protect temporary transfer data: $error');
      }
      return false;
    }
  }

  bool _canAcceptSftpPaneDrag(
    _SftpPaneSlot targetSlot,
    _SftpPaneDragPayload payload,
  ) {
    if (payload.entries.isEmpty || payload.sourceSlot == targetSlot) {
      return false;
    }
    final targetEndpoint = switch (targetSlot) {
      _SftpPaneSlot.left => _leftPaneEndpoint,
      _SftpPaneSlot.right => _rightPaneEndpoint,
    };
    if (payload.sourceRemote) {
      return targetEndpoint == _SftpPaneEndpoint.local ||
          (targetEndpoint == _SftpPaneEndpoint.remote &&
              _fileDropRemoteSessionForSlot(targetSlot) != null);
    }
    return targetEndpoint == _SftpPaneEndpoint.remote &&
        _fileDropRemoteSessionForSlot(targetSlot) != null;
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
              ? 'Upload here'
              : 'Download here'
        : 'Upload here';
    if (_fileDropHoverSlot == targetSlot && _fileDropHoverLabel == label) {
      return;
    }
    setState(() {
      _fileDropHoverSlot = targetSlot;
      _fileDropHoverLabel = label;
    });
  }

  void _acceptSftpPaneDrag(
    _SftpPaneSlot targetSlot,
    _SftpPaneDragPayload payload,
  ) {
    _clearFileDropHover();
    if (!_canAcceptSftpPaneDrag(targetSlot, payload)) return;
    if (payload.sourceRemote) {
      final sourceSession = _remoteSessionForSlot(payload.sourceSlot);
      final targetEndpoint = switch (targetSlot) {
        _SftpPaneSlot.left => _leftPaneEndpoint,
        _SftpPaneSlot.right => _rightPaneEndpoint,
      };
      if (targetEndpoint == _SftpPaneEndpoint.remote) {
        unawaited(
          _copyRemoteEntriesToRemote(
            payload.entries,
            sourceSession,
            _remoteSessionForSlot(targetSlot),
          ),
        );
      } else {
        _downloadRemoteEntriesToLocal(
          payload.entries,
          sourceSession,
          _localSessionForSlot(targetSlot),
        );
      }
      return;
    }
    final remoteSession = _remoteSessionForSlot(targetSlot);
    unawaited(_uploadLocalEntriesToRemote(payload.entries, remoteSession));
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
      if (session.connection?.phase == _SftpConnectionPhase.connected) {
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

    final candidates = <_SftpRemotePaneSession>[
      if (_rightPaneEndpoint == _SftpPaneEndpoint.remote) _rightRemote,
      if (_leftPaneEndpoint == _SftpPaneEndpoint.remote) _leftRemote,
      _rightRemote,
      _leftRemote,
    ];
    for (final session in candidates) {
      if (session.connection?.phase == _SftpConnectionPhase.connected) {
        return session;
      }
    }
    return null;
  }

  void _updateFileDropHover(NautermFileDropEvent event) {
    final slot = _fileDropTargetSlot(event);
    final targetSlot =
        slot != null && _fileDropRemoteSessionForSlot(slot) != null
        ? slot
        : null;
    if (_fileDropHoverSlot == targetSlot &&
        _fileDropHoverLabel == (targetSlot == null ? null : 'Upload here')) {
      return;
    }
    setState(() {
      _fileDropHoverSlot = targetSlot;
      _fileDropHoverLabel = targetSlot == null ? null : 'Upload here';
    });
  }

  void _clearFileDropHover() {
    if (_fileDropHoverSlot == null && _fileDropHoverLabel == null) {
      return;
    }
    setState(() {
      _fileDropHoverSlot = null;
      _fileDropHoverLabel = null;
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
    unawaited(_uploadLocalPathsToRemote(event.paths, auth, session));
  }

  Future<void> _uploadLocalPathsToRemote(
    List<String> paths,
    _SftpRemoteAuth auth,
    _SftpRemotePaneSession session, {
    bool withSudo = false,
  }) async {
    final uniquePaths = <String>{};
    final reservedTargets = <String>{};
    final siblingCache = <String, Set<String>>{};
    for (final rawPath in paths) {
      final path = rawPath.trim();
      if (path.isEmpty || !uniquePaths.add(path)) {
        continue;
      }
      final type = io.FileSystemEntity.typeSync(path, followLinks: false);
      if (type == io.FileSystemEntityType.notFound) {
        continue;
      }
      final name = _basename(path);
      if (name.isEmpty) {
        continue;
      }
      final originalTargetPath = _joinRemoteSftpPath(session.path, name);
      final target = await _prepareRemoteSftpTarget(
        session,
        auth,
        originalTargetPath,
        actionLabel: 'Upload',
        reservedTargets: reservedTargets,
        siblingCache: siblingCache,
      );
      if (!mounted) {
        return;
      }
      if (target == null) {
        continue;
      }
      final targetPath = target.targetPath;
      reservedTargets.add(_collapseRemoteSftpPath(targetPath));
      _enqueueSftpTask(
        type: _SftpTaskType.upload,
        slot: _slotForRemoteSession(session),
        displayName: targetPath == originalTargetPath
            ? name
            : '$name -> ${_remoteSftpName(targetPath)}',
        sourcePath: path,
        targetPath: targetPath,
        auth: auth,
        operation: {
          'op': _SftpTaskType.upload.wireName,
          'local_path': path,
          'remote_path': targetPath,
          'replace_existing': target.replaceExisting,
        },
        refreshRemote: true,
        remoteSession: session,
        withSudo: withSudo,
      );
    }
  }

  Future<void> _pickLocalFilesForRemoteUpload(
    _SftpRemotePaneSession session, {
    bool withSudo = false,
  }) async {
    final connection = session.connection;
    final auth = connection?.auth;
    if (connection == null || auth == null) {
      return;
    }
    final List<XFile> files;
    try {
      files = await openFiles();
    } on Object catch (error) {
      if (mounted) {
        _showSftpSnack('Unable to choose files: $error');
      }
      return;
    }
    if (!mounted || files.isEmpty) {
      return;
    }
    await _uploadLocalPathsToRemote(
      [for (final file in files) file.path],
      auth,
      session,
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
      title: 'Rename',
      label: 'Name',
      initialValue: entry.name,
      confirmLabel: 'Rename',
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
    final input = await _promptSftpInput(
      title: 'Move to',
      label: 'Destination path',
      initialValue: entry.path,
      confirmLabel: 'Move',
    );
    if (input == null || !mounted) {
      return;
    }
    final targetPath = _normalizeRemoteSftpPath(input, base: session.path);
    if (targetPath == entry.path) {
      return;
    }
    final target = await _prepareRemoteSftpTarget(
      session,
      auth,
      targetPath,
      actionLabel: 'Move',
    );
    if (target == null || !mounted) {
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
    final input = await _promptSftpInput(
      title: 'Copy to',
      label: 'Destination path',
      initialValue: _defaultRemoteCopyPath(session, entry),
      confirmLabel: 'Copy',
    );
    if (input == null || !mounted) {
      return;
    }
    final targetPath = _normalizeRemoteSftpPath(input, base: session.path);
    if (targetPath == entry.path) {
      return;
    }
    final target = await _prepareRemoteSftpTarget(
      session,
      auth,
      targetPath,
      actionLabel: 'Copy',
    );
    if (target == null || !mounted) {
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
    required String title,
    required String label,
    required String initialValue,
    required String confirmLabel,
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

          return _WorkspaceDialogFrame(
            title: Text(title),
            content: _WorkspaceInput(
              controller: controller,
              label: label,
              autofocus: true,
              textInputAction: TextInputAction.done,
              onSubmitted: (_) => submit(),
            ),
            actions: [
              _WorkspaceButton(
                label: 'Cancel',
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
    if (!mounted) {
      return false;
    }
    final singleEntry = entries.length == 1 ? entries.single : null;
    final itemKind = singleEntry == null
        ? '${entries.length} remote items'
        : singleEntry.isDirectory
        ? 'folder'
        : 'file';
    final message = singleEntry == null
        ? 'Delete $itemKind from the SFTP server? This cannot be undone.'
        : 'Delete $itemKind "${singleEntry.name}" from the SFTP server? This cannot be undone.';
    final result = await _showSftpWorkspaceDialog<bool>(
      builder: (_) {
        return _WorkspaceConfirmDialog(
          title: Text(
            tr('sftp.dialog.deleteRemoteItem', fallback: 'Delete remote item?'),
          ),
          message: message,
          confirmLabel: 'Delete',
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
          unawaited(_openLocalFileWithOtherApplication(selected));
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
    final applications = entry.isDirectory || entry.isParent
        ? const <SystemFileApplication>[]
        : await _loadSftpFileApplications(entry.name);
    if (!mounted) return;
    final command = await showNautermContextMenu<_SftpMenuCommand>(
      context: context,
      position: details.globalPosition,
      scaleAnimation: false,
      entries: _sftpFileContextMenuEntries(
        _sftpRowMenuEntries(entry, remote: false),
        fileName: entry.name,
        applications: applications,
        sshEditorAvailable: false,
      ),
    );
    if (command != null) {
      unawaited(
        _dispatchSftpMenuAction(
          () => _handleAction(
            session,
            slot,
            command.action,
            application: command.application,
          ),
        ),
      );
    }
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
          _sftpActionMenuEntries(
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

  void _showRemoteEntryMenu(
    _SftpRemotePaneSession session,
    TapDownDetails details,
    _SftpFileEntry entry,
  ) {
    unawaited(_showRemoteEntryMenuWithApplications(session, details, entry));
  }

  Future<void> _showRemoteEntryMenuWithApplications(
    _SftpRemotePaneSession session,
    TapDownDetails details,
    _SftpFileEntry entry,
  ) async {
    _setSftpState(() {
      if (session.selectedPaths.contains(entry.path)) {
        session.selectedPath = entry.path;
      } else {
        _selectRemoteEntry(session, entry);
      }
    });
    final applications = entry.isDirectory || entry.isParent
        ? const <SystemFileApplication>[]
        : await _loadSftpFileApplications(entry.name);
    if (!mounted) return;
    final command = await showNautermContextMenu<_SftpMenuCommand>(
      context: context,
      position: details.globalPosition,
      scaleAnimation: false,
      entries: _sftpFileContextMenuEntries(
        _sftpRowMenuEntries(
          entry,
          remote: true,
          sshEditorAvailable: _remoteSessionSharesSshEditor(session),
        ),
        fileName: entry.name,
        applications: applications,
        sshEditorAvailable: _remoteSessionSharesSshEditor(session),
      ),
    );
    if (command != null) {
      unawaited(
        _dispatchSftpMenuAction(
          () => _handleRemoteAction(
            session,
            command.action,
            application: command.application,
            withSudo: command.withSudo,
          ),
        ),
      );
    }
  }

  void _showCompactRemoteEntryMenu(
    _SftpRemotePaneSession session,
    TapDownDetails details,
    _SftpFileEntry entry,
    _AiAssistantColors colors,
  ) {
    unawaited(
      _showCompactRemoteEntryMenuWithApplications(
        session,
        details,
        entry,
        colors,
      ),
    );
  }

  Future<void> _showCompactRemoteEntryMenuWithApplications(
    _SftpRemotePaneSession session,
    TapDownDetails details,
    _SftpFileEntry entry,
    _AiAssistantColors colors,
  ) async {
    _setSftpState(() {
      if (session.selectedPaths.contains(entry.path)) {
        session.selectedPath = entry.path;
      } else {
        _selectRemoteEntry(session, entry);
      }
    });
    final applications = entry.isDirectory || entry.isParent
        ? const <SystemFileApplication>[]
        : await _loadSftpFileApplications(entry.name);
    if (!mounted) return;
    final command = await showNautermContextMenu<_SftpMenuCommand>(
      context: context,
      position: details.globalPosition,
      width: 210,
      style: _terminalSftpMenuStyle(colors),
      scaleAnimation: false,
      entries: _sftpFileContextMenuEntries(
        _sftpRowMenuEntries(
          entry,
          remote: true,
          sshEditorAvailable: _remoteSessionSharesSshEditor(session),
        ),
        fileName: entry.name,
        applications: applications,
        sshEditorAvailable: _remoteSessionSharesSshEditor(session),
      ),
    );
    if (command != null) {
      unawaited(
        _dispatchSftpMenuAction(
          () => _handleRemoteAction(
            session,
            command.action,
            application: command.application,
            withSudo: command.withSudo,
          ),
        ),
      );
    }
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
          _sftpActionMenuEntries(
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
                            _fileDropHoverLabel ?? 'Transfer here',
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
      onPathPanelDismiss: () {},
      onPathFavoriteToggle: () {},
      onFavoriteListToggle: () {},
      onFavoritePathSelected: (_) {},
      onSortChanged: (column) => _sortLocalEntriesBy(session, column),
      onAction: (action) => _handleAction(session, slot, action),
      createDragPayload: (entries) => _SftpPaneDragPayload(
        sourceSlot: slot,
        sourceRemote: false,
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
      if (compactColors != null) {
        return _TerminalSftpBrowser(
          colors: compactColors,
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
          onHome: () => _goRemoteHome(session),
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
          },
          onSelectionChanged: (selection) {
            _setSftpState(() => _setRemoteSelection(session, selection));
          },
          onEntryDoubleTap: (entry) => _openRemoteEntry(session, entry),
          onEntrySecondaryTapDown: (details, entry) =>
              _showCompactRemoteEntryMenu(
                session,
                details,
                entry,
                compactColors,
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
          onPanelsDismiss: _dismissSftpPathPanels,
          onPathFavoriteToggle: () =>
              _toggleRemoteSftpFavoritePath(session, connection.host),
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
        tasks: _tasksForSlot(paneSlot),
        taskListOpen:
            _controller.taskListOpen && _controller.taskListSlot == paneSlot,
        favoriteListOpen:
            _controller.favoriteListOpen &&
            _controller.favoriteListSlot == paneSlot,
        favoritePaths: session.favoritePaths,
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
        },
        onSelectionChanged: (selection) {
          _setSftpState(() {
            _setRemoteSelection(session, selection);
          });
        },
        onEntryDoubleTap: (entry) => _openRemoteEntry(session, entry),
        onEntrySecondaryTapDown: (details, entry) =>
            _showRemoteEntryMenu(session, details, entry),
        onBlankSecondaryTapDown: (details) =>
            _showRemoteBlankMenu(session, details),
        onTaskListToggle: () => _toggleSftpTaskList(paneSlot),
        onTaskClearCompleted: _clearCompletedSftpTasks,
        onTaskDismiss: _dismissSftpTask,
        onTaskCancel: _cancelSftpTask,
        onTaskPauseToggle: _toggleSftpTaskPause,
        onPathPanelDismiss: _dismissSftpPathPanels,
        onPathFavoriteToggle: () =>
            _toggleRemoteSftpFavoritePath(session, connection.host),
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
          sourceSlot: paneSlot,
          sourceRemote: true,
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
