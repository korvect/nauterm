part of 'nauterm_workspace.dart';

class _SftpPanelSurface extends StatelessWidget {
  const _SftpPanelSurface({required this.height, required this.child});

  final double height;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: _workspaceMenuBackground,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(color: _workspaceMenuBorder),
          boxShadow: _workspaceMenuShadows,
        ),
        child: SizedBox(height: height, child: child),
      ),
    );
  }
}

class _SftpTaskListButton extends StatelessWidget {
  const _SftpTaskListButton({
    required this.tasks,
    required this.open,
    required this.onTap,
  });

  final List<_SftpTask> tasks;
  final bool open;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final activeCount = tasks
        .where(
          (task) =>
              task.status == _SftpTaskStatus.running ||
              task.status == _SftpTaskStatus.queued ||
              task.status == _SftpTaskStatus.paused,
        )
        .length;

    return Tooltip(
      message: tr(
        open ? 'sftp.label.hideTasks' : 'sftp.label.showTasks',
        fallback: open ? 'Hide tasks' : 'Show tasks',
      ),
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: open
                  ? _blue.withValues(alpha: _workspaceDark ? 0.18 : 0.10)
                  : _sidebarHover,
              borderRadius: BorderRadius.circular(7),
              border: Border.all(
                color: open ? _blue.withValues(alpha: 0.42) : _sidebarDivider,
              ),
            ),
            child: SizedBox(
              width: 30,
              height: 30,
              child: Stack(
                clipBehavior: Clip.none,
                children: [
                  Center(
                    child: Icon(
                      Icons.format_list_bulleted_rounded,
                      size: 18,
                      color: open ? _blue : _text,
                    ),
                  ),
                  if (activeCount > 0)
                    Positioned(
                      right: -4,
                      top: -4,
                      child: DecoratedBox(
                        decoration: BoxDecoration(
                          color: _blue,
                          borderRadius: BorderRadius.circular(999),
                          border: Border.all(
                            color: _workspaceMenuBackground,
                            width: 1.5,
                          ),
                        ),
                        child: ConstrainedBox(
                          constraints: const BoxConstraints(
                            minWidth: 17,
                            minHeight: 17,
                          ),
                          child: Padding(
                            padding: const EdgeInsets.symmetric(horizontal: 4),
                            child: Center(
                              child: Text(
                                activeCount > 99 ? '99+' : '$activeCount',
                                style: TextStyle(
                                  color: Colors.white,
                                  fontSize: 9,
                                  fontWeight: NautermFontWeights.semibold,
                                  letterSpacing: 0,
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
          ),
        ),
      ),
    );
  }
}

class _SftpTaskBar extends StatelessWidget {
  const _SftpTaskBar({
    required this.tasks,
    required this.onClearCompleted,
    required this.onDismissTask,
    required this.onCancelTask,
    required this.onPauseToggle,
  });

  final List<_SftpTask> tasks;
  final VoidCallback onClearCompleted;
  final ValueChanged<int> onDismissTask;
  final ValueChanged<int> onCancelTask;
  final ValueChanged<int> onPauseToggle;

  @override
  Widget build(BuildContext context) {
    final orderedTasks = _orderedSftpTasks(tasks);
    final queuePositions = _sftpTaskQueuePositions(tasks);
    final hasFinished = tasks.any(_isFinishedSftpTask);

    return _SftpPanelSurface(
      height: 246,
      child: Column(
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(13, 13, 12, 7),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    tr('common.label.tasks', fallback: 'Tasks'),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: _text,
                      fontSize: NautermFontSizes.labelLarge,
                      fontWeight: NautermFontWeights.semibold,
                      letterSpacing: 0,
                    ),
                  ),
                ),
                _SftpTaskTextAction(
                  label: tr('sftp.label.clearTasks', fallback: 'Clear Tasks'),
                  onTap: hasFinished ? onClearCompleted : null,
                ),
              ],
            ),
          ),
          Expanded(
            child: orderedTasks.isEmpty
                ? const _SftpTaskEmptyState()
                : ClipRRect(
                    borderRadius: const BorderRadius.vertical(
                      bottom: Radius.circular(8),
                    ),
                    child: ListView.separated(
                      padding: EdgeInsets.zero,
                      itemCount: orderedTasks.length,
                      separatorBuilder: (_, _) => Divider(
                        height: 1,
                        thickness: 1,
                        indent: 13,
                        endIndent: 13,
                        color: _workspaceMenuBorder,
                      ),
                      itemBuilder: (context, index) {
                        final task = orderedTasks[index];
                        return _SftpTaskRow(
                          task: task,
                          queuePosition: queuePositions[task.id],
                          onDismiss: () => onDismissTask(task.id),
                          onCancel: () => onCancelTask(task.id),
                          onPauseToggle: () => onPauseToggle(task.id),
                        );
                      },
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}

class _SftpTaskEmptyState extends StatelessWidget {
  const _SftpTaskEmptyState();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Text(
        tr('common.label.noTasks', fallback: 'No tasks'),
        style: TextStyle(
          color: _mutedText,
          fontSize: NautermFontSizes.labelMedium,
          fontWeight: NautermFontWeights.medium,
          letterSpacing: 0,
        ),
      ),
    );
  }
}

class _SftpTaskRow extends StatelessWidget {
  const _SftpTaskRow({
    required this.task,
    required this.onDismiss,
    required this.onCancel,
    required this.onPauseToggle,
    this.queuePosition,
  });

  final _SftpTask task;
  final VoidCallback onDismiss;
  final VoidCallback onCancel;
  final VoidCallback onPauseToggle;
  final int? queuePosition;

  @override
  Widget build(BuildContext context) {
    final cancellable =
        task.status == _SftpTaskStatus.queued ||
        task.status == _SftpTaskStatus.running ||
        task.status == _SftpTaskStatus.paused;
    final accent = _sftpTaskStatusColor(task.status);
    final progressValue = task.totalBytes > 0
        ? (task.bytes / task.totalBytes).clamp(0.0, 1.0).toDouble()
        : null;
    final pathUnavailable = _sftpTaskLocalPathUnavailable(task);

    return SizedBox(
      height: 45,
      child: Stack(
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 5),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Padding(
                  padding: const EdgeInsets.only(left: 19, right: 67),
                  child: Text.rich(
                    TextSpan(
                      children: [
                        TextSpan(
                          text: _sftpTaskActionLabel(task.type),
                          style: TextStyle(
                            color: _text,
                            fontSize: NautermFontSizes.labelSmall,
                            fontWeight: NautermFontWeights.semibold,
                            letterSpacing: 0,
                          ),
                        ),
                        TextSpan(
                          text: ' ${_sftpTaskPrimaryPath(task)}',
                          style: TextStyle(
                            color: _text,
                            fontSize: NautermFontSizes.labelSmall,
                            fontWeight: NautermFontWeights.medium,
                            letterSpacing: 0,
                            decoration:
                                task.status == _SftpTaskStatus.cancelled ||
                                    pathUnavailable
                                ? TextDecoration.lineThrough
                                : TextDecoration.none,
                            decorationColor: const Color(0xff6d7f86),
                          ),
                        ),
                      ],
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                SizedBox(height: 2),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 19),
                  child: Text(
                    _sftpTaskSubtitle(task, queuePosition: queuePosition),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: task.status == _SftpTaskStatus.failed
                          ? const Color(0xffd54b3f)
                          : _mutedText,
                      fontSize: NautermFontSizes.labelSmall,
                      fontWeight: NautermFontWeights.medium,
                      letterSpacing: 0,
                    ),
                  ),
                ),
              ],
            ),
          ),
          if (task.status == _SftpTaskStatus.running ||
              task.status == _SftpTaskStatus.paused)
            Positioned(
              left: 19,
              right: 19,
              bottom: 0,
              child: ClipRRect(
                borderRadius: BorderRadius.circular(999),
                child: LinearProgressIndicator(
                  minHeight: 2,
                  value: progressValue,
                  backgroundColor: _sidebarHover,
                  valueColor: AlwaysStoppedAnimation<Color>(accent),
                ),
              ),
            ),
          if (_isPausableSftpTask(task) &&
              (task.status == _SftpTaskStatus.queued ||
                  task.status == _SftpTaskStatus.running ||
                  task.status == _SftpTaskStatus.paused))
            Positioned(
              right: 35,
              top: 8,
              child: Tooltip(
                message: task.status == _SftpTaskStatus.paused
                    ? tr('common.action.continue', fallback: 'Continue')
                    : tr('common.action.pause', fallback: 'Pause'),
                child: InkResponse(
                  radius: 15,
                  onTap:
                      task.pauseRequested ||
                          task.cancelRequested ||
                          (task.status != _SftpTaskStatus.queued &&
                              task.status != _SftpTaskStatus.running &&
                              task.status != _SftpTaskStatus.paused)
                      ? null
                      : onPauseToggle,
                  child: Icon(
                    task.status == _SftpTaskStatus.paused
                        ? LucideIcons.play
                        : LucideIcons.circlePause,
                    size: 13,
                    color: task.pauseRequested || task.cancelRequested
                        ? const Color(0xffb7c6cc)
                        : const Color(0xff182433),
                  ),
                ),
              ),
            ),
          Positioned(
            right: 11,
            top: 8,
            child: Tooltip(
              message: tr(
                cancellable
                    ? 'sftp.label.cancelTask'
                    : 'sftp.label.deleteRecord',
                fallback: cancellable ? 'Cancel task' : 'Delete record',
              ),
              child: InkResponse(
                radius: 15,
                onTap: cancellable && task.cancelRequested
                    ? null
                    : cancellable
                    ? onCancel
                    : onDismiss,
                child: Icon(
                  Icons.close_rounded,
                  size: 14,
                  color: cancellable && task.cancelRequested
                      ? const Color(0xffb7c6cc)
                      : const Color(0xff182433),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _SftpTaskTextAction extends StatelessWidget {
  const _SftpTaskTextAction({required this.label, required this.onTap});

  final String label;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: InkWell(
        borderRadius: BorderRadius.circular(5),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 3),
          child: Text(
            label,
            style: TextStyle(
              color: onTap == null ? const Color(0xffb4c3c8) : _text,
              fontSize: NautermFontSizes.labelSmall,
              fontWeight: NautermFontWeights.semibold,
              letterSpacing: 0,
            ),
          ),
        ),
      ),
    );
  }
}

List<_SftpTask> _orderedSftpTasks(List<_SftpTask> tasks) {
  final running = <_SftpTask>[];
  final queued = <_SftpTask>[];
  final finished = <_SftpTask>[];
  for (final task in tasks) {
    switch (task.status) {
      case _SftpTaskStatus.running:
        running.add(task);
      case _SftpTaskStatus.queued:
        queued.add(task);
      case _SftpTaskStatus.paused:
        queued.add(task);
      case _SftpTaskStatus.completed:
      case _SftpTaskStatus.failed:
      case _SftpTaskStatus.cancelled:
        finished.add(task);
    }
  }
  running.sort(_compareSftpTasksOldestFirst);
  queued.sort(_compareSftpTasksOldestFirst);
  finished.sort(_compareFinishedSftpTasksNewestFirst);
  return [...running, ...queued, ...finished];
}

Map<int, int> _sftpTaskQueuePositions(List<_SftpTask> tasks) {
  var position = 1;
  final positions = <int, int>{};
  final queued =
      tasks
          .where((task) => task.status == _SftpTaskStatus.queued)
          .toList(growable: false)
        ..sort(_compareSftpTasksOldestFirst);
  for (final task in queued) {
    positions[task.id] = position;
    position += 1;
  }
  return positions;
}

int _compareSftpTasksOldestFirst(_SftpTask left, _SftpTask right) {
  final created = left.createdAt.compareTo(right.createdAt);
  if (created != 0) return created;
  return _sftpTaskStableSequence(
    left,
  ).compareTo(_sftpTaskStableSequence(right));
}

int _compareFinishedSftpTasksNewestFirst(_SftpTask left, _SftpTask right) {
  final leftFinished = left.finishedAt ?? left.createdAt;
  final rightFinished = right.finishedAt ?? right.createdAt;
  final finished = rightFinished.compareTo(leftFinished);
  if (finished != 0) return finished;
  final created = right.createdAt.compareTo(left.createdAt);
  if (created != 0) return created;
  return _sftpTaskStableSequence(
    right,
  ).compareTo(_sftpTaskStableSequence(left));
}

int _sftpTaskStableSequence(_SftpTask task) => task.historyId ?? task.id;

bool _isFinishedSftpTask(_SftpTask task) {
  return task.status == _SftpTaskStatus.completed ||
      task.status == _SftpTaskStatus.failed ||
      task.status == _SftpTaskStatus.cancelled;
}

Color _sftpTaskStatusColor(_SftpTaskStatus status) {
  return switch (status) {
    _SftpTaskStatus.queued => const Color(0xff7a8f98),
    _SftpTaskStatus.running => _blue,
    _SftpTaskStatus.paused => const Color(0xffd18b22),
    _SftpTaskStatus.completed => const Color(0xff22a861),
    _SftpTaskStatus.failed => const Color(0xffd54b3f),
    _SftpTaskStatus.cancelled => const Color(0xff7a8f98),
  };
}

String _sftpTaskActionLabel(_SftpTaskType type) {
  return switch (type) {
    _SftpTaskType.download || _SftpTaskType.transferDownload => tr(
      'sftp.task.download',
      fallback: 'Download',
    ),
    _SftpTaskType.upload ||
    _SftpTaskType.transferUpload => tr('sftp.task.upload', fallback: 'Upload'),
    _SftpTaskType.edit => tr('sftp.task.edit', fallback: 'Edit'),
    _SftpTaskType.move => tr('sftp.task.move', fallback: 'Move'),
    _SftpTaskType.copy => tr('sftp.task.copy', fallback: 'Copy'),
    _SftpTaskType.newFolder => tr(
      'sftp.action.newFolder',
      fallback: 'New Folder',
    ),
    _SftpTaskType.delete => tr('sftp.task.delete', fallback: 'Delete'),
  };
}

String _sftpTaskPrimaryPath(_SftpTask task) {
  final path = switch (task.type) {
    _SftpTaskType.download => task.targetPath,
    _SftpTaskType.upload => task.sourcePath,
    _SftpTaskType.transferDownload => task.sourcePath,
    _SftpTaskType.transferUpload => task.targetPath,
    _SftpTaskType.edit => task.targetPath,
    _SftpTaskType.move => task.targetPath,
    _SftpTaskType.copy => task.targetPath,
    _SftpTaskType.newFolder => task.targetPath,
    _SftpTaskType.delete => task.sourcePath,
  };
  if (path.trim().isNotEmpty) {
    return path;
  }
  return task.displayName;
}

bool _sftpTaskLocalPathUnavailable(_SftpTask task) {
  if (task.status != _SftpTaskStatus.completed) return false;
  final path = switch (task.type) {
    _SftpTaskType.download => task.targetPath,
    _SftpTaskType.upload => task.sourcePath,
    _SftpTaskType.transferDownload || _SftpTaskType.transferUpload => null,
    _ => null,
  };
  if (path == null || path.trim().isEmpty) return false;
  try {
    return io.FileSystemEntity.typeSync(path, followLinks: false) ==
        io.FileSystemEntityType.notFound;
  } on Object {
    return true;
  }
}
