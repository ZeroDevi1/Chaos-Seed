import 'json_utils.dart';

class LyricsSearchParams {
  final String title;
  final String? album;
  final String? artist;
  final int durationMs;
  final int limit;
  final bool strictMatch;
  final List<String> services;
  final int timeoutMs;

  const LyricsSearchParams({
    required this.title,
    required this.album,
    required this.artist,
    required this.durationMs,
    required this.limit,
    required this.strictMatch,
    required this.services,
    required this.timeoutMs,
  });

  Map<String, dynamic> toDaemonJson() => {
        'title': title,
        'album': album,
        'artist': artist,
        'durationMs': durationMs,
        'limit': limit,
        'strictMatch': strictMatch,
        'services': services,
        'timeoutMs': timeoutMs,
      };
}

class LyricsSearchResult {
  final String service;
  final String serviceToken;
  final String? title;
  final String? artist;
  final String? album;
  final int? durationMs;
  final int matchPercentage;
  final double quality;
  final bool matched;
  final bool hasTranslation;
  final bool hasInlineTimetags;
  final String lyricsOriginal;
  final String? lyricsTranslation;
  final Object? debug;

  const LyricsSearchResult({
    required this.service,
    required this.serviceToken,
    required this.title,
    required this.artist,
    required this.album,
    required this.durationMs,
    required this.matchPercentage,
    required this.quality,
    required this.matched,
    required this.hasTranslation,
    required this.hasInlineTimetags,
    required this.lyricsOriginal,
    required this.lyricsTranslation,
    required this.debug,
  });

  factory LyricsSearchResult.fromJson(Map<String, dynamic> json) {
    return LyricsSearchResult(
      service: pickString(json, ['service']),
      serviceToken: pickString(json, ['serviceToken', 'service_token']),
      title: pick<String>(json, ['title']),
      artist: pick<String>(json, ['artist']),
      album: pick<String>(json, ['album']),
      durationMs: pick<int>(json, ['durationMs', 'duration_ms']),
      matchPercentage: pickInt(json, ['matchPercentage', 'match_percentage']),
      quality: (pick<dynamic>(json, ['quality']) as num?)?.toDouble() ?? 0,
      matched: pickBool(json, ['matched']),
      hasTranslation: pickBool(json, ['hasTranslation', 'has_translation']),
      hasInlineTimetags:
          pickBool(json, ['hasInlineTimetags', 'has_inline_timetags']),
      lyricsOriginal: pickString(json, ['lyricsOriginal', 'lyrics_original']),
      lyricsTranslation:
          pick<String>(json, ['lyricsTranslation', 'lyrics_translation']),
      debug: pick<dynamic>(json, ['debug']),
    );
  }
}
