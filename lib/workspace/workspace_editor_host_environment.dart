part of 'nauterm_workspace.dart';

class _HostEnvironmentNavigationField extends StatelessWidget {
  const _HostEnvironmentNavigationField({
    required this.variables,
    required this.onPressed,
  });

  final List<HostEnvironmentVariable> variables;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final size =
        _WorkspaceControlSizeScope.maybeOf(context) ??
        _WorkspaceControlSize.medium;
    final summary = variables
        .where((entry) => entry.variable.trim().isNotEmpty)
        .map((entry) => '${entry.variable.trim()}=${entry.value}')
        .join(', ');
    final hasVariables = summary.isNotEmpty;

    return KeyedSubtree(
      key: const ValueKey('host-environment-field'),
      child: _WorkspaceFieldFrame(
        label: 'Environment variables',
        size: size,
        focused: false,
        floatingLabel: true,
        hasContent: hasVariables,
        height: size.inputHeight,
        onTap: onPressed,
        child: LayoutBuilder(
          builder: (context, constraints) {
            final showSuffix =
                constraints.maxWidth >=
                _workspaceFieldAffixWidth + _workspaceSelectAffixGap;
            return Row(
              children: [
                Expanded(
                  child: Text(
                    summary,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: _text,
                      fontSize: size.inputFontSize,
                      fontWeight: NautermFontWeights.regular,
                      height: size.inputLineHeight,
                      letterSpacing: 0,
                    ),
                  ),
                ),
                if (showSuffix) ...[
                  SizedBox(width: _workspaceSelectAffixGap),
                  _WorkspaceSelectSuffixButton(
                    icon: Icons.chevron_right_rounded,
                    width: _workspaceFieldAffixWidth,
                    onPressed: onPressed,
                  ),
                ],
              ],
            );
          },
        ),
      ),
    );
  }
}

class _HostEnvironmentEditorContent extends StatefulWidget {
  const _HostEnvironmentEditorContent({
    required this.request,
    required this.onClose,
    required this.onSave,
  });

  final _HostEnvironmentEditorRequest request;
  final VoidCallback onClose;
  final _SaveHostEnvironment onSave;

  @override
  State<_HostEnvironmentEditorContent> createState() =>
      _HostEnvironmentEditorContentState();
}

class _HostEnvironmentEditorContentState
    extends State<_HostEnvironmentEditorContent> {
  late final List<_HostEnvironmentVariableDraft> _drafts;
  _HostEnvironmentVariableDraft? _autofocusDraft;
  String? _error;

  @override
  void initState() {
    super.initState();
    _drafts = widget.request.variables
        .map(_HostEnvironmentVariableDraft.fromEntry)
        .toList();
  }

  @override
  void dispose() {
    for (final draft in _drafts) {
      draft.dispose();
    }
    super.dispose();
  }

  void _addVariable() {
    setState(() {
      _error = null;
      final draft = _HostEnvironmentVariableDraft();
      _drafts.add(draft);
      _autofocusDraft = draft;
    });
  }

  void _removeVariable(int index) {
    if (index < 0 || index >= _drafts.length) {
      return;
    }
    setState(() {
      _error = null;
      final draft = _drafts.removeAt(index);
      if (identical(_autofocusDraft, draft)) {
        _autofocusDraft = null;
      }
      draft.dispose();
    });
  }

  void _save() {
    final variables = <HostEnvironmentVariable>[];
    final names = <String>{};
    for (final draft in _drafts) {
      final variable = draft.variableController.text.trim();
      if (variable.isEmpty) {
        continue;
      }
      if (!names.add(variable)) {
        setState(() => _error = 'Environment variable names must be unique.');
        return;
      }
      variables.add(
        HostEnvironmentVariable(
          variable: variable,
          value: draft.valueController.text,
        ),
      );
    }
    widget.onSave(variables);
  }

  @override
  Widget build(BuildContext context) {
    return KeyedSubtree(
      key: const ValueKey('host-environment-editor'),
      child: _EditorShell(
        title: 'Environment Variables',
        onClose: widget.onClose,
        onSave: _save,
        error: _error,
        children: [
          _WorkspaceFormSection(
            title: 'Environment variables',
            children: [
              Text(
                tr(
                  'Set environment variables for ${widget.request.hostLabel}.',
                ),
                style: TextStyle(
                  color: _text,
                  fontSize: NautermFontSizes.labelLarge,
                  fontWeight: NautermFontWeights.medium,
                  height: 1.35,
                  letterSpacing: 0,
                ),
              ),
              SizedBox(height: 5),
              Text(
                'Some SSH servers by default only allow variables with prefix '
                'LC_ and LANG_.',
                style: TextStyle(
                  color: _mutedText,
                  fontSize: NautermFontSizes.labelMedium,
                  fontWeight: NautermFontWeights.regular,
                  height: 1.4,
                  letterSpacing: 0,
                ),
              ),
              SizedBox(height: 14),
              _WorkspaceButton(
                icon: Icons.add_rounded,
                label: 'Add variable',
                variant: _WorkspaceButtonVariant.filled,
                size: _WorkspaceControlSize.large,
                fullWidth: true,
                onPressed: _addVariable,
              ),
            ],
          ),
          for (var index = 0; index < _drafts.length; index++) ...[
            SizedBox(height: 14),
            _HostEnvironmentVariableCard(
              key: ObjectKey(_drafts[index]),
              draft: _drafts[index],
              autofocus: identical(_drafts[index], _autofocusDraft),
              onRemove: () => _removeVariable(index),
            ),
          ],
        ],
      ),
    );
  }
}

class _HostEnvironmentVariableDraft {
  _HostEnvironmentVariableDraft({String variable = '', String value = ''})
    : variableController = TextEditingController(text: variable),
      valueController = TextEditingController(text: value);

  factory _HostEnvironmentVariableDraft.fromEntry(
    HostEnvironmentVariable entry,
  ) {
    return _HostEnvironmentVariableDraft(
      variable: entry.variable,
      value: entry.value,
    );
  }

  final TextEditingController variableController;
  final TextEditingController valueController;

  void dispose() {
    variableController.dispose();
    valueController.dispose();
  }
}

class _HostEnvironmentVariableCard extends StatefulWidget {
  const _HostEnvironmentVariableCard({
    super.key,
    required this.draft,
    required this.autofocus,
    required this.onRemove,
  });

  final _HostEnvironmentVariableDraft draft;
  final bool autofocus;
  final VoidCallback onRemove;

  @override
  State<_HostEnvironmentVariableCard> createState() =>
      _HostEnvironmentVariableCardState();
}

class _HostEnvironmentVariableCardState
    extends State<_HostEnvironmentVariableCard> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: _card,
          borderRadius: BorderRadius.circular(10),
        ),
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Text(
                      tr('common.label.variable', fallback: 'Variable'),
                      style: TextStyle(
                        color: _text,
                        fontSize: NautermFontSizes.labelLarge,
                        fontWeight: NautermFontWeights.semibold,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                  ExcludeSemantics(
                    excluding: !_hovered,
                    child: IgnorePointer(
                      ignoring: !_hovered,
                      child: AnimatedOpacity(
                        opacity: _hovered ? 1 : 0,
                        duration: const Duration(milliseconds: 120),
                        curve: Curves.easeOutCubic,
                        child: _WorkspaceButton(
                          icon: Icons.close_rounded,
                          tooltip: tr(
                            'workspace.label.removeVariable',
                            fallback: 'Remove variable',
                          ),
                          variant: _WorkspaceButtonVariant.text,
                          size: _WorkspaceControlSize.tiny,
                          onPressed: widget.onRemove,
                        ),
                      ),
                    ),
                  ),
                ],
              ),
              SizedBox(height: 12),
              _WorkspaceInput(
                controller: widget.draft.variableController,
                label: 'Variable',
                autofocus: widget.autofocus,
              ),
              SizedBox(height: _workspaceFormFieldGap),
              _WorkspaceInput(
                controller: widget.draft.valueController,
                label: 'Value',
              ),
            ],
          ),
        ),
      ),
    );
  }
}
