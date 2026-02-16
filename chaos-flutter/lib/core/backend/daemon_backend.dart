import 'dart:async';

import '../daemon/json_rpc_lsp_client.dart';
import '../models/danmaku.dart';
import '../models/live.dart';
import '../models/live_directory.dart';
import '../models/lyrics.dart';
import '../models/music.dart';
import '../models/now_playing.dart';
import '../models/subtitles.dart';
import 'chaos_backend.dart';

class DaemonChaosBackend implements ChaosBackend {
  DaemonChaosBackend(this._rpc);

  final JsonRpcLspClient _rpc;

  final _danmakuCtrls = <String, StreamController<DanmakuMessage>>{};

  bool _started = false;

  @override
  String get name => 'daemon';

  Future<void> _ensureStarted() async {
    if (_started) return;
    _started = true;
    await _rpc.start();
    _rpc.notifications.listen((n) {
      if (n.method != 'danmaku.message') return;
      final params = n.params;
      if (params is! Map) return;
      final msg = DanmakuMessage.fromDaemonJson(params.cast<String, dynamic>());
      final c = _danmakuCtrls[msg.sessionId];
      if (c != null && !c.isClosed) c.add(msg);
    });
  }

  @override
  Future<List<LiveDirCategory>> categories(String site) async {
    await _ensureStarted();
    final res = await _rpc.invoke('liveDir.categories', {'site': site});
    if (res is! List) return const [];
    return res
        .whereType<Map>()
        .map((e) => LiveDirCategory.fromJson(e.cast<String, dynamic>()))
        .toList(growable: false);
  }

  @override
  Future<LiveDirRoomListResult> recommendRooms(String site, int page) async {
    await _ensureStarted();
    final res = await _rpc
        .invoke('liveDir.recommendRooms', {'site': site, 'page': page});
    return LiveDirRoomListResult.fromJson((res as Map).cast<String, dynamic>());
  }

  @override
  Future<LiveDirRoomListResult> categoryRooms(
    String site,
    String? parentId,
    String categoryId,
    int page,
  ) async {
    await _ensureStarted();
    final res = await _rpc.invoke('liveDir.categoryRooms', {
      'site': site,
      'parentId': parentId,
      'categoryId': categoryId,
      'page': page,
    });
    return LiveDirRoomListResult.fromJson((res as Map).cast<String, dynamic>());
  }

  @override
  Future<LiveDirRoomListResult> searchRooms(
      String site, String keyword, int page) async {
    await _ensureStarted();
    final res = await _rpc.invoke('liveDir.searchRooms',
        {'site': site, 'keyword': keyword, 'page': page});
    return LiveDirRoomListResult.fromJson((res as Map).cast<String, dynamic>());
  }

  @override
  Future<LivestreamDecodeManifestResult> decodeManifest(String input) async {
    await _ensureStarted();
    final res =
        await _rpc.invoke('livestream.decodeManifest', {'input': input});
    return LivestreamDecodeManifestResult.fromJson(
        (res as Map).cast<String, dynamic>());
  }

  @override
  Future<LiveOpenResult> openLive(String input, {String? variantId}) async {
    await _ensureStarted();
    final payload = <String, dynamic>{
      'input': input,
      'preferredQuality': 'highest',
    };
    if (variantId != null && variantId.trim().isNotEmpty) {
      payload['variantId'] = variantId.trim();
    }
    final res = await _rpc.invoke('live.open', payload);
    return LiveOpenResult.fromJson((res as Map).cast<String, dynamic>());
  }

  @override
  Future<void> closeLive(String sessionId) async {
    await _ensureStarted();
    await _rpc.invoke('live.close', {'sessionId': sessionId});
    final c = _danmakuCtrls.remove(sessionId);
    if (c != null && !c.isClosed) await c.close();
  }

  @override
  Stream<DanmakuMessage> danmakuStream(String sessionId) {
    return _danmakuCtrls
        .putIfAbsent(
          sessionId,
          () => StreamController<DanmakuMessage>.broadcast(),
        )
        .stream;
  }

  @override
  Future<DanmakuFetchImageResult> fetchDanmakuImage(
      String sessionId, String url) async {
    await _ensureStarted();
    final res = await _rpc
        .invoke('danmaku.fetchImage', {'sessionId': sessionId, 'url': url});
    return DanmakuFetchImageResult.fromJson(
        (res as Map).cast<String, dynamic>());
  }

  @override
  Future<NowPlayingSnapshot> nowPlayingSnapshot(
      NowPlayingSnapshotParams params) async {
    await _ensureStarted();
    final res = await _rpc.invoke('nowPlaying.snapshot', params.toJson());
    return NowPlayingSnapshot.fromJson((res as Map).cast<String, dynamic>());
  }

  @override
  Future<List<LyricsSearchResult>> lyricsSearch(
      LyricsSearchParams params) async {
    await _ensureStarted();
    final res = await _rpc.invoke('lyrics.search', params.toDaemonJson());
    if (res is! List) return const [];
    return res
        .whereType<Map>()
        .map((e) => LyricsSearchResult.fromJson(e.cast<String, dynamic>()))
        .toList(growable: false);
  }

  @override
  Future<void> musicConfigSet(MusicProviderConfig cfg) async {
    await _ensureStarted();
    await _rpc.invoke('music.config.set', cfg.toJson());
  }

  Future<List<T>> _listCall<T>(String method, Map<String, dynamic> params,
      T Function(Map<String, dynamic>) fromJson) async {
    await _ensureStarted();
    final res = await _rpc.invoke(method, params);
    if (res is! List) return const [];
    return res
        .whereType<Map>()
        .map((e) => fromJson(e.cast<String, dynamic>()))
        .toList(growable: false);
  }

  @override
  Future<List<MusicTrack>> searchTracks(MusicSearchParams p) =>
      _listCall('music.searchTracks', p.toJson(), MusicTrack.fromJson);
  @override
  Future<List<MusicAlbum>> searchAlbums(MusicSearchParams p) =>
      _listCall('music.searchAlbums', p.toJson(), MusicAlbum.fromJson);
  @override
  Future<List<MusicArtist>> searchArtists(MusicSearchParams p) =>
      _listCall('music.searchArtists', p.toJson(), MusicArtist.fromJson);
  @override
  Future<List<MusicTrack>> albumTracks(MusicAlbumTracksParams p) =>
      _listCall('music.albumTracks', p.toJson(), MusicTrack.fromJson);
  @override
  Future<List<MusicAlbum>> artistAlbums(MusicArtistAlbumsParams p) =>
      _listCall('music.artistAlbums', p.toJson(), MusicAlbum.fromJson);

  @override
  Future<MusicTrackPlayUrlResult> trackPlayUrl(
      MusicTrackPlayUrlParams p) async {
    await _ensureStarted();
    final res = await _rpc.invoke('music.trackPlayUrl', p.toJson());
    return MusicTrackPlayUrlResult.fromJson(
        (res as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicLoginQr> qqLoginQrCreate(String loginType) async {
    await _ensureStarted();
    final res =
        await _rpc.invoke('music.qq.loginQrCreate', {'loginType': loginType});
    return MusicLoginQr.fromJson((res as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicLoginQrPollResult> qqLoginQrPoll(String sessionId) async {
    await _ensureStarted();
    final res =
        await _rpc.invoke('music.qq.loginQrPoll', {'sessionId': sessionId});
    return MusicLoginQrPollResult.fromJson(
        (res as Map).cast<String, dynamic>());
  }

  @override
  Future<QqMusicCookie> qqRefreshCookie(QqMusicCookie cookie) async {
    await _ensureStarted();
    final res = await _rpc
        .invoke('music.qq.refreshCookie', {'cookie': cookie.toJson()});
    return QqMusicCookie.fromJson((res as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicLoginQr> kugouLoginQrCreate(String loginType) async {
    await _ensureStarted();
    final res = await _rpc
        .invoke('music.kugou.loginQrCreate', {'loginType': loginType});
    return MusicLoginQr.fromJson((res as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicLoginQrPollResult> kugouLoginQrPoll(String sessionId) async {
    await _ensureStarted();
    final res =
        await _rpc.invoke('music.kugou.loginQrPoll', {'sessionId': sessionId});
    return MusicLoginQrPollResult.fromJson(
        (res as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicDownloadStartResult> downloadStart(
      MusicDownloadStartParams p) async {
    await _ensureStarted();
    final res = await _rpc.invoke('music.download.start', p.toJson());
    return MusicDownloadStartResult.fromJson(
        (res as Map).cast<String, dynamic>());
  }

  @override
  Future<MusicDownloadStatus> downloadStatus(String sessionId) async {
    await _ensureStarted();
    final res =
        await _rpc.invoke('music.download.status', {'sessionId': sessionId});
    return MusicDownloadStatus.fromJson((res as Map).cast<String, dynamic>());
  }

  @override
  Future<void> downloadCancel(String sessionId) async {
    await _ensureStarted();
    await _rpc.invoke('music.download.cancel', {'sessionId': sessionId});
  }

  @override
  Future<List<ThunderSubtitleItem>> subtitleSearch(
      SubtitleSearchParams p) async {
    // daemon doesn't expose subtitle APIs yet (only exists in FFI). Keep as unsupported for now.
    return const [];
  }

  @override
  Future<SubtitleDownloadReply> subtitleDownload(
      SubtitleDownloadParams p) async {
    throw UnsupportedError(
        'subtitle download is not supported via daemon transport');
  }

  @override
  Future<void> dispose() async {
    for (final c in _danmakuCtrls.values) {
      if (!c.isClosed) await c.close();
    }
    _danmakuCtrls.clear();
    await _rpc.dispose();
  }
}
