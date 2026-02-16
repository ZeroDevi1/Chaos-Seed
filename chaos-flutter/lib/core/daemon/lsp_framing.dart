import 'dart:convert';

List<int> encodeLspFrame(String json) {
  final body = utf8.encode(json);
  final header = utf8.encode('Content-Length: ${body.length}\r\n\r\n');
  return [...header, ...body];
}

class LspFrameDecoder {
  final _buf = <int>[];

  /// Push raw bytes and emit any complete JSON frames.
  Iterable<List<int>> push(List<int> chunk) sync* {
    _buf.addAll(chunk);
    while (true) {
      final headerEnd = _indexOf(_buf, const [13, 10, 13, 10]); // \r\n\r\n
      if (headerEnd < 0) return;
      final headerBytes = _buf.sublist(0, headerEnd);
      final headerText = ascii.decode(headerBytes, allowInvalid: true);
      final len = _parseContentLength(headerText);
      if (len == null) {
        // Drop invalid header and resync by removing up to delimiter.
        _buf.removeRange(0, headerEnd + 4);
        continue;
      }
      final frameStart = headerEnd + 4;
      final frameEnd = frameStart + len;
      if (_buf.length < frameEnd) return;
      final body = _buf.sublist(frameStart, frameEnd);
      _buf.removeRange(0, frameEnd);
      yield body;
    }
  }

  static int _indexOf(List<int> buf, List<int> needle) {
    if (needle.isEmpty) return 0;
    for (var i = 0; i <= buf.length - needle.length; i++) {
      var ok = true;
      for (var j = 0; j < needle.length; j++) {
        if (buf[i + j] != needle[j]) {
          ok = false;
          break;
        }
      }
      if (ok) return i;
    }
    return -1;
  }

  static int? _parseContentLength(String header) {
    for (final line in header.split('\r\n')) {
      final idx = line.indexOf(':');
      if (idx < 0) continue;
      final key = line.substring(0, idx).trim().toLowerCase();
      if (key != 'content-length') continue;
      final v = line.substring(idx + 1).trim();
      return int.tryParse(v);
    }
    return null;
  }
}
