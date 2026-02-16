import 'dart:async';
import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/models/lyrics.dart';
import '../../core/models/now_playing.dart';

class LyricsPage extends StatefulWidget {
  const LyricsPage({super.key, required this.backend});
  final ChaosBackend backend;

  @override
  State<LyricsPage> createState() => _LyricsPageState();
}

class _LyricsPageState extends State<LyricsPage> {
  bool _loading = false;
  String? _err;

  NowPlayingSnapshot? _np;
  List<LyricsSearchResult> _items = const [];

  @override
  void initState() {
    super.initState();
    unawaited(_refreshNowPlaying());
  }

  Future<void> _refreshNowPlaying() async {
    setState(() {
      _loading = true;
      _err = null;
    });
    try {
      final snap = await widget.backend
          .nowPlayingSnapshot(const NowPlayingSnapshotParams(
        includeThumbnail: false,
        maxThumbnailBytes: 262144,
        maxSessions: 32,
      ));
      if (!mounted) return;
      setState(() => _np = snap);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Future<void> _searchFromNowPlaying() async {
    final np = _np?.nowPlaying;
    if (np == null) return;
    final title = (np.title ?? '').trim();
    if (title.isEmpty) return;

    setState(() {
      _loading = true;
      _err = null;
      _items = const [];
    });
    try {
      final res = await widget.backend.lyricsSearch(LyricsSearchParams(
        title: title,
        album: np.albumTitle,
        artist: np.artist,
        durationMs: np.durationMs ?? 0,
        limit: 5,
        strictMatch: false,
        services: const ['qq', 'netease', 'lrclib'],
        timeoutMs: 8000,
      ));
      if (!mounted) return;
      setState(() => _items = res);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (!Platform.isWindows) {
      return Scaffold(
        appBar: AppBar(title: const Text('歌词')),
        body: const Center(child: Text('Android 端第一阶段不实现歌词页。')),
      );
    }

    if (Platform.isWindows) {
      return fluent.ScaffoldPage(
        header: fluent.PageHeader(title: const fluent.Text('歌词')),
        content: fluent.Padding(
          padding: const EdgeInsets.all(12),
          child: fluent.Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              fluent.Row(children: [
                fluent.Button(
                    onPressed: _loading ? null : _refreshNowPlaying,
                    child: const fluent.Text('刷新 Now Playing')),
                const fluent.SizedBox(width: 8),
                fluent.Button(
                    onPressed: _loading ? null : _searchFromNowPlaying,
                    child: const fluent.Text('搜索歌词')),
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
              fluent.Text(_np?.nowPlaying?.title ?? '(no title)'),
              fluent.Text((_np?.nowPlaying?.artist ?? '').toString(),
                  style: const fluent.TextStyle(fontSize: 12)),
              const fluent.SizedBox(height: 12),
              fluent.Expanded(
                child: fluent.ListView(
                  children: [
                    for (final it in _items)
                      fluent.Expander(
                        header: fluent.Text('${it.service}: ${it.title ?? ''}'),
                        content: fluent.Text(it.lyricsOriginal),
                      ),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    }

    return const SizedBox.shrink();
  }
}
