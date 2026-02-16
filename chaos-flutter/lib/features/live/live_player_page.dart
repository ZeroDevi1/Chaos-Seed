import 'dart:async';
import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';
import 'package:media_kit/media_kit.dart';
import 'package:media_kit_video/media_kit_video.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/models/danmaku.dart';
import '../../core/models/live.dart';
import '../../core/platform/android_bridge.dart';
import '../widgets/material_error_card.dart';

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

  bool _loading = false;
  String? _err;

  LiveOpenResult? _open;
  StreamSubscription<DanmakuMessage>? _dmSub;
  final List<DanmakuMessage> _dmTail = [];

  bool _paused = false;
  bool _muted = false;
  DanmakuRegion _dmRegion = DanmakuRegion.bottom;
  bool _pipSupported = false;

  // Android：弹幕叠加层（第一阶段简化：仅渲染最近若干条文本）。
  bool _dmOverlayEnabled = true;
  double _dmOverlayOpacity = 0.45;
  double _dmFontSize = 13;
  int _dmOverlayLines = 6;

  @override
  void initState() {
    super.initState();
    unawaited(_openAndPlay(widget.initialVariantId));
    unawaited(_probePip());
  }

  Future<void> _probePip() async {
    if (!Platform.isAndroid) return;
    final v = await AndroidBridge.isPipSupported();
    if (!mounted) return;
    setState(() => _pipSupported = v);
  }

  @override
  void dispose() {
    _dmSub?.cancel();
    final sid = _open?.sessionId;
    if (sid != null && sid.trim().isNotEmpty) {
      widget.backend.closeLive(sid).ignore();
    }
    _player.dispose();
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
      _dmSub = widget.backend.danmakuStream(res.sessionId).listen((msg) {
        if (_dmRegion == DanmakuRegion.off) return;
        setState(() {
          _dmTail.add(msg);
          if (_dmTail.length > 250) {
            _dmTail.removeRange(0, _dmTail.length - 250);
          }
        });
      });

      final headers = <String, String>{};
      if ((res.referer ?? '').trim().isNotEmpty)
        headers['Referer'] = res.referer!.trim();
      if ((res.userAgent ?? '').trim().isNotEmpty)
        headers['User-Agent'] = res.userAgent!.trim();

      await _player.open(
        Media(res.url, httpHeaders: headers.isEmpty ? null : headers),
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
                  onPressed: () => Navigator.of(context).pop()),
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
    try {
      await AndroidBridge.enterPip(aspectW: 16, aspectH: 9);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
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
                    subtitle: Text('第一阶段：仅提供基础叠加层控制。'),
                  ),
                  SwitchListTile(
                    title: const Text('叠加层'),
                    subtitle: const Text('在视频画面上叠加最近弹幕'),
                    value: _dmOverlayEnabled,
                    onChanged: (v) => setAll(() => _dmOverlayEnabled = v),
                  ),
                  ListTile(
                    title: const Text('透明度'),
                    subtitle: Text(_dmOverlayOpacity.toStringAsFixed(2)),
                  ),
                  Slider(
                    value: _dmOverlayOpacity,
                    min: 0.0,
                    max: 0.9,
                    divisions: 18,
                    onChanged: (v) => setAll(() => _dmOverlayOpacity = v),
                  ),
                  ListTile(
                    title: const Text('字号'),
                    subtitle: Text(_dmFontSize.toStringAsFixed(0)),
                  ),
                  Slider(
                    value: _dmFontSize,
                    min: 10,
                    max: 22,
                    divisions: 12,
                    onChanged: (v) => setAll(() => _dmFontSize = v),
                  ),
                  ListTile(
                    title: const Text('行数'),
                    subtitle: Text('$_dmOverlayLines'),
                  ),
                  Slider(
                    value: _dmOverlayLines.toDouble(),
                    min: 1,
                    max: 12,
                    divisions: 11,
                    onChanged: (v) => setAll(() => _dmOverlayLines = v.round()),
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
                () => _dmRegion = v ? DanmakuRegion.bottom : DanmakuRegion.off),
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
                      child: fluent.Container(
                        child: Video(controller: _video),
                      ),
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
          IconButton(
            tooltip: '清晰度',
            onPressed: _loading ? null : _pickQuality,
            icon: const Icon(Icons.hd),
          ),
          PopupMenuButton<DanmakuRegion>(
            tooltip: '弹幕区域',
            onSelected: (v) async {
              setState(() => _dmRegion = v);
              if (v == DanmakuRegion.sheet) {
                await _showDanmakuSheet();
              }
            },
            itemBuilder: (_) => const [
              PopupMenuItem(value: DanmakuRegion.off, child: Text('弹幕：关闭')),
              PopupMenuItem(value: DanmakuRegion.bottom, child: Text('弹幕：底部')),
              PopupMenuItem(value: DanmakuRegion.sheet, child: Text('弹幕：弹出面板')),
            ],
          ),
        ],
      ),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _buildControls(),
            if (_err != null) ...[
              const SizedBox(height: 8),
              MaterialErrorCard(
                  message: _err!, onDismiss: () => setState(() => _err = null)),
            ],
            if (_loading) const LinearProgressIndicator(),
            const SizedBox(height: 8),
            AspectRatio(
              aspectRatio: 16 / 9,
              child: Stack(
                children: [
                  Positioned.fill(child: Video(controller: _video)),
                  if (_dmRegion != DanmakuRegion.off && _dmOverlayEnabled)
                    Positioned(
                      left: 8,
                      right: 8,
                      bottom: 8,
                      child: ClipRRect(
                        borderRadius: BorderRadius.circular(12),
                        child: Container(
                          padding: const EdgeInsets.symmetric(
                              horizontal: 10, vertical: 8),
                          color:
                              Colors.black.withValues(alpha: _dmOverlayOpacity),
                          child: DefaultTextStyle(
                            style: TextStyle(
                              color: Colors.white,
                              fontSize: _dmFontSize,
                              height: 1.2,
                            ),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                for (final m in _dmTail.reversed
                                    .take(_dmOverlayLines)
                                    .toList()
                                    .reversed)
                                  Builder(
                                    builder: (context) {
                                      final u = m.user.trim();
                                      final prefix = u.isEmpty ? '' : '$u: ';
                                      return Text(
                                        '$prefix${m.text}',
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                      );
                                    },
                                  ),
                              ],
                            ),
                          ),
                        ),
                      ),
                    ),
                ],
              ),
            ),
            const SizedBox(height: 8),
            if (_dmRegion == DanmakuRegion.bottom)
              Expanded(
                child: Card(
                  child: _buildDanmakuList(),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

extension _Ignore on Future<void> {
  void ignore() {}
}
