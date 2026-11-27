part of 'settings_panel.dart';

@visibleForTesting
CloudOAuthClient Function()? settingsCloudOAuthClientFactoryOverride;

class _CloudVendorDefinition {
  const _CloudVendorDefinition({
    required this.id,
    required this.scheme,
    required this.label,
    required this.subtitle,
    required this.icon,
  });

  final String id;
  final String scheme;
  final String label;
  final String subtitle;
  final IconData icon;
}

class _CloudProviderInstance {
  const _CloudProviderInstance({
    required this.id,
    required this.scheme,
    required this.vendor,
    required this.name,
    required this.config,
    required this.hasCredentials,
  });

  factory _CloudProviderInstance.fromJson(Map<String, dynamic> json) {
    return _CloudProviderInstance(
      id: json['id'] as String? ?? '',
      scheme: json['scheme'] as String? ?? '',
      vendor: json['vendor'] as String? ?? '',
      name: json['name'] as String? ?? '',
      config: Map<String, String>.from(
        (json['config'] as Map?)?.map(
              (key, value) => MapEntry(key.toString(), value.toString()),
            ) ??
            const <String, String>{},
      ),
      hasCredentials: json['has_credentials'] as bool? ?? false,
    );
  }

  final String id;
  final String scheme;
  final String vendor;
  final String name;
  final Map<String, String> config;
  final bool hasCredentials;

  bool get supportsHistory => const {
    's3',
    'azblob',
    'gcs',
    'gdrive',
    'onedrive',
    'dropbox',
  }.contains(scheme);

  _CloudVendorDefinition get definition => _cloudVendorCatalog.firstWhere(
    (item) => item.id == vendor,
    orElse: () => _genericS3Vendor,
  );

  Map<String, dynamic> toProviderJson() => <String, dynamic>{
    'id': id,
    'scheme': scheme,
    'vendor': vendor,
    'name': name,
    'config': config,
  };
}

const _genericS3Vendor = _CloudVendorDefinition(
  id: 's3-compatible',
  scheme: 's3',
  label: 'S3 Compatible',
  subtitle: 'Connect any service exposing a compatible S3 API.',
  icon: LucideIcons.database,
);

const List<_CloudVendorDefinition> _cloudVendorCatalog = [
  _CloudVendorDefinition(
    id: 'minio',
    scheme: 's3',
    label: 'MinIO',
    subtitle: 'Connect a self-hosted MinIO deployment.',
    icon: LucideIcons.server,
  ),
  _genericS3Vendor,
  _CloudVendorDefinition(
    id: 'webdav',
    scheme: 'webdav',
    label: 'WebDAV',
    subtitle: 'Connect a WebDAV server or compatible cloud drive.',
    icon: LucideIcons.server,
  ),
  _CloudVendorDefinition(
    id: 'azure-blob',
    scheme: 'azblob',
    label: 'Azure Blob Storage',
    subtitle: 'Store backups in an Azure Blob container.',
    icon: LucideIcons.cloud,
  ),
  _CloudVendorDefinition(
    id: 'google-cloud-storage',
    scheme: 'gcs',
    label: 'Google Cloud Storage',
    subtitle: 'Store backups in a GCS bucket.',
    icon: LucideIcons.cloud,
  ),
  _CloudVendorDefinition(
    id: 'google-drive',
    scheme: 'gdrive',
    label: 'Google Drive',
    subtitle: 'Store the encrypted backup in Google Drive.',
    icon: LucideIcons.triangle,
  ),
  _CloudVendorDefinition(
    id: 'onedrive',
    scheme: 'onedrive',
    label: 'OneDrive',
    subtitle: 'Store the encrypted backup in OneDrive.',
    icon: LucideIcons.cloud,
  ),
  _CloudVendorDefinition(
    id: 'dropbox',
    scheme: 'dropbox',
    label: 'Dropbox',
    subtitle: 'Store the encrypted backup in Dropbox.',
    icon: LucideIcons.box,
  ),
];

final List<_CloudVendorDefinition> _addableCloudVendorCatalog =
    _cloudVendorCatalog
        .where((provider) => provider.id != 's3-compatible')
        .toList(growable: false);

const String _dropboxRootPrefix = '/Nauterm/';

String _dropboxRelativeRoot(String root) =>
    root.trim().split('/').where((segment) => segment.isNotEmpty).join('/');

class _ProviderCatalogSelection {
  const _ProviderCatalogSelection.cloud(this.cloud);

  final _CloudVendorDefinition cloud;
}

Future<Map<String, dynamic>> _saveCloudProviderInBackground({
  required String databasePath,
  required Map<String, dynamic> provider,
  Map<String, String>? credentials,
}) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.cloudSaveProvider(
        provider: provider,
        credentials: credentials,
      );
    } finally {
      store.dispose();
    }
  });
}

Future<void> _deleteCloudProviderInBackground(
  String databasePath,
  String providerId,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      store.cloudDeleteProvider(providerId);
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, String>> _loadCloudProviderCredentialsInBackground(
  String databasePath,
  String providerId,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.cloudLoadCredentials(providerId) ?? const <String, String>{};
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, dynamic>> _runCloudProviderSyncInBackground(
  String databasePath,
  String providerId,
  String? masterKey,
  String strategy,
  int backupCount,
) {
  final runner = settingsSyncRunnerOverride;
  if (runner != null) {
    return runner(
      'cloud:$providerId',
      databasePath,
      masterKey,
      strategy,
      backupCount,
    );
  }
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.cloudSync(
        providerId: providerId,
        masterKey: masterKey,
        strategy: strategy,
        backupCount: backupCount,
      );
    } finally {
      store.dispose();
    }
  });
}

Future<Map<String, dynamic>> _restoreCloudVersionInBackground(
  String databasePath, {
  required String? providerId,
  required String versionId,
  required int backupCount,
}) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      if (providerId == null) {
        return store.s3RestoreVersion(versionId, backupCount: backupCount);
      } else {
        return store.cloudRestoreVersion(
          providerId: providerId,
          versionId: versionId,
          backupCount: backupCount,
        );
      }
    } finally {
      store.dispose();
    }
  });
}

extension _CloudSyncActions on _SettingsPanelState {
  List<_SyncProvider> get _visibleBuiltInSyncProviders => const [
    _SyncProvider.githubRepository,
    _SyncProvider.githubGist,
    _SyncProvider.s3Compatible,
  ];

  Future<void> _showAddSyncProviderCatalog() async {
    final selection = await showNautermDialog<_ProviderCatalogSelection>(
      context: context,
      builder: (_) => const _AddSyncProviderDialog(),
    );
    if (!mounted || selection == null) return;
    await _editCloudProvider(definition: selection.cloud);
  }

  Future<void> _editCloudProvider({
    required _CloudVendorDefinition definition,
    _CloudProviderInstance? provider,
  }) async {
    final credentials = provider == null
        ? const <String, String>{}
        : await _loadCloudProviderCredentialsInBackground(
            _githubSyncDatabasePath,
            provider.id,
          );
    if (!mounted) return;
    final result = await showNautermDialog<_CloudProviderEditResult>(
      context: context,
      builder: (_) => _CloudProviderEditorDialog(
        definition: definition,
        provider: provider,
        credentials: credentials,
      ),
    );
    if (!mounted || result == null) return;
    if (result.delete && provider != null) {
      await _deleteCloudProvider(provider);
      return;
    }
    _mutate(() {
      _githubSyncRunning = true;
      _githubSyncStatus = null;
    });
    try {
      final saved = await _saveCloudProviderInBackground(
        databasePath: _githubSyncDatabasePath,
        provider: result.provider,
        credentials: result.credentials,
      );
      if (!mounted) return;
      final instance = _CloudProviderInstance.fromJson(saved);
      _mutate(() {
        _cloudProviders = [
          for (final item in _cloudProviders)
            if (item.id != instance.id) item,
          instance,
        ];
        _githubSyncStatus = '${instance.name} is ready to sync.';
        _githubSyncStatusIsError = false;
      });
      _cacheGithubSyncSetting(
        _githubSyncDatabasePath,
        'cloudProviders',
        _cloudProviders
            .map(
              (item) => <String, dynamic>{
                ...item.toProviderJson(),
                'has_credentials': item.hasCredentials,
              },
            )
            .toList(),
      );
    } on Object catch (error) {
      if (mounted) _setGithubSyncError(error.toString());
    } finally {
      if (mounted) _mutate(() => _githubSyncRunning = false);
    }
  }

  Future<void> _deleteCloudProvider(_CloudProviderInstance provider) async {
    _mutate(() => _githubSyncRunning = true);
    try {
      await _deleteCloudProviderInBackground(
        _githubSyncDatabasePath,
        provider.id,
      );
      if (!mounted) return;
      final wasActive = _activeSyncProviderId == 'cloud:${provider.id}';
      _mutate(() {
        _cloudProviders = _cloudProviders
            .where((item) => item.id != provider.id)
            .toList(growable: false);
        _githubSyncStatus = '${provider.name} was removed.';
        _githubSyncStatusIsError = false;
        if (wasActive) _activeSyncProviderId = null;
      });
      if (wasActive) await _persistSyncPreferences();
      _cacheGithubSyncSetting(
        _githubSyncDatabasePath,
        'cloudProviders',
        _cloudProviders
            .map(
              (item) => <String, dynamic>{
                ...item.toProviderJson(),
                'has_credentials': item.hasCredentials,
              },
            )
            .toList(),
      );
    } on Object catch (error) {
      if (mounted) _setGithubSyncError(error.toString());
    } finally {
      if (mounted) _mutate(() => _githubSyncRunning = false);
    }
  }

  Future<void> _runCloudProviderSync(_CloudProviderInstance provider) async {
    final masterKey = _syncMasterKeyController.text;
    final confirmation = _syncMasterKeyConfirmController.text;
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
    if (enteringMasterKey && masterKey != confirmation) {
      _setGithubSyncError(
        tr(
          'settings.sync.masterKey.error.mismatch',
          fallback: 'Master Key confirmation does not match.',
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
      final result = await _runCloudProviderSyncInBackground(
        _githubSyncDatabasePath,
        provider.id,
        enteringMasterKey ? masterKey : null,
        _syncStrategy.storageValue,
        _syncBackupCount,
      );
      if (!mounted) return;
      final etag = ((result['etag'] as String?) ?? '').replaceAll('"', '');
      final shortEtag = etag.length > 12 ? etag.substring(0, 12) : etag;
      _mutate(() {
        _rememberSyncSnapshot(
          result['revision'] as int?,
          result['snapshot_id'] as String?,
        );
        if (enteringMasterKey) _hasLocalSyncKey = true;
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
      if (mounted) _setGithubSyncError(error.toString());
    } finally {
      _finishSyncAttempt();
    }
  }

  Future<void> _showCloudProviderHistory(_CloudProviderInstance provider) =>
      _showObjectHistory(provider: provider);

  Future<void> _showS3History() => _showObjectHistory();

  Future<void> _showObjectHistory({_CloudProviderInstance? provider}) {
    final databasePath = _githubSyncDatabasePath;
    final backupCount = _syncBackupCount;
    return showNautermDialog<void>(
      context: context,
      builder: (context) => _SyncHistoryDialog(
        databasePath: databasePath,
        providerName:
            provider?.name ?? context.tr('settings.label.s3Compatible'),
        scope: provider == null ? 's3' : 'cloud:${provider.id}',
        backupCount: backupCount,
        emptyHint: switch (provider?.scheme) {
          'gdrive' ||
          'onedrive' ||
          'dropbox' => 'settings.sync.history.objectVersionsHint',
          _ => 'settings.sync.history.bucketVersioningHint',
        },
        onRestore: (version) async {
          final result = await _restoreCloudVersionInBackground(
            databasePath,
            providerId: provider?.id,
            versionId: version,
            backupCount: backupCount,
          );
          notifyNautermDatabaseBulkChange();
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
        },
      ),
    );
  }
}

class _CloudProviderEditResult {
  const _CloudProviderEditResult({
    required this.provider,
    required this.credentials,
    this.delete = false,
  });

  final Map<String, dynamic> provider;
  final Map<String, String>? credentials;
  final bool delete;
}

class _CloudProviderEditorDialog extends StatefulWidget {
  const _CloudProviderEditorDialog({
    required this.definition,
    required this.credentials,
    this.provider,
  });

  final _CloudVendorDefinition definition;
  final _CloudProviderInstance? provider;
  final Map<String, String> credentials;

  @override
  State<_CloudProviderEditorDialog> createState() =>
      _CloudProviderEditorDialogState();
}

class _CloudProviderEditorDialogState
    extends State<_CloudProviderEditorDialog> {
  late final TextEditingController _nameController;
  late final TextEditingController _endpointController;
  late final TextEditingController _regionController;
  late final TextEditingController _rootController;
  late final TextEditingController _bucketController;
  late final TextEditingController _accessKeyController;
  late final TextEditingController _secretKeyController;
  late final TextEditingController _prefixController;
  late final TextEditingController _filenameController;
  String _oauthClientSecret = '';
  CloudOAuthClient? _oauthClient;
  CloudOAuthPhase _oauthPhase = CloudOAuthPhase.starting;
  bool _oauthCredentialsChanged = false;
  bool _confirmingDelete = false;
  String? _error;

  void _delete(_CloudProviderInstance provider) {
    Navigator.of(context).pop(
      _CloudProviderEditResult(
        provider: provider.toProviderJson(),
        credentials: null,
        delete: true,
      ),
    );
  }

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(
      text: widget.provider?.name ?? widget.definition.label,
    );
    final config = widget.provider?.config ?? const <String, String>{};
    _endpointController = TextEditingController(text: config['endpoint'] ?? '');
    _regionController = TextEditingController(
      text:
          config['region'] ??
          switch (widget.definition.id) {
            'minio' => 'auto',
            's3-compatible' => 'auto',
            _ => '',
          },
    );
    final root = config['root'] ?? '/';
    _rootController = TextEditingController(
      text: widget.definition.id == 'dropbox'
          ? _dropboxRelativeRoot(root)
          : root,
    );
    _bucketController = TextEditingController(text: config['bucket'] ?? '');
    final credentialKeys = switch (widget.definition.id) {
      'webdav' => const ('username', 'password'),
      'azure-blob' => const ('account_name', 'account_key'),
      'google-cloud-storage' ||
      'google-drive' ||
      'onedrive' ||
      'dropbox' => const ('client_id', 'refresh_token'),
      _ => const ('access_key_id', 'secret_access_key'),
    };
    _accessKeyController = TextEditingController(
      text: widget.credentials[credentialKeys.$1] ?? '',
    );
    _secretKeyController = TextEditingController(
      text: widget.credentials[credentialKeys.$2] ?? '',
    );
    _prefixController = TextEditingController(text: config['prefix'] ?? '');
    _filenameController = TextEditingController(
      text: config['filename'] ?? 'nauterm-sync.enc',
    );
    _oauthClientSecret = widget.credentials['client_secret'] ?? '';
  }

  @override
  void dispose() {
    _oauthClient?.cancel();
    _nameController.dispose();
    _endpointController.dispose();
    _regionController.dispose();
    _rootController.dispose();
    _bucketController.dispose();
    _accessKeyController.dispose();
    _secretKeyController.dispose();
    _prefixController.dispose();
    _filenameController.dispose();
    super.dispose();
  }

  void _save() {
    if (!_validateSharedFields()) return;
    switch (widget.definition.id) {
      case 'minio':
        return _saveDirectS3();
      case 'webdav':
        return _saveWebdav();
      case 'azure-blob':
        return _saveAzureBlob();
      case 'google-cloud-storage':
        return _saveGcs();
      case 'google-drive' || 'onedrive' || 'dropbox':
        return _saveOAuthDrive();
      default:
        return _saveDirectS3();
    }
  }

  bool _validateSharedFields() {
    if (!_require(_nameController, 'Display Name')) return false;
    if (!_require(_filenameController, 'Filename')) return false;
    return true;
  }

  bool _validateCredentialPair({
    String firstLabel = 'Access Key ID',
    String secondLabel = 'Secret Access Key',
  }) {
    final accessKey = _accessKeyController.text.trim();
    final secretKey = _secretKeyController.text.trim();
    if ((accessKey.isEmpty) != (secretKey.isEmpty)) {
      setState(() => _error = 'Enter both $firstLabel and $secondLabel.');
      return false;
    }
    if (accessKey.isEmpty && widget.provider?.hasCredentials != true) {
      setState(() => _error = '$firstLabel is required.');
      return false;
    }
    return true;
  }

  bool _validateS3Fields() =>
      _require(_bucketController, 'Bucket') && _validateCredentialPair();

  bool _require(TextEditingController controller, String label) {
    if (controller.text.trim().isNotEmpty) return true;
    setState(() => _error = '$label is required.');
    return false;
  }

  void _saveDirectS3() {
    if (!_validateS3Fields()) return;
    if (!_require(_endpointController, 'Endpoint')) return;
    _finish(<String, String>{
      'endpoint': _endpointController.text.trim(),
      'region': _regionController.text.trim().isEmpty
          ? 'auto'
          : _regionController.text.trim(),
      ..._objectConfig,
    });
  }

  void _saveWebdav() {
    if (!_require(_endpointController, 'Endpoint') ||
        !_validateCredentialPair(
          firstLabel: 'Username',
          secondLabel: 'Password',
        )) {
      return;
    }
    _finish(
      <String, String>{
        'endpoint': _endpointController.text.trim(),
        'root': _rootController.text.trim().isEmpty
            ? '/'
            : _rootController.text.trim(),
        ..._filenameConfig,
      },
      credentialKeys: const ('username', 'password'),
    );
  }

  void _saveAzureBlob() {
    if (!_require(_endpointController, 'Endpoint') ||
        !_require(_bucketController, 'Container') ||
        !_validateCredentialPair(
          firstLabel: 'Account Name',
          secondLabel: 'Account Key',
        )) {
      return;
    }
    _finish(
      <String, String>{
        'endpoint': _endpointController.text.trim(),
        'container': _bucketController.text.trim(),
        ..._pathConfig,
      },
      credentialKeys: const ('account_name', 'account_key'),
    );
  }

  void _saveGcs() {
    if (!_require(_bucketController, 'Bucket')) return;
    final refreshToken = _secretKeyController.text.trim();
    final clientId = _accessKeyController.text.trim();
    if (refreshToken.isEmpty || clientId.isEmpty) {
      setState(() {
        _error = context.tr(
          'settings.sync.authorization.validation.required',
          fallback: 'This provider needs authorization.',
        );
      });
      return;
    }
    _finish(
      _objectConfig,
      credentials: refreshToken.isEmpty || clientId.isEmpty
          ? null
          : _oauthCredentialValues(
              clientId,
              refreshToken,
              clientSecret: _oauthClientSecret,
            ),
    );
  }

  void _saveOAuthDrive() {
    final refreshToken = _secretKeyController.text.trim();
    final clientId = _accessKeyController.text.trim();
    if ((refreshToken.isEmpty || clientId.isEmpty) &&
        widget.provider?.hasCredentials != true) {
      setState(() {
        _error = context.tr(
          'settings.sync.authorization.validation.required',
          fallback: 'This provider needs authorization.',
        );
      });
      return;
    }
    final root = _oauthDriveRoot();
    if (root == null) return;
    _finish(
      <String, String>{'root': root, ..._filenameConfig},
      credentials: refreshToken.isEmpty || clientId.isEmpty
          ? null
          : _oauthCredentialValues(
              clientId,
              refreshToken,
              clientSecret: widget.definition.id == 'google-drive'
                  ? _oauthClientSecret
                  : null,
            ),
    );
  }

  String? _oauthDriveRoot() {
    final root = _rootController.text.trim();
    if (widget.definition.id != 'dropbox') {
      return root.isEmpty ? '/' : root;
    }
    if (root.isEmpty) return '/';
    if (root.startsWith('/') || root.contains(r'\')) {
      setState(
        () => _error = 'Enter a folder relative to $_dropboxRootPrefix.',
      );
      return null;
    }
    final segments = root.split('/');
    if (segments.any(
      (segment) => segment.isEmpty || segment == '.' || segment == '..',
    )) {
      setState(
        () => _error = 'Enter a folder relative to $_dropboxRootPrefix.',
      );
      return null;
    }
    return '/$root';
  }

  Map<String, String> _oauthCredentialValues(
    String clientId,
    String refreshToken, {
    String? clientSecret,
  }) => <String, String>{
    'client_id': clientId,
    'refresh_token': refreshToken,
    if (clientSecret != null && clientSecret.isNotEmpty)
      'client_secret': clientSecret,
  };

  Map<String, String> get _objectConfig => <String, String>{
    'bucket': _bucketController.text.trim(),
    ..._pathConfig,
  };

  Map<String, String> get _pathConfig => <String, String>{
    'prefix': _prefixController.text.trim(),
    ..._filenameConfig,
  };

  Map<String, String> get _filenameConfig => <String, String>{
    'filename': _filenameController.text.trim(),
  };

  void _finish(
    Map<String, String> config, {
    (String, String) credentialKeys = const (
      'access_key_id',
      'secret_access_key',
    ),
    Map<String, String>? credentials,
  }) {
    final accessKey = _accessKeyController.text.trim();
    final secretKey = _secretKeyController.text.trim();
    credentials ??= accessKey.isEmpty
        ? null
        : <String, String>{
            credentialKeys.$1: accessKey,
            credentialKeys.$2: secretKey,
          };
    final id =
        widget.provider?.id ??
        'cloud-${DateTime.now().microsecondsSinceEpoch}-${math.Random.secure().nextInt(1 << 32)}';
    Navigator.of(context).pop(
      _CloudProviderEditResult(
        provider: <String, dynamic>{
          'id': id,
          'scheme': widget.definition.scheme,
          'vendor': widget.definition.id,
          'name': _nameController.text.trim(),
          'config': config,
        },
        credentials: credentials,
      ),
    );
  }

  Widget _field({
    required String title,
    required String subtitle,
    required TextEditingController controller,
    required String hint,
    bool secret = false,
    String? prefixText,
  }) {
    return Padding(
      padding: const EdgeInsets.only(top: 18),
      child: _SettingsRow(
        title: title,
        subtitle: subtitle,
        showSubtitle: false,
        trailing: _SettingsTextField(
          controller: controller,
          obscureText: secret,
          revealable: secret,
          hint: hint,
          prefixText: prefixText,
          onChanged: (_) => setState(() => _error = null),
        ),
      ),
    );
  }

  List<Widget> get _pathFields => <Widget>[
    _field(
      title: 'common.label.folder',
      subtitle: 'Optional folder path for the encrypted backup.',
      controller: _prefixController,
      hint: 'nauterm',
    ),
    ..._filenameFields,
  ];

  List<Widget> get _filenameFields => <Widget>[
    _field(
      title: 'common.label.filename',
      subtitle: 'Encrypted backup filename inside the prefix.',
      controller: _filenameController,
      hint: 'nauterm-sync.enc',
    ),
  ];

  Widget _oauthAuthorizationField() {
    final client = _oauthClient;
    final authorized =
        _accessKeyController.text.trim().isNotEmpty &&
        _secretKeyController.text.trim().isNotEmpty;
    return Padding(
      padding: const EdgeInsets.only(top: 18),
      child: Column(
        children: [
          _SettingsRow(
            title: 'settings.label.authorization',
            subtitle: switch ((
              client != null,
              _oauthCredentialsChanged,
              authorized,
            )) {
              (true, _, _) => 'settings.sync.authorization.browser.description',
              (false, true, _) =>
                'settings.sync.authorization.pending.description',
              (false, false, true) =>
                'settings.sync.authorization.saved.description',
              _ => 'settings.sync.authorization.required.description',
            },
            trailing: Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                if (authorized) ...[
                  Icon(LucideIcons.circleCheck, size: 14, color: _secondary),
                  const SizedBox(width: 6),
                  Text(
                    tr('common.label.connected', fallback: 'Connected'),
                    style: TextStyle(color: _secondary, fontSize: 11),
                  ),
                  const Spacer(),
                ],
                _SettingsOutlineButton(
                  label: client != null
                      ? 'settings.sync.authorization.action.cancel'
                      : authorized
                      ? 'common.label.reconnect'
                      : 'common.action.connect',
                  onTap: client == null
                      ? () => unawaited(_authorizeOAuth())
                      : client.isCancelled
                      ? null
                      : () => setState(client.cancel),
                ),
              ],
            ),
          ),
          if (client != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Row(
                children: [
                  SizedBox.square(
                    dimension: 13,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: _mutedText,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      context.tr(switch (_oauthPhase) {
                        CloudOAuthPhase.starting =>
                          'settings.sync.authorization.status.starting',
                        CloudOAuthPhase.waitingForBrowser => 'settings.sync.authorization.status.waitingForBrowser',
                        CloudOAuthPhase.exchanging =>
                          'settings.sync.authorization.status.exchanging',
                      }),
                      style: TextStyle(color: _mutedText, fontSize: 11),
                    ),
                  ),
                  if (_oauthPhase == CloudOAuthPhase.waitingForBrowser) ...[
                    const SizedBox(width: 8),
                    _SettingsOutlineButton(
                      label: 'settings.sync.authorization.action.openBrowser',
                      onTap: client.isCancelled
                          ? null
                          : () => unawaited(_reopenOAuthBrowser(client)),
                    ),
                  ],
                ],
              ),
            ),
        ],
      ),
    );
  }

  Future<void> _reopenOAuthBrowser(CloudOAuthClient client) async {
    try {
      await client.reopenBrowser();
    } on Object catch (error) {
      if (mounted && identical(_oauthClient, client) && !client.isCancelled) {
        setState(() => _error = error.toString());
      }
    }
  }

  Future<void> _authorizeOAuth() async {
    if (_oauthClient != null) return;
    final vendor = switch (widget.definition.id) {
      'google-cloud-storage' => CloudOAuthVendor.googleCloudStorage,
      'google-drive' => CloudOAuthVendor.googleDrive,
      'onedrive' => CloudOAuthVendor.oneDrive,
      'dropbox' => CloudOAuthVendor.dropbox,
      _ => null,
    };
    if (vendor == null) return;
    final client =
        settingsCloudOAuthClientFactoryOverride?.call() ?? CloudOAuthClient();
    setState(() {
      _oauthClient = client;
      _oauthPhase = CloudOAuthPhase.starting;
      _error = null;
    });
    try {
      final credentials = await client.authorize(
        vendor,
        onPhase: (phase) {
          if (mounted && !client.isCancelled) {
            setState(() => _oauthPhase = phase);
          }
        },
      );
      if (!mounted || credentials == null || client.isCancelled) return;
      setState(() {
        _accessKeyController.text = credentials.clientId;
        _secretKeyController.text = credentials.refreshToken;
        _oauthClientSecret = credentials.clientSecret ?? '';
        _oauthCredentialsChanged = true;
        _error = null;
      });
    } on Object catch (error) {
      if (mounted && !client.isCancelled) {
        setState(() => _error = error.toString());
      }
    } finally {
      client.close();
      if (mounted) setState(() => _oauthClient = null);
    }
  }

  List<Widget> _vendorFields() {
    if (widget.definition.id == 'webdav') {
      return <Widget>[
        _field(
          title: 'common.label.endpoint',
          subtitle: 'WebDAV server URL.',
          controller: _endpointController,
          hint: 'https://dav.example.com',
        ),
        _field(
          title: 'settings.label.rootFolder',
          subtitle: 'Folder containing the encrypted backup.',
          controller: _rootController,
          hint: '/nauterm',
        ),
        _field(
          title: 'common.label.username',
          subtitle: 'Stored in the encrypted local database.',
          controller: _accessKeyController,
          hint: 'common.label.username',
        ),
        _field(
          title: 'common.label.password',
          subtitle: 'Stored in the encrypted local database.',
          controller: _secretKeyController,
          hint: 'common.label.password',
          secret: true,
        ),
        ..._filenameFields,
      ];
    }
    if (widget.definition.id == 'google-drive' ||
        widget.definition.id == 'onedrive' ||
        widget.definition.id == 'dropbox') {
      return <Widget>[
        _field(
          title: 'settings.label.rootFolder',
          subtitle: 'Folder containing the encrypted backup.',
          controller: _rootController,
          hint: widget.definition.id == 'dropbox' ? 'Backups' : '/Nauterm',
          prefixText: widget.definition.id == 'dropbox'
              ? _dropboxRootPrefix
              : null,
        ),
        _oauthAuthorizationField(),
        ..._filenameFields,
      ];
    }
    if (widget.definition.id == 'google-cloud-storage') {
      return <Widget>[
        _field(
          title: 'settings.label.bucket',
          subtitle: 'GCS bucket containing the encrypted backup.',
          controller: _bucketController,
          hint: 'nauterm-sync',
        ),
        _oauthAuthorizationField(),
        ..._pathFields,
      ];
    }
    if (widget.definition.id == 'azure-blob') {
      return <Widget>[
        _field(
          title: 'common.label.endpoint',
          subtitle: 'Azure Blob service endpoint.',
          controller: _endpointController,
          hint: 'https://account.blob.core.windows.net',
        ),
        _field(
          title: 'common.label.container',
          subtitle: 'Blob container containing the encrypted backup.',
          controller: _bucketController,
          hint: 'nauterm-sync',
        ),
        _field(
          title: 'Account Name',
          subtitle: 'Azure Storage account name.',
          controller: _accessKeyController,
          hint: 'settings.label.accountName',
        ),
        _field(
          title: 'settings.label.accountKey',
          subtitle: 'Stored in the encrypted local database.',
          controller: _secretKeyController,
          hint: 'Account key',
          secret: true,
        ),
        ..._pathFields,
      ];
    }
    return <Widget>[
      _field(
        title: 'common.label.endpoint',
        subtitle: 'HTTP or HTTPS S3 API endpoint.',
        controller: _endpointController,
        hint: 'https://s3.example.com',
      ),
      _field(
        title: 'common.label.region',
        subtitle: 'Storage region. Use auto when the service discovers it.',
        controller: _regionController,
        hint: 'auto',
      ),
      _field(
        title: 'settings.label.bucket',
        subtitle: 'Bucket containing the encrypted sync object.',
        controller: _bucketController,
        hint: 'nauterm-sync',
      ),
      _field(
        title: 'settings.label.accessKeyId',
        subtitle: 'Stored in the encrypted local database.',
        controller: _accessKeyController,
        hint: 'settings.label.accessKey',
        secret: true,
      ),
      _field(
        title: 'settings.label.secretAccessKey',
        subtitle: 'Stored in the encrypted local database.',
        controller: _secretKeyController,
        hint: 'settings.label.secretKey',
        secret: true,
      ),
      ..._pathFields,
    ];
  }

  @override
  Widget build(BuildContext context) {
    final provider = widget.provider;
    if (_confirmingDelete && provider != null) {
      return _buildSyncProviderRemovalConfirmation(
        context,
        name: provider.name,
        disconnect: false,
        onCancel: () => setState(() => _confirmingDelete = false),
        onConfirm: () => _delete(provider),
      );
    }
    return _SyncDialogFrame(
      icon: widget.definition.icon,
      title: widget.provider == null
          ? 'Add ${widget.definition.label}'
          : widget.provider!.name,
      subtitle: widget.definition.subtitle,
      child: Column(
        children: [
          _SettingsRow(
            title: 'settings.label.displayName',
            subtitle: 'settings.sync.provider.displayName.description',
            showSubtitle: false,
            trailing: _SettingsTextField(
              controller: _nameController,
              hint: widget.definition.label,
              onChanged: (_) => setState(() => _error = null),
            ),
          ),
          ..._vendorFields(),
          if (_error != null) ...[
            SizedBox(height: 14),
            Align(
              alignment: Alignment.centerRight,
              child: Text(
                _error!,
                style: const TextStyle(color: Color(0xffdc2626), fontSize: 11),
              ),
            ),
          ],
          SizedBox(height: 18),
          Row(
            children: [
              if (widget.provider != null)
                _SettingsOutlineButton(
                  label: 'settings.label.removeFromApplication',
                  onTap: _oauthClient == null
                      ? () => setState(() => _confirmingDelete = true)
                      : null,
                ),
              Spacer(),
              _SettingsOutlineButton(
                label: widget.provider == null
                    ? 'Add Provider'
                    : 'settings.sync.action.saveSettings',
                onTap: _oauthClient == null ? _save : null,
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _AddSyncProviderDialog extends StatelessWidget {
  const _AddSyncProviderDialog();

  @override
  Widget build(BuildContext context) {
    return _SyncDialogFrame(
      icon: LucideIcons.cloud,
      title: 'settings.label.addSyncProvider',
      subtitle: 'settings.sync.provider.description',
      child: Column(
        children: [
          for (
            var index = 0;
            index < _addableCloudVendorCatalog.length;
            index++
          ) ...[
            _ProviderCatalogRow(
              icon: _addableCloudVendorCatalog[index].icon,
              label: _addableCloudVendorCatalog[index].label,
              subtitle: _addableCloudVendorCatalog[index].subtitle,
              onTap: () => Navigator.of(context).pop(
                _ProviderCatalogSelection.cloud(
                  _addableCloudVendorCatalog[index],
                ),
              ),
            ),
            if (index != _addableCloudVendorCatalog.length - 1)
              SizedBox(height: 8),
          ],
        ],
      ),
    );
  }
}

class _ProviderCatalogRow extends StatelessWidget {
  const _ProviderCatalogRow({
    required this.icon,
    required this.label,
    required this.subtitle,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final String subtitle;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(9),
        child: Ink(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          decoration: BoxDecoration(
            border: Border.all(color: _softOutline),
            borderRadius: BorderRadius.circular(9),
          ),
          child: Row(
            children: [
              Icon(icon, size: 18, color: _primary),
              SizedBox(width: 12),
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
                    SizedBox(height: 2),
                    Text(
                      tr(subtitle),
                      style: TextStyle(color: _mutedText, fontSize: 10),
                    ),
                  ],
                ),
              ),
              Icon(LucideIcons.chevronRight, size: 15, color: _faintText),
            ],
          ),
        ),
      ),
    );
  }
}

class _CloudObjectVersionCard extends StatelessWidget {
  const _CloudObjectVersionCard({
    required this.version,
    required this.restoring,
    required this.disabled,
    required this.onRestore,
  });

  final Map<String, dynamic> version;
  final bool restoring;
  final bool disabled;
  final VoidCallback? onRestore;

  @override
  Widget build(BuildContext context) {
    final isCurrent = version['is_current'] as bool? ?? false;
    final versionId = version['version_id'] as String? ?? '';
    final shortVersion = versionId.length > 18
        ? versionId.substring(0, 18)
        : versionId;
    final etag = ((version['etag'] as String?) ?? '').replaceAll('"', '');
    final shortEtag = etag.length > 12 ? etag.substring(0, 12) : etag;
    final modified = version['last_modified'] as String? ?? '';
    final bytes = version['content_length'] as int? ?? 0;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: isCurrent ? _primary.withValues(alpha: 0.06) : _surfaceContainer,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: isCurrent ? _primary.withValues(alpha: 0.28) : _softOutline,
        ),
      ),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  isCurrent ? tr('settings.sync.history.current') : modified,
                  style: TextStyle(
                    color: _text,
                    fontSize: 12,
                    fontWeight: NautermFontWeights.semibold,
                  ),
                ),
                if (isCurrent && modified.isNotEmpty) ...[
                  SizedBox(height: 2),
                  Text(
                    modified,
                    style: TextStyle(color: _mutedText, fontSize: 10),
                  ),
                ],
                SizedBox(height: 2),
                Text(
                  tr('${_formatCloudObjectBytes(bytes)} · ETag $shortEtag'),
                  style: TextStyle(color: _mutedText, fontSize: 10),
                ),
              ],
            ),
          ),
          Tooltip(
            message: versionId,
            child: Text(
              shortVersion,
              style: TextStyle(
                color: _mutedText,
                fontSize: 10,
                fontFamily: 'monospace',
              ),
            ),
          ),
          if (onRestore != null) ...[
            SizedBox(width: 8),
            _SettingsOutlineButton(
              label: restoring
                  ? tr('common.label.restoring', fallback: 'Restoring…')
                  : tr('common.action.restore', fallback: 'Restore'),
              onTap: disabled || versionId.isEmpty ? null : onRestore,
            ),
          ],
        ],
      ),
    );
  }
}

String _formatCloudObjectBytes(int bytes) {
  if (bytes < 1024) return '$bytes B';
  final kib = bytes / 1024;
  if (kib < 1024) return '${kib.toStringAsFixed(kib < 10 ? 1 : 0)} KiB';
  final mib = kib / 1024;
  return '${mib.toStringAsFixed(mib < 10 ? 1 : 0)} MiB';
}
