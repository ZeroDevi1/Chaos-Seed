import 'package:flutter_test/flutter_test.dart';

import 'package:chaos_flutter/features/live/player/gesture_adjust.dart';

void main() {
  test('GestureAdjust decides brightness on left half', () {
    final d = GestureAdjust.decide(
      startX: 100,
      screenWidth: 400,
      screenHeight: 800,
      deltaY: -80,
      locked: false,
    );
    expect(d.kind, GestureAdjustKind.brightness);
    expect(d.delta, isNot(0));
    expect(d.tipText, contains('亮度'));
  });

  test('GestureAdjust decides volume on right half', () {
    final d = GestureAdjust.decide(
      startX: 350,
      screenWidth: 400,
      screenHeight: 800,
      deltaY: 120,
      locked: false,
    );
    expect(d.kind, GestureAdjustKind.volume);
    expect(d.tipText, contains('音量'));
  });

  test('GestureAdjust is disabled when locked', () {
    final d = GestureAdjust.decide(
      startX: 350,
      screenWidth: 400,
      screenHeight: 800,
      deltaY: 120,
      locked: true,
    );
    expect(d.kind, GestureAdjustKind.none);
    expect(d.delta, 0);
  });

  test('GestureAdjust clamps volume between 0..1', () {
    final v1 = GestureAdjust.applyNormalized(
      current: 0.95,
      delta: 0.2,
    );
    expect(v1, 1.0);

    final v2 = GestureAdjust.applyNormalized(
      current: 0.05,
      delta: -0.2,
    );
    expect(v2, 0.0);
  });
}
