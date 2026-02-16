import 'json_utils.dart';

class NowPlayingSnapshotParams {
  final bool includeThumbnail;
  final int maxThumbnailBytes;
  final int maxSessions;

  const NowPlayingSnapshotParams({
    required this.includeThumbnail,
    required this.maxThumbnailBytes,
    required this.maxSessions,
  });

  Map<String, dynamic> toJson() => {
        'includeThumbnail': includeThumbnail,
        'maxThumbnailBytes': maxThumbnailBytes,
        'maxSessions': maxSessions,
      };
}

class NowPlayingThumbnail {
  final String mime;
  final String base64;

  const NowPlayingThumbnail({required this.mime, required this.base64});

  factory NowPlayingThumbnail.fromJson(Map<String, dynamic> json) {
    return NowPlayingThumbnail(
      mime: pickString(json, ['mime']),
      base64: pickString(json, ['base64']),
    );
  }
}

class NowPlayingSession {
  final String appId;
  final bool isCurrent;
  final String playbackStatus;
  final String? title;
  final String? artist;
  final String? albumTitle;
  final int? positionMs;
  final int? durationMs;
  final List<String> genres;
  final String? songId;
  final NowPlayingThumbnail? thumbnail;
  final String? error;

  const NowPlayingSession({
    required this.appId,
    required this.isCurrent,
    required this.playbackStatus,
    required this.title,
    required this.artist,
    required this.albumTitle,
    required this.positionMs,
    required this.durationMs,
    required this.genres,
    required this.songId,
    required this.thumbnail,
    required this.error,
  });

  factory NowPlayingSession.fromJson(Map<String, dynamic> json) {
    final rawGenres = pickList(json, ['genres']) ?? const [];
    final thumb = pickMap(json, ['thumbnail']);
    return NowPlayingSession(
      appId: pickString(json, ['appId', 'app_id']),
      isCurrent: pickBool(json, ['isCurrent', 'is_current']),
      playbackStatus: pickString(json, ['playbackStatus', 'playback_status']),
      title: pick<String>(json, ['title']),
      artist: pick<String>(json, ['artist']),
      albumTitle: pick<String>(json, ['albumTitle', 'album_title']),
      positionMs: pick<int>(json, ['positionMs', 'position_ms']),
      durationMs: pick<int>(json, ['durationMs', 'duration_ms']),
      genres: rawGenres.whereType<String>().toList(growable: false),
      songId: pick<String>(json, ['songId', 'song_id']),
      thumbnail: thumb == null ? null : NowPlayingThumbnail.fromJson(thumb),
      error: pick<String>(json, ['error']),
    );
  }
}

class NowPlayingSnapshot {
  final bool supported;
  final int retrievedAtUnixMs;
  final String? pickedAppId;
  final NowPlayingSession? nowPlaying;
  final List<NowPlayingSession> sessions;

  const NowPlayingSnapshot({
    required this.supported,
    required this.retrievedAtUnixMs,
    required this.pickedAppId,
    required this.nowPlaying,
    required this.sessions,
  });

  factory NowPlayingSnapshot.fromJson(Map<String, dynamic> json) {
    final rawSessions = pickList(json, ['sessions']) ?? const [];
    final nowPlayingMap = pickMap(json, ['nowPlaying', 'now_playing']);
    return NowPlayingSnapshot(
      supported: pickBool(json, ['supported']),
      retrievedAtUnixMs:
          pickInt(json, ['retrievedAtUnixMs', 'retrieved_at_unix_ms']),
      pickedAppId: pick<String>(json, ['pickedAppId', 'picked_app_id']),
      nowPlaying: nowPlayingMap == null
          ? null
          : NowPlayingSession.fromJson(nowPlayingMap),
      sessions: rawSessions
          .whereType<Map>()
          .map((e) => NowPlayingSession.fromJson(e.cast<String, dynamic>()))
          .toList(growable: false),
    );
  }
}
