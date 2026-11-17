part of 'nauterm_workspace.dart';

/// Only form values survive closing an editor. Controllers, widgets, and active
/// operations belong to the mounted editor and are released when it closes.
class _WorkspaceEditorDraft extends ChangeNotifier {
  Map<String, Object?>? values;
  final Map<Object, _WorkspaceEditorDraft> children = {};
  Map<String, Object?>? Function()? capture;
  bool Function()? readHasChanges;
  bool _hasChanges = false;
  bool _discarded = false;

  bool get hasChanges => _hasChanges;

  void refreshChanges({bool notify = true}) {
    if (_discarded || readHasChanges == null) return;
    final changed = readHasChanges!();
    if (changed == _hasChanges) return;
    _hasChanges = changed;
    if (notify) notifyListeners();
  }

  void checkpoint() {
    if (!_discarded && capture != null) {
      values = capture!();
      refreshChanges(notify: false);
    }
  }

  void discard() {
    _discarded = true;
    values = null;
    capture = null;
    readHasChanges = null;
    for (final child in children.values) {
      child.discard();
    }
    children.clear();
    if (_hasChanges) {
      _hasChanges = false;
      notifyListeners();
    }
  }
}

class _WorkspaceDraftField {
  const _WorkspaceDraftField(
    this.read,
    this.write, {
    this.changes,
    this.affectsContent = true,
  });

  factory _WorkspaceDraftField.text(TextEditingController controller) =>
      _WorkspaceDraftField(
        () => controller.value,
        (value) => controller.value = value as TextEditingValue,
        changes: controller,
      );

  final Object? Function() read;
  final ValueChanged<Object?> write;
  final Listenable? changes;
  final bool affectsContent;
}

_WorkspaceDraftField _draftValue<T>(T Function() read, ValueChanged<T> write) =>
    _WorkspaceDraftField(read, (value) => write(value as T));

_WorkspaceDraftField _draftViewValue<T>(
  T Function() read,
  ValueChanged<T> write,
) => _WorkspaceDraftField(
  read,
  (value) => write(value as T),
  affectsContent: false,
);

class _WorkspaceEditorDraftScope extends InheritedWidget {
  const _WorkspaceEditorDraftScope({
    super.key,
    required this.draft,
    required this.onDiscard,
    required super.child,
  });

  final _WorkspaceEditorDraft draft;
  final VoidCallback onDiscard;

  static _WorkspaceEditorDraftScope? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<_WorkspaceEditorDraftScope>();

  @override
  bool updateShouldNotify(_WorkspaceEditorDraftScope oldWidget) =>
      !identical(draft, oldWidget.draft);
}

mixin _WorkspaceEditorDraftState<T extends StatefulWidget> on State<T> {
  Map<String, _WorkspaceDraftField> get draftFields;
  Iterable<Listenable> get draftChanges => const [];

  _WorkspaceEditorDraft? _editorDraft;
  Map<String, _WorkspaceDraftField>? _fields;
  final Set<Listenable> _changes = {};
  Map<String, Object?>? Function()? _captureDraft;
  bool restoringEditorDraft = false;

  void didRestoreEditorDraft() {}

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final draft = _WorkspaceEditorDraftScope.maybeOf(context)?.draft;
    if (draft == null || identical(draft, _editorDraft)) return;
    _editorDraft = draft;
    final fields = draftFields;
    _fields = fields;
    final defaults = {
      for (final entry in fields.entries) entry.key: entry.value.read(),
    };
    final values = draft.values;
    if (values != null) {
      restoringEditorDraft = true;
      try {
        for (final entry in fields.entries) {
          if (values.containsKey(entry.key)) {
            entry.value.write(values[entry.key]);
          }
        }
      } finally {
        restoringEditorDraft = false;
      }
    }
    _captureDraft = () {
      final changed = <String, Object?>{};
      for (final entry in fields.entries) {
        final value = entry.value.read();
        if (!_editorDraftValuesEqual(value, defaults[entry.key])) {
          changed[entry.key] = value;
        }
      }
      return changed.isEmpty ? null : changed;
    };
    if (values != null) didRestoreEditorDraft();
    draft._discarded = false;
    draft.capture = _captureDraft;
    draft.readHasChanges = () => fields.entries.any(
      (entry) =>
          entry.value.affectsContent &&
          !_editorDraftValuesEqual(entry.value.read(), defaults[entry.key]),
    );
    draft.refreshChanges(notify: false);
    _syncDraftChanges();
  }

  void _syncDraftChanges() {
    final sources = {
      for (final field in _fields?.values ?? <_WorkspaceDraftField>[])
        ?field.changes,
      ...draftChanges,
    };
    for (final source in _changes.difference(sources)) {
      source.removeListener(_refreshDraftChanges);
    }
    for (final source in sources.difference(_changes)) {
      source.addListener(_refreshDraftChanges);
    }
    _changes
      ..clear()
      ..addAll(sources);
  }

  void _refreshDraftChanges() {
    if (!restoringEditorDraft) _editorDraft?.refreshChanges();
  }

  @override
  void setState(VoidCallback fn) {
    super.setState(fn);
    if (_editorDraft != null && !restoringEditorDraft) {
      _syncDraftChanges();
      _refreshDraftChanges();
    }
  }

  @override
  void deactivate() {
    if (identical(_editorDraft?.capture, _captureDraft)) {
      _editorDraft?.checkpoint();
    }
    super.deactivate();
  }

  @override
  void dispose() {
    for (final source in _changes) {
      source.removeListener(_refreshDraftChanges);
    }
    _changes.clear();
    if (identical(_editorDraft?.capture, _captureDraft)) {
      _editorDraft?.capture = null;
      _editorDraft?.readHasChanges = null;
    }
    super.dispose();
  }
}

bool _editorDraftValuesEqual(Object? a, Object? b) {
  if (a is TextEditingValue && b is TextEditingValue) return a.text == b.text;
  if (a is Set && b is Set) return setEquals(a, b);
  if (a is HostEnvironmentVariable && b is HostEnvironmentVariable) {
    return a.variable == b.variable && a.value == b.value;
  }
  if (a is ({TextEditingValue variable, TextEditingValue value}) &&
      b is ({TextEditingValue variable, TextEditingValue value})) {
    return a.variable.text == b.variable.text && a.value.text == b.value.text;
  }
  if (a is List && b is List) {
    return a.length == b.length &&
        Iterable<int>.generate(a.length)
            .every((index) => _editorDraftValuesEqual(a[index], b[index]));
  }
  return a == b;
}
