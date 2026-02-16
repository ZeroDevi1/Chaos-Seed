import 'package:flutter_test/flutter_test.dart';

import 'package:chaos_flutter/core/models/danmaku.dart';

void main() {
  test('DanmakuMessage.fromFfiEventJson uses top-level text when present', () {
    final m = DanmakuMessage.fromFfiEventJson('s1', {
      'received_at_ms': 1,
      'user': 'u',
      'text': 'hello',
      'dms': [
        {'text': 'ignored'},
      ],
    });
    expect(m.user, 'u');
    expect(m.text, 'hello');
  });

  test(
      'DanmakuMessage.fromFfiEventJson falls back to dms[0].text when text is empty',
      () {
    final m = DanmakuMessage.fromFfiEventJson('s1', {
      'received_at_ms': 1,
      'user': 'u',
      'text': '',
      'dms': [
        {'text': 'from_dms'},
      ],
    });
    expect(m.text, 'from_dms');
  });

  test('DanmakuMessage.fromFfiEventJson concatenates dms[].text when needed',
      () {
    final m = DanmakuMessage.fromFfiEventJson('s1', {
      'received_at_ms': 1,
      'user': 'u',
      'text': '   ',
      'dms': [
        {'text': 'a'},
        {'text': 'b'},
        {'text': 'c'},
      ],
    });
    expect(m.text, 'abc');
  });

  test(
      'DanmakuMessage.fromFfiEventJson extracts image_url and image_width from dms[0]',
      () {
    final m = DanmakuMessage.fromFfiEventJson('s1', {
      'received_at_ms': 1,
      'user': 'u',
      'text': '',
      'dms': [
        {'text': 'x', 'image_url': 'http://img', 'image_width': 123},
      ],
    });
    expect(m.imageUrl, 'http://img');
    expect(m.imageWidth, 123);
  });
}
