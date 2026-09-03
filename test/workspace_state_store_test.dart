import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:nauterm/data/nauterm_paths.dart';
import 'package:nauterm/workspace/workspace_state_store.dart';

void main() {
  late Directory directory;
  late NautermPaths paths;
  late WorkspaceStateStore store;

  setUp(() async {
    directory = await Directory.systemTemp.createTemp(
      'nauterm_workspace_state_test_',
    );
    paths = NautermPaths(configDirectory: directory, dataDirectory: directory);
    store = WorkspaceStateStore(paths);
  });

  tearDown(() async {
    if (await directory.exists()) {
      await directory.delete(recursive: true);
    }
  });

  test('workspace state round trips layout and stable references', () async {
    final savedAt = DateTime.utc(2026, 9, 4, 1, 2, 3);
    final snapshot = NautermWorkspaceStateSnapshot(
      cleanShutdown: true,
      restoreOnNextLaunch: true,
      savedAt: savedAt,
      selectedWorkspaceId: 7,
      activePortForwardUuids: const ['forward-uuid'],
      workspaces: [
        WorkspaceSnapshot(
          id: 7,
          name: 'Production',
          colorValue: 0xff075e92,
          selectedTabId: 11,
          selectedPaneId: 13,
          tabs: [
            WorkspaceTabSnapshot(
              id: 11,
              title: 'Servers',
              showSftp: true,
              layout: WorkspaceLayoutSnapshot.split(
                splitId: 12,
                axis: WorkspaceSplitAxis.horizontal,
                fractions: const [0.35, 0.65],
                children: [
                  WorkspaceLayoutSnapshot.leaf(
                    WorkspacePaneSnapshot(
                      id: 13,
                      selectedSessionId: 13,
                      workingDirectory: '/srv/api',
                      sessions: const [
                        WorkspaceTerminalSessionSnapshot(
                          id: 13,
                          title: 'API',
                          target: WorkspaceTerminalTargetSnapshot(
                            protocol: WorkspaceTerminalProtocol.ssh,
                            hostUuid: 'host-api-uuid',
                          ),
                        ),
                      ],
                    ),
                  ),
                  WorkspaceLayoutSnapshot.leaf(
                    WorkspacePaneSnapshot(
                      id: 14,
                      selectedSessionId: 14,
                      sessions: const [
                        WorkspaceTerminalSessionSnapshot(
                          id: 14,
                          title: 'Local',
                          target: WorkspaceTerminalTargetSnapshot(
                            protocol: WorkspaceTerminalProtocol.local,
                            shellPath: '/bin/zsh',
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ],
    );

    await store.save(snapshot);
    final loaded = await store.load();

    expect(loaded, isNotNull);
    expect(loaded!.cleanShutdown, isTrue);
    expect(loaded.restoreOnNextLaunch, isTrue);
    expect(loaded.savedAt, savedAt);
    expect(loaded.selectedWorkspaceId, 7);
    expect(loaded.activePortForwardUuids, ['forward-uuid']);
    final tab = loaded.workspaces.single.tabs.single;
    expect(tab.showSftp, isTrue);
    expect(tab.layout.axis, WorkspaceSplitAxis.horizontal);
    expect(tab.layout.fractions, [0.35, 0.65]);
    expect(tab.layout.sessions.length, 2);
    expect(tab.layout.sessions.first.target.hostUuid, 'host-api-uuid');

    final json = jsonDecode(await paths.workspaceStateFile.readAsString());
    final serialized = jsonEncode(json);
    expect(serialized, isNot(contains('password')));
    expect(serialized, isNot(contains('privateKey')));
    expect(serialized, isNot(contains('certificate')));
    expect(
      await File('${paths.workspaceStateFile.path}.tmp').exists(),
      isFalse,
    );
  });

  test('beginRun preserves an unexpected-exit recovery candidate', () async {
    final previous = _restorableSnapshot(
      cleanShutdown: false,
      restoreOnNextLaunch: false,
    );
    await store.save(previous);

    final loadedPrevious = await store.beginRun();
    final running = await store.load();

    expect(loadedPrevious?.cleanShutdown, isFalse);
    expect(running?.cleanShutdown, isFalse);
    expect(running?.restoreOnNextLaunch, isFalse);
    expect(running?.hasRestorableContent, isTrue);
  });

  test('unexpected exits always require a restore decision', () {
    expect(
      _restorableSnapshot(
        cleanShutdown: false,
        restoreOnNextLaunch: false,
      ).launchAction,
      WorkspaceRestoreLaunchAction.ask,
    );
    expect(
      _restorableSnapshot(
        cleanShutdown: false,
        restoreOnNextLaunch: true,
      ).launchAction,
      WorkspaceRestoreLaunchAction.ask,
    );
  });

  test('clean exits restore only when selected for the next launch', () {
    expect(
      _restorableSnapshot(
        cleanShutdown: true,
        restoreOnNextLaunch: true,
      ).launchAction,
      WorkspaceRestoreLaunchAction.restore,
    );
    expect(
      _restorableSnapshot(
        cleanShutdown: true,
        restoreOnNextLaunch: false,
      ).launchAction,
      WorkspaceRestoreLaunchAction.none,
    );
  });

  test('beginRun preserves a clean snapshot selected for restore', () async {
    await store.save(
      _restorableSnapshot(cleanShutdown: true, restoreOnNextLaunch: true),
    );

    final loadedPrevious = await store.beginRun();
    final running = await store.load();

    expect(loadedPrevious?.launchAction, WorkspaceRestoreLaunchAction.restore);
    expect(running?.cleanShutdown, isFalse);
    expect(running?.restoreOnNextLaunch, isFalse);
    expect(running?.hasRestorableContent, isTrue);
  });

  test('beginRun discards a clean snapshot not selected for restore', () async {
    await store.save(
      _restorableSnapshot(cleanShutdown: true, restoreOnNextLaunch: false),
    );

    final loadedPrevious = await store.beginRun();
    final running = await store.load();

    expect(loadedPrevious?.cleanShutdown, isTrue);
    expect(loadedPrevious?.hasRestorableContent, isTrue);
    expect(running?.cleanShutdown, isFalse);
    expect(running?.hasRestorableContent, isFalse);
  });

  test('malformed state starts a new run without failing startup', () async {
    await paths.ensureCreated();
    await paths.workspaceStateFile.writeAsString('{not json');

    final previous = await store.beginRun();
    final running = await store.load();

    expect(previous, isNull);
    expect(running, isNotNull);
    expect(running?.cleanShutdown, isFalse);
    expect(running?.hasRestorableContent, isFalse);
  });
}

NautermWorkspaceStateSnapshot _restorableSnapshot({
  required bool cleanShutdown,
  required bool restoreOnNextLaunch,
}) {
  return NautermWorkspaceStateSnapshot(
    cleanShutdown: cleanShutdown,
    restoreOnNextLaunch: restoreOnNextLaunch,
    savedAt: DateTime.utc(2026, 9, 4),
    selectedWorkspaceId: 1,
    workspaces: const [
      WorkspaceSnapshot(
        id: 1,
        name: 'Default',
        colorValue: 0xff075e92,
        selectedTabId: 2,
        selectedPaneId: 2,
        tabs: [
          WorkspaceTabSnapshot(
            id: 2,
            title: 'Host',
            layout: WorkspaceLayoutSnapshot.leaf(
              WorkspacePaneSnapshot(
                id: 2,
                selectedSessionId: 2,
                sessions: [
                  WorkspaceTerminalSessionSnapshot(
                    id: 2,
                    title: 'Host',
                    target: WorkspaceTerminalTargetSnapshot(
                      protocol: WorkspaceTerminalProtocol.ssh,
                      hostUuid: 'host-uuid',
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    ],
  );
}
