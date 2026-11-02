import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart' show mapEquals;
import 'package:anthropic_sdk_dart/anthropic_sdk_dart.dart' as anthropic;
import 'package:googleai_dart/googleai_dart.dart' as google;
import 'package:http/io_client.dart';
import 'package:ollama_dart/ollama_dart.dart' as ollama;
import 'package:openai_dart/openai_dart.dart' as openai;

import 'ai_config.dart';
import 'ai_attachment.dart';

enum AiChatRole { system, user, assistant, tool }

sealed class AiStreamEvent {
  const AiStreamEvent();
}

class AiTextDelta extends AiStreamEvent {
  const AiTextDelta(this.text);

  final String text;
}

class AiProviderMetadata extends AiStreamEvent {
  const AiProviderMetadata(this.metadata);
  final Map<String, Object?> metadata;
}

class AiToolCall extends AiStreamEvent {
  const AiToolCall({
    required this.id,
    required this.name,
    required this.arguments,
    this.metadata = const {},
  });

  factory AiToolCall.fromJson(Map<String, Object?> json) {
    return AiToolCall(
      id: json['id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      arguments: _nestedMap(json['arguments']) ?? const {},
      metadata: _toolMetadata(json, const {'id', 'name', 'arguments'}),
    );
  }

  final String id;
  final String name;
  final Map<String, Object?> arguments;

  /// Provider replay fields remain opaque when another frontend saves history.
  final Map<String, Object?> metadata;

  Map<String, Object?> toJson() => {
    ...metadata,
    'id': id,
    'name': name,
    'arguments': arguments,
  };

  String? get terminalCommand {
    if (name != 'run_terminal_command') {
      return null;
    }
    return _nonEmptyString(arguments['command']);
  }

  String? get terminalCommandExplanation {
    if (name != 'run_terminal_command') {
      return null;
    }
    return _nonEmptyString(arguments['explanation']);
  }
}

class AiChatMessage {
  const AiChatMessage({
    this.persistenceId,
    required this.role,
    required this.content,
    this.context = '',
    this.sequence = 0,
    this.toolCalls = const [],
    this.toolResult,
    this.attachments = const [],
    this.providerMetadata = const {},
    this.createdAt,
    this.updatedAt,
  });

  final String? persistenceId;
  final AiChatRole role;
  final String content;
  final String context;
  final int sequence;
  final List<AiToolCall> toolCalls;
  final AiToolResult? toolResult;
  final List<AiAttachment> attachments;
  final Map<String, Object?> providerMetadata;
  final DateTime? createdAt;
  final DateTime? updatedAt;

  String get requestContent {
    final trimmedContext = context.trim();
    if (trimmedContext.isEmpty) {
      return content;
    }
    return '$content\n\n$trimmedContext';
  }

  AiChatMessage copyWith({
    String? content,
    List<AiToolCall>? toolCalls,
    AiToolResult? toolResult,
    List<AiAttachment>? attachments,
    Map<String, Object?>? providerMetadata,
    DateTime? updatedAt,
  }) {
    return AiChatMessage(
      persistenceId: persistenceId,
      role: role,
      content: content ?? this.content,
      context: context,
      sequence: sequence,
      toolCalls: toolCalls ?? this.toolCalls,
      toolResult: toolResult ?? this.toolResult,
      attachments: attachments ?? this.attachments,
      providerMetadata:
          providerMetadata ??
          (content != null && content != this.content
              ? const {}
              : this.providerMetadata),
      createdAt: createdAt,
      updatedAt: updatedAt ?? this.updatedAt,
    );
  }
}

class AiToolResult {
  const AiToolResult({
    required this.toolCallId,
    required this.name,
    required this.content,
    this.isError = false,
    this.metadata = const {},
  });

  factory AiToolResult.fromJson(Map<String, Object?> json) {
    return AiToolResult(
      toolCallId: json['tool_call_id'] as String? ?? '',
      name: json['name'] as String? ?? '',
      content: json['content'] as String? ?? '',
      isError: json['is_error'] as bool? ?? false,
      metadata: _toolMetadata(json, const {
        'tool_call_id',
        'name',
        'content',
        'is_error',
      }),
    );
  }

  final String toolCallId;
  final String name;
  final String content;
  final bool isError;
  final Map<String, Object?> metadata;

  Map<String, Object?> toJson() => {
    ...metadata,
    'tool_call_id': toolCallId,
    'name': name,
    'content': content,
    'is_error': isError,
  };
}

Map<String, Object?> _toolMetadata(
  Map<String, Object?> json,
  Set<String> known,
) => Map.unmodifiable({
  for (final entry in json.entries)
    if (!known.contains(entry.key)) entry.key: entry.value,
});

class AiProtocolException implements Exception {
  const AiProtocolException(this.message, {this.statusCode});

  final String message;
  final int? statusCode;

  @override
  String toString() => message;
}

typedef AiHttpClientFactory = HttpClient Function();

class AiProtocolClient {
  AiProtocolClient({AiHttpClientFactory? httpClientFactory})
    : _httpClientFactory = httpClientFactory ?? HttpClient.new;

  final AiHttpClientFactory _httpClientFactory;

  Stream<AiStreamEvent> streamCompletion({
    required AiAssistantConfig config,
    required List<AiChatMessage> messages,
    bool enableTerminalTool = false,
  }) {
    HttpClient? activeClient;
    return _cancellableStream(
      source: () {
        final client = _httpClientFactory();
        activeClient = client;
        client.connectionTimeout = const Duration(seconds: 20);
        return switch (config.protocol) {
          AiApiProtocol.openAi => _streamOpenAi(
            client,
            config,
            messages,
            enableTerminalTool: enableTerminalTool,
          ),
          AiApiProtocol.anthropic => _streamAnthropic(
            client,
            config,
            messages,
            enableTerminalTool: enableTerminalTool,
          ),
          AiApiProtocol.google => _streamGoogle(
            client,
            config,
            messages,
            enableTerminalTool: enableTerminalTool,
          ),
          AiApiProtocol.ollama => _streamOllama(
            client,
            config,
            messages,
            enableTerminalTool: enableTerminalTool,
          ),
        };
      },
      abort: () => activeClient?.close(force: true),
    );
  }

  Stream<AiStreamEvent> _streamOpenAi(
    HttpClient client,
    AiAssistantConfig config,
    List<AiChatMessage> messages, {
    required bool enableTerminalTool,
  }) async* {
    final toolCalls = <int, _ToolCallAccumulator>{};
    final httpClient = IOClient(client);
    final sdk = openai.OpenAIClient(
      config: openai.OpenAIConfig(
        authProvider: openai.ApiKeyProvider(config.apiKey.trim()),
        baseUrl: config.baseUrl.trim(),
        retryPolicy: const openai.RetryPolicy(maxRetries: 0),
      ),
      httpClient: httpClient,
      streamClientFactory: () => httpClient,
    );
    try {
      final request = openai.ChatCompletionCreateRequest.fromJson(
        <String, dynamic>{
          'model': config.model.trim(),
          'messages': [for (final message in messages) _openAiMessage(message)],
          if (config.maxTokens != null) 'max_tokens': config.maxTokens,
          if (config.temperature != null) 'temperature': config.temperature,
          if (enableTerminalTool) 'tools': [_openAiTerminalCommandTool],
        },
      );
      await for (final event in sdk.chat.completions.createStream(request)) {
        final text = event.textDelta;
        if (text != null && text.isNotEmpty) yield AiTextDelta(text);
        for (final choice in event.choices ?? const []) {
          for (final toolCall in choice.delta.toolCalls ?? const []) {
            final accumulator = toolCalls.putIfAbsent(
              toolCall.index,
              _ToolCallAccumulator.new,
            );
            accumulator.id += toolCall.id ?? '';
            accumulator.name += toolCall.function?.name ?? '';
            accumulator.arguments += toolCall.function?.arguments ?? '';
          }
        }
      }
      for (final toolCall
          in toolCalls.entries.toList()
            ..sort((left, right) => left.key.compareTo(right.key))) {
        final event = toolCall.value.toEvent();
        if (event != null) {
          yield event;
        }
      }
    } on openai.ApiException catch (error) {
      throw AiProtocolException(error.message, statusCode: error.statusCode);
    } finally {
      sdk.close();
      httpClient.close();
      client.close(force: true);
    }
  }

  Stream<AiStreamEvent> _streamAnthropic(
    HttpClient client,
    AiAssistantConfig config,
    List<AiChatMessage> messages, {
    required bool enableTerminalTool,
  }) async* {
    final toolCalls = <int, _ToolCallAccumulator>{};
    final httpClient = IOClient(client);
    final sdk = anthropic.AnthropicClient(
      config: anthropic.AnthropicConfig(
        baseUrl: _withoutTrailingVersion(config.baseUrl),
        authProvider: anthropic.ApiKeyProvider(config.apiKey.trim()),
        retryPolicy: const anthropic.RetryPolicy(maxRetries: 0),
      ),
      httpClient: httpClient,
    );
    try {
      final system = messages
          .where((message) => message.role == AiChatRole.system)
          .map((message) => message.content)
          .where((content) => content.trim().isNotEmpty)
          .join('\n\n');
      final request = anthropic.MessageCreateRequest.fromJson(<String, dynamic>{
        'model': config.model.trim(),
        'max_tokens': config.maxTokens ?? 4096,
        if (config.temperature != null) 'temperature': config.temperature,
        if (system.isNotEmpty) 'system': system,
        'messages': _anthropicMessages(messages),
        if (enableTerminalTool) 'tools': [_anthropicTerminalCommandTool],
      });
      await for (final event in sdk.messages.createStream(request)) {
        if (event is anthropic.ContentBlockStartEvent &&
            event.contentBlock is anthropic.ToolUseBlock) {
          final block = event.contentBlock as anthropic.ToolUseBlock;
          final accumulator = toolCalls.putIfAbsent(
            event.index,
            _ToolCallAccumulator.new,
          );
          accumulator.id = block.id;
          accumulator.name = block.name;
          if (block.input.isNotEmpty) {
            accumulator.arguments = jsonEncode(block.input);
          }
        }
        if (event is anthropic.ContentBlockDeltaEvent) {
          final delta = event.delta;
          if (delta is anthropic.TextDelta && delta.text.isNotEmpty) {
            yield AiTextDelta(delta.text);
          } else if (delta is anthropic.InputJsonDelta) {
            toolCalls
                    .putIfAbsent(event.index, _ToolCallAccumulator.new)
                    .arguments +=
                delta.partialJson;
          }
        }
      }
      yield* _toolCallEvents(toolCalls);
    } on anthropic.ApiException catch (error) {
      throw AiProtocolException(error.message, statusCode: error.statusCode);
    } finally {
      sdk.close();
      httpClient.close();
      client.close(force: true);
    }
  }

  Stream<AiStreamEvent> _streamGoogle(
    HttpClient client,
    AiAssistantConfig config,
    List<AiChatMessage> messages, {
    required bool enableTerminalTool,
  }) async* {
    final httpClient = IOClient(client);
    final sdk = google.GoogleAIClient(
      config: google.GoogleAIConfig(
        baseUrl: _withoutTrailingVersion(config.baseUrl),
        apiMode: google.ApiMode.googleAI,
        apiVersion: google.ApiVersion.v1beta,
        authProvider: google.ApiKeyProvider(config.apiKey.trim()),
        retryPolicy: const google.RetryPolicy(maxRetries: 0),
      ),
      httpClient: httpClient,
    );
    try {
      final request = google.GenerateContentRequest.fromJson(
        _googleRequest(messages, config, enableTerminalTool),
      );
      var toolIndex = 0;
      final responseParts = <Map<String, Object?>>[];
      var metadataBytes = 0;
      var completed = false;
      await for (final event in sdk.models.streamGenerateContent(
        model: config.model.trim(),
        request: request,
      )) {
        for (final candidate in event.candidates ?? const []) {
          completed = candidate.toJson()['finishReason'] == 'STOP' || completed;
          for (final part in candidate.content?.parts ?? const []) {
            final json = part.toJson();
            metadataBytes += utf8.encode(jsonEncode(json)).length + 1;
            if (responseParts.length >= 65536 ||
                metadataBytes > 16 * 1024 * 1024) {
              throw const AiProtocolException(
                'Gemini response metadata exceeds the size limit.',
              );
            }
            responseParts.add(Map<String, Object?>.from(json));
            final text = json['text'];
            if (text is String && json['thought'] != true) {
              yield AiTextDelta(text);
            }
            final call = _nestedMap(json['functionCall']);
            final name = _nonEmptyString(call?['name']);
            if (name != null) {
              yield AiToolCall(
                id: _nonEmptyString(call?['id']) ?? 'google-${toolIndex++}',
                name: name,
                arguments: (_nestedMap(call?['args']) ?? const {}),
                metadata: {
                  'omit_provider_id': _nonEmptyString(call?['id']) == null,
                },
              );
            }
          }
        }
      }
      if (completed && responseParts.isNotEmpty) {
        yield AiProviderMetadata({'google_parts': responseParts});
      }
    } on google.ApiException catch (error) {
      throw AiProtocolException(error.message, statusCode: error.statusCode);
    } finally {
      sdk.close();
      httpClient.close();
      client.close(force: true);
    }
  }

  Stream<AiStreamEvent> _streamOllama(
    HttpClient client,
    AiAssistantConfig config,
    List<AiChatMessage> messages, {
    required bool enableTerminalTool,
  }) async* {
    final httpClient = IOClient(client);
    final apiKey = config.apiKey.trim();
    final sdk = ollama.OllamaClient(
      config: ollama.OllamaConfig(
        baseUrl: config.baseUrl.trim(),
        authProvider: apiKey.isEmpty
            ? null
            : ollama.BearerTokenProvider(apiKey),
        retryPolicy: const ollama.RetryPolicy(maxRetries: 0),
      ),
      httpClient: httpClient,
    );
    try {
      final request = ollama.ChatRequest.fromJson(
        _ollamaRequest(messages, config, enableTerminalTool),
      );
      var toolIndex = 0;
      await for (final event in sdk.chat.createStream(request: request)) {
        final message = event.message;
        final text = message?.content;
        if (text != null && text.isNotEmpty) yield AiTextDelta(text);
        for (final call in message?.toolCalls ?? const []) {
          final function = call.function;
          if (function != null) {
            yield AiToolCall(
              id: 'ollama-${toolIndex++}',
              name: function.name,
              arguments: function.arguments ?? const {},
            );
          }
        }
      }
    } on ollama.ApiException catch (error) {
      throw AiProtocolException(error.message, statusCode: error.statusCode);
    } finally {
      sdk.close();
      httpClient.close();
      client.close(force: true);
    }
  }
}

Stream<AiStreamEvent> _toolCallEvents(
  Map<int, _ToolCallAccumulator> toolCalls,
) async* {
  for (final entry
      in toolCalls.entries.toList()
        ..sort((left, right) => left.key.compareTo(right.key))) {
    final event = entry.value.toEvent();
    if (event != null) yield event;
  }
}

Map<String, dynamic> _googleRequest(
  List<AiChatMessage> messages,
  AiAssistantConfig config,
  bool enableTerminalTool,
) {
  final generationConfig = <String, dynamic>{
    if (config.maxTokens != null) 'maxOutputTokens': config.maxTokens,
    if (config.temperature != null) 'temperature': config.temperature,
  };
  final system = messages
      .where((message) => message.role == AiChatRole.system)
      .map((message) => message.requestContent)
      .where((content) => content.trim().isNotEmpty)
      .join('\n\n');
  return <String, dynamic>{
    'contents': _googleMessages(messages),
    if (system.isNotEmpty)
      'systemInstruction': <String, dynamic>{
        'parts': [
          <String, dynamic>{'text': system},
        ],
      },
    if (generationConfig.isNotEmpty) 'generationConfig': generationConfig,
    if (enableTerminalTool)
      'tools': [
        <String, dynamic>{
          'functionDeclarations': [
            <String, dynamic>{
              'name': 'run_terminal_command',
              'description': _terminalCommandDescription,
              'parameters': _terminalCommandParameters,
            },
          ],
        },
      ],
  };
}

List<Map<String, dynamic>> _googleMessages(List<AiChatMessage> messages) {
  final encoded = <Map<String, dynamic>>[];
  var previousResult = false;
  for (final message in messages) {
    if (message.role == AiChatRole.system) continue;
    final value = _googleMessage(message);
    final result = message.toolResult != null;
    if (result && previousResult) {
      (encoded.last['parts'] as List).addAll(value['parts'] as List);
    } else {
      encoded.add(value);
    }
    previousResult = result;
  }
  return encoded;
}

Map<String, dynamic> _googleMessage(AiChatMessage message) {
  final result = message.toolResult;
  if (result != null) {
    return <String, dynamic>{
      'role': 'user',
      'parts': [
        <String, dynamic>{
          'functionResponse': <String, dynamic>{
            if (result.metadata['omit_provider_id'] != true)
              'id': result.toolCallId,
            'name': result.name,
            'response': <String, dynamic>{
              'content': result.content,
              if (result.isError) 'is_error': true,
            },
          },
        },
      ],
    };
  }
  final replay =
      message.providerMetadata['google_parts'] ??
      message.toolCalls.firstOrNull?.metadata['google_parts'];
  if (replay != null) {
    if (replay is! List ||
        replay.length > 65536 ||
        utf8.encode(jsonEncode(replay)).length > 16 * 1024 * 1024 ||
        message.toolCalls
            .skip(1)
            .any((call) => call.metadata.containsKey('google_parts'))) {
      throw const AiProtocolException('Invalid Gemini response metadata.');
    }
    final functions = <Map<String, Object?>>[
      for (final part in replay)
        if (_nestedMap(part)?['functionCall'] case final Map function)
          function.map((key, value) => MapEntry(key.toString(), value)),
    ];
    if (functions.length != message.toolCalls.length) {
      throw const AiProtocolException('Gemini response tools no longer match.');
    }
    final visibleText = StringBuffer();
    for (final raw in replay) {
      final part = _nestedMap(raw);
      if (part == null ||
          (part['text'] != null && part['text'] is! String) ||
          (part['thought'] != null && part['thought'] is! bool) ||
          (part['thoughtSignature'] != null &&
              part['thoughtSignature'] is! String) ||
          (message.toolCalls.isEmpty &&
              part.keys.any(
                (key) => !const {
                  'text',
                  'thought',
                  'thoughtSignature',
                }.contains(key),
              ))) {
        throw const AiProtocolException('Invalid Gemini response part.');
      }
      if (part['thought'] != true) {
        visibleText.write(part['text'] ?? '');
      }
    }
    if (message.role != AiChatRole.assistant ||
        (message.toolCalls.isEmpty &&
            visibleText.toString() != message.content)) {
      throw const AiProtocolException(
        'Gemini response content no longer matches.',
      );
    }
    for (var index = 0; index < functions.length; index++) {
      final function = functions[index];
      final call = message.toolCalls[index];
      final id = _nonEmptyString(function['id']);
      if (function['name'] != call.name ||
          !mapEquals(_nestedMap(function['args']), call.arguments) ||
          (call.metadata['omit_provider_id'] == true
              ? id != null
              : id != call.id)) {
        throw const AiProtocolException(
          'A Gemini command changed since it was proposed.',
        );
      }
    }
    return {'role': 'model', 'parts': List.of(replay)};
  }
  return <String, dynamic>{
    'role': message.role == AiChatRole.assistant ? 'model' : 'user',
    'parts': [
      if (message.requestContent.isNotEmpty)
        <String, dynamic>{'text': message.requestContent},
      for (final attachment in message.attachments)
        if (attachment.kind == AiAttachmentKind.text)
          <String, dynamic>{'text': _textAttachmentContent(attachment)}
        else
          <String, dynamic>{
            'inlineData': <String, dynamic>{
              'mimeType': attachment.mimeType,
              'data': attachment.base64Data,
            },
          },
      for (final call in message.toolCalls)
        <String, dynamic>{
          if (call.metadata['thought_signature'] case final String signature)
            'thoughtSignature': signature,
          'functionCall': <String, dynamic>{
            if (call.metadata['thought_signature'] != null &&
                call.metadata['omit_provider_id'] != true)
              'id': call.id,
            'name': call.name,
            'args': call.arguments,
          },
        },
    ],
  };
}

Map<String, dynamic> _ollamaRequest(
  List<AiChatMessage> messages,
  AiAssistantConfig config,
  bool enableTerminalTool,
) {
  final options = <String, dynamic>{
    if (config.maxTokens != null) 'num_predict': config.maxTokens,
    if (config.temperature != null) 'temperature': config.temperature,
  };
  return <String, dynamic>{
    'model': config.model.trim(),
    'messages': [for (final message in messages) _ollamaMessage(message)],
    if (options.isNotEmpty) 'options': options,
    if (enableTerminalTool)
      'tools': [
        <String, dynamic>{
          'type': 'function',
          'function': <String, dynamic>{
            'name': 'run_terminal_command',
            'description': _terminalCommandDescription,
            'parameters': _terminalCommandParameters,
          },
        },
      ],
  };
}

Map<String, dynamic> _ollamaMessage(AiChatMessage message) {
  final result = message.toolResult;
  if (result != null) {
    return <String, dynamic>{'role': 'tool', 'content': result.content};
  }
  final textAttachments = message.attachments
      .where((attachment) => attachment.kind == AiAttachmentKind.text)
      .map(_textAttachmentContent);
  return <String, dynamic>{
    'role': message.role.name,
    'content': [
      message.requestContent,
      ...textAttachments,
    ].where((part) => part.isNotEmpty).join('\n\n'),
    if (message.attachments.any(
      (attachment) => attachment.kind == AiAttachmentKind.image,
    ))
      'images': [
        for (final attachment in message.attachments)
          if (attachment.kind == AiAttachmentKind.image) attachment.base64Data,
      ],
    if (message.toolCalls.isNotEmpty)
      'tool_calls': [
        for (final call in message.toolCalls)
          <String, dynamic>{
            'function': <String, dynamic>{
              'name': call.name,
              'arguments': call.arguments,
            },
          },
      ],
  };
}

String _withoutTrailingVersion(String baseUrl) => baseUrl
    .trim()
    .replaceFirst(RegExp(r'/v\d+(?:beta\d*)?/?$'), '')
    .replaceFirst(RegExp(r'/+$'), '');

Stream<T> _cancellableStream<T>({
  required Stream<T> Function() source,
  required void Function() abort,
}) {
  StreamSubscription<T>? sourceSubscription;
  late final StreamController<T> controller;
  controller = StreamController<T>(
    sync: true,
    onListen: () {
      try {
        sourceSubscription = source().listen(
          controller.add,
          onError: controller.addError,
          onDone: controller.close,
        );
      } on Object catch (error, stackTrace) {
        controller.addError(error, stackTrace);
        unawaited(controller.close());
      }
    },
    onPause: () => sourceSubscription?.pause(),
    onResume: () => sourceSubscription?.resume(),
    onCancel: () async {
      abort();
      try {
        await sourceSubscription?.cancel();
      } on Object {
        // Aborting an in-flight request completes its pending I/O with an
        // exception, which is expected after the consumer cancels the stream.
      }
    },
  );
  return controller.stream;
}

Map<String, Object?> _openAiMessage(AiChatMessage message) {
  final toolResult = message.toolResult;
  if (toolResult != null) {
    return <String, Object?>{
      'role': 'tool',
      'tool_call_id': toolResult.toolCallId,
      'content': toolResult.content,
    };
  }
  if (message.role == AiChatRole.user && message.attachments.isNotEmpty) {
    return <String, Object?>{
      'role': 'user',
      'content': <Object?>[
        if (message.requestContent.isNotEmpty)
          <String, Object?>{'type': 'text', 'text': message.requestContent},
        for (final attachment in message.attachments)
          if (attachment.kind == AiAttachmentKind.text)
            <String, Object?>{
              'type': 'text',
              'text': _textAttachmentContent(attachment),
            }
          else
            <String, Object?>{
              'type': 'image_url',
              'image_url': <String, Object?>{
                'url':
                    'data:${attachment.mimeType};base64,${attachment.base64Data}',
              },
            },
      ],
    };
  }
  return <String, Object?>{
    'role': message.role.name,
    'content': message.toolCalls.isNotEmpty && message.content.isEmpty
        ? null
        : message.requestContent,
    if (message.toolCalls.isNotEmpty)
      'tool_calls': [
        for (final toolCall in message.toolCalls)
          <String, Object?>{
            'id': toolCall.id,
            'type': 'function',
            'function': <String, Object?>{
              'name': toolCall.name,
              'arguments': jsonEncode(toolCall.arguments),
            },
          },
      ],
  };
}

Map<String, Object?> _anthropicMessage(AiChatMessage message) {
  final toolResult = message.toolResult;
  if (toolResult != null) {
    return <String, Object?>{
      'role': 'user',
      'content': <Object?>[
        <String, Object?>{
          'type': 'tool_result',
          'tool_use_id': toolResult.toolCallId,
          'content': toolResult.content,
          if (toolResult.isError) 'is_error': true,
        },
      ],
    };
  }
  if (message.role == AiChatRole.user && message.attachments.isNotEmpty) {
    return <String, Object?>{
      'role': 'user',
      'content': <Object?>[
        if (message.requestContent.isNotEmpty)
          <String, Object?>{'type': 'text', 'text': message.requestContent},
        for (final attachment in message.attachments)
          if (attachment.kind == AiAttachmentKind.text)
            <String, Object?>{
              'type': 'text',
              'text': _textAttachmentContent(attachment),
            }
          else
            <String, Object?>{
              'type': 'image',
              'source': <String, Object?>{
                'type': 'base64',
                'media_type': attachment.mimeType,
                'data': attachment.base64Data,
              },
            },
      ],
    };
  }
  if (message.toolCalls.isEmpty) {
    return <String, Object?>{
      'role': message.role.name,
      'content': message.requestContent,
    };
  }
  return <String, Object?>{
    'role': 'assistant',
    'content': <Object?>[
      if (message.content.isNotEmpty)
        <String, Object?>{'type': 'text', 'text': message.content},
      for (final toolCall in message.toolCalls)
        <String, Object?>{
          'type': 'tool_use',
          'id': toolCall.id,
          'name': toolCall.name,
          'input': toolCall.arguments,
        },
    ],
  };
}

List<Map<String, Object?>> _anthropicMessages(
  Iterable<AiChatMessage> messages,
) {
  final encoded = <Map<String, Object?>>[];
  List<Object?>? toolResults;

  void flushToolResults() {
    final results = toolResults;
    if (results == null || results.isEmpty) {
      return;
    }
    encoded.add(<String, Object?>{'role': 'user', 'content': results});
    toolResults = null;
  }

  for (final message in messages) {
    if (message.role == AiChatRole.system) {
      continue;
    }
    final toolResult = message.toolResult;
    if (toolResult != null) {
      (toolResults ??= <Object?>[]).add(<String, Object?>{
        'type': 'tool_result',
        'tool_use_id': toolResult.toolCallId,
        'content': <Object?>[
          <String, Object?>{'type': 'text', 'text': toolResult.content},
        ],
        if (toolResult.isError) 'is_error': true,
      });
      continue;
    }
    flushToolResults();
    encoded.add(_anthropicMessage(message));
  }
  flushToolResults();
  return encoded;
}

String _textAttachmentContent(AiAttachment attachment) {
  return jsonEncode(<String, Object?>{
    'type': 'text_attachment',
    'name': attachment.name,
    'mime_type': attachment.mimeType,
    'content': attachment.text ?? '',
  });
}

const Map<String, Object?> _terminalCommandParameters = <String, Object?>{
  'type': 'object',
  'properties': <String, Object?>{
    'command': <String, Object?>{
      'type': 'string',
      'description': 'The exact command to run in the active terminal.',
    },
    'explanation': <String, Object?>{
      'type': 'string',
      'description': 'A concise plain-language explanation of what the command does, why it is needed, and any important risk or side effect.',
    },
  },
  'required': <String>['command', 'explanation'],
  'additionalProperties': false,
};

const String _terminalCommandDescription =
    'Propose a command with an explanation for the user to review and run in the active terminal.';

const Map<String, Object?> _openAiTerminalCommandTool = <String, Object?>{
  'type': 'function',
  'function': <String, Object?>{
    'name': 'run_terminal_command',
    'description': _terminalCommandDescription,
    'parameters': _terminalCommandParameters,
  },
};

const Map<String, Object?> _anthropicTerminalCommandTool = <String, Object?>{
  'name': 'run_terminal_command',
  'description': _terminalCommandDescription,
  'input_schema': _terminalCommandParameters,
};

class _ToolCallAccumulator {
  String id = '';
  String name = '';
  String arguments = '';

  AiToolCall? toEvent() {
    if (name.isEmpty || arguments.isEmpty) {
      return null;
    }
    try {
      return AiToolCall(id: id, name: name, arguments: _jsonMap(arguments));
    } on FormatException {
      return null;
    }
  }
}

Map<String, Object?> _jsonMap(String source) {
  final decoded = jsonDecode(source);
  if (decoded is! Map) {
    throw const FormatException('Expected a JSON object.');
  }
  return decoded.cast<String, Object?>();
}

Map<String, Object?>? _nestedMap(Object? value) {
  return value is Map ? value.cast<String, Object?>() : null;
}

String? _nonEmptyString(Object? value) {
  if (value is! String || value.isEmpty) {
    return null;
  }
  return value;
}
