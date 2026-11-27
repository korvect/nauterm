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
Future<Map<String, dynamic>> Function(String databasePath)?
settingsRemoteSyncStatusLoaderOverride;

@visibleForTesting
Future<Map<String, dynamic>> Function(
  String providerId,
  String databasePath,
  String? masterKey,
  String strategy,
  int backupCount,
)?
settingsSyncRunnerOverride;

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
  final loader = settingsRemoteSyncStatusLoaderOverride;
  if (loader != null) return loader(databasePath);
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
  String gistId,
  String? masterKey,
  String strategy,
  int backupCount,
) async {
  final runner = settingsSyncRunnerOverride;
  if (runner != null) {
    return runner(
      'github_gist',
      databasePath,
      masterKey,
      strategy,
      backupCount,
    );
  }
  await _saveGithubGistConfigInBackground(
    databasePath: databasePath,
    gistId: gistId,
  );
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
  final runner = settingsSyncRunnerOverride;
  if (runner != null) {
    return runner(
      'github_repository',
      databasePath,
      masterKey,
      strategy,
      backupCount,
    );
  }
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

Future<void> _removeS3CredentialsInBackground(String databasePath) async {
  await Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      store.s3DeleteCredentials();
    } finally {
      store.dispose();
    }
  });
  _cacheGithubSyncSetting(databasePath, 'hasS3Credentials', false);
}

Future<Map<String, dynamic>> _runS3SyncInBackground(
  String databasePath,
  String? masterKey,
  String strategy,
  int backupCount,
) {
  final runner = settingsSyncRunnerOverride;
  if (runner != null) {
    return runner('s3', databasePath, masterKey, strategy, backupCount);
  }
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
  final busy = state._githubSyncRunning;
  final providers = [
    for (final provider in state._visibleBuiltInSyncProviders)
      _SyncProviderRow(
        key: ValueKey('settings-sync-provider-${provider.name}'),
        name: provider.label,
        kind: provider == _SyncProvider.s3Compatible
            ? 'S3 Compatible'
            : provider.label,
        connected: state._isSyncProviderConnected(provider),
        active:
            state._activeSyncProviderId == state._builtInProviderId(provider),
        busy: busy,
        onOpen: () => state._openSyncProvider(provider),
        onActivate: () => state._activateBuiltInProvider(provider),
        onSync: switch (provider) {
          _SyncProvider.githubRepository => state._runGithubSync,
          _SyncProvider.githubGist => state._runGithubGistSync,
          _SyncProvider.s3Compatible => state._runS3Sync,
          _ => null,
        },
        onHistory: switch (provider) {
          _SyncProvider.githubRepository => state._showGithubSyncHistory,
          _SyncProvider.githubGist => state._showGithubGistHistory,
          _SyncProvider.s3Compatible => state._showS3History,
          _ => null,
        },
      ),
    for (final provider in state._cloudProviders)
      _SyncProviderRow(
        key: ValueKey('settings-cloud-provider-${provider.id}'),
        name: provider.name,
        kind: provider.definition.label,
        connected: provider.hasCredentials,
        active: state._activeSyncProviderId == 'cloud:${provider.id}',
        busy: busy,
        onOpen: () => state._editCloudProvider(
          definition: provider.definition,
          provider: provider,
        ),
        onActivate: () => state._activateCloudProvider(provider),
        onSync: () => state._runCloudProviderSync(provider),
        onHistory: provider.supportsHistory
            ? () => state._showCloudProviderHistory(provider)
            : null,
      ),
  ];
  return Scrollbar(
    controller: state._contentScrollController,
    thumbVisibility: true,
    child: ListView(
      controller: state._contentScrollController,
      padding: _settingsContentPadding(state.context),
      children: [
        Text(
          tr('settings.pages.sync.title', fallback: 'Sync & Backup'),
          style: TextStyle(
            color: _text,
            fontSize: 20,
            height: 1.4,
            fontWeight: NautermFontWeights.semibold,
          ),
        ),
        const SizedBox(height: 3),
        Text(
          tr(
            'settings.pages.sync.description',
            fallback: 'Configure encrypted backups, automatic sync, and conflict handling.',
          ),
          style: TextStyle(color: _mutedText, fontSize: 12),
        ),
        const SizedBox(height: 30),
        _SyncOverviewHeader(
          state: state,
          activeProvider: providers
              .where((provider) => provider.active)
              .firstOrNull,
        ),
        const SizedBox(height: 12),
        _SyncPreferencesPanel(state: state),
        if (state._githubSyncStatus case final String message) ...[
          const SizedBox(height: 12),
          _SyncStatusMessage(
            message: message,
            isError: state._githubSyncStatusIsError,
          ),
        ],
        const SizedBox(height: 20),
        _SettingsSection(
          icon: LucideIcons.cloud,
          title: 'Providers',
          localizationKey: 'common.label.providers',
          trailing: _SettingsOutlineButton(
            label: 'settings.label.addProvider',
            leading: const Icon(LucideIcons.plus, size: 14),
            onTap: busy ? null : state._showAddSyncProviderCatalog,
          ),
          child: Container(
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: _softOutline),
            ),
            child: Column(
              children: [
                if (providers.isEmpty)
                  Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 10,
                    ),
                    child: Text(
                      tr('settings.sync.providers.empty'),
                      style: TextStyle(color: _mutedText, fontSize: 12),
                    ),
                  ),
                for (final (index, provider) in providers.indexed) ...[
                  if (index > 0)
                    Divider(height: 1, thickness: 1, color: _softOutline),
                  provider,
                ],
              ],
            ),
          ),
        ),
      ],
    ),
  );
}

Widget _buildGithubAuthenticationSettings(
  _SettingsPanelState state,
  VoidCallback onDisconnect,
) {
  return _SettingsSection(
    icon: LucideIcons.badgeCheck,
    title: 'settings.label.githubAuthentication',
    child: Column(
      children: [
        _SettingsRow(
          title: 'settings.label.accessToken',
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
                      label: 'common.action.disconnect',
                      onTap: state._githubSyncRunning ? null : onDisconnect,
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

Widget _buildGithubGistAuthorizationSettings(
  _SettingsPanelState state,
  VoidCallback onDisconnect,
) {
  return _SettingsSection(
    icon: LucideIcons.badgeCheck,
    title: 'settings.label.githubAuthorization',
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
              label: 'common.action.disconnect',
              onTap: state._githubSyncRunning ? null : onDisconnect,
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
    title: 'common.label.repository',
    child: Column(
      children: [
        _SettingsRow(
          title: 'settings.label.repositoryUrl',
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
          title: 'settings.label.branch',
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
          title: 'common.label.path',
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
            label: 'settings.label.saveRepository',
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
              hint: 'settings.label.createAutomatically',
              onChanged: (_) => state._mutate(() {}),
            ),
          ),
          SizedBox(height: 18),
          _SettingsRow(
            title: 'common.label.filename',
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
              label: 'settings.label.saveGistSettings',
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
    required this.state,
    required this.activeProvider,
  });

  final _SettingsPanelState state;
  final _SyncProviderRow? activeProvider;

  @override
  Widget build(BuildContext context) {
    final remoteRevision = state._remoteSyncRevision;
    final inSync =
        remoteRevision != null && state._syncRevision == remoteRevision;
    final busy = state._githubSyncRunning;
    final refreshing = state._remoteSyncStatusRefreshing;
    final (statusKey, color) = switch ((
      busy,
      state._githubSyncStatusIsError,
      activeProvider != null,
      inSync,
    )) {
      (true, _, _, _) => ('settings.sync.status.syncing', _primary),
      (_, true, _, _) => (
        'settings.sync.status.attentionNeeded',
        const Color(0xffef4444),
      ),
      (_, _, false, _) => ('settings.sync.status.noActiveProvider', _mutedText),
      (_, _, _, true) => ('settings.sync.status.upToDate', _secondary),
      _ => ('settings.sync.status.changesPending', const Color(0xffd97706)),
    };
    final keyHint = tr(
      state._hasLocalSyncKey
          ? 'settings.sync.status.keySaved'
          : 'settings.sync.status.keyNotSaved',
    );
    final details = Row(
      children: [
        Container(
          width: 36,
          height: 36,
          decoration: BoxDecoration(
            color: color.withValues(alpha: 0.14),
            borderRadius: BorderRadius.circular(10),
          ),
          alignment: Alignment.center,
          child: Icon(LucideIcons.cloud, size: 18, color: color),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Flexible(
                    child: Text(
                      tr(statusKey),
                      style: TextStyle(
                        color: _text,
                        fontSize: 14,
                        fontWeight: NautermFontWeights.semibold,
                      ),
                    ),
                  ),
                  const SizedBox(width: 6),
                  _SyncStatusDot(color: color),
                  if (refreshing) ...[
                    const SizedBox(width: 6),
                    const SizedBox.square(
                      dimension: 12,
                      child: CircularProgressIndicator(strokeWidth: 1.5),
                    ),
                  ],
                ],
              ),
              const SizedBox(height: 3),
              Wrap(
                crossAxisAlignment: WrapCrossAlignment.center,
                spacing: 6,
                runSpacing: 3,
                children: [
                  Text(
                    activeProvider == null
                        ? tr('settings.sync.status.addOrActivateProvider')
                        : tr(
                            'settings.sync.status.viaProvider',
                            args: {'provider': activeProvider!.name},
                          ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(color: _mutedText, fontSize: 11),
                  ),
                  Wrap(
                    crossAxisAlignment: WrapCrossAlignment.center,
                    spacing: 6,
                    children: [
                      _SyncVersionValue(
                        labelKey: 'common.label.local',
                        revision: state._syncRevision,
                        snapshotId: state._syncSnapshotId,
                      ),
                      Icon(
                        LucideIcons.arrowLeftRight,
                        size: 11,
                        color: _mutedText,
                      ),
                      _SyncVersionValue(
                        labelKey: 'common.label.remote',
                        revision: remoteRevision,
                        snapshotId: state._remoteSyncSnapshotId,
                        color: inSync || remoteRevision == null ? null : color,
                      ),
                    ],
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
    final actions = Wrap(
      crossAxisAlignment: WrapCrossAlignment.center,
      alignment: WrapAlignment.end,
      spacing: 6,
      runSpacing: 6,
      children: [
        Tooltip(
          message: keyHint,
          child: TextButton.icon(
            onPressed: busy ? null : state._showSyncKeyDetails,
            style: TextButton.styleFrom(
              foregroundColor: _text,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(7),
              ),
            ),
            icon: Stack(
              clipBehavior: Clip.none,
              children: [
                const Icon(LucideIcons.keyRound, size: 16),
                Positioned(
                  right: -2,
                  top: -2,
                  child: _SyncStatusDot(
                    color: state._hasLocalSyncKey
                        ? _secondary
                        : const Color(0xffd97706),
                  ),
                ),
              ],
            ),
            label: Text(tr('common.label.key')),
          ),
        ),
        _SyncRowAction(
          icon: LucideIcons.rotateCcw,
          labelKey: 'settings.sync.action.refreshStatus',
          onTap: busy || refreshing
              ? null
              : () => state._refreshRemoteSyncStatus(manual: true),
        ),
        FilledButton.icon(
          key: const ValueKey('settings-sync-now'),
          onPressed: busy || refreshing || activeProvider?.connected != true
              ? null
              : activeProvider?.onSync,
          style: FilledButton.styleFrom(
            backgroundColor: _primary,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(7),
            ),
          ),
          icon: const Icon(LucideIcons.refreshCw, size: 16),
          label: Text(
            tr(
              busy
                  ? 'settings.sync.status.syncing'
                  : 'settings.sync.action.syncNow',
            ),
          ),
        ),
      ],
    );
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < 550) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              details,
              const SizedBox(height: 8),
              Align(alignment: Alignment.centerRight, child: actions),
            ],
          );
        }
        return Row(
          children: [
            Expanded(child: details),
            const SizedBox(width: 12),
            actions,
          ],
        );
      },
    );
  }
}

class _SyncProviderRow extends StatelessWidget {
  const _SyncProviderRow({
    super.key,
    required this.name,
    required this.kind,
    required this.connected,
    required this.active,
    required this.busy,
    required this.onOpen,
    required this.onActivate,
    this.onSync,
    this.onHistory,
  });

  final String name;
  final String kind;
  final bool connected;
  final bool active;
  final bool busy;
  final VoidCallback onOpen;
  final VoidCallback onActivate;
  final VoidCallback? onSync;
  final VoidCallback? onHistory;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: busy ? null : onOpen,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
          child: Row(
            children: [
              _SyncStatusDot(
                color: !connected
                    ? const Color(0xffd97706)
                    : active
                    ? _secondary
                    : _mutedText,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Wrap(
                  spacing: 8,
                  runSpacing: 2,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    Text(
                      name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: _text,
                        fontSize: 14,
                        fontWeight: NautermFontWeights.medium,
                      ),
                    ),
                    if (kind != name || !connected)
                      Text(
                        connected
                            ? kind
                            : tr(
                                'settings.sync.provider.status.credentialsRequired',
                                args: {'provider': kind},
                              ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(color: _mutedText, fontSize: 11),
                      ),
                  ],
                ),
              ),
              const SizedBox(width: 6),
              if (active)
                Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 6,
                    vertical: 1,
                  ),
                  decoration: BoxDecoration(
                    border: Border.all(color: _primary),
                    borderRadius: BorderRadius.circular(4),
                  ),
                  child: Text(
                    tr('settings.ai.provider.active').toUpperCase(),
                    style: TextStyle(color: _primary, fontSize: 10),
                  ),
                )
              else
                TextButton(
                  onPressed: busy || !connected ? null : onActivate,
                  style: TextButton.styleFrom(
                    minimumSize: const Size(0, 30),
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    foregroundColor: _text,
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(7),
                    ),
                  ),
                  child: Text(tr('settings.ai.provider.use')),
                ),
              if (onHistory != null)
                _SyncRowAction(
                  icon: LucideIcons.clock,
                  labelKey: 'settings.sync.action.versionHistory',
                  onTap: busy || !connected ? null : onHistory,
                ),
              _SyncRowAction(
                icon: LucideIcons.settings,
                labelKey: 'settings.sync.action.configureProvider',
                onTap: busy ? null : onOpen,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _SyncStatusDot extends StatelessWidget {
  const _SyncStatusDot({required this.color});

  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 6,
      height: 6,
      decoration: BoxDecoration(color: color, shape: BoxShape.circle),
    );
  }
}

class _SyncRowAction extends StatelessWidget {
  const _SyncRowAction({
    required this.icon,
    required this.labelKey,
    required this.onTap,
  });

  final IconData icon;
  final String labelKey;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return SizedBox.square(
      dimension: 30,
      child: IconButton(
        tooltip: tr(labelKey),
        onPressed: onTap,
        padding: EdgeInsets.zero,
        style: IconButton.styleFrom(
          foregroundColor: _text,
          disabledForegroundColor: _faintText,
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(7)),
        ),
        icon: Icon(icon, size: 15),
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
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: _surface,
        borderRadius: BorderRadius.circular(8),
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
          const SizedBox(height: 10),
          _SettingsRow(
            localizationKey: 'settings.sync.automatic',
            title: 'Automatic Sync',
            subtitle: 'Sync the active provider on the selected interval.',
            trailing: Row(
              children: [
                Expanded(
                  child: Semantics(
                    label: tr('settings.sync.automatic.interval.label'),
                    child: _SettingsTextField(
                      controller: state._autoSyncMinutesController,
                      hint:
                          '${nautermDefaultSyncIntervalMilliseconds ~/ 60000}',
                      suffixText: tr(
                        'settings.sync.automatic.unit',
                        fallback: 'minutes',
                      ),
                      onChanged: (_) => state._scheduleSyncPreferencesSave(
                        wakeAutoSync: true,
                      ),
                      onSubmitted: (_) =>
                          state._persistSyncPreferences(wakeAutoSync: true),
                    ),
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
          const SizedBox(height: 10),
          _SettingsRow(
            title: 'settings.label.localBackups',
            subtitle: 'settings.sync.localBackups.description',
            trailing: Row(
              children: [
                Expanded(
                  child: Semantics(
                    label: tr('settings.sync.localBackups.retention.label'),
                    child: _SettingsTextField(
                      controller: state._syncBackupCountController,
                      hint: '10',
                      suffixText: tr('settings.sync.localBackups.unit'),
                      onChanged: (_) => state._scheduleSyncPreferencesSave(
                        wakeAutoSync: false,
                      ),
                      onSubmitted: (_) => state._persistSyncPreferences(),
                    ),
                  ),
                ),
                SizedBox(width: 8),
                _SettingsOutlineButton(
                  label: 'common.action.browse',
                  leading: const Icon(LucideIcons.clock, size: 14),
                  onTap: state._githubSyncRunning
                      ? null
                      : state._showLocalSyncBackups,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _SyncVersionValue extends StatelessWidget {
  const _SyncVersionValue({
    required this.labelKey,
    required this.revision,
    required this.snapshotId,
    this.color,
  });

  final String labelKey;
  final int? revision;
  final String? snapshotId;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final id = snapshotId;
    return Text.rich(
      TextSpan(
        text: '${tr(labelKey)} ',
        children: [
          TextSpan(
            text: revision == null ? '—' : 'r$revision',
            style: TextStyle(
              color: color ?? _text,
              fontWeight: NautermFontWeights.semibold,
            ),
          ),
          if (id != null && id.isNotEmpty)
            TextSpan(text: ' ${id.length > 7 ? id.substring(0, 7) : id}'),
        ],
      ),
      style: TextStyle(color: _mutedText, fontSize: 11),
    );
  }
}

String _syncCompletionMessage(
  Map<String, dynamic> result, {
  String statusKey = 'settings.sync.result.complete',
  String? remoteDetail,
}) {
  final summary = tr(
    'settings.sync.result.summary',
    args: {
      'status': tr(statusKey),
      'revision': result['revision'],
      'imported': result['imported_records'],
      'total': result['total_records'],
    },
  );
  return remoteDetail == null ? summary : '$summary $remoteDetail';
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
        isError ? message : tr(message),
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
    title: 'settings.label.authorization',
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
    title: 'settings.label.webdavConnection',
    child: ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: _settingsContentMaxWidth),
      child: Column(
        children: [
          _SettingsRow(
            title: 'settings.label.serverUrl',
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
            title: 'common.label.username',
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
            title: 'common.label.password',
            subtitle: 'Password or application-specific password.',
            showSubtitle: false,
            trailing: _SettingsTextField(
              controller: state._webdavPasswordController,
              obscureText: true,
              revealable: true,
              hint: 'common.label.password',
              onChanged: (_) => state._mutate(() {}),
            ),
          ),
          SizedBox(height: 18),
          _SettingsRow(
            title: 'settings.label.remotePath',
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

Widget _buildS3Settings(_SettingsPanelState state, VoidCallback onDisconnect) {
  return ConstrainedBox(
    constraints: const BoxConstraints(maxWidth: _settingsContentMaxWidth),
    child: Column(
      children: [
        _SettingsRow(
          title: 'common.label.endpoint',
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
          title: 'common.label.region',
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
          title: 'settings.label.bucket',
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
          title: 'settings.label.accessKeyId',
          subtitle: 'S3 API credential.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._s3AccessKeyController,
            obscureText: true,
            revealable: true,
            hint: 'settings.label.accessKey',
            onChanged: (_) =>
                state._mutate(() => state._s3CredentialsDirty = true),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          title: 'settings.label.secretAccessKey',
          subtitle: 'Stored only in the encrypted local database.',
          showSubtitle: false,
          trailing: _SettingsTextField(
            controller: state._s3SecretKeyController,
            obscureText: true,
            revealable: true,
            hint: 'settings.label.secretKey',
            onChanged: (_) =>
                state._mutate(() => state._s3CredentialsDirty = true),
          ),
        ),
        SizedBox(height: 18),
        _SettingsRow(
          title: 'common.label.folder',
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
          title: 'common.label.filename',
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
                label: 'common.action.disconnect',
                onTap: state._githubSyncRunning ? null : onDisconnect,
              ),
              SizedBox(width: 8),
            ],
            _SettingsOutlineButton(
              label: state._s3HasCredentials
                  ? 'settings.sync.action.saveSettings'
                  : 'Connect S3',
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
      padding: _settingsContentPadding(state.context),
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
            fallback: 'Use the same Master Key on every device. {appName} uses it only to wrap or unwrap the random Sync DEK; it never stores the Master Key.',
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
          localizationKey: 'settings.label.unlockSync',
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxWidth: _settingsContentMaxWidth,
            ),
            child: Column(
              children: [
                _SettingsRow(
                  title: 'common.label.masterKey',
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
                  title: 'settings.label.confirmMasterKey',
                  subtitle: 'settings.sync.masterKey.confirm.description',
                  trailing: _SettingsTextField(
                    key: const ValueKey('settings-sync-master-key-confirm'),
                    controller: state._syncMasterKeyConfirmController,
                    obscureText: true,
                    hint: 'settings.label.repeatMasterKey',
                    onChanged: (_) => state._mutate(() {}),
                    onSubmitted: (_) => state._continueToSyncManagement(),
                  ),
                ),
                SizedBox(height: 18),
                Align(
                  alignment: Alignment.centerRight,
                  child: _SettingsOutlineButton(
                    label: 'common.action.continue',
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
  String get _githubSyncDatabasePath =>
      widget.syncDatabasePath ?? NautermPaths.resolve().databasePath;
  int get _syncBackupCount =>
      (int.tryParse(_syncBackupCountController.text.trim()) ?? 10).clamp(
        0,
        100,
      );

  Future<void> _refreshRemoteSyncStatus({bool manual = false}) async {
    if (_remoteSyncStatusRefreshing) return;
    _mutate(() => _remoteSyncStatusRefreshing = true);
    if (manual) _clearGithubSyncStatus();
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
      if (manual) _setGithubSyncError(error.toString());
    } finally {
      if (mounted) _mutate(() => _remoteSyncStatusRefreshing = false);
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
      _syncManagementUnlocked =
          _hasLocalSyncKey ||
          (_syncManagementUnlocked && _syncMasterKeyController.text.isNotEmpty);
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

  List<String> get _connectedSyncProviderNames => [
    for (final provider in _visibleBuiltInSyncProviders)
      if (_isSyncProviderConnected(provider)) provider.label,
    for (final provider in _cloudProviders)
      if (provider.hasCredentials) provider.name,
  ];

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
    final disconnect = await showNautermDialog<bool>(
      context: context,
      builder: (_) => _SyncProviderDialog(provider: provider, state: this),
    );
    if (!mounted) return;
    if (disconnect == true && !_githubSyncRunning) {
      switch (provider) {
        case _SyncProvider.githubRepository:
          await _removeGithubToken();
        case _SyncProvider.githubGist:
          await _removeGithubGistToken();
        case _SyncProvider.s3Compatible:
          await _removeS3Credentials();
        default:
          break;
      }
      if (!mounted) return;
    }
    if (provider == _SyncProvider.githubRepository) {
      _githubTokenController.clear();
    } else if (provider == _SyncProvider.s3Compatible) {
      _s3AccessKeyController.clear();
      _s3SecretKeyController.clear();
      _s3CredentialsDirty = false;
    }
  }

  Future<void> _showGithubSyncHistory() => showNautermDialog<void>(
    context: context,
    builder: (_) => _SyncHistoryDialog(
      databasePath: _githubSyncDatabasePath,
      providerName: _SyncProvider.githubRepository.label,
      scope: 'github_repository',
      backupCount: _syncBackupCount,
      onRestore: _restoreGithubRevision,
    ),
  );

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
            _githubSyncStatus = tr('settings.sync.backup.restore.success');
            _githubSyncStatusIsError = false;
          });
        },
      ),
    );
  }

  Future<void> _showGithubGistHistory() => showNautermDialog<void>(
    context: context,
    builder: (_) => _SyncHistoryDialog(
      databasePath: _githubSyncDatabasePath,
      providerName: _SyncProvider.githubGist.label,
      scope: 'github_gist',
      backupCount: _syncBackupCount,
      onRestore: _restoreGithubGistVersion,
    ),
  );

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
        _githubSyncStatus = _syncCompletionMessage(
          result,
          statusKey: 'settings.sync.result.cloudRestored',
        );
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
        _githubSyncStatus = _syncCompletionMessage(
          result,
          statusKey: 'settings.sync.result.cloudRestored',
        );
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
        connectedProviders: _connectedSyncProviderNames,
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
          builder: (_) => _ChangeSyncMasterKeyDialog(
            databasePath: _githubSyncDatabasePath,
            backupCount: _syncBackupCount,
            onChange: _changeSyncMasterKey,
          ),
        );
      case _SyncKeyAction.forgetMasterKey:
        final confirmed = await showNautermDialog<bool>(
          context: context,
          builder: (dialogContext) => _SyncDialogFrame(
            icon: LucideIcons.keyRound,
            title: dialogContext.tr(
              'settings.sync.forgetKey.title',
              fallback: 'Forget Master Key?',
            ),
            subtitle: dialogContext.tr(
              'settings.sync.forgetKey.summary',
              fallback: 'Clear the local sync key cache.',
            ),
            width: 400,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  dialogContext.tr(
                    'settings.sync.forgetKey.description',
                    fallback:
                        'Enter the Master Key again the next time you sync. '
                        'Your provider credentials, local data, and cloud files are kept.',
                  ),
                  style: TextStyle(color: _text, fontSize: 12, height: 1.45),
                ),
                const SizedBox(height: 18),
                Wrap(
                  alignment: WrapAlignment.end,
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    _SettingsOutlineButton(
                      label: dialogContext.tr(
                        'common.action.cancel',
                        fallback: 'Cancel',
                      ),
                      onTap: () => Navigator.of(dialogContext).pop(false),
                    ),
                    SizedBox(
                      height: 30,
                      child: FilledButton(
                        key: const ValueKey('settings-sync-confirm-forget-key'),
                        onPressed: () => Navigator.of(dialogContext).pop(true),
                        style: FilledButton.styleFrom(
                          backgroundColor: const Color(0xffdc2626),
                          foregroundColor: Colors.white,
                          padding: const EdgeInsets.symmetric(horizontal: 11),
                          shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(7),
                          ),
                          textStyle: TextStyle(
                            fontSize: 12,
                            fontWeight: NautermFontWeights.medium,
                          ),
                        ),
                        child: Text(
                          dialogContext.tr(
                            'settings.label.forgetMasterKey',
                            fallback: 'Forget Master Key',
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        );
        if (confirmed == true && mounted) {
          await _forgetSyncMasterKey();
        }
    }
  }

  Future<void> _forgetSyncMasterKey() async {
    if (_githubSyncRunning) return;
    _mutate(() => _githubSyncRunning = true);
    try {
      await _forgetSyncKeyInBackground(_githubSyncDatabasePath);
      if (!mounted) return;
      _syncMasterKeyController.clear();
      _syncMasterKeyConfirmController.clear();
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
    } finally {
      if (mounted) _mutate(() => _githubSyncRunning = false);
    }
  }

  Future<SyncKeyRotationResult> _changeSyncMasterKey(
    String reviewStamp,
    String currentMasterKey,
    String newMasterKey,
    int backupCount,
    ValueChanged<SyncKeyRotationProgress> onProgress,
  ) async {
    if (_githubSyncRunning) {
      throw StateError('A sync operation is already running.');
    }
    _mutate(() => _githubSyncRunning = true);
    final databasePath = _githubSyncDatabasePath;
    try {
      final runner =
          settingsSyncKeyRotationRunnerOverride ?? _runSyncKeyRotation;
      final result = await runner(
        databasePath,
        reviewStamp,
        currentMasterKey,
        newMasterKey,
        backupCount,
        onProgress,
      );
      notifyNautermSyncCompleted();
      notifyNautermDatabaseBulkChange();
      if (mounted) {
        _mutate(() {
          if (result.hasLocalKey case final hasLocalKey?) {
            _hasLocalSyncKey = hasLocalKey;
            _syncManagementUnlocked = _syncManagementUnlocked || hasLocalKey;
          }
          if (result.failed == null) {
            _githubSyncStatus = context.tr('settings.sync.keyRotation.success');
            _githubSyncStatusIsError = false;
          }
        });
      }
      return result;
    } finally {
      _finishSyncAttempt();
    }
  }

  Future<void> _loadGithubSyncSettings() async {
    try {
      final databasePath = _githubSyncDatabasePath;
      final result = await _loadGithubSyncSettingsInBackground(databasePath);
      if (!mounted) return;
      _applyGithubSyncSettings(result, notify: true);
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'sync',
        'Unable to load sync storage.',
        error: error,
        stackTrace: stackTrace,
      );
      if (mounted) {
        _setGithubSyncError(
          context.tr('settings.sync.error.storageUnavailable'),
        );
      }
    }
  }

  Future<void> _saveGithubToken() async {
    final token = _githubTokenController.text.trim();
    if (token.isEmpty) {
      _setGithubSyncError(
        tr(
          'settings.sync.error.githubTokenRequired',
          fallback: 'Enter a GitHub PAT to save.',
        ),
      );
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
      final wasActive = _activeSyncProviderId == 'github_repository';
      _mutate(() {
        _githubHasToken = false;
        _githubTokenController.clear();
        _githubSyncStatus = 'GitHub token removed.';
        _githubSyncStatusIsError = false;
        if (wasActive) _activeSyncProviderId = null;
      });
      if (wasActive) await _persistSyncPreferences();
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
      final wasActive = _activeSyncProviderId == 'github_gist';
      _mutate(() {
        _githubGistHasToken = false;
        _githubSyncStatus = 'GitHub Gist disconnected.';
        _githubSyncStatusIsError = false;
        if (wasActive) _activeSyncProviderId = null;
      });
      if (wasActive) await _persistSyncPreferences();
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
      _setGithubSyncError(
        tr(
          'settings.sync.error.repositoryUrlRequired',
          fallback: 'Repository URL is required.',
        ),
      );
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
      _setGithubSyncError(
        tr(
          'settings.sync.error.s3EndpointRequired',
          fallback: 'S3 endpoint is required.',
        ),
      );
      return;
    }
    if (bucket.isEmpty) {
      _setGithubSyncError(
        tr(
          'settings.sync.error.s3BucketRequired',
          fallback: 'S3 bucket is required.',
        ),
      );
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
        tr(
          'settings.sync.error.s3CredentialsRequired',
          fallback: 'Enter both the S3 Access Key ID and Secret Access Key.',
        ),
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

  Future<void> _removeS3Credentials() async {
    try {
      await _removeS3CredentialsInBackground(_githubSyncDatabasePath);
      if (!mounted) return;
      final wasActive = _activeSyncProviderId == 's3';
      _mutate(() {
        _s3HasCredentials = false;
        _s3CredentialsDirty = false;
        _s3AccessKeyController.clear();
        _s3SecretKeyController.clear();
        _githubSyncStatus = 'S3-compatible storage disconnected.';
        _githubSyncStatusIsError = false;
        if (wasActive) _activeSyncProviderId = null;
      });
      if (wasActive) await _persistSyncPreferences();
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
      _setGithubSyncError(
        tr(
          'settings.sync.masterKey.error.required',
          fallback: 'Enter the Master Key.',
        ),
      );
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
      _setGithubSyncError(
        tr(
          'settings.sync.masterKey.error.confirmationRequired',
          fallback: 'Confirm the Master Key.',
        ),
      );
      return;
    }
    if (enteringMasterKey && masterKey != masterKeyConfirm) {
      _setGithubSyncError(
        tr(
          'settings.sync.masterKey.error.mismatch',
          fallback: 'Master Key confirmation does not match.',
        ),
      );
      return;
    }
    if (!_githubHasToken) {
      _setGithubSyncError(
        tr(
          'settings.sync.error.githubTokenMissing',
          fallback: 'Save a GitHub token first.',
        ),
      );
      return;
    }
    if (_githubRepositoryUrlController.text.trim().isEmpty) {
      _setGithubSyncError(
        tr(
          'settings.sync.error.repositoryMissing',
          fallback: 'Configure the repository first.',
        ),
      );
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
        _githubSyncStatus = _syncCompletionMessage(
          result,
          remoteDetail: shortSha.isEmpty
              ? null
              : tr('settings.sync.result.commit', args: {'commit': shortSha}),
        );
      });
      if (enteringMasterKey) {
        _cacheGithubSyncSetting(databasePath, 'hasLocalSyncKey', true);
      }
      notifyNautermDatabaseBulkChange();
    } on Object catch (error) {
      if (!mounted) return;
      _setGithubSyncError(error.toString());
    } finally {
      _finishSyncAttempt();
    }
  }

  Future<void> _runGithubGistSync() async {
    final masterKey = _syncMasterKeyController.text;
    final masterKeyConfirm = _syncMasterKeyConfirmController.text;
    final enteringMasterKey = masterKey.isNotEmpty;
    if (!enteringMasterKey && !_hasLocalSyncKey) {
      _setGithubSyncError(
        tr(
          'settings.sync.masterKey.error.required',
          fallback: 'Enter the Master Key.',
        ),
      );
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
      _setGithubSyncError(
        tr(
          'settings.sync.masterKey.error.confirmationRequired',
          fallback: 'Confirm the Master Key.',
        ),
      );
      return;
    }
    if (enteringMasterKey && masterKey != masterKeyConfirm) {
      _setGithubSyncError(
        tr(
          'settings.sync.masterKey.error.mismatch',
          fallback: 'Master Key confirmation does not match.',
        ),
      );
      return;
    }
    if (!_githubGistHasToken) {
      _setGithubSyncError(
        tr(
          'settings.sync.error.gistConnectionRequired',
          fallback: 'Connect GitHub Gist first.',
        ),
      );
      return;
    }
    _mutate(() {
      _githubSyncRunning = true;
      _githubSyncStatus = null;
      _githubSyncStatusIsError = false;
    });
    try {
      final result = await _runGithubGistSyncInBackground(
        _githubSyncDatabasePath,
        _githubGistIdController.text.trim(),
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
        _githubSyncStatus = _syncCompletionMessage(
          result,
          remoteDetail: shortVersion.isEmpty
              ? null
              : tr(
                  'settings.sync.result.remoteRevision',
                  args: {'revision': shortVersion},
                ),
        );
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
    } on Object catch (error) {
      if (mounted) {
        _setGithubSyncError(error.toString());
      }
    } finally {
      _finishSyncAttempt();
    }
  }

  Future<void> _runS3Sync() async {
    final masterKey = _syncMasterKeyController.text;
    final masterKeyConfirm = _syncMasterKeyConfirmController.text;
    final enteringMasterKey = masterKey.isNotEmpty;
    if (!enteringMasterKey && !_hasLocalSyncKey) {
      _setGithubSyncError(
        tr(
          'settings.sync.masterKey.error.required',
          fallback: 'Enter the Master Key.',
        ),
      );
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
      _setGithubSyncError(
        tr(
          'settings.sync.masterKey.error.mismatch',
          fallback: 'Master Key confirmation does not match.',
        ),
      );
      return;
    }
    if (!_isSyncProviderConnected(_SyncProvider.s3Compatible)) {
      _setGithubSyncError(
        tr(
          'settings.sync.error.s3ConnectionRequired',
          fallback: 'Connect S3-compatible storage first.',
        ),
      );
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
      _mutate(() {
        _rememberSyncSnapshot(
          revision as int?,
          result['snapshot_id'] as String?,
        );
        if (enteringMasterKey) {
          _hasLocalSyncKey = true;
        }
        _githubSyncStatus = _syncCompletionMessage(
          result,
          remoteDetail: shortEtag.isEmpty
              ? null
              : tr('settings.sync.result.etag', args: {'etag': shortEtag}),
        );
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
      _finishSyncAttempt();
    }
  }

  void _finishSyncAttempt() {
    if (!mounted) return;
    if (_hasLocalSyncKey) {
      _syncMasterKeyController.clear();
      _syncMasterKeyConfirmController.clear();
    }
    _mutate(() => _githubSyncRunning = false);
  }

  void _showProviderConnectionRequirement(String provider) {
    _setGithubSyncError(
      tr(
        'settings.sync.error.oauthClientRequired',
        fallback: '{provider} authorization requires a {appName} OAuth client ID before sign-in can be enabled.',
        args: {'provider': provider},
      ),
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
      _setGithubSyncError(
        tr(
          'settings.sync.masterKey.error.mismatch',
          fallback: 'Master Key confirmation does not match.',
        ),
      );
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
      title: 'settings.label.connectToGithub',
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
            hint: 'settings.label.createAutomatically',
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
            child: _SettingsOutlineButton(
              label: 'settings.label.tryAgain',
              onTap: _begin,
            ),
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
                label: Text(
                  _copied
                      ? context.tr('common.status.copied')
                      : context.tr('common.action.copyCode'),
                ),
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

class _SyncProviderDialog extends StatefulWidget {
  const _SyncProviderDialog({required this.provider, required this.state});

  final _SyncProvider provider;
  final _SettingsPanelState state;

  @override
  State<_SyncProviderDialog> createState() => _SyncProviderDialogState();
}

class _SyncProviderDialogState extends State<_SyncProviderDialog> {
  bool _confirmingDisconnect = false;
  _SyncProvider get provider => widget.provider;

  void _requestDisconnect() => setState(() => _confirmingDisconnect = true);

  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    if (_confirmingDisconnect) {
      return _buildSyncProviderRemovalConfirmation(
        context,
        name: context.tr(provider.label),
        disconnect: true,
        onCancel: () => setState(() => _confirmingDisconnect = false),
        onConfirm: () => Navigator.of(context).pop(true),
      );
    }
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
            _buildGithubAuthenticationSettings(state, _requestDisconnect),
            SizedBox(height: 24),
          ],
          if (provider == _SyncProvider.githubGist) ...[
            _buildGithubGistAuthorizationSettings(state, _requestDisconnect),
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
            _SyncProvider.s3Compatible => _buildS3Settings(
              state,
              _requestDisconnect,
            ),
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

Widget _buildSyncActionConfirmation(
  BuildContext context, {
  required String titleKey,
  required String description,
  required VoidCallback onCancel,
  required VoidCallback onConfirm,
  String actionKey = 'common.action.restore',
  IconData icon = LucideIcons.history,
  String? notice,
}) {
  return _SyncDialogFrame(
    icon: icon,
    title: titleKey,
    subtitle: '',
    width: 420,
    onClose: onCancel,
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          description,
          style: TextStyle(color: _text, fontSize: 12, height: 1.45),
        ),
        if (notice != null) ...[
          const SizedBox(height: 8),
          Text(notice, style: TextStyle(color: _mutedText, fontSize: 12)),
        ],
        const SizedBox(height: 18),
        Wrap(
          alignment: WrapAlignment.end,
          spacing: 8,
          runSpacing: 8,
          children: [
            _SettingsOutlineButton(
              label: 'common.action.cancel',
              onTap: onCancel,
            ),
            SizedBox(
              height: 30,
              child: FilledButton(
                style: FilledButton.styleFrom(
                  backgroundColor: _primary,
                  padding: const EdgeInsets.symmetric(horizontal: 11),
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(7),
                  ),
                  textStyle: TextStyle(
                    fontSize: 12,
                    fontWeight: NautermFontWeights.medium,
                  ),
                ),
                onPressed: onConfirm,
                child: Text(context.tr(actionKey)),
              ),
            ),
          ],
        ),
      ],
    ),
  );
}

Widget _buildSyncProviderRemovalConfirmation(
  BuildContext context, {
  required String name,
  required bool disconnect,
  required VoidCallback onCancel,
  required VoidCallback onConfirm,
}) {
  return _buildSyncActionConfirmation(
    context,
    titleKey: disconnect
        ? 'settings.sync.provider.disconnect.title'
        : 'settings.sync.provider.delete.title',
    description: context.tr(
      disconnect
          ? 'settings.sync.provider.disconnect.description'
          : 'settings.sync.provider.delete.description',
      args: {'name': name},
    ),
    actionKey: disconnect ? 'common.action.disconnect' : 'common.action.delete',
    icon: disconnect ? LucideIcons.unplug : LucideIcons.trash2,
    onCancel: onCancel,
    onConfirm: onConfirm,
  );
}

Future<bool> _confirmSyncRestore(
  BuildContext context, {
  required String titleKey,
  required String description,
  String? notice,
}) async {
  return await showNautermDialog<bool>(
        context: context,
        builder: (context) => _buildSyncActionConfirmation(
          context,
          titleKey: titleKey,
          description: description,
          notice: notice,
          onCancel: () => Navigator.of(context).pop(false),
          onConfirm: () => Navigator.of(context).pop(true),
        ),
      ) ??
      false;
}

Future<bool> _confirmCloudSyncRestore(
  BuildContext context, {
  required String providerName,
  required String versionLabel,
  required int backupCount,
}) {
  return _confirmSyncRestore(
    context,
    titleKey: 'settings.sync.cloudHistory.restore.title',
    description: context.tr(
      'settings.sync.cloudHistory.restore.description',
      args: {'version': versionLabel, 'provider': providerName},
    ),
    notice: context.tr(
      backupCount == 0
          ? 'settings.sync.cloudHistory.restore.backupDisabled'
          : 'settings.sync.cloudHistory.restore.backupNotice',
    ),
  );
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

  Future<void> _restore(Map<String, dynamic> backup) async {
    final backupId = backup['id'] as String;
    final confirmed = await _confirmSyncRestore(
      context,
      titleKey: 'settings.sync.localBackups.restore.title',
      description: context.tr(
        'settings.sync.localBackups.restore.description',
        args: {'date': _localSyncBackupDate(context, backup)},
      ),
    );
    if (!mounted || !confirmed || _restoring != null) return;
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
      title: 'settings.label.localBackups',
      subtitle: 'settings.sync.localBackups.listDescription',
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
                onRestore: () => _restore(backup),
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
                  '${_localSyncBackupDate(context, backup)} · ${_formatLocalBackupBytes(bytes)}',
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

String _localSyncBackupDate(BuildContext context, Map<String, dynamic> backup) {
  final timestamp = backup['created_at'];
  const maxTimestamp = 8640000000000000;
  if (timestamp is! int ||
      timestamp < -maxTimestamp ||
      timestamp > maxTimestamp) {
    return context.tr('common.label.unknownDate');
  }
  final date = DateTime.fromMillisecondsSinceEpoch(timestamp);
  final localization = MaterialLocalizations.of(context);
  final time = localization.formatTimeOfDay(TimeOfDay.fromDateTime(date));
  return '${localization.formatShortDate(date)} $time';
}

String _formatLocalBackupBytes(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}

typedef _SyncHistoryResult = ({
  List<Map<String, dynamic>> versions,
  bool canRestore,
});

@visibleForTesting
Future<({List<Map<String, dynamic>> versions, bool canRestore})> Function(
  String scope,
  String databasePath,
)?
settingsSyncHistoryLoaderOverride;

Future<_SyncHistoryResult> _readSyncHistoryInBackground(
  String databasePath,
  String scope,
) {
  if (settingsSyncHistoryLoaderOverride case final override?) {
    return override(scope, databasePath);
  }
  return Isolate.run(() {
    const limit = 100;
    final store = NautermDataStore.openPath(databasePath);
    try {
      final versions = switch (scope) {
        'github_repository' => store.githubListHistory(limit: limit),
        'github_gist' => store.githubGistListHistory(limit: limit),
        's3' => store.s3ListHistory(limit: limit),
        final scope when scope.startsWith('cloud:') => store.cloudListHistory(
          providerId: scope.substring('cloud:'.length),
          limit: limit,
        ),
        _ => throw ArgumentError.value(scope, 'scope'),
      };
      return (versions: versions, canRestore: store.hasLocalSyncKey());
    } finally {
      store.dispose();
    }
  });
}

class _SyncHistoryDialog extends StatefulWidget {
  const _SyncHistoryDialog({
    required this.databasePath,
    required this.providerName,
    required this.scope,
    required this.backupCount,
    required this.onRestore,
    this.emptyHint,
  });

  final String databasePath;
  final String providerName;
  final String scope;
  final int backupCount;
  final Future<void> Function(String version) onRestore;
  final String? emptyHint;

  bool get isObjectHistory => scope == 's3' || scope.startsWith('cloud:');

  @override
  State<_SyncHistoryDialog> createState() => _SyncHistoryDialogState();
}

class _SyncHistoryDialogState extends State<_SyncHistoryDialog> {
  _SyncHistoryResult? _history;
  String? _restoring;
  String? _error;
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    unawaited(_loadHistory());
  }

  Future<void> _loadHistory() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final history = await _readSyncHistoryInBackground(
        widget.databasePath,
        widget.scope,
      );
      if (mounted) setState(() => _history = history);
    } on Object catch (error) {
      if (mounted) {
        setState(() {
          _history = null;
          _error = error.toString();
        });
      }
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _restore(String revision) async {
    if (_history?.canRestore != true || _restoring != null) return;
    final confirmed = await _confirmCloudSyncRestore(
      context,
      providerName: widget.providerName,
      versionLabel: widget.isObjectHistory
          ? String.fromCharCodes(revision.runes.take(12))
          : _shortGitSyncRevision(revision),
      backupCount: widget.backupCount,
    );
    if (!mounted || !confirmed || _restoring != null) return;
    setState(() {
      _restoring = revision;
      _error = null;
    });
    try {
      await widget.onRestore(revision);
      if (!mounted) return;
      if (widget.isObjectHistory) {
        await _loadHistory();
      } else {
        Navigator.of(context).pop();
      }
    } on Object catch (error) {
      if (mounted) setState(() => _error = error.toString());
    } finally {
      if (mounted) setState(() => _restoring = null);
    }
  }

  @override
  Widget build(BuildContext context) {
    return _SyncDialogFrame(
      icon: LucideIcons.history,
      title: context.tr(
        'settings.sync.history.providerTitle',
        args: {'provider': widget.providerName},
      ),
      subtitle: 'settings.sync.history.description',
      width: 560,
      child: _buildHistory(context),
    );
  }

  Widget _buildHistory(BuildContext context) {
    if (_loading) {
      return Semantics(
        label: context.tr('settings.sync.history.loading'),
        child: const Padding(
          padding: EdgeInsets.symmetric(vertical: 30),
          child: Center(
            child: SizedBox(
              width: 18,
              height: 18,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
          ),
        ),
      );
    }

    final history = _history;
    final versions = history?.versions ?? const [];
    final canRestore = history?.canRestore == true;
    final disabled = !canRestore || _restoring != null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (_error case final error?) ...[
          Text(
            error,
            style: const TextStyle(color: Color(0xffdc2626), fontSize: 11),
          ),
          const SizedBox(height: 10),
          if (history == null)
            Align(
              alignment: Alignment.centerRight,
              child: _SettingsOutlineButton(
                label: context.tr('common.action.retry'),
                onTap: _loadHistory,
              ),
            ),
        ],
        if (history != null && !canRestore) ...[
          Text(
            context.tr('settings.sync.history.syncRequired'),
            style: TextStyle(color: _mutedText, fontSize: 12),
          ),
          const SizedBox(height: 10),
        ],
        if (history != null && versions.isEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 26),
            child: Column(
              children: [
                Icon(LucideIcons.history, size: 24, color: _faintText),
                const SizedBox(height: 9),
                Text(
                  context.tr('settings.sync.history.empty'),
                  style: TextStyle(color: _mutedText, fontSize: 12),
                ),
                if (widget.emptyHint case final hint?) ...[
                  const SizedBox(height: 4),
                  Text(
                    context.tr(hint),
                    textAlign: TextAlign.center,
                    style: TextStyle(color: _faintText, fontSize: 10),
                  ),
                ],
              ],
            ),
          ),
        for (var index = 0; index < versions.length; index++) ...[
          if (index > 0) const SizedBox(height: 7),
          if (widget.isObjectHistory)
            _CloudObjectVersionCard(
              version: versions[index],
              restoring: _restoring == versions[index]['version_id'],
              disabled: disabled,
              onRestore: versions[index]['is_current'] == true
                  ? null
                  : () => _restore(versions[index]['version_id'] as String),
            )
          else
            _SyncHistoryCard(
              revision: versions[index],
              label: index == 0
                  ? 'settings.sync.history.current'
                  : 'common.label.version',
              selected: index == 0,
              restoring: _restoring == versions[index]['sha'],
              onRestore: index == 0 || disabled ? null : _restore,
            ),
        ],
      ],
    );
  }
}

String _shortGitSyncRevision(String revision) =>
    revision.length > 7 ? revision.substring(0, 7) : revision;

class _SyncHistoryCard extends StatelessWidget {
  const _SyncHistoryCard({
    required this.revision,
    required this.label,
    required this.selected,
    required this.restoring,
    required this.onRestore,
  });

  final Map<String, dynamic> revision;
  final String label;
  final bool selected;
  final bool restoring;
  final Future<void> Function(String revision)? onRestore;

  @override
  Widget build(BuildContext context) {
    final sha = revision['sha'] as String? ?? '';
    final shortSha = _shortGitSyncRevision(sha);
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
          if (!selected) ...[
            SizedBox(width: 8),
            _SettingsOutlineButton(
              label: restoring
                  ? 'common.label.restoring'
                  : 'common.action.restore',
              onTap: onRestore == null || sha.isEmpty
                  ? null
                  : () => onRestore!(sha),
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
    required this.connectedProviders,
    required this.onChangeMasterKey,
    required this.onForgetMasterKey,
  });

  final bool hasLocalSyncKey;
  final List<String> connectedProviders;
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
          const SizedBox(height: 16),
          Text(
            context.tr(
              'settings.sync.key.connectedProviders',
              args: {'count': connectedProviders.length},
            ),
            style: TextStyle(
              color: _mutedText,
              fontSize: 11,
              fontWeight: NautermFontWeights.medium,
            ),
          ),
          if (connectedProviders.isEmpty)
            Padding(
              padding: const EdgeInsets.only(top: 6),
              child: Text(
                context.tr('settings.sync.key.noConnectedProviders'),
                style: TextStyle(color: _mutedText, fontSize: 12),
              ),
            ),
          for (final provider in connectedProviders)
            Padding(
              padding: const EdgeInsets.only(top: 6),
              child: Text(
                provider,
                style: TextStyle(color: _text, fontSize: 12),
              ),
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
                onTap: onForgetMasterKey,
              ),
              _SettingsOutlineButton(
                label: tr(
                  'settings.label.changeMasterKey',
                  fallback: 'Change Master Key',
                ),
                onTap: connectedProviders.isNotEmpty ? onChangeMasterKey : null,
              ),
            ],
          ),
        ],
      ),
    );
  }
}

@visibleForTesting
Future<SyncKeyRotationReview> Function(String databasePath)?
settingsSyncKeyRotationReviewOverride;

@visibleForTesting
Future<SyncKeyRotationResult> Function(
  String databasePath,
  String reviewStamp,
  String currentKey,
  String newKey,
  int backupCount,
  ValueChanged<SyncKeyRotationProgress> onProgress,
)?
settingsSyncKeyRotationRunnerOverride;

Future<SyncKeyRotationReview> _readSyncKeyRotationReview(String databasePath) {
  if (settingsSyncKeyRotationReviewOverride case final override?) {
    return override(databasePath);
  }
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.reviewSyncKeyRotation();
    } finally {
      store.dispose();
    }
  });
}

Future<SyncKeyRotationResult> _rotateSyncKeyInBackground(
  String databasePath,
  String reviewStamp,
  String currentKey,
  String newKey,
  int backupCount,
  SendPort progress,
) => Isolate.run(() {
  final store = NautermDataStore.openPath(databasePath);
  try {
    return store.rotateSyncMasterKey(
      reviewStamp: reviewStamp,
      currentMasterKey: currentKey,
      newMasterKey: newKey,
      backupCount: backupCount,
      onProgress: progress.send,
    );
  } finally {
    store.dispose();
  }
});

Future<SyncKeyRotationResult> _runSyncKeyRotation(
  String databasePath,
  String reviewStamp,
  String currentKey,
  String newKey,
  int backupCount,
  ValueChanged<SyncKeyRotationProgress> onProgress,
) async {
  final progress = ReceivePort();
  final subscription = progress.cast<SyncKeyRotationProgress>().listen(
    onProgress,
  );
  try {
    return await _rotateSyncKeyInBackground(
      databasePath,
      reviewStamp,
      currentKey,
      newKey,
      backupCount,
      progress.sendPort,
    );
  } finally {
    await subscription.cancel();
    progress.close();
  }
}

class _ChangeSyncMasterKeyDialog extends StatefulWidget {
  const _ChangeSyncMasterKeyDialog({
    required this.databasePath,
    required this.backupCount,
    required this.onChange,
  });

  final String databasePath;
  final int backupCount;
  final Future<SyncKeyRotationResult> Function(
    String reviewStamp,
    String currentKey,
    String newKey,
    int backupCount,
    ValueChanged<SyncKeyRotationProgress> onProgress,
  )
  onChange;

  @override
  State<_ChangeSyncMasterKeyDialog> createState() =>
      _ChangeSyncMasterKeyDialogState();
}

class _ChangeSyncMasterKeyDialogState
    extends State<_ChangeSyncMasterKeyDialog> {
  final TextEditingController _currentController = TextEditingController();
  final TextEditingController _newController = TextEditingController();
  final TextEditingController _confirmController = TextEditingController();
  final Map<String, SyncKeyRotationPhase> _phases = {};
  List<SyncKeyRotationTarget> _targets = const [];
  String? _reviewStamp;
  bool _loading = true;
  bool _running = false;
  bool _retrySamePair = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    unawaited(_reloadReview());
  }

  Future<void> _reloadReview() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final review = await _readSyncKeyRotationReview(widget.databasePath);
      if (!mounted) return;
      setState(() {
        _targets = review.targets;
        _reviewStamp = review.stamp;
        if (!_retrySamePair) _phases.clear();
      });
    } on Object catch (error) {
      if (mounted) setState(() => _error = error.toString());
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  void dispose() {
    _currentController.dispose();
    _newController.dispose();
    _confirmController.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final reviewStamp = _reviewStamp;
    if (_running || _loading || reviewStamp == null || _targets.isEmpty) {
      return;
    }
    final currentKey = _currentController.text;
    final newKey = _newController.text;
    final confirmation = _confirmController.text;
    String? error;
    final currentKeyError = _masterKeyValidationError(
      currentKey,
      subject: context.tr('settings.label.currentMasterKey'),
    );
    final newKeyError = _masterKeyValidationError(
      newKey,
      subject: context.tr('settings.label.newMasterKey'),
    );
    if (currentKeyError != null) {
      error = currentKeyError;
    } else if (newKeyError != null) {
      error = newKeyError;
    } else if (newKey == currentKey) {
      error = context.tr('settings.sync.masterKey.error.unchanged');
    } else if (newKey != confirmation) {
      error = context.tr('settings.sync.masterKey.error.newKeyMismatch');
    }
    if (error != null) {
      setState(() => _error = error);
      return;
    }

    setState(() {
      _running = true;
      _error = null;
      _phases.removeWhere((_, phase) => phase == SyncKeyRotationPhase.failed);
    });
    try {
      final outcome = await widget.onChange(
        reviewStamp,
        currentKey,
        newKey,
        widget.backupCount,
        (progress) {
          if (!mounted || !_running) return;
          setState(() {
            _retrySamePair = true;
            _phases[progress.scope] = progress.phase;
          });
        },
      );
      if (!mounted) return;
      final failure = outcome.failed;
      if (failure == null) {
        Navigator.of(context).pop();
        return;
      }
      final name = _targets
          .firstWhere((target) => target.scope == failure.scope)
          .name;
      setState(() {
        _retrySamePair = true;
        _reviewStamp = outcome.retryStamp;
        _phases[failure.scope] = SyncKeyRotationPhase.failed;
        for (final scope in outcome.completed) {
          _phases[scope] = SyncKeyRotationPhase.updated;
        }
        final summary = context.tr('settings.sync.keyRotation.failure');
        _error = '$summary\n$name: ${failure.error}';
      });
    } on Object catch (error) {
      if (mounted) {
        setState(() {
          _reviewStamp = null;
          _error = error.toString();
        });
      }
    } finally {
      if (mounted) setState(() => _running = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final fieldsLocked = _running || _retrySamePair;
    return PopScope(
      canPop: !_running,
      child: _SyncDialogFrame(
        icon: LucideIcons.keyRound,
        title: 'settings.label.changeMasterKey',
        subtitle: 'settings.sync.keyRotation.description',
        canClose: !_running,
        width: 500,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              context.tr('settings.sync.keyRotation.warning'),
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
              readOnly: fieldsLocked,
              hint: context.tr('settings.label.currentMasterKey'),
              onChanged: (_) => setState(() => _error = null),
              onSubmitted: (_) => _submit(),
            ),
            const SizedBox(height: 8),
            Divider(height: 1, color: _softOutline),
            const SizedBox(height: 8),
            _MasterKeyFieldWithStrength(
              key: const ValueKey('settings-sync-new-master-key'),
              controller: _newController,
              hint: context.tr('settings.label.newMasterKey'),
              readOnly: fieldsLocked,
              onChanged: (_) => setState(() => _error = null),
              onSubmitted: (_) => _submit(),
            ),
            SizedBox(height: 10),
            _SettingsTextField(
              key: const ValueKey('settings-sync-new-master-key-confirm'),
              controller: _confirmController,
              obscureText: true,
              readOnly: fieldsLocked,
              hint: context.tr('settings.label.confirmNewMasterKey'),
              onChanged: (_) => setState(() => _error = null),
              onSubmitted: (_) => _submit(),
            ),
            const SizedBox(height: 12),
            Text(
              context.tr(
                widget.backupCount == 0
                    ? 'settings.sync.keyRotation.backupDisabled'
                    : 'settings.sync.keyRotation.backupFirst',
              ),
              style: TextStyle(color: _mutedText, fontSize: 11),
            ),
            if (_loading) ...[
              const SizedBox(height: 10),
              Text(context.tr('settings.sync.keyRotation.loading')),
            ],
            if (!_loading && _reviewStamp != null && _targets.isEmpty) ...[
              const SizedBox(height: 10),
              Text(context.tr('settings.sync.key.noConnectedProviders')),
            ],
            for (final target in _targets)
              Padding(
                padding: const EdgeInsets.only(top: 6),
                child: Row(
                  children: [
                    Expanded(
                      child: Text(target.name, overflow: TextOverflow.ellipsis),
                    ),
                    const SizedBox(width: 8),
                    Text(
                      context.tr(switch (_phases[target.scope]) {
                        SyncKeyRotationPhase.updating =>
                          'settings.sync.keyRotation.updating',
                        SyncKeyRotationPhase.updated =>
                          'settings.sync.keyRotation.updated',
                        SyncKeyRotationPhase.failed => 'common.label.failed',
                        null => 'settings.sync.keyRotation.pending',
                      }),
                      style: TextStyle(color: _mutedText, fontSize: 11),
                    ),
                  ],
                ),
              ),
            if (_retrySamePair && !_running) ...[
              const SizedBox(height: 10),
              Text(
                context.tr('settings.sync.keyRotation.retryWithSameKeys'),
                style: TextStyle(color: _mutedText, fontSize: 12),
              ),
            ],
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
            Wrap(
              alignment: WrapAlignment.end,
              spacing: 8,
              runSpacing: 8,
              children: [
                if (!_loading && !_running && _reviewStamp == null)
                  _SettingsOutlineButton(
                    label: 'common.action.refresh',
                    onTap: _reloadReview,
                  ),
                SizedBox(
                  height: 32,
                  child: FilledButton(
                    key: const ValueKey(
                      'settings-sync-change-master-key-submit',
                    ),
                    onPressed:
                        _running ||
                            _loading ||
                            _reviewStamp == null ||
                            _targets.isEmpty
                        ? null
                        : _submit,
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
                    child: Text(
                      context.tr(
                        _running
                            ? 'settings.sync.keyRotation.updating'
                            : _retrySamePair
                            ? 'common.action.retry'
                            : 'settings.label.changeMasterKey',
                      ),
                    ),
                  ),
                ),
              ],
            ),
          ],
        ),
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
    this.readOnly = false,
    this.onSubmitted,
  });

  final TextEditingController controller;
  final String hint;
  final ValueChanged<String> onChanged;
  final bool readOnly;
  final ValueChanged<String>? onSubmitted;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _SettingsTextField(
          controller: controller,
          obscureText: true,
          readOnly: readOnly,
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
    this.canClose = true,
    this.onClose,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final Widget child;
  final double width;
  final String? titleTooltip;
  final bool canClose;
  final VoidCallback? onClose;

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
                            if (subtitle.isNotEmpty) ...[
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
                          ],
                        ),
                      ),
                      IconButton(
                        tooltip: tr('common.action.close', fallback: 'Close'),
                        onPressed: canClose
                            ? onClose ?? () => Navigator.of(context).pop()
                            : null,
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
