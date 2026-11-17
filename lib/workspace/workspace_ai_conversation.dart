part of 'nauterm_workspace.dart';

class _AiAssistantNotice extends StatelessWidget {
  const _AiAssistantNotice({
    required this.message,
    required this.icon,
    required this.colors,
    this.muted = false,
  });

  final String message;
  final IconData icon;
  final _AiAssistantColors colors;
  final bool muted;

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.fromLTRB(12, 8, 12, 0),
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 7),
      decoration: BoxDecoration(
        color: Color.lerp(
          colors.background,
          muted ? colors.foreground : colors.accent,
          muted ? 0.035 : 0.055,
        ),
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: colors.border),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 14, color: muted ? colors.muted : colors.accent),
          SizedBox(width: 7),
          Expanded(
            child: Text(
              tr(message),
              style: TextStyle(
                color: muted ? colors.muted : colors.foreground,
                fontSize: 11.5,
                height: 1.35,
                letterSpacing: 0,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _AiComposerIconButton extends StatelessWidget {
  const _AiComposerIconButton({
    super.key,
    required this.tooltip,
    required this.onPressed,
    required this.icon,
    required this.color,
    this.progress = false,
    this.fillColor,
  });

  final String tooltip;
  final VoidCallback? onPressed;
  final IconData icon;
  final Color color;
  final bool progress;
  final Color? fillColor;

  @override
  Widget build(BuildContext context) {
    return IconButton(
      tooltip: tooltip,
      onPressed: onPressed,
      icon: progress
          ? SizedBox(
              width: 14,
              height: 14,
              child: CircularProgressIndicator(strokeWidth: 1.5, color: color),
            )
          : Icon(icon, size: 17),
      color: color,
      disabledColor: color.withValues(alpha: 0.38),
      padding: EdgeInsets.zero,
      constraints: const BoxConstraints.tightFor(width: 30, height: 30),
      visualDensity: VisualDensity.compact,
      style: IconButton.styleFrom(
        backgroundColor: fillColor,
        disabledBackgroundColor: fillColor?.withValues(alpha: 0.32),
        hoverColor: fillColor == null
            ? color.withValues(alpha: 0.12)
            : Color.alphaBlend(color.withValues(alpha: 0.10), fillColor!),
        highlightColor: fillColor == null
            ? color.withValues(alpha: 0.16)
            : Color.alphaBlend(color.withValues(alpha: 0.16), fillColor!),
        shape: const CircleBorder(),
      ),
    );
  }
}

class _AiConversationHistoryView extends StatelessWidget {
  const _AiConversationHistoryView({
    required this.colors,
    required this.entries,
    required this.currentConversationId,
    required this.loading,
    required this.error,
    required this.query,
    required this.deletingIds,
    required this.onQueryChanged,
    required this.onOpen,
    required this.onDelete,
  });

  final _AiAssistantColors colors;
  final List<AiConversationEntry> entries;
  final String? currentConversationId;
  final bool loading;
  final String? error;
  final String query;
  final Set<String> deletingIds;
  final ValueChanged<String> onQueryChanged;
  final Future<void> Function(AiConversationEntry entry) onOpen;
  final Future<void> Function(AiConversationEntry entry) onDelete;

  @override
  Widget build(BuildContext context) {
    if (loading && entries.isEmpty) {
      return Center(
        child: SizedBox(
          width: 18,
          height: 18,
          child: CircularProgressIndicator(
            strokeWidth: 1.6,
            color: colors.accent,
          ),
        ),
      );
    }
    if (entries.isEmpty && error == null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(LucideIcons.history, size: 24, color: colors.muted),
            SizedBox(height: 10),
            Text(
              tr(
                'ai.label.noConversationHistory',
                fallback: 'No conversation history',
              ),
              style: TextStyle(
                color: colors.foreground,
                fontSize: 13,
                fontWeight: NautermFontWeights.medium,
                letterSpacing: 0,
              ),
            ),
          ],
        ),
      );
    }
    final normalizedQuery = query.trim().toLowerCase();
    final visibleEntries = normalizedQuery.isEmpty
        ? entries
        : entries
              .where((entry) {
                return entry.title.toLowerCase().contains(normalizedQuery) ||
                    (entry.preview ?? '').toLowerCase().contains(
                      normalizedQuery,
                    );
              })
              .toList(growable: false);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (loading)
          LinearProgressIndicator(
            minHeight: 1,
            color: colors.accent,
            backgroundColor: colors.border,
          ),
        if (error case final message?)
          Padding(
            padding: const EdgeInsets.fromLTRB(14, 12, 14, 4),
            child: Text(
              tr(message),
              style: TextStyle(
                color: colors.foreground,
                fontSize: 12,
                height: 1.35,
                letterSpacing: 0,
              ),
            ),
          ),
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
          child: SizedBox(
            height: 34,
            child: TextField(
              onChanged: onQueryChanged,
              style: TextStyle(
                color: colors.foreground,
                fontSize: 12,
                letterSpacing: 0,
              ),
              cursorColor: colors.accent,
              decoration: InputDecoration(
                hintText: tr(
                  'ai.label.searchConversations',
                  fallback: 'Search conversations',
                ),
                hintStyle: TextStyle(
                  color: colors.muted,
                  fontSize: 12,
                  letterSpacing: 0,
                ),
                prefixIcon: Icon(
                  LucideIcons.search,
                  size: 16,
                  color: colors.muted,
                ),
                prefixIconConstraints: const BoxConstraints.tightFor(
                  width: 32,
                  height: 32,
                ),
                filled: true,
                fillColor: colors.inputBackground,
                contentPadding: const EdgeInsets.only(right: 8),
                enabledBorder: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(8),
                  borderSide: BorderSide(color: colors.border),
                ),
                focusedBorder: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(8),
                  borderSide: BorderSide(color: colors.accent),
                ),
              ),
            ),
          ),
        ),
        Divider(height: 1, color: colors.border),
        Expanded(
          child: visibleEntries.isEmpty
              ? Center(
                  child: Text(
                    tr(
                      'ai.label.noMatchingConversations',
                      fallback: 'No matching conversations',
                    ),
                    style: TextStyle(
                      color: colors.muted,
                      fontSize: 12,
                      letterSpacing: 0,
                    ),
                  ),
                )
              : ListView.separated(
                  padding: const EdgeInsets.fromLTRB(8, 8, 8, 14),
                  itemCount: visibleEntries.length,
                  separatorBuilder: (_, _) => const SizedBox(height: 4),
                  itemBuilder: (context, index) {
                    final entry = visibleEntries[index];
                    final uuid = entry.uuid;
                    return _AiConversationHistoryRow(
                      colors: colors,
                      entry: entry,
                      active: uuid != null && uuid == currentConversationId,
                      deleting: uuid != null && deletingIds.contains(uuid),
                      enabled: !loading && deletingIds.isEmpty,
                      onOpen: () => unawaited(onOpen(entry)),
                      onDelete: () => unawaited(onDelete(entry)),
                    );
                  },
                ),
        ),
      ],
    );
  }
}

class _AiConversationHistoryRow extends StatelessWidget {
  const _AiConversationHistoryRow({
    required this.colors,
    required this.entry,
    required this.active,
    required this.deleting,
    required this.enabled,
    required this.onOpen,
    required this.onDelete,
  });

  final _AiAssistantColors colors;
  final AiConversationEntry entry;
  final bool active;
  final bool deleting;
  final bool enabled;
  final VoidCallback onOpen;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final time = entry.updatedAt == null
        ? ''
        : _compactTerminalDateTime(entry.updatedAt!);
    return Material(
      color: active
          ? Color.lerp(colors.background, colors.accent, 0.12)
          : colors.inputBackground.withValues(alpha: 0.52),
      borderRadius: BorderRadius.circular(8),
      child: InkWell(
        borderRadius: BorderRadius.circular(8),
        onTap: enabled ? onOpen : null,
        hoverColor: colors.inputBackground,
        splashColor: colors.accent.withValues(alpha: 0.14),
        highlightColor: colors.accent.withValues(alpha: 0.10),
        child: Padding(
          padding: const EdgeInsets.fromLTRB(10, 8, 5, 8),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Expanded(
                          child: Text(
                            entry.title,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: TextStyle(
                              color: colors.foreground,
                              fontSize: 12,
                              fontWeight: NautermFontWeights.medium,
                              height: 1.35,
                              letterSpacing: 0,
                            ),
                          ),
                        ),
                        if (active) ...[
                          SizedBox(width: 6),
                          Icon(
                            LucideIcons.check,
                            size: 14,
                            color: colors.accent,
                          ),
                        ],
                      ],
                    ),
                    if (time.isNotEmpty) ...[
                      const SizedBox(height: 5),
                      Text(
                        time,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: colors.muted,
                          fontSize: 10,
                          fontWeight: NautermFontWeights.medium,
                          letterSpacing: 0,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              SizedBox(width: 4),
              IconButton(
                tooltip: tr(
                  'ai.label.deleteConversation',
                  fallback: 'Delete conversation',
                ),
                onPressed: enabled && !deleting ? onDelete : null,
                icon: deleting
                    ? SizedBox(
                        width: 12,
                        height: 12,
                        child: CircularProgressIndicator(
                          strokeWidth: 1.3,
                          color: colors.muted,
                        ),
                      )
                    : Icon(LucideIcons.trash2, size: 14),
                color: colors.muted,
                disabledColor: colors.muted.withValues(alpha: 0.35),
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(
                  width: 26,
                  height: 26,
                ),
                visualDensity: VisualDensity.compact,
                style: IconButton.styleFrom(
                  hoverColor: colors.inputBackground,
                  highlightColor: colors.accent.withValues(alpha: 0.10),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _AiConversationMessage extends StatefulWidget {
  const _AiConversationMessage({
    super.key,
    required this.message,
    required this.sending,
    required this.colors,
    required this.editable,
    required this.onEdit,
  });

  final AiChatMessage message;
  final bool sending;
  final _AiAssistantColors colors;
  final bool editable;
  final ValueChanged<String> onEdit;

  @override
  State<_AiConversationMessage> createState() => _AiConversationMessageState();
}

class _AiConversationMessageState extends State<_AiConversationMessage> {
  late final TextEditingController _editController;
  late final FocusNode _editFocusNode;
  Timer? _copiedResetTimer;
  bool _hovered = false;
  bool _editing = false;
  bool _copied = false;

  @override
  void initState() {
    super.initState();
    _editController = TextEditingController(text: widget.message.content);
    _editFocusNode = FocusNode(onKeyEvent: _handleEditKeyEvent);
  }

  @override
  void didUpdateWidget(_AiConversationMessage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.message.sequence != widget.message.sequence) {
      _editing = false;
      _copied = false;
    }
    if (!_editing && oldWidget.message.content != widget.message.content) {
      _editController.text = widget.message.content;
    }
  }

  @override
  void dispose() {
    _copiedResetTimer?.cancel();
    _editController.dispose();
    _editFocusNode.dispose();
    super.dispose();
  }

  KeyEventResult _handleEditKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent || !_isEnterKey(event.logicalKey)) {
      return KeyEventResult.ignored;
    }
    final keyboard = HardwareKeyboard.instance;
    if (keyboard.isShiftPressed ||
        keyboard.isControlPressed ||
        keyboard.isAltPressed ||
        keyboard.isMetaPressed ||
        _hasActiveComposingRegion(_editController.value)) {
      return KeyEventResult.ignored;
    }
    _submitEdit();
    return KeyEventResult.handled;
  }

  void _startEditing() {
    if (!widget.editable) {
      return;
    }
    _editController
      ..text = widget.message.content
      ..selection = TextSelection.collapsed(
        offset: widget.message.content.length,
      );
    setState(() => _editing = true);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && _editing) {
        _editFocusNode.requestFocus();
      }
    });
  }

  void _cancelEdit() {
    _editController.text = widget.message.content;
    setState(() => _editing = false);
  }

  void _submitEdit() {
    final content = _editController.text.trim();
    if ((content.isEmpty && widget.message.attachments.isEmpty) ||
        !widget.editable) {
      return;
    }
    setState(() => _editing = false);
    if (content != widget.message.content) {
      widget.onEdit(content);
    }
  }

  Future<void> _copyMessage() async {
    if (widget.message.content.isEmpty) {
      return;
    }
    await Clipboard.setData(ClipboardData(text: widget.message.content));
    if (!mounted) {
      return;
    }
    _copiedResetTimer?.cancel();
    setState(() => _copied = true);
    _copiedResetTimer = Timer(const Duration(milliseconds: 1400), () {
      if (mounted) {
        setState(() => _copied = false);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final message = widget.message;
    final colors = widget.colors;
    final assistant = message.role == AiChatRole.assistant;
    final bubbleColor = Color.lerp(
      colors.background,
      colors.foreground,
      0.055,
    )!;
    final markdownStyle = MarkdownStyleSheet(
      p: TextStyle(
        color: colors.foreground,
        fontSize: 13,
        height: 1.45,
        letterSpacing: 0,
      ),
      a: TextStyle(
        color: colors.accent,
        fontSize: 13,
        decoration: TextDecoration.underline,
        decorationColor: colors.accent,
        letterSpacing: 0,
      ),
      code: TextStyle(
        color: colors.foreground,
        fontSize: 12,
        fontFamily: 'monospace',
        letterSpacing: 0,
      ),
      h1: _aiMarkdownHeading(colors.foreground, 18),
      h2: _aiMarkdownHeading(colors.foreground, 16),
      h3: _aiMarkdownHeading(colors.foreground, 14),
      h4: _aiMarkdownHeading(colors.foreground, 13),
      h5: _aiMarkdownHeading(colors.foreground, 13),
      h6: _aiMarkdownHeading(colors.foreground, 13),
      strong: TextStyle(fontWeight: FontWeight.w700),
      em: TextStyle(fontStyle: FontStyle.italic),
      blockSpacing: 9,
      listIndent: 20,
      listBullet: TextStyle(
        color: colors.foreground,
        fontSize: 13,
        letterSpacing: 0,
      ),
      blockquote: TextStyle(
        color: colors.muted,
        fontSize: 13,
        height: 1.4,
        letterSpacing: 0,
      ),
      blockquotePadding: const EdgeInsets.fromLTRB(10, 7, 8, 7),
      blockquoteDecoration: BoxDecoration(
        color: colors.background,
        border: Border(left: BorderSide(color: colors.accent, width: 2)),
      ),
      codeblockPadding: const EdgeInsets.all(10),
      codeblockDecoration: BoxDecoration(
        color: colors.inputBackground,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: colors.border),
      ),
      tableHead: TextStyle(
        color: colors.foreground,
        fontSize: 12,
        fontWeight: NautermFontWeights.semibold,
        letterSpacing: 0,
      ),
      tableBody: TextStyle(
        color: colors.foreground,
        fontSize: 12,
        letterSpacing: 0,
      ),
      tableBorder: TableBorder.all(color: colors.border),
      tableCellsPadding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
      horizontalRuleDecoration: BoxDecoration(
        border: Border(top: BorderSide(color: colors.border)),
      ),
    );
    final messageContent = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (message.attachments.isNotEmpty) ...[
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: [
              for (final attachment in message.attachments)
                _AiSentAttachmentChip(attachment: attachment, colors: colors),
            ],
          ),
          if (message.content.isNotEmpty) SizedBox(height: 8),
        ],
        if (_editing)
          _buildEditor()
        else if (assistant && message.content.isEmpty && widget.sending)
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              SizedBox(
                width: 12,
                height: 12,
                child: CircularProgressIndicator(
                  strokeWidth: 1.5,
                  color: colors.accent,
                ),
              ),
              SizedBox(width: 8),
              Text(
                tr('ai.label.thinking', fallback: 'Thinking'),
                style: TextStyle(
                  color: colors.muted,
                  fontSize: 12,
                  fontWeight: NautermFontWeights.medium,
                  letterSpacing: 0,
                ),
              ),
            ],
          )
        else if (assistant)
          // flutter_markdown_plus still uses Flutter's legacy Material widgets.
          // ignore: deprecated_member_use
          MaterialUiCompatibilityBridge(
            child: MarkdownBody(
              data: message.content,
              selectable: false,
              softLineBreak: true,
              styleSheet: markdownStyle,
              onTapLink: (text, href, title) => _openAiExternalLink(href),
              imageBuilder: (uri, title, alt) => Text(
                alt ?? 'Image',
                style: TextStyle(
                  color: colors.muted,
                  fontSize: 12,
                  letterSpacing: 0,
                ),
              ),
            ),
          )
        else if (message.content.isNotEmpty)
          Text(
            message.content,
            style: TextStyle(
              color: colors.foreground,
              fontSize: 13,
              height: 1.45,
              letterSpacing: 0,
            ),
          ),
      ],
    );
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final content = assistant
              ? Padding(
                  padding: const EdgeInsets.only(right: 4),
                  child: messageContent,
                )
              : Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 9,
                  ),
                  decoration: BoxDecoration(
                    color: bubbleColor,
                    borderRadius: BorderRadius.circular(16),
                  ),
                  child: messageContent,
                );
          return Align(
            alignment: assistant ? Alignment.centerLeft : Alignment.centerRight,
            child: ConstrainedBox(
              constraints: BoxConstraints(
                maxWidth: assistant
                    ? constraints.maxWidth
                    : constraints.maxWidth * 0.88,
              ),
              child: Column(
                crossAxisAlignment: assistant
                    ? CrossAxisAlignment.start
                    : CrossAxisAlignment.end,
                children: [content, _buildMessageActions(assistant)],
              ),
            ),
          );
        },
      ),
    );
  }

  Widget _buildEditor() {
    final colors = widget.colors;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        TextField(
          key: ValueKey('ai-message-editor-${widget.message.sequence}'),
          controller: _editController,
          focusNode: _editFocusNode,
          minLines: 1,
          maxLines: 8,
          textInputAction: TextInputAction.newline,
          style: TextStyle(
            color: colors.foreground,
            fontSize: 13,
            height: 1.45,
            letterSpacing: 0,
          ),
          cursorColor: colors.accent,
          decoration: InputDecoration(
            isDense: true,
            border: InputBorder.none,
            enabledBorder: InputBorder.none,
            focusedBorder: InputBorder.none,
            contentPadding: EdgeInsets.zero,
          ),
        ),
        SizedBox(height: 6),
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            _AiMessageActionButton(
              tooltip: tr('ai.label.cancelEditing', fallback: 'Cancel editing'),
              icon: LucideIcons.x,
              color: colors.muted,
              onPressed: _cancelEdit,
            ),
            SizedBox(width: 2),
            _AiMessageActionButton(
              tooltip: tr(
                'ai.label.saveAndResend',
                fallback: 'Save and resend',
              ),
              icon: LucideIcons.check,
              color: colors.accent,
              onPressed: _submitEdit,
            ),
          ],
        ),
      ],
    );
  }

  Widget _buildMessageActions(bool assistant) {
    final visible = (assistant || _hovered) && !_editing;
    final timestamp = _aiMessageTime(widget.message);
    return SizedBox(
      height: 24,
      child: IgnorePointer(
        ignoring: !visible,
        child: AnimatedOpacity(
          opacity: visible ? 1 : 0,
          duration: const Duration(milliseconds: 100),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: assistant
                ? MainAxisAlignment.start
                : MainAxisAlignment.end,
            children: [
              if (timestamp.isNotEmpty) ...[
                Text(
                  timestamp,
                  style: TextStyle(
                    color: widget.colors.muted,
                    fontSize: 10,
                    letterSpacing: 0,
                  ),
                ),
                SizedBox(width: 5),
              ],
              _AiMessageActionButton(
                tooltip: _copied ? 'Copied' : 'Copy message',
                icon: _copied ? LucideIcons.check : LucideIcons.copy,
                color: _copied ? widget.colors.accent : widget.colors.muted,
                onPressed: widget.message.content.isEmpty ? null : _copyMessage,
              ),
              if (!assistant)
                _AiMessageActionButton(
                  tooltip: tr('ai.label.editMessage', fallback: 'Edit message'),
                  icon: LucideIcons.pencil,
                  color: widget.colors.muted,
                  onPressed: widget.editable ? _startEditing : null,
                ),
            ],
          ),
        ),
      ),
    );
  }
}

void _openAiExternalLink(String? href) {
  final value = href?.trim();
  if (value == null || value.isEmpty) {
    return;
  }
  final uri = Uri.tryParse(value);
  if (uri == null ||
      !uri.hasScheme ||
      (uri.scheme != 'http' && uri.scheme != 'https') ||
      uri.host.isEmpty) {
    NautermLog.warning('ai', 'Ignored unsupported AI Assistant link.');
    return;
  }
  try {
    final result = UrlOpener.instance.open(uri.toString());
    if (!result.success) {
      NautermLog.warning('ai', 'Unable to open AI Assistant link.');
    }
  } on Object catch (error, stackTrace) {
    NautermLog.warning(
      'ai',
      'Unable to open AI Assistant link.',
      error: error,
      stackTrace: stackTrace,
    );
  }
}

class _AiMessageActionButton extends StatefulWidget {
  const _AiMessageActionButton({
    required this.tooltip,
    required this.icon,
    required this.color,
    required this.onPressed,
  });

  final String tooltip;
  final IconData icon;
  final Color color;
  final VoidCallback? onPressed;

  @override
  State<_AiMessageActionButton> createState() => _AiMessageActionButtonState();
}

class _AiMessageActionButtonState extends State<_AiMessageActionButton> {
  bool _hovered = false;
  bool _pressed = false;

  @override
  Widget build(BuildContext context) {
    final enabled = widget.onPressed != null;
    final iconColor = enabled
        ? widget.color.withValues(alpha: _hovered ? 1 : 0.78)
        : widget.color.withValues(alpha: 0.30);
    return Tooltip(
      message: tr(widget.tooltip),
      waitDuration: const Duration(milliseconds: 280),
      child: MouseRegion(
        cursor: enabled ? SystemMouseCursors.click : SystemMouseCursors.basic,
        onEnter: enabled ? (_) => setState(() => _hovered = true) : null,
        onExit: enabled
            ? (_) => setState(() {
                _hovered = false;
                _pressed = false;
              })
            : null,
        child: AnimatedScale(
          scale: _pressed ? 0.94 : 1,
          duration: const Duration(milliseconds: 90),
          curve: Curves.easeOutCubic,
          child: AnimatedContainer(
            duration: const Duration(milliseconds: 110),
            curve: Curves.easeOutCubic,
            width: 26,
            height: 24,
            decoration: BoxDecoration(
              color: Colors.transparent,
              borderRadius: BorderRadius.circular(8),
            ),
            child: Material(
              type: MaterialType.transparency,
              borderRadius: BorderRadius.circular(8),
              clipBehavior: Clip.antiAlias,
              child: InkWell(
                onTap: widget.onPressed,
                onHighlightChanged: enabled
                    ? (value) => setState(() => _pressed = value)
                    : null,
                hoverColor: Colors.transparent,
                highlightColor: Colors.transparent,
                splashColor: widget.color.withValues(alpha: 0.18),
                child: Center(
                  child: Icon(widget.icon, size: 14, color: iconColor),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

String _aiMessageTime(AiChatMessage message) {
  final value = message.createdAt;
  if (value == null) {
    return '';
  }
  final local = value.toLocal();
  final hour = local.hour.toString().padLeft(2, '0');
  final minute = local.minute.toString().padLeft(2, '0');
  return '$hour:$minute';
}

bool _isEnterKey(LogicalKeyboardKey key) {
  return key == LogicalKeyboardKey.enter ||
      key == LogicalKeyboardKey.numpadEnter;
}

bool _hasActiveComposingRegion(TextEditingValue value) {
  return value.composing.isValid && !value.composing.isCollapsed;
}

TextStyle _aiMarkdownHeading(Color color, double size) {
  return TextStyle(
    color: color,
    fontSize: size,
    height: 1.3,
    fontWeight: NautermFontWeights.semibold,
    letterSpacing: 0,
  );
}

class _AiPendingAttachmentChip extends StatelessWidget {
  const _AiPendingAttachmentChip({
    required this.attachment,
    required this.colors,
    required this.onRemove,
  });

  final AiAttachment attachment;
  final _AiAssistantColors colors;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    return ConstrainedBox(
      constraints: const BoxConstraints(maxWidth: 210),
      child: Container(
        key: ValueKey('ai-pending-attachment-${attachment.id}'),
        height: 29,
        padding: const EdgeInsets.only(left: 7, right: 2),
        decoration: BoxDecoration(
          color: colors.background,
          borderRadius: BorderRadius.circular(6),
          border: Border.all(color: colors.border),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              attachment.kind == AiAttachmentKind.image
                  ? LucideIcons.image
                  : LucideIcons.fileText,
              size: 13,
              color: colors.accent,
            ),
            SizedBox(width: 5),
            Flexible(
              child: Text(
                attachment.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: colors.foreground,
                  fontSize: 11,
                  letterSpacing: 0,
                ),
              ),
            ),
            IconButton(
              tooltip: tr('Remove ${attachment.name}'),
              onPressed: onRemove,
              icon: Icon(LucideIcons.x, size: 13),
              color: colors.muted,
              padding: EdgeInsets.zero,
              visualDensity: VisualDensity.compact,
              constraints: const BoxConstraints.tightFor(width: 24, height: 24),
              style: IconButton.styleFrom(
                hoverColor: colors.border,
                highlightColor: colors.border,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _AiSentAttachmentChip extends StatelessWidget {
  const _AiSentAttachmentChip({required this.attachment, required this.colors});

  final AiAttachment attachment;
  final _AiAssistantColors colors;

  @override
  Widget build(BuildContext context) {
    return Container(
      constraints: const BoxConstraints(maxWidth: 190),
      padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 5),
      decoration: BoxDecoration(
        color: colors.background,
        borderRadius: BorderRadius.circular(5),
        border: Border.all(color: colors.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            attachment.kind == AiAttachmentKind.image
                ? LucideIcons.image
                : LucideIcons.fileText,
            size: 13,
            color: colors.accent,
          ),
          SizedBox(width: 5),
          Flexible(
            child: Text(
              attachment.redacted
                  ? '${attachment.name} (redacted)'
                  : attachment.name,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                color: colors.foreground,
                fontSize: 10,
                letterSpacing: 0,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _AiContextChip extends StatelessWidget {
  const _AiContextChip({
    required this.attachment,
    required this.colors,
    required this.onRemove,
  });

  final AiContextAttachment attachment;
  final _AiAssistantColors colors;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    return Container(
      key: ValueKey('ai-context-${attachment.kind.name}'),
      height: 27,
      padding: const EdgeInsets.only(left: 8, right: 3),
      decoration: BoxDecoration(
        color: colors.background,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: colors.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            attachment.kind == AiContextKind.terminalSelection
                ? LucideIcons.textSelect
                : LucideIcons.squareTerminal,
            size: 13,
            color: colors.accent,
          ),
          SizedBox(width: 5),
          Text(
            attachment.redacted
                ? '${attachment.label} (redacted)'
                : attachment.label,
            style: TextStyle(
              color: colors.foreground,
              fontSize: 11,
              fontWeight: NautermFontWeights.medium,
              letterSpacing: 0,
            ),
          ),
          SizedBox(width: 2),
          IconButton(
            tooltip: tr('Remove ${attachment.label.toLowerCase()}'),
            onPressed: onRemove,
            icon: Icon(LucideIcons.x, size: 13),
            color: colors.muted,
            visualDensity: VisualDensity.compact,
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints.tightFor(width: 24, height: 24),
            style: IconButton.styleFrom(
              hoverColor: colors.border,
              highlightColor: colors.border,
            ),
          ),
        ],
      ),
    );
  }
}

class _AiTerminalCommandElapsed extends StatefulWidget {
  const _AiTerminalCommandElapsed({
    required this.startedAt,
    required this.color,
    this.finishedAt,
    this.prefix = '',
  });

  final DateTime startedAt;
  final DateTime? finishedAt;
  final Color color;
  final String prefix;

  @override
  State<_AiTerminalCommandElapsed> createState() =>
      _AiTerminalCommandElapsedState();
}

class _AiTerminalCommandElapsedState extends State<_AiTerminalCommandElapsed> {
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _syncTimer();
  }

  @override
  void didUpdateWidget(covariant _AiTerminalCommandElapsed oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.startedAt != widget.startedAt ||
        oldWidget.finishedAt != widget.finishedAt) {
      _syncTimer();
    }
  }

  void _syncTimer() {
    _timer?.cancel();
    _timer = widget.finishedAt == null
        ? Timer.periodic(const Duration(seconds: 1), (_) {
            if (mounted) {
              setState(() {});
            }
          })
        : null;
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final end = widget.finishedAt ?? DateTime.now();
    final elapsed = end.difference(widget.startedAt);
    return Text(
      tr('${widget.prefix}${_formatAiCommandDuration(elapsed)}'),
      style: TextStyle(
        color: widget.color,
        fontSize: 11,
        fontWeight: NautermFontWeights.medium,
        letterSpacing: 0,
      ),
    );
  }
}

String _formatAiCommandDuration(Duration duration) {
  final seconds = duration.isNegative ? 0 : duration.inSeconds;
  final hours = seconds ~/ 3600;
  final minutes = (seconds % 3600) ~/ 60;
  final remainingSeconds = seconds % 60;
  if (hours > 0) {
    return '$hours:${minutes.toString().padLeft(2, '0')}:'
        '${remainingSeconds.toString().padLeft(2, '0')}';
  }
  return '$minutes:${remainingSeconds.toString().padLeft(2, '0')}';
}

class _AiTerminalCommandCard extends StatelessWidget {
  const _AiTerminalCommandCard({
    required this.command,
    required this.colors,
    required this.terminalAvailable,
    required this.anotherCommandRunning,
    required this.onRun,
    required this.onSkip,
    required this.onStop,
  });

  final AiTerminalCommand command;
  final _AiAssistantColors colors;
  final bool terminalAvailable;
  final bool anotherCommandRunning;
  final VoidCallback onRun;
  final VoidCallback onSkip;
  final VoidCallback onStop;

  @override
  Widget build(BuildContext context) {
    final running = command.status == AiTerminalCommandStatus.running;
    final submitted = command.status == AiTerminalCommandStatus.submitted;
    final succeeded = command.status == AiTerminalCommandStatus.succeeded;
    final failed = command.status == AiTerminalCommandStatus.failed;
    final cancelled = command.status == AiTerminalCommandStatus.cancelled;
    final skipped = command.status == AiTerminalCommandStatus.skipped;
    final finished = submitted || succeeded || failed || cancelled || skipped;
    final statusColor = running || submitted || succeeded
        ? colors.accent
        : failed || cancelled || skipped
        ? colors.muted
        : colors.muted;
    final cardBorder = running || submitted || succeeded
        ? Color.lerp(colors.border, colors.accent, 0.32)!
        : colors.border;
    return Container(
      key: ValueKey('ai-terminal-command-${command.id}'),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Color.lerp(colors.background, colors.inputBackground, 0.72),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: cardBorder),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Container(
                width: 26,
                height: 26,
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  color: statusColor.withValues(alpha: 0.10),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Icon(
                  LucideIcons.squareTerminal,
                  size: 14,
                  color: statusColor,
                ),
              ),
              SizedBox(width: 8),
              Expanded(
                child: Text(
                  tr('common.label.command', fallback: 'Command'),
                  style: TextStyle(
                    color: colors.foreground,
                    fontSize: 11.5,
                    fontWeight: NautermFontWeights.semibold,
                    letterSpacing: 0,
                  ),
                ),
              ),
              _AiCopyCommandButton(
                command: command.command,
                color: colors.muted,
                accent: colors.accent,
              ),
              SizedBox(width: 4),
              if (running)
                SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(
                    strokeWidth: 1.6,
                    color: colors.accent,
                  ),
                )
              else if (succeeded)
                Icon(LucideIcons.check, size: 16, color: colors.accent)
              else if (submitted)
                Icon(LucideIcons.send, size: 16, color: colors.accent)
              else if (failed)
                Icon(LucideIcons.circleAlert, size: 16, color: colors.muted)
              else if (cancelled)
                Icon(LucideIcons.squareStop, size: 16, color: colors.muted)
              else if (skipped)
                Icon(LucideIcons.ban, size: 16, color: colors.muted),
            ],
          ),
          SizedBox(height: 10),
          Container(
            width: double.infinity,
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 9),
            decoration: BoxDecoration(
              color: Color.lerp(
                colors.inputBackground,
                colors.background,
                0.55,
              ),
              borderRadius: BorderRadius.circular(9),
              border: Border.all(color: colors.border.withValues(alpha: 0.72)),
            ),
            child: Text(
              command.command,
              style: TextStyle(
                color: colors.foreground,
                fontSize: 12,
                height: 1.4,
                fontFamily: 'monospace',
                letterSpacing: 0,
              ),
            ),
          ),
          if (command.output case final output? when output.isNotEmpty) ...[
            SizedBox(height: 10),
            Container(
              width: double.infinity,
              padding: const EdgeInsets.all(8),
              decoration: BoxDecoration(
                color: colors.background,
                borderRadius: BorderRadius.circular(9),
              ),
              child: Text(
                output,
                maxLines: 10,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: colors.foreground,
                  fontSize: 11,
                  height: 1.4,
                  fontFamily: 'monospace',
                  letterSpacing: 0,
                ),
              ),
            ),
          ],
          if (command.error case final error? when error.isNotEmpty) ...[
            SizedBox(height: 9),
            Text(
              error,
              style: TextStyle(
                color: colors.muted,
                fontSize: 11,
                height: 1.35,
                letterSpacing: 0,
              ),
            ),
          ],
          SizedBox(height: 10),
          Align(
            alignment: Alignment.centerRight,
            child: finished
                ? Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        skipped
                            ? 'Skipped'
                            : submitted
                            ? 'Submitted · result not tracked'
                            : cancelled
                            ? 'Stopped'
                            : command.exitCode == null
                            ? 'Failed'
                            : 'Exited ${command.exitCode}',
                        style: TextStyle(
                          color: succeeded || submitted
                              ? colors.accent
                              : colors.muted,
                          fontSize: 11,
                          fontWeight: NautermFontWeights.medium,
                          letterSpacing: 0,
                        ),
                      ),
                      if (command.startedAt != null) ...[
                        Text(tr(' · '), style: TextStyle(color: colors.muted)),
                        _AiTerminalCommandElapsed(
                          startedAt: command.startedAt!,
                          finishedAt: command.finishedAt,
                          color: colors.muted,
                        ),
                      ],
                    ],
                  )
                : running
                ? Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (command.startedAt case final startedAt?)
                        _AiTerminalCommandElapsed(
                          startedAt: startedAt,
                          color: colors.muted,
                          prefix: command.cancellationRequested
                              ? 'Stopping '
                              : 'Running ',
                        ),
                      SizedBox(width: 8),
                      _AiCommandActionButton(
                        key: ValueKey('stop-ai-terminal-command-${command.id}'),
                        onPressed: command.cancellationRequested
                            ? null
                            : onStop,
                        icon: Icons.stop_rounded,
                        label: command.cancellationRequested
                            ? 'Stopping'
                            : 'Stop',
                        colors: colors,
                      ),
                    ],
                  )
                : Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      _AiCommandActionButton(
                        key: ValueKey('skip-ai-terminal-command-${command.id}'),
                        onPressed: onSkip,
                        label: 'Skip',
                        colors: colors,
                      ),
                      SizedBox(width: 6),
                      _AiCommandActionButton(
                        key: ValueKey('run-ai-terminal-command-${command.id}'),
                        onPressed: terminalAvailable && !anotherCommandRunning
                            ? onRun
                            : null,
                        icon: LucideIcons.play,
                        label: !terminalAvailable
                            ? 'No active terminal'
                            : anotherCommandRunning
                            ? 'Command running'
                            : 'Run',
                        colors: colors,
                        primary: true,
                      ),
                    ],
                  ),
          ),
        ],
      ),
    );
  }
}

class _AiCommandActionButton extends StatefulWidget {
  const _AiCommandActionButton({
    super.key,
    required this.label,
    required this.colors,
    required this.onPressed,
    this.icon,
    this.primary = false,
  });

  final String label;
  final IconData? icon;
  final _AiAssistantColors colors;
  final VoidCallback? onPressed;
  final bool primary;

  @override
  State<_AiCommandActionButton> createState() => _AiCommandActionButtonState();
}

class _AiCommandActionButtonState extends State<_AiCommandActionButton> {
  bool _hovered = false;
  bool _pressed = false;

  @override
  Widget build(BuildContext context) {
    final colors = widget.colors;
    final enabled = widget.onPressed != null;
    final foreground = enabled
        ? widget.primary
              ? colors.accent
              : colors.muted
        : colors.muted.withValues(alpha: 0.42);
    final secondaryIdle = Color.lerp(
      colors.background,
      colors.inputBackground,
      0.55,
    )!;
    final background = !enabled
        ? secondaryIdle.withValues(alpha: 0.46)
        : widget.primary
        ? colors.accent.withValues(
            alpha: _pressed
                ? 0.24
                : _hovered
                ? 0.18
                : 0.12,
          )
        : _pressed
        ? colors.inputBackground
        : _hovered
        ? Color.lerp(colors.background, colors.inputBackground, 0.85)!
        : secondaryIdle;

    return MouseRegion(
      cursor: enabled ? SystemMouseCursors.click : SystemMouseCursors.basic,
      onEnter: enabled ? (_) => setState(() => _hovered = true) : null,
      onExit: enabled
          ? (_) => setState(() {
              _hovered = false;
              _pressed = false;
            })
          : null,
      child: AnimatedScale(
        scale: _pressed ? 0.97 : 1,
        duration: const Duration(milliseconds: 90),
        curve: Curves.easeOutCubic,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 110),
          curve: Curves.easeOutCubic,
          height: 28,
          decoration: BoxDecoration(
            color: background,
            borderRadius: BorderRadius.circular(9),
            border: Border.all(
              color: widget.primary
                  ? colors.accent.withValues(alpha: enabled ? 0.22 : 0.08)
                  : colors.border.withValues(alpha: enabled ? 0.76 : 0.34),
            ),
          ),
          child: Material(
            type: MaterialType.transparency,
            borderRadius: BorderRadius.circular(9),
            clipBehavior: Clip.antiAlias,
            child: InkWell(
              onTap: widget.onPressed,
              onHighlightChanged: enabled
                  ? (value) => setState(() => _pressed = value)
                  : null,
              hoverColor: Colors.transparent,
              highlightColor: Colors.transparent,
              splashColor: (widget.primary ? colors.accent : colors.muted)
                  .withValues(alpha: 0.18),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 10),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (widget.icon != null) ...[
                      Icon(widget.icon, size: 14, color: foreground),
                      const SizedBox(width: 4),
                    ],
                    Text(
                      widget.label,
                      style: TextStyle(
                        color: foreground,
                        fontSize: 11,
                        fontWeight: NautermFontWeights.semibold,
                        height: 1,
                        letterSpacing: 0,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _AiCopyCommandButton extends StatefulWidget {
  const _AiCopyCommandButton({
    required this.command,
    required this.color,
    required this.accent,
  });

  final String command;
  final Color color;
  final Color accent;

  @override
  State<_AiCopyCommandButton> createState() => _AiCopyCommandButtonState();
}

class _AiCopyCommandButtonState extends State<_AiCopyCommandButton> {
  Timer? _resetTimer;
  bool _copied = false;

  @override
  void dispose() {
    _resetTimer?.cancel();
    super.dispose();
  }

  Future<void> _copy() async {
    await Clipboard.setData(ClipboardData(text: widget.command));
    if (!mounted) {
      return;
    }
    _resetTimer?.cancel();
    setState(() => _copied = true);
    _resetTimer = Timer(const Duration(milliseconds: 1400), () {
      if (mounted) {
        setState(() => _copied = false);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return _AiMessageActionButton(
      tooltip: _copied ? 'Copied' : 'Copy command',
      onPressed: _copy,
      icon: _copied ? LucideIcons.check : LucideIcons.copy,
      color: _copied ? widget.accent : widget.color,
    );
  }
}
