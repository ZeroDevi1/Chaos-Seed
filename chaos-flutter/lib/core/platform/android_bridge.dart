import 'dart:io';

import 'package:flutter/services.dart';

/// Android 侧的少量平台能力（路径、PiP 等）。
///
/// 说明：
/// - 这里只放“Flutter 需要但纯 Dart 拿不到”的东西。
/// - Android 端固定使用 FFI 调用 core，本文件与后端选择无关。
class AndroidBridge {
  static const MethodChannel _ch = MethodChannel('chaos_seed/android');

  static Future<String?> getPublicDownloadsDir() async {
    if (!Platform.isAndroid) return null;
    final v = await _ch.invokeMethod<String>('getPublicDownloadsDir');
    return v?.trim().isEmpty == true ? null : v;
  }

  static Future<({String displayPath, bool skipped})?> exportIntoDownloads({
    required String outDir,
    required String sourcePath,
    bool overwrite = false,
  }) async {
    if (!Platform.isAndroid) return null;
    final raw = await _ch.invokeMethod<Map>('exportIntoDownloads', {
      'outDir': outDir,
      'sourcePath': sourcePath,
      'overwrite': overwrite,
    });
    if (raw == null) return null;
    final m = raw.cast<String, dynamic>();
    final dp = (m['displayPath'] as String?)?.trim() ?? '';
    final skipped = (m['skipped'] as bool?) ?? false;
    if (dp.isEmpty) return null;
    return (displayPath: dp, skipped: skipped);
  }

  static Future<bool> isPipSupported() async {
    if (!Platform.isAndroid) return false;
    final v = await _ch.invokeMethod<bool>('isPipSupported');
    return v ?? false;
  }

  static Future<void> enterPip({int aspectW = 16, int aspectH = 9}) async {
    if (!Platform.isAndroid) return;
    await _ch.invokeMethod<void>('enterPip', {
      'aspectW': aspectW,
      'aspectH': aspectH,
    });
  }
}
