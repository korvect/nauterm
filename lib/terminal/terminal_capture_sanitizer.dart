import 'dart:typed_data';

/// Gates hidden shell setup output before terminal bytes are captured.
///
/// Raw output listeners still receive the original stream so hidden shell
/// setup can complete its marker handshake. Shell integration control
/// sequences are intentionally preserved in the encrypted capture and are
/// filtered only when the capture is replayed.
class TerminalCaptureSanitizer {
  final List<int> _suppressionPending = [];
  List<int>? _suppressionMarker;

  void suppressUntil(Uint8List marker) {
    _suppressionPending.clear();
    _suppressionMarker = marker.isEmpty ? null : marker.toList(growable: false);
  }

  void cancelSuppression() {
    _suppressionPending.clear();
    _suppressionMarker = null;
  }

  Uint8List add(Uint8List chunk) {
    if (chunk.isEmpty) return Uint8List(0);

    List<int> input = chunk;
    final marker = _suppressionMarker;
    if (marker != null) {
      _suppressionPending.addAll(chunk);
      final markerIndex = _indexOfBytes(_suppressionPending, marker);
      if (markerIndex < 0) {
        final retain = marker.length - 1;
        if (_suppressionPending.length > retain) {
          _suppressionPending.removeRange(
            0,
            _suppressionPending.length - retain,
          );
        }
        return Uint8List(0);
      }
      input = _suppressionPending.sublist(markerIndex + marker.length);
      _suppressionPending.clear();
      _suppressionMarker = null;
    }

    return Uint8List.fromList(input);
  }

  Uint8List close() {
    _suppressionPending.clear();
    _suppressionMarker = null;
    return Uint8List(0);
  }
}

int _indexOfBytes(List<int> input, List<int> needle) {
  if (needle.isEmpty) return 0;
  for (var index = 0; index + needle.length <= input.length; index++) {
    var matches = true;
    for (var offset = 0; offset < needle.length; offset++) {
      if (input[index + offset] != needle[offset]) {
        matches = false;
        break;
      }
    }
    if (matches) return index;
  }
  return -1;
}
