import 'dart:io';

import 'package:flutter/material.dart';
import 'package:fluent_ui/fluent_ui.dart' as fluent;

import '../features/shell/android_shell.dart';
import '../features/shell/windows_shell.dart';

class Routes {
  static const home = '/';

  static Route<dynamic> onGenerateRoute(RouteSettings settings) {
    final name = settings.name ?? home;
    if (name == home) {
      if (Platform.isWindows) {
        return fluent.FluentPageRoute(builder: (_) => const WindowsShell());
      }
      return MaterialPageRoute(builder: (_) => const AndroidShell());
    }

    // Fallback.
    if (Platform.isWindows) {
      return fluent.FluentPageRoute(builder: (_) => const WindowsShell());
    }
    return MaterialPageRoute(builder: (_) => const AndroidShell());
  }
}
