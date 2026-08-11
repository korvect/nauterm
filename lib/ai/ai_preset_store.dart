import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:http/http.dart' as http;

import '../app/nauterm_log.dart';
import '../data/nauterm_paths.dart';
import 'ai_provider_preset.dart';

const String _defaultPresetsUrl =
    'https://raw.githubusercontent.com/korvect/nauterm-ai-presets/main/presets.json';
const String _bundledPresetsAsset = 'assets/config/ai-presets.json';

class AiPresetStore {
  AiPresetStore(this.paths);

  final NautermPaths paths;

  Map<String, AiProviderPreset>? _cachedPresets;
  bool _isLoading = false;

  File get _cacheFile =>
      File('${paths.dataDirectory.path}/ai_presets_cache.json');

  /// Load presets from cache or defaults, then fetch remote in background
  Future<Map<String, AiProviderPreset>> loadPresets({String? remoteUrl}) async {
    // 1. Load from cache immediately if available
    final cached = await _loadFromCache();
    if (cached != null && cached.isNotEmpty) {
      _cachedPresets = cached;
      // Background fetch, don't await
      _fetchRemoteInBackground(remoteUrl ?? _defaultPresetsUrl);
      return cached;
    }

    // 2. Fall back to bundled presets immediately, then update in background.
    final bundled = await _loadBundledDefaults();
    if (bundled.isNotEmpty) {
      _cachedPresets = bundled;
      _fetchRemoteInBackground(remoteUrl ?? _defaultPresetsUrl);
      return bundled;
    }

    // 3. No cache or bundled presets, try remote (blocking for first load).
    return await _fetchRemote(remoteUrl ?? _defaultPresetsUrl);
  }

  /// Get cached presets synchronously
  Map<String, AiProviderPreset> getPresets() {
    return _cachedPresets ?? _builtInDefaults();
  }

  Future<Map<String, AiProviderPreset>> refreshPresets({String? remoteUrl}) {
    return _fetchRemote(remoteUrl ?? _defaultPresetsUrl);
  }

  Future<Map<String, AiProviderPreset>> _fetchRemote(String url) async {
    if (_isLoading) return _cachedPresets ?? _builtInDefaults();
    _isLoading = true;

    try {
      final response = await http
          .get(Uri.parse(url))
          .timeout(const Duration(seconds: 5));

      if (response.statusCode == 200) {
        final presets = _parsePresets(response.body);
        if (presets.isNotEmpty) {
          _cachedPresets = presets;
          await _saveToCache(response.body);
          return presets;
        }
      }
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'ai',
        'Unable to fetch AI presets.',
        error: error,
        stackTrace: stackTrace,
      );
    } finally {
      _isLoading = false;
    }

    final cachedPresets = _cachedPresets;
    if (cachedPresets != null) {
      return cachedPresets;
    }
    final bundled = await _loadBundledDefaults();
    return bundled.isNotEmpty ? bundled : _builtInDefaults();
  }

  void _fetchRemoteInBackground(String url) {
    unawaited(_fetchRemote(url));
  }

  Future<Map<String, AiProviderPreset>?> _loadFromCache() async {
    try {
      if (await _cacheFile.exists()) {
        final content = await _cacheFile.readAsString();
        return _parsePresets(content);
      }
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'ai',
        'Unable to load AI presets cache.',
        error: error,
        stackTrace: stackTrace,
      );
    }
    return null;
  }

  Future<Map<String, AiProviderPreset>> _loadBundledDefaults() async {
    try {
      final content = await rootBundle.loadString(_bundledPresetsAsset);
      return _parsePresets(content);
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'ai',
        'Unable to load bundled AI presets.',
        error: error,
        stackTrace: stackTrace,
      );
      return {};
    }
  }

  Future<void> _saveToCache(String content) async {
    try {
      await paths.ensureCreated();
      await _cacheFile.writeAsString(content);
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'ai',
        'Unable to save AI presets cache.',
        error: error,
        stackTrace: stackTrace,
      );
    }
  }

  Map<String, AiProviderPreset> _parsePresets(String content) {
    try {
      final json = jsonDecode(content) as Map<String, Object?>;
      final presets = json['presets'] as Map<String, Object?>? ?? {};
      return presets.map((key, value) {
        if (value is Map<String, Object?>) {
          return MapEntry(
            key,
            _normalizeNativePreset(AiProviderPreset.fromJson(key, value)),
          );
        }
        return MapEntry(
          key,
          AiProviderPreset(id: key, name: key, protocol: 'openai', baseUrl: ''),
        );
      });
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'ai',
        'Unable to parse AI presets.',
        error: error,
        stackTrace: stackTrace,
      );
      return {};
    }
  }

  AiProviderPreset _normalizeNativePreset(AiProviderPreset preset) {
    final nativeProtocol = switch (preset.id) {
      'google' => ('google', 'https://generativelanguage.googleapis.com'),
      'ollama' => ('ollama', 'http://localhost:11434'),
      _ => null,
    };
    if (nativeProtocol == null) return preset;
    return AiProviderPreset(
      id: preset.id,
      name: preset.name,
      protocol: nativeProtocol.$1,
      baseUrl: nativeProtocol.$2,
      defaultModels: preset.defaultModels,
      websiteUrl: preset.websiteUrl,
      apiKeyUrl: preset.apiKeyUrl,
    );
  }

  Map<String, AiProviderPreset> _builtInDefaults() {
    return {
      'openai': const AiProviderPreset(
        id: 'openai',
        name: 'OpenAI',
        protocol: 'openai',
        baseUrl: 'https://api.openai.com/v1',
        defaultModels: ['gpt-5.6-sol', 'gpt-5.6-terra', 'gpt-5.6-luna'],
      ),
      'anthropic': const AiProviderPreset(
        id: 'anthropic',
        name: 'Anthropic',
        protocol: 'anthropic',
        baseUrl: 'https://api.anthropic.com',
        defaultModels: [
          'claude-fable-5',
          'claude-opus-5',
          'claude-sonnet-5',
          'claude-haiku-4-5-20251001',
        ],
      ),
      'google': const AiProviderPreset(
        id: 'google',
        name: 'Google Gemini',
        protocol: 'google',
        baseUrl: 'https://generativelanguage.googleapis.com',
        defaultModels: ['gemini-2.5-pro', 'gemini-2.5-flash'],
      ),
      'ollama': const AiProviderPreset(
        id: 'ollama',
        name: 'Ollama',
        protocol: 'ollama',
        baseUrl: 'http://localhost:11434',
        defaultModels: ['qwen3', 'llama3.2'],
      ),
    };
  }
}
