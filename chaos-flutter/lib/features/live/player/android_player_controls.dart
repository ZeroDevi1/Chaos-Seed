import 'dart:async';
import 'dart:io';

import 'package:canvas_danmaku/canvas_danmaku.dart';
import 'package:flutter/material.dart';
import 'package:media_kit/media_kit.dart';
import 'package:media_kit_video/media_kit_video.dart';
import 'package:screen_brightness/screen_brightness.dart';
import 'package:volume_controller/volume_controller.dart';

import '../../../core/models/live.dart';
import 'android_player_controller.dart';
import 'danmaku_overlay.dart';
import 'gesture_adjust.dart';

class AndroidPlayerControls extends StatefulWidget {
  const AndroidPlayerControls({
    super.key,
    required this.videoState,
    required this.controller,
    required this.player,
    required this.title,
    required this.muted,
    required this.variants,
    required this.currentVariantId,
    required this.lines,
    required this.currentLineIndex,
    required this.danmakuEnabled,
    required this.danmakuSettings,
    required this.onDanmakuControllerReady,
    required this.onBack,
    required this.onToggleFullScreen,
    required this.onEnterPip,
    required this.onToggleMute,
    required this.onToggleDanmaku,
    required this.onShowDanmakuSettings,
    required this.onShowPlaySourceSheet,
    required this.onPickVariant,
    required this.onPickLine,
  });

  final VideoState videoState;
  final AndroidPlayerController controller;
  final Player player;
  final String title;
  final bool muted;

  final List<LivestreamVariant> variants;
  final String currentVariantId;

  final List<String> lines;
  final int currentLineIndex;

  final bool danmakuEnabled;
  final DanmakuOverlaySettings danmakuSettings;
  final void Function(DanmakuController) onDanmakuControllerReady;

  final VoidCallback onBack;
  final VoidCallback onToggleFullScreen;
  final Future<void> Function() onEnterPip;
  final Future<void> Function() onToggleMute;
  final VoidCallback onToggleDanmaku;
  final VoidCallback onShowDanmakuSettings;
  final Future<void> Function() onShowPlaySourceSheet;
  final Future<void> Function(LivestreamVariant v) onPickVariant;
  final Future<void> Function(int idx) onPickLine;

  @override
  State<AndroidPlayerControls> createState() => _AndroidPlayerControlsState();
}

class _AndroidPlayerControlsState extends State<AndroidPlayerControls> {
  double? _dragStartX;
  double _dragDy = 0;
  double? _startVolume; // 0..1
  double? _startBrightness; // 0..1

  @override
  void initState() {
    super.initState();
    if (Platform.isAndroid) {
      // 避免系统音量 UI 干扰（simple_live 同款）。
      VolumeController.instance.showSystemUI = false;
    }
  }

  Future<void> _onVerticalDragStart(DragStartDetails d) async {
    if (widget.controller.locked) return;
    final size = MediaQuery.sizeOf(context);
    final dy = d.globalPosition.dy;
    // simple_live: start position must be in the middle half of the screen.
    if (dy < size.height * 0.25 || dy > size.height * 0.75) {
      _dragStartX = null;
      return;
    }

    _dragStartX = d.globalPosition.dx;
    _dragDy = 0;
    try {
      _startVolume = await VolumeController.instance.getVolume();
    } catch (_) {
      _startVolume = null;
    }
    try {
      // Per-app brightness (best-effort).
      _startBrightness = await ScreenBrightness.instance.application;
    } catch (_) {
      _startBrightness = null;
    }
  }

  Future<void> _onVerticalDragUpdate(DragUpdateDetails d) async {
    if (widget.controller.locked) return;
    if (_dragStartX == null) return;
    _dragDy += d.delta.dy;

    final size = MediaQuery.sizeOf(context);
    final decision = GestureAdjust.decide(
      startX: _dragStartX!,
      screenWidth: size.width,
      screenHeight: size.height,
      deltaY: _dragDy,
      locked: widget.controller.locked,
    );
    if (decision.kind == GestureAdjustKind.none) return;

    if (decision.kind == GestureAdjustKind.volume) {
      final cur = _startVolume ?? 0.5;
      final next =
          GestureAdjust.applyNormalized(current: cur, delta: decision.delta);
      try {
        await VolumeController.instance.setVolume(next);
      } catch (_) {}
      widget.controller.showTip('音量 ${(next * 100).round()}%');
      return;
    }

    if (decision.kind == GestureAdjustKind.brightness) {
      final cur = _startBrightness ?? 0.5;
      final next =
          GestureAdjust.applyNormalized(current: cur, delta: decision.delta);
      try {
        await ScreenBrightness.instance.setApplicationScreenBrightness(next);
      } catch (_) {}
      widget.controller.showTip('亮度 ${(next * 100).round()}%');
      return;
    }
  }

  void _onVerticalDragEnd(DragEndDetails d) {
    _dragStartX = null;
    _dragDy = 0;
  }

  Widget _buildCenterBuffering() {
    return Center(
      child: StreamBuilder<bool>(
        stream: widget.player.stream.buffering,
        initialData: widget.player.state.buffering,
        builder: (_, s) => Visibility(
          visible: s.data ?? false,
          child: const Center(
            child: CircularProgressIndicator(),
          ),
        ),
      ),
    );
  }

  Widget _buildGestureTip() {
    return AnimatedOpacity(
      opacity: widget.controller.showGestureTip ? 1 : 0,
      duration: const Duration(milliseconds: 120),
      child: Center(
        child: Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: Colors.grey.shade900.withValues(alpha: 0.85),
            borderRadius: BorderRadius.circular(12),
          ),
          child: Text(
            widget.controller.gestureTipText,
            style: const TextStyle(fontSize: 18, color: Colors.white),
          ),
        ),
      ),
    );
  }

  Widget _buildLockButton() {
    return InkWell(
      onTap: widget.controller.toggleLock,
      child: Container(
        decoration: BoxDecoration(
          color: Colors.black45,
          borderRadius: BorderRadius.circular(8),
        ),
        width: 40,
        height: 40,
        child: Center(
          child: Icon(
            widget.controller.locked ? Icons.lock_outline : Icons.lock_open,
            color: Colors.white,
            size: 20,
          ),
        ),
      ),
    );
  }

  Widget _buildBottomBar({required EdgeInsets padding}) {
    final show = widget.controller.showControls && !widget.controller.locked;
    final bottom = show ? 0.0 : -(56 + padding.bottom);

    final qualityLabel = _variantLabel(widget.currentVariantId);
    final lineLabel = '线路${widget.currentLineIndex + 1}';

    return AnimatedPositioned(
      left: 0,
      right: 0,
      bottom: bottom,
      duration: const Duration(milliseconds: 200),
      child: Container(
        decoration: const BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: [
              Colors.transparent,
              Colors.black87,
            ],
          ),
        ),
        padding: EdgeInsets.only(
          left: padding.left + 12,
          right: padding.right + 12,
          bottom: padding.bottom,
        ),
        child: LayoutBuilder(
          builder: (context, c) {
            final isPortrait =
                MediaQuery.orientationOf(context) == Orientation.portrait;
            final compact = isPortrait || c.maxWidth < 420;

            return Row(
              children: [
                IconButton(
                  onPressed: () async {
                    widget.controller.resetHideTimer();
                    if (widget.player.state.playing) {
                      await widget.player.pause();
                    } else {
                      await widget.player.play();
                    }
                  },
                  icon: Icon(
                    widget.player.state.playing ? Icons.pause : Icons.play_arrow,
                    color: Colors.white,
                  ),
                ),
                IconButton(
                  onPressed: () async {
                    widget.controller.resetHideTimer();
                    await widget.onToggleMute();
                  },
                  icon: Icon(
                    widget.muted ? Icons.volume_off : Icons.volume_up,
                    color: Colors.white,
                  ),
                ),
                IconButton(
                  onPressed: () {
                    widget.controller.resetHideTimer();
                    widget.onToggleDanmaku();
                  },
                  icon: ImageIcon(
                    AssetImage(
                      // same as simple_live: off -> open icon; on -> close icon
                      widget.danmakuEnabled
                          ? 'assets/icons/icon_danmaku_close.png'
                          : 'assets/icons/icon_danmaku_open.png',
                    ),
                    size: 24,
                    color: Colors.white,
                  ),
                ),
                IconButton(
                  onPressed: () {
                    widget.controller.resetHideTimer();
                    widget.onShowDanmakuSettings();
                  },
                  icon: const ImageIcon(
                    AssetImage('assets/icons/icon_danmaku_setting.png'),
                    size: 24,
                    color: Colors.white,
                  ),
                ),
                const Spacer(),
                if (compact)
                  IconButton(
                    tooltip: '清晰度/线路',
                    onPressed: () async {
                      widget.controller.resetHideTimer();
                      await widget.onShowPlaySourceSheet();
                    },
                    icon: const Icon(Icons.hd, color: Colors.white),
                  )
                else ...[
                  TextButton(
                    onPressed: () async {
                      widget.controller.resetHideTimer();
                      await widget.onShowPlaySourceSheet();
                    },
                    child: Text(
                      qualityLabel,
                      style: const TextStyle(color: Colors.white, fontSize: 15),
                    ),
                  ),
                  TextButton(
                    onPressed: () async {
                      widget.controller.resetHideTimer();
                      await widget.onShowPlaySourceSheet();
                    },
                    child: Text(
                      lineLabel,
                      style: const TextStyle(color: Colors.white, fontSize: 15),
                    ),
                  ),
                ],
                IconButton(
                  onPressed: () {
                    widget.controller.resetHideTimer();
                    widget.onToggleFullScreen();
                  },
                  icon: Icon(
                    widget.controller.fullScreen
                        ? Icons.fullscreen_exit
                        : Icons.fullscreen,
                    color: Colors.white,
                  ),
                ),
              ],
            );
          },
        ),
      ),
    );
  }

  Widget _buildTopBar({required EdgeInsets padding}) {
    final show = widget.controller.showControls && !widget.controller.locked;
    final top = show ? 0.0 : -(48 + padding.top);

    return AnimatedPositioned(
      left: 0,
      right: 0,
      top: top,
      duration: const Duration(milliseconds: 200),
      child: Container(
        height: 48 + padding.top,
        padding: EdgeInsets.only(
          left: padding.left + 12,
          right: padding.right + 12,
          top: padding.top,
        ),
        decoration: const BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.bottomCenter,
            end: Alignment.topCenter,
            colors: [
              Colors.transparent,
              Colors.black87,
            ],
          ),
        ),
        child: Row(
          children: [
            IconButton(
              onPressed: () {
                widget.controller.resetHideTimer();
                if (widget.controller.fullScreen) {
                  widget.onToggleFullScreen();
                } else {
                  widget.onBack();
                }
              },
              icon: const Icon(Icons.arrow_back, color: Colors.white),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                widget.title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(color: Colors.white, fontSize: 16),
              ),
            ),
            IconButton(
              onPressed: () async {
                widget.controller.resetHideTimer();
                await widget.onEnterPip();
              },
              icon: const Icon(
                Icons.picture_in_picture,
                color: Colors.white,
              ),
            ),
          ],
        ),
      ),
    );
  }

  String _variantLabel(String id) {
    for (final v in widget.variants) {
      if (v.id == id) return v.label;
    }
    return id;
  }

  @override
  Widget build(BuildContext context) {
    final padding = MediaQuery.of(context).padding;

    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: widget.controller.toggleControls,
      onDoubleTap: widget.controller.locked ? null : widget.onToggleFullScreen,
      onVerticalDragStart: _onVerticalDragStart,
      onVerticalDragUpdate: _onVerticalDragUpdate,
      onVerticalDragEnd: _onVerticalDragEnd,
      child: Stack(
        children: [
          // Danmaku layer.
          DanmakuOverlay(
            visible: widget.danmakuEnabled,
            settings: widget.danmakuSettings,
            onControllerReady: widget.onDanmakuControllerReady,
            padding: EdgeInsets.only(top: padding.top, bottom: padding.bottom),
          ),

          _buildCenterBuffering(),

          if (widget.controller.showGestureTip) _buildGestureTip(),

          if (widget.controller.fullScreen) _buildTopBar(padding: padding),
          _buildBottomBar(padding: padding),

          if (widget.controller.fullScreen)
            AnimatedPositioned(
              top: 0,
              bottom: 0,
              left: widget.controller.showControls ? padding.left + 12 : -64,
              duration: const Duration(milliseconds: 200),
              child: Center(child: _buildLockButton()),
            ),
          if (widget.controller.fullScreen)
            AnimatedPositioned(
              top: 0,
              bottom: 0,
              right: widget.controller.showControls ? padding.right + 12 : -64,
              duration: const Duration(milliseconds: 200),
              child: Center(child: _buildLockButton()),
            ),
        ],
      ),
    );
  }
}
