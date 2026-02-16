import 'dart:ffi';

import 'package:ffi/ffi.dart';

class ChaosFfiException implements Exception {
  final String message;
  final String? lastErrorJson;
  const ChaosFfiException(this.message, {this.lastErrorJson});

  @override
  String toString() => lastErrorJson == null
      ? 'ChaosFfiException: $message'
      : 'ChaosFfiException: $message (last_error=$lastErrorJson)';
}

class ChaosFfiBindings {
  ChaosFfiBindings(this._lib) {
    _apiVersion = _lib.lookupFunction<Uint32 Function(), int Function()>(
        'chaos_ffi_api_version');
    _lastErrorJson =
        _lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>(
            'chaos_ffi_last_error_json');
    _stringFree = _lib.lookupFunction<Void Function(Pointer<Void>),
        void Function(Pointer<Void>)>(
      'chaos_ffi_string_free',
    );

    _subtitleSearch = _lib.lookupFunction<
        Pointer<Utf8> Function(
            Pointer<Utf8>, Uint32, Double, Pointer<Utf8>, Uint32),
        Pointer<Utf8> Function(Pointer<Utf8>, int, double, Pointer<Utf8>,
            int)>('chaos_subtitle_search_json');

    _subtitleDownload = _lib.lookupFunction<
        Pointer<Utf8> Function(
            Pointer<Utf8>, Pointer<Utf8>, Uint32, Uint32, Uint8),
        Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, int, int,
            int)>('chaos_subtitle_download_item_json');

    _liveDecodeManifest = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>, Uint8),
        Pointer<Utf8> Function(
            Pointer<Utf8>, int)>('chaos_livestream_decode_manifest_json');

    _resolveVariant2 = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>,
            Pointer<Utf8>)>('chaos_livestream_resolve_variant2_json');

    _liveDirCategories = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_live_dir_categories_json',
    );
    _liveDirRecommend = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>, Uint32),
        Pointer<Utf8> Function(Pointer<Utf8>, int)>(
      'chaos_live_dir_recommend_rooms_json',
    );
    _liveDirCategoryRooms = _lib.lookupFunction<
        Pointer<Utf8> Function(
            Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>, Uint32),
        Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>,
            int)>('chaos_live_dir_category_rooms_json');
    _liveDirSearch = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Uint32),
        Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>,
            int)>('chaos_live_dir_search_rooms_json');

    _danmakuConnect = _lib.lookupFunction<Pointer<Void> Function(Pointer<Utf8>),
        Pointer<Void> Function(Pointer<Utf8>)>('chaos_danmaku_connect');
    _danmakuPoll = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Void>, Uint32),
        Pointer<Utf8> Function(Pointer<Void>, int)>(
      'chaos_danmaku_poll_json',
    );
    _danmakuDisconnect = _lib.lookupFunction<Int32 Function(Pointer<Void>),
        int Function(Pointer<Void>)>('chaos_danmaku_disconnect');

    // Lyrics / now playing
    _nowPlayingSnapshot = _lib.lookupFunction<
        Pointer<Utf8> Function(Uint8, Uint32, Uint32),
        Pointer<Utf8> Function(
            int, int, int)>('chaos_now_playing_snapshot_json');
    _lyricsSearch = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>,
            Uint32, Uint32, Uint8, Pointer<Utf8>, Uint32),
        Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>, int,
            int, int, Pointer<Utf8>, int)>('chaos_lyrics_search_json');

    // Music
    _musicConfigSet = _lib.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_config_set_json',
    );
    _musicSearchTracks = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_search_tracks_json',
    );
    _musicSearchAlbums = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_search_albums_json',
    );
    _musicSearchArtists = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_search_artists_json',
    );
    _musicAlbumTracks = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_album_tracks_json',
    );
    _musicArtistAlbums = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_artist_albums_json',
    );
    _musicTrackPlayUrl = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_track_play_url_json',
    );
    _musicQqLoginQrCreate = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_qq_login_qr_create_json',
    );
    _musicQqLoginQrPoll = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_qq_login_qr_poll_json',
    );
    _musicQqRefreshCookie = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_qq_refresh_cookie_json',
    );
    _musicKugouLoginQrCreate = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_kugou_login_qr_create_json',
    );
    _musicKugouLoginQrPoll = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_kugou_login_qr_poll_json',
    );
    _musicDownloadStart = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_download_start_json',
    );
    _musicDownloadStatus = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_download_status_json',
    );
    _musicDownloadCancel = _lib.lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)>(
      'chaos_music_download_cancel_json',
    );
  }

  final DynamicLibrary _lib;

  late final int Function() _apiVersion;
  late final Pointer<Utf8> Function() _lastErrorJson;
  late final void Function(Pointer<Void>) _stringFree;

  late final Pointer<Utf8> Function(
      Pointer<Utf8>, int, double, Pointer<Utf8>, int) _subtitleSearch;
  late final Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, int, int, int)
      _subtitleDownload;

  late final Pointer<Utf8> Function(Pointer<Utf8>, int) _liveDecodeManifest;
  late final Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>)
      _resolveVariant2;

  late final Pointer<Utf8> Function(Pointer<Utf8>) _liveDirCategories;
  late final Pointer<Utf8> Function(Pointer<Utf8>, int) _liveDirRecommend;
  late final Pointer<Utf8> Function(
      Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>, int) _liveDirCategoryRooms;
  late final Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, int)
      _liveDirSearch;

  late final Pointer<Void> Function(Pointer<Utf8>) _danmakuConnect;
  late final Pointer<Utf8> Function(Pointer<Void>, int) _danmakuPoll;
  late final int Function(Pointer<Void>) _danmakuDisconnect;

  late final Pointer<Utf8> Function(int, int, int) _nowPlayingSnapshot;
  late final Pointer<Utf8> Function(
    Pointer<Utf8>,
    Pointer<Utf8>,
    Pointer<Utf8>,
    int,
    int,
    int,
    Pointer<Utf8>,
    int,
  ) _lyricsSearch;

  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicConfigSet;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicSearchTracks;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicSearchAlbums;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicSearchArtists;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicAlbumTracks;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicArtistAlbums;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicTrackPlayUrl;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicQqLoginQrCreate;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicQqLoginQrPoll;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicQqRefreshCookie;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicKugouLoginQrCreate;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicKugouLoginQrPoll;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicDownloadStart;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicDownloadStatus;
  late final Pointer<Utf8> Function(Pointer<Utf8>) _musicDownloadCancel;

  int apiVersion() => _apiVersion();

  String? takeLastErrorJson() {
    final p = _lastErrorJson();
    if (p == nullptr) return null;
    final s = p.toDartString();
    _stringFree(p.cast<Void>());
    return s;
  }

  String _takeStringOrThrow(Pointer<Utf8> p, {required String op}) {
    if (p == nullptr) {
      throw ChaosFfiException('$op returned null',
          lastErrorJson: takeLastErrorJson());
    }
    final s = p.toDartString();
    _stringFree(p.cast<Void>());
    if (s.trim().isEmpty) {
      throw ChaosFfiException('$op returned empty string',
          lastErrorJson: takeLastErrorJson());
    }
    return s;
  }

  String subtitleSearchJson({
    required String query,
    required int limit,
    required double? minScore,
    required String? lang,
    required int timeoutMs,
  }) {
    final q = query.toNativeUtf8();
    final l = (lang == null) ? nullptr : lang.toNativeUtf8();
    try {
      final p = _subtitleSearch(q, limit, minScore ?? -1.0, l, timeoutMs);
      return _takeStringOrThrow(p, op: 'chaos_subtitle_search_json');
    } finally {
      malloc.free(q);
      if (l != nullptr) malloc.free(l);
    }
  }

  String subtitleDownloadJson({
    required String itemJson,
    required String outDir,
    required int timeoutMs,
    required int retries,
    required bool overwrite,
  }) {
    final item = itemJson.toNativeUtf8();
    final dir = outDir.toNativeUtf8();
    try {
      final p =
          _subtitleDownload(item, dir, timeoutMs, retries, overwrite ? 1 : 0);
      return _takeStringOrThrow(p, op: 'chaos_subtitle_download_item_json');
    } finally {
      malloc.free(item);
      malloc.free(dir);
    }
  }

  String liveDecodeManifestJson(
      {required String input, bool dropInaccessibleHighQualities = true}) {
    final s = input.toNativeUtf8();
    try {
      final p = _liveDecodeManifest(s, dropInaccessibleHighQualities ? 1 : 0);
      return _takeStringOrThrow(p, op: 'chaos_livestream_decode_manifest_json');
    } finally {
      malloc.free(s);
    }
  }

  String resolveVariant2Json({
    required String site,
    required String roomId,
    required String variantId,
  }) {
    final a = site.toNativeUtf8();
    final b = roomId.toNativeUtf8();
    final c = variantId.toNativeUtf8();
    try {
      final p = _resolveVariant2(a, b, c);
      return _takeStringOrThrow(p,
          op: 'chaos_livestream_resolve_variant2_json');
    } finally {
      malloc.free(a);
      malloc.free(b);
      malloc.free(c);
    }
  }

  String liveDirCategoriesJson(String site) {
    final s = site.toNativeUtf8();
    try {
      final p = _liveDirCategories(s);
      return _takeStringOrThrow(p, op: 'chaos_live_dir_categories_json');
    } finally {
      malloc.free(s);
    }
  }

  String liveDirRecommendRoomsJson(String site, int page) {
    final s = site.toNativeUtf8();
    try {
      final p = _liveDirRecommend(s, page);
      return _takeStringOrThrow(p, op: 'chaos_live_dir_recommend_rooms_json');
    } finally {
      malloc.free(s);
    }
  }

  String liveDirCategoryRoomsJson(
      String site, String? parentId, String categoryId, int page) {
    final s = site.toNativeUtf8();
    final p0 = (parentId == null) ? nullptr : parentId.toNativeUtf8();
    final c = categoryId.toNativeUtf8();
    try {
      final p = _liveDirCategoryRooms(s, p0, c, page);
      return _takeStringOrThrow(p, op: 'chaos_live_dir_category_rooms_json');
    } finally {
      malloc.free(s);
      if (p0 != nullptr) malloc.free(p0);
      malloc.free(c);
    }
  }

  String liveDirSearchRoomsJson(String site, String keyword, int page) {
    final s = site.toNativeUtf8();
    final k = keyword.toNativeUtf8();
    try {
      final p = _liveDirSearch(s, k, page);
      return _takeStringOrThrow(p, op: 'chaos_live_dir_search_rooms_json');
    } finally {
      malloc.free(s);
      malloc.free(k);
    }
  }

  Pointer<Void> danmakuConnect(String input) {
    final s = input.toNativeUtf8();
    try {
      final h = _danmakuConnect(s);
      if (h == nullptr) {
        throw ChaosFfiException('chaos_danmaku_connect failed',
            lastErrorJson: takeLastErrorJson());
      }
      return h;
    } finally {
      malloc.free(s);
    }
  }

  String danmakuPollJson(Pointer<Void> handle, int maxEvents) {
    final p = _danmakuPoll(handle, maxEvents);
    // Poll may return empty array '[]'; treat as OK.
    if (p == nullptr) {
      throw ChaosFfiException('chaos_danmaku_poll_json failed',
          lastErrorJson: takeLastErrorJson());
    }
    final s = p.toDartString();
    _stringFree(p.cast<Void>());
    return s;
  }

  void danmakuDisconnect(Pointer<Void> handle) {
    final rc = _danmakuDisconnect(handle);
    if (rc != 0) {
      throw ChaosFfiException('chaos_danmaku_disconnect failed',
          lastErrorJson: takeLastErrorJson());
    }
  }

  String nowPlayingSnapshotJson({
    required bool includeThumbnail,
    required int maxThumbnailBytes,
    required int maxSessions,
  }) {
    final p = _nowPlayingSnapshot(
        includeThumbnail ? 1 : 0, maxThumbnailBytes, maxSessions);
    return _takeStringOrThrow(p, op: 'chaos_now_playing_snapshot_json');
  }

  String lyricsSearchJson({
    required String title,
    required String? album,
    required String? artist,
    required int durationMs,
    required int limit,
    required bool strictMatch,
    required String? servicesCsv,
    required int timeoutMs,
  }) {
    final t = title.toNativeUtf8();
    final a = (album == null) ? nullptr : album.toNativeUtf8();
    final ar = (artist == null) ? nullptr : artist.toNativeUtf8();
    final csv = (servicesCsv == null) ? nullptr : servicesCsv.toNativeUtf8();
    try {
      final p = _lyricsSearch(
          t, a, ar, durationMs, limit, strictMatch ? 1 : 0, csv, timeoutMs);
      return _takeStringOrThrow(p, op: 'chaos_lyrics_search_json');
    } finally {
      malloc.free(t);
      if (a != nullptr) malloc.free(a);
      if (ar != nullptr) malloc.free(ar);
      if (csv != nullptr) malloc.free(csv);
    }
  }

  String musicConfigSetJson(String configJson) {
    final s = configJson.toNativeUtf8();
    try {
      final p = _musicConfigSet(s);
      return _takeStringOrThrow(p, op: 'chaos_music_config_set_json');
    } finally {
      malloc.free(s);
    }
  }

  String _musicCallJson(
      Pointer<Utf8> Function(Pointer<Utf8>) fn, String json, String op) {
    final s = json.toNativeUtf8();
    try {
      final p = fn(s);
      return _takeStringOrThrow(p, op: op);
    } finally {
      malloc.free(s);
    }
  }

  String musicSearchTracksJson(String paramsJson) => _musicCallJson(
      _musicSearchTracks, paramsJson, 'chaos_music_search_tracks_json');
  String musicSearchAlbumsJson(String paramsJson) => _musicCallJson(
      _musicSearchAlbums, paramsJson, 'chaos_music_search_albums_json');
  String musicSearchArtistsJson(String paramsJson) => _musicCallJson(
      _musicSearchArtists, paramsJson, 'chaos_music_search_artists_json');
  String musicAlbumTracksJson(String paramsJson) => _musicCallJson(
      _musicAlbumTracks, paramsJson, 'chaos_music_album_tracks_json');
  String musicArtistAlbumsJson(String paramsJson) => _musicCallJson(
      _musicArtistAlbums, paramsJson, 'chaos_music_artist_albums_json');
  String musicTrackPlayUrlJson(String paramsJson) => _musicCallJson(
      _musicTrackPlayUrl, paramsJson, 'chaos_music_track_play_url_json');

  String musicQqLoginQrCreateJson(String loginType) {
    final s = loginType.toNativeUtf8();
    try {
      final p = _musicQqLoginQrCreate(s);
      return _takeStringOrThrow(p, op: 'chaos_music_qq_login_qr_create_json');
    } finally {
      malloc.free(s);
    }
  }

  String musicQqLoginQrPollJson(String sessionId) => _musicCallJson(
      _musicQqLoginQrPoll, sessionId, 'chaos_music_qq_login_qr_poll_json');

  String musicQqRefreshCookieJson(String cookieJson) => _musicCallJson(
      _musicQqRefreshCookie, cookieJson, 'chaos_music_qq_refresh_cookie_json');

  String musicKugouLoginQrCreateJson(String loginType) {
    final s = loginType.toNativeUtf8();
    try {
      final p = _musicKugouLoginQrCreate(s);
      return _takeStringOrThrow(p,
          op: 'chaos_music_kugou_login_qr_create_json');
    } finally {
      malloc.free(s);
    }
  }

  String musicKugouLoginQrPollJson(String sessionId) => _musicCallJson(
      _musicKugouLoginQrPoll,
      sessionId,
      'chaos_music_kugou_login_qr_poll_json');

  String musicDownloadStartJson(String paramsJson) => _musicCallJson(
      _musicDownloadStart, paramsJson, 'chaos_music_download_start_json');
  String musicDownloadStatusJson(String sessionId) => _musicCallJson(
      _musicDownloadStatus, sessionId, 'chaos_music_download_status_json');
  String musicDownloadCancelJson(String sessionId) => _musicCallJson(
      _musicDownloadCancel, sessionId, 'chaos_music_download_cancel_json');
}
