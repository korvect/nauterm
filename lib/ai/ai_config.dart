import 'package:flutter/foundation.dart';

enum AiApiProtocol {
  openAi('openai', 'OpenAI'),
  anthropic('anthropic', 'Anthropic'),
  google('google', 'Google Gemini'),
  ollama('ollama', 'Ollama');

  const AiApiProtocol(this.storageValue, this.label);

  final String storageValue;
  final String label;

  String get defaultBaseUrl => switch (this) {
    AiApiProtocol.openAi => AiAssistantConfig.openAiDefaultBaseUrl,
    AiApiProtocol.anthropic => AiAssistantConfig.anthropicDefaultBaseUrl,
    AiApiProtocol.google => AiAssistantConfig.googleDefaultBaseUrl,
    AiApiProtocol.ollama => AiAssistantConfig.ollamaDefaultBaseUrl,
  };

  bool get requiresApiKey => this != AiApiProtocol.ollama;

  static AiApiProtocol fromString(Object? value) {
    return switch (value) {
      'anthropic' => AiApiProtocol.anthropic,
      'google' || 'googleai' || 'gemini' => AiApiProtocol.google,
      'ollama' => AiApiProtocol.ollama,
      _ => AiApiProtocol.openAi,
    };
  }
}

@immutable
class AiAssistantConfig {
  const AiAssistantConfig({
    this.protocol = AiApiProtocol.openAi,
    this.baseUrl = openAiDefaultBaseUrl,
    this.model = '',
    this.apiKey = '',
    this.maxTokens,
    this.temperature,
    this.includeTerminalSelection = true,
    this.includeRecentTerminalOutput = true,
  });

  static const String openAiDefaultBaseUrl = 'https://api.openai.com/v1';
  static const String anthropicDefaultBaseUrl = 'https://api.anthropic.com';
  static const String googleDefaultBaseUrl =
      'https://generativelanguage.googleapis.com';
  static const String ollamaDefaultBaseUrl = 'http://localhost:11434';

  final AiApiProtocol protocol;
  final String baseUrl;
  final String model;
  final String apiKey;
  final int? maxTokens;
  final double? temperature;
  final bool includeTerminalSelection;
  final bool includeRecentTerminalOutput;

  String? get validationError {
    final uri = Uri.tryParse(baseUrl.trim());
    if (uri == null ||
        !uri.hasScheme ||
        (uri.scheme != 'http' && uri.scheme != 'https') ||
        uri.host.isEmpty) {
      return 'Configure a valid AI base URL in Settings.';
    }
    if (model.trim().isEmpty) {
      return 'Configure an AI model in Settings.';
    }
    if (protocol.requiresApiKey && apiKey.trim().isEmpty) {
      return 'Configure an AI API key in Settings.';
    }
    return null;
  }

  AiAssistantConfig copyWith({
    AiApiProtocol? protocol,
    String? baseUrl,
    String? model,
    String? apiKey,
    Object? maxTokens = _unset,
    Object? temperature = _unset,
    bool? includeTerminalSelection,
    bool? includeRecentTerminalOutput,
  }) {
    return AiAssistantConfig(
      protocol: protocol ?? this.protocol,
      baseUrl: baseUrl ?? this.baseUrl,
      model: model ?? this.model,
      apiKey: apiKey ?? this.apiKey,
      maxTokens: identical(maxTokens, _unset)
          ? this.maxTokens
          : maxTokens as int?,
      temperature: identical(temperature, _unset)
          ? this.temperature
          : temperature as double?,
      includeTerminalSelection:
          includeTerminalSelection ?? this.includeTerminalSelection,
      includeRecentTerminalOutput:
          includeRecentTerminalOutput ?? this.includeRecentTerminalOutput,
    );
  }

  Map<String, Object?> toJson() {
    return <String, Object?>{
      'protocol': protocol.storageValue,
      'baseUrl': baseUrl,
      'model': model,
      'apiKey': apiKey,
      'maxTokens': maxTokens,
      'temperature': temperature,
      'includeTerminalSelection': includeTerminalSelection,
      'includeRecentTerminalOutput': includeRecentTerminalOutput,
    };
  }

  factory AiAssistantConfig.fromJson(Object? value) {
    if (value is! Map) {
      return const AiAssistantConfig();
    }
    final json = value.cast<String, Object?>();
    final protocol = AiApiProtocol.fromString(json['protocol']);
    final defaultBaseUrl = protocol.defaultBaseUrl;
    final baseUrl = (json['baseUrl'] as String?)?.trim();
    return AiAssistantConfig(
      protocol: protocol,
      baseUrl: baseUrl == null || baseUrl.isEmpty ? defaultBaseUrl : baseUrl,
      model: (json['model'] as String?)?.trim() ?? '',
      apiKey: (json['apiKey'] as String?)?.trim() ?? '',
      maxTokens: (json['maxTokens'] as num?)?.toInt(),
      temperature: (json['temperature'] as num?)?.toDouble(),
      includeTerminalSelection:
          json['includeTerminalSelection'] as bool? ?? true,
      includeRecentTerminalOutput:
          json['includeRecentTerminalOutput'] as bool? ?? true,
    );
  }
}

const Object _unset = Object();

AiAssistantConfig aiAssistantConfig = const AiAssistantConfig();

final ValueNotifier<AiAssistantConfig> aiAssistantConfigListenable =
    ValueNotifier(aiAssistantConfig);

void setAiAssistantConfig(AiAssistantConfig config) {
  aiAssistantConfig = config;
  aiAssistantConfigListenable.value = config;
}
