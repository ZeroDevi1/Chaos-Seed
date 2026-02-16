enum GestureAdjustKind {
  none,
  brightness,
  volume,
}

class GestureAdjustDecision {
  final GestureAdjustKind kind;
  final double delta; // normalized delta in [-1, 1]
  final String tipText;

  const GestureAdjustDecision({
    required this.kind,
    required this.delta,
    required this.tipText,
  });
}

class GestureAdjust {
  static GestureAdjustDecision decide({
    required double startX,
    required double screenWidth,
    required double screenHeight,
    required double deltaY,
    required bool locked,
  }) {
    if (locked) {
      return const GestureAdjustDecision(
        kind: GestureAdjustKind.none,
        delta: 0,
        tipText: '',
      );
    }
    if (screenWidth <= 0) {
      return const GestureAdjustDecision(
        kind: GestureAdjustKind.none,
        delta: 0,
        tipText: '',
      );
    }
    if (screenHeight <= 0) {
      return const GestureAdjustDecision(
        kind: GestureAdjustKind.none,
        delta: 0,
        tipText: '',
      );
    }

    final kind = startX < (screenWidth / 2)
        ? GestureAdjustKind.brightness
        : GestureAdjustKind.volume;

    // deltaY 向上为负；向上滑应该增加亮度/音量。
    // simple_live: 以屏幕高度的 1/2 作为“满量程”手势距离。
    final raw = (-deltaY) / (screenHeight * 0.5);
    final delta = raw.clamp(-1.0, 1.0);

    final tipText = switch (kind) {
      GestureAdjustKind.brightness => '亮度 ${_fmtDelta(delta)}',
      GestureAdjustKind.volume => '音量 ${_fmtDelta(delta)}',
      GestureAdjustKind.none => '',
    };

    return GestureAdjustDecision(
      kind: kind,
      delta: delta,
      tipText: tipText,
    );
  }

  static double applyNormalized({required double current, required double delta}) {
    final next = current + delta;
    if (next.isNaN) return 0;
    return next.clamp(0.0, 1.0);
  }

  static String _fmtDelta(double delta) {
    final p = (delta.abs() * 100).round();
    return delta >= 0 ? '+$p%' : '-$p%';
  }
}
