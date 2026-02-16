import 'dart:async';

import 'package:fluent_ui/fluent_ui.dart';
import 'package:provider/provider.dart';

import '../../core/backend/backend_factory.dart';
import '../../core/backend/chaos_backend.dart';
import '../../core/settings/settings_controller.dart';
import '../categories/categories_page.dart';
import '../danmaku/danmaku_page.dart';
import '../home/home_page.dart';
import '../live/live_decode_page.dart';
import '../lyrics/lyrics_page.dart';
import '../music/music_page.dart';
import '../settings/settings_page.dart';
import '../subtitles/subtitles_page.dart';

class WindowsShell extends StatefulWidget {
  const WindowsShell({super.key});

  @override
  State<WindowsShell> createState() => _WindowsShellState();
}

class _WindowsShellState extends State<WindowsShell> {
  ChaosBackend? _backend;
  Object? _err;
  int _idx = 0;
  bool _initInFlight = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    unawaited(_init());
  }

  Future<void> _init() async {
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
    // 让 SettingsController.loaded 变化时触发重建，从而在加载完成后再初始化 backend。
    final settingsLoaded =
        context.select<SettingsController, bool>((s) => s.loaded);
    if (settingsLoaded && _backend == null && _err == null) {
      unawaited(_init());
    }

    final b = _backend;
    if (_err != null) {
      return ScaffoldPage(content: Center(child: Text('后端初始化失败：$_err')));
    }
    if (b == null) {
      return const ScaffoldPage(content: Center(child: ProgressRing()));
    }

    final pages = <NavigationPaneItem>[
      PaneItem(
        icon: const Icon(FluentIcons.home),
        title: const Text('首页'),
        body: HomePage(backend: b),
      ),
      PaneItem(
        icon: const Icon(FluentIcons.all_apps),
        title: const Text('分类'),
        body: CategoriesPage(backend: b),
      ),
      PaneItem(
        icon: const Icon(FluentIcons.play),
        title: const Text('直播'),
        body: LiveDecodePage(backend: b),
      ),
      PaneItem(
        icon: const Icon(FluentIcons.comment),
        title: const Text('弹幕'),
        body: DanmakuPage(backend: b),
      ),
      PaneItem(
        icon: const Icon(FluentIcons.text_document),
        title: const Text('歌词'),
        body: LyricsPage(backend: b),
      ),
      PaneItem(
        icon: const Icon(FluentIcons.music_note),
        title: const Text('歌曲'),
        body: MusicPage(backend: b),
      ),
      PaneItem(
        icon: const Icon(FluentIcons.text_document),
        title: const Text('字幕'),
        body: SubtitlesPage(backend: b),
      ),
    ];

    final footer = <NavigationPaneItem>[
      PaneItem(
        icon: const Icon(FluentIcons.settings),
        title: const Text('设置'),
        body: SettingsPage(backend: b),
      ),
    ];

    return NavigationView(
      pane: NavigationPane(
        selected: _idx,
        onChanged: (i) => setState(() => _idx = i),
        displayMode: PaneDisplayMode.compact,
        items: pages,
        footerItems: footer,
      ),
    );
  }
}
