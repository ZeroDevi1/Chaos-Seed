import 'dart:async';
import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';
import 'package:flutter_acrylic/flutter_acrylic.dart' as acrylic;

import '../core/settings/settings_controller.dart';
import '../core/settings/settings_service.dart';
import 'routes.dart';

class ChaosSeedApp extends StatefulWidget {
  const ChaosSeedApp({super.key});

  @override
  State<ChaosSeedApp> createState() => _ChaosSeedAppState();
}

class _ChaosSeedAppState extends State<ChaosSeedApp> {
  late final SettingsController _settings;

  @override
  void initState() {
    super.initState();
    _settings = SettingsController(SettingsService());
    _settings.load();

    unawaited(_initWindows());
    _initAndroidEdgeToEdge();
  }

  Future<void> _initWindows() async {
    if (!Platform.isWindows) return;
    await windowManager.ensureInitialized();
    await acrylic.Window.initialize();
  }

  void _initAndroidEdgeToEdge() {
    if (!Platform.isAndroid) return;
    // 适配 Android 手势导航（小白条）沉浸：使用 edge-to-edge + 透明系统栏。
    SystemChrome.setEnabledSystemUIMode(SystemUiMode.edgeToEdge);
  }

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: _settings,
      child: Consumer<SettingsController>(
        builder: (context, s, _) {
          if (Platform.isWindows) {
            return fluent.FluentApp(
              debugShowCheckedModeBanner: false,
              title: 'Chaos Seed',
              themeMode: s.fluentThemeMode,
              theme: s.fluentLightTheme,
              darkTheme: s.fluentDarkTheme,
              initialRoute: Routes.home,
              onGenerateRoute: Routes.onGenerateRoute,
            );
          }

          return MaterialApp(
            debugShowCheckedModeBanner: false,
            title: 'Chaos Seed',
            themeMode: s.materialThemeMode,
            theme: s.materialLightTheme,
            darkTheme: s.materialDarkTheme,
            initialRoute: Routes.home,
            onGenerateRoute: Routes.onGenerateRoute,
            builder: (context, child) {
              final isDark = Theme.of(context).brightness == Brightness.dark;
              final style = SystemUiOverlayStyle(
                statusBarColor: Colors.transparent,
                systemNavigationBarColor: Colors.transparent,
                systemNavigationBarDividerColor: Colors.transparent,
                systemStatusBarContrastEnforced: false,
                systemNavigationBarContrastEnforced: false,
                statusBarIconBrightness:
                    isDark ? Brightness.light : Brightness.dark,
                systemNavigationBarIconBrightness:
                    isDark ? Brightness.light : Brightness.dark,
              );
              return AnnotatedRegion<SystemUiOverlayStyle>(
                value: style,
                child: child ?? const SizedBox.shrink(),
              );
            },
          );
        },
      ),
    );
  }
}
