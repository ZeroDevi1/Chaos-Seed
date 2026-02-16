import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:http/http.dart' as http;
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

import '../ffi/ffi_isolate_runner.dart';
import '../models/danmaku.dart';
import '../models/live.dart';
import '../models/live_directory.dart';
import '../models/lyrics.dart';
import '../models/music.dart';
import '../models/now_playing.dart';
import '../models/subtitles.dart';
import '../platform/android_bridge.dart';
import 'chaos_backend.dart';

class FfiChaosBackend implements ChaosBackend {
  FfiChaosBackend({FfiIsolateRunner? runner, http.Client? httpClient})
      : _runner = runner ?? FfiIsolateRunner(),
        _http = httpClient ?? http.Client();

  final FfiIsolateRunner _runner;
  final http.Client _http;

  LivestreamDecodeManifestResult? _lastManifest;

  final _danmaku = <String, StreamController<DanmakuMessage>>{};
  final _activeSessions = <String>{};

  @override
  String get name => 'FFI';

  @override
  Future<List<LiveDirCategory>> categories(String site) async {
    final jsonStr =
        await _runner.call('liveDir.categories', {'site': site}) as String;
    final raw = jsonDecode(jsonStr);
    final list = (raw as List)
        .whereType<Map>()
        .map((e) => LiveDirCategory.fromJson(e.cast<String, dynamic>()))
        .toList();
    return list;
  }

  @override
  Future<LiveDirRoomListResult> recommendRooms(String site, int page) async {
    final jsonStr = await _runner
        .call('liveDir.recommendRooms', {'site': site, 'page': page}) as String;
    return LiveDirRoomListResult.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<LiveDirRoomListResult> categoryRooms(
    String site,
    String? parentId,
    String categoryId,
    int page,
  ) async {
    final jsonStr = await _runner.call('liveDir.categoryRooms', {
      'site': site,
      'parentId': parentId,
      'categoryId': categoryId,
      'page': page,
    }) as String;
    return LiveDirRoomListResult.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<LiveDirRoomListResult> searchRooms(
      String site, String keyword, int page) async {
    final jsonStr = await _runner.call('liveDir.searchRooms',
        {'site': site, 'keyword': keyword, 'page': page}) as String;
    return LiveDirRoomListResult.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<LivestreamDecodeManifestResult> decodeManifest(String input) async {
    final jsonStr =
        await _runner.call('live.decodeManifest', {'input': input}) as String;
    final man = LivestreamDecodeManifestResult.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
    _lastManifest = man;
    return man;
  }

  LivestreamVariant _pickVariant(
      LivestreamDecodeManifestResult man, String? requestedId) {
    final rid = (requestedId ?? '').trim();
    if (rid.isNotEmpty) {
      for (final v in man.variants) {
        if (v.id == rid) return v;
      }
      throw StateError('variant not found: $rid');
    }

    // Prefer highest quality that has a direct url/backups.
    final sorted = [...man.variants]
      ..sort((a, b) => b.quality.compareTo(a.quality));
    for (final v in sorted) {
      final ok = (v.url != null && v.url!.trim().isNotEmpty) ||
          v.backupUrls.isNotEmpty;
      if (ok) return v;
    }
    if (sorted.isEmpty) throw StateError('no variants');
    return sorted.first;
  }

  @override
  Future<LiveOpenResult> openLive(String input, {String? variantId}) async {
    // `_lastManifest` is a global cache shared across features. If the input differs
    // (or the cached manifest is incomplete), we must decode again.
    var man = _lastManifest;
    final inTrim = input.trim();
    final manTrim = (man?.rawInput ?? '').trim();
    if (man == null || manTrim.isEmpty || manTrim != inTrim || man.roomId.trim().isEmpty) {
      man = await decodeManifest(input);
    }
    final picked = _pickVariant(man, variantId);

    LivestreamVariant finalV = picked;
    final hasUrl = (picked.url != null && picked.url!.trim().isNotEmpty) ||
        picked.backupUrls.isNotEmpty;
    if (!hasUrl) {
      final rid = man.roomId.trim();
      if (rid.isEmpty) {
        // Should never happen (core returns canonical room_id), but keep a clearer error here.
        throw StateError('roomId 为空：请先重新解析直播间后再切换清晰度');
      }
      final resolvedJson = await _runner.call('live.resolveVariant2', {
        'site': man.site,
        'roomId': rid,
        'variantId': picked.id,
      }) as String;
      finalV = LivestreamVariant.fromJson(
          (jsonDecode(resolvedJson) as Map).cast<String, dynamic>());
    }

    final url = (finalV.url ?? '').trim();
    final backups = finalV.backupUrls
        .map((e) => e.trim())
        .where((e) => e.isNotEmpty)
        .toList(growable: false);
    if (url.isEmpty && backups.isEmpty) {
      throw StateError('empty url');
    }

    final sessionId = 'ffi-${DateTime.now().microsecondsSinceEpoch}';
    _activeSessions.add(sessionId);
    await _runner.danmakuConnect(sessionId: sessionId, input: input);

    return LiveOpenResult(
      sessionId: sessionId,
      site: man.site,
      roomId: man.roomId,
      title: man.info.title,
      variantId: finalV.id.isNotEmpty ? finalV.id : picked.id,
      variantLabel: (finalV.label.isNotEmpty ? finalV.label : picked.label),
      url: url.isNotEmpty ? url : backups.first,
      backupUrls: backups,
      referer: man.playback.referer,
      userAgent: man.playback.userAgent,
    );
  }

  @override
  Future<void> closeLive(String sessionId) async {
    if (!_activeSessions.remove(sessionId)) return;
    try {
      await _runner.danmakuDisconnect(sessionId: sessionId);
    } catch (_) {
      // ignore
    }
    final c = _danmaku.remove(sessionId);
    if (c != null && !c.isClosed) {
      await c.close();
    }
  }

  @override
  Stream<DanmakuMessage> danmakuStream(String sessionId) {
    final ctrl = _danmaku.putIfAbsent(sessionId, () {
      final c = StreamController<DanmakuMessage>.broadcast(onCancel: () {});
      // Bridge isolate event-json stream -> DanmakuMessage.
      _runner.danmakuEventsJson(sessionId).listen((eventsJson) {
        try {
          final raw = jsonDecode(eventsJson);
          if (raw is! List) return;
          for (final ev in raw) {
            if (ev is Map) {
              c.add(DanmakuMessage.fromFfiEventJson(
                  sessionId, ev.cast<String, dynamic>()));
            }
          }
        } catch (_) {
          // ignore malformed frames
        }
      });
      return c;
    });
    return ctrl.stream;
  }

  @override
  Future<DanmakuFetchImageResult> fetchDanmakuImage(
      String sessionId, String url) async {
    // FFI doesn't provide a safe image fetch API; do a minimal best-effort fetch here.
    final u = _normalizeHttpUrl(url);
    if (u == null)
      return const DanmakuFetchImageResult(mime: '', base64: '', width: 0);
    final resp = await _http.get(u);
    if (resp.statusCode < 200 || resp.statusCode >= 300) {
      return const DanmakuFetchImageResult(mime: '', base64: '', width: 0);
    }
    final mime = resp.headers['content-type'] ?? 'application/octet-stream';
    final b64 = base64Encode(resp.bodyBytes);
    return DanmakuFetchImageResult(mime: mime, base64: b64, width: 0);
  }

  Uri? _normalizeHttpUrl(String raw) {
    var s = raw.trim();
    if (s.isEmpty) return null;
    if (s.startsWith('//')) s = 'https:$s';
    final u = Uri.tryParse(s);
    if (u == null) return null;
    if (u.scheme != 'http' && u.scheme != 'https') return null;
    return u;
  }

  @override
  Future<NowPlayingSnapshot> nowPlayingSnapshot(
      NowPlayingSnapshotParams params) async {
    if (!Platform.isWindows) {
      return const NowPlayingSnapshot(
        supported: false,
        retrievedAtUnixMs: 0,
        pickedAppId: null,
        nowPlaying: null,
        sessions: [],
      );
    }

    final jsonStr =
        await _runner.call('nowPlaying.snapshot', params.toJson()) as String;
    return NowPlayingSnapshot.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<List<LyricsSearchResult>> lyricsSearch(
      LyricsSearchParams params) async {
    final servicesCsv =
        params.services.isEmpty ? null : params.services.join(',');
    final jsonStr = await _runner.call('lyrics.search', {
      'title': params.title,
      'album': params.album,
      'artist': params.artist,
      'durationMs': params.durationMs,
      'limit': params.limit,
      'strictMatch': params.strictMatch,
      'servicesCsv': servicesCsv,
      'timeoutMs': params.timeoutMs,
    }) as String;
    final raw = jsonDecode(jsonStr);
    final list = (raw as List)
        .whereType<Map>()
        .map((e) => LyricsSearchResult.fromJson(e.cast<String, dynamic>()))
        .toList(growable: false);
    return list;
  }

  @override
  Future<void> musicConfigSet(MusicProviderConfig cfg) async {
    await _runner
        .call('music.config.set', {'configJson': jsonEncode(cfg.toJson())});
  }

  Future<List<T>> _decodeList<T>(
      String jsonStr, T Function(Map<String, dynamic>) fromJson) async {
    final raw = jsonDecode(jsonStr);
    final list = (raw as List)
        .whereType<Map>()
        .map((e) => fromJson(e.cast<String, dynamic>()))
        .toList(growable: false);
    return list;
  }

  @override
  Future<List<MusicTrack>> searchTracks(MusicSearchParams p) async {
    final jsonStr = await _runner.call(
        'music.searchTracks', {'paramsJson': jsonEncode(p.toJson())}) as String;
    return _decodeList(jsonStr, MusicTrack.fromJson);
  }

  @override
  Future<List<MusicAlbum>> searchAlbums(MusicSearchParams p) async {
    final jsonStr = await _runner.call(
        'music.searchAlbums', {'paramsJson': jsonEncode(p.toJson())}) as String;
    return _decodeList(jsonStr, MusicAlbum.fromJson);
  }

  @override
  Future<List<MusicArtist>> searchArtists(MusicSearchParams p) async {
    final jsonStr = await _runner
            .call('music.searchArtists', {'paramsJson': jsonEncode(p.toJson())})
        as String;
    return _decodeList(jsonStr, MusicArtist.fromJson);
  }

  @override
  Future<List<MusicTrack>> albumTracks(MusicAlbumTracksParams p) async {
    final jsonStr = await _runner.call(
        'music.albumTracks', {'paramsJson': jsonEncode(p.toJson())}) as String;
    return _decodeList(jsonStr, MusicTrack.fromJson);
  }

  @override
  Future<List<MusicAlbum>> artistAlbums(MusicArtistAlbumsParams p) async {
    final jsonStr = await _runner.call(
        'music.artistAlbums', {'paramsJson': jsonEncode(p.toJson())}) as String;
    return _decodeList(jsonStr, MusicAlbum.fromJson);
  }

  @override
  Future<MusicTrackPlayUrlResult> trackPlayUrl(
      MusicTrackPlayUrlParams p) async {
    final jsonStr = await _runner.call(
        'music.trackPlayUrl', {'paramsJson': jsonEncode(p.toJson())}) as String;
    return MusicTrackPlayUrlResult.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicLoginQr> qqLoginQrCreate(String loginType) async {
    final jsonStr = await _runner
        .call('music.qq.loginQrCreate', {'loginType': loginType}) as String;
    return MusicLoginQr.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicLoginQrPollResult> qqLoginQrPoll(String sessionId) async {
    final jsonStr = await _runner
        .call('music.qq.loginQrPoll', {'sessionId': sessionId}) as String;
    return MusicLoginQrPollResult.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<QqMusicCookie> qqRefreshCookie(QqMusicCookie cookie) async {
    final jsonStr = await _runner.call('music.qq.refreshCookie',
        {'cookieJson': jsonEncode(cookie.toJson())}) as String;
    return QqMusicCookie.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicLoginQr> kugouLoginQrCreate(String loginType) async {
    final jsonStr = await _runner
        .call('music.kugou.loginQrCreate', {'loginType': loginType}) as String;
    return MusicLoginQr.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicLoginQrPollResult> kugouLoginQrPoll(String sessionId) async {
    final jsonStr = await _runner
        .call('music.kugou.loginQrPoll', {'sessionId': sessionId}) as String;
    return MusicLoginQrPollResult.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicDownloadStartResult> downloadStart(
      MusicDownloadStartParams p) async {
    final jsonStr = await _runner.call(
            'music.download.start', {'paramsJson': jsonEncode(p.toJson())})
        as String;
    return MusicDownloadStartResult.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicDownloadStatus> downloadStatus(String sessionId) async {
    final jsonStr = await _runner
        .call('music.download.status', {'sessionId': sessionId}) as String;
    return MusicDownloadStatus.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  @override
  Future<void> downloadCancel(String sessionId) async {
    await _runner.call('music.download.cancel', {'sessionId': sessionId});
  }

  @override
  Future<List<ThunderSubtitleItem>> subtitleSearch(
      SubtitleSearchParams p) async {
    final jsonStr = await _runner.call('subtitle.search', {
      'query': p.query,
      'limit': p.limit,
      'minScore': p.minScore,
      'lang': p.lang,
      'timeoutMs': p.timeoutMs,
    }) as String;
    final raw = jsonDecode(jsonStr);
    return (raw as List)
        .whereType<Map>()
        .map((e) => ThunderSubtitleItem.fromJson(e.cast<String, dynamic>()))
        .toList(growable: false);
  }

  @override
  Future<SubtitleDownloadReply> subtitleDownload(
      SubtitleDownloadParams p) async {
    // Ensure output folder exists (important on Android).
    final dir = Directory(p.outDir);
    if (!await dir.exists()) {
      await dir.create(recursive: true);
    }
    final jsonStr = await _runner.call('subtitle.download', {
      'itemJson': jsonEncode(p.item.toJson()),
      'outDir': p.outDir,
      'timeoutMs': p.timeoutMs,
      'retries': p.retries,
      'overwrite': p.overwrite,
    }) as String;
    return SubtitleDownloadReply.fromJson(
        (jsonDecode(jsonStr) as Map).cast<String, dynamic>());
  }

  // Convenience: recommended download dir for Android if caller doesn't provide one.
  static Future<String> defaultAndroidDownloadDir() async {
    // 首选：系统公共 Downloads/ChaosSeed（更符合用户预期）。
    // 但在 Android 10+ 的 Scoped Storage 下，直接写入公共 Downloads 可能失败；
    // 我们会探测可写性并自动回退到应用专用目录。
    final publicDownloads = await AndroidBridge.getPublicDownloadsDir();
    if (publicDownloads != null && publicDownloads.isNotEmpty) {
      final cand = p.join(publicDownloads, 'ChaosSeed');
      if (await _ensureWritableDir(cand)) return cand;
    }

    // 回退：应用专用 external files（无需额外权限，稳定可写）。
    final base = await getExternalStorageDirectory();
    final d = base ?? await getApplicationDocumentsDirectory();
    final fallback = p.join(d.path, 'Download', 'ChaosSeed');
    await Directory(fallback).create(recursive: true);
    return fallback;
  }

  static Future<bool> _ensureWritableDir(String dir) async {
    try {
      final d = Directory(dir);
      if (!await d.exists()) await d.create(recursive: true);
      final probe = File(p.join(dir, '.probe_write'));
      await probe.writeAsString('ok', flush: true);
      await probe.delete();
      return true;
    } catch (_) {
      return false;
    }
  }

  @override
  Future<void> dispose() async {
    for (final sid in _activeSessions.toList(growable: false)) {
      try {
        await closeLive(sid);
      } catch (_) {}
    }
    for (final c in _danmaku.values) {
      if (!c.isClosed) await c.close();
    }
    _danmaku.clear();
    _http.close();
    await _runner.dispose();
  }
}
