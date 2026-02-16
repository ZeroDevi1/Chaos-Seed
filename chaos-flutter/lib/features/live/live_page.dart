import 'dart:async';
import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';
import 'package:media_kit/media_kit.dart';
import 'package:media_kit_video/media_kit_video.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/models/danmaku.dart';
import '../../core/models/live.dart';
import '../widgets/material_error_card.dart';

class LivePage extends StatefulWidget {
  const LivePage({super.key, required this.backend});
  final ChaosBackend backend;

  @override
  State<LivePage> createState() => _LivePageState();
}

class _LivePageState extends State<LivePage> {
  final _input = TextEditingController();
  bool _loading = false;
  String? _err;

  LivestreamDecodeManifestResult? _man;
  LiveOpenResult? _open;

  late final Player _player = Player();
  late final VideoController _video = VideoController(_player);

  StreamSubscription<DanmakuMessage>? _dmSub;
  final List<DanmakuMessage> _dmTail = [];
  bool _showDanmaku = true;

  @override
  void dispose() {
    _dmSub?.cancel();
    final sid = _open?.sessionId;
    if (sid != null && sid.trim().isNotEmpty) {
      widget.backend.closeLive(sid).ignore();
    }
    _player.dispose();
    _input.dispose();
    super.dispose();
  }

  Future<void> _decode() async {
    final s = _input.text.trim();
    if (s.isEmpty) return;
    setState(() {
      _loading = true;
      _err = null;
      _man = null;
      _open = null;
      _dmTail.clear();
    });
    try {
      final man = await widget.backend.decodeManifest(s);
      if (!mounted) return;
      setState(() => _man = man);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Future<void> _openVariant(LivestreamVariant v) async {
    final s = _input.text.trim();
    if (s.isEmpty) return;
    setState(() {
      _loading = true;
      _err = null;
    });
    try {
      // Close previous session.
      if (_open != null) {
        await widget.backend.closeLive(_open!.sessionId);
      }

      final res = await widget.backend.openLive(s, variantId: v.id);
      _dmSub?.cancel();
      _dmSub = widget.backend.danmakuStream(res.sessionId).listen((msg) {
        if (!_showDanmaku) return;
        setState(() {
          _dmTail.add(msg);
          if (_dmTail.length > 200) {
            _dmTail.removeRange(0, _dmTail.length - 200);
          }
        });
      });

      final headers = <String, String>{};
      if ((res.referer ?? '').trim().isNotEmpty) {
        headers['Referer'] = res.referer!.trim();
      }
      if ((res.userAgent ?? '').trim().isNotEmpty) {
        headers['User-Agent'] = res.userAgent!.trim();
      }

      await _player.open(
        Media(res.url, httpHeaders: headers.isEmpty ? null : headers),
        play: true,
      );

      if (!mounted) return;
      setState(() => _open = res);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Widget _buildVariantList() {
    final man = _man;
    if (man == null) {
      return Text(
        '请先输入直播间地址并解析。',
        style: TextStyle(color: Platform.isWindows ? null : Colors.grey),
      );
    }
    final variants = [...man.variants]
      ..sort((a, b) => b.quality.compareTo(a.quality));
    if (Platform.isWindows) {
      return GridView.builder(
        gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
          maxCrossAxisExtent: 260,
          mainAxisSpacing: 10,
          crossAxisSpacing: 10,
          childAspectRatio: 2.6,
        ),
        itemCount: variants.length,
        itemBuilder: (context, i) {
          final v = variants[i];
          return fluent.Button(
            onPressed: _loading ? null : () => _openVariant(v),
            child: fluent.Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                fluent.Text(v.label,
                    style: const fluent.TextStyle(fontWeight: FontWeight.w600)),
                fluent.Text('quality=${v.quality}',
                    style: const fluent.TextStyle(fontSize: 12)),
              ],
            ),
          );
        },
      );
    }

    return ListView.builder(
      itemCount: variants.length,
      itemBuilder: (context, i) {
        final v = variants[i];
        return Card(
          child: ListTile(
            title: Text(v.label,
                style: const TextStyle(fontWeight: FontWeight.w600)),
            subtitle: Text('清晰度：${v.quality}'),
            trailing: const Icon(Icons.play_arrow),
            onTap: _loading ? null : () => _openVariant(v),
          ),
        );
      },
    );
  }

  Widget _buildPlayer() {
    final opened = _open;
    if (opened == null) {
      return const SizedBox.shrink();
    }
    final title = opened.title.isEmpty ? 'Live' : opened.title;
    final dm = _dmTail.reversed.take(30).toList(growable: false);

    if (Platform.isWindows) {
      return fluent.Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          fluent.Row(
            children: [
              fluent.Text(title,
                  style: const fluent.TextStyle(
                      fontSize: 16, fontWeight: FontWeight.w600)),
              const fluent.SizedBox(width: 12),
              fluent.ToggleButton(
                checked: _showDanmaku,
                onChanged: (v) => setState(() => _showDanmaku = v),
                child: const fluent.Text('弹幕'),
              ),
            ],
          ),
          const fluent.SizedBox(height: 8),
          fluent.Expanded(
            child: fluent.Row(
              children: [
                fluent.Expanded(
                  flex: 3,
                  child: fluent.Container(
                    decoration: BoxDecoration(
                        border: Border.all(
                            color: fluent.Colors.grey.withOpacity(0.2))),
                    child: Video(controller: _video),
                  ),
                ),
                const fluent.SizedBox(width: 12),
                fluent.Expanded(
                  flex: 2,
                  child: fluent.ListView(
                    children: [
                      for (final m in dm)
                        fluent.Text(
                          '${m.user}: ${m.text}',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ],
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
                child: Text(title,
                    style: const TextStyle(fontWeight: FontWeight.w600))),
            Switch(
                value: _showDanmaku,
                onChanged: (v) => setState(() => _showDanmaku = v)),
          ],
        ),
        const SizedBox(height: 8),
        AspectRatio(
          aspectRatio: 16 / 9,
          child: Video(controller: _video),
        ),
        const SizedBox(height: 8),
        Expanded(
          child: ListView(
            children: [
              for (final m in dm)
                Text(
                  '${m.user}: ${m.text}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
            ],
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    if (Platform.isWindows) {
      return fluent.ScaffoldPage(
        header: fluent.PageHeader(title: const fluent.Text('直播')),
        content: fluent.Padding(
          padding: const EdgeInsets.all(12),
          child: fluent.Column(
            children: [
              fluent.Row(children: [
                fluent.Expanded(
                  child: fluent.TextBox(
                    controller: _input,
                    placeholder: '输入直播间地址',
                    onSubmitted: (_) => _decode(),
                  ),
                ),
                const fluent.SizedBox(width: 8),
                fluent.Button(
                    onPressed: _loading ? null : _decode,
                    child: const fluent.Text('解析')),
              ]),
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
                    fluent.Expanded(child: _buildVariantList()),
                    const fluent.SizedBox(width: 12),
                    fluent.Expanded(child: _buildPlayer()),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    }

    return Scaffold(
      appBar: AppBar(title: const Text('直播')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          children: [
            TextField(
              controller: _input,
              decoration: const InputDecoration(labelText: '直播间地址'),
              onSubmitted: (_) => _decode(),
            ),
            const SizedBox(height: 8),
            FilledButton(
                onPressed: _loading ? null : _decode, child: const Text('解析')),
            if (_err != null) ...[
              const SizedBox(height: 8),
              MaterialErrorCard(
                  message: _err!, onDismiss: () => setState(() => _err = null)),
            ],
            if (_loading) const LinearProgressIndicator(),
            const SizedBox(height: 8),
            Expanded(
              child: LayoutBuilder(
                builder: (context, c) {
                  // 手机竖屏优先上下布局；平板/横屏/大屏再左右布局。
                  final wide = c.maxWidth >= 840;

                  if (_open == null) {
                    return _buildVariantList();
                  }

                  if (wide) {
                    return Row(
                      children: [
                        Expanded(child: _buildVariantList()),
                        const SizedBox(width: 12),
                        Expanded(child: _buildPlayer()),
                      ],
                    );
                  }

                  return Column(
                    children: [
                      Expanded(flex: 3, child: _buildPlayer()),
                      const SizedBox(height: 12),
                      Expanded(flex: 2, child: _buildVariantList()),
                    ],
                  );
                },
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
