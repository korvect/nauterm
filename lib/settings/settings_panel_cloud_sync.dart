part of 'settings_panel.dart';

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

Future<List<Map<String, dynamic>>> _loadCloudProviderHistoryInBackground(
  String databasePath,
  String providerId,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.cloudListHistory(providerId: providerId);
    } finally {
      store.dispose();
    }
  });
}

Future<List<Map<String, dynamic>>> _loadS3HistoryInBackground(
  String databasePath,
) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.s3ListHistory();
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

Future<Map<String, dynamic>> _changeCloudProviderMasterKeyInBackground(
  String databasePath, {
  required String providerId,
  required String currentMasterKey,
  required String newMasterKey,
}) {
  return Isolate.run(() {
    final store = NautermDataStore.openPath(databasePath);
    try {
      return store.cloudChangeMasterKey(
        providerId: providerId,
        currentMasterKey: currentMasterKey,
        newMasterKey: newMasterKey,
      );
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
      _mutate(() {
        _cloudProviders = _cloudProviders
            .where((item) => item.id != provider.id)
            .toList(growable: false);
        _githubSyncStatus = '${provider.name} was removed.';
        _githubSyncStatusIsError = false;
        if (_activeSyncProviderId == 'cloud:${provider.id}') {
          _activeSyncProviderId = null;
        }
      });
      await _persistSyncPreferences();
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
    if (enteringMasterKey && masterKey != confirmation) {
      _setGithubSyncError('Master Key confirmation does not match.');
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
        _githubSyncStatus =
            '${provider.name} sync complete · ETag $shortEtag · imported ${result['imported_records']} · revision ${result['revision']}.';
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
      _syncMasterKeyController.clear();
      _syncMasterKeyConfirmController.clear();
      if (mounted) _mutate(() => _githubSyncRunning = false);
    }
  }

  Future<void> _showCloudProviderHistory(
    _CloudProviderInstance provider,
  ) async {
    await showNautermDialog<void>(
      context: context,
      builder: (_) => _CloudProviderHistoryDialog(
        databasePath: _githubSyncDatabasePath,
        provider: provider,
        backupCount: _syncBackupCount,
        onRestored: (result) {
          _mutate(() {
            _rememberSyncSnapshot(
              result['revision'] as int?,
              result['snapshot_id'] as String?,
            );
            _githubSyncStatus = '${provider.name} history restored.';
            _githubSyncStatusIsError = false;
          });
          notifyNautermDatabaseBulkChange();
        },
      ),
    );
  }

  Future<void> _showS3History() async {
    await showNautermDialog<void>(
      context: context,
      builder: (_) => _CloudProviderHistoryDialog.s3(
        databasePath: _githubSyncDatabasePath,
        backupCount: _syncBackupCount,
        onRestored: (result) {
          _mutate(() {
            _rememberSyncSnapshot(
              result['revision'] as int?,
              result['snapshot_id'] as String?,
            );
            _githubSyncStatus = 'S3 history restored.';
            _githubSyncStatusIsError = false;
          });
          notifyNautermDatabaseBulkChange();
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
  bool _oauthAuthorizing = false;
  String? _error;

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
      setState(() => _error = 'Authorize this provider before saving.');
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
      setState(() => _error = 'Authorize this provider before saving.');
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
      title: 'Folder',
      subtitle: 'Optional folder path for the encrypted backup.',
      controller: _prefixController,
      hint: 'nauterm',
    ),
    ..._filenameFields,
  ];

  List<Widget> get _filenameFields => <Widget>[
    _field(
      title: 'Filename',
      subtitle: 'Encrypted backup filename inside the prefix.',
      controller: _filenameController,
      hint: 'nauterm-sync.enc',
    ),
  ];

  Widget _oauthAuthorizationField() {
    final authorized =
        _accessKeyController.text.trim().isNotEmpty &&
        _secretKeyController.text.trim().isNotEmpty;
    return Padding(
      padding: const EdgeInsets.only(top: 18),
      child: _SettingsRow(
        title: 'Authorization',
        subtitle: '',
        showSubtitle: false,
        trailing: Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            if (authorized) ...[
              Icon(LucideIcons.circleCheck, size: 14, color: _secondary),
              SizedBox(width: 6),
              Text(
                tr('common.label.connected', fallback: 'Connected'),
                style: TextStyle(color: _secondary, fontSize: 11),
              ),
              Spacer(),
            ],
            _SettingsOutlineButton(
              label: _oauthAuthorizing
                  ? 'Opening Browser…'
                  : authorized
                  ? 'Reconnect'
                  : 'Connect',
              onTap: _oauthAuthorizing
                  ? null
                  : () => unawaited(_authorizeOAuth()),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _authorizeOAuth() async {
    final vendor = switch (widget.definition.id) {
      'google-cloud-storage' => CloudOAuthVendor.googleCloudStorage,
      'google-drive' => CloudOAuthVendor.googleDrive,
      'onedrive' => CloudOAuthVendor.oneDrive,
      'dropbox' => CloudOAuthVendor.dropbox,
      _ => null,
    };
    if (vendor == null) return;
    setState(() {
      _oauthAuthorizing = true;
      _error = null;
    });
    final client = CloudOAuthClient();
    try {
      final credentials = await client.authorize(vendor);
      if (!mounted) return;
      setState(() {
        _accessKeyController.text = credentials.clientId;
        _secretKeyController.text = credentials.refreshToken;
        _oauthClientSecret = credentials.clientSecret ?? '';
        _error = null;
      });
    } on Object catch (error) {
      if (mounted) setState(() => _error = error.toString());
    } finally {
      client.close();
      if (mounted) setState(() => _oauthAuthorizing = false);
    }
  }

  List<Widget> _vendorFields() {
    if (widget.definition.id == 'webdav') {
      return <Widget>[
        _field(
          title: 'Endpoint',
          subtitle: 'WebDAV server URL.',
          controller: _endpointController,
          hint: 'https://dav.example.com',
        ),
        _field(
          title: 'Root Folder',
          subtitle: 'Folder containing the encrypted backup.',
          controller: _rootController,
          hint: '/nauterm',
        ),
        _field(
          title: 'Username',
          subtitle: 'Stored in the encrypted local database.',
          controller: _accessKeyController,
          hint: 'Username',
        ),
        _field(
          title: 'Password',
          subtitle: 'Stored in the encrypted local database.',
          controller: _secretKeyController,
          hint: 'Password',
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
          title: 'Root Folder',
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
          title: 'Bucket',
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
          title: 'Endpoint',
          subtitle: 'Azure Blob service endpoint.',
          controller: _endpointController,
          hint: 'https://account.blob.core.windows.net',
        ),
        _field(
          title: 'Container',
          subtitle: 'Blob container containing the encrypted backup.',
          controller: _bucketController,
          hint: 'nauterm-sync',
        ),
        _field(
          title: 'Account Name',
          subtitle: 'Azure Storage account name.',
          controller: _accessKeyController,
          hint: 'Account name',
        ),
        _field(
          title: 'Account Key',
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
        title: 'Endpoint',
        subtitle: 'HTTP or HTTPS S3 API endpoint.',
        controller: _endpointController,
        hint: 'https://s3.example.com',
      ),
      _field(
        title: 'Region',
        subtitle: 'Storage region. Use auto when the service discovers it.',
        controller: _regionController,
        hint: 'auto',
      ),
      _field(
        title: 'Bucket',
        subtitle: 'Bucket containing the encrypted sync object.',
        controller: _bucketController,
        hint: 'nauterm-sync',
      ),
      _field(
        title: 'Access Key ID',
        subtitle: 'Stored in the encrypted local database.',
        controller: _accessKeyController,
        hint: 'Access key',
        secret: true,
      ),
      _field(
        title: 'Secret Access Key',
        subtitle: 'Stored in the encrypted local database.',
        controller: _secretKeyController,
        hint: 'Secret key',
        secret: true,
      ),
      ..._pathFields,
    ];
  }

  @override
  Widget build(BuildContext context) {
    return _SyncDialogFrame(
      icon: widget.definition.icon,
      title: widget.provider == null
          ? 'Add ${widget.definition.label}'
          : widget.provider!.name,
      subtitle: widget.definition.subtitle,
      child: Column(
        children: [
          _SettingsRow(
            title: 'Display Name',
            subtitle: 'Name shown on the Sync & Backup page.',
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
                  label: 'Remove from Nauterm',
                  onTap: () => Navigator.of(context).pop(
                    _CloudProviderEditResult(
                      provider: widget.provider!.toProviderJson(),
                      credentials: null,
                      delete: true,
                    ),
                  ),
                ),
              Spacer(),
              _SettingsOutlineButton(
                label: widget.provider == null ? 'Add Provider' : 'Save',
                onTap: _save,
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
      title: 'Add Sync Provider',
      subtitle: 'Choose where Nauterm stores the encrypted sync vault.',
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

class _CloudProviderRow extends StatelessWidget {
  const _CloudProviderRow({
    super.key,
    required this.provider,
    required this.busy,
    required this.active,
    required this.onOpen,
    required this.onSync,
    required this.onHistory,
    required this.onActivate,
  });

  final _CloudProviderInstance provider;
  final bool busy;
  final bool active;
  final VoidCallback onOpen;
  final VoidCallback onSync;
  final VoidCallback? onHistory;
  final VoidCallback onActivate;

  @override
  Widget build(BuildContext context) {
    final statusKey = active
        ? 'settings.sync.provider.status.connected'
        : provider.hasCredentials
        ? 'settings.sync.provider.status.ready'
        : 'settings.sync.provider.status.notConfigured';
    final statusFallback = active
        ? 'Connected'
        : provider.hasCredentials
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
                        color: _primary.withValues(alpha: 0.09),
                        borderRadius: BorderRadius.circular(9),
                      ),
                      alignment: Alignment.center,
                      child: Icon(
                        provider.definition.icon,
                        size: 18,
                        color: _primary,
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
                                  provider.name,
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
                                ready: provider.hasCredentials,
                              ),
                            ],
                          ),
                          SizedBox(height: 3),
                          Text(
                            '${provider.definition.label} · '
                            '${tr(statusKey, fallback: statusFallback)}',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(color: _mutedText, fontSize: 11),
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
                      if (active && provider.hasCredentials)
                        _SyncRowAction(
                          icon: LucideIcons.refreshCw,
                          label: busy ? 'Syncing…' : 'Sync',
                          onTap: busy ? null : onSync,
                        ),
                      if (active &&
                          provider.hasCredentials &&
                          onHistory != null)
                        _SyncRowAction(
                          icon: LucideIcons.history,
                          label: 'History',
                          onTap: busy ? null : onHistory,
                        ),
                      if (!active)
                        _SettingsOutlineButton(
                          label: 'Activate',
                          onTap: provider.hasCredentials ? onActivate : null,
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

class _CloudProviderHistoryDialog extends StatefulWidget {
  const _CloudProviderHistoryDialog({
    required this.databasePath,
    required this.provider,
    required this.onRestored,
    required this.backupCount,
  });

  const _CloudProviderHistoryDialog.s3({
    required this.databasePath,
    required this.onRestored,
    required this.backupCount,
  }) : provider = null;

  final String databasePath;
  final _CloudProviderInstance? provider;
  final ValueChanged<Map<String, dynamic>> onRestored;
  final int backupCount;

  @override
  State<_CloudProviderHistoryDialog> createState() =>
      _CloudProviderHistoryDialogState();
}

class _CloudProviderHistoryDialogState
    extends State<_CloudProviderHistoryDialog> {
  List<Map<String, dynamic>> _versions = const [];
  String? _error;
  bool _loading = true;
  String? _restoringVersion;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    try {
      final versions = widget.provider == null
          ? await _loadS3HistoryInBackground(widget.databasePath)
          : await _loadCloudProviderHistoryInBackground(
              widget.databasePath,
              widget.provider!.id,
            );
      if (!mounted) return;
      setState(() {
        _versions = versions;
        _error = null;
      });
    } on Object catch (error) {
      if (!mounted) return;
      setState(() => _error = error.toString());
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _restore(String versionId) async {
    setState(() {
      _restoringVersion = versionId;
      _error = null;
    });
    try {
      final result = await _restoreCloudVersionInBackground(
        widget.databasePath,
        providerId: widget.provider?.id,
        versionId: versionId,
        backupCount: widget.backupCount,
      );
      if (!mounted) return;
      widget.onRestored(result);
      await _load();
    } on Object catch (error) {
      if (mounted) setState(() => _error = error.toString());
    } finally {
      if (mounted) setState(() => _restoringVersion = null);
    }
  }

  @override
  Widget build(BuildContext context) {
    return _SyncDialogFrame(
      icon: LucideIcons.history,
      title: '${widget.provider?.name ?? 'S3 Compatible'} History',
      subtitle: 'Encrypted object versions retained by the storage provider.',
      width: 560,
      child: _loading
          ? const Padding(
              padding: EdgeInsets.symmetric(vertical: 30),
              child: Center(
                child: SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
              ),
            )
          : _error != null
          ? Padding(
              padding: const EdgeInsets.symmetric(vertical: 18),
              child: Text(
                _error!,
                style: const TextStyle(color: Color(0xffdc2626), fontSize: 11),
              ),
            )
          : _versions.isEmpty
          ? Padding(
              padding: const EdgeInsets.symmetric(vertical: 26),
              child: Column(
                children: [
                  Icon(LucideIcons.history, size: 24, color: _faintText),
                  SizedBox(height: 9),
                  Text(
                    tr(
                      'settings.sync.objectHistory.empty',
                      fallback: 'No object versions found.',
                    ),
                    style: TextStyle(color: _mutedText, fontSize: 12),
                  ),
                  SizedBox(height: 4),
                  Text(
                    switch (widget.provider?.scheme) {
                      'gdrive' => 'Google Drive revisions appear after the sync file has been updated.',
                      'onedrive' => 'OneDrive versions appear after the sync file has been updated.',
                      'dropbox' => 'Dropbox revisions appear after the sync file has been updated.',
                      _ => 'Enable Bucket Versioning in the storage provider to retain history.',
                    },
                    textAlign: TextAlign.center,
                    style: TextStyle(color: _faintText, fontSize: 10),
                  ),
                ],
              ),
            )
          : Column(
              children: [
                for (var index = 0; index < _versions.length; index++) ...[
                  _CloudObjectVersionCard(
                    version: _versions[index],
                    restoring:
                        _restoringVersion == _versions[index]['version_id'],
                    onRestore:
                        (_versions[index]['is_current'] as bool? ?? false)
                        ? null
                        : () => _restore(
                            _versions[index]['version_id'] as String? ?? '',
                          ),
                  ),
                  if (index != _versions.length - 1) SizedBox(height: 7),
                ],
              ],
            ),
    );
  }
}

class _CloudObjectVersionCard extends StatelessWidget {
  const _CloudObjectVersionCard({
    required this.version,
    required this.restoring,
    required this.onRestore,
  });

  final Map<String, dynamic> version;
  final bool restoring;
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
                  isCurrent ? 'Current' : modified,
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
              onTap: restoring ? null : onRestore,
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
