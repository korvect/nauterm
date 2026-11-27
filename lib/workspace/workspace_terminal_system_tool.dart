part of 'nauterm_workspace.dart';

@visibleForTesting
Widget buildTerminalSystemInfoPanelForTesting(
  Future<FfiHostSystemInfoResult> Function() load,
) => Column(
  children: [
    _TerminalSystemInfoPanel(
      colors: _AiAssistantColors.fromTerminalTheme(defaultTerminalTheme),
      load: load,
      target: 'example.test',
      source: 'example.test',
      sessionConnected: true,
    ),
  ],
);

class _TerminalSystemInfoPanel extends StatefulWidget {
  const _TerminalSystemInfoPanel({
    required this.colors,
    required this.load,
    required this.target,
    required this.source,
    required this.sessionConnected,
  });

  final _AiAssistantColors colors;
  final Future<FfiHostSystemInfoResult> Function()? load;
  final String? target;
  final Object? source;
  final bool sessionConnected;

  @override
  State<_TerminalSystemInfoPanel> createState() =>
      _TerminalSystemInfoPanelState();
}

class _TerminalSystemInfoPanelState extends State<_TerminalSystemInfoPanel>
    with WidgetsBindingObserver {
  FfiHostSystemInfoResult? _info;
  bool _loading = false;
  int _requestId = 0;
  Timer? _pollTimer;
  bool _polling = false;
  AppLifecycleState? _lifecycle;
  DateTime? _sampledAt;
  String? _selectedNetworkInterface;
  double _downloadRate = 0;
  double _uploadRate = 0;
  final List<double> _downloadHistory = [];
  final List<double> _uploadHistory = [];
  final List<double> _latencyHistory = [];

  @override
  void initState() {
    super.initState();
    final binding = WidgetsBinding.instance;
    _lifecycle = binding.lifecycleState;
    binding.addObserver(this);
    _syncPolling();
  }

  bool get _hasSession => widget.load != null && widget.sessionConnected;

  void _syncPolling({bool reset = false}) {
    final active =
        _hasSession &&
        (_lifecycle == null || _lifecycle == AppLifecycleState.resumed);
    if (!reset && _polling == active) return;
    _polling = active;
    _requestId += 1;
    _pollTimer?.cancel();
    _pollTimer = null;
    _loading = false;
    _sampledAt = null;
    _downloadRate = 0;
    _uploadRate = 0;
    if (reset) {
      _info = null;
      _selectedNetworkInterface = null;
      _downloadHistory.clear();
      _uploadHistory.clear();
      _latencyHistory.clear();
    }
    if (active) {
      unawaited(_refresh());
      _pollTimer = Timer.periodic(
        const Duration(seconds: 5),
        (_) => unawaited(_refresh()),
      );
    }
  }

  @override
  void didUpdateWidget(covariant _TerminalSystemInfoPanel oldWidget) {
    super.didUpdateWidget(oldWidget);
    _syncPolling(
      reset:
          oldWidget.source != widget.source ||
          oldWidget.target != widget.target,
    );
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    setState(() => _lifecycle = state);
    _syncPolling();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _pollTimer?.cancel();
    super.dispose();
  }

  Future<void> _refresh() async {
    final load = widget.load;
    if (load == null || _loading || !_polling) {
      return;
    }
    final requestId = ++_requestId;
    setState(() => _loading = true);
    FfiHostSystemInfoResult result;
    try {
      result = await load();
    } on Object catch (error) {
      result = FfiHostSystemInfoResult(error: '$error');
    }
    if (!mounted || requestId != _requestId) {
      return;
    }
    final now = DateTime.now();
    final elapsed = _sampledAt == null
        ? null
        : now.difference(_sampledAt!).inMilliseconds / 1000;
    final selectedInterface = _resolveSelectedInterface(result);
    final currentNetwork = result.networkInterfaces
        .where((item) => item.name == selectedInterface)
        .firstOrNull;
    final previousNetwork = _info?.networkInterfaces
        .where((item) => item.name == selectedInterface)
        .firstOrNull;
    final downloadRate =
        elapsed != null &&
            elapsed > 0 &&
            currentNetwork != null &&
            previousNetwork != null
        ? math.max(
                0,
                currentNetwork.receivedBytes - previousNetwork.receivedBytes,
              ) /
              elapsed
        : 0.0;
    final uploadRate =
        elapsed != null &&
            elapsed > 0 &&
            currentNetwork != null &&
            previousNetwork != null
        ? math.max(
                0,
                currentNetwork.transmittedBytes -
                    previousNetwork.transmittedBytes,
              ) /
              elapsed
        : 0.0;
    setState(() {
      _info = result;
      _loading = false;
      _sampledAt = now;
      _selectedNetworkInterface = selectedInterface;
      _downloadRate = downloadRate;
      _uploadRate = uploadRate;
      _appendSystemSample(_downloadHistory, downloadRate);
      _appendSystemSample(_uploadHistory, uploadRate);
      if (result.latencyMs case final latency?) {
        _appendSystemSample(_latencyHistory, latency);
      }
    });
  }

  String? _resolveSelectedInterface(FfiHostSystemInfoResult info) {
    final selected = _selectedNetworkInterface;
    if (selected != null &&
        info.networkInterfaces.any((item) => item.name == selected)) {
      return selected;
    }
    return info.networkInterfaces.firstOrNull?.name;
  }

  void _selectNetworkInterface(String value) {
    setState(() {
      _selectedNetworkInterface = value;
      _downloadRate = 0;
      _uploadRate = 0;
      _downloadHistory.clear();
      _uploadHistory.clear();
      _sampledAt = null;
    });
  }

  @override
  Widget build(BuildContext context) {
    final colors = widget.colors;
    final info = _info;
    final hostname = info?.hostname;
    final target = widget.target;
    return Expanded(
      key: const ValueKey('terminal-system-info-panel'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _TerminalToolSectionHeader(
            title: 'workspace.label.systemInformation',
            subtitle: [
              ?hostname,
              if (target != null && target != hostname) target,
            ].join(' · '),
            colors: colors,
            trailing: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (widget.target != null)
                  IconButton(
                    tooltip: tr(
                      'workspace.label.copyHost',
                      fallback: 'Copy host',
                    ),
                    onPressed: () => unawaited(
                      Clipboard.setData(ClipboardData(text: widget.target!)),
                    ),
                    icon: const Icon(LucideIcons.copy, size: 14),
                    color: colors.muted,
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints.tightFor(
                      width: 28,
                      height: 28,
                    ),
                  ),
                IconButton(
                  key: const ValueKey('terminal-system-info-refresh'),
                  tooltip: tr(
                    'workspace.label.refreshSystemInformation',
                    fallback: 'Refresh system information',
                  ),
                  onPressed: !_polling || _loading ? null : _refresh,
                  icon: _loading
                      ? SizedBox.square(
                          dimension: 14,
                          child: CircularProgressIndicator(
                            strokeWidth: 1.5,
                            color: colors.accent,
                          ),
                        )
                      : const Icon(LucideIcons.refreshCw, size: 17),
                  color: colors.muted,
                  disabledColor: colors.muted.withValues(alpha: 0.35),
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints.tightFor(
                    width: 30,
                    height: 30,
                  ),
                  style: IconButton.styleFrom(
                    hoverColor: colors.inputBackground,
                    highlightColor: colors.accent.withValues(alpha: 0.12),
                  ),
                ),
              ],
            ),
          ),
          Divider(height: 1, color: colors.border),
          if (info != null && !_polling)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
              child: Text(
                _hasSession
                    ? context.tr(
                        'workspace.systemInfo.updatesPaused',
                        fallback: 'Updates paused',
                      )
                    : context.tr(
                        'workspace.label.noSshSession',
                        fallback: 'No SSH session',
                      ),
                style: TextStyle(color: colors.muted, fontSize: 11),
              ),
            ),
          Expanded(child: _buildBody(info)),
        ],
      ),
    );
  }

  Widget _buildBody(FfiHostSystemInfoResult? info) {
    final colors = widget.colors;
    if (!_hasSession && info == null) {
      return _TerminalToolEmptyState(
        icon: LucideIcons.activity,
        title: 'workspace.label.noSshSession',
        description: context.tr('workspace.systemInfo.connectFirst'),
        colors: colors,
      );
    }
    if (info == null) {
      if (!_polling) {
        return Center(
          child: Text(
            context.tr(
              'workspace.systemInfo.updatesPaused',
              fallback: 'Updates paused',
            ),
            style: TextStyle(color: colors.muted, fontSize: 11),
          ),
        );
      }
      return Center(
        child: CircularProgressIndicator(
          strokeWidth: 1.8,
          color: colors.accent,
        ),
      );
    }
    if (!info.hasData) {
      return _TerminalToolEmptyState(
        icon: LucideIcons.circleAlert,
        title: 'workspace.label.unableToLoadSystemInformation',
        description: info.error ?? context.tr('workspace.systemInfo.noData'),
        colors: colors,
        actionLabel: _polling ? context.tr('common.action.retry') : null,
        onAction: _polling ? _refresh : null,
      );
    }

    final memoryPercent = _usagePercent(
      info.memoryUsedBytes,
      info.memoryTotalBytes,
    );
    final swapPercent = _usagePercent(info.swapUsedBytes, info.swapTotalBytes);
    return ClipRect(
      child: ListView(
        padding: const EdgeInsets.fromLTRB(12, 12, 12, 18),
        children: [
          _TerminalSystemMetric(
            label: 'workspace.label.cpu',
            value: info.cpuUsagePercent == null
                ? '—'
                : '${info.cpuUsagePercent!.toStringAsFixed(1)}%',
            detail: [
              if (info.cpuCount case final count?)
                context.tr(
                  'terminal.systemInfo.cpu.cores',
                  args: {'count': count},
                ),
              if (info.loadAverage case final load?)
                context.tr(
                  'terminal.systemInfo.cpu.load',
                  args: {'load': load.toStringAsFixed(2)},
                ),
            ].join(' · '),
            progress: info.cpuUsagePercent == null
                ? null
                : info.cpuUsagePercent! / 100,
            colors: colors,
          ),
          const SizedBox(height: 8),
          _TerminalSystemMetric(
            label: 'common.label.memory',
            value: memoryPercent == null
                ? '—'
                : '${(memoryPercent * 100).toStringAsFixed(0)}%',
            detail: _usedOfTotal(info.memoryUsedBytes, info.memoryTotalBytes),
            progress: memoryPercent,
            colors: colors,
          ),
          const SizedBox(height: 8),
          _TerminalSystemMetric(
            label: 'workspace.label.swap',
            value: swapPercent == null
                ? '—'
                : '${(swapPercent * 100).toStringAsFixed(0)}%',
            detail: _usedOfTotal(info.swapUsedBytes, info.swapTotalBytes),
            progress: swapPercent,
            colors: colors,
          ),
          const SizedBox(height: 14),
          _TerminalSystemDetails(
            colors: colors,
            rows: [
              if (info.osName != null)
                ('workspace.systemInfo.operatingSystem', info.osName!),
              if (info.kernel != null)
                ('workspace.systemInfo.kernel', info.kernel!),
              if (info.architecture != null)
                ('workspace.systemInfo.architecture', info.architecture!),
              if (info.uptimeSeconds != null)
                (
                  'workspace.systemInfo.uptime',
                  _formatUptime(info.uptimeSeconds!),
                ),
              if (info.loadAverage != null)
                (
                  'workspace.systemInfo.loadAverage',
                  [info.loadAverage, info.loadAverage5, info.loadAverage15]
                      .whereType<double>()
                      .map((v) => v.toStringAsFixed(2))
                      .join('  '),
                ),
            ],
          ),
          if (info.processes.isNotEmpty) ...[
            const SizedBox(height: 14),
            _TerminalSystemProcesses(processes: info.processes, colors: colors),
          ],
          if (info.networkInterfaces.isNotEmpty) ...[
            const SizedBox(height: 14),
            _TerminalSystemNetwork(
              interfaces: info.networkInterfaces,
              selectedInterface:
                  _selectedNetworkInterface ??
                  info.networkInterfaces.first.name,
              downloadRate: _downloadRate,
              uploadRate: _uploadRate,
              downloadHistory: _downloadHistory,
              uploadHistory: _uploadHistory,
              colors: colors,
              onInterfaceChanged: _selectNetworkInterface,
            ),
          ],
          if (info.latencyMs != null) ...[
            const SizedBox(height: 14),
            _TerminalSystemLatency(
              latency: info.latencyMs!,
              history: _latencyHistory,
              colors: colors,
            ),
          ],
          if (info.filesystems.isNotEmpty) ...[
            const SizedBox(height: 14),
            _TerminalSystemFilesystems(
              filesystems: info.filesystems,
              colors: colors,
            ),
          ],
          if (info.error != null) ...[
            const SizedBox(height: 10),
            Text(
              info.error!,
              style: TextStyle(
                color: colors.muted,
                fontSize: 10.5,
                height: 1.4,
                letterSpacing: 0,
              ),
            ),
          ],
        ],
      ),
    );
  }
}

void _appendSystemSample(List<double> samples, double value) {
  samples.add(value);
  if (samples.length > 30) {
    samples.removeAt(0);
  }
}

enum _TerminalSystemProcessSort { memory, cpu, command }

class _TerminalSystemProcesses extends StatefulWidget {
  const _TerminalSystemProcesses({
    required this.processes,
    required this.colors,
  });

  final List<FfiHostProcessInfo> processes;
  final _AiAssistantColors colors;

  @override
  State<_TerminalSystemProcesses> createState() =>
      _TerminalSystemProcessesState();
}

class _TerminalSystemProcessesState extends State<_TerminalSystemProcesses> {
  _TerminalSystemProcessSort _sort = _TerminalSystemProcessSort.memory;
  bool _ascending = false;

  void _selectSort(_TerminalSystemProcessSort sort) {
    setState(() {
      if (_sort == sort) {
        _ascending = !_ascending;
        return;
      }
      _sort = sort;
      _ascending = sort == _TerminalSystemProcessSort.command;
    });
  }

  List<FfiHostProcessInfo> get _sortedProcesses {
    final sorted = widget.processes.toList();
    sorted.sort((left, right) {
      final result = switch (_sort) {
        _TerminalSystemProcessSort.memory => left.memoryBytes.compareTo(
          right.memoryBytes,
        ),
        _TerminalSystemProcessSort.cpu => left.cpuUsagePercent.compareTo(
          right.cpuUsagePercent,
        ),
        _TerminalSystemProcessSort.command =>
          left.command.toLowerCase().compareTo(right.command.toLowerCase()),
      };
      final directed = _ascending ? result : -result;
      if (directed != 0) {
        return directed;
      }
      return left.command.toLowerCase().compareTo(right.command.toLowerCase());
    });
    return sorted;
  }

  @override
  Widget build(BuildContext context) {
    return _TerminalSystemSection(
      title: 'workspace.label.topProcesses',
      colors: widget.colors,
      child: Column(
        children: [
          _TerminalSystemProcessHeader(
            sort: _sort,
            ascending: _ascending,
            colors: widget.colors,
            onSortSelected: _selectSort,
          ),
          for (final process in _sortedProcesses)
            _TerminalSystemTableRow(
              cells: [
                _formatBytes(process.memoryBytes),
                '${process.cpuUsagePercent.toStringAsFixed(1)}%',
                process.command,
              ],
              flexes: const [3, 2, 5],
              colors: widget.colors,
            ),
        ],
      ),
    );
  }
}

class _TerminalSystemProcessHeader extends StatelessWidget {
  const _TerminalSystemProcessHeader({
    required this.sort,
    required this.ascending,
    required this.colors,
    required this.onSortSelected,
  });

  final _TerminalSystemProcessSort sort;
  final bool ascending;
  final _AiAssistantColors colors;
  final ValueChanged<_TerminalSystemProcessSort> onSortSelected;

  @override
  Widget build(BuildContext context) {
    return Container(
      color: colors.inputBackground,
      child: Row(
        children: [
          _TerminalSystemProcessHeaderCell(
            label: 'common.label.memory',
            sort: _TerminalSystemProcessSort.memory,
            flex: 3,
            active: sort == _TerminalSystemProcessSort.memory,
            ascending: ascending,
            colors: colors,
            onPressed: onSortSelected,
            contentPadding: const EdgeInsets.only(left: 9, right: 4),
          ),
          _TerminalSystemProcessHeaderCell(
            label: 'CPU',
            sort: _TerminalSystemProcessSort.cpu,
            flex: 2,
            active: sort == _TerminalSystemProcessSort.cpu,
            ascending: ascending,
            colors: colors,
            onPressed: onSortSelected,
            contentPadding: const EdgeInsets.symmetric(horizontal: 4),
          ),
          _TerminalSystemProcessHeaderCell(
            label: 'common.label.command',
            sort: _TerminalSystemProcessSort.command,
            flex: 5,
            active: sort == _TerminalSystemProcessSort.command,
            ascending: ascending,
            colors: colors,
            onPressed: onSortSelected,
            contentPadding: const EdgeInsets.only(left: 4, right: 9),
          ),
        ],
      ),
    );
  }
}

class _TerminalSystemProcessHeaderCell extends StatelessWidget {
  const _TerminalSystemProcessHeaderCell({
    required this.label,
    required this.sort,
    required this.flex,
    required this.active,
    required this.ascending,
    required this.colors,
    required this.onPressed,
    required this.contentPadding,
  });

  final String label;
  final _TerminalSystemProcessSort sort;
  final int flex;
  final bool active;
  final bool ascending;
  final _AiAssistantColors colors;
  final ValueChanged<_TerminalSystemProcessSort> onPressed;
  final EdgeInsets contentPadding;

  @override
  Widget build(BuildContext context) {
    return Expanded(
      flex: flex,
      child: Material(
        color: Colors.transparent,
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          key: ValueKey('terminal-system-process-sort:${sort.name}'),
          onTap: () => onPressed(sort),
          mouseCursor: SystemMouseCursors.click,
          hoverColor: colors.foreground.withValues(alpha: 0.05),
          splashColor: colors.accent.withValues(alpha: 0.14),
          highlightColor: colors.foreground.withValues(alpha: 0.04),
          child: SizedBox(
            height: 29,
            child: Padding(
              padding: contentPadding,
              child: Row(
                children: [
                  Flexible(
                    child: Text(
                      tr(label),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: active ? colors.foreground : colors.muted,
                        fontSize: 9.5,
                        fontWeight: NautermFontWeights.semibold,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                  const SizedBox(width: 2),
                  SizedBox(
                    width: 11,
                    child: active
                        ? Icon(
                            ascending
                                ? LucideIcons.chevronUp
                                : LucideIcons.chevronDown,
                            size: 11,
                            color: colors.accent,
                          )
                        : null,
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

class _TerminalSystemNetwork extends StatelessWidget {
  const _TerminalSystemNetwork({
    required this.interfaces,
    required this.selectedInterface,
    required this.downloadRate,
    required this.uploadRate,
    required this.downloadHistory,
    required this.uploadHistory,
    required this.colors,
    required this.onInterfaceChanged,
  });

  final List<FfiHostNetworkInterface> interfaces;
  final String selectedInterface;
  final double downloadRate;
  final double uploadRate;
  final List<double> downloadHistory;
  final List<double> uploadHistory;
  final _AiAssistantColors colors;
  final ValueChanged<String> onInterfaceChanged;

  @override
  Widget build(BuildContext context) {
    return _TerminalSystemSection(
      title: 'common.label.network',
      colors: colors,
      trailing: SizedBox(
        width: 104,
        child: _TerminalToolDropdown<String>(
          value: selectedInterface,
          options: [
            for (final interface in interfaces)
              NautermContextMenuAction(
                value: interface.name,
                label: interface.name,
              ),
          ],
          colors: colors,
          fontSize: 10.5,
          fontWeight: NautermFontWeights.medium,
          onChanged: onInterfaceChanged,
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(10, 8, 10, 10),
        child: Column(
          children: [
            Row(
              children: [
                _TerminalSystemRate(
                  icon: LucideIcons.arrowDown,
                  label: _formatRate(downloadRate),
                  color: colors.accent,
                ),
                const SizedBox(width: 12),
                _TerminalSystemRate(
                  icon: LucideIcons.arrowUp,
                  label: _formatRate(uploadRate),
                  color: colors.muted,
                ),
              ],
            ),
            const SizedBox(height: 8),
            SizedBox(
              height: 58,
              child: CustomPaint(
                painter: _TerminalSystemTrendPainter(
                  primary: downloadHistory,
                  secondary: uploadHistory,
                  primaryColor: colors.accent,
                  secondaryColor: colors.muted,
                  gridColor: colors.border,
                ),
                size: Size.infinite,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _TerminalSystemLatency extends StatelessWidget {
  const _TerminalSystemLatency({
    required this.latency,
    required this.history,
    required this.colors,
  });

  final double latency;
  final List<double> history;
  final _AiAssistantColors colors;

  @override
  Widget build(BuildContext context) {
    return _TerminalSystemSection(
      title: 'Latency',
      colors: colors,
      trailing: Text(
        tr('${latency.toStringAsFixed(1)} ms'),
        style: TextStyle(
          color: colors.accent,
          fontSize: 10.5,
          fontWeight: NautermFontWeights.semibold,
          letterSpacing: 0,
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(10, 7, 10, 10),
        child: SizedBox(
          height: 48,
          child: CustomPaint(
            painter: _TerminalSystemTrendPainter(
              primary: history,
              primaryColor: colors.accent,
              gridColor: colors.border,
            ),
            size: Size.infinite,
          ),
        ),
      ),
    );
  }
}

class _TerminalSystemFilesystems extends StatelessWidget {
  const _TerminalSystemFilesystems({
    required this.filesystems,
    required this.colors,
  });

  final List<FfiHostFilesystemInfo> filesystems;
  final _AiAssistantColors colors;

  @override
  Widget build(BuildContext context) {
    return _TerminalSystemSection(
      title: 'workspace.label.filesystems',
      colors: colors,
      child: Column(
        children: [
          for (final filesystem in filesystems.take(12))
            Padding(
              padding: const EdgeInsets.fromLTRB(10, 7, 10, 7),
              child: Column(
                children: [
                  Row(
                    children: [
                      Expanded(
                        child: Text(
                          filesystem.path,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            color: colors.foreground,
                            fontSize: 10.5,
                            fontFamily: 'monospace',
                            letterSpacing: 0,
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      Text(
                        _availableOfTotal(
                          context,
                          filesystem.usedBytes,
                          filesystem.totalBytes,
                        ),
                        style: TextStyle(
                          color: colors.muted,
                          fontSize: 9.5,
                          letterSpacing: 0,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 5),
                  _TerminalSystemProgress(
                    value: _usagePercent(
                      filesystem.usedBytes,
                      filesystem.totalBytes,
                    ),
                    colors: colors,
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

class _TerminalSystemSection extends StatelessWidget {
  const _TerminalSystemSection({
    required this.title,
    required this.colors,
    required this.child,
    this.trailing,
  });

  final String title;
  final _AiAssistantColors colors;
  final Widget child;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: colors.inputBackground.withValues(alpha: 0.34),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: colors.border),
      ),
      clipBehavior: Clip.antiAlias,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(10, 7, 8, 7),
            child: Row(
              children: [
                Expanded(
                  child: Text(
                    tr(title),
                    style: TextStyle(
                      color: colors.foreground,
                      fontSize: 11.5,
                      fontWeight: NautermFontWeights.semibold,
                      letterSpacing: 0,
                    ),
                  ),
                ),
                ?trailing,
              ],
            ),
          ),
          Divider(height: 1, color: colors.border),
          child,
        ],
      ),
    );
  }
}

class _TerminalSystemTableRow extends StatelessWidget {
  const _TerminalSystemTableRow({
    required this.cells,
    required this.flexes,
    required this.colors,
  });

  final List<String> cells;
  final List<int> flexes;
  final _AiAssistantColors colors;

  @override
  Widget build(BuildContext context) {
    return Container(
      color: Colors.transparent,
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 6),
      child: Row(
        children: [
          for (var index = 0; index < cells.length; index++)
            Expanded(
              flex: flexes[index],
              child: Text(
                cells[index],
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: colors.foreground,
                  fontSize: 10.5,
                  fontWeight: NautermFontWeights.regular,
                  fontFamily: index == cells.length - 1 ? 'monospace' : null,
                  letterSpacing: 0,
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _TerminalSystemRate extends StatelessWidget {
  const _TerminalSystemRate({
    required this.icon,
    required this.label,
    required this.color,
  });

  final IconData icon;
  final String label;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 13, color: color),
        const SizedBox(width: 3),
        Text(
          tr(label),
          style: TextStyle(
            color: color,
            fontSize: 10.5,
            fontWeight: NautermFontWeights.semibold,
            letterSpacing: 0,
          ),
        ),
      ],
    );
  }
}

class _TerminalSystemProgress extends StatelessWidget {
  const _TerminalSystemProgress({
    super.key,
    required this.value,
    required this.colors,
    this.height = 3,
  });

  final double? value;
  final _AiAssistantColors colors;
  final double height;

  @override
  Widget build(BuildContext context) {
    final progress = _normalizedSystemProgress(value);
    return ClipRRect(
      borderRadius: BorderRadius.circular(2),
      child: SizedBox(
        height: height,
        child: LayoutBuilder(
          builder: (context, constraints) {
            final fillWidth = constraints.maxWidth * progress;
            return Stack(
              fit: StackFit.expand,
              children: [
                ColoredBox(color: colors.border),
                if (fillWidth > 0)
                  Align(
                    alignment: Alignment.centerLeft,
                    child: SizedBox(
                      key: const ValueKey('terminal-system-progress-fill'),
                      width: fillWidth,
                      height: height,
                      child: ColoredBox(color: colors.accent),
                    ),
                  ),
              ],
            );
          },
        ),
      ),
    );
  }
}

class _TerminalSystemTrendPainter extends CustomPainter {
  const _TerminalSystemTrendPainter({
    required this.primary,
    required this.primaryColor,
    required this.gridColor,
    this.secondary = const [],
    this.secondaryColor,
  });

  final List<double> primary;
  final List<double> secondary;
  final Color primaryColor;
  final Color? secondaryColor;
  final Color gridColor;

  @override
  void paint(Canvas canvas, Size size) {
    final gridPaint = Paint()
      ..color = gridColor.withValues(alpha: 0.55)
      ..strokeWidth = 1;
    canvas.drawLine(
      Offset(0, size.height - 0.5),
      Offset(size.width, size.height - 0.5),
      gridPaint,
    );
    final maximum = [
      ...primary,
      ...secondary,
    ].fold<double>(1, (value, sample) => math.max(value, sample));
    _paintSeries(canvas, size, primary, primaryColor, maximum);
    if (secondaryColor != null) {
      _paintSeries(canvas, size, secondary, secondaryColor!, maximum);
    }
  }

  void _paintSeries(
    Canvas canvas,
    Size size,
    List<double> values,
    Color color,
    double maximum,
  ) {
    if (values.isEmpty) {
      return;
    }
    final path = Path();
    for (var index = 0; index < values.length; index++) {
      final x = values.length == 1
          ? size.width
          : index * size.width / (values.length - 1);
      final y = size.height - (values[index] / maximum) * size.height;
      if (index == 0) {
        path.moveTo(x, y);
      } else {
        path.lineTo(x, y);
      }
    }
    canvas.drawPath(
      path,
      Paint()
        ..color = color
        ..strokeWidth = 1.4
        ..style = PaintingStyle.stroke,
    );
  }

  @override
  bool shouldRepaint(covariant _TerminalSystemTrendPainter oldDelegate) {
    return true;
  }
}

String _formatRate(double bytesPerSecond) {
  return '${_formatBytes(bytesPerSecond.round())}/s';
}

class _TerminalToolSectionHeader extends StatelessWidget {
  const _TerminalToolSectionHeader({
    required this.title,
    required this.colors,
    this.subtitle,
    this.trailing,
  });

  final String title;
  final String? subtitle;
  final _AiAssistantColors colors;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 54,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 0, 10, 0),
        child: Row(
          children: [
            Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    tr(title),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: colors.foreground,
                      fontSize: 13,
                      fontWeight: NautermFontWeights.semibold,
                      letterSpacing: 0,
                    ),
                  ),
                  if (subtitle?.trim().isNotEmpty == true) ...[
                    const SizedBox(height: 2),
                    Text(
                      subtitle!,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: colors.muted,
                        fontSize: 10.5,
                        letterSpacing: 0,
                      ),
                    ),
                  ],
                ],
              ),
            ),
            ?trailing,
          ],
        ),
      ),
    );
  }
}

class _TerminalSystemMetric extends StatelessWidget {
  const _TerminalSystemMetric({
    required this.label,
    required this.value,
    required this.detail,
    required this.progress,
    required this.colors,
  });

  final String label;
  final String value;
  final String detail;
  final double? progress;
  final _AiAssistantColors colors;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.fromLTRB(10, 9, 10, 10),
      decoration: BoxDecoration(
        color: colors.inputBackground.withValues(alpha: 0.52),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: colors.border),
      ),
      child: Column(
        children: [
          Row(
            children: [
              Text(
                tr(label),
                style: TextStyle(
                  color: colors.foreground,
                  fontSize: 11.5,
                  fontWeight: NautermFontWeights.semibold,
                  letterSpacing: 0,
                ),
              ),
              const Spacer(),
              if (detail.isNotEmpty)
                Flexible(
                  child: Text(
                    detail,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    textAlign: TextAlign.right,
                    style: TextStyle(
                      color: colors.muted,
                      fontSize: 10,
                      letterSpacing: 0,
                    ),
                  ),
                ),
              const SizedBox(width: 8),
              Text(
                value,
                style: TextStyle(
                  color: colors.foreground,
                  fontSize: 11.5,
                  fontWeight: NautermFontWeights.semibold,
                  letterSpacing: 0,
                  fontFeatures: const [FontFeature.tabularFigures()],
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          _TerminalSystemProgress(
            key: ValueKey('terminal-system-progress-${label.toLowerCase()}'),
            value: progress,
            colors: colors,
            height: 4,
          ),
        ],
      ),
    );
  }
}

class _TerminalSystemDetails extends StatelessWidget {
  const _TerminalSystemDetails({required this.colors, required this.rows});

  final _AiAssistantColors colors;
  final List<(String, String)> rows;

  @override
  Widget build(BuildContext context) {
    if (rows.isEmpty) {
      return const SizedBox.shrink();
    }
    return Semantics(
      container: true,
      label: context.tr('workspace.systemInfo.details.title'),
      child: Container(
        decoration: BoxDecoration(
          border: Border.all(color: colors.border),
          borderRadius: BorderRadius.circular(8),
        ),
        child: Column(
          children: [
            for (var index = 0; index < rows.length; index++) ...[
              Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 8,
                ),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SizedBox(
                      width: 92,
                      child: Text(
                        context.tr(rows[index].$1),
                        style: TextStyle(
                          color: colors.muted,
                          fontSize: 10.5,
                          letterSpacing: 0,
                        ),
                      ),
                    ),
                    Expanded(
                      child: Text(
                        rows[index].$2,
                        textAlign: TextAlign.right,
                        style: TextStyle(
                          color: colors.foreground,
                          fontSize: 10.5,
                          fontWeight: NautermFontWeights.medium,
                          height: 1.35,
                          letterSpacing: 0,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
              if (index + 1 < rows.length)
                Divider(height: 1, color: colors.border),
            ],
          ],
        ),
      ),
    );
  }
}

class _TerminalToolEmptyState extends StatelessWidget {
  const _TerminalToolEmptyState({
    required this.icon,
    required this.title,
    required this.description,
    required this.colors,
    this.actionLabel,
    this.onAction,
  });

  final IconData icon;
  final String title;
  final String description;
  final _AiAssistantColors colors;
  final String? actionLabel;
  final VoidCallback? onAction;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 24, color: colors.muted),
            const SizedBox(height: 12),
            Text(
              tr(title),
              textAlign: TextAlign.center,
              style: TextStyle(
                color: colors.foreground,
                fontSize: 12.5,
                fontWeight: NautermFontWeights.semibold,
                letterSpacing: 0,
              ),
            ),
            const SizedBox(height: 6),
            Text(
              description,
              textAlign: TextAlign.center,
              style: TextStyle(
                color: colors.muted,
                fontSize: 11,
                height: 1.4,
                letterSpacing: 0,
              ),
            ),
            if (actionLabel != null && onAction != null) ...[
              const SizedBox(height: 12),
              TextButton(
                onPressed: onAction,
                style: TextButton.styleFrom(
                  foregroundColor: colors.accent,
                  overlayColor: colors.accent.withValues(alpha: 0.10),
                ),
                child: Text(actionLabel!),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

double? _usagePercent(int? used, int? total) {
  if (used == null || total == null || total <= 0) {
    return null;
  }
  return (used / total).clamp(0.0, 1.0);
}

double _normalizedSystemProgress(double? value) {
  if (value == null || !value.isFinite) {
    return 0;
  }
  return value.clamp(0.0, 1.0);
}

String _usedOfTotal(int? used, int? total) {
  if (used == null || total == null || total <= 0) {
    return '';
  }
  return '${_formatBytes(used)} / ${_formatBytes(total)}';
}

String _availableOfTotal(BuildContext context, int used, int total) {
  final available = math.max(0, total - used);
  return context.tr(
    'terminal.systemInfo.disk.free',
    args: {'free': _formatBytes(available), 'total': _formatBytes(total)},
  );
}

String _formatBytes(int bytes) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  var value = bytes.toDouble();
  var unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  final digits = value >= 100 || unit == 0 ? 0 : 1;
  return '${value.toStringAsFixed(digits)} ${units[unit]}';
}

String _formatUptime(int seconds) {
  final days = seconds ~/ Duration.secondsPerDay;
  final hours = (seconds % Duration.secondsPerDay) ~/ Duration.secondsPerHour;
  final minutes =
      (seconds % Duration.secondsPerHour) ~/ Duration.secondsPerMinute;
  final parts = <String>[
    if (days > 0) '${days}d',
    if (hours > 0 || days > 0) '${hours}h',
    '${minutes}m',
  ];
  return parts.join(' ');
}
