import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:chaos_flutter/features/live/player/android_player_controller.dart';

void main() {
  test('controls auto-hide after 5 seconds', () {
    fakeAsync((async) {
      final c = AndroidPlayerController();
      expect(c.showControls, isFalse);

      c.toggleControls();
      expect(c.showControls, isTrue);

      async.elapse(const Duration(seconds: 4));
      expect(c.showControls, isTrue);

      async.elapse(const Duration(seconds: 1));
      expect(c.showControls, isFalse);
      c.dispose();
    });
  });

  test('reset timer keeps controls visible', () {
    fakeAsync((async) {
      final c = AndroidPlayerController();
      c.showControlsNow();
      async.elapse(const Duration(seconds: 3));
      c.resetHideTimer();
      async.elapse(const Duration(seconds: 3));
      expect(c.showControls, isTrue);
      async.elapse(const Duration(seconds: 2));
      expect(c.showControls, isFalse);
      c.dispose();
    });
  });

  test('lock hides controls and disables gestures', () {
    final c = AndroidPlayerController();
    c.showControlsNow();
    c.toggleLock();
    expect(c.locked, isTrue);
    expect(c.showControls, isFalse);
    c.toggleLock();
    expect(c.locked, isFalse);
    c.dispose();
  });
}

