import 'dart:io';
import 'dart:isolate';

import '../app/nauterm_log.dart';
import '../terminal/terminal_recording_config.dart';
import 'known_hosts_store.dart';
import 'nauterm_data_store.dart';
import 'nauterm_paths.dart';
import 'shell_history_file_store.dart';
import 'terminal_recording_store.dart';
import 'terminal_retention_policy.dart';

typedef WorkspaceDataLoader = Future<NautermWorkspaceData> Function(
  NautermPaths paths,
  NautermRecordingConfig recording,
);

/// Initial workspace data; database and capture handles remain in the worker.
class NautermWorkspaceData {
  const NautermWorkspaceData({
    this.groups = const [],
    this.hosts = const [],
    this.keys = const [],
    this.identities = const [],
    this.tags = const [],
    this.portForwards = const [],
    this.proxies = const [],
    this.snippetPackages = const [],
    this.snippets = const [],
    this.aiProviders = const [],
    this.terminalLogs = const [],
    this.shellHistory = const [],
    this.knownHostsText = '',
    this.captureDiskUsage = 0,
    this.deletedLogIds = const {},
  });

  static const logPageSize = 40;
  final List<HostGroup> groups;
  final List<HostEntry> hosts;
  final List<KeyEntry> keys;
  final List<IdentityEntry> identities;
  final List<TagEntry> tags;
  final List<PortForwardEntry> portForwards;
  final List<ProxyEntry> proxies;
  final List<SnippetPackageEntry> snippetPackages;
  final List<SnippetEntry> snippets;
  final List<AiProviderEntry> aiProviders;
  final List<TerminalLogEntry> terminalLogs;
  final List<ShellHistoryEntry> shellHistory;
  final String knownHostsText;
  final int captureDiskUsage;
  final Set<String> deletedLogIds;
}

Future<NautermWorkspaceData> loadNautermWorkspaceData(
  NautermPaths paths,
  NautermRecordingConfig recording,
) {
  final databasePath = paths.databasePath;
  final capturesPath = paths.terminalLogsDirectory.path;
  final knownHostsPath = paths.knownHostsFile.path;
  final historyPath = paths.shellHistoryFile.path;
  return Isolate.run(() async {
    final store = NautermDataStore.openPath(databasePath);
    try {
      final captures = TerminalLogCaptureStore(Directory(capturesPath));
      await captures.prepare();
      for (final log in store.listIncompleteTerminalCaptures()) {
        try {
          if (!await captures.captureExists(log.captureFile)) {
            store.clearMissingTerminalCapture(log.id);
            continue;
          }
          final recovered = await captures.recover(
            logId: log.id,
            captureFile: log.captureFile,
          );
          store.finalizeRecoveredTerminalCapture(
            logId: log.id,
            captureBytes: recovered.ciphertextBytes,
            captureSha256: recovered.fileSha256,
            endedAt: DateTime.now().toUtc(),
          );
        } on Object catch (error, stackTrace) {
          NautermLog.warning(
            'recording',
            'Unable to recover terminal capture.',
            error: error,
            stackTrace: stackTrace,
          );
        }
      }
      await captures.cleanupOrphans(
        store.listTerminalCaptureFiles().toSet(),
        referencedStateFiles: store
            .listIncompleteTerminalCaptures()
            .map((log) => log.captureFile)
            .toSet(),
      );
      final retention = await applyTerminalRetentionPolicy(
        dataStore: store,
        captureStore: captures,
        config: recording,
      );
      return NautermWorkspaceData(
        groups: store.listGroups(),
        hosts: store.listHosts(),
        keys: store.listKeys(),
        identities: store.listIdentities(),
        tags: store.listTags(),
        portForwards: store.listPortForwards(),
        proxies: store.listProxies(),
        snippetPackages: store.listSnippetPackages(),
        snippets: store.listSnippets(),
        aiProviders: store.listAiProviders(),
        terminalLogs: store.listTerminalLogs(
          limit: NautermWorkspaceData.logPageSize,
        ),
        shellHistory: await ShellHistoryFileStore(File(historyPath)).read(),
        knownHostsText: await KnownHostsStore(File(knownHostsPath)).readText(),
        captureDiskUsage: await captures.diskUsage(),
        deletedLogIds: retention.deletedLogIds,
      );
    } finally {
      store.dispose();
    }
  });
}
