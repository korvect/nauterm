import 'dart:convert';
import 'dart:io';

import '../data/nauterm_paths.dart';

const int workspaceStateSchemaVersion = 1;

enum WorkspaceTerminalProtocol { local, ssh, mosh, telnet }

enum WorkspaceSplitAxis { horizontal, vertical }

enum WorkspaceRestoreLaunchAction { none, ask, restore }

class WorkspaceTerminalTargetSnapshot {
  const WorkspaceTerminalTargetSnapshot({
    required this.protocol,
    this.hostUuid,
    this.shellPath,
  });

  final WorkspaceTerminalProtocol protocol;
  final String? hostUuid;
  final String? shellPath;

  Map<String, Object?> toJson() => <String, Object?>{
    'protocol': protocol.name,
    if (hostUuid != null) 'hostUuid': hostUuid,
    if (shellPath != null) 'shellPath': shellPath,
  };

  factory WorkspaceTerminalTargetSnapshot.fromJson(Object? value) {
    final json = _jsonMap(value);
    final protocolName = json['protocol'];
    final protocol = WorkspaceTerminalProtocol.values
        .where((candidate) => candidate.name == protocolName)
        .firstOrNull;
    if (protocol == null) {
      throw const FormatException('Invalid terminal protocol.');
    }
    return WorkspaceTerminalTargetSnapshot(
      protocol: protocol,
      hostUuid: _nonEmptyString(json['hostUuid']),
      shellPath: _nonEmptyString(json['shellPath']),
    );
  }
}

class WorkspaceTerminalSessionSnapshot {
  const WorkspaceTerminalSessionSnapshot({
    required this.id,
    required this.title,
    required this.target,
  });

  final int id;
  final String title;
  final WorkspaceTerminalTargetSnapshot target;

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'title': title,
    'target': target.toJson(),
  };

  factory WorkspaceTerminalSessionSnapshot.fromJson(Object? value) {
    final json = _jsonMap(value);
    return WorkspaceTerminalSessionSnapshot(
      id: _positiveInt(json['id']),
      title: _nonEmptyString(json['title']) ?? 'Terminal',
      target: WorkspaceTerminalTargetSnapshot.fromJson(json['target']),
    );
  }
}

class WorkspacePaneSnapshot {
  const WorkspacePaneSnapshot({
    required this.id,
    required this.sessions,
    this.selectedSessionId,
    this.composerVisible = true,
    this.workingDirectory,
  });

  final int id;
  final List<WorkspaceTerminalSessionSnapshot> sessions;
  final int? selectedSessionId;
  final bool composerVisible;
  final String? workingDirectory;

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'selectedSessionId': selectedSessionId,
    'composerVisible': composerVisible,
    if (workingDirectory != null) 'workingDirectory': workingDirectory,
    'sessions': [for (final session in sessions) session.toJson()],
  };

  factory WorkspacePaneSnapshot.fromJson(Object? value) {
    final json = _jsonMap(value);
    return WorkspacePaneSnapshot(
      id: _positiveInt(json['id']),
      selectedSessionId: _optionalPositiveInt(json['selectedSessionId']),
      composerVisible: json['composerVisible'] as bool? ?? true,
      workingDirectory: _nonEmptyString(json['workingDirectory']),
      sessions: _jsonList(json['sessions'])
          .map(WorkspaceTerminalSessionSnapshot.fromJson)
          .toList(growable: false),
    );
  }
}

class WorkspaceLayoutSnapshot {
  const WorkspaceLayoutSnapshot.leaf(this.pane)
    : splitId = null,
      axis = null,
      fractions = const [],
      children = const [];

  const WorkspaceLayoutSnapshot.split({
    required int this.splitId,
    required WorkspaceSplitAxis this.axis,
    required this.fractions,
    required this.children,
  }) : pane = null;

  final WorkspacePaneSnapshot? pane;
  final int? splitId;
  final WorkspaceSplitAxis? axis;
  final List<double> fractions;
  final List<WorkspaceLayoutSnapshot> children;

  bool get isLeaf => pane != null;

  Iterable<WorkspaceTerminalSessionSnapshot> get sessions sync* {
    final leaf = pane;
    if (leaf != null) {
      yield* leaf.sessions;
      return;
    }
    for (final child in children) {
      yield* child.sessions;
    }
  }

  Map<String, Object?> toJson() {
    final leaf = pane;
    if (leaf != null) {
      return <String, Object?>{'type': 'leaf', 'pane': leaf.toJson()};
    }
    return <String, Object?>{
      'type': 'split',
      'id': splitId,
      'axis': axis!.name,
      'fractions': fractions,
      'children': [for (final child in children) child.toJson()],
    };
  }

  factory WorkspaceLayoutSnapshot.fromJson(Object? value) {
    final json = _jsonMap(value);
    if (json['type'] == 'leaf') {
      return WorkspaceLayoutSnapshot.leaf(
        WorkspacePaneSnapshot.fromJson(json['pane']),
      );
    }
    if (json['type'] != 'split') {
      throw const FormatException('Invalid workspace layout node.');
    }
    final axisName = json['axis'];
    final axis = WorkspaceSplitAxis.values
        .where((candidate) => candidate.name == axisName)
        .firstOrNull;
    if (axis == null) {
      throw const FormatException('Invalid workspace split axis.');
    }
    final children = _jsonList(json['children'])
        .map(WorkspaceLayoutSnapshot.fromJson)
        .toList(growable: false);
    if (children.length < 2) {
      throw const FormatException('A split must contain at least two panes.');
    }
    return WorkspaceLayoutSnapshot.split(
      splitId: _positiveInt(json['id']),
      axis: axis,
      fractions: _normalizedFractions(json['fractions'], children.length),
      children: children,
    );
  }
}

class WorkspaceTabSnapshot {
  const WorkspaceTabSnapshot({
    required this.id,
    required this.title,
    required this.layout,
    this.showSftp = false,
  });

  final int id;
  final String title;
  final WorkspaceLayoutSnapshot layout;
  final bool showSftp;

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'title': title,
    'showSftp': showSftp,
    'layout': layout.toJson(),
  };

  factory WorkspaceTabSnapshot.fromJson(Object? value) {
    final json = _jsonMap(value);
    return WorkspaceTabSnapshot(
      id: _positiveInt(json['id']),
      title: _nonEmptyString(json['title']) ?? 'Terminal',
      showSftp: json['showSftp'] as bool? ?? false,
      layout: WorkspaceLayoutSnapshot.fromJson(json['layout']),
    );
  }
}

class WorkspaceSnapshot {
  const WorkspaceSnapshot({
    required this.id,
    required this.name,
    required this.colorValue,
    required this.tabs,
    this.selectedTabId,
    this.selectedPaneId,
  });

  final int id;
  final String name;
  final int colorValue;
  final List<WorkspaceTabSnapshot> tabs;
  final int? selectedTabId;
  final int? selectedPaneId;

  Map<String, Object?> toJson() => <String, Object?>{
    'id': id,
    'name': name,
    'color': colorValue,
    'selectedTabId': selectedTabId,
    'selectedPaneId': selectedPaneId,
    'tabs': [for (final tab in tabs) tab.toJson()],
  };

  factory WorkspaceSnapshot.fromJson(Object? value) {
    final json = _jsonMap(value);
    return WorkspaceSnapshot(
      id: _positiveInt(json['id']),
      name: _nonEmptyString(json['name']) ?? 'Workspace',
      colorValue: (json['color'] as num?)?.toInt() ?? 0xff075e92,
      selectedTabId: _optionalPositiveInt(json['selectedTabId']),
      selectedPaneId: _optionalPositiveInt(json['selectedPaneId']),
      tabs: _jsonList(json['tabs'])
          .map(WorkspaceTabSnapshot.fromJson)
          .toList(growable: false),
    );
  }
}

class NautermWorkspaceStateSnapshot {
  const NautermWorkspaceStateSnapshot({
    this.version = workspaceStateSchemaVersion,
    required this.cleanShutdown,
    required this.restoreOnNextLaunch,
    required this.savedAt,
    this.selectedWorkspaceId,
    this.workspaces = const [],
    this.activePortForwardUuids = const [],
  });

  factory NautermWorkspaceStateSnapshot.running({DateTime? savedAt}) =>
      NautermWorkspaceStateSnapshot(
        cleanShutdown: false,
        restoreOnNextLaunch: false,
        savedAt: savedAt ?? DateTime.now().toUtc(),
      );

  final int version;
  final bool cleanShutdown;
  final bool restoreOnNextLaunch;
  final DateTime savedAt;
  final int? selectedWorkspaceId;
  final List<WorkspaceSnapshot> workspaces;
  final List<String> activePortForwardUuids;

  bool get hasRestorableContent =>
      activePortForwardUuids.isNotEmpty ||
      workspaces.any(
        (workspace) =>
            workspace.tabs.any((tab) => tab.layout.sessions.isNotEmpty),
      );

  WorkspaceRestoreLaunchAction get launchAction {
    if (!hasRestorableContent) return WorkspaceRestoreLaunchAction.none;
    if (!cleanShutdown) return WorkspaceRestoreLaunchAction.ask;
    return restoreOnNextLaunch
        ? WorkspaceRestoreLaunchAction.restore
        : WorkspaceRestoreLaunchAction.none;
  }

  NautermWorkspaceStateSnapshot copyWith({
    bool? cleanShutdown,
    bool? restoreOnNextLaunch,
    DateTime? savedAt,
    int? selectedWorkspaceId,
    List<WorkspaceSnapshot>? workspaces,
    List<String>? activePortForwardUuids,
  }) {
    return NautermWorkspaceStateSnapshot(
      version: version,
      cleanShutdown: cleanShutdown ?? this.cleanShutdown,
      restoreOnNextLaunch: restoreOnNextLaunch ?? this.restoreOnNextLaunch,
      savedAt: savedAt ?? this.savedAt,
      selectedWorkspaceId: selectedWorkspaceId ?? this.selectedWorkspaceId,
      workspaces: workspaces ?? this.workspaces,
      activePortForwardUuids:
          activePortForwardUuids ?? this.activePortForwardUuids,
    );
  }

  Map<String, Object?> toJson() => <String, Object?>{
    'version': version,
    'cleanShutdown': cleanShutdown,
    'restoreOnNextLaunch': restoreOnNextLaunch,
    'savedAt': savedAt.toUtc().toIso8601String(),
    'selectedWorkspaceId': selectedWorkspaceId,
    'activePortForwardUuids': activePortForwardUuids,
    'workspaces': [for (final workspace in workspaces) workspace.toJson()],
  };

  factory NautermWorkspaceStateSnapshot.fromJson(Object? value) {
    final json = _jsonMap(value);
    final version = (json['version'] as num?)?.toInt();
    if (version != workspaceStateSchemaVersion) {
      throw FormatException('Unsupported workspace state version: $version');
    }
    final savedAt = DateTime.tryParse(json['savedAt'] as String? ?? '');
    if (savedAt == null) {
      throw const FormatException('Invalid workspace state timestamp.');
    }
    return NautermWorkspaceStateSnapshot(
      version: version!,
      cleanShutdown: json['cleanShutdown'] as bool? ?? false,
      restoreOnNextLaunch: json['restoreOnNextLaunch'] as bool? ?? false,
      savedAt: savedAt.toUtc(),
      selectedWorkspaceId: _optionalPositiveInt(json['selectedWorkspaceId']),
      activePortForwardUuids: _jsonList(json['activePortForwardUuids'])
          .whereType<String>()
          .map((value) => value.trim())
          .where((value) => value.isNotEmpty)
          .toSet()
          .toList(growable: false),
      workspaces: _jsonList(json['workspaces'])
          .map(WorkspaceSnapshot.fromJson)
          .toList(growable: false),
    );
  }
}

class WorkspaceStateStore {
  WorkspaceStateStore(this.paths);

  final NautermPaths paths;
  Future<void> _saveQueue = Future<void>.value();

  Future<NautermWorkspaceStateSnapshot?> load() async {
    try {
      final file = paths.workspaceStateFile;
      if (!await file.exists() || await file.length() == 0) {
        return null;
      }
      return NautermWorkspaceStateSnapshot.fromJson(
        jsonDecode(await file.readAsString()),
      );
    } on Object {
      return null;
    }
  }

  Future<NautermWorkspaceStateSnapshot?> beginRun() async {
    final previous = await load();
    final preservePrevious =
        previous != null &&
        previous.launchAction != WorkspaceRestoreLaunchAction.none;
    await save(
      preservePrevious
          ? previous.copyWith(
              cleanShutdown: false,
              restoreOnNextLaunch: false,
              savedAt: DateTime.now().toUtc(),
            )
          : NautermWorkspaceStateSnapshot.running(),
    );
    return previous;
  }

  Future<void> save(NautermWorkspaceStateSnapshot snapshot) {
    final operation = _saveQueue.then((_) => _write(snapshot));
    _saveQueue = operation.then<void>(
      (_) {},
      onError: (Object _, StackTrace _) {},
    );
    return operation;
  }

  Future<void> _write(NautermWorkspaceStateSnapshot snapshot) async {
    await paths.ensureCreated();
    final destination = paths.workspaceStateFile;
    final temporary = File('${destination.path}.tmp');
    const encoder = JsonEncoder.withIndent('  ');
    try {
      await temporary.writeAsString(
        '${encoder.convert(snapshot.toJson())}\n',
        flush: true,
      );
      if (!Platform.isWindows) {
        try {
          await Process.run('chmod', ['600', temporary.path]);
        } on Object {
          // The state contains references only; restrictive permissions are a
          // best-effort hardening step on Unix-like systems.
        }
      }
      await temporary.rename(destination.path);
    } finally {
      if (await temporary.exists()) {
        await temporary.delete();
      }
    }
  }
}

Map<String, Object?> _jsonMap(Object? value) {
  if (value is Map) return value.cast<String, Object?>();
  throw const FormatException('Expected a JSON object.');
}

List<Object?> _jsonList(Object? value) {
  if (value == null) return const [];
  if (value is List) return value.cast<Object?>();
  throw const FormatException('Expected a JSON array.');
}

int _positiveInt(Object? value) {
  final result = (value as num?)?.toInt();
  if (result == null || result <= 0) {
    throw const FormatException('Expected a positive integer.');
  }
  return result;
}

int? _optionalPositiveInt(Object? value) {
  if (value == null) return null;
  return _positiveInt(value);
}

String? _nonEmptyString(Object? value) {
  if (value is! String) return null;
  final result = value.trim();
  return result.isEmpty ? null : result;
}

List<double> _normalizedFractions(Object? value, int count) {
  final values = value is List
      ? value
            .whereType<num>()
            .map((number) => number.toDouble())
            .toList(growable: false)
      : const <double>[];
  if (values.length != count || values.any((value) => value <= 0)) {
    return List<double>.filled(count, 1 / count, growable: false);
  }
  final total = values.fold<double>(0, (sum, value) => sum + value);
  if (!total.isFinite || total <= 0) {
    return List<double>.filled(count, 1 / count, growable: false);
  }
  return [for (final value in values) value / total];
}
