import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../core/backend/backend_factory.dart';
import '../../core/backend/chaos_backend.dart';
import '../../core/settings/settings_controller.dart';
import '../categories/categories_page.dart';
import '../home/home_page.dart';
import '../live/live_decode_page.dart';
import '../music/music_page.dart';
import '../settings/settings_page.dart';

class AndroidShell extends StatefulWidget {
  const AndroidShell({super.key});

  @override
  State<AndroidShell> createState() => _AndroidShellState();
}

class _AndroidShellState extends State<AndroidShell> {
  ChaosBackend? _backend;
  Object? _err;
  int _idx = 0;
  bool _initInFlight = false;

  @override
  void initState() {
    super.initState();
    // Delay until widget is mounted so Provider is available.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      unawaited(_maybeInit());
    });
  }

  Future<void> _maybeInit() async {
    if (_backend != null || _err != null || _initInFlight) return;
    final settings = context.read<SettingsController>();
    if (!settings.loaded) return;
    _initInFlight = true;
    try {
      final b = await BackendFactory.create(settings);
      if (mounted) setState(() => _backend = b);
    } catch (e) {
      if (mounted) setState(() => _err = e);
    } finally {
      _initInFlight = false;
    }
  }

  @override
  void dispose() {
    _backend?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // 订阅 settings.loaded：避免“首次 build 时 loaded=false -> 以后不再触发初始化”。
    final settingsLoaded =
        context.select<SettingsController, bool>((s) => s.loaded);
    if (settingsLoaded && _backend == null && _err == null) {
      unawaited(_maybeInit());
    }

    final b = _backend;
    if (_err != null) {
      return Scaffold(body: Center(child: Text('后端初始化失败：$_err')));
    }
    if (b == null) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }

    final pages = <Widget>[
      HomePage(backend: b),
      CategoriesPage(backend: b),
      LiveDecodePage(backend: b),
      MusicPage(backend: b),
      SettingsPage(backend: b),
    ];

    return LayoutBuilder(
      builder: (context, c) {
        final wide = c.maxWidth >= 840;
        if (!wide) {
          return Scaffold(
            body: pages[_idx],
            bottomNavigationBar: NavigationBar(
              selectedIndex: _idx,
              onDestinationSelected: (v) => setState(() => _idx = v),
              destinations: const [
                NavigationDestination(icon: Icon(Icons.home), label: '主页'),
                NavigationDestination(icon: Icon(Icons.category), label: '分类'),
                NavigationDestination(icon: Icon(Icons.live_tv), label: '直播'),
                NavigationDestination(
                    icon: Icon(Icons.music_note), label: '歌曲'),
                NavigationDestination(icon: Icon(Icons.settings), label: '设置'),
              ],
            ),
          );
        }

        return Scaffold(
          body: Row(
            children: [
              NavigationRail(
                selectedIndex: _idx,
                onDestinationSelected: (v) => setState(() => _idx = v),
                labelType: NavigationRailLabelType.all,
                destinations: const [
                  NavigationRailDestination(
                      icon: Icon(Icons.home), label: Text('主页')),
                  NavigationRailDestination(
                      icon: Icon(Icons.category), label: Text('分类')),
                  NavigationRailDestination(
                      icon: Icon(Icons.live_tv), label: Text('直播')),
                  NavigationRailDestination(
                      icon: Icon(Icons.music_note), label: Text('歌曲')),
                  NavigationRailDestination(
                      icon: Icon(Icons.settings), label: Text('设置')),
                ],
              ),
              const VerticalDivider(width: 1),
              Expanded(child: pages[_idx]),
            ],
          ),
        );
      },
    );
  }
}
