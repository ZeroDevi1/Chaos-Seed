import 'dart:io';

import '../settings/settings_controller.dart';
import 'chaos_backend.dart';
import 'ffi_backend.dart';
import 'hybrid_backend.dart';

class BackendFactory {
  // Single backend instance (shared across features).
  static Future<ChaosBackend> create(SettingsController settings) async {
    // Android: always call Rust core through FFI (no Dart re-implementation).
    if (!Platform.isWindows) {
      return FfiChaosBackend();
    }

    // Windows: use per-feature backend selection (Auto/FFI/daemon) via a hybrid facade.
    return HybridChaosBackend(settings);
  }
}
