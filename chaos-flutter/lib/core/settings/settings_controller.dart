import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';

import 'settings_model.dart';
import 'settings_service.dart';

class SettingsController extends ChangeNotifier {
  SettingsController(this._service);

  final SettingsService _service;

  AppSettings _settings = AppSettings.defaults();
  AppSettings get settings => _settings;

  bool _loaded = false;
  bool get loaded => _loaded;

  Future<void> load() async {
    try {
      _settings = await _service.load();
    } catch (e) {
      // 不要因为本地配置读取失败而卡住整个应用启动。
      debugPrint('Settings load failed, fallback to defaults: $e');
      _settings = AppSettings.defaults();
    }
    _loaded = true;
    notifyListeners();
  }

  Future<void> update(AppSettings next) async {
    _settings = next;
    notifyListeners();
    await _service.save(_settings);
  }

  // ---- theme mapping ----

  ThemeMode get materialThemeMode {
    switch (_settings.themeMode) {
      case AppThemeMode.system:
        return ThemeMode.system;
      case AppThemeMode.dark:
        return ThemeMode.dark;
      case AppThemeMode.light:
        return ThemeMode.light;
    }
  }

  fluent.ThemeMode get fluentThemeMode {
    switch (_settings.themeMode) {
      case AppThemeMode.system:
        return fluent.ThemeMode.system;
      case AppThemeMode.dark:
        return fluent.ThemeMode.dark;
      case AppThemeMode.light:
        return fluent.ThemeMode.light;
    }
  }

  ThemeData get materialLightTheme => ThemeData(
        useMaterial3: true,
        colorSchemeSeed: Colors.blue,
        cardTheme: CardThemeData(
          clipBehavior: Clip.antiAlias,
          shape:
              RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        ),
        appBarTheme: const AppBarTheme(
          centerTitle: true,
          titleTextStyle: TextStyle(fontSize: 16),
        ),
      );
  ThemeData get materialDarkTheme => ThemeData(
        useMaterial3: true,
        brightness: Brightness.dark,
        colorSchemeSeed: Colors.blue,
        cardTheme: CardThemeData(
          clipBehavior: Clip.antiAlias,
          shape:
              RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        ),
        appBarTheme: const AppBarTheme(
          centerTitle: true,
          titleTextStyle: TextStyle(fontSize: 16),
        ),
      );

  fluent.FluentThemeData get fluentLightTheme => fluent.FluentThemeData(
        brightness: fluent.Brightness.light,
        accentColor: fluent.Colors.blue,
      );

  fluent.FluentThemeData get fluentDarkTheme => fluent.FluentThemeData(
        brightness: fluent.Brightness.dark,
        accentColor: fluent.Colors.blue,
      );

  bool get supportsDaemon => Platform.isWindows;

  BackendMode effectiveBackendMode(BackendMode requested) {
    if (!supportsDaemon && requested == BackendMode.daemon) {
      return BackendMode.ffi;
    }
    return requested;
  }

  // Convenience: access per-module backend selections.
  BackendMode get liveBackendMode =>
      effectiveBackendMode(_settings.liveBackendMode);
  BackendMode get musicBackendMode =>
      effectiveBackendMode(_settings.musicBackendMode);
  BackendMode get lyricsBackendMode =>
      effectiveBackendMode(_settings.lyricsBackendMode);
  BackendMode get danmakuBackendMode =>
      effectiveBackendMode(_settings.danmakuBackendMode);
}
