import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';

import '../../core/models/live_directory.dart';
import 'network_image_view.dart';

String? formatOnlineCount(int? n) {
  if (n == null) return null;
  if (n < 0) return null;
  if (n < 10000) return n.toString();
  final w = (n / 10000.0);
  return '${w.toStringAsFixed(w >= 10 ? 0 : 1)}万';
}

class RoomCard extends StatelessWidget {
  const RoomCard({
    super.key,
    required this.room,
    this.onTap,
  });

  final LiveDirRoomCard room;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return Platform.isWindows
        ? _WindowsRoomCard(room: room, onTap: onTap)
        : _MaterialRoomCard(room: room, onTap: onTap);
  }
}

class _MaterialRoomCard extends StatelessWidget {
  const _MaterialRoomCard({required this.room, this.onTap});
  final LiveDirRoomCard room;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final online = formatOnlineCount(room.online);
    final cs = Theme.of(context).colorScheme;

    return LayoutBuilder(
      builder: (context, c) {
        // 卡片高度较小时（例如网格列数很多时），降低信息密度避免 RenderFlex overflow。
        final compact = c.maxHeight > 0 && c.maxHeight < 180;
        final titleLines = compact ? 1 : 2;
        final showUser = !compact;
        final pad = compact
            ? const EdgeInsets.fromLTRB(12, 8, 12, 10)
            : const EdgeInsets.fromLTRB(12, 10, 12, 12);

        return Card(
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            onTap: onTap,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                AspectRatio(
                  aspectRatio: 16 / 9,
                  child: Stack(
                    fit: StackFit.expand,
                    children: [
                      NetworkImageView(url: room.cover, borderRadius: 0),
                      if (online != null)
                        Positioned(
                          left: 0,
                          right: 0,
                          bottom: 0,
                          child: Container(
                            decoration: BoxDecoration(
                              gradient: LinearGradient(
                                begin: Alignment.bottomCenter,
                                end: Alignment.topCenter,
                                colors: [
                                  Colors.black.withOpacity(0.75),
                                  Colors.transparent,
                                ],
                              ),
                            ),
                            padding: const EdgeInsets.fromLTRB(6, 10, 6, 6),
                            child: Row(
                              mainAxisAlignment: MainAxisAlignment.end,
                              children: [
                                const Icon(Icons.local_fire_department,
                                    color: Colors.white, size: 14),
                                const SizedBox(width: 4),
                                Text(
                                  online,
                                  style: const TextStyle(
                                    fontSize: 12,
                                    color: Colors.white,
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
                Padding(
                  padding: pad,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        room.title,
                        maxLines: titleLines,
                        overflow: TextOverflow.ellipsis,
                        style: Theme.of(context)
                            .textTheme
                            .titleSmall
                            ?.copyWith(fontWeight: FontWeight.w600),
                      ),
                      if (showUser) ...[
                        const SizedBox(height: 6),
                        Text(
                          room.userName ?? '',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: Theme.of(context)
                              .textTheme
                              .bodySmall
                              ?.copyWith(color: cs.onSurfaceVariant),
                        ),
                      ],
                    ],
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _WindowsRoomCard extends StatelessWidget {
  const _WindowsRoomCard({required this.room, this.onTap});
  final LiveDirRoomCard room;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final online = formatOnlineCount(room.online);

    return LayoutBuilder(
      builder: (context, c) {
        final compact = c.maxHeight > 0 && c.maxHeight < 180;
        final titleLines = compact ? 1 : 2;
        final showUser = !compact;
        final pad = compact
            ? const EdgeInsets.fromLTRB(12, 8, 12, 10)
            : const EdgeInsets.fromLTRB(12, 10, 12, 12);

        // fluent_ui 的 Card + Button 组合，视觉上更接近 WinUI3 的“卡片”交互。
        return fluent.Button(
          onPressed: onTap,
          style: const fluent.ButtonStyle(
            padding: fluent.WidgetStatePropertyAll(EdgeInsets.zero),
          ),
          child: fluent.Card(
            padding: EdgeInsets.zero,
            child: fluent.Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                fluent.AspectRatio(
                  aspectRatio: 16 / 9,
                  child: Stack(
                    fit: StackFit.expand,
                    children: [
                      // Windows 端直接用 Image.network，避免额外依赖。
                      ClipRRect(
                        borderRadius: const BorderRadius.only(
                          topLeft: Radius.circular(8),
                          topRight: Radius.circular(8),
                        ),
                        child: (room.cover ?? '').trim().isEmpty
                            ? Container(color: Colors.black12)
                            : Image.network(
                                (room.cover ?? '').trim(),
                                fit: BoxFit.cover,
                                errorBuilder: (context, _, __) =>
                                    Container(color: Colors.black12),
                                loadingBuilder: (context, child, progress) {
                                  if (progress == null) return child;
                                  return Container(color: Colors.black12);
                                },
                              ),
                      ),
                      if (online != null)
                        Positioned(
                          left: 0,
                          right: 0,
                          bottom: 0,
                          child: Container(
                            decoration: BoxDecoration(
                              gradient: LinearGradient(
                                begin: Alignment.bottomCenter,
                                end: Alignment.topCenter,
                                colors: [
                                  Colors.black.withOpacity(0.75),
                                  Colors.transparent,
                                ],
                              ),
                            ),
                            padding: const EdgeInsets.fromLTRB(6, 10, 6, 6),
                            child: Row(
                              mainAxisAlignment: MainAxisAlignment.end,
                              children: [
                                const Icon(Icons.local_fire_department,
                                    color: Colors.white, size: 14),
                                const SizedBox(width: 4),
                                Text(
                                  online,
                                  style: const TextStyle(
                                    fontSize: 12,
                                    color: Colors.white,
                                    fontWeight: FontWeight.w600,
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
                fluent.Padding(
                  padding: pad,
                  child: fluent.Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      fluent.Text(
                        room.title,
                        maxLines: titleLines,
                        overflow: TextOverflow.ellipsis,
                        style: const fluent.TextStyle(
                            fontSize: 13, fontWeight: FontWeight.w600),
                      ),
                      if (showUser) ...[
                        const fluent.SizedBox(height: 6),
                        fluent.Text(
                          room.userName ?? '',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const fluent.TextStyle(fontSize: 12),
                        ),
                      ],
                    ],
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}
