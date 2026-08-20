import 'dart:collection';

import '../ai/ai_conversation.dart';
import '../terminal/terminal_controller.dart';

typedef AiConversationRelease = void Function(
  AiConversationController conversation,
);

/// Owns idempotent disposal of terminal and AI session resources.
///
/// The workspace may reach the same resource through a tab, workspace, history
/// watcher, and shutdown traversal. Keeping the identity sets here prevents
/// those paths from releasing persistence or native resources more than once.
class TerminalLifecycleService {
  TerminalLifecycleService({required this._releaseConversation});

  final AiConversationRelease _releaseConversation;
  final Set<TerminalController> _disposedTerminals = HashSet.identity();
  final Set<AiConversationController> _disposedConversations =
      HashSet.identity();

  void disposeTerminal(TerminalController controller) {
    if (!_disposedTerminals.add(controller) || controller.isDisposed) {
      return;
    }
    controller.dispose();
  }

  void disposeConversation(AiConversationController conversation) {
    if (!_disposedConversations.add(conversation)) {
      return;
    }
    _releaseConversation(conversation);
    conversation.dispose();
  }
}
