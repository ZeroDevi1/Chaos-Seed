import 'json_utils.dart';

class ThunderSubtitleItem {
  final String gcid;
  final String cid;
  final String url;
  final String ext;
  final String name;
  final int duration;
  final List<String> languages;
  final int source;
  final double score;
  final double fingerprintfScore;
  final String extraName;
  final int mt;

  const ThunderSubtitleItem({
    required this.gcid,
    required this.cid,
    required this.url,
    required this.ext,
    required this.name,
    required this.duration,
    required this.languages,
    required this.source,
    required this.score,
    required this.fingerprintfScore,
    required this.extraName,
    required this.mt,
  });

  factory ThunderSubtitleItem.fromJson(Map<String, dynamic> json) {
    final rawLang = pickList(json, ['languages']) ?? const [];
    return ThunderSubtitleItem(
      gcid: pickString(json, ['gcid']),
      cid: pickString(json, ['cid']),
      url: pickString(json, ['url']),
      ext: pickString(json, ['ext']),
      name: pickString(json, ['name']),
      duration: pickInt(json, ['duration']),
      languages: rawLang.whereType<String>().toList(growable: false),
      source: pickInt(json, ['source']),
      score: (pick<dynamic>(json, ['score']) as num?)?.toDouble() ?? 0,
      fingerprintfScore:
          (pick<dynamic>(json, ['fingerprintf_score', 'fingerprintfScore'])
                      as num?)
                  ?.toDouble() ??
              0,
      extraName: pickString(json, ['extra_name', 'extraName']),
      mt: pickInt(json, ['mt']),
    );
  }

  Map<String, dynamic> toJson() => {
        'gcid': gcid,
        'cid': cid,
        'url': url,
        'ext': ext,
        'name': name,
        'duration': duration,
        'languages': languages,
        'source': source,
        'score': score,
        'fingerprintf_score': fingerprintfScore,
        'extra_name': extraName,
        'mt': mt,
      };
}

class SubtitleSearchParams {
  final String query;
  final int limit;
  final double? minScore;
  final String? lang;
  final int timeoutMs;

  const SubtitleSearchParams({
    required this.query,
    required this.limit,
    required this.minScore,
    required this.lang,
    required this.timeoutMs,
  });
}

class SubtitleDownloadParams {
  final ThunderSubtitleItem item;
  final String outDir;
  final int timeoutMs;
  final int retries;
  final bool overwrite;

  const SubtitleDownloadParams({
    required this.item,
    required this.outDir,
    required this.timeoutMs,
    required this.retries,
    required this.overwrite,
  });
}

class SubtitleDownloadReply {
  final String path;
  final int bytes;

  const SubtitleDownloadReply({required this.path, required this.bytes});

  factory SubtitleDownloadReply.fromJson(Map<String, dynamic> json) {
    return SubtitleDownloadReply(
      path: pickString(json, ['path']),
      bytes: pickInt(json, ['bytes']),
    );
  }
}
