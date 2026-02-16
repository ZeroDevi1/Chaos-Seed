import 'json_utils.dart';

enum MusicService {
  qq,
  kugou,
  netease,
  kuwo,
}

MusicService musicServiceFromJson(dynamic v) {
  if (v is String) {
    for (final s in MusicService.values) {
      if (s.name == v) return s;
    }
  }
  return MusicService.qq;
}

class MusicQuality {
  final String id;
  final String label;
  final String format;
  final int? bitrateKbps;
  final bool lossless;

  const MusicQuality({
    required this.id,
    required this.label,
    required this.format,
    required this.bitrateKbps,
    required this.lossless,
  });

  factory MusicQuality.fromJson(Map<String, dynamic> json) {
    return MusicQuality(
      id: pickString(json, ['id']),
      label: pickString(json, ['label']),
      format: pickString(json, ['format']),
      bitrateKbps: pick<int>(json, ['bitrateKbps', 'bitrate_kbps']),
      lossless: pickBool(json, ['lossless']),
    );
  }

  Map<String, dynamic> toJson() => {
        'id': id,
        'label': label,
        'format': format,
        'bitrateKbps': bitrateKbps,
        'lossless': lossless,
      };
}

class MusicTrack {
  final MusicService service;
  final String id;
  final String title;
  final List<String> artists;
  final List<String> artistIds;
  final String? album;
  final String? albumId;
  final int? durationMs;
  final String? coverUrl;
  final List<MusicQuality> qualities;

  const MusicTrack({
    required this.service,
    required this.id,
    required this.title,
    required this.artists,
    required this.artistIds,
    required this.album,
    required this.albumId,
    required this.durationMs,
    required this.coverUrl,
    required this.qualities,
  });

  factory MusicTrack.fromJson(Map<String, dynamic> json) {
    final rawArtists = pickList(json, ['artists']) ?? const [];
    final rawArtistIds =
        pickList(json, ['artistIds', 'artist_ids']) ?? const [];
    final rawQualities = pickList(json, ['qualities']) ?? const [];
    return MusicTrack(
      service: musicServiceFromJson(pick<dynamic>(json, ['service'])),
      id: pickString(json, ['id']),
      title: pickString(json, ['title']),
      artists: rawArtists.whereType<String>().toList(growable: false),
      artistIds: rawArtistIds.whereType<String>().toList(growable: false),
      album: pick<String>(json, ['album']),
      albumId: pick<String>(json, ['albumId', 'album_id']),
      durationMs: pick<int>(json, ['durationMs', 'duration_ms']),
      coverUrl: pick<String>(json, ['coverUrl', 'cover_url']),
      qualities: rawQualities
          .whereType<Map>()
          .map((e) => MusicQuality.fromJson(e.cast<String, dynamic>()))
          .toList(growable: false),
    );
  }

  Map<String, dynamic> toJson() => {
        'service': service.name,
        'id': id,
        'title': title,
        'artists': artists,
        'artistIds': artistIds,
        'album': album,
        'albumId': albumId,
        'durationMs': durationMs,
        'coverUrl': coverUrl,
        'qualities': qualities.map((q) => q.toJson()).toList(growable: false),
      };
}

class MusicAlbum {
  final MusicService service;
  final String id;
  final String title;
  final String? artist;
  final String? artistId;
  final String? coverUrl;
  final String? publishTime;
  final int? trackCount;

  const MusicAlbum({
    required this.service,
    required this.id,
    required this.title,
    required this.artist,
    required this.artistId,
    required this.coverUrl,
    required this.publishTime,
    required this.trackCount,
  });

  factory MusicAlbum.fromJson(Map<String, dynamic> json) {
    return MusicAlbum(
      service: musicServiceFromJson(pick<dynamic>(json, ['service'])),
      id: pickString(json, ['id']),
      title: pickString(json, ['title']),
      artist: pick<String>(json, ['artist']),
      artistId: pick<String>(json, ['artistId', 'artist_id']),
      coverUrl: pick<String>(json, ['coverUrl', 'cover_url']),
      publishTime: pick<String>(json, ['publishTime', 'publish_time']),
      trackCount: pick<int>(json, ['trackCount', 'track_count']),
    );
  }
}

class MusicArtist {
  final MusicService service;
  final String id;
  final String name;
  final String? coverUrl;
  final int? albumCount;

  const MusicArtist({
    required this.service,
    required this.id,
    required this.name,
    required this.coverUrl,
    required this.albumCount,
  });

  factory MusicArtist.fromJson(Map<String, dynamic> json) {
    return MusicArtist(
      service: musicServiceFromJson(pick<dynamic>(json, ['service'])),
      id: pickString(json, ['id']),
      name: pickString(json, ['name']),
      coverUrl: pick<String>(json, ['coverUrl', 'cover_url']),
      albumCount: pick<int>(json, ['albumCount', 'album_count']),
    );
  }
}

class MusicProviderConfig {
  final String? kugouBaseUrl;
  final List<String> neteaseBaseUrls;
  final String? neteaseAnonymousCookieUrl;

  const MusicProviderConfig({
    required this.kugouBaseUrl,
    required this.neteaseBaseUrls,
    required this.neteaseAnonymousCookieUrl,
  });

  Map<String, dynamic> toJson() => {
        'kugouBaseUrl': kugouBaseUrl,
        'neteaseBaseUrls': neteaseBaseUrls,
        'neteaseAnonymousCookieUrl': neteaseAnonymousCookieUrl,
      };
}

class MusicSearchParams {
  final MusicService service;
  final String keyword;
  final int page;
  final int pageSize;

  const MusicSearchParams({
    required this.service,
    required this.keyword,
    required this.page,
    required this.pageSize,
  });

  Map<String, dynamic> toJson() => {
        'service': service.name,
        'keyword': keyword,
        'page': page,
        'pageSize': pageSize,
      };
}

class MusicAlbumTracksParams {
  final MusicService service;
  final String albumId;

  const MusicAlbumTracksParams({required this.service, required this.albumId});

  Map<String, dynamic> toJson() => {
        'service': service.name,
        'albumId': albumId,
      };
}

class MusicArtistAlbumsParams {
  final MusicService service;
  final String artistId;

  const MusicArtistAlbumsParams(
      {required this.service, required this.artistId});

  Map<String, dynamic> toJson() => {
        'service': service.name,
        'artistId': artistId,
      };
}

class MusicTrackPlayUrlParams {
  final MusicService service;
  final String trackId;
  final String? qualityId;
  final MusicAuthState auth;

  const MusicTrackPlayUrlParams({
    required this.service,
    required this.trackId,
    required this.qualityId,
    required this.auth,
  });

  Map<String, dynamic> toJson() => {
        'service': service.name,
        'trackId': trackId,
        'qualityId': qualityId,
        'auth': auth.toJson(),
      };
}

class MusicTrackPlayUrlResult {
  final String url;
  final String ext;

  const MusicTrackPlayUrlResult({required this.url, required this.ext});

  factory MusicTrackPlayUrlResult.fromJson(Map<String, dynamic> json) {
    return MusicTrackPlayUrlResult(
      url: pickString(json, ['url']),
      ext: pickString(json, ['ext']),
    );
  }
}

class QqMusicCookie {
  final Map<String, dynamic> raw;

  const QqMusicCookie(this.raw);

  factory QqMusicCookie.fromJson(Map<String, dynamic> json) =>
      QqMusicCookie(json);
  Map<String, dynamic> toJson() => raw;
}

class KugouUserInfo {
  final String token;
  final String userid;

  const KugouUserInfo({required this.token, required this.userid});

  factory KugouUserInfo.fromJson(Map<String, dynamic> json) {
    return KugouUserInfo(
      token: pickString(json, ['token']),
      userid: pickString(json, ['userid']),
    );
  }

  Map<String, dynamic> toJson() => {
        'token': token,
        'userid': userid,
      };
}

class MusicAuthState {
  final QqMusicCookie? qq;
  final KugouUserInfo? kugou;
  final String? neteaseCookie;

  const MusicAuthState(
      {required this.qq, required this.kugou, required this.neteaseCookie});

  factory MusicAuthState.empty() =>
      const MusicAuthState(qq: null, kugou: null, neteaseCookie: null);

  Map<String, dynamic> toJson() => {
        'qq': qq?.toJson(),
        'kugou': kugou?.toJson(),
        'neteaseCookie': neteaseCookie,
      };
}

class MusicLoginQr {
  final String sessionId;
  final String loginType;
  final String mime;
  final String base64;
  final String identifier;
  final int createdAtUnixMs;

  const MusicLoginQr({
    required this.sessionId,
    required this.loginType,
    required this.mime,
    required this.base64,
    required this.identifier,
    required this.createdAtUnixMs,
  });

  factory MusicLoginQr.fromJson(Map<String, dynamic> json) {
    return MusicLoginQr(
      sessionId: pickString(json, ['sessionId', 'session_id']),
      loginType: pickString(json, ['loginType', 'login_type']),
      mime: pickString(json, ['mime']),
      base64: pickString(json, ['base64']),
      identifier: pickString(json, ['identifier']),
      createdAtUnixMs: pickInt(json, ['createdAtUnixMs', 'created_at_unix_ms']),
    );
  }
}

class MusicLoginQrPollResult {
  final String sessionId;
  final String state;
  final String? message;
  final QqMusicCookie? cookie;
  final KugouUserInfo? kugouUser;

  const MusicLoginQrPollResult({
    required this.sessionId,
    required this.state,
    required this.message,
    required this.cookie,
    required this.kugouUser,
  });

  factory MusicLoginQrPollResult.fromJson(Map<String, dynamic> json) {
    final cookie = pickMap(json, ['cookie']);
    final kugouUser = pickMap(json, ['kugouUser', 'kugou_user']);
    return MusicLoginQrPollResult(
      sessionId: pickString(json, ['sessionId', 'session_id']),
      state: pickString(json, ['state']),
      message: pick<String>(json, ['message']),
      cookie: cookie == null ? null : QqMusicCookie.fromJson(cookie),
      kugouUser: kugouUser == null ? null : KugouUserInfo.fromJson(kugouUser),
    );
  }
}

class MusicDownloadStartParams {
  final MusicProviderConfig config;
  final MusicAuthState auth;
  final Map<String, dynamic>
      target; // Keep flexible: matches proto tagged enum.
  final MusicDownloadOptions options;

  const MusicDownloadStartParams({
    required this.config,
    required this.auth,
    required this.target,
    required this.options,
  });

  Map<String, dynamic> toJson() => {
        'config': config.toJson(),
        'auth': auth.toJson(),
        'target': target,
        'options': options.toJson(),
      };
}

class MusicDownloadOptions {
  final String qualityId;
  final String outDir;
  final String? pathTemplate;
  final bool overwrite;
  final int concurrency;
  final int retries;

  const MusicDownloadOptions({
    required this.qualityId,
    required this.outDir,
    required this.pathTemplate,
    required this.overwrite,
    required this.concurrency,
    required this.retries,
  });

  Map<String, dynamic> toJson() => {
        'qualityId': qualityId,
        'outDir': outDir,
        'pathTemplate': pathTemplate,
        'overwrite': overwrite,
        'concurrency': concurrency,
        'retries': retries,
      };
}

class MusicDownloadStartResult {
  final String sessionId;

  const MusicDownloadStartResult({required this.sessionId});

  factory MusicDownloadStartResult.fromJson(Map<String, dynamic> json) {
    return MusicDownloadStartResult(
      sessionId: pickString(json, ['sessionId', 'session_id']),
    );
  }
}

class MusicDownloadStatus {
  final bool done;
  final Map<String, dynamic> totals;
  final List<Map<String, dynamic>> jobs;

  const MusicDownloadStatus({
    required this.done,
    required this.totals,
    required this.jobs,
  });

  factory MusicDownloadStatus.fromJson(Map<String, dynamic> json) {
    final totals = pickMap(json, ['totals']) ?? const <String, dynamic>{};
    final rawJobs = pickList(json, ['jobs']) ?? const [];
    return MusicDownloadStatus(
      done: pickBool(json, ['done']),
      totals: totals,
      jobs: rawJobs
          .whereType<Map>()
          .map((e) => e.cast<String, dynamic>())
          .toList(growable: false),
    );
  }
}
