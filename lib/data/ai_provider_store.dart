import '../ai/ai_config.dart';
import 'nauterm_data_store.dart';
import 'nauterm_paths.dart';

final Map<String, List<AiProviderEntry>> _providerCatalogCache = {};

class LoadedAiProvider {
  const LoadedAiProvider({required this.config, this.entry});

  final AiAssistantConfig config;
  final AiProviderEntry? entry;
}

class LoadedAiProviderCatalog {
  const LoadedAiProviderCatalog({
    required this.active,
    required this.providers,
  });

  final LoadedAiProvider active;
  final List<AiProviderEntry> providers;
}

class AiProviderStore {
  AiProviderStore(this.paths);

  final NautermPaths paths;

  LoadedAiProvider load(AiAssistantConfig fallback) {
    return loadCatalog(fallback).active;
  }

  LoadedAiProviderCatalog loadCatalog(AiAssistantConfig fallback) {
    final cached = _providerCatalogCache[paths.databasePath];
    if (cached != null) {
      return _catalogFromProviders(cached, fallback);
    }
    final store = NautermDataStore.openPath(paths.databasePath);
    try {
      final entry = store.getActiveAiProvider();
      final providers = store.listAiProviders();
      _providerCatalogCache[paths.databasePath] = providers;
      return LoadedAiProviderCatalog(
        active: LoadedAiProvider(
          config: entry == null ? fallback : _configFromEntry(entry, fallback),
          entry: entry,
        ),
        providers: List<AiProviderEntry>.unmodifiable(providers),
      );
    } finally {
      store.dispose();
    }
  }

  AiProviderEntry save(
    AiAssistantConfig config, {
    AiProviderEntry? existing,
    String? name,
    bool? active,
  }) {
    final store = NautermDataStore.openPath(paths.databasePath);
    try {
      final saved = store.saveAiProvider(
        _entryFromConfig(
          config,
          existing: existing,
          name: name,
          active: active,
        ),
      );
      _providerCatalogCache[paths.databasePath] = store.listAiProviders();
      return saved;
    } finally {
      store.dispose();
    }
  }

  List<AiProviderEntry> listProviders() {
    final cached = _providerCatalogCache[paths.databasePath];
    if (cached != null) {
      return List<AiProviderEntry>.unmodifiable(cached);
    }
    final store = NautermDataStore.openPath(paths.databasePath);
    try {
      final providers = store.listAiProviders();
      _providerCatalogCache[paths.databasePath] = providers;
      return List<AiProviderEntry>.unmodifiable(providers);
    } finally {
      store.dispose();
    }
  }

  void deleteProvider(AiProviderEntry entry) {
    final id = entry.id;
    if (id == null) {
      return;
    }
    final store = NautermDataStore.openPath(paths.databasePath);
    try {
      store.deleteAiProvider(id);
      _providerCatalogCache[paths.databasePath] = store.listAiProviders();
    } finally {
      store.dispose();
    }
  }
}

LoadedAiProviderCatalog _catalogFromProviders(
  List<AiProviderEntry> providers,
  AiAssistantConfig fallback,
) {
  final entry = providers.where((provider) => provider.active).firstOrNull;
  return LoadedAiProviderCatalog(
    active: LoadedAiProvider(
      config: entry == null ? fallback : _configFromEntry(entry, fallback),
      entry: entry,
    ),
    providers: List<AiProviderEntry>.unmodifiable(providers),
  );
}

AiProviderEntry _entryFromConfig(
  AiAssistantConfig config, {
  AiProviderEntry? existing,
  String? name,
  bool? active,
}) {
  final providerConfig = <String, Object?>{...?existing?.config};
  if (config.maxTokens == null) {
    providerConfig.remove('max_tokens');
  } else {
    providerConfig['max_tokens'] = config.maxTokens;
  }
  _setOptionalConfigValue(providerConfig, 'temperature', config.temperature);
  return AiProviderEntry(
    id: existing?.id,
    uuid: existing?.uuid,
    name: name ?? existing?.name ?? config.protocol.label,
    protocol: config.protocol.storageValue,
    baseUrl: config.baseUrl.trim(),
    model: config.model.trim(),
    apiKey: config.apiKey,
    config: providerConfig,
    active: active ?? existing?.active ?? true,
    createdAt: existing?.createdAt,
    updatedAt: existing?.updatedAt,
    version: existing?.version,
    createdDeviceId: existing?.createdDeviceId,
    updatedDeviceId: existing?.updatedDeviceId,
  );
}

AiAssistantConfig _configFromEntry(
  AiProviderEntry entry,
  AiAssistantConfig preferences,
) {
  return AiAssistantConfig(
    protocol: AiApiProtocol.fromString(entry.protocol),
    baseUrl: entry.baseUrl,
    model: entry.model,
    apiKey: entry.apiKey,
    maxTokens: entry.maxTokens,
    temperature: entry.temperature,
    includeTerminalSelection: preferences.includeTerminalSelection,
    includeRecentTerminalOutput: preferences.includeRecentTerminalOutput,
  );
}

void _setOptionalConfigValue(
  Map<String, Object?> config,
  String key,
  Object? value,
) {
  if (value == null || (value is Iterable && value.isEmpty)) {
    config.remove(key);
  } else {
    config[key] = value;
  }
}
