import 'dart:async';

import 'package:flutter/foundation.dart';

class AndroidPlayerController extends ChangeNotifier {
  bool _showControls = false;
  bool get showControls => _showControls;

  bool _locked = false;
  bool get locked => _locked;

  bool _fullScreen = false;
  bool get fullScreen => _fullScreen;

  bool _showGestureTip = false;
  bool get showGestureTip => _showGestureTip;

  String _gestureTipText = '';
  String get gestureTipText => _gestureTipText;

  Timer? _hideControlsTimer;
  Timer? _hideTipTimer;

  void toggleControls() {
    _showControls = !_showControls;
    if (_showControls) {
      resetHideTimer();
    } else {
      _hideControlsTimer?.cancel();
    }
    notifyListeners();
  }

  void showControlsNow() {
    _showControls = true;
    resetHideTimer();
    notifyListeners();
  }

  void hideControlsNow() {
    _showControls = false;
    _hideControlsTimer?.cancel();
    notifyListeners();
  }

  void resetHideTimer() {
    if (!_showControls) return;
    _hideControlsTimer?.cancel();
    _hideControlsTimer = Timer(const Duration(seconds: 5), () {
      _showControls = false;
      notifyListeners();
    });
  }

  void toggleLock() {
    _locked = !_locked;
    if (_locked) {
      _showControls = false;
      _hideControlsTimer?.cancel();
    } else {
      // 解锁后立即显示控件（对齐 simple_live 的手感）。
      _showControls = true;
      resetHideTimer();
    }
    notifyListeners();
  }

  void enterFullScreen() {
    _fullScreen = true;
    showControlsNow();
    notifyListeners();
  }

  void exitFullScreen() {
    _fullScreen = false;
    _locked = false;
    showControlsNow();
    notifyListeners();
  }

  void showTip(String text) {
    _gestureTipText = text;
    _showGestureTip = text.trim().isNotEmpty;
    _hideTipTimer?.cancel();
    if (_showGestureTip) {
      _hideTipTimer = Timer(const Duration(milliseconds: 600), () {
        _showGestureTip = false;
        notifyListeners();
      });
    }
    notifyListeners();
  }

  @override
  void dispose() {
    _hideControlsTimer?.cancel();
    _hideTipTimer?.cancel();
    super.dispose();
  }
}
