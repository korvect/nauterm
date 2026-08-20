part of 'settings_panel.dart';

enum _SyncProvider {
  githubRepository,
  githubGist,
  googleDrive,
  oneDrive,
  webdav,
  s3Compatible,
}

enum _SyncStrategy { smartMerge, localWins, remoteWins }

extension on _SyncStrategy {
  String get storageValue => switch (this) {
    _SyncStrategy.smartMerge => 'smart_merge',
    _SyncStrategy.localWins => 'local_wins',
    _SyncStrategy.remoteWins => 'remote_wins',
  };

  String get label => switch (this) {
    _SyncStrategy.smartMerge => tr(
      'settings.sync.mergeStrategy.smartMerge',
      fallback: 'Smart Merge',
    ),
    _SyncStrategy.localWins => tr(
      'settings.sync.mergeStrategy.localWins',
      fallback: 'Local Overwrite Cloud',
    ),
    _SyncStrategy.remoteWins => tr(
      'settings.sync.mergeStrategy.remoteWins',
      fallback: 'Cloud Overwrite Local',
    ),
  };
}

Map<String, Object?>? _cachedGithubSyncSettings;
String? _cachedGithubSyncDatabasePath;

@visibleForTesting
void cacheGithubSyncSettingsForTesting(
  String databasePath,
  Map<String, Object?> settings,
) {
  _cachedGithubSyncDatabasePath = databasePath;
  _cachedGithubSyncSettings = Map<String, Object?>.from(settings);
}

@visibleForTesting
void clearGithubSyncSettingsCacheForTesting() {
  _cachedGithubSyncDatabasePath = null;
  _cachedGithubSyncSettings = null;
}

void _cacheGithubSyncSetting(String databasePath, String key, Object? value) {
  if (_cachedGithubSyncDatabasePath == databasePath) {
    _cachedGithubSyncSettings?[key] = value;
  }
}

extension on _SyncProvider {
  String get label => switch (this) {
    _SyncProvider.githubRepository => 'GitHub Repository',
    _SyncProvider.githubGist => 'GitHub Gist',
    _SyncProvider.googleDrive => 'Google Drive',
    _SyncProvider.oneDrive => 'OneDrive',
    _SyncProvider.webdav => 'WebDAV',
    _SyncProvider.s3Compatible => 'S3',
  };

  IconData get icon => switch (this) {
    _SyncProvider.githubRepository => LucideIcons.gitBranch,
    _SyncProvider.githubGist => LucideIcons.fileCode2,
    _SyncProvider.googleDrive => LucideIcons.triangle,
    _SyncProvider.oneDrive => LucideIcons.cloud,
    _SyncProvider.webdav => LucideIcons.server,
    _SyncProvider.s3Compatible => LucideIcons.database,
  };
}

Future<Map<String, Object?>> _loadGithubSyncSettingsInBackground(
  String databasePath,
) async {
  final cached = _cachedGithubSyncSettings;
  if (cached != null && _cachedGithubSyncDatabasePath == databasePath) {
    return Future.value(Map<String, Object?>.from(cached));
  }
  final configStore = NautermConfigStore(NautermPaths.resolve());
  final syncConfig = await configStore.loadSyncConfig();
  final result = await Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return <String, Object?>{
        'config': store.githubLoadConfig(),
        'gistConfig': store.githubGistLoadConfig(),
        'hasToken': store.githubHasToken(),
        'hasGistToken': store.githubGistHasToken(),
        's3Config': store.s3LoadConfig(),
        'hasS3Credentials': store.s3HasCredentials(),
        'cloudProviders': store.cloudListProviders(),
        'syncPreferences': store.syncPreferences(),
        'hasLocalSyncKey': store.hasLocalSyncKey(),
      };
    } finally {
      store.dispose();
    }
  });
  result['syncConfig'] = syncConfig.toJson();
  _cachedGithubSyncDatabasePath = databasePath;
  _cachedGithubSyncSettings = Map<String, Object?>.from(result);
  return result;
}

Future<Map<String, dynamic>> _loadSyncStatusInBackground(String databasePath) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.syncPreferences();
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, dynamic>> _refreshRemoteSyncStatusInBackground(
  String databasePath,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.refreshRemoteSyncStatus();
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, String>> _loadBuiltInProviderCredentialsInBackground(
  String databasePath,
  _SyncProvider provider,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      if (provider == _SyncProvider.githubRepository) {
        final token = store.githubReadToken();
        return token == null
            ? const <String, String>{}
            : <String, String>{'token': token};
      }
      if (provider == _SyncProvider.s3Compatible) {
        return store.s3ReadCredentials() ?? const <String, String>{};
      }
      return const <String, String>{};
    } finally {
      store.dispose();
    }
  });
}

Future<void> _forgetSyncKeyInBackground(String databasePath) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      store.forgetSyncKey();
    } finally {
      store.dispose();
    }
  });
}

Future<List<Map<String, dynamic>>> _loadLocalSyncBackupsInBackground(
  String databasePath,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.localSyncBackups();
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, dynamic>> _restoreLocalSyncBackupInBackground(
  String databasePath,
  String backupId,
  int backupCount,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.restoreLocalSyncBackup(backupId, backupCount: backupCount);
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, dynamic>> _saveSyncPreferencesInBackground(
  String databasePath, {
  required String? activeProviderId,
  required String strategy,
  required bool autoSync,
  required int autoSyncMinutes,
  required int backupCount,
}) async {
  final saveDatabase = Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.saveSyncPreferences(activeProviderId: activeProviderId);
    } finally {
      store.dispose();
    }
  });
  final saveConfig = NautermConfigStore(NautermPaths.resolve()).saveSyncConfig(
    NautermSyncConfig(
      mergeStrategy: strategy,
      automatic: autoSync,
      interval: autoSyncMinutes * 60000,
      backupCount: backupCount,
    ),
  );
  final results = await Future.wait<Object?>([saveDatabase, saveConfig]);
  return Map<String, dynamic>.from(results.first! as Map);
}

Future<void> _saveGithubTokenInBackground(
  String databasePath,
  String token,
) async {
  await Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      store.githubSaveToken(token);
    } finally {
      store.dispose();
    }
  });
  _cacheGithubSyncSetting(databasePath, 'hasToken', true);
}

Future<void> _removeGithubTokenInBackground(String databasePath) async {
  await Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      store.githubDeleteToken();
    } finally {
      store.dispose();
    }
  });
  _cacheGithubSyncSetting(databasePath, 'hasToken', false);
}

Future<void> _saveGithubGistTokenInBackground(
  String databasePath,
  String token,
) async {
  await Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      store.githubGistSaveToken(token);
    } finally {
      store.dispose();
    }
  });
  _cacheGithubSyncSetting(databasePath, 'hasGistToken', true);
}

Future<void> _removeGithubGistTokenInBackground(String databasePath) async {
  await Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      store.githubGistDeleteToken();
    } finally {
      store.dispose();
    }
  });
  _cacheGithubSyncSetting(databasePath, 'hasGistToken', false);
}

Future<void> _saveGithubGistConfigInBackground({
  required String databasePath,
  required String gistId,
}) async {
  await Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      store.githubGistSaveConfig(gistId: gistId);
    } finally {
      store.dispose();
    }
  });
  _cacheGithubSyncSetting(databasePath, 'gistConfig', <String, dynamic>{
    'gist_id': gistId,
  });
}

Future<Map<String, dynamic>> _runGithubGistSyncInBackground(
  String databasePath,
  String? masterKey,
  String strategy,
  int backupCount,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.githubGistSync(
        masterKey: masterKey,
        strategy: strategy,
        backupCount: backupCount,
      );
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, dynamic>> _changeGithubGistMasterKeyInBackground(
  String databasePath, {
  required String currentMasterKey,
  required String newMasterKey,
}) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.githubGistChangeMasterKey(
        currentMasterKey: currentMasterKey,
        newMasterKey: newMasterKey,
      );
    } finally {
      store.dispose();
    }
  });
}

Future<List<Map<String, dynamic>>> _loadGithubGistHistoryInBackground(
  String databasePath,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.githubGistListHistory();
    } finally {
      store.dispose();
    }
  });
}

Future<void> _saveGithubConfigInBackground({
  required String databasePath,
  required String repositoryUrl,
  required String branch,
  required String path,
}) async {
  await Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      store.githubSaveConfig(
        repositoryUrl: repositoryUrl,
        branch: branch,
        path: path,
      );
    } finally {
      store.dispose();
    }
  });
  _cacheGithubSyncSetting(databasePath, 'config', <String, dynamic>{
    'repository_url': repositoryUrl,
    'branch': branch,
    'path': path,
  });
}

@visibleForTesting
Future<void> saveGithubConfigInBackgroundForTesting({
  required String databasePath,
  required String repositoryUrl,
  required String branch,
  required String path,
}) {
  return _saveGithubConfigInBackground(
    databasePath: databasePath,
    repositoryUrl: repositoryUrl,
    branch: branch,
    path: path,
  );
}

Future<Map<String, dynamic>> _runGithubSyncInBackground(
  String databasePath,
  String? masterKey,
  String strategy,
  int backupCount,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.githubSync(
        masterKey: masterKey,
        strategy: strategy,
        backupCount: backupCount,
      );
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, dynamic>> _changeGithubMasterKeyInBackground(
  String databasePath, {
  required String currentMasterKey,
  required String newMasterKey,
}) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.githubChangeMasterKey(
        currentMasterKey: currentMasterKey,
        newMasterKey: newMasterKey,
      );
    } finally {
      store.dispose();
    }
  });
}

Future<void> _saveS3SettingsInBackground({
  required String databasePath,
  required String endpoint,
  required String region,
  required String bucket,
  required String prefix,
  required String filename,
  String? accessKeyId,
  String? secretAccessKey,
}) async {
  await Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      store.s3SaveConfig(
        endpoint: endpoint,
        region: region,
        bucket: bucket,
        prefix: prefix,
        filename: filename,
      );
      if (accessKeyId != null && secretAccessKey != null) {
        store.s3SaveCredentials(
          accessKeyId: accessKeyId,
          secretAccessKey: secretAccessKey,
        );
      }
    } finally {
      store.dispose();
    }
  });
  _cacheGithubSyncSetting(databasePath, 's3Config', <String, dynamic>{
    'endpoint': endpoint,
    'region': region,
    'bucket': bucket,
    'prefix': prefix,
    'filename': filename,
  });
  if (accessKeyId != null) {
    _cacheGithubSyncSetting(databasePath, 'hasS3Credentials', true);
  }
}

Future<void> _removeS3SettingsInBackground(String databasePath) async {
  await Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      store.s3DeleteCredentials();
      store.s3DeleteConfig();
    } finally {
      store.dispose();
    }
  });
  _cacheGithubSyncSetting(databasePath, 's3Config', null);
  _cacheGithubSyncSetting(databasePath, 'hasS3Credentials', false);
}

Future<Map<String, dynamic>> _runS3SyncInBackground(
  String databasePath,
  String? masterKey,
  String strategy,
  int backupCount,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.s3Sync(
        masterKey: masterKey,
        strategy: strategy,
        backupCount: backupCount,
      );
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, dynamic>> _changeS3MasterKeyInBackground(
  String databasePath, {
  required String currentMasterKey,
  required String newMasterKey,
}) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.s3ChangeMasterKey(
        currentMasterKey: currentMasterKey,
        newMasterKey: newMasterKey,
      );
    } finally {
      store.dispose();
    }
  });
}

Future<List<Map<String, dynamic>>> _loadGithubSyncHistoryInBackground(
  String databasePath,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.githubListHistory();
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, dynamic>> _restoreGithubRevisionInBackground(
  String databasePath,
  String commitSha,
  int backupCount,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.githubRestoreRevision(commitSha, backupCount: backupCount);
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, dynamic>> _restoreGithubGistVersionInBackground(
  String databasePath,
  String version,
  int backupCount,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.githubGistRestoreVersion(version, backupCount: backupCount);
    } finally {
      store.dispose();
    }
  });
}

Widget _buildSettingsSyncContent(_SettingsPanelState state) {
  if (!state._hasLocalSyncKey && !state._syncManagementUnlocked) {
    return _buildSettingsMasterKeyContent(state);
  }
  final connectedCount = state._activeSyncProviderConnectedCount;
  return Scrollbar(
    controller: state._contentScrollController,
    thumbVisibility: true,
    child: ListView(
      controller: state._contentScrollController,
      padding: state._contentPadding,
      children: [
        Text(
          tr('settings.pages.sync.title', fallback: 'Sync & Backup'),
          style: TextStyle(
            color: _text,
            fontSize: 20,
            height: 1.4,
            fontWeight: NautermFontWeights.semibold,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 3),
        Text(
          tr(
            'Keep one active encrypted backup provider and control how changes are reconciled.',
          ),
          style: TextStyle(
            color: _mutedText,
            fontSize: 12,
            fontWeight: NautermFontWeights.regular,
            letterSpacing: 0,
          ),
        ),
        SizedBox(height: 24),
        ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: _settingsContentMaxWidth),
          child: _SyncOverviewHeader(
            connectedCount: connectedCount,
            hasLocalSyncKey: state._hasLocalSyncKey,
            onKeyDetails: state._showSyncKeyDetails,
          ),
        ),
        SizedBox(height: 18),
        ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: _settingsContentMaxWidth),
          child: _SyncPreferencesPanel(state: state),
        ),
        SizedBox(height: 14),
        ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: _settingsContentMaxWidth),
          child: Column(
            children: [
              LayoutBuilder(
                builder: (context, constraints) {
                  const columnCount = 3;
                  const spacing = 9.0;
                  final cardWidth =
                      (constraints.maxWidth - spacing * (columnCount - 1)) /
                      columnCount;
                  return Align(
                    alignment: Alignment.centerLeft,
                    child: Wrap(
                      spacing: spacing,
                      runSpacing: spacing,
                      children: [
                        for (final provider
                            in state._visibleBuiltInSyncProviders)
                          SizedBox(
                            width: cardWidth,
                            child: _SyncProviderRow(
                              key: ValueKey(
                                'settings-sync-provider-${provider.name}',
                              ),
                              provider: provider,
                              connected: state._isSyncProviderConnected(
                                provider,
                              ),
                              active:
                                  state._activeSyncProviderId ==
                                  state._builtInProviderId(provider),
                              busy: state._githubSyncRunning,
                              onOpen: () => state._openSyncProvider(provider),
                              onActivate: () =>
                                  state._activateBuiltInProvider(provider),
                              onSync: state._isSyncProviderConnected(provider)
                                  ? switch (provider) {
                                      _SyncProvider.githubRepository =>
                                        state._runGithubSync,
                                      _SyncProvider.githubGist =>
                                        state._runGithubGistSync,
                                      _SyncProvider.s3Compatible =>
                                        state._runS3Sync,
                                      _ => null,
                                    }
                                  : null,
                              onHistory:
                                  state._isSyncProviderConnected(provider)
                                  ? switch (provider) {
                                      _SyncProvider.githubRepository =>
                                        state._showGithubSyncHistory,
                                      _SyncProvider.githubGist =>
                                        state._showGithubGistHistory,
                                      _SyncProvider.s3Compatible =>
                                        state._showS3History,
                                      _ => null,
                                    }
                                  : null,
                            ),
                          ),
                        for (final provider in state._cloudProviders)
                          SizedBox(
                            width: cardWidth,
                            child: _CloudProviderRow(
                              key: ValueKey(
                                'settings-cloud-provider-${provider.id}',
                              ),
                              provider: provider,
                              busy: state._githubSyncRunning,
                              active:
                                  state._activeSyncProviderId ==
                                  'cloud:${provider.id}',
                              onOpen: () => state._editCloudProvider(
                                definition: provider.definition,
                                provider: provider,
                              ),
                              onSync: () =>
                                  state._runCloudProviderSync(provider),
                              onHistory: provider.supportsHistory
                                  ? () => state._showCloudProviderHistory(
                                      provider,
                                    )
                                  : null,
                              onActivate: () =>
                                  state._activateCloudProvider(provider),
                            ),
                          ),
                      ],
                    ),
                  );
                },
              ),
              SizedBox(height: 9),
              Align(
                alignment: Alignment.centerRight,
                child: _SettingsOutlineButton(
                  label: 'Add Provider',
                  onTap: state._githubSyncRunning
                      ? null
                      : state._showAddSyncProviderCatalog,
                ),
              ),
            ],
          ),
        ),
        if (state._githubSyncStatus != null) ...[
          SizedBox(height: 12),
          ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: _SyncStatusMessage(
              message: state._githubSyncStatus!,
              isError: state._githubSyncStatusIsError,
            ),
          ),
        ],
      ],
    ),
  );
}

Widget _buildGithubAuthenticationSettings(_SettingsPanelState state) {
  return _SettingsSection(
    icon: LucideIcons.badgeCheck,
    title: 'GitHub Authentication',
    child: Column(
      children: [
        _SettingsRow(
          title: 'Access Token',
          subtitle: 'Requires repository Contents: Read and write. Stored in the encrypted local database.',
          showSubtitle: false,
          trailing: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _SettingsTextField(
                controller: state._githubTokenController,
                obscureText: true,
                revealable: true,
                hint: 'github_pat_...',
                onChanged: (_) => state._mutate(() {}),
              ),
              SizedBox(height: 8),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  if (state._githubHasToken) ...[
                    _SettingsOutlineButton(
                      label: 'Disconnect',
                      onTap: state._githubSyncRunning
                          ? null
                          : state._removeGithubToken,
                    ),
                    SizedBox(width: 8),
                  ],
                  _SettingsOutlineButton(
                    label: state._githubHasToken
                        ? 'Update Authentication'
                        : 'Authenticate',
                    onTap: state._githubSyncRunning
                        ? null
                        : state._saveGithubToken,
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    ),
  );
}

Widget _buildGithubGistAuthorizationSettings(_SettingsPanelState state) {
  return _SettingsSection(
    icon: LucideIcons.badgeCheck,
    title: 'GitHub Authorization',
    child: _SettingsRow(
      title: state._githubGistHasToken ? 'GitHub connected' : 'Not connected',
      subtitle: state._githubGistHasToken
          ? 'Authorized with the gist and read:user scopes.'
          : 'Authorize Nauterm with GitHub Device Flow. No token needs to be pasted.',
      showSubtitle: false,
      trailing: Row(
        mainAxisAlignment: MainAxisAlignment.end,
        children: [
          if (state._githubGistHasToken) ...[
            _SettingsOutlineButton(
              label: 'Disconnect',
              onTap: state._removeGithubGistToken,
            ),
            SizedBox(width: 8),
          ],
          _SettingsOutlineButton(
            label: state._githubGistHasToken ? 'Reconnect' : 'Connect GitHub',
            onTap: state._startGithubGistDeviceFlow,
          ),
        ],
      ),
    ),
  );
}

Widget _buildGithubRepositorySettings(_SettingsPanelState state) {
  return _SettingsSection(
    icon: LucideIcons.gitBranch,
    title: 'Repository',
    child: Column(
      children: [
        _SettingsRow(
          title: 'Repository URL',
          subtitle: 'HTTPS or SSH Git repository URL.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._githubRepositoryUrlController,
            hint: 'https://github.com/yourname/nauterm-sync.git',
            onChanged: (_) => state._mutate(() {}),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          title: 'Branch',
          subtitle: 'The branch that stores encrypted revision history.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._githubBranchController,
            hint: 'main',
            onChanged: (_) => state._mutate(() {}),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          title: 'Path',
          subtitle: 'File path inside the repository.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._githubPathController,
            hint: 'nauterm-sync.enc',
            onChanged: (_) => state._mutate(() {}),
          ),
        ),
        SizedBox(height: 18),
        Align(
          alignment: Alignment.centerRight,
          child: _SettingsOutlineButton(
            label: 'Save Repository',
            onTap: state._githubSyncRunning ? null : state._saveGithubConfig,
          ),
        ),
      ],
    ),
  );
}

Widget _buildGithubGistSettings(_SettingsPanelState state) {
  return _SettingsSection(
    icon: LucideIcons.fileCode2,
    title: 'Gist',
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: _settingsContentMaxWidth),
      child: Column(
        children: [
          _SettingsRow(
            title: 'Gist ID',
            subtitle:
                'Leave empty to create a new private gist on the first sync.',
            showSubtitle: false,
            trailing: _SettingsTextField(
              controller: state._githubGistIdController,
              hint: 'Create automatically',
              onChanged: (_) => state._mutate(() {}),
            ),
          ),
          SizedBox(height: 18),
          _SettingsRow(
            title: 'Filename',
            subtitle: 'Encrypted sync payload inside the gist.',
            showSubtitle: false,
            trailing: Align(
              alignment: Alignment.centerLeft,
              child: Text(
                'nauterm-sync.enc',
                style: TextStyle(
                  color: _text,
                  fontSize: 13,
                  fontWeight: NautermFontWeights.medium,
                ),
              ),
            ),
          ),
          SizedBox(height: 18),
          Align(
            alignment: Alignment.centerRight,
            child: _SettingsOutlineButton(
              label: 'Save Gist Settings',
              onTap: state._githubSyncRunning
                  ? null
                  : state._saveGithubGistConfig,
            ),
          ),
        ],
      ),
    ),
  );
}

class _SyncOverviewHeader extends StatelessWidget {
  const _SyncOverviewHeader({
    required this.connectedCount,
    required this.hasLocalSyncKey,
    required this.onKeyDetails,
  });

  final int connectedCount;
  final bool hasLocalSyncKey;
  final VoidCallback onKeyDetails;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Container(
          width: 42,
          height: 42,
          decoration: BoxDecoration(
            color: _secondary.withValues(alpha: 0.1),
            shape: BoxShape.circle,
          ),
          alignment: Alignment.center,
          child: Icon(LucideIcons.shieldCheck, size: 19, color: _secondary),
        ),
        SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    hasLocalSyncKey
                        ? tr(
                            'settings.sync.status.ready',
                            fallback: 'Sync ready',
                          )
                        : tr(
                            'settings.sync.status.unlocked',
                            fallback: 'Sync unlocked',
                          ),
                    style: TextStyle(
                      color: _text,
                      fontSize: 14,
                      fontWeight: NautermFontWeights.semibold,
                    ),
                  ),
                  SizedBox(width: 7),
                  Container(
                    width: 7,
                    height: 7,
                    decoration: BoxDecoration(
                      color: _secondary,
                      shape: BoxShape.circle,
                    ),
                  ),
                ],
              ),
              SizedBox(height: 3),
              Text(
                tr(
                  connectedCount == 1
                      ? 'settings.sync.status.connectedProvider'
                      : 'settings.sync.status.connectedProviders',
                  fallback: connectedCount == 1
                      ? '{count} provider connected'
                      : '{count} providers connected',
                  args: {'count': connectedCount},
                ),
                style: TextStyle(
                  color: _mutedText,
                  fontSize: 11,
                  fontWeight: NautermFontWeights.regular,
                ),
              ),
            ],
          ),
        ),
        _SyncHeaderAction(
          icon: LucideIcons.keyRound,
          label: 'Key details',
          onTap: onKeyDetails,
        ),
      ],
    );
  }
}

class _SyncHeaderAction extends StatelessWidget {
  const _SyncHeaderAction({
    required this.icon,
    required this.label,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return TextButton.icon(
      onPressed: onTap,
      style: TextButton.styleFrom(
        foregroundColor: _text,
        padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 7),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(7)),
        textStyle: TextStyle(
          fontSize: 12,
          fontWeight: NautermFontWeights.medium,
        ),
      ),
      icon: Icon(icon, size: 14),
      label: Text(tr(label)),
    );
  }
}

class _SyncProviderRow extends StatelessWidget {
  const _SyncProviderRow({
    super.key,
    required this.provider,
    required this.connected,
    required this.active,
    required this.busy,
    required this.onOpen,
    required this.onActivate,
    this.onSync,
    this.onHistory,
  });

  final _SyncProvider provider;
  final bool connected;
  final bool active;
  final bool busy;
  final VoidCallback onOpen;
  final VoidCallback onActivate;
  final Future<void> Function()? onSync;
  final Future<void> Function()? onHistory;

  @override
  Widget build(BuildContext context) {
    final statusKey = active
        ? 'settings.sync.provider.status.connected'
        : connected
        ? 'settings.sync.provider.status.ready'
        : 'settings.sync.provider.status.notConfigured';
    final statusFallback = active
        ? 'Connected'
        : connected
        ? 'Ready'
        : 'Not configured';
    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: onOpen,
        borderRadius: BorderRadius.circular(10),
        child: ConstrainedBox(
          constraints: const BoxConstraints(minHeight: 72),
          child: Ink(
            padding: const EdgeInsets.fromLTRB(12, 10, 10, 10),
            decoration: BoxDecoration(
              color: active
                  ? _primary.withValues(alpha: _settingsDark ? 0.13 : 0.06)
                  : _surface,
              borderRadius: BorderRadius.circular(10),
              border: Border.all(
                color: active ? _primary.withValues(alpha: 0.78) : _softOutline,
                width: active ? 1.5 : 1,
              ),
              boxShadow: active
                  ? [
                      BoxShadow(
                        color: _primary.withValues(alpha: 0.1),
                        blurRadius: 12,
                        offset: const Offset(0, 3),
                      ),
                    ]
                  : null,
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Row(
                  children: [
                    Container(
                      width: 36,
                      height: 36,
                      decoration: BoxDecoration(
                        color: active
                            ? _primary.withValues(alpha: 0.09)
                            : _surfaceContainer,
                        borderRadius: BorderRadius.circular(9),
                      ),
                      alignment: Alignment.center,
                      child: Icon(
                        provider.icon,
                        size: 18,
                        color: active ? _primary : _mutedText,
                      ),
                    ),
                    SizedBox(width: 10),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              Flexible(
                                child: Text(
                                  provider.label,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: TextStyle(
                                    color: _text,
                                    fontSize: 13,
                                    fontWeight: NautermFontWeights.semibold,
                                  ),
                                ),
                              ),
                              SizedBox(width: 7),
                              _SyncProviderStatusDot(
                                active: active,
                                ready: connected,
                              ),
                            ],
                          ),
                          SizedBox(height: 3),
                          Text(
                            tr(statusKey, fallback: statusFallback),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: _mutedText,
                              fontSize: 11,
                              fontWeight: NautermFontWeights.regular,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
                SizedBox(height: 9),
                Align(
                  alignment: Alignment.centerRight,
                  child: Wrap(
                    spacing: 4,
                    runSpacing: 4,
                    children: [
                      if (active && onSync != null)
                        _SyncRowAction(
                          icon: LucideIcons.refreshCw,
                          label: busy ? 'Syncing…' : 'Sync',
                          onTap: busy ? null : () => onSync!(),
                        ),
                      if (active && onHistory != null)
                        _SyncRowAction(
                          icon: LucideIcons.history,
                          label: 'History',
                          onTap: () => onHistory!(),
                        ),
                      if (!active)
                        _SettingsOutlineButton(
                          label: 'Activate',
                          onTap: connected ? onActivate : null,
                        ),
                      _SyncRowAction(
                        icon: LucideIcons.settings,
                        label: 'Settings',
                        onTap: onOpen,
                      ),
                    ],
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

class _SyncProviderStatusDot extends StatelessWidget {
  const _SyncProviderStatusDot({required this.active, required this.ready});

  final bool active;
  final bool ready;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 6,
      height: 6,
      decoration: BoxDecoration(
        color: active
            ? _secondary
            : ready
            ? _primary
            : _faintText,
        shape: BoxShape.circle,
      ),
    );
  }
}

class _SyncRowAction extends StatelessWidget {
  const _SyncRowAction({
    required this.icon,
    required this.label,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: label,
      child: SizedBox.square(
        dimension: 28,
        child: IconButton(
          onPressed: onTap,
          padding: EdgeInsets.zero,
          constraints: const BoxConstraints.tightFor(width: 28, height: 28),
          style: IconButton.styleFrom(
            foregroundColor: _text,
            disabledForegroundColor: _faintText,
            hoverColor: _text.withValues(alpha: 0.07),
            highlightColor: _text.withValues(alpha: 0.1),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(7),
            ),
          ),
          icon: Icon(icon, size: 14),
        ),
      ),
    );
  }
}

class _SyncPreferencesPanel extends StatelessWidget {
  const _SyncPreferencesPanel({required this.state});

  final _SettingsPanelState state;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: _surface,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: _softOutline),
      ),
      child: Column(
        children: [
          _SettingsRow(
            localizationKey: 'settings.sync.mergeStrategy',
            title: 'Merge Strategy',
            subtitle: 'Choose how local and cloud changes are reconciled.',
            trailing: _SettingsSelect(
              label: tr(
                'settings.sync.mergeStrategy.label',
                fallback: 'Merge Strategy',
              ),
              showLabel: false,
              value: state._syncStrategy.storageValue,
              values: _SyncStrategy.values
                  .map((value) => value.storageValue)
                  .toList(growable: false),
              format: (value) => _SyncStrategy.values
                  .firstWhere((item) => item.storageValue == value)
                  .label,
              onChanged: (value) {
                state._mutate(() {
                  state._syncStrategy = _SyncStrategy.values.firstWhere(
                    (item) => item.storageValue == value,
                  );
                });
                unawaited(state._persistSyncPreferences());
              },
            ),
          ),
          SizedBox(height: 14),
          _SettingsRow(
            localizationKey: 'settings.sync.automatic',
            title: 'Automatic Sync',
            subtitle: 'Sync the active provider on the selected interval.',
            trailing: Row(
              children: [
                Expanded(
                  child: _SettingsTextField(
                    controller: state._autoSyncMinutesController,
                    hint: '${nautermDefaultSyncIntervalMilliseconds ~/ 60000}',
                    suffixText: tr(
                      'settings.sync.automatic.unit',
                      fallback: 'minutes',
                    ),
                    onChanged: (_) =>
                        state._scheduleSyncPreferencesSave(wakeAutoSync: true),
                    onSubmitted: (_) =>
                        state._persistSyncPreferences(wakeAutoSync: true),
                  ),
                ),
                SizedBox(width: 8),
                _SettingsSwitch(
                  value: state._autoSync,
                  onChanged: (value) {
                    state._mutate(() => state._autoSync = value);
                    unawaited(
                      state._persistSyncPreferences(wakeAutoSync: true),
                    );
                  },
                ),
              ],
            ),
          ),
          SizedBox(height: 14),
          _SettingsRow(
            title: 'Local Backups',
            subtitle: 'Create an encrypted backup before each sync.',
            trailing: Row(
              children: [
                Expanded(
                  child: _SettingsTextField(
                    controller: state._syncBackupCountController,
                    hint: '10 copies',
                    onChanged: (_) =>
                        state._scheduleSyncPreferencesSave(wakeAutoSync: false),
                    onSubmitted: (_) => state._persistSyncPreferences(),
                  ),
                ),
                SizedBox(width: 8),
                _SettingsOutlineButton(
                  label: 'View',
                  onTap: state._showLocalSyncBackups,
                ),
              ],
            ),
          ),
          SizedBox(height: 14),
          Row(
            children: [
              Expanded(
                child: _SyncVersionValue(
                  label: 'Local Version',
                  value: state._syncRevision,
                  snapshotId: state._syncSnapshotId,
                ),
              ),
              SizedBox(width: 8),
              Expanded(
                child: _SyncVersionValue(
                  label: 'Remote Version',
                  value: state._remoteSyncRevision,
                  snapshotId: state._remoteSyncSnapshotId,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _SyncVersionValue extends StatelessWidget {
  const _SyncVersionValue({
    required this.label,
    required this.value,
    required this.snapshotId,
  });

  final String label;
  final int? value;
  final String? snapshotId;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 11, vertical: 9),
      decoration: BoxDecoration(
        color: _surfaceContainer,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        children: [
          Text(tr(label), style: TextStyle(color: _mutedText, fontSize: 10)),
          const Spacer(),
          if (snapshotId != null && snapshotId!.isNotEmpty) ...[
            Text(
              snapshotId!.length > 8
                  ? snapshotId!.substring(0, 8)
                  : snapshotId!,
              style: TextStyle(color: _mutedText, fontSize: 10),
            ),
            const SizedBox(width: 8),
          ],
          Text(
            value == null ? '—' : 'v$value',
            style: TextStyle(
              color: _text,
              fontSize: 11,
              fontWeight: NautermFontWeights.semibold,
            ),
          ),
        ],
      ),
    );
  }
}

class _SyncStatusMessage extends StatelessWidget {
  const _SyncStatusMessage({required this.message, required this.isError});

  final String message;
  final bool isError;

  @override
  Widget build(BuildContext context) {
    final color = isError ? const Color(0xffdc2626) : _secondary;
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.07),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: color.withValues(alpha: 0.2)),
      ),
      child: Text(
        tr(message),
        style: TextStyle(
          color: color,
          fontSize: 11,
          height: 1.4,
          fontWeight: NautermFontWeights.medium,
        ),
      ),
    );
  }
}

Widget _buildOAuthSyncProviderSettings(
  _SettingsPanelState state, {
  required IconData icon,
  required String title,
  required String description,
  required String buttonLabel,
}) {
  return _SettingsSection(
    icon: icon,
    title: 'Authorization',
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: _settingsContentMaxWidth),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            description,
            style: TextStyle(
              color: _mutedText,
              fontSize: 12,
              height: 1.5,
              fontWeight: NautermFontWeights.regular,
            ),
          ),
          SizedBox(height: 18),
          Align(
            alignment: Alignment.centerRight,
            child: _SettingsOutlineButton(
              label: buttonLabel,
              onTap: () => state._showProviderConnectionRequirement(title),
            ),
          ),
        ],
      ),
    ),
  );
}

Widget _buildWebdavSettings(_SettingsPanelState state) {
  return _SettingsSection(
    icon: LucideIcons.server,
    title: 'WebDAV Connection',
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: _settingsContentMaxWidth),
      child: Column(
        children: [
          _SettingsRow(
            title: 'Server URL',
            subtitle: 'HTTPS endpoint of the WebDAV service.',
            showSubtitle: false,
            trailing: _SettingsTextField(
              controller: state._webdavUrlController,
              hint: 'https://dav.example.com/remote.php/dav/files/user',
              onChanged: (_) => state._mutate(() {}),
            ),
          ),
          SizedBox(height: 18),
          _SettingsRow(
            title: 'Username',
            subtitle: 'WebDAV account name.',
            showSubtitle: false,
            trailing: _SettingsTextField(
              controller: state._webdavUsernameController,
              hint: 'username',
              onChanged: (_) => state._mutate(() {}),
            ),
          ),
          SizedBox(height: 18),
          _SettingsRow(
            title: 'Password',
            subtitle: 'Password or application-specific password.',
            showSubtitle: false,
            trailing: _SettingsTextField(
              controller: state._webdavPasswordController,
              obscureText: true,
              revealable: true,
              hint: 'Password',
              onChanged: (_) => state._mutate(() {}),
            ),
          ),
          SizedBox(height: 18),
          _SettingsRow(
            title: 'Remote Path',
            subtitle: 'Path of the encrypted sync file.',
            showSubtitle: false,
            trailing: _SettingsTextField(
              controller: state._webdavPathController,
              hint: 'nauterm-sync.enc',
              onChanged: (_) => state._mutate(() {}),
            ),
          ),
        ],
      ),
    ),
  );
}

Widget _buildS3Settings(_SettingsPanelState state) {
  return ConstrainedBox(
    constraints: const BoxConstraints(maxWidth: _settingsContentMaxWidth),
    child: Column(
      children: [
        _SettingsRow(
          title: 'Endpoint',
          subtitle: 'HTTP or HTTPS S3 API endpoint.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._s3EndpointController,
            hint: 'https://s3.example.com',
            onChanged: (_) => state._mutate(() {}),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          title: 'Region',
          subtitle: 'Use auto when the provider discovers the region.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._s3RegionController,
            hint: 'auto',
            onChanged: (_) => state._mutate(() {}),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          title: 'Bucket',
          subtitle: 'Bucket containing the encrypted object.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._s3BucketController,
            hint: 'nauterm-sync',
            onChanged: (_) => state._mutate(() {}),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          title: 'Access Key ID',
          subtitle: 'S3 API credential.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._s3AccessKeyController,
            obscureText: true,
            revealable: true,
            hint: 'Access key',
            onChanged: (_) =>
                state._mutate(() => state._s3CredentialsDirty = true),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          title: 'Secret Access Key',
          subtitle: 'Stored only in the encrypted local database.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._s3SecretKeyController,
            obscureText: true,
            revealable: true,
            hint: 'Secret key',
            onChanged: (_) =>
                state._mutate(() => state._s3CredentialsDirty = true),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          title: 'Folder',
          subtitle: 'Optional folder path inside the bucket.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._s3PrefixController,
            hint: 'nauterm',
            onChanged: (_) => state._mutate(() {}),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          title: 'Filename',
          subtitle: 'Encrypted backup filename inside the prefix.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._s3FilenameController,
            hint: 'nauterm-sync.enc',
            onChanged: (_) => state._mutate(() {}),
          ),
        ),
        SizedBox(height: 18),
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            if (state._s3HasCredentials) ...[
              _SettingsOutlineButton(
                label: 'Disconnect',
                onTap: state._githubSyncRunning
                    ? null
                    : state._removeS3Settings,
              ),
              SizedBox(width: 8),
            ],
            _SettingsOutlineButton(
              label: state._s3HasCredentials ? 'Save Settings' : 'Connect S3',
              onTap: state._githubSyncRunning ? null : state._saveS3Settings,
            ),
          ],
        ),
      ],
    ),
  );
}

Widget _buildSettingsMasterKeyContent(_SettingsPanelState state) {
  return Scrollbar(
    controller: state._contentScrollController,
    thumbVisibility: true,
    child: ListView(
      controller: state._contentScrollController,
      padding: state._contentPadding,
      children: [
        Text(
          tr('common.label.masterKey', fallback: 'Master Key'),
          style: TextStyle(
            color: _text,
            fontSize: 20,
            height: 1.4,
            fontWeight: NautermFontWeights.semibold,
          ),
        ),
        SizedBox(height: 3),
        Text(
          tr(
            'settings.sync.masterKey.description',
            fallback: 'Use the same Master Key on every device. Nauterm uses it only to wrap or unwrap the random Sync DEK; it never stores the Master Key.',
          ),
          style: TextStyle(
            color: _mutedText,
            fontSize: 12,
            height: 1.5,
            fontWeight: NautermFontWeights.regular,
          ),
        ),
        SizedBox(height: 30),
        _SettingsSection(
          icon: LucideIcons.keyRound,
          title: 'Unlock Sync',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  title: 'Master Key',
                  subtitle: tr(
                    'settings.sync.masterKey.requirements',
                    fallback: 'Use at least 12 characters and include at least 3 of the 4 character types below.',
                  ),
                  trailing: _MasterKeyFieldWithStrength(
                    key: const ValueKey('settings-sync-master-key'),
                    controller: state._syncMasterKeyController,
                    hint: tr(
                      'settings.sync.masterKey.hint',
                      fallback: 'At least 12 characters',
                    ),
                    onChanged: (_) => state._mutate(() {}),
                  ),
                ),
                SizedBox(height: 14),
                _SettingsRow(
                  title: 'Confirm Master Key',
                  subtitle: 'Enter the same Master Key again.',
                  trailing: _SettingsTextField(
                    key: const ValueKey('settings-sync-master-key-confirm'),
                    controller: state._syncMasterKeyConfirmController,
                    obscureText: true,
                    hint: 'Repeat Master Key',
                    onChanged: (_) => state._mutate(() {}),
                    onSubmitted: (_) => state._continueToSyncManagement(),
                  ),
                ),
                SizedBox(height: 18),
                Align(
                  alignment: Alignment.centerRight,
                  child: _SettingsOutlineButton(
                    label: 'Continue',
                    onTap: state._continueToSyncManagement,
                  ),
                ),
                if (state._githubSyncStatusIsError &&
                    state._githubSyncStatus != null) ...[
                  SizedBox(height: 12),
                  Align(
                    alignment: Alignment.centerRight,
                    child: Text(
                      state._githubSyncStatus!,
                      style: TextStyle(
                        color: const Color(0xffdc2626),
                        fontSize: 12,
                        fontWeight: NautermFontWeights.medium,
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ],
    ),
  );
}

extension _SettingsSyncActions on _SettingsPanelState {
  String get _githubSyncDatabasePath => NautermPaths.resolve().databasePath;
  int get _syncBackupCount =>
      (int.tryParse(_syncBackupCountController.text.trim()) ?? 10).clamp(
        1,
        100,
      );

  Future<void> _refreshRemoteSyncStatus() async {
    if (_remoteSyncStatusRefreshing) return;
    _remoteSyncStatusRefreshing = true;
    try {
      await _loadGithubSyncSettings();
      if (!mounted || _selectedPage != _SettingsPage.sync) return;
      final preferences = await _refreshRemoteSyncStatusInBackground(
        _githubSyncDatabasePath,
      );
      if (!mounted) return;
      _mutate(() {
        _remoteSyncRevision = preferences['remote_revision'] as int?;
        _remoteSyncSnapshotId = preferences['remote_snapshot_id'] as String?;
      });
      _cacheGithubSyncSetting(
        _githubSyncDatabasePath,
        'syncPreferences',
        preferences,
      );
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'sync',
        'Unable to refresh remote sync status.',
        error: error,
        stackTrace: stackTrace,
      );
    } finally {
      _remoteSyncStatusRefreshing = false;
    }
  }

  void _rememberSyncSnapshot(int? revision, String? snapshotId) {
    if (revision == null) return;
    _syncRevision = revision;
    _syncSnapshotId = snapshotId;
    _remoteSyncRevision = revision;
    _remoteSyncSnapshotId = snapshotId;
    final cachedPreferences = Map<String, dynamic>.from(
      _cachedGithubSyncSettings?['syncPreferences'] as Map? ??
          const <String, dynamic>{},
    );
    _cacheGithubSyncSetting(
      _githubSyncDatabasePath,
      'syncPreferences',
      <String, dynamic>{
        ...cachedPreferences,
        'sync_snapshot': <String, dynamic>{
          'revision': revision,
          'snapshot_id': snapshotId ?? '',
        },
        'remote_revision': revision,
        'remote_snapshot_id': snapshotId,
      },
    );
  }

  String _builtInProviderId(_SyncProvider provider) => switch (provider) {
    _SyncProvider.githubRepository => 'github_repository',
    _SyncProvider.githubGist => 'github_gist',
    _SyncProvider.s3Compatible => 's3',
    _ => provider.name,
  };

  Future<void> _activateBuiltInProvider(_SyncProvider provider) =>
      _setActiveSyncProvider(_builtInProviderId(provider));

  Future<void> _activateCloudProvider(_CloudProviderInstance provider) =>
      _setActiveSyncProvider('cloud:${provider.id}');

  Future<void> _setActiveSyncProvider(String providerId) async {
    _mutate(() => _activeSyncProviderId = providerId);
    await _persistSyncPreferences(wakeAutoSync: true);
  }

  void _scheduleSyncPreferencesSave({required bool wakeAutoSync}) {
    _syncPreferencesSaveTimer?.cancel();
    _syncPreferencesSaveTimer = Timer(const Duration(milliseconds: 400), () {
      _syncPreferencesSaveTimer = null;
      unawaited(_persistSyncPreferences(wakeAutoSync: wakeAutoSync));
    });
  }

  Future<void> _persistSyncPreferences({bool wakeAutoSync = false}) async {
    _syncPreferencesSaveTimer?.cancel();
    _syncPreferencesSaveTimer = null;
    final minutes =
        int.tryParse(_autoSyncMinutesController.text.trim()) ??
        nautermDefaultSyncIntervalMilliseconds ~/ 60000;
    try {
      final persistedPreferences = await _saveSyncPreferencesInBackground(
        _githubSyncDatabasePath,
        activeProviderId: _activeSyncProviderId,
        strategy: _syncStrategy.storageValue,
        autoSync: _autoSync,
        autoSyncMinutes: minutes.clamp(5, 10080),
        backupCount: _syncBackupCount,
      );
      final remoteRevision = persistedPreferences['remote_revision'] as int?;
      final remoteSnapshotId =
          persistedPreferences['remote_snapshot_id'] as String?;
      _mutate(() {
        _remoteSyncRevision = remoteRevision;
        _remoteSyncSnapshotId = remoteSnapshotId;
      });
      _cacheGithubSyncSetting(
        _githubSyncDatabasePath,
        'syncPreferences',
        <String, dynamic>{
          'active_provider_id': _activeSyncProviderId,
          'sync_snapshot': _syncRevision == null
              ? null
              : <String, dynamic>{
                  'revision': _syncRevision,
                  'snapshot_id': _syncSnapshotId ?? '',
                },
          'remote_revision': remoteRevision,
          'remote_snapshot_id': remoteSnapshotId,
        },
      );
      _cacheGithubSyncSetting(
        _githubSyncDatabasePath,
        'syncConfig',
        <String, dynamic>{
          'mergeStrategy': _syncStrategy.storageValue,
          'automatic': _autoSync,
          'interval': minutes.clamp(5, 10080) * 60000,
          'backupCount': _syncBackupCount,
        },
      );
      if (wakeAutoSync) notifyNautermSyncPreferencesChanged();
    } on Object catch (error) {
      if (mounted) _setGithubSyncError(error.toString());
    }
  }

  void _applyCachedGithubSyncSettings() {
    final cached = _cachedGithubSyncSettings;
    if (cached == null ||
        _cachedGithubSyncDatabasePath != _githubSyncDatabasePath) {
      return;
    }
    _applyGithubSyncSettings(cached, notify: false);
  }

  void _applyGithubSyncSettings(
    Map<String, Object?> result, {
    required bool notify,
  }) {
    final cfg = result['config'] as Map<String, dynamic>?;
    final gistCfg = result['gistConfig'] as Map<String, dynamic>?;
    final s3Cfg = result['s3Config'] as Map<String, dynamic>?;
    final cloudProviders =
        (result['cloudProviders'] as List?)
            ?.map(
              (item) => _CloudProviderInstance.fromJson(
                Map<String, dynamic>.from(item as Map),
              ),
            )
            .toList(growable: false) ??
        const <_CloudProviderInstance>[];
    final preferences = Map<String, dynamic>.from(
      result['syncPreferences'] as Map? ?? const <String, dynamic>{},
    );
    final syncConfig = Map<String, dynamic>.from(
      result['syncConfig'] as Map? ?? const <String, dynamic>{},
    );
    void apply() {
      _githubHasToken = result['hasToken'] as bool? ?? false;
      _githubGistHasToken = result['hasGistToken'] as bool? ?? false;
      _s3HasCredentials = result['hasS3Credentials'] as bool? ?? false;
      _cloudProviders = cloudProviders;
      _activeSyncProviderId = preferences['active_provider_id'] as String?;
      _syncStrategy = switch (syncConfig['mergeStrategy'] as String?) {
        'local_wins' => _SyncStrategy.localWins,
        'remote_wins' => _SyncStrategy.remoteWins,
        _ => _SyncStrategy.smartMerge,
      };
      _autoSync = syncConfig['automatic'] as bool? ?? false;
      _autoSyncMinutesController.text =
          ((syncConfig['interval'] as int? ??
                      nautermDefaultSyncIntervalMilliseconds) ~/
                  60000)
              .toString();
      _syncBackupCountController.text =
          (syncConfig['backupCount'] as int? ?? 10).toString();
      final syncSnapshot = Map<String, dynamic>.from(
        preferences['sync_snapshot'] as Map? ?? const <String, dynamic>{},
      );
      _syncRevision = syncSnapshot['revision'] as int?;
      _syncSnapshotId = syncSnapshot['snapshot_id'] as String?;
      _remoteSyncRevision = preferences['remote_revision'] as int?;
      _remoteSyncSnapshotId = preferences['remote_snapshot_id'] as String?;
      _hasLocalSyncKey = result['hasLocalSyncKey'] as bool? ?? false;
      _syncManagementUnlocked = _hasLocalSyncKey;
      if (cfg != null) {
        _githubRepositoryUrlController.text =
            (cfg['repository_url'] as String?) ?? '';
        final branch = (cfg['branch'] as String?)?.trim();
        _githubBranchController.text = branch == null || branch.isEmpty
            ? 'main'
            : branch;
        final path = (cfg['path'] as String?)?.trim();
        _githubPathController.text = path == null || path.isEmpty
            ? 'nauterm-sync.enc'
            : path;
      }
      if (gistCfg != null) {
        _githubGistIdController.text = (gistCfg['gist_id'] as String?) ?? '';
      }
      if (s3Cfg != null) {
        _s3EndpointController.text = (s3Cfg['endpoint'] as String?) ?? '';
        final region = (s3Cfg['region'] as String?)?.trim();
        _s3RegionController.text = region == null || region.isEmpty
            ? 'auto'
            : region;
        _s3BucketController.text = (s3Cfg['bucket'] as String?) ?? '';
        _s3PrefixController.text = (s3Cfg['prefix'] as String?)?.trim() ?? '';
        final filename = (s3Cfg['filename'] as String?)?.trim();
        _s3FilenameController.text = filename == null || filename.isEmpty
            ? 'nauterm-sync.enc'
            : filename;
      }
    }

    if (notify) {
      _mutate(apply);
    } else {
      apply();
    }
  }

  int get _connectedSyncProviderCount =>
      _SyncProvider.values.where(_isSyncProviderConnected).length +
      _cloudProviders.where((provider) => provider.hasCredentials).length;

  int get _activeSyncProviderConnectedCount {
    final activeProviderId = _activeSyncProviderId;
    if (activeProviderId == null) {
      return 0;
    }
    for (final provider in _SyncProvider.values) {
      if (activeProviderId == _builtInProviderId(provider) &&
          _isSyncProviderConnected(provider)) {
        return 1;
      }
    }
    if (activeProviderId.startsWith('cloud:')) {
      final providerId = activeProviderId.substring('cloud:'.length);
      if (_cloudProviders.any(
        (provider) => provider.id == providerId && provider.hasCredentials,
      )) {
        return 1;
      }
    }
    return 0;
  }

  bool _isSyncProviderConnected(_SyncProvider provider) {
    return switch (provider) {
      _SyncProvider.githubRepository =>
        _githubHasToken &&
            _githubRepositoryUrlController.text.trim().isNotEmpty,
      _SyncProvider.githubGist => _githubGistHasToken,
      _SyncProvider.s3Compatible =>
        _s3HasCredentials &&
            _s3EndpointController.text.trim().isNotEmpty &&
            _s3BucketController.text.trim().isNotEmpty,
      _ => false,
    };
  }

  Future<void> _openSyncProvider(_SyncProvider provider) async {
    if (provider == _SyncProvider.githubGist && !_githubGistHasToken) {
      await _startGithubGistDeviceFlow();
      return;
    }
    if ((provider == _SyncProvider.githubRepository && _githubHasToken) ||
        (provider == _SyncProvider.s3Compatible && _s3HasCredentials)) {
      final credentials = await _loadBuiltInProviderCredentialsInBackground(
        _githubSyncDatabasePath,
        provider,
      );
      if (!mounted) return;
      if (provider == _SyncProvider.githubRepository) {
        _githubTokenController.text = credentials['token'] ?? '';
      } else {
        _s3AccessKeyController.text = credentials['access_key_id'] ?? '';
        _s3SecretKeyController.text = credentials['secret_access_key'] ?? '';
        _s3CredentialsDirty = false;
      }
    }
    await showNautermDialog<void>(
      context: context,
      builder: (_) => _SyncProviderDialog(provider: provider, state: this),
    );
    if (!mounted) return;
    if (provider == _SyncProvider.githubRepository) {
      _githubTokenController.clear();
    } else if (provider == _SyncProvider.s3Compatible) {
      _s3AccessKeyController.clear();
      _s3SecretKeyController.clear();
      _s3CredentialsDirty = false;
    }
  }

  Future<void> _showGithubSyncHistory() async {
    await _loadGithubSyncHistory();
    if (!mounted) return;
    await showNautermDialog<void>(
      context: context,
      builder: (_) => _SyncHistoryDialog(
        revisions: _githubSyncHistory,
        loading: _githubSyncHistoryLoading,
        onRestore: _restoreGithubRevision,
      ),
    );
  }

  Future<void> _showLocalSyncBackups() async {
    await showNautermDialog<void>(
      context: context,
      builder: (_) => _LocalSyncBackupsDialog(
        databasePath: _githubSyncDatabasePath,
        backupCount: _syncBackupCount,
        onRestored: () {
          notifyNautermDatabaseBulkChange();
          notifyNautermSyncCompleted();
          _mutate(() {
            _githubSyncStatus = 'Local backup restored.';
            _githubSyncStatusIsError = false;
          });
        },
      ),
    );
  }

  Future<void> _showGithubGistHistory() async {
    await _loadGithubGistHistory();
    if (!mounted) return;
    await showNautermDialog<void>(
      context: context,
      builder: (_) => _SyncHistoryDialog(
        revisions: _githubGistSyncHistory,
        loading: _githubGistSyncHistoryLoading,
        onRestore: _restoreGithubGistVersion,
      ),
    );
  }

  Future<void> _restoreGithubRevision(String commitSha) async {
    _mutate(() => _githubSyncRunning = true);
    try {
      final result = await _restoreGithubRevisionInBackground(
        _githubSyncDatabasePath,
        commitSha,
        _syncBackupCount,
      );
      if (!mounted) return;
      _mutate(() {
        _rememberSyncSnapshot(
          result['revision'] as int?,
          result['snapshot_id'] as String?,
        );
        _githubSyncStatus = 'GitHub revision restored as a new revision.';
        _githubSyncStatusIsError = false;
      });
      notifyNautermDatabaseBulkChange();
    } on Object catch (error) {
      if (mounted) _setGithubSyncError(error.toString());
      rethrow;
    } finally {
      if (mounted) _mutate(() => _githubSyncRunning = false);
    }
  }

  Future<void> _restoreGithubGistVersion(String version) async {
    _mutate(() => _githubSyncRunning = true);
    try {
      final result = await _restoreGithubGistVersionInBackground(
        _githubSyncDatabasePath,
        version,
        _syncBackupCount,
      );
      if (!mounted) return;
      _mutate(() {
        _rememberSyncSnapshot(
          result['revision'] as int?,
          result['snapshot_id'] as String?,
        );
        _githubSyncStatus = 'Gist revision restored as a new revision.';
        _githubSyncStatusIsError = false;
      });
      notifyNautermDatabaseBulkChange();
    } on Object catch (error) {
      if (mounted) _setGithubSyncError(error.toString());
      rethrow;
    } finally {
      if (mounted) _mutate(() => _githubSyncRunning = false);
    }
  }

  Future<void> _showSyncKeyDetails() async {
    final action = await showNautermDialog<_SyncKeyAction>(
      context: context,
      builder: (dialogContext) => _SyncKeyDetailsDialog(
        hasLocalSyncKey: _hasLocalSyncKey,
        canChangeMasterKey: _connectedSyncProviderCount > 0,
        onChangeMasterKey: () =>
            Navigator.of(dialogContext).pop(_SyncKeyAction.changeMasterKey),
        onForgetMasterKey: () =>
            Navigator.of(dialogContext).pop(_SyncKeyAction.forgetMasterKey),
      ),
    );
    if (action == null || !mounted) {
      return;
    }
    switch (action) {
      case _SyncKeyAction.changeMasterKey:
        await showNautermDialog<void>(
          context: context,
          builder: (_) =>
              _ChangeSyncMasterKeyDialog(onChange: _changeSyncMasterKey),
        );
      case _SyncKeyAction.forgetMasterKey:
        await _forgetSyncMasterKey();
    }
  }

  Future<void> _forgetSyncMasterKey() async {
    try {
      await _forgetSyncKeyInBackground(_githubSyncDatabasePath);
      if (!mounted) return;
      _mutate(() {
        _hasLocalSyncKey = false;
        _syncManagementUnlocked = false;
        _syncRevision = null;
        _syncSnapshotId = null;
        _githubSyncStatus = null;
        _githubSyncStatusIsError = false;
      });
      _cacheGithubSyncSetting(
        _githubSyncDatabasePath,
        'hasLocalSyncKey',
        false,
      );
      notifyNautermSyncCompleted();
    } on Object catch (error) {
      if (mounted) _setGithubSyncError(error.toString());
    }
  }

  Future<String?> _changeSyncMasterKey(
    String currentMasterKey,
    String newMasterKey,
  ) async {
    final providers = <_SyncProvider>[
      if (_isSyncProviderConnected(_SyncProvider.githubRepository))
        _SyncProvider.githubRepository,
      if (_isSyncProviderConnected(_SyncProvider.githubGist))
        _SyncProvider.githubGist,
      if (_isSyncProviderConnected(_SyncProvider.s3Compatible))
        _SyncProvider.s3Compatible,
    ];
    final cloudProviders = _cloudProviders
        .where((provider) => provider.hasCredentials)
        .toList(growable: false);
    if (providers.isEmpty && cloudProviders.isEmpty) {
      return 'Connect a sync provider before changing the Master Key.';
    }

    final completed = <String>[];
    try {
      for (final provider in providers) {
        switch (provider) {
          case _SyncProvider.githubRepository:
            await _changeGithubMasterKeyInBackground(
              _githubSyncDatabasePath,
              currentMasterKey: currentMasterKey,
              newMasterKey: newMasterKey,
            );
          case _SyncProvider.githubGist:
            await _changeGithubGistMasterKeyInBackground(
              _githubSyncDatabasePath,
              currentMasterKey: currentMasterKey,
              newMasterKey: newMasterKey,
            );
          case _SyncProvider.s3Compatible:
            await _changeS3MasterKeyInBackground(
              _githubSyncDatabasePath,
              currentMasterKey: currentMasterKey,
              newMasterKey: newMasterKey,
            );
          case _SyncProvider.googleDrive ||
              _SyncProvider.oneDrive ||
              _SyncProvider.webdav:
            continue;
        }
        completed.add(provider.label);
      }
      for (final provider in cloudProviders) {
        await _changeCloudProviderMasterKeyInBackground(
          _githubSyncDatabasePath,
          providerId: provider.id,
          currentMasterKey: currentMasterKey,
          newMasterKey: newMasterKey,
        );
        completed.add(provider.name);
      }
    } on Object catch (error) {
      final prefix = completed.isEmpty
          ? ''
          : '${completed.join(', ')} already uses the new Master Key. ';
      return '$prefix${error.toString()}';
    }

    if (mounted) {
      _mutate(() {
        _githubSyncStatus =
            'Master Key changed for ${completed.join(' and ')}.';
        _githubSyncStatusIsError = false;
      });
      unawaited(_loadGithubSyncHistory());
      unawaited(_loadGithubGistHistory());
    }
    return null;
  }

  Future<void> _loadGithubSyncSettings() async {
    try {
      final databasePath = _githubSyncDatabasePath;
      final result = await _loadGithubSyncSettingsInBackground(databasePath);
      if (!mounted) return;
      _applyGithubSyncSettings(result, notify: true);
    } on Object {
      return;
    }
  }

  Future<void> _saveGithubToken() async {
    final token = _githubTokenController.text.trim();
    if (token.isEmpty) {
      _setGithubSyncError('Enter a GitHub PAT to save.');
      return;
    }
    _clearGithubSyncStatus();
    try {
      final databasePath = _githubSyncDatabasePath;
      await _saveGithubTokenInBackground(databasePath, token);
      if (!mounted) return;
      _mutate(() {
        _githubHasToken = true;
        _githubSyncStatus =
            'GitHub token saved to the encrypted local database.';
        _githubSyncStatusIsError = false;
        _githubTokenController.clear();
      });
    } on Object catch (error) {
      if (!mounted) return;
      _setGithubSyncError(error.toString());
    }
  }

  Future<void> _removeGithubToken() async {
    try {
      final databasePath = _githubSyncDatabasePath;
      await _removeGithubTokenInBackground(databasePath);
      if (!mounted) return;
      _mutate(() {
        _githubHasToken = false;
        _githubTokenController.clear();
        _githubSyncStatus = 'GitHub token removed.';
        _githubSyncStatusIsError = false;
        if (_activeSyncProviderId == 'github_repository') {
          _activeSyncProviderId = null;
        }
      });
      await _persistSyncPreferences();
    } on Object catch (error) {
      if (!mounted) return;
      _setGithubSyncError(error.toString());
    }
  }

  Future<bool> _saveGithubGistToken(String token) async {
    try {
      await _saveGithubGistTokenInBackground(_githubSyncDatabasePath, token);
      if (!mounted) return false;
      _mutate(() {
        _githubGistHasToken = true;
        _githubSyncStatus = 'GitHub Gist authorization completed.';
        _githubSyncStatusIsError = false;
      });
      return true;
    } on Object catch (error) {
      if (mounted) {
        _setGithubSyncError(error.toString());
      }
      return false;
    }
  }

  Future<void> _removeGithubGistToken() async {
    try {
      await _removeGithubGistTokenInBackground(_githubSyncDatabasePath);
      if (!mounted) return;
      _mutate(() {
        _githubGistHasToken = false;
        _githubSyncStatus = 'GitHub Gist disconnected.';
        _githubSyncStatusIsError = false;
        if (_activeSyncProviderId == 'github_gist') {
          _activeSyncProviderId = null;
        }
      });
      await _persistSyncPreferences();
    } on Object catch (error) {
      if (mounted) {
        _setGithubSyncError(error.toString());
      }
    }
  }

  Future<void> _startGithubGistDeviceFlow() async {
    final gistId = await showNautermDialog<String>(
      context: context,
      barrierDismissible: true,
      builder: (_) => _GithubDeviceFlowDialog(
        initialGistId: _githubGistIdController.text.trim(),
        onAuthorized: _saveGithubGistToken,
      ),
    );
    if (!mounted || gistId == null) return;
    _githubGistIdController.text = gistId;
    await _saveGithubGistConfig();
  }

  Future<void> _saveGithubGistConfig() async {
    try {
      await _saveGithubGistConfigInBackground(
        databasePath: _githubSyncDatabasePath,
        gistId: _githubGistIdController.text.trim(),
      );
      if (!mounted) return;
      _mutate(() {
        _githubSyncStatus = 'GitHub Gist settings saved.';
        _githubSyncStatusIsError = false;
      });
    } on Object catch (error) {
      if (mounted) {
        _setGithubSyncError(error.toString());
      }
    }
  }

  Future<void> _saveGithubConfig() async {
    final repositoryUrl = _githubRepositoryUrlController.text.trim();
    var branch = _githubBranchController.text.trim();
    var path = _githubPathController.text.trim();
    if (repositoryUrl.isEmpty) {
      _setGithubSyncError('Repository URL is required.');
      return;
    }
    if (branch.isEmpty) branch = 'main';
    if (path.isEmpty) path = 'nauterm-sync.enc';
    _clearGithubSyncStatus();
    try {
      final databasePath = _githubSyncDatabasePath;
      await _saveGithubConfigInBackground(
        databasePath: databasePath,
        repositoryUrl: repositoryUrl,
        branch: branch,
        path: path,
      );
      if (!mounted) return;
      _mutate(() {
        _githubBranchController.text = branch;
        _githubPathController.text = path;
        _githubSyncStatus = 'Repository configuration saved.';
        _githubSyncStatusIsError = false;
      });
    } on Object catch (error) {
      if (!mounted) return;
      _setGithubSyncError(error.toString());
    }
  }

  Future<void> _saveS3Settings() async {
    final endpoint = _s3EndpointController.text.trim();
    var region = _s3RegionController.text.trim();
    final bucket = _s3BucketController.text.trim();
    final prefix = _s3PrefixController.text.trim();
    var filename = _s3FilenameController.text.trim();
    final accessKeyId = _s3AccessKeyController.text.trim();
    final secretAccessKey = _s3SecretKeyController.text.trim();
    if (endpoint.isEmpty) {
      _setGithubSyncError('S3 endpoint is required.');
      return;
    }
    if (bucket.isEmpty) {
      _setGithubSyncError('S3 bucket is required.');
      return;
    }
    if (region.isEmpty) region = 'auto';
    if (filename.isEmpty) filename = 'nauterm-sync.enc';
    final updatingCredentials =
        !_s3HasCredentials ||
        _s3CredentialsDirty ||
        accessKeyId.isNotEmpty ||
        secretAccessKey.isNotEmpty;
    if (updatingCredentials &&
        (accessKeyId.isEmpty || secretAccessKey.isEmpty)) {
      _setGithubSyncError(
        'Enter both the S3 Access Key ID and Secret Access Key.',
      );
      return;
    }
    _clearGithubSyncStatus();
    try {
      await _saveS3SettingsInBackground(
        databasePath: _githubSyncDatabasePath,
        endpoint: endpoint,
        region: region,
        bucket: bucket,
        prefix: prefix,
        filename: filename,
        accessKeyId: updatingCredentials ? accessKeyId : null,
        secretAccessKey: updatingCredentials ? secretAccessKey : null,
      );
      if (!mounted) return;
      _mutate(() {
        _s3HasCredentials = true;
        _s3CredentialsDirty = false;
        _s3RegionController.text = region;
        _s3PrefixController.text = prefix;
        _s3FilenameController.text = filename;
        _s3AccessKeyController.clear();
        _s3SecretKeyController.clear();
        _githubSyncStatus = 'S3-compatible storage connected.';
        _githubSyncStatusIsError = false;
      });
    } on Object catch (error) {
      if (mounted) {
        _setGithubSyncError(error.toString());
      }
    }
  }

  Future<void> _removeS3Settings() async {
    try {
      await _removeS3SettingsInBackground(_githubSyncDatabasePath);
      if (!mounted) return;
      _mutate(() {
        _s3HasCredentials = false;
        _s3CredentialsDirty = false;
        _s3EndpointController.clear();
        _s3RegionController.text = 'auto';
        _s3BucketController.clear();
        _s3AccessKeyController.clear();
        _s3SecretKeyController.clear();
        _s3PrefixController.clear();
        _s3FilenameController.text = 'nauterm-sync.enc';
        _githubSyncStatus = 'S3-compatible storage disconnected.';
        _githubSyncStatusIsError = false;
        if (_activeSyncProviderId == 's3') {
          _activeSyncProviderId = null;
        }
      });
      await _persistSyncPreferences();
    } on Object catch (error) {
      if (mounted) {
        _setGithubSyncError(error.toString());
      }
    }
  }

  Future<void> _runGithubSync() async {
    final masterKey = _syncMasterKeyController.text;
    final masterKeyConfirm = _syncMasterKeyConfirmController.text;
    final enteringMasterKey = masterKey.isNotEmpty;
    if (!enteringMasterKey && !_hasLocalSyncKey) {
      _setGithubSyncError('Enter the Master Key.');
      return;
    }
    final validationError = enteringMasterKey
        ? _masterKeyValidationError(masterKey)
        : null;
    if (validationError != null) {
      _setGithubSyncError(validationError);
      return;
    }
    if (enteringMasterKey && masterKeyConfirm.isEmpty) {
      _setGithubSyncError('Confirm the Master Key.');
      return;
    }
    if (enteringMasterKey && masterKey != masterKeyConfirm) {
      _setGithubSyncError('Master Key confirmation does not match.');
      return;
    }
    if (!_githubHasToken) {
      _setGithubSyncError('Save a GitHub token first.');
      return;
    }
    if (_githubRepositoryUrlController.text.trim().isEmpty) {
      _setGithubSyncError('Configure the repository first.');
      return;
    }
    _mutate(() {
      _githubSyncRunning = true;
      _githubSyncStatus = null;
      _githubSyncStatusIsError = false;
    });
    try {
      final databasePath = _githubSyncDatabasePath;
      final result = await _runGithubSyncInBackground(
        databasePath,
        enteringMasterKey ? masterKey : null,
        _syncStrategy.storageValue,
        _syncBackupCount,
      );
      if (!mounted) return;
      final commit = (result['commit_sha'] as String?) ?? '';
      final revision = result['revision'];
      final imported = result['imported_records'];
      final shortSha = commit.length > 7 ? commit.substring(0, 7) : commit;
      _mutate(() {
        _rememberSyncSnapshot(
          revision as int?,
          result['snapshot_id'] as String?,
        );
        if (enteringMasterKey) {
          _hasLocalSyncKey = true;
        }
        _githubSyncStatusIsError = false;
        _githubSyncStatus =
            'Sync complete · commit $shortSha · imported $imported · revision $revision.';
      });
      if (enteringMasterKey) {
        _cacheGithubSyncSetting(databasePath, 'hasLocalSyncKey', true);
      }
      notifyNautermDatabaseBulkChange();
      unawaited(_loadGithubSyncHistory());
    } on Object catch (error) {
      if (!mounted) return;
      _setGithubSyncError(error.toString());
    } finally {
      _syncMasterKeyController.clear();
      _syncMasterKeyConfirmController.clear();
      if (mounted) {
        _mutate(() => _githubSyncRunning = false);
      }
    }
  }

  Future<void> _runGithubGistSync() async {
    final masterKey = _syncMasterKeyController.text;
    final masterKeyConfirm = _syncMasterKeyConfirmController.text;
    final enteringMasterKey = masterKey.isNotEmpty;
    if (!enteringMasterKey && !_hasLocalSyncKey) {
      _setGithubSyncError('Enter the Master Key.');
      return;
    }
    final validationError = enteringMasterKey
        ? _masterKeyValidationError(masterKey)
        : null;
    if (validationError != null) {
      _setGithubSyncError(validationError);
      return;
    }
    if (enteringMasterKey && masterKeyConfirm.isEmpty) {
      _setGithubSyncError('Confirm the Master Key.');
      return;
    }
    if (enteringMasterKey && masterKey != masterKeyConfirm) {
      _setGithubSyncError('Master Key confirmation does not match.');
      return;
    }
    if (!_githubGistHasToken) {
      _setGithubSyncError('Connect GitHub Gist first.');
      return;
    }
    _mutate(() {
      _githubSyncRunning = true;
      _githubSyncStatus = null;
      _githubSyncStatusIsError = false;
    });
    try {
      await _saveGithubGistConfigInBackground(
        databasePath: _githubSyncDatabasePath,
        gistId: _githubGistIdController.text.trim(),
      );
      final result = await _runGithubGistSyncInBackground(
        _githubSyncDatabasePath,
        enteringMasterKey ? masterKey : null,
        _syncStrategy.storageValue,
        _syncBackupCount,
      );
      if (!mounted) return;
      final gistId = (result['gist_id'] as String?) ?? '';
      final version = (result['version'] as String?) ?? '';
      final shortVersion = version.length > 7
          ? version.substring(0, 7)
          : version;
      final revision = result['revision'];
      final imported = result['imported_records'];
      _mutate(() {
        _rememberSyncSnapshot(
          revision as int?,
          result['snapshot_id'] as String?,
        );
        if (enteringMasterKey) {
          _hasLocalSyncKey = true;
        }
        _githubGistIdController.text = gistId;
        _githubSyncStatusIsError = false;
        _githubSyncStatus =
            'Gist sync complete · revision $shortVersion · imported $imported · local revision $revision.';
      });
      _cacheGithubSyncSetting(
        _githubSyncDatabasePath,
        'gistConfig',
        <String, dynamic>{'gist_id': gistId},
      );
      if (enteringMasterKey) {
        _cacheGithubSyncSetting(
          _githubSyncDatabasePath,
          'hasLocalSyncKey',
          true,
        );
      }
      notifyNautermDatabaseBulkChange();
      unawaited(_loadGithubGistHistory());
    } on Object catch (error) {
      if (mounted) {
        _setGithubSyncError(error.toString());
      }
    } finally {
      _syncMasterKeyController.clear();
      _syncMasterKeyConfirmController.clear();
      if (mounted) {
        _mutate(() => _githubSyncRunning = false);
      }
    }
  }

  Future<void> _runS3Sync() async {
    final masterKey = _syncMasterKeyController.text;
    final masterKeyConfirm = _syncMasterKeyConfirmController.text;
    final enteringMasterKey = masterKey.isNotEmpty;
    if (!enteringMasterKey && !_hasLocalSyncKey) {
      _setGithubSyncError('Enter the Master Key.');
      return;
    }
    final validationError = enteringMasterKey
        ? _masterKeyValidationError(masterKey)
        : null;
    if (validationError != null) {
      _setGithubSyncError(validationError);
      return;
    }
    if (enteringMasterKey && masterKey != masterKeyConfirm) {
      _setGithubSyncError('Master Key confirmation does not match.');
      return;
    }
    if (!_isSyncProviderConnected(_SyncProvider.s3Compatible)) {
      _setGithubSyncError('Connect S3-compatible storage first.');
      return;
    }
    _mutate(() {
      _githubSyncRunning = true;
      _githubSyncStatus = null;
      _githubSyncStatusIsError = false;
    });
    try {
      final result = await _runS3SyncInBackground(
        _githubSyncDatabasePath,
        enteringMasterKey ? masterKey : null,
        _syncStrategy.storageValue,
        _syncBackupCount,
      );
      if (!mounted) return;
      final etag = ((result['etag'] as String?) ?? '').replaceAll('"', '');
      final shortEtag = etag.length > 12 ? etag.substring(0, 12) : etag;
      final revision = result['revision'];
      final imported = result['imported_records'];
      _mutate(() {
        _rememberSyncSnapshot(
          revision as int?,
          result['snapshot_id'] as String?,
        );
        if (enteringMasterKey) {
          _hasLocalSyncKey = true;
        }
        _githubSyncStatus =
            'S3 sync complete · ETag $shortEtag · imported $imported · revision $revision.';
        _githubSyncStatusIsError = false;
      });
      if (enteringMasterKey) {
        _cacheGithubSyncSetting(
          _githubSyncDatabasePath,
          'hasLocalSyncKey',
          true,
        );
      }
      notifyNautermDatabaseBulkChange();
    } on Object catch (error) {
      if (mounted) {
        _setGithubSyncError(error.toString());
      }
    } finally {
      _syncMasterKeyController.clear();
      _syncMasterKeyConfirmController.clear();
      if (mounted) {
        _mutate(() => _githubSyncRunning = false);
      }
    }
  }

  void _showProviderConnectionRequirement(String provider) {
    _setGithubSyncError(
      '$provider authorization requires a Nauterm OAuth client ID before sign-in can be enabled.',
    );
  }

  void _continueToSyncManagement() {
    final masterKey = _syncMasterKeyController.text;
    final confirmation = _syncMasterKeyConfirmController.text;
    final validationError = _masterKeyValidationError(masterKey);
    if (validationError != null) {
      _setGithubSyncError(validationError);
      return;
    }
    if (masterKey != confirmation) {
      _setGithubSyncError('Master Key confirmation does not match.');
      return;
    }
    _mutate(() {
      _githubSyncStatus = null;
      _githubSyncStatusIsError = false;
      _syncManagementUnlocked = true;
    });
  }

  void _setGithubSyncError(String message) {
    if (!mounted) return;
    _mutate(() {
      _githubSyncStatus = message;
      _githubSyncStatusIsError = true;
    });
  }

  void _clearGithubSyncStatus() {
    if (!mounted) return;
    _mutate(() {
      _githubSyncStatus = null;
      _githubSyncStatusIsError = false;
    });
  }

  Future<void> _loadGithubSyncHistory() async {
    if (_githubSyncHistoryLoading) return;
    _mutate(() => _githubSyncHistoryLoading = true);
    try {
      final history = await _loadGithubSyncHistoryInBackground(
        _githubSyncDatabasePath,
      );
      if (!mounted) return;
      _mutate(() => _githubSyncHistory = history);
    } on Object {
      // History is supplemental; a transient provider error must not replace sync status.
    } finally {
      if (mounted) {
        _mutate(() => _githubSyncHistoryLoading = false);
      }
    }
  }

  Future<void> _loadGithubGistHistory() async {
    if (_githubGistSyncHistoryLoading) return;
    _mutate(() => _githubGistSyncHistoryLoading = true);
    try {
      final history = await _loadGithubGistHistoryInBackground(
        _githubSyncDatabasePath,
      );
      if (!mounted) return;
      _mutate(() => _githubGistSyncHistory = history);
    } on Object {
      // History is supplemental; sync status remains the primary feedback.
    } finally {
      if (mounted) {
        _mutate(() => _githubGistSyncHistoryLoading = false);
      }
    }
  }
}

class _GithubDeviceFlowDialog extends StatefulWidget {
  const _GithubDeviceFlowDialog({
    required this.initialGistId,
    required this.onAuthorized,
  });

  final String initialGistId;
  final Future<bool> Function(String token) onAuthorized;

  @override
  State<_GithubDeviceFlowDialog> createState() =>
      _GithubDeviceFlowDialogState();
}

class _GithubDeviceFlowDialogState extends State<_GithubDeviceFlowDialog> {
  late final GithubDeviceFlowClient _client;
  late final TextEditingController _gistIdController;
  GithubDeviceFlowSession? _session;
  Timer? _pollTimer;
  Duration _pollInterval = const Duration(seconds: 5);
  bool _starting = true;
  bool _polling = false;
  bool _authorized = false;
  bool _copied = false;
  String? _authorizedLogin;
  String? _error;

  @override
  void initState() {
    super.initState();
    _client = GithubDeviceFlowClient(clientId: githubDeviceFlowClientId);
    _gistIdController = TextEditingController(text: widget.initialGistId);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        unawaited(_begin());
      }
    });
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    _client.close();
    _gistIdController.dispose();
    super.dispose();
  }

  Future<void> _begin() async {
    _pollTimer?.cancel();
    setState(() {
      _session = null;
      _starting = true;
      _polling = false;
      _authorized = false;
      _copied = false;
      _authorizedLogin = null;
      _error = null;
    });
    try {
      final session = await _client.start();
      if (!mounted) return;
      setState(() {
        _session = session;
        _pollInterval = session.interval;
        _starting = false;
      });
      _schedulePoll();
    } on Object catch (error) {
      if (!mounted) return;
      setState(() {
        _starting = false;
        _error = error.toString();
      });
    }
  }

  void _schedulePoll() {
    final session = _session;
    if (!mounted || session == null || _authorized || _error != null) {
      return;
    }
    if (!DateTime.now().isBefore(session.expiresAt)) {
      setState(() => _error = 'The GitHub device code expired. Try again.');
      return;
    }
    _pollTimer?.cancel();
    _pollTimer = Timer(_pollInterval, () => unawaited(_poll()));
  }

  Future<void> _poll() async {
    final session = _session;
    if (!mounted || session == null || _polling) return;
    setState(() => _polling = true);
    try {
      final result = await _client.poll(session.deviceCode);
      if (!mounted) return;
      switch (result.status) {
        case GithubDeviceFlowPollStatus.pending:
          break;
        case GithubDeviceFlowPollStatus.slowDown:
          _pollInterval =
              result.interval ?? _pollInterval + const Duration(seconds: 5);
          break;
        case GithubDeviceFlowPollStatus.authorized:
          final token = result.accessToken;
          final scopes = (result.scope ?? '')
              .split(RegExp(r'[,\s]+'))
              .where((scope) => scope.isNotEmpty)
              .toSet();
          if (token == null || token.isEmpty) {
            throw const GithubDeviceFlowException(
              'GitHub completed authorization without returning a token.',
            );
          }
          if (!scopes.contains('gist')) {
            throw const GithubDeviceFlowException(
              'GitHub did not grant the required gist scope.',
            );
          }
          final login = await _client.loadUserLogin(token);
          final saved = await widget.onAuthorized(token);
          if (!mounted) return;
          if (!saved) {
            throw const GithubDeviceFlowException(
              'Nauterm could not store the GitHub authorization.',
            );
          }
          setState(() {
            _authorized = true;
            _authorizedLogin = login;
            _error = null;
          });
          return;
        case GithubDeviceFlowPollStatus.expired:
          setState(
            () => _error =
                result.errorDescription ??
                'The GitHub device code expired. Try again.',
          );
          return;
        case GithubDeviceFlowPollStatus.denied:
          setState(
            () => _error =
                result.errorDescription ?? 'GitHub authorization was denied.',
          );
          return;
      }
    } on Object catch (error) {
      if (!mounted) return;
      setState(() => _error = error.toString());
      return;
    } finally {
      if (mounted) {
        setState(() => _polling = false);
      }
    }
    _schedulePoll();
  }

  Future<void> _copyCode() async {
    final code = _session?.userCode;
    if (code == null) return;
    await Clipboard.setData(ClipboardData(text: code));
    if (!mounted) return;
    setState(() => _copied = true);
  }

  Future<void> _openGithub() async {
    final uri = _session?.verificationUri;
    if (uri == null) return;
    try {
      if (Platform.isMacOS) {
        await Process.start('open', [uri.toString()]);
      } else if (Platform.isWindows) {
        await Process.start('rundll32', [
          'url.dll,FileProtocolHandler',
          uri.toString(),
        ]);
      } else {
        await Process.start('xdg-open', [uri.toString()]);
      }
    } on Object catch (error) {
      if (!mounted) return;
      setState(() => _error = 'Unable to open GitHub: $error');
    }
  }

  @override
  Widget build(BuildContext context) {
    return _SyncDialogFrame(
      icon: LucideIcons.gitFork,
      title: 'Connect to GitHub',
      subtitle: _authorized
          ? 'GitHub Gist authorization is ready.'
          : 'Authorize Nauterm without pasting an access token.',
      width: 460,
      child: AnimatedSize(
        duration: const Duration(milliseconds: 160),
        curve: Curves.easeOutCubic,
        child: _buildContent(context),
      ),
    );
  }

  Widget _buildContent(BuildContext context) {
    if (_starting) {
      return const SizedBox(
        height: 150,
        child: Center(
          child: SizedBox(
            width: 20,
            height: 20,
            child: CircularProgressIndicator(strokeWidth: 2),
          ),
        ),
      );
    }
    if (_authorized) {
      return Column(
        children: [
          Container(
            width: 48,
            height: 48,
            decoration: BoxDecoration(
              color: _secondary.withValues(alpha: 0.1),
              shape: BoxShape.circle,
            ),
            alignment: Alignment.center,
            child: Icon(LucideIcons.circleCheck, size: 23, color: _secondary),
          ),
          SizedBox(height: 12),
          Text(
            tr('settings.label.githubConnected', fallback: 'GitHub connected'),
            style: TextStyle(
              color: _text,
              fontSize: 14,
              fontWeight: NautermFontWeights.semibold,
            ),
          ),
          SizedBox(height: 5),
          Text(
            _authorizedLogin == null
                ? 'Nauterm can now access private Gists for sync.'
                : 'Connected as @$_authorizedLogin.',
            textAlign: TextAlign.center,
            style: TextStyle(color: _mutedText, fontSize: 11),
          ),
          SizedBox(height: 16),
          Align(
            alignment: Alignment.centerLeft,
            child: Text(
              tr('settings.label.gistId', fallback: 'Gist ID'),
              style: TextStyle(
                color: _text,
                fontSize: 12,
                fontWeight: NautermFontWeights.semibold,
              ),
            ),
          ),
          SizedBox(height: 7),
          _SettingsTextField(
            key: const ValueKey('github-gist-id-after-authorization'),
            controller: _gistIdController,
            hint: 'Create automatically',
            onChanged: (_) => setState(() {}),
            onSubmitted: (_) => _confirmGist(),
          ),
          SizedBox(height: 7),
          Align(
            alignment: Alignment.centerLeft,
            child: Text(
              tr(
                'settings.sync.gist.id.description',
                fallback: 'Leave empty to create a new private gist on the first sync.',
              ),
              style: TextStyle(color: _mutedText, fontSize: 11, height: 1.4),
            ),
          ),
          SizedBox(height: 18),
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              key: const ValueKey('confirm-github-gist-after-authorization'),
              onPressed: _confirmGist,
              style: FilledButton.styleFrom(
                backgroundColor: _primary,
                foregroundColor: Colors.white,
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(7),
                ),
              ),
              child: Text(
                tr(
                  'settings.label.saveGistSettings',
                  fallback: 'Save Gist Settings',
                ),
              ),
            ),
          ),
        ],
      );
    }
    if (_error != null) {
      return Column(
        children: [
          _SyncStatusMessage(message: _error!, isError: true),
          SizedBox(height: 14),
          Align(
            alignment: Alignment.centerRight,
            child: _SettingsOutlineButton(label: 'Try Again', onTap: _begin),
          ),
        ],
      );
    }
    final session = _session!;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          tr(
            'settings.sync.github.deviceCode.description',
            fallback:
                'Copy this code, then enter it on GitHub to authorize Nauterm.',
          ),
          textAlign: TextAlign.center,
          style: TextStyle(
            color: _mutedText,
            fontSize: 12,
            height: 1.45,
            fontWeight: NautermFontWeights.regular,
          ),
        ),
        SizedBox(height: 16),
        Container(
          padding: const EdgeInsets.fromLTRB(16, 17, 16, 14),
          decoration: BoxDecoration(
            color: _surfaceContainer,
            borderRadius: BorderRadius.circular(10),
            border: Border.all(color: _softOutline),
          ),
          child: Column(
            children: [
              SelectableText(
                session.userCode,
                style: TextStyle(
                  color: _text,
                  fontSize: 23,
                  letterSpacing: 3,
                  fontWeight: NautermFontWeights.semibold,
                ),
              ),
              SizedBox(height: 10),
              TextButton.icon(
                onPressed: _copyCode,
                icon: Icon(
                  _copied ? LucideIcons.check : LucideIcons.copy,
                  size: 14,
                ),
                label: Text(_copied ? 'Copied' : 'Copy code'),
                style: TextButton.styleFrom(
                  foregroundColor: _text,
                  textStyle: TextStyle(
                    fontSize: 11,
                    fontWeight: NautermFontWeights.medium,
                  ),
                ),
              ),
            ],
          ),
        ),
        SizedBox(height: 12),
        SizedBox(
          height: 36,
          child: FilledButton.icon(
            onPressed: _openGithub,
            style: FilledButton.styleFrom(
              backgroundColor: _primary,
              foregroundColor: Colors.white,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(7),
              ),
            ),
            icon: Icon(LucideIcons.externalLink, size: 14),
            label: Text(
              tr('settings.label.openGithub', fallback: 'Open GitHub'),
            ),
          ),
        ),
        SizedBox(height: 14),
        Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            SizedBox(
              width: 13,
              height: 13,
              child: CircularProgressIndicator(
                strokeWidth: 1.7,
                color: _mutedText,
              ),
            ),
            SizedBox(width: 8),
            Text(
              tr(
                'settings.label.waitingForAuthorization',
                fallback: 'Waiting for authorization…',
              ),
              style: TextStyle(color: _mutedText, fontSize: 11),
            ),
          ],
        ),
      ],
    );
  }

  void _confirmGist() {
    Navigator.of(context).pop(_gistIdController.text.trim());
  }
}

class _SyncProviderDialog extends StatelessWidget {
  const _SyncProviderDialog({required this.provider, required this.state});

  final _SyncProvider provider;
  final _SettingsPanelState state;

  @override
  Widget build(BuildContext context) {
    return _SyncDialogFrame(
      icon: provider.icon,
      title: provider.label,
      subtitle: _subtitle,
      titleTooltip: provider == _SyncProvider.s3Compatible
          ? context.tr(
              'settings.sync.s3.compatibility.description',
              fallback:
                  'Supports Amazon S3, Cloudflare R2, Backblaze B2, '
                  'DigitalOcean Spaces, Wasabi, Alibaba Cloud OSS, Tencent '
                  'Cloud COS, and other services exposing an S3-compatible '
                  'API. MinIO is also available as a separate provider preset.',
            )
          : null,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (provider == _SyncProvider.githubRepository) ...[
            _buildGithubAuthenticationSettings(state),
            SizedBox(height: 24),
          ],
          if (provider == _SyncProvider.githubGist) ...[
            _buildGithubGistAuthorizationSettings(state),
            SizedBox(height: 24),
          ],
          switch (provider) {
            _SyncProvider.githubRepository => _buildGithubRepositorySettings(
              state,
            ),
            _SyncProvider.githubGist => _buildGithubGistSettings(state),
            _SyncProvider.googleDrive => _buildOAuthSyncProviderSettings(
              state,
              icon: LucideIcons.triangle,
              title: 'Google Drive',
              description: 'Nauterm stores nauterm-sync.enc in Google Drive and requests drive.file access, limited to files created or opened by Nauterm.',
              buttonLabel: 'Connect Google Drive',
            ),
            _SyncProvider.oneDrive => _buildOAuthSyncProviderSettings(
              state,
              icon: LucideIcons.cloud,
              title: 'OneDrive',
              description: 'Nauterm stores nauterm-sync.enc in its OneDrive application folder.',
              buttonLabel: 'Connect OneDrive',
            ),
            _SyncProvider.webdav => _buildWebdavSettings(state),
            _SyncProvider.s3Compatible => _buildS3Settings(state),
          },
          if (provider != _SyncProvider.githubRepository &&
              provider != _SyncProvider.githubGist &&
              provider != _SyncProvider.s3Compatible) ...[
            SizedBox(height: 18),
            _SyncUnavailableNotice(provider: provider),
          ],
        ],
      ),
    );
  }

  String get _subtitle => switch (provider) {
    _SyncProvider.githubRepository =>
      'Store encrypted revisions in a private Git repository.',
    _SyncProvider.githubGist =>
      'Store the encrypted payload in a private GitHub Gist.',
    _SyncProvider.googleDrive =>
      'Authorize Nauterm to manage its encrypted sync file in Google Drive.',
    _SyncProvider.oneDrive =>
      'Authorize Nauterm to use its private application folder.',
    _SyncProvider.webdav => 'Connect a self-hosted or managed WebDAV endpoint.',
    _SyncProvider.s3Compatible =>
      'Connect Amazon S3 or another S3-compatible service.',
  };
}

class _SyncUnavailableNotice extends StatelessWidget {
  const _SyncUnavailableNotice({required this.provider});

  final _SyncProvider provider;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: _surfaceContainer,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: _softOutline),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(LucideIcons.info, size: 15, color: _mutedText),
          SizedBox(width: 9),
          Expanded(
            child: Text(
              tr(
                '${provider.label} connection settings are prepared, but this transport is not enabled in the current build.',
              ),
              style: TextStyle(
                color: _mutedText,
                fontSize: 11,
                height: 1.45,
                fontWeight: NautermFontWeights.regular,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _LocalSyncBackupsDialog extends StatefulWidget {
  const _LocalSyncBackupsDialog({
    required this.databasePath,
    required this.backupCount,
    required this.onRestored,
  });

  final String databasePath;
  final int backupCount;
  final VoidCallback onRestored;

  @override
  State<_LocalSyncBackupsDialog> createState() =>
      _LocalSyncBackupsDialogState();
}

class _LocalSyncBackupsDialogState extends State<_LocalSyncBackupsDialog> {
  List<Map<String, dynamic>> _backups = const [];
  String? _restoring;
  String? _error;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    try {
      final backups = await _loadLocalSyncBackupsInBackground(
        widget.databasePath,
      );
      if (mounted) setState(() => _backups = backups);
    } on Object catch (error) {
      if (mounted) setState(() => _error = error.toString());
    }
  }

  Future<void> _restore(String backupId) async {
    setState(() {
      _restoring = backupId;
      _error = null;
    });
    try {
      await _restoreLocalSyncBackupInBackground(
        widget.databasePath,
        backupId,
        widget.backupCount,
      );
      if (!mounted) return;
      widget.onRestored();
      await _load();
    } on Object catch (error) {
      if (mounted) setState(() => _error = error.toString());
    } finally {
      if (mounted) setState(() => _restoring = null);
    }
  }

  @override
  Widget build(BuildContext context) {
    return _SyncDialogFrame(
      icon: LucideIcons.databaseBackup,
      title: 'Local Backups',
      subtitle: 'Encrypted database snapshots created before sync.',
      width: 540,
      child: Column(
        children: [
          if (_error != null) ...[
            Text(
              _error!,
              style: TextStyle(color: const Color(0xffdc2626), fontSize: 11),
            ),
            SizedBox(height: 10),
          ],
          if (_backups.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 26),
              child: Text(
                tr(
                  'settings.sync.localBackups.empty',
                  fallback: 'No local backups yet.',
                ),
                style: TextStyle(color: _mutedText, fontSize: 12),
              ),
            )
          else
            for (final backup in _backups) ...[
              _LocalSyncBackupCard(
                backup: backup,
                restoring: _restoring == backup['id'],
                disabled: _restoring != null,
                onRestore: () => _restore(backup['id'] as String),
              ),
              if (backup != _backups.last) SizedBox(height: 7),
            ],
        ],
      ),
    );
  }
}

class _LocalSyncBackupCard extends StatelessWidget {
  const _LocalSyncBackupCard({
    required this.backup,
    required this.restoring,
    required this.disabled,
    required this.onRestore,
  });

  final Map<String, dynamic> backup;
  final bool restoring;
  final bool disabled;
  final VoidCallback onRestore;

  @override
  Widget build(BuildContext context) {
    final createdAt = DateTime.fromMillisecondsSinceEpoch(
      backup['created_at'] as int? ?? 0,
    ).toLocal();
    final bytes = backup['bytes'] as int? ?? 0;
    const reason = 'Sync';
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: _surfaceContainer,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: _softOutline),
      ),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  reason,
                  style: TextStyle(
                    color: _text,
                    fontSize: 12,
                    fontWeight: NautermFontWeights.semibold,
                  ),
                ),
                SizedBox(height: 2),
                Text(
                  tr(
                    '${createdAt.toString().substring(0, 19)} · ${_formatLocalBackupBytes(bytes)}',
                  ),
                  style: TextStyle(color: _mutedText, fontSize: 10),
                ),
              ],
            ),
          ),
          _SettingsOutlineButton(
            label: restoring
                ? tr('common.label.restoring', fallback: 'Restoring…')
                : tr('common.action.restore', fallback: 'Restore'),
            onTap: disabled ? null : onRestore,
          ),
        ],
      ),
    );
  }
}

String _formatLocalBackupBytes(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}

class _SyncHistoryDialog extends StatelessWidget {
  const _SyncHistoryDialog({
    required this.revisions,
    required this.loading,
    required this.onRestore,
  });

  final List<Map<String, dynamic>> revisions;
  final bool loading;
  final Future<void> Function(String revision) onRestore;

  @override
  Widget build(BuildContext context) {
    return _SyncDialogFrame(
      icon: LucideIcons.history,
      title: 'Sync History',
      subtitle: 'Browse encrypted revisions stored in GitHub.',
      width: 540,
      child: loading
          ? Padding(
              padding: const EdgeInsets.symmetric(vertical: 30),
              child: Center(
                child: SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    color: _primary,
                  ),
                ),
              ),
            )
          : revisions.isEmpty
          ? Padding(
              padding: const EdgeInsets.symmetric(vertical: 26),
              child: Column(
                children: [
                  Icon(LucideIcons.history, size: 24, color: _faintText),
                  SizedBox(height: 9),
                  Text(
                    tr(
                      'settings.sync.history.empty',
                      fallback: 'No encrypted revisions found yet.',
                    ),
                    style: TextStyle(
                      color: _mutedText,
                      fontSize: 12,
                      fontWeight: NautermFontWeights.regular,
                    ),
                  ),
                ],
              ),
            )
          : Column(
              children: [
                for (var index = 0; index < revisions.length; index++) ...[
                  _SyncHistoryCard(
                    revision: revisions[index],
                    label: index == 0
                        ? 'Current'
                        : 'Revision #${revisions.length - index}',
                    selected: index == 0,
                    onRestore: index == 0 ? null : onRestore,
                  ),
                  if (index != revisions.length - 1) SizedBox(height: 7),
                ],
              ],
            ),
    );
  }
}

class _SyncHistoryCard extends StatelessWidget {
  const _SyncHistoryCard({
    required this.revision,
    required this.label,
    required this.selected,
    required this.onRestore,
  });

  final Map<String, dynamic> revision;
  final String label;
  final bool selected;
  final Future<void> Function(String revision)? onRestore;

  @override
  Widget build(BuildContext context) {
    final sha = revision['sha'] as String? ?? '';
    final shortSha = sha.length > 7 ? sha.substring(0, 7) : sha;
    final committedAt = (revision['committed_at'] as String?)?.trim() ?? '';
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: selected ? _primary.withValues(alpha: 0.06) : _surfaceContainer,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: selected ? _primary.withValues(alpha: 0.28) : _softOutline,
        ),
      ),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  tr(label),
                  style: TextStyle(
                    color: _text,
                    fontSize: 12,
                    fontWeight: NautermFontWeights.semibold,
                  ),
                ),
                if (committedAt.isNotEmpty) ...[
                  SizedBox(height: 2),
                  Text(
                    committedAt,
                    style: TextStyle(
                      color: _mutedText,
                      fontSize: 10,
                      fontWeight: NautermFontWeights.regular,
                    ),
                  ),
                ],
              ],
            ),
          ),
          Text(
            shortSha,
            style: TextStyle(
              color: _mutedText,
              fontSize: 10,
              fontFamily: 'monospace',
              fontWeight: NautermFontWeights.regular,
            ),
          ),
          if (onRestore != null) ...[
            SizedBox(width: 8),
            _SettingsOutlineButton(
              label: tr('common.action.restore', fallback: 'Restore'),
              onTap: () async {
                try {
                  await onRestore!(sha);
                  if (context.mounted) Navigator.of(context).pop();
                } on Object {
                  // The settings page displays the provider error and keeps history open.
                }
              },
            ),
          ],
        ],
      ),
    );
  }
}

enum _SyncKeyAction { changeMasterKey, forgetMasterKey }

class _SyncKeyDetailsDialog extends StatelessWidget {
  const _SyncKeyDetailsDialog({
    required this.hasLocalSyncKey,
    required this.canChangeMasterKey,
    required this.onChangeMasterKey,
    required this.onForgetMasterKey,
  });

  final bool hasLocalSyncKey;
  final bool canChangeMasterKey;
  final VoidCallback onChangeMasterKey;
  final VoidCallback onForgetMasterKey;

  @override
  Widget build(BuildContext context) {
    return _SyncDialogFrame(
      icon: LucideIcons.keyRound,
      title: 'settings.label.syncKey',
      subtitle: 'settings.sync.key.description',
      width: 460,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _SyncKeyDetailRow(
            icon: LucideIcons.shieldCheck,
            title: hasLocalSyncKey
                ? 'settings.sync.key.localAvailable.label'
                : 'settings.sync.key.localMissing.label',
            description: hasLocalSyncKey
                ? 'settings.sync.key.localAvailable.description'
                : 'settings.sync.key.localMissing.description',
          ),
          SizedBox(height: 14),
          _SyncKeyDetailRow(
            icon: LucideIcons.eyeOff,
            title: 'settings.label.masterKeyIsNotStored',
            description: 'settings.sync.key.masterKeyNotStored.description',
          ),
          SizedBox(height: 18),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: 8,
            runSpacing: 8,
            children: [
              _SettingsOutlineButton(
                label: tr(
                  'settings.label.forgetMasterKey',
                  fallback: 'Forget Master Key',
                ),
                onTap: hasLocalSyncKey ? onForgetMasterKey : null,
              ),
              _SettingsOutlineButton(
                label: tr(
                  'settings.label.changeMasterKey',
                  fallback: 'Change Master Key',
                ),
                onTap: canChangeMasterKey ? onChangeMasterKey : null,
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _ChangeSyncMasterKeyDialog extends StatefulWidget {
  const _ChangeSyncMasterKeyDialog({required this.onChange});

  final Future<String?> Function(String currentKey, String newKey) onChange;

  @override
  State<_ChangeSyncMasterKeyDialog> createState() =>
      _ChangeSyncMasterKeyDialogState();
}

class _ChangeSyncMasterKeyDialogState
    extends State<_ChangeSyncMasterKeyDialog> {
  final TextEditingController _currentController = TextEditingController();
  final TextEditingController _newController = TextEditingController();
  final TextEditingController _confirmController = TextEditingController();
  bool _running = false;
  String? _error;

  @override
  void dispose() {
    _currentController.dispose();
    _newController.dispose();
    _confirmController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (_running) return;
    final currentKey = _currentController.text;
    final newKey = _newController.text;
    final confirmation = _confirmController.text;
    String? error;
    final currentKeyError = _masterKeyValidationError(
      currentKey,
      subject: 'Current Master Key',
    );
    final newKeyError = _masterKeyValidationError(
      newKey,
      subject: 'New Master Key',
    );
    if (currentKeyError != null) {
      error = currentKeyError;
    } else if (newKeyError != null) {
      error = newKeyError;
    } else if (newKey == currentKey) {
      error = 'New Master Key must be different from the current key.';
    } else if (newKey != confirmation) {
      error = 'New Master Key confirmation does not match.';
    }
    if (error != null) {
      setState(() => _error = error);
      return;
    }

    setState(() {
      _running = true;
      _error = null;
    });
    final changeError = await widget.onChange(currentKey, newKey);
    if (!mounted) return;
    if (changeError == null) {
      Navigator.of(context).pop();
      return;
    }
    setState(() {
      _running = false;
      _error = changeError;
    });
  }

  @override
  Widget build(BuildContext context) {
    return _SyncDialogFrame(
      icon: LucideIcons.keyRound,
      title: 'Change Master Key',
      subtitle: 'Re-wrap the Sync DEK for every connected provider.',
      width: 500,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            tr(
              'Keep the current key until every provider is updated. Encrypted records and local captures are not re-encrypted.',
            ),
            style: TextStyle(
              color: _mutedText,
              fontSize: 11,
              height: 1.45,
              fontWeight: NautermFontWeights.regular,
            ),
          ),
          SizedBox(height: 16),
          _SettingsTextField(
            key: const ValueKey('settings-sync-current-master-key'),
            controller: _currentController,
            obscureText: true,
            hint: 'Current Master Key',
            onChanged: (_) => setState(() => _error = null),
            onSubmitted: (_) => _submit(),
          ),
          SizedBox(height: 10),
          _MasterKeyFieldWithStrength(
            key: const ValueKey('settings-sync-new-master-key'),
            controller: _newController,
            hint: 'New Master Key',
            onChanged: (_) => setState(() => _error = null),
            onSubmitted: (_) => _submit(),
          ),
          SizedBox(height: 10),
          _SettingsTextField(
            key: const ValueKey('settings-sync-new-master-key-confirm'),
            controller: _confirmController,
            obscureText: true,
            hint: 'Confirm new Master Key',
            onChanged: (_) => setState(() => _error = null),
            onSubmitted: (_) => _submit(),
          ),
          if (_error != null) ...[
            SizedBox(height: 10),
            Text(
              _error!,
              style: TextStyle(
                color: const Color(0xffdc2626),
                fontSize: 11,
                fontWeight: NautermFontWeights.medium,
              ),
            ),
          ],
          SizedBox(height: 18),
          Align(
            alignment: Alignment.centerRight,
            child: SizedBox(
              height: 32,
              child: FilledButton(
                key: const ValueKey('settings-sync-change-master-key-submit'),
                onPressed: _running ? null : _submit,
                style: FilledButton.styleFrom(
                  backgroundColor: _primary,
                  foregroundColor: Colors.white,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(7),
                  ),
                  textStyle: TextStyle(
                    fontSize: 12,
                    fontWeight: NautermFontWeights.medium,
                  ),
                ),
                child: Text(_running ? 'Changing…' : 'Change Master Key'),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

enum _MasterKeyStrength { weak, fair, good, strong }

const int _masterKeyMinimumLength = 12;
const int _masterKeyMinimumCharacterClasses = 3;

bool _masterKeyUsesPrintableAscii(String value) =>
    value.codeUnits.every((codeUnit) => codeUnit >= 0x20 && codeUnit <= 0x7e);

bool _masterKeyHasUppercase(String value) => RegExp(r'[A-Z]').hasMatch(value);

bool _masterKeyHasLowercase(String value) => RegExp(r'[a-z]').hasMatch(value);

bool _masterKeyHasDigit(String value) => RegExp(r'[0-9]').hasMatch(value);

bool _masterKeyHasSymbol(String value) => value.codeUnits.any(
  (codeUnit) =>
      codeUnit >= 0x20 &&
      codeUnit <= 0x7e &&
      !((codeUnit >= 0x41 && codeUnit <= 0x5a) ||
          (codeUnit >= 0x61 && codeUnit <= 0x7a) ||
          (codeUnit >= 0x30 && codeUnit <= 0x39)),
);

int _masterKeyCharacterClassCount(String value) => <bool>[
  _masterKeyHasUppercase(value),
  _masterKeyHasLowercase(value),
  _masterKeyHasDigit(value),
  _masterKeyHasSymbol(value),
].where((present) => present).length;

String? _masterKeyValidationError(
  String value, {
  String subject = 'Master Key',
}) {
  if (value.characters.length < _masterKeyMinimumLength) {
    return tr(
      'settings.sync.masterKey.error.length',
      fallback: '{subject} must contain at least 12 characters.',
      args: {'subject': subject},
    );
  }
  if (!_masterKeyUsesPrintableAscii(value)) {
    return tr(
      'settings.sync.masterKey.error.characters',
      fallback: '{subject} contains unsupported characters.',
      args: {'subject': subject},
    );
  }
  if (_masterKeyCharacterClassCount(value) <
      _masterKeyMinimumCharacterClasses) {
    return tr(
      'settings.sync.masterKey.error.characterClasses',
      fallback: '{subject} must include at least 3 of these character types: uppercase letters, lowercase letters, numbers, and symbols.',
      args: {'subject': subject},
    );
  }
  return null;
}

_MasterKeyStrength _masterKeyStrength(String value) {
  final length = value.characters.length;
  final characterClasses = _masterKeyCharacterClassCount(value);
  if (length < _masterKeyMinimumLength ||
      !_masterKeyUsesPrintableAscii(value) ||
      characterClasses < _masterKeyMinimumCharacterClasses) {
    return _MasterKeyStrength.weak;
  }

  if ((length >= 20 && characterClasses >= 3) ||
      (length >= 24 && characterClasses >= 2)) {
    return _MasterKeyStrength.strong;
  }
  if ((length >= 16 && characterClasses >= 2) || length >= 20) {
    return _MasterKeyStrength.good;
  }
  return _MasterKeyStrength.fair;
}

extension on _MasterKeyStrength {
  String get localizationKey => 'settings.sync.masterKey.strength.$name';

  String get fallback => switch (this) {
    _MasterKeyStrength.weak => 'Weak',
    _MasterKeyStrength.fair => 'Fair',
    _MasterKeyStrength.good => 'Good',
    _MasterKeyStrength.strong => 'Strong',
  };

  Color get color => switch (this) {
    _MasterKeyStrength.weak => const Color(0xffdc2626),
    _MasterKeyStrength.fair => const Color(0xffd97706),
    _MasterKeyStrength.good => const Color(0xff2563eb),
    _MasterKeyStrength.strong => const Color(0xff16a34a),
  };
}

class _MasterKeyFieldWithStrength extends StatelessWidget {
  const _MasterKeyFieldWithStrength({
    super.key,
    required this.controller,
    required this.hint,
    required this.onChanged,
    this.onSubmitted,
  });

  final TextEditingController controller;
  final String hint;
  final ValueChanged<String> onChanged;
  final ValueChanged<String>? onSubmitted;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _SettingsTextField(
          controller: controller,
          obscureText: true,
          revealable: true,
          hint: hint,
          onChanged: onChanged,
          onSubmitted: onSubmitted,
        ),
        ValueListenableBuilder<TextEditingValue>(
          valueListenable: controller,
          builder: (context, value, _) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                if (value.text.isNotEmpty)
                  _MasterKeyStrengthIndicator(
                    strength: _masterKeyStrength(value.text),
                  ),
                _MasterKeyRequirements(value: value.text),
              ],
            );
          },
        ),
      ],
    );
  }
}

class _MasterKeyRequirements extends StatelessWidget {
  const _MasterKeyRequirements({required this.value});

  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 7),
      child: Wrap(
        key: const ValueKey('settings-sync-master-key-requirements'),
        spacing: 10,
        runSpacing: 5,
        children: [
          _MasterKeyRequirement(
            label: tr(
              'settings.sync.masterKey.requirement.length',
              fallback: '12+ characters',
            ),
            satisfied: value.characters.length >= _masterKeyMinimumLength,
          ),
          _MasterKeyRequirement(
            label: tr(
              'settings.sync.masterKey.requirement.uppercase',
              fallback: 'Uppercase',
            ),
            satisfied: _masterKeyHasUppercase(value),
          ),
          _MasterKeyRequirement(
            label: tr(
              'settings.sync.masterKey.requirement.lowercase',
              fallback: 'Lowercase',
            ),
            satisfied: _masterKeyHasLowercase(value),
          ),
          _MasterKeyRequirement(
            label: tr(
              'settings.sync.masterKey.requirement.number',
              fallback: 'Number',
            ),
            satisfied: _masterKeyHasDigit(value),
          ),
          _MasterKeyRequirement(
            label: tr(
              'settings.sync.masterKey.requirement.symbol',
              fallback: 'Symbol',
            ),
            satisfied: _masterKeyHasSymbol(value),
          ),
        ],
      ),
    );
  }
}

class _MasterKeyRequirement extends StatelessWidget {
  const _MasterKeyRequirement({required this.label, required this.satisfied});

  final String label;
  final bool satisfied;

  @override
  Widget build(BuildContext context) {
    final color = satisfied ? const Color(0xff16a34a) : _faintText;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(
          satisfied ? LucideIcons.circleCheck : LucideIcons.circle,
          size: 12,
          color: color,
        ),
        const SizedBox(width: 4),
        Text(
          label,
          style: TextStyle(
            color: color,
            fontSize: 10,
            height: 1.2,
            fontWeight: NautermFontWeights.medium,
          ),
        ),
      ],
    );
  }
}

class _MasterKeyStrengthIndicator extends StatelessWidget {
  const _MasterKeyStrengthIndicator({required this.strength});

  final _MasterKeyStrength strength;

  @override
  Widget build(BuildContext context) {
    final level = tr(strength.localizationKey, fallback: strength.fallback);
    final label = tr(
      'settings.sync.masterKey.strength.label',
      fallback: 'Password strength',
    );
    return Semantics(
      key: const ValueKey('settings-sync-master-key-strength'),
      label: '$label: $level',
      child: Padding(
        padding: const EdgeInsets.only(top: 7),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                for (
                  var index = 0;
                  index < _MasterKeyStrength.values.length;
                  index++
                ) ...[
                  if (index > 0) const SizedBox(width: 4),
                  Expanded(
                    child: AnimatedContainer(
                      duration: const Duration(milliseconds: 140),
                      height: 3,
                      decoration: BoxDecoration(
                        color: index <= strength.index
                            ? strength.color
                            : _softOutline,
                        borderRadius: BorderRadius.circular(2),
                      ),
                    ),
                  ),
                ],
              ],
            ),
            const SizedBox(height: 5),
            Text(
              '$label: $level',
              textAlign: TextAlign.right,
              style: TextStyle(
                color: strength.color,
                fontSize: 11,
                fontWeight: NautermFontWeights.medium,
                letterSpacing: 0,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _SyncKeyDetailRow extends StatelessWidget {
  const _SyncKeyDetailRow({
    required this.icon,
    required this.title,
    required this.description,
  });

  final IconData icon;
  final String title;
  final String description;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          width: 32,
          height: 32,
          decoration: BoxDecoration(
            color: _surfaceContainer,
            borderRadius: BorderRadius.circular(8),
          ),
          alignment: Alignment.center,
          child: Icon(icon, size: 15, color: _mutedText),
        ),
        SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                tr(title),
                style: TextStyle(
                  color: _text,
                  fontSize: 12,
                  fontWeight: NautermFontWeights.semibold,
                ),
              ),
              SizedBox(height: 3),
              Text(
                tr(description),
                style: TextStyle(
                  color: _mutedText,
                  fontSize: 11,
                  height: 1.45,
                  fontWeight: NautermFontWeights.regular,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _SyncDialogFrame extends StatelessWidget {
  const _SyncDialogFrame({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.child,
    this.width = 660,
    this.titleTooltip,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final Widget child;
  final double width;
  final String? titleTooltip;

  @override
  Widget build(BuildContext context) {
    final maxHeight = math.min(MediaQuery.sizeOf(context).height - 48, 680.0);
    return Dialog(
      backgroundColor: Colors.transparent,
      surfaceTintColor: Colors.transparent,
      insetPadding: const EdgeInsets.symmetric(horizontal: 24, vertical: 24),
      child: ConstrainedBox(
        constraints: BoxConstraints(maxWidth: width, maxHeight: maxHeight),
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: _surface,
            borderRadius: BorderRadius.circular(12),
            border: Border.all(color: _softOutline),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(
                  alpha: _settingsDark ? 0.38 : 0.14,
                ),
                blurRadius: 24,
                offset: const Offset(0, 14),
              ),
            ],
          ),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(12),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Container(
                  padding: const EdgeInsets.fromLTRB(15, 10, 8, 9),
                  decoration: BoxDecoration(
                    color: _surfaceContainer,
                    border: Border(bottom: BorderSide(color: _softOutline)),
                  ),
                  child: Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Container(
                        width: 28,
                        height: 28,
                        decoration: BoxDecoration(
                          color: _primary.withValues(alpha: 0.09),
                          borderRadius: BorderRadius.circular(7),
                        ),
                        alignment: Alignment.center,
                        child: Icon(icon, size: 14, color: _primary),
                      ),
                      SizedBox(width: 9),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(
                              children: [
                                Flexible(
                                  child: Text(
                                    tr(title),
                                    style: TextStyle(
                                      color: _text,
                                      fontSize: 14,
                                      fontWeight: NautermFontWeights.semibold,
                                    ),
                                  ),
                                ),
                                if (titleTooltip case final message?) ...[
                                  SizedBox(width: 5),
                                  Tooltip(
                                    message: message,
                                    waitDuration: const Duration(
                                      milliseconds: 300,
                                    ),
                                    constraints: const BoxConstraints(
                                      maxWidth: 360,
                                    ),
                                    child: Icon(
                                      LucideIcons.info,
                                      key: const ValueKey(
                                        'settings-sync-provider-info',
                                      ),
                                      size: 13,
                                      color: _mutedText,
                                    ),
                                  ),
                                ],
                              ],
                            ),
                            SizedBox(height: 1),
                            Text(
                              tr(subtitle),
                              style: TextStyle(
                                color: _mutedText,
                                fontSize: 10,
                                fontWeight: NautermFontWeights.regular,
                              ),
                            ),
                          ],
                        ),
                      ),
                      IconButton(
                        tooltip: tr('common.action.close', fallback: 'Close'),
                        onPressed: () => Navigator.of(context).pop(),
                        icon: Icon(LucideIcons.x, size: 15),
                        color: _mutedText,
                        visualDensity: VisualDensity.compact,
                        style: IconButton.styleFrom(
                          hoverColor: _softOutline,
                          highlightColor: _softOutline,
                        ),
                      ),
                    ],
                  ),
                ),
                Flexible(
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.all(18),
                    child: child,
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

class _SettingsOutlineButton extends StatelessWidget {
  const _SettingsOutlineButton({
    super.key,
    required this.label,
    required this.onTap,
    this.leading,
  });

  final String label;
  final VoidCallback? onTap;
  final Widget? leading;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 30,
      child: OutlinedButton(
        onPressed: onTap,
        style: OutlinedButton.styleFrom(
          foregroundColor: _text,
          side: BorderSide(color: _softOutline),
          padding: const EdgeInsets.symmetric(horizontal: 11),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(7)),
          textStyle: TextStyle(
            fontSize: 12,
            fontWeight: NautermFontWeights.medium,
          ),
        ),
        child: leading == null
            ? Text(tr(label))
            : Row(
                mainAxisSize: MainAxisSize.min,
                children: [leading!, const SizedBox(width: 6), Text(tr(label))],
              ),
      ),
    );
  }
}
