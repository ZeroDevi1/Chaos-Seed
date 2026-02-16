import 'package:flutter_test/flutter_test.dart';

import 'package:chaos_flutter/core/models/danmaku.dart';
import 'package:chaos_flutter/features/live/player/danmaku_overlay.dart';

void main() {
  test('Danmaku mapping formats text with user prefix', () {
    final m = DanmakuMessage(
      sessionId: 's',
      receivedAtMs: 1,
      user: 'u',
      text: 't',
      imageUrl: null,
      imageWidth: null,
    );
    expect(DanmakuOverlay.formatText(m), 'u: t');
  });

  test('Danmaku mapping handles empty user', () {
    final m = DanmakuMessage(
      sessionId: 's',
      receivedAtMs: 1,
      user: '',
      text: 't',
      imageUrl: null,
      imageWidth: null,
    );
    expect(DanmakuOverlay.formatText(m), 't');
  });

  test('Danmaku mapping handles empty text', () {
    final m = DanmakuMessage(
      sessionId: 's',
      receivedAtMs: 1,
      user: 'u',
      text: '',
      imageUrl: null,
      imageWidth: null,
    );
    expect(DanmakuOverlay.formatText(m), 'u');
  });
}

