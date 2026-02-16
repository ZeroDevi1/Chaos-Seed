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

  bool _deviceIsPortrait(BuildContext context) {
    // `Video(controls: ...)` may wrap the controls in a MediaQuery whose `size`
    // equals the video box (often landscape 16:9 even when the device is portrait).
    // For "portrait live" behavior we must look at the actual device view size.
    final v = View.of(context);
    final logical = v.physicalSize / v.devicePixelRatio;
    return logical.height >= logical.width;
  }

  void _safeUpdateOption() {
    final c = _danmaku;
    if (c == null) return;
    // canvas_danmaku 的 DanmakuScreen 会在 initState 早期触发 createdController，
    // 此时内部 AnimationController 可能尚未初始化；直接 updateOption 会抛 LateInitializationError。
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      try {
        c.updateOption(widget.settings.toOption());
      } catch (_) {
        // ignore: best-effort update
      }
    });
  }

  @override
  void didUpdateWidget(covariant DanmakuOverlay oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_danmaku != null && oldWidget.settings != widget.settings) {
      _safeUpdateOption();
    }
  }

  @override
  Widget build(BuildContext context) {
    if (!widget.visible) return const SizedBox.shrink();

    return Positioned.fill(
      top: widget.padding.top,
      bottom: widget.padding.bottom,
      child: LayoutBuilder(
        builder: (context, c) {
          final portrait = _deviceIsPortrait(context);
          final screen = DanmakuScreen(
            createdController: (dc) {
              _danmaku = dc;
              widget.onControllerReady(dc);
              // 初次 option 已通过 DanmakuScreen.option 传入，这里只做一次安全兜底更新。
              _safeUpdateOption();
            },
            option: widget.settings.toOption(),
          );

          if (!portrait) return screen;

          // 竖屏状态下，把弹幕限制在画面的下半部分渲染（对齐 simple_live 的竖屏直播间体验）。
          return Align(
            alignment: Alignment.bottomCenter,
            child: SizedBox(
              height: c.maxHeight * 0.5,
              child: ClipRect(child: screen),
            ),
          );
        },
      ),
    );
  }
}
