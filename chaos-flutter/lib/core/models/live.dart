import 'json_utils.dart';

class LivestreamPlaybackHints {
  final String? referer;
  final String? userAgent;

  const LivestreamPlaybackHints(
      {required this.referer, required this.userAgent});

  factory LivestreamPlaybackHints.fromJson(Map<String, dynamic> json) {
    return LivestreamPlaybackHints(
      referer: pick<String>(json, ['referer']),
      userAgent: pick<String>(json, ['userAgent', 'user_agent']),
    );
  }
}

class LivestreamInfo {
  final String title;
  final String? name;
  final String? avatar;
  final String? cover;
  final bool isLiving;

  const LivestreamInfo({
    required this.title,
    required this.name,
    required this.avatar,
    required this.cover,
    required this.isLiving,
  });

  factory LivestreamInfo.fromJson(Map<String, dynamic> json) {
    return LivestreamInfo(
      title: pickString(json, ['title']),
      name: pick<String>(json, ['name']),
      avatar: pick<String>(json, ['avatar']),
      cover: pick<String>(json, ['cover']),
      isLiving: pickBool(json, ['isLiving', 'is_living']),
    );
  }
}

class LivestreamVariant {
  final String id;
  final String label;
  final int quality;
  final int? rate;
  final String? url;
  final List<String> backupUrls;

  const LivestreamVariant({
    required this.id,
    required this.label,
    required this.quality,
    required this.rate,
    required this.url,
    required this.backupUrls,
  });

  factory LivestreamVariant.fromJson(Map<String, dynamic> json) {
    final backups = pickList(json, ['backupUrls', 'backup_urls']) ?? const [];
    return LivestreamVariant(
      id: pickString(json, ['id']),
      label: pickString(json, ['label']),
      quality: pickInt(json, ['quality']),
      rate: pick<int>(json, ['rate']),
      url: pick<String>(json, ['url']),
      backupUrls: backups.whereType<String>().toList(growable: false),
    );
  }
}

class LivestreamDecodeManifestResult {
  final String site;
  final String roomId;
  final String rawInput;
  final LivestreamInfo info;
  final LivestreamPlaybackHints playback;
  final List<LivestreamVariant> variants;

  const LivestreamDecodeManifestResult({
    required this.site,
    required this.roomId,
    required this.rawInput,
    required this.info,
    required this.playback,
    required this.variants,
  });

  factory LivestreamDecodeManifestResult.fromJson(Map<String, dynamic> json) {
    final info = pickMap(json, ['info']) ?? const <String, dynamic>{};
    final playback = pickMap(json, ['playback']) ?? const <String, dynamic>{};
    final rawVariants = pickList(json, ['variants']) ?? const [];
    return LivestreamDecodeManifestResult(
      site: pickString(json, ['site']),
      roomId: pickString(json, ['roomId', 'room_id']),
      rawInput: pickString(json, ['rawInput', 'raw_input']),
      info: LivestreamInfo.fromJson(info),
      playback: LivestreamPlaybackHints.fromJson(playback),
      variants: rawVariants
          .whereType<Map>()
          .map((e) => LivestreamVariant.fromJson(e.cast<String, dynamic>()))
          .toList(growable: false),
    );
  }
}

class LiveOpenResult {
  final String sessionId;
  final String site;
  final String roomId;
  final String title;
  final String variantId;
  final String variantLabel;
  final String url;
  final List<String> backupUrls;
  final String? referer;
  final String? userAgent;

  const LiveOpenResult({
    required this.sessionId,
    required this.site,
    required this.roomId,
    required this.title,
    required this.variantId,
    required this.variantLabel,
    required this.url,
    required this.backupUrls,
    required this.referer,
    required this.userAgent,
  });

  factory LiveOpenResult.fromJson(Map<String, dynamic> json) {
    final backups = pickList(json, ['backupUrls', 'backup_urls']) ?? const [];
    return LiveOpenResult(
      sessionId: pickString(json, ['sessionId', 'session_id']),
      site: pickString(json, ['site']),
      roomId: pickString(json, ['roomId', 'room_id']),
      title: pickString(json, ['title']),
      variantId: pickString(json, ['variantId', 'variant_id']),
      variantLabel: pickString(json, ['variantLabel', 'variant_label']),
      url: pickString(json, ['url']),
      backupUrls: backups.whereType<String>().toList(growable: false),
      referer: pick<String>(json, ['referer']),
      userAgent: pick<String>(json, ['userAgent', 'user_agent']),
    );
  }
}
