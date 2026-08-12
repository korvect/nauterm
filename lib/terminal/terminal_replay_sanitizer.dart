import 'dart:typed_data';

const int _escape = 0x1b;
const int _osc = 0x5d;
const int _bell = 0x07;
const int _stringTerminator = 0x5c;
const int _semicolon = 0x3b;
const int _maxPendingOscBytes = 64 * 1024;
const Set<String> _internalOscCodes = {'7', '133', '4545', '777'};

const List<int> _zshPromptEolMarker = [
  0x1b,
  0x5b,
  0x37,
  0x6d,
  0x25,
  0x1b,
  0x5b,
  0x32,
  0x37,
  0x6d,
];
const List<int> _restorePrimaryScreen = [
  0x1b,
  0x5b,
  0x3f,
  0x31,
  0x30,
  0x34,
  0x39,
  0x6c,
];
const List<List<int>> _alternateScreenEnterSequences = [
  [_escape, 0x5b, 0x3f, 0x34, 0x37, 0x68],
  [_escape, 0x5b, 0x3f, 0x31, 0x30, 0x34, 0x37, 0x68],
  [_escape, 0x5b, 0x3f, 0x31, 0x30, 0x34, 0x39, 0x68],
];
const List<List<int>> _alternateScreenExitSequences = [
  [_escape, 0x5b, 0x3f, 0x34, 0x37, 0x6c],
  [_escape, 0x5b, 0x3f, 0x31, 0x30, 0x34, 0x37, 0x6c],
  [_escape, 0x5b, 0x3f, 0x31, 0x30, 0x34, 0x39, 0x6c],
];

class TerminalReplaySanitizer {
  final List<int> _markerPending = [];
  final List<int> _oscPending = [];
  final List<int> _alternateScreenPending = [];
  bool _alternateScreen = false;

  Uint8List add(Uint8List chunk) {
    if (chunk.isEmpty) {
      return Uint8List(0);
    }

    return _stripPromptMarkers(_stripInternalOsc(_stripAlternateScreen(chunk)));
  }

  Uint8List _stripAlternateScreen(Uint8List chunk) {
    final input = <int>[..._alternateScreenPending, ...chunk];
    _alternateScreenPending.clear();
    final output = BytesBuilder(copy: false);
    var index = 0;

    while (index < input.length) {
      final sequences = _alternateScreen
          ? _alternateScreenExitSequences
          : _alternateScreenEnterSequences;
      final match = _firstSequenceMatch(input, index, sequences);
      if (match != null) {
        if (!_alternateScreen && match.index > index) {
          output.add(input.sublist(index, match.index));
        }
        _alternateScreen = !_alternateScreen;
        index = match.index + match.length;
        continue;
      }

      final pendingLength = _sequencePrefixSuffixLength(
        input,
        index,
        sequences,
      );
      final end = input.length - pendingLength;
      if (!_alternateScreen && end > index) {
        output.add(input.sublist(index, end));
      }
      if (pendingLength > 0) {
        _alternateScreenPending.addAll(input.sublist(end));
      }
      break;
    }

    return output.takeBytes();
  }

  Uint8List _stripPromptMarkers(Uint8List chunk) {
    if (chunk.isEmpty) {
      return Uint8List(0);
    }

    final input = <int>[..._markerPending, ...chunk];
    _markerPending.clear();
    final output = BytesBuilder(copy: false);
    var index = 0;

    while (index < input.length) {
      if (_matchesMarkerAt(input, index)) {
        index += _zshPromptEolMarker.length;
        continue;
      }

      final remaining = input.length - index;
      if (remaining < _zshPromptEolMarker.length &&
          _matchesMarkerPrefix(input, index)) {
        _markerPending.addAll(input.sublist(index));
        break;
      }

      output.addByte(input[index]);
      index++;
    }

    return output.takeBytes();
  }

  Uint8List close({bool restorePrimaryScreen = false}) {
    final trailingAlternateScreen = _alternateScreen
        ? Uint8List(0)
        : Uint8List.fromList(_alternateScreenPending);
    _alternateScreenPending.clear();
    _alternateScreen = false;
    final sanitizedTrailingAlternateScreen = _stripInternalOsc(
      trailingAlternateScreen,
    );
    final trailingOsc = Uint8List.fromList([
      ...sanitizedTrailingAlternateScreen,
      ..._oscPending,
    ]);
    _oscPending.clear();
    final sanitizedTrailingOsc = _stripPromptMarkers(trailingOsc);
    final remaining = Uint8List.fromList([
      ...sanitizedTrailingOsc,
      ..._markerPending,
      if (restorePrimaryScreen) ..._restorePrimaryScreen,
    ]);
    _markerPending.clear();
    return remaining;
  }

  Uint8List _stripInternalOsc(List<int> bytes) {
    final input = <int>[..._oscPending, ...bytes];
    _oscPending.clear();
    final output = BytesBuilder(copy: false);
    var index = 0;

    while (index < input.length) {
      final start = _indexOfOsc(input, index);
      if (start < 0) {
        final retainEscape = input.last == _escape;
        final end = retainEscape ? input.length - 1 : input.length;
        if (end > index) output.add(input.sublist(index, end));
        if (retainEscape) _oscPending.add(_escape);
        break;
      }
      if (start > index) output.add(input.sublist(index, start));

      final terminator = _oscTerminator(input, start + 2);
      if (terminator == null) {
        _oscPending.addAll(input.sublist(start));
        if (_oscPending.length > _maxPendingOscBytes) {
          output.add(_oscPending);
          _oscPending.clear();
        }
        break;
      }

      final payloadEnd = terminator.$1;
      final sequenceEnd = payloadEnd + terminator.$2;
      final separator = input.indexOf(_semicolon, start + 2);
      final codeEnd = separator >= 0 && separator < payloadEnd
          ? separator
          : payloadEnd;
      final code = String.fromCharCodes(input.sublist(start + 2, codeEnd));
      if (!_internalOscCodes.contains(code)) {
        output.add(input.sublist(start, sequenceEnd));
      }
      index = sequenceEnd;
    }

    return output.takeBytes();
  }

  bool _matchesMarkerAt(List<int> input, int start) {
    if (input.length - start < _zshPromptEolMarker.length) {
      return false;
    }
    for (var index = 0; index < _zshPromptEolMarker.length; index++) {
      if (input[start + index] != _zshPromptEolMarker[index]) {
        return false;
      }
    }
    return true;
  }

  bool _matchesMarkerPrefix(List<int> input, int start) {
    final length = input.length - start;
    for (var index = 0; index < length; index++) {
      if (input[start + index] != _zshPromptEolMarker[index]) {
        return false;
      }
    }
    return true;
  }
}

class _SequenceMatch {
  const _SequenceMatch(this.index, this.length);

  final int index;
  final int length;
}

_SequenceMatch? _firstSequenceMatch(
  List<int> input,
  int start,
  List<List<int>> sequences,
) {
  for (var index = start; index < input.length; index++) {
    for (final sequence in sequences) {
      if (_matchesSequenceAt(input, index, sequence)) {
        return _SequenceMatch(index, sequence.length);
      }
    }
  }
  return null;
}

bool _matchesSequenceAt(List<int> input, int start, List<int> sequence) {
  if (input.length - start < sequence.length) return false;
  for (var index = 0; index < sequence.length; index++) {
    if (input[start + index] != sequence[index]) return false;
  }
  return true;
}

int _sequencePrefixSuffixLength(
  List<int> input,
  int start,
  List<List<int>> sequences,
) {
  final available = input.length - start;
  final maxLength = sequences.fold<int>(
    0,
    (length, sequence) => sequence.length > length ? sequence.length : length,
  );
  for (
    var length = available < maxLength ? available : maxLength - 1;
    length > 0;
    length--
  ) {
    final suffixStart = input.length - length;
    if (suffixStart < start) continue;
    for (final sequence in sequences) {
      if (length >= sequence.length) continue;
      var matches = true;
      for (var index = 0; index < length; index++) {
        if (input[suffixStart + index] != sequence[index]) {
          matches = false;
          break;
        }
      }
      if (matches) return length;
    }
  }
  return 0;
}

int _indexOfOsc(List<int> input, int start) {
  for (var index = start; index + 1 < input.length; index++) {
    if (input[index] == _escape && input[index + 1] == _osc) return index;
  }
  return -1;
}

(int, int)? _oscTerminator(List<int> input, int start) {
  for (var index = start; index < input.length; index++) {
    if (input[index] == _bell) return (index, 1);
    if (input[index] == _escape &&
        index + 1 < input.length &&
        input[index + 1] == _stringTerminator) {
      return (index, 2);
    }
  }
  return null;
}
