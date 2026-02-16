import 'package:canvas_danmaku/canvas_danmaku.dart';
import 'package:flutter/material.dart';

import '../../../core/models/danmaku.dart';

class DanmakuOverlaySettings {
  final double fontSize;
  final double opacity;
  final double area; // 0..1
  final int durationSeconds;

  const DanmakuOverlaySettings({
    required this.fontSize,
    required this.opacity,
    required this.area,
    required this.durationSeconds,
  });

  DanmakuOption toOption() => DanmakuOption(
        fontSize: fontSize,
        opacity: opacity,
        area: area,
        duration: durationSeconds,
      );
}

class DanmakuOverlay extends StatefulWidget {
  const DanmakuOverlay({
    super.key,
    required this.visible,
    required this.settings,
    required this.onControllerReady,
    this.padding = EdgeInsets.zero,
  });

  final bool visible;
  final DanmakuOverlaySettings settings;
  final void Function(DanmakuController controller) onControllerReady;
  final EdgeInsets padding;

  static String formatText(DanmakuMessage m) {
    final u = m.user.trim();
    final t = m.text.trim();
    if (u.isEmpty) return t;
    if (t.isEmpty) return u;
    return '$u: $t';
  }

  @override
  State<DanmakuOverlay> createState() => _DanmakuOverlayState();
}

class _DanmakuOverlayState extends State<DanmakuOverlay> {
  DanmakuController? _danmaku;

  @override
  void didUpdateWidget(covariant DanmakuOverlay oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_danmaku != null && oldWidget.settings != widget.settings) {
      _danmaku!.updateOption(widget.settings.toOption());
    }
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.visible) return const SizedBox.shrink();

    return Positioned.fill(
      top: widget.padding.top,
      bottom: widget.padding.bottom,
      child: DanmakuScreen(
        createdController: (c) {
          _danmaku = c;
          c.updateOption(widget.settings.toOption());
          widget.onControllerReady(c);
        },
        option: widget.settings.toOption(),
      ),
    );
  }
}

