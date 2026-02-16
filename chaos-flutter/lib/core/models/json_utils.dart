T? pick<T>(Map<String, dynamic> json, List<String> keys) {
  for (final k in keys) {
    final v = json[k];
    if (v is T) return v;
  }
  return null;
}

String pickString(Map<String, dynamic> json, List<String> keys,
    {String fallback = ''}) {
  final v = pick<dynamic>(json, keys);
  if (v is String) return v;
  return fallback;
}

int pickInt(Map<String, dynamic> json, List<String> keys, {int fallback = 0}) {
  final v = pick<dynamic>(json, keys);
  if (v is int) return v;
  if (v is num) return v.toInt();
  return fallback;
}

bool pickBool(Map<String, dynamic> json, List<String> keys,
    {bool fallback = false}) {
  final v = pick<dynamic>(json, keys);
  if (v is bool) return v;
  return fallback;
}

Map<String, dynamic>? pickMap(Map<String, dynamic> json, List<String> keys) {
  final v = pick<dynamic>(json, keys);
  if (v is Map) return v.cast<String, dynamic>();
  return null;
}

List<dynamic>? pickList(Map<String, dynamic> json, List<String> keys) {
  final v = pick<dynamic>(json, keys);
  if (v is List) return v;
  return null;
}
