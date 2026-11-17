import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

enum AppLanguage {
  system,
  english,
  simplifiedChinese;

  static AppLanguage fromString(String? value) {
    return switch (value) {
      'en' => AppLanguage.english,
      'zh-CN' || 'zh' => AppLanguage.simplifiedChinese,
      _ => AppLanguage.system,
    };
  }

  String get configValue => switch (this) {
    AppLanguage.system => 'system',
    AppLanguage.english => 'en',
    AppLanguage.simplifiedChinese => 'zh-CN',
  };

  Locale? get locale => switch (this) {
    AppLanguage.system => null,
    AppLanguage.english => const Locale('en'),
    AppLanguage.simplifiedChinese => const Locale('zh', 'CN'),
  };

  String displayName(NautermLocalizations localizations) => switch (this) {
    AppLanguage.system => localizations.tr(
      'settings.general.language.system',
      fallback: 'System Default',
    ),
    AppLanguage.english => localizations.tr('language.english.autonym'),
    AppLanguage.simplifiedChinese => localizations.tr(
      'language.simplifiedChinese.autonym',
    ),
  };
}

AppLanguage appLanguage = AppLanguage.system;
final ValueNotifier<AppLanguage> appLanguageListenable =
    ValueNotifier<AppLanguage>(appLanguage);

void setAppLanguage(AppLanguage value) {
  appLanguage = value;
  appLanguageListenable.value = value;
}

@immutable
class NautermLocalizationPattern {
  const NautermLocalizationPattern(this.expression, this.replacement);

  final RegExp expression;
  final String replacement;

  String? translate(String source) {
    final match = expression.firstMatch(source);
    if (match == null) return null;
    var result = replacement;
    for (var index = 1; index <= match.groupCount; index++) {
      result = result.replaceAll('{$index}', match[index] ?? '');
    }
    return result;
  }
}

class NautermLocalizations {
  const NautermLocalizations(
    this.locale, {
    this.messages = const {},
    this.patterns = const [],
    this.keysByEnglishFallback = const {},
  });

  final Locale locale;
  final Map<String, String> messages;
  final List<NautermLocalizationPattern> patterns;
  final Map<String, String> keysByEnglishFallback;

  static NautermLocalizations current = const NautermLocalizations(
    Locale('en'),
  );

  static const supportedLocales = <Locale>[Locale('en'), Locale('zh', 'CN')];

  static const LocalizationsDelegate<NautermLocalizations> delegate =
      _NautermLocalizationsDelegate();
  static final Map<String, NautermLocalizations> _loaded = {};

  static String _cacheKey(Locale locale) =>
      locale.languageCode == 'zh' ? 'zh_CN' : 'en';

  static NautermLocalizations? cached(Locale locale) =>
      _loaded[_cacheKey(locale)];

  static NautermLocalizations of(BuildContext context) {
    return Localizations.of<NautermLocalizations>(
          context,
          NautermLocalizations,
        ) ??
        current;
  }

  String tr(
    String key, {
    String? fallback,
    Map<String, Object?> args = const {},
  }) {
    if (locale.languageCode == 'en' && fallback != null) {
      return _interpolateLocalizationArgs(fallback, args);
    }
    final resolvedKey = messages.containsKey(key)
        ? key
        : keysByEnglishFallback[key];
    final exact = resolvedKey == null ? null : messages[resolvedKey];
    if (exact != null) return _interpolateLocalizationArgs(exact, args);
    for (final pattern in patterns) {
      final translated = pattern.translate(key);
      if (translated != null) {
        return _interpolateLocalizationArgs(translated, args);
      }
    }
    return _interpolateLocalizationArgs(fallback ?? key, args);
  }

  static Future<NautermLocalizations> load(Locale locale) async {
    final asset = locale.languageCode == 'zh'
        ? 'assets/i18n/zh_CN.json'
        : 'assets/i18n/en.json';
    final loaded = await Future.wait([
      rootBundle.loadString(asset),
      if (asset != 'assets/i18n/en.json')
        rootBundle.loadString('assets/i18n/en.json'),
    ]);
    return parse(
      locale,
      loaded.first,
      englishJson: loaded.length == 1 ? loaded.first : loaded.last,
    );
  }

  @visibleForTesting
  static NautermLocalizations parse(
    Locale locale,
    String localizedJson, {
    String? englishJson,
  }) {
    final decoded = jsonDecode(localizedJson) as Map<String, dynamic>;
    final englishDecoded =
        jsonDecode(englishJson ?? localizedJson) as Map<String, dynamic>;
    final messages = <String, String>{};
    for (final entry in decoded.entries) {
      if (entry.key != '_patterns' && entry.value is String) {
        messages[entry.key] = entry.value as String;
      }
    }
    final patterns = <NautermLocalizationPattern>[
      for (final value in decoded['_patterns'] as List<dynamic>? ?? const [])
        if (value case {
          'pattern': final String pattern,
          'replacement': final String replacement,
        })
          NautermLocalizationPattern(RegExp(pattern), replacement),
    ];
    final keysByEnglishFallback = <String, String>{};
    for (final entry in englishDecoded.entries) {
      if (entry.key != '_patterns' && entry.value is String) {
        keysByEnglishFallback.putIfAbsent(
          entry.value as String,
          () => entry.key,
        );
      }
    }
    final localizations = NautermLocalizations(
      locale,
      messages: Map.unmodifiable(messages),
      patterns: List.unmodifiable(patterns),
      keysByEnglishFallback: Map.unmodifiable(keysByEnglishFallback),
    );
    _loaded[_cacheKey(locale)] = localizations;
    return localizations;
  }
}

String _interpolateLocalizationArgs(String message, Map<String, Object?> args) {
  var value = message;
  for (final entry in args.entries) {
    value = value.replaceAll('{${entry.key}}', entry.value?.toString() ?? '');
  }
  return value;
}

String tr(
  String key, {
  String? fallback,
  Map<String, Object?> args = const {},
}) {
  if (appLanguage == AppLanguage.english &&
      NautermLocalizations.current.locale.languageCode != 'en') {
    return _interpolateLocalizationArgs(fallback ?? key, args);
  }
  return NautermLocalizations.current.tr(key, fallback: fallback, args: args);
}

extension NautermLocalizationsContext on BuildContext {
  NautermLocalizations get l10n => NautermLocalizations.of(this);

  String tr(
    String key, {
    String? fallback,
    Map<String, Object?> args = const {},
  }) => l10n.tr(key, fallback: fallback, args: args);
}

class _NautermLocalizationsDelegate
    extends LocalizationsDelegate<NautermLocalizations> {
  const _NautermLocalizationsDelegate();

  @override
  bool isSupported(Locale locale) =>
      locale.languageCode == 'en' || locale.languageCode == 'zh';

  @override
  Future<NautermLocalizations> load(Locale locale) {
    final cached = NautermLocalizations.cached(locale);
    if (cached != null) {
      NautermLocalizations.current = cached;
      return SynchronousFuture(cached);
    }
    return NautermLocalizations.load(locale).then((localizations) {
      NautermLocalizations.current = localizations;
      return localizations;
    });
  }

  @override
  bool shouldReload(_NautermLocalizationsDelegate old) => false;
}
