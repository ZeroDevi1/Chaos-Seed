import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';
import 'package:path/path.dart' as p;
import 'package:provider/provider.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/backend/ffi_backend.dart';
import '../../core/models/lyrics.dart';
import '../../core/models/music.dart';
import '../../core/settings/settings_controller.dart';
import '../widgets/material_error_card.dart';
import 'qq_login_dialog.dart';

class MusicPage extends StatefulWidget {
  const MusicPage({super.key, required this.backend});
  final ChaosBackend backend;

  @override
  State<MusicPage> createState() => _MusicPageState();
}

class _MusicPageState extends State<MusicPage> {
  final _q = TextEditingController();
  bool _loading = false;
  String? _err;
  MusicService _svc = MusicService.qq;
  List<MusicTrack> _tracks = const [];

  @override
  void dispose() {
    _q.dispose();
    super.dispose();
  }

  MusicProviderConfig _cfgFromSettings(SettingsController s) {
    final raw = s.settings.neteaseBaseUrls
        .split(';')
        .map((e) => e.trim())
        .where((e) => e.isNotEmpty)
        .toList(growable: false);
    return MusicProviderConfig(
      kugouBaseUrl: (s.settings.kugouBaseUrl ?? '').trim().isEmpty
          ? null
          : s.settings.kugouBaseUrl!.trim(),
      neteaseBaseUrls: raw,
      neteaseAnonymousCookieUrl:
          s.settings.neteaseAnonymousCookieUrl.trim().isEmpty
              ? null
              : s.settings.neteaseAnonymousCookieUrl.trim(),
    );
  }

  Future<void> _search() async {
    final keyword = _q.text.trim();
    if (keyword.isEmpty) return;
    setState(() {
      _loading = true;
      _err = null;
      _tracks = const [];
    });
    try {
      final s = context.read<SettingsController>();
      await widget.backend.musicConfigSet(_cfgFromSettings(s));
      final items = await widget.backend.searchTracks(MusicSearchParams(
        service: _svc,
        keyword: keyword,
        page: 1,
        pageSize: 20,
      ));
      if (!mounted) return;
      setState(() => _tracks = items);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Future<void> _download(MusicTrack t) async {
    setState(() {
      _loading = true;
      _err = null;
    });
    try {
      final settings = context.read<SettingsController>();
      final cfg = _cfgFromSettings(settings);

      // Android：默认写入 Downloads/ChaosSeed（如果系统限制导致不可写，会自动回退到应用专用目录）。
      final outDir = Platform.isAndroid
          ? await FfiChaosBackend.defaultAndroidDownloadDir()
          : (Directory.current.path);
      await Directory(outDir).create(recursive: true);

      MusicAuthState auth = MusicAuthState.empty();
      if (t.service == MusicService.qq) {
        final raw = (settings.settings.qqMusicCookieJson ?? '').trim();
        if (raw.isEmpty) {
          throw Exception('QQ 音乐：未登录（请先扫码登录以获取 Cookie）。');
        }
        final m = (jsonDecode(raw) as Map).cast<String, dynamic>();
        auth = MusicAuthState(
            qq: QqMusicCookie.fromJson(m), kugou: null, neteaseCookie: null);
      }

      final params = MusicDownloadStartParams(
        config: cfg,
        auth: auth,
        target: {
          'type': 'track',
          'track': t.toJson(),
        },
        options: MusicDownloadOptions(
          qualityId:
              (t.qualities.isNotEmpty ? t.qualities.first.id : 'standard'),
          outDir: outDir,
          pathTemplate:
              _sanitizePathTemplate(settings.settings.musicPathTemplate),
          overwrite: false,
          concurrency: settings.settings.musicDownloadConcurrency,
          retries: settings.settings.musicDownloadRetries,
        ),
      );

      final res = await widget.backend.downloadStart(params);
      // 下载完成后确保歌词也落盘（.lrc）。Rust 侧已做 best-effort，这里再做一次兜底重试。
      unawaited(_ensureLyricsForSingleTrack(sessionId: res.sessionId, track: t));
      if (!mounted) return;
      final msg = '下载开始: sessionId=${res.sessionId}';
      if (Platform.isWindows) {
        await showDialog(
          context: context,
          builder: (_) => fluent.ContentDialog(
            title: const fluent.Text('歌曲下载'),
            content: fluent.Text(msg),
            actions: [
              fluent.Button(
                  child: const fluent.Text('OK'),
                  onPressed: () => Navigator.pop(context))
            ],
          ),
        );
      } else {
        if (!mounted) return;
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(msg)));
      }
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  static String? _sanitizePathTemplate(String? raw) {
    final s = (raw ?? '').trim();
    if (s.isEmpty) return null;
    // WinUI3/XAML 常用 "{}" 前缀转义花括号；Flutter/Rust 不需要。
    if (s.startsWith('{}')) return s.substring(2).trim();
    return s;
  }

  Future<void> _ensureLyricsForSingleTrack({
    required String sessionId,
    required MusicTrack track,
  }) async {
    // 仅在后台执行，不影响 UI 主流程。
    // 目标：找到下载产物的音频文件路径，然后写入同名 .lrc。
    try {
      final deadline = DateTime.now().add(const Duration(minutes: 10));
      MusicDownloadStatus? last;
      while (DateTime.now().isBefore(deadline)) {
        await Future.delayed(const Duration(seconds: 1));
        try {
          last = await widget.backend.downloadStatus(sessionId);
        } catch (_) {
          continue;
        }
        if (last.done) break;
      }
      if (last == null) return;

      String? audioPath;
      for (final j in last.jobs) {
        final state = (j['state'] as String?) ?? '';
        final path = (j['path'] as String?) ?? '';
        if (path.trim().isEmpty) continue;
        if (state.toLowerCase() == 'done') {
          audioPath = path;
          break;
        }
      }
      if (audioPath == null) return;

      final lrcPath = p.setExtension(audioPath, '.lrc');
      if (await File(lrcPath).exists()) return;

      final title = track.title.trim();
      if (title.isEmpty) return;
      final artist = track.artists.join(' / ').trim().isEmpty
          ? null
          : track.artists.join(' / ').trim();

      final items = await widget.backend.lyricsSearch(LyricsSearchParams(
        title: title,
        album: (track.album ?? '').trim().isEmpty ? null : track.album!.trim(),
        artist: artist,
        durationMs: track.durationMs ?? 0,
        limit: 5,
        strictMatch: false,
        services: const ['qq', 'netease', 'lrclib'],
        timeoutMs: 8000,
      ));

      final picked = items
          .where((e) => e.lyricsOriginal.trim().isNotEmpty)
          .toList(growable: false);
      if (picked.isEmpty) return;
      final best = [...picked]
        ..sort((a, b) {
          final q = b.quality.compareTo(a.quality);
          if (q != 0) return q;
          return b.matchPercentage.compareTo(a.matchPercentage);
        });
      final top = best.first;

      var content = top.lyricsOriginal;
      final tr = (top.lyricsTranslation ?? '').trim();
      if (tr.isNotEmpty) {
        content = '$content\n\n$tr';
      }

      await File(lrcPath).writeAsString(content, flush: true);
    } catch (_) {
      // ignore: lyrics is best-effort
    }
  }

  Widget _svcPicker() {
    final svcs = MusicService.values;
    if (Platform.isWindows) {
      return fluent.DropDownButton(
        title: fluent.Text(_svc.name),
        items: [
          for (final s in svcs)
            fluent.MenuFlyoutItem(
              text: fluent.Text(s.name),
              onPressed: () => setState(() => _svc = s),
            ),
        ],
      );
    }
    return DropdownButton<MusicService>(
      value: _svc,
      items: svcs
          .map((s) => DropdownMenuItem(value: s, child: Text(s.name)))
          .toList(),
      onChanged: (v) => setState(() => _svc = v ?? _svc),
    );
  }

  @override
  Widget build(BuildContext context) {
    if (Platform.isWindows) {
      return fluent.ScaffoldPage(
        header: fluent.PageHeader(title: const fluent.Text('歌曲')),
        content: fluent.Padding(
          padding: const EdgeInsets.all(12),
          child: fluent.Column(
            children: [
              fluent.Row(children: [
                _svcPicker(),
                const fluent.SizedBox(width: 8),
                fluent.Expanded(
                  child: fluent.TextBox(
                    controller: _q,
                    placeholder: '搜索歌曲',
                    onSubmitted: (_) => _search(),
                  ),
                ),
                const fluent.SizedBox(width: 8),
                fluent.Button(
                    onPressed: _loading ? null : _search,
                    child: const fluent.Text('搜索')),
              ]),
              const fluent.SizedBox(height: 12),
              if (_err != null)
                fluent.InfoBar(
                  title: const fluent.Text('错误'),
                  content: fluent.Text(_err!),
                  severity: fluent.InfoBarSeverity.error,
                ),
              if (_loading) const fluent.ProgressRing(),
              fluent.Expanded(
                child: fluent.ListView.builder(
                  itemCount: _tracks.length,
                  itemBuilder: (context, i) {
                    final t = _tracks[i];
                    final artist = t.artists.join(' / ');
                    return fluent.ListTile.selectable(
                      title: fluent.Text(t.title),
                      subtitle: fluent.Text(artist),
                      trailing: fluent.Button(
                        onPressed: _loading ? null : () => _download(t),
                        child: const fluent.Text('下载'),
                      ),
                    );
                  },
                ),
              ),
            ],
          ),
        ),
      );
    }

    final s = context.watch<SettingsController>();
    final qqLoggedIn = (s.settings.qqMusicCookieJson ?? '').trim().isNotEmpty;

    return Scaffold(
      appBar: AppBar(
        title: const Text('歌曲'),
        actions: [
          if (_svc == MusicService.qq)
            IconButton(
              tooltip: qqLoggedIn ? 'QQ 音乐：已登录' : 'QQ 音乐：扫码登录',
              icon: Icon(qqLoggedIn ? Icons.verified : Icons.qr_code),
              onPressed: () async {
                final cookie = await showDialog<String?>(
                  context: context,
                  builder: (_) => QqLoginDialog(backend: widget.backend),
                );
                if (cookie == null) return;
                if (!context.mounted) return;
                await s.update(s.settings.copyWith(qqMusicCookieJson: cookie));
              },
            ),
        ],
      ),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          children: [
            if (_svc == MusicService.qq) ...[
              Card(
                child: ListTile(
                  leading: const Icon(Icons.person),
                  title:
                      Text(qqLoggedIn ? 'QQ 音乐：已登录（Cookie 已缓存）' : 'QQ 音乐：未登录'),
                  subtitle: const Text('下载失败时通常是未登录或 Cookie 失效，可点右上角扫码重新登录。'),
                ),
              ),
              const SizedBox(height: 8),
            ],
            Row(
              children: [
                _svcPicker(),
                const SizedBox(width: 8),
                Expanded(
                  child: TextField(
                    controller: _q,
                    decoration: const InputDecoration(labelText: '搜索歌曲'),
                    onSubmitted: (_) => _search(),
                  ),
                ),
                const SizedBox(width: 8),
                FilledButton(
                    onPressed: _loading ? null : _search,
                    child: const Text('搜索')),
              ],
            ),
            const SizedBox(height: 12),
            if (_err != null)
              MaterialErrorCard(
                  message: _err!, onDismiss: () => setState(() => _err = null)),
            if (_loading) const LinearProgressIndicator(),
            Expanded(
              child: ListView.builder(
                itemCount: _tracks.length,
                itemBuilder: (context, i) {
                  final t = _tracks[i];
                  return Card(
                    child: ListTile(
                      title: Text(t.title,
                          maxLines: 1, overflow: TextOverflow.ellipsis),
                      subtitle: Text(t.artists.join(' / '),
                          maxLines: 1, overflow: TextOverflow.ellipsis),
                      trailing: IconButton(
                        icon: const Icon(Icons.download),
                        onPressed: _loading ? null : () => _download(t),
                      ),
                    ),
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
