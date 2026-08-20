import 'dart:async';

import 'package:flutter/foundation.dart';

import '../ai/ai_conversation.dart';
import '../app/nauterm_log.dart';
import 'nauterm_data_store.dart';

@immutable
class AiConversationPersistenceTarget {
  const AiConversationPersistenceTarget({required this.scope, this.hostUuid});

  final String scope;
  final String? hostUuid;
}

/// Owns AI conversation persistence timing and database operations.
///
/// UI code remains responsible for choosing a persistence target and asking
/// for destructive-action confirmation. This repository owns listeners,
/// debounce timers, save ordering, and history reads/writes.
class AiConversationRepository {
  AiConversationRepository({required this._dataStore});

  static const Duration _saveDebounce = Duration(milliseconds: 400);

  final NautermDataStore? Function() _dataStore;
  final Map<AiConversationController, VoidCallback> _listeners = {};
  final Map<AiConversationController, AiConversationPersistenceTarget>
  _targets = {};
  final Map<AiConversationController, Timer> _saveTimers = {};

  Set<AiConversationController> get watchedConversations =>
      _listeners.keys.toSet();

  void watch(
    AiConversationController conversation,
    AiConversationPersistenceTarget target,
  ) {
    _targets[conversation] = target;
    if (_listeners.containsKey(conversation)) return;
    void listener() => scheduleSave(conversation);
    _listeners[conversation] = listener;
    conversation.addListener(listener);
  }

  void scheduleSave(AiConversationController conversation) {
    _saveTimers.remove(conversation)?.cancel();
    if (!_canSave(conversation)) return;
    _saveTimers[conversation] = Timer(_saveDebounce, () {
      _saveTimers.remove(conversation);
      saveNow(conversation);
    });
  }

  bool saveNow(AiConversationController conversation) {
    _saveTimers.remove(conversation)?.cancel();
    final store = _dataStore();
    final target = _targets[conversation];
    if (store == null || target == null) return false;
    if (conversation.sending || conversation.hasRunningTerminalCommand) {
      return false;
    }
    if (_isEmptyUnsaved(conversation)) return true;
    try {
      conversation.saveTo(
        store,
        scope: target.scope,
        hostUuid: target.hostUuid,
      );
      return true;
    } on Object catch (error, stackTrace) {
      NautermLog.warning(
        'ai',
        'Unable to save AI conversation.',
        error: error,
        stackTrace: stackTrace,
      );
      return false;
    }
  }

  void release(AiConversationController conversation) {
    conversation.prepareForClose();
    saveNow(conversation);
    final listener = _listeners.remove(conversation);
    if (listener != null) conversation.removeListener(listener);
    _targets.remove(conversation);
  }

  Future<List<AiConversationEntry>> loadHistory(
    AiConversationController conversation,
  ) async {
    _saveTimers.remove(conversation)?.cancel();
    if (!saveNow(conversation)) {
      throw StateError('Unable to save the current AI conversation.');
    }
    final store = _dataStore();
    final target = _targets[conversation];
    if (store == null || target == null) return const [];
    return store.listAiConversations(
      scope: target.scope,
      hostUuid: target.hostUuid,
      limit: 100,
    );
  }

  Future<bool> openHistory(
    AiConversationController conversation,
    AiConversationEntry entry,
  ) async {
    _saveTimers.remove(conversation)?.cancel();
    if (!saveNow(conversation)) return false;
    final uuid = entry.uuid;
    final stored = uuid == null ? null : _dataStore()?.getAiConversation(uuid);
    return stored != null && conversation.load(stored);
  }

  bool deleteHistory(
    AiConversationController conversation,
    AiConversationEntry entry,
  ) {
    final uuid = entry.uuid;
    final store = _dataStore();
    if (uuid == null || store == null) return false;
    _saveTimers.remove(conversation)?.cancel();
    final deleted = store.deleteAiConversation(uuid) > 0;
    if (deleted && conversation.persistenceId == uuid) conversation.clear();
    return deleted;
  }

  void dispose() {
    for (final timer in _saveTimers.values) {
      timer.cancel();
    }
    _saveTimers.clear();
    for (final entry in _listeners.entries.toList(growable: false)) {
      entry.key.removeListener(entry.value);
    }
    _listeners.clear();
    _targets.clear();
  }

  bool _canSave(AiConversationController conversation) {
    return !conversation.sending &&
        !conversation.hasRunningTerminalCommand &&
        !_isEmptyUnsaved(conversation);
  }

  bool _isEmptyUnsaved(AiConversationController conversation) {
    return conversation.persistenceId == null &&
        conversation.messages.isEmpty &&
        conversation.terminalCommands.isEmpty;
  }
}
