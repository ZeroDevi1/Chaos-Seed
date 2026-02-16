import 'dart:io';

import '../daemon/json_rpc_lsp_client.dart';
import '../settings/settings_controller.dart';
import '../settings/settings_model.dart';
import '../models/danmaku.dart';
import '../models/live.dart';
import '../models/live_directory.dart';
import '../models/lyrics.dart';
import '../models/music.dart';
import '../models/now_playing.dart';
import '../models/subtitles.dart';
import 'chaos_backend.dart';
import 'daemon_backend.dart';
import 'ffi_backend.dart';

class HybridChaosBackend implements ChaosBackend {
  HybridChaosBackend(this._settings) : _ffi = FfiChaosBackend();

  final SettingsController _settings;
  final FfiChaosBackend _ffi;

  DaemonChaosBackend? _daemon;
  Object? _daemonInitError;

  @override
  String get name => _daemon != null ? 'hybrid(daemon+ffi)' : 'hybrid(ffi)';

  bool get _daemonSupported => Platform.isWindows;

  Future<DaemonChaosBackend?> _ensureDaemon() async {
    if (!_daemonSupported) return null;
    if (_daemon != null) return _daemon;
    if (_daemonInitError != null) return null;

    try {
      final exe = _findBinary('chaos-daemon.exe');
      final rpc = JsonRpcLspClient(
        executable: exe,
        args: const [],
        authToken: _randomToken(),
      );
      _daemon = DaemonChaosBackend(rpc);
      return _daemon;
    } catch (e) {
      _daemonInitError = e;
      return null;
    }
  }

  Future<ChaosBackend> _pick(BackendMode mode) async {
    if (!_daemonSupported) return _ffi;

    switch (mode) {
      case BackendMode.ffi:
        return _ffi;
      case BackendMode.daemon:
        return (await _ensureDaemon()) ?? _ffi;
      case BackendMode.auto:
        return (await _ensureDaemon()) ?? _ffi;
    }
  }

  bool _isFfiSession(String sessionId) => sessionId.startsWith('ffi-');

  @override
  Future<List<LiveDirCategory>> categories(String site) async {
    final b = await _pick(_settings.liveBackendMode);
    return b.categories(site);
  }

  @override
  Future<LiveDirRoomListResult> recommendRooms(String site, int page) async {
    final b = await _pick(_settings.liveBackendMode);
    return b.recommendRooms(site, page);
  }

  @override
  Future<LiveDirRoomListResult> categoryRooms(
    String site,
    String? parentId,
    String categoryId,
    int page,
  ) async {
    final b = await _pick(_settings.liveBackendMode);
    return b.categoryRooms(site, parentId, categoryId, page);
  }

  @override
  Future<LiveDirRoomListResult> searchRooms(
      String site, String keyword, int page) async {
    final b = await _pick(_settings.liveBackendMode);
    return b.searchRooms(site, keyword, page);
  }

  @override
  Future<LivestreamDecodeManifestResult> decodeManifest(String input) async {
    final b = await _pick(_settings.liveBackendMode);
    return b.decodeManifest(input);
  }

  @override
  Future<LiveOpenResult> openLive(String input, {String? variantId}) async {
    final b = await _pick(_settings.liveBackendMode);
    return b.openLive(input, variantId: variantId);
  }

  @override
  Future<void> closeLive(String sessionId) async {
    final b = _isFfiSession(sessionId) ? _ffi : (await _ensureDaemon()) ?? _ffi;
    return b.closeLive(sessionId);
  }

  @override
  Stream<DanmakuMessage> danmakuStream(String sessionId) {
    if (_isFfiSession(sessionId)) {
      return _ffi.danmakuStream(sessionId);
    }
    return (_daemon ?? _ffi).danmakuStream(sessionId);
  }

  @override
  Future<DanmakuFetchImageResult> fetchDanmakuImage(
      String sessionId, String url) async {
    final b = _isFfiSession(sessionId) ? _ffi : (await _ensureDaemon()) ?? _ffi;
    return b.fetchDanmakuImage(sessionId, url);
  }

  @override
  Future<NowPlayingSnapshot> nowPlayingSnapshot(
      NowPlayingSnapshotParams params) async {
    final b = await _pick(_settings.lyricsBackendMode);
    return b.nowPlayingSnapshot(params);
  }

  @override
  Future<List<LyricsSearchResult>> lyricsSearch(
      LyricsSearchParams params) async {
    final b = await _pick(_settings.lyricsBackendMode);
    return b.lyricsSearch(params);
  }

  @override
  Future<void> musicConfigSet(MusicProviderConfig cfg) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.musicConfigSet(cfg);
  }

  @override
  Future<List<MusicTrack>> searchTracks(MusicSearchParams p) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.searchTracks(p);
  }

  @override
  Future<List<MusicAlbum>> searchAlbums(MusicSearchParams p) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.searchAlbums(p);
  }

  @override
  Future<List<MusicArtist>> searchArtists(MusicSearchParams p) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.searchArtists(p);
  }

  @override
  Future<List<MusicTrack>> albumTracks(MusicAlbumTracksParams p) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.albumTracks(p);
  }

  @override
  Future<List<MusicAlbum>> artistAlbums(MusicArtistAlbumsParams p) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.artistAlbums(p);
  }

  @override
  Future<MusicTrackPlayUrlResult> trackPlayUrl(
      MusicTrackPlayUrlParams p) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.trackPlayUrl(p);
  }

  @override
  Future<MusicLoginQr> qqLoginQrCreate(String loginType) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.qqLoginQrCreate(loginType);
  }

  @override
  Future<MusicLoginQrPollResult> qqLoginQrPoll(String sessionId) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.qqLoginQrPoll(sessionId);
  }

  @override
  Future<QqMusicCookie> qqRefreshCookie(QqMusicCookie cookie) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.qqRefreshCookie(cookie);
  }

  @override
  Future<MusicLoginQr> kugouLoginQrCreate(String loginType) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.kugouLoginQrCreate(loginType);
  }

  @override
  Future<MusicLoginQrPollResult> kugouLoginQrPoll(String sessionId) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.kugouLoginQrPoll(sessionId);
  }

  @override
  Future<MusicDownloadStartResult> downloadStart(
      MusicDownloadStartParams p) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.downloadStart(p);
  }

  @override
  Future<MusicDownloadStatus> downloadStatus(String sessionId) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.downloadStatus(sessionId);
  }

  @override
  Future<void> downloadCancel(String sessionId) async {
    final b = await _pick(_settings.musicBackendMode);
    return b.downloadCancel(sessionId);
  }

  @override
  Future<List<ThunderSubtitleItem>> subtitleSearch(
      SubtitleSearchParams p) async {
    // Subtitles exist only in chaos-ffi currently.
    return _ffi.subtitleSearch(p);
  }

  @override
  Future<SubtitleDownloadReply> subtitleDownload(
      SubtitleDownloadParams p) async {
    return _ffi.subtitleDownload(p);
  }

  @override
  Future<void> dispose() async {
    await _daemon?.dispose();
    await _ffi.dispose();
  }

  static String _randomToken() {
    return DateTime.now().microsecondsSinceEpoch.toRadixString(16) +
        '-' +
        (DateTime.now().millisecondsSinceEpoch ^ 0xabcdef).toRadixString(16);
  }

  static String _findBinary(String name) {
    final exeDir = File(Platform.resolvedExecutable).parent;
    final candidates = <String>[
      File(Platform.resolvedExecutable).parent.uri.resolve(name).toFilePath(),
      File(exeDir.path + Platform.pathSeparator + name).path,
      Directory.current.uri.resolve(name).toFilePath(),
      Directory.current.uri.resolve('windows/deps/$name').toFilePath(),
      Directory.current.uri
          .resolve('chaos-flutter/windows/deps/$name')
          .toFilePath(),
    ];
    for (final c in candidates) {
      if (File(c).existsSync()) return c;
    }
    throw FileSystemException(
        'missing $name (searched: ${candidates.join(', ')})');
  }
}
