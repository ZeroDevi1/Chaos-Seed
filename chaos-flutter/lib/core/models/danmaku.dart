import 'json_utils.dart';

class DanmakuMessage {
  final String sessionId;
  final int receivedAtMs;
  final String user;
  final String text;
  final String? imageUrl;
  final int? imageWidth;

  const DanmakuMessage({
    required this.sessionId,
    required this.receivedAtMs,
    required this.user,
    required this.text,
    required this.imageUrl,
    required this.imageWidth,
  });

  factory DanmakuMessage.fromDaemonJson(Map<String, dynamic> json) {
    return DanmakuMessage(
      sessionId: pickString(json, ['sessionId', 'session_id']),
      receivedAtMs: pickInt(json, ['receivedAtMs', 'received_at_ms']),
      user: pickString(json, ['user'], fallback: ''),
      text: pickString(json, ['text'], fallback: ''),
      imageUrl: pick<String>(json, ['imageUrl', 'image_url']),
      imageWidth: pick<int>(json, ['imageWidth', 'image_width']),
    );
  }

  factory DanmakuMessage.fromFfiEventJson(
      String sessionId, Map<String, dynamic> json) {
    // FFI emits chaos-core DanmakuEvent (snake_case). Map it to a simplified message.
    // If the platform supports image DMs, they are in `dms[0].image_url` etc.
    String? imageUrl;
    int? imageWidth;
    final dms = pickList(json, ['dms']);
    if (dms != null && dms.isNotEmpty && dms.first is Map) {
      final dm0 = (dms.first as Map).cast<String, dynamic>();
      imageUrl = pick<String>(dm0, ['image_url']);
      final w = pick<dynamic>(dm0, ['image_width']);
      if (w is int) imageWidth = w;
      if (w is num) imageWidth = w.toInt();
    }

    var text = pickString(json, ['text'], fallback: '');
    if (text.trim().isEmpty) {
      // 有些站点/弹幕类型会把内容拆分到 dms 数组里，顶层 text 为空。
      if (dms != null && dms.isNotEmpty) {
        final parts = <String>[];
        for (final it in dms) {
          if (it is Map) {
            final s = pick<String>(it.cast<String, dynamic>(), ['text']);
            if (s != null && s.trim().isNotEmpty) parts.add(s.trim());
          }
        }
        if (parts.isNotEmpty) text = parts.join('');
      }
    }

    return DanmakuMessage(
      sessionId: sessionId,
      receivedAtMs: pickInt(json, ['received_at_ms', 'receivedAtMs']),
      user: pickString(json, ['user'], fallback: ''),
      text: text,
      imageUrl: imageUrl,
      imageWidth: imageWidth,
    );
  }
}

class DanmakuFetchImageResult {
  final String mime;
  final String base64;
  final int width;

  const DanmakuFetchImageResult({
    required this.mime,
    required this.base64,
    required this.width,
  });

  factory DanmakuFetchImageResult.fromJson(Map<String, dynamic> json) {
    return DanmakuFetchImageResult(
      mime: pickString(json, ['mime']),
      base64: pickString(json, ['base64']),
      width: pickInt(json, ['width']),
    );
  }
}
