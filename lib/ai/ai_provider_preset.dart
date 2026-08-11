import 'package:flutter/foundation.dart';

@immutable
class AiProviderPreset {
  const AiProviderPreset({
    required this.id,
    required this.name,
    required this.protocol,
    required this.baseUrl,
    this.defaultModels = const [],
    this.websiteUrl,
    this.apiKeyUrl,
  });

  final String id;
  final String name;
  final String protocol; // 'openai' | 'anthropic' | 'google' | 'ollama'
  final String baseUrl;
  final List<String> defaultModels;
  final String? websiteUrl;
  final String? apiKeyUrl;

  factory AiProviderPreset.fromJson(String id, Map<String, Object?> json) {
    return AiProviderPreset(
      id: id,
      name: json['name'] as String? ?? id,
      protocol: json['protocol'] as String? ?? 'openai',
      baseUrl: json['base_url'] as String? ?? '',
      defaultModels:
          (json['default_models'] as List<dynamic>?)
              ?.map((e) => e.toString())
              .toList() ??
          const [],
      websiteUrl: json['website_url'] as String?,
      apiKeyUrl: json['api_key_url'] as String?,
    );
  }

  Map<String, Object?> toJson() {
    return {
      'name': name,
      'protocol': protocol,
      'base_url': baseUrl,
      'default_models': defaultModels,
      'website_url': websiteUrl,
      'api_key_url': apiKeyUrl,
    };
  }
}
