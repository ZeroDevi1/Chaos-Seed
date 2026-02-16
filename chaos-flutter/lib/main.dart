import 'dart:io';

import 'package:flutter/material.dart';
import 'package:media_kit/media_kit.dart';

import 'app/chaos_seed_app.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  MediaKit.ensureInitialized();

  // Platform specific init is handled inside the app.
  runApp(const ChaosSeedApp());
}

bool get isWindows => Platform.isWindows;
