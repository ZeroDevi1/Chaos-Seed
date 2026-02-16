import 'package:flutter_test/flutter_test.dart';

import 'package:chaos_flutter/core/models/json_utils.dart';
import 'package:chaos_flutter/core/models/live.dart';

void main() {
  test('pickString converts num to string', () {
    expect(pickString({'room_id': 123}, ['room_id']), '123');
    expect(pickString({'x': 1.5}, ['x']), '1.5');
  });

  test('LivestreamDecodeManifestResult parses numeric room_id', () {
    final man = LivestreamDecodeManifestResult.fromJson({
      'site': 'BiliLive',
      'room_id': 999,
      'raw_input': 'https://live.bilibili.com/999',
      'info': {
        'title': 't',
        'is_living': true,
      },
      'playback': {},
      'variants': const [],
    });
    expect(man.roomId, '999');
  });
}

