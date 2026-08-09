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

class TerminalReplaySanitizer {
  final List<int> _markerPending = [];
  final List<int> _oscPending = [];

  Uint8List add(Uint8List chunk) {
    if (chunk.isEmpty) {
      return Uint8List(0);
    }

    return _stripPromptMarkers(_stripInternalOsc(chunk));
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
    final trailingOsc = Uint8List.fromList(_oscPending);
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
