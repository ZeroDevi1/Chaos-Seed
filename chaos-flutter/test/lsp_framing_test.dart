import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';

import 'package:chaos_flutter/core/daemon/lsp_framing.dart';

void main() {
  test('encode/decode single frame', () {
    final json =
        jsonEncode({'jsonrpc': '2.0', 'id': 1, 'method': 'ping', 'params': {}});
    final bytes = encodeLspFrame(json);
    final dec = LspFrameDecoder();
    final frames = dec.push(bytes).toList();
    expect(frames, hasLength(1));
    expect(utf8.decode(frames.single), json);
  });

  test('decode handles split chunks', () {
    final json = jsonEncode({'a': 1});
    final bytes = encodeLspFrame(json);
    final a = bytes.sublist(0, 10);
    final b = bytes.sublist(10);
    final dec = LspFrameDecoder();
    final f1 = dec.push(a).toList();
    expect(f1, isEmpty);
    final f2 = dec.push(b).toList();
    expect(f2, hasLength(1));
    expect(utf8.decode(f2.single), json);
  });

  test('decode handles multiple frames in one chunk', () {
    final j1 = jsonEncode({'id': 1});
    final j2 = jsonEncode({'id': 2});
    final bytes = [...encodeLspFrame(j1), ...encodeLspFrame(j2)];
    final dec = LspFrameDecoder();
    final frames = dec.push(bytes).toList();
    expect(frames, hasLength(2));
    expect(utf8.decode(frames[0]), j1);
    expect(utf8.decode(frames[1]), j2);
  });
}
