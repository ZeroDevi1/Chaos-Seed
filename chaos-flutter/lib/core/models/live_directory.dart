import 'json_utils.dart';

class LiveDirSubCategory {
  final String id;
  final String parentId;
  final String name;
  final String? pic;

  const LiveDirSubCategory({
    required this.id,
    required this.parentId,
    required this.name,
    required this.pic,
  });

  factory LiveDirSubCategory.fromJson(Map<String, dynamic> json) {
    return LiveDirSubCategory(
      id: pickString(json, ['id']),
      parentId: pickString(json, ['parentId', 'parent_id']),
      name: pickString(json, ['name']),
      pic: pick<String>(json, ['pic']),
    );
  }
}

class LiveDirCategory {
  final String id;
  final String name;
  final List<LiveDirSubCategory> children;

  const LiveDirCategory({
    required this.id,
    required this.name,
    required this.children,
  });

  factory LiveDirCategory.fromJson(Map<String, dynamic> json) {
    final rawChildren = pickList(json, ['children']) ?? const [];
    return LiveDirCategory(
      id: pickString(json, ['id']),
      name: pickString(json, ['name']),
      children: rawChildren
          .whereType<Map>()
          .map((e) => LiveDirSubCategory.fromJson(e.cast<String, dynamic>()))
          .toList(growable: false),
    );
  }
}

class LiveDirRoomCard {
  final String site;
  final String roomId;
  final String input;
  final String title;
  final String? cover;
  final String? userName;
  final int? online;

  const LiveDirRoomCard({
    required this.site,
    required this.roomId,
    required this.input,
    required this.title,
    required this.cover,
    required this.userName,
    required this.online,
  });

  factory LiveDirRoomCard.fromJson(Map<String, dynamic> json) {
    final onlineRaw = pick<dynamic>(json, ['online']);
    int? online;
    if (onlineRaw is int) online = onlineRaw;
    if (onlineRaw is num) online = onlineRaw.toInt();

    return LiveDirRoomCard(
      site: pickString(json, ['site']),
      roomId: pickString(json, ['roomId', 'room_id']),
      input: pickString(json, ['input']),
      title: pickString(json, ['title']),
      cover: pick<String>(json, ['cover']),
      userName: pick<String>(json, ['userName', 'user_name']),
      online: online,
    );
  }
}

class LiveDirRoomListResult {
  final bool hasMore;
  final List<LiveDirRoomCard> items;

  const LiveDirRoomListResult({required this.hasMore, required this.items});

  factory LiveDirRoomListResult.fromJson(Map<String, dynamic> json) {
    final rawItems = pickList(json, ['items']) ?? const [];
    return LiveDirRoomListResult(
      hasMore: pickBool(json, ['hasMore', 'has_more']),
      items: rawItems
          .whereType<Map>()
          .map((e) => LiveDirRoomCard.fromJson(e.cast<String, dynamic>()))
          .toList(growable: false),
    );
  }
}
