import 'package:flutter/material.dart';

import '../../../core/models/live.dart';

class PlaySourceSheet {
  static Future<void> show({
    required BuildContext context,
    required List<LivestreamVariant> variants,
    required String currentVariantId,
    required Future<void> Function(LivestreamVariant v) onPickVariant,
    required List<String> lines,
    required int currentLineIndex,
    required Future<void> Function(int idx) onPickLine,
  }) async {
    final sorted = [...variants]..sort((a, b) => b.quality.compareTo(a.quality));

    await showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (_) {
        return SafeArea(
          child: ListView(
            children: [
              const ListTile(
                title: Text('播放设置'),
                subtitle: Text('清晰度 / 线路'),
              ),
              const Divider(height: 1),
              const ListTile(title: Text('清晰度')),
              for (final v in sorted)
                ListTile(
                  title: Text(v.label),
                  subtitle: Text('quality=${v.quality}'),
                  trailing: v.id == currentVariantId
                      ? const Icon(Icons.check)
                      : const Icon(Icons.chevron_right),
                  onTap: () async {
                    Navigator.of(context).pop();
                    await onPickVariant(v);
                  },
                ),
              const Divider(height: 1),
              const ListTile(title: Text('线路')),
              for (var i = 0; i < lines.length; i++)
                ListTile(
                  title: Text('线路${i + 1}'),
                  subtitle: Text(_lineHint(lines[i])),
                  trailing: i == currentLineIndex
                      ? const Icon(Icons.check)
                      : const Icon(Icons.chevron_right),
                  onTap: () async {
                    Navigator.of(context).pop();
                    await onPickLine(i);
                  },
                ),
              const SizedBox(height: 12),
            ],
          ),
        );
      },
    );
  }

  static String _lineHint(String url) {
    final u = url.toLowerCase();
    if (u.contains('.m3u8')) return 'HLS';
    if (u.contains('.flv')) return 'FLV';
    return 'URL';
  }
}

