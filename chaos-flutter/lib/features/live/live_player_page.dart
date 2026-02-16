import 'dart:async';
import 'dart:io';

import 'package:canvas_danmaku/canvas_danmaku.dart';
import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:floating/floating.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:media_kit/media_kit.dart';
import 'package:media_kit_video/media_kit_video.dart';
import 'package:provider/provider.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/models/danmaku.dart';
import '../../core/models/live.dart';
import '../../core/settings/settings_controller.dart';
import '../../core/settings/settings_model.dart';
import '../widgets/material_error_card.dart';
import 'player/android_player_controller.dart';
import 'player/android_player_controls.dart';
import 'player/danmaku_overlay.dart';
import 'player/play_source_sheet.dart';

enum DanmakuRegion {
  off,
  bottom,
  sheet,
}

class LivePlayerPage extends StatefulWidget {
  const LivePlayerPage({
    super.key,
    required this.backend,
    required this.input,
    required this.variants,
    required this.initialVariantId,
  });

  final ChaosBackend backend;
  final String input;
  final List<LivestreamVariant> variants;
  final String initialVariantId;

  @override
  State<LivePlayerPage> createState() => _LivePlayerPageState();
}

class _LivePlayerPageState extends State<LivePlayerPage> {
  late final Player _player = Player();
  late final VideoController _video = VideoController(_player);

  // Android-only controller (single/double tap, lock, fullscreen, tips).
  late final AndroidPlayerController _android = AndroidPlayerController();
  final Floating _pip = Floating();
  StreamSubscription<PiPStatus>? _pipSub;
  bool _pipSupported = false;
  bool _danmakuBeforePip = false;

  bool _loading = false;
  String? _err;

  LiveOpenResult? _open;
  StreamSubscription<DanmakuMessage>? _dmSub;
  final List<DanmakuMessage> _dmTail = [];

  bool _paused = false;
  bool _muted = false;
  DanmakuRegion _dmRegion = DanmakuRegion.bottom;

  // Android: canvas_danmaku overlay.
  bool _dmEnabled = true;
  late DanmakuOverlaySettings _dmSettings = const DanmakuOverlaySettings(
    fontSize: 18,
    opacity: 0.85,
    area: 0.6,
    durationSeconds: 8,
  );
  bool _pipHideDanmu = true;
  DanmakuController? _danmakuCtrl;
  final List<String> _pendingDanmaku = <String>[];

  int _currentLineIndex = 0;
  List<String> _lines = const <String>[];

  bool _androidSettingsInit = false;

  @override
  void initState() {
    super.initState();
    if (Platform.isAndroid) {
      // Android 弹幕不走“列表区域”，避免每条弹幕都 setState 导致掉帧。
      _dmRegion = DanmakuRegion.off;
    }
    unawaited(_openAndPlay(widget.initialVariantId));
    unawaited(_probePip());

    // Let the app manage wakelock; media_kit Video(wakelock: false).
    _player.stream.playing.listen((playing) {
      if (playing) {
        WakelockPlus.enable();
      } else {
        WakelockPlus.disable();
      }
    });
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!Platform.isAndroid) return;

    final s = context.watch<SettingsController>();
    if (!_androidSettingsInit && s.loaded) {
      final cur = s.settings;
      _pipHideDanmu = cur.pipHideDanmu;
      _dmSettings = DanmakuOverlaySettings(
        fontSize: cur.danmuFontSize,
        opacity: cur.danmuOpacity,
        area: cur.danmuArea,
        durationSeconds: cur.danmuSpeedSeconds,
      );
      _androidSettingsInit = true;
    }
  }

  Future<void> _probePip() async {
    if (!Platform.isAndroid) return;
    final v = await _pip.isPipAvailable;
    if (!mounted) return;
    setState(() => _pipSupported = v);
  }

  @override
  void dispose() {
    _pipSub?.cancel();
    _dmSub?.cancel();
    final sid = _open?.sessionId;
    if (sid != null && sid.trim().isNotEmpty) {
      widget.backend.closeLive(sid).ignore();
    }
    if (Platform.isAndroid) {
      // Best-effort restore.
      SystemChrome.setEnabledSystemUIMode(
        SystemUiMode.edgeToEdge,
        overlays: SystemUiOverlay.values,
      ).ignore();
      SystemChrome.setPreferredOrientations(DeviceOrientation.values).ignore();
    }
    _player.dispose();
    _android.dispose();
    super.dispose();
  }

  Future<void> _openAndPlay(String variantId) async {
    if (_loading) return;
    setState(() {
      _loading = true;
      _err = null;
    });

    try {
      // Close previous session.
      if (_open != null) {
        await widget.backend.closeLive(_open!.sessionId);
      }

      final res =
          await widget.backend.openLive(widget.input, variantId: variantId);
      _dmSub?.cancel();

      _pendingDanmaku.clear();
      _danmakuCtrl?.clear();

      _dmSub = widget.backend.danmakuStream(res.sessionId).listen((msg) {
        // Tail for debug/list.
        if (Platform.isWindows && _dmRegion != DanmakuRegion.off) {
          if (!mounted) return;
          setState(() {
            _dmTail.add(msg);
            if (_dmTail.length > 250) {
              _dmTail.removeRange(0, _dmTail.length - 250);
            }
          });
        }

        // Android: feed canvas_danmaku.
        if (Platform.isAndroid && _dmEnabled) {
          final text = DanmakuOverlay.formatText(msg).trim();
          if (text.isEmpty) return;
          if (_danmakuCtrl == null) {
            _pendingDanmaku.add(text);
            if (_pendingDanmaku.length > 200) {
              _pendingDanmaku.removeRange(0, _pendingDanmaku.length - 200);
            }
            return;
          }
          _danmakuCtrl!.addDanmaku(
            DanmakuContentItem(text, color: Colors.white),
          );
        }
      });

      final headers = _httpHeadersForOpen(res);
      final lines = <String>[
        res.url,
        ...res.backupUrls,
      ].map((e) => e.trim()).where((e) => e.isNotEmpty).toList(growable: false);
      if (lines.isEmpty) {
        throw StateError('播放地址为空');
      }

      _lines = lines;
      _currentLineIndex = 0;

      await _player.open(
        Media(lines.first, httpHeaders: headers.isEmpty ? null : headers),
        play: true,
      );

      if (_muted) {
        await _player.setVolume(0);
      } else {
        await _player.setVolume(100);
      }

      if (!mounted) return;
      setState(() {
        _open = res;
        _paused = false;
      });

      if (_dmRegion == DanmakuRegion.sheet) {
        unawaited(_showDanmakuSheet());
      }
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Map<String, String> _httpHeadersForOpen(LiveOpenResult res) {
    final headers = <String, String>{};
    if ((res.referer ?? '').trim().isNotEmpty) headers['Referer'] = res.referer!.trim();
    if ((res.userAgent ?? '').trim().isNotEmpty) headers['User-Agent'] = res.userAgent!.trim();
    return headers;
  }

  String _variantLabel(String id) {
    for (final v in widget.variants) {
      if (v.id == id) return v.label;
    }
    return id;
  }

  Future<void> _pickQuality() async {
    if (Platform.isWindows) {
      final v = await showDialog<LivestreamVariant>(
        context: context,
        builder: (_) {
          final sorted = [...widget.variants]
            ..sort((a, b) => b.quality.compareTo(a.quality));
          return fluent.ContentDialog(
            title: const fluent.Text('选择清晰度'),
            content: fluent.SizedBox(
              width: 420,
              height: 320,
              child: fluent.ListView(
                children: [
                  for (final it in sorted)
                    fluent.ListTile.selectable(
                      title: fluent.Text(it.label),
                      subtitle: fluent.Text('quality=${it.quality}'),
                      onPressed: () => Navigator.of(context).pop(it),
                    ),
                ],
              ),
            ),
            actions: [
              fluent.Button(
                child: const fluent.Text('取消'),
                onPressed: () => Navigator.of(context).pop(),
              ),
            ],
          );
        },
      );
      if (v != null) await _openAndPlay(v.id);
      return;
    }

    final picked = await showModalBottomSheet<LivestreamVariant>(
      context: context,
      showDragHandle: true,
      builder: (_) {
        final sorted = [...widget.variants]
          ..sort((a, b) => b.quality.compareTo(a.quality));
        return SafeArea(
          child: ListView(
            children: [
              const ListTile(title: Text('选择清晰度')),
              for (final it in sorted)
                ListTile(
                  title: Text(it.label),
                  subtitle: Text('清晰度：${it.quality}'),
                  trailing: const Icon(Icons.chevron_right),
                  onTap: () => Navigator.of(context).pop(it),
                ),
              const SizedBox(height: 12),
            ],
          ),
        );
      },
    );
    if (picked != null) await _openAndPlay(picked.id);
  }

  Future<void> _togglePause() async {
    try {
      if (_paused) {
        await _player.play();
      } else {
        await _player.pause();
      }
      if (!mounted) return;
      setState(() => _paused = !_paused);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    }
  }

  Future<void> _toggleMute() async {
    try {
      if (_muted) {
        await _player.setVolume(100);
      } else {
        await _player.setVolume(0);
      }
      if (!mounted) return;
      setState(() => _muted = !_muted);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    }
  }

  Future<void> _enterPip() async {
    if (!Platform.isAndroid) return;
    if (await _pip.isPipAvailable == false) {
      _android.showTip('设备不支持小窗播放');
      return;
    }

    _danmakuBeforePip = _dmEnabled;
    if (_pipHideDanmu && _danmakuBeforePip) {
      setState(() => _dmEnabled = false);
      _danmakuCtrl?.clear();
    }
    _android.hideControlsNow();

    final w = _player.state.width ?? 0;
    final h = _player.state.height ?? 0;
    final ratio =
        (h > w) ? const Rational.vertical() : const Rational.landscape();

    await _pip.enable(ImmediatePiP(aspectRatio: ratio));

    _pipSub ??= _pip.pipStatusStream.listen((st) {
      if (st == PiPStatus.disabled) {
        if (!mounted) return;
        if (_pipHideDanmu) {
          setState(() => _dmEnabled = _danmakuBeforePip);
        }
      }
    });
  }

  Future<void> _toggleFullScreen() async {
    if (!Platform.isAndroid) return;
    final entering = !_android.fullScreen;
    if (entering) {
      _android.enterFullScreen();
      await SystemChrome.setEnabledSystemUIMode(
        SystemUiMode.manual,
        overlays: [],
      );
      // Do not force landscape for vertical streams.
      final w = _player.state.width ?? 0;
      final h = _player.state.height ?? 0;
      final isVertical = (h > w) && (w > 0);
      if (!isVertical) {
        await SystemChrome.setPreferredOrientations(const [
          DeviceOrientation.landscapeLeft,
          DeviceOrientation.landscapeRight,
        ]);
      }
    } else {
      _android.exitFullScreen();
      await SystemChrome.setEnabledSystemUIMode(
        SystemUiMode.edgeToEdge,
        overlays: SystemUiOverlay.values,
      );
      await SystemChrome.setPreferredOrientations(DeviceOrientation.values);
    }
  }

  Future<void> _showDanmakuSheet() async {
    if (!mounted) return;
    if (Platform.isWindows) return;

    await showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      isScrollControlled: true,
      builder: (_) {
        return SafeArea(
          child: SizedBox(
            height: MediaQuery.of(context).size.height * 0.6,
            child: _buildDanmakuList(),
          ),
        );
      },
    );
  }

  Future<void> _showDanmakuSettings() async {
    if (!mounted) return;
    if (Platform.isWindows) return;

    final settingsController = context.read<SettingsController>();
    final current = settingsController.loaded
        ? settingsController.settings
        : AppSettings.defaults();

    await showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (_) {
        return SafeArea(
          child: StatefulBuilder(
            builder: (context, setLocal) {
              void setAll(void Function() fn) {
                setLocal(fn);
                if (!mounted) return;
                setState(fn);
              }

              return ListView(
                padding: const EdgeInsets.fromLTRB(16, 8, 16, 16),
                children: [
                  const ListTile(
                    title: Text('弹幕设置'),
                    subtitle: Text('播放页内弹幕（canvas_danmaku）。'),
                  ),
                  SwitchListTile(
                    title: const Text('弹幕开关'),
                    subtitle: const Text('在视频画面上叠加滚动弹幕'),
                    value: _dmEnabled,
                    onChanged: (v) => setAll(() {
                      _dmEnabled = v;
                      if (!v) _danmakuCtrl?.clear();
                    }),
                  ),
                  SwitchListTile(
                    title: const Text('PiP 时隐藏弹幕'),
                    subtitle: const Text('进入小窗播放时，临时关闭弹幕，退出后恢复'),
                    value: _pipHideDanmu,
                    onChanged: (v) async {
                      setAll(() => _pipHideDanmu = v);
                      await settingsController
                          .update(current.copyWith(pipHideDanmu: v));
                    },
                  ),
                  const Divider(),
                  ListTile(
                    title: const Text('字号'),
                    subtitle: Text(_dmSettings.fontSize.toStringAsFixed(0)),
                  ),
                  Slider(
                    value: _dmSettings.fontSize,
                    min: 12,
                    max: 28,
                    divisions: 16,
                    onChanged: (v) async {
                      setAll(() => _dmSettings = DanmakuOverlaySettings(
                            fontSize: v,
                            opacity: _dmSettings.opacity,
                            area: _dmSettings.area,
                            durationSeconds: _dmSettings.durationSeconds,
                          ));
                      _danmakuCtrl?.updateOption(_dmSettings.toOption());
                      await settingsController
                          .update(current.copyWith(danmuFontSize: v));
                    },
                  ),
                  ListTile(
                    title: const Text('透明度'),
                    subtitle: Text(_dmSettings.opacity.toStringAsFixed(2)),
                  ),
                  Slider(
                    value: _dmSettings.opacity,
                    min: 0.2,
                    max: 1.0,
                    divisions: 16,
                    onChanged: (v) async {
                      setAll(() => _dmSettings = DanmakuOverlaySettings(
                            fontSize: _dmSettings.fontSize,
                            opacity: v,
                            area: _dmSettings.area,
                            durationSeconds: _dmSettings.durationSeconds,
                          ));
                      _danmakuCtrl?.updateOption(_dmSettings.toOption());
                      await settingsController
                          .update(current.copyWith(danmuOpacity: v));
                    },
                  ),
                  ListTile(
                    title: const Text('区域（占屏高度）'),
                    subtitle: Text(_dmSettings.area.toStringAsFixed(2)),
                  ),
                  Slider(
                    value: _dmSettings.area,
                    min: 0.2,
                    max: 1.0,
                    divisions: 16,
                    onChanged: (v) async {
                      setAll(() => _dmSettings = DanmakuOverlaySettings(
                            fontSize: _dmSettings.fontSize,
                            opacity: _dmSettings.opacity,
                            area: v,
                            durationSeconds: _dmSettings.durationSeconds,
                          ));
                      _danmakuCtrl?.updateOption(_dmSettings.toOption());
                      await settingsController
                          .update(current.copyWith(danmuArea: v));
                    },
                  ),
                  ListTile(
                    title: const Text('速度（秒/屏）'),
                    subtitle: Text('${_dmSettings.durationSeconds}'),
                  ),
                  Slider(
                    value: _dmSettings.durationSeconds.toDouble(),
                    min: 4,
                    max: 16,
                    divisions: 12,
                    onChanged: (v) async {
                      final next = v.round().clamp(4, 16);
                      setAll(() => _dmSettings = DanmakuOverlaySettings(
                            fontSize: _dmSettings.fontSize,
                            opacity: _dmSettings.opacity,
                            area: _dmSettings.area,
                            durationSeconds: next,
                          ));
                      _danmakuCtrl?.updateOption(_dmSettings.toOption());
                      await settingsController.update(
                        current.copyWith(danmuSpeedSeconds: next),
                      );
                    },
                  ),
                ],
              );
            },
          ),
        );
      },
    );
  }

  Future<void> _cycleDanmakuRegion() async {
    final next = switch (_dmRegion) {
      DanmakuRegion.off => DanmakuRegion.bottom,
      DanmakuRegion.bottom => DanmakuRegion.sheet,
      DanmakuRegion.sheet => DanmakuRegion.off,
    };
    setState(() => _dmRegion = next);
    if (next == DanmakuRegion.sheet) {
      await _showDanmakuSheet();
    }
  }

  Widget _buildDanmakuList() {
    final dm = _dmTail.reversed.take(200).toList(growable: false);

    String fmt(DanmakuMessage m) {
      final u = m.user.trim();
      final t = m.text.trim();
      if (u.isEmpty) return t;
      if (t.isEmpty) return u;
      return '$u: $t';
    }

    if (Platform.isWindows) {
      return fluent.ListView(
        children: [
          for (final m in dm)
            fluent.Text(
              fmt(m),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
        ],
      );
    }

    return ListView.builder(
      itemCount: dm.length,
      itemBuilder: (context, i) {
        final m = dm[i];
        return ListTile(
          dense: true,
          title: Text(
            fmt(m),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
        );
      },
    );
  }

  Widget _buildControls() {
    final open = _open;
    final qualityLabel = open == null ? '-' : _variantLabel(open.variantId);

    if (Platform.isWindows) {
      return fluent.Row(
        children: [
          fluent.Button(
            onPressed: _loading ? null : _togglePause,
            child: fluent.Text(_paused ? '播放' : '暂停'),
          ),
          const fluent.SizedBox(width: 8),
          fluent.Button(
            onPressed: _loading ? null : _toggleMute,
            child: fluent.Text(_muted ? '取消静音' : '静音'),
          ),
          const fluent.SizedBox(width: 8),
          fluent.Button(
            onPressed: _loading ? null : _pickQuality,
            child: fluent.Text('清晰度：$qualityLabel'),
          ),
          const fluent.SizedBox(width: 8),
          fluent.ToggleButton(
            checked: _dmRegion != DanmakuRegion.off,
            onChanged: (v) => setState(
              () => _dmRegion = v ? DanmakuRegion.bottom : DanmakuRegion.off,
            ),
            child: const fluent.Text('弹幕'),
          ),
        ],
      );
    }

    return Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        FilledButton.icon(
          onPressed: _loading ? null : _togglePause,
          icon: Icon(_paused ? Icons.play_arrow : Icons.pause),
          label: Text(_paused ? '播放' : '暂停'),
        ),
        FilledButton.tonalIcon(
          onPressed: _loading ? null : _toggleMute,
          icon: Icon(_muted ? Icons.volume_up : Icons.volume_off),
          label: Text(_muted ? '取消静音' : '静音'),
        ),
        OutlinedButton.icon(
          onPressed: _loading ? null : _pickQuality,
          icon: const Icon(Icons.hd),
          label: Text('清晰度：$qualityLabel'),
        ),
        OutlinedButton.icon(
          onPressed: _loading ? null : _cycleDanmakuRegion,
          icon: const Icon(Icons.comment),
          label: Text(
            switch (_dmRegion) {
              DanmakuRegion.off => '弹幕：关',
              DanmakuRegion.bottom => '弹幕：底部',
              DanmakuRegion.sheet => '弹幕：面板',
            },
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final title = (_open?.title ?? '').trim().isEmpty ? '播放' : _open!.title;

    if (Platform.isWindows) {
      return fluent.ScaffoldPage(
        header: fluent.PageHeader(title: fluent.Text(title)),
        content: fluent.Padding(
          padding: const EdgeInsets.all(12),
          child: fluent.Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _buildControls(),
              const fluent.SizedBox(height: 8),
              if (_err != null)
                fluent.InfoBar(
                  title: const fluent.Text('错误'),
                  content: fluent.Text(_err!),
                  severity: fluent.InfoBarSeverity.error,
                ),
              if (_loading) const fluent.ProgressRing(),
              const fluent.SizedBox(height: 8),
              fluent.Expanded(
                child: fluent.Row(
                  children: [
                    fluent.Expanded(
                      flex: 3,
                      child: Video(controller: _video),
                    ),
                    const fluent.SizedBox(width: 12),
                    if (_dmRegion != DanmakuRegion.off)
                      fluent.Expanded(
                        flex: 2,
                        child: _buildDanmakuList(),
                      ),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    }

    Widget buildMediaPlayer() {
      final open = _open;
      final variantId = open?.variantId ?? widget.initialVariantId;
      final lines = _lines.isEmpty ? const <String>[''] : _lines;

      return Video(
        controller: _video,
        controls: (state) {
          return AnimatedBuilder(
            animation: _android,
            builder: (context, _) {
              return AndroidPlayerControls(
                videoState: state,
                controller: _android,
                player: _player,
                title: title,
                muted: _muted,
                variants: widget.variants,
                currentVariantId: variantId,
                lines: lines,
                currentLineIndex: _currentLineIndex,
                danmakuEnabled: _dmEnabled,
                danmakuSettings: _dmSettings,
                onDanmakuControllerReady: (c) {
                  _danmakuCtrl = c;
                  if (_pendingDanmaku.isNotEmpty) {
                    for (final t in _pendingDanmaku) {
                      c.addDanmaku(DanmakuContentItem(t, color: Colors.white));
                    }
                    _pendingDanmaku.clear();
                  }
                },
                onBack: () => Navigator.of(context).maybePop(),
                onToggleFullScreen: _toggleFullScreen,
                onEnterPip: _enterPip,
                onToggleMute: _toggleMute,
                onToggleDanmaku: () {
                  setState(() {
                    _dmEnabled = !_dmEnabled;
                    if (!_dmEnabled) _danmakuCtrl?.clear();
                  });
                },
                onShowDanmakuSettings: _showDanmakuSettings,
                onShowPlaySourceSheet: () async {
                  await PlaySourceSheet.show(
                    context: context,
                    variants: widget.variants,
                    currentVariantId: variantId,
                    onPickVariant: (v) async => _openAndPlay(v.id),
                    lines: lines,
                    currentLineIndex: _currentLineIndex,
                    onPickLine: (idx) async {
                      if (_open == null) return;
                      if (idx < 0 || idx >= lines.length) return;
                      if (idx == _currentLineIndex) return;

                      setState(() => _currentLineIndex = idx);
                      final headers = _httpHeadersForOpen(_open!);
                      try {
                        await _player.open(
                          Media(
                            lines[idx],
                            httpHeaders: headers.isEmpty ? null : headers,
                          ),
                          play: true,
                        );
                      } catch (e) {
                        if (!mounted) return;
                        setState(() => _err = e.toString());
                        // Fallback: refresh by re-opening current variant.
                        unawaited(_openAndPlay(variantId));
                      }
                    },
                  );
                },
                onPickVariant: (v) async => _openAndPlay(v.id),
                onPickLine: (idx) async {},
              );
            },
          );
        },
        wakelock: false,
      );
    }

    final page = AnimatedBuilder(
      animation: _android,
      builder: (context, _) {
        if (_android.fullScreen) {
          return PopScope(
            canPop: false,
            onPopInvokedWithResult: (didPop, result) {
              if (didPop) return;
              unawaited(_toggleFullScreen());
            },
            child: Scaffold(
              body: SizedBox.expand(child: buildMediaPlayer()),
            ),
          );
        }

        return Scaffold(
          appBar: AppBar(
            title: Text(title),
            actions: [
              if (_pipSupported)
                IconButton(
                  tooltip: '画中画/小窗播放',
                  onPressed: _enterPip,
                  icon: const Icon(Icons.picture_in_picture_alt),
                ),
              IconButton(
                tooltip: '弹幕设置',
                onPressed: _showDanmakuSettings,
                icon: const Icon(Icons.tune),
              ),
              IconButton(
                tooltip: _paused ? '播放' : '暂停',
                onPressed: _loading ? null : _togglePause,
                icon: Icon(_paused ? Icons.play_arrow : Icons.pause),
              ),
              IconButton(
                tooltip: _muted ? '取消静音' : '静音',
                onPressed: _loading ? null : _toggleMute,
                icon: Icon(_muted ? Icons.volume_up : Icons.volume_off),
              ),
            ],
          ),
          body: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (_err != null) ...[
                  MaterialErrorCard(
                    message: _err!,
                    onDismiss: () => setState(() => _err = null),
                  ),
                  const SizedBox(height: 8),
                ],
                if (_loading) const LinearProgressIndicator(),
                const SizedBox(height: 8),
                AspectRatio(
                  aspectRatio: 16 / 9,
                  child: buildMediaPlayer(),
                ),
                const SizedBox(height: 8),
              ],
            ),
          ),
        );
      },
    );

    return Platform.isAndroid
        ? PiPSwitcher(
            floating: _pip,
            childWhenDisabled: page,
            childWhenEnabled: Scaffold(
              body: SizedBox.expand(child: buildMediaPlayer()),
            ),
          )
        : page;
  }
}

extension _Ignore on Future<void> {
  void ignore() {}
}
