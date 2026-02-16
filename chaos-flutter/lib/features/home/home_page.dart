import 'dart:async';
import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/models/live_directory.dart';
import '../widgets/room_card.dart';
import '../live/live_decode_page.dart';
import 'home_rooms_tab.dart';

class HomePage extends StatefulWidget {
  const HomePage({super.key, required this.backend});
  final ChaosBackend backend;

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  static const _sites = <String, String>{
    'bili_live': '哔哩哔哩',
    'huya': '虎牙直播',
    'douyu': '斗鱼直播',
  };

  static const _siteIcons = <String, String>{
    'bili_live': 'assets/images/bilibili.png',
    'huya': 'assets/images/huya.png',
    'douyu': 'assets/images/douyu.png',
  };

  String _site = 'bili_live';
  final _q = TextEditingController();
  bool _loading = false;
  String? _err;
  List<LiveDirRoomCard> _items = const [];

  @override
  void initState() {
    super.initState();
    // Android 端用 TabBarView 各自加载，不在这里触发一次全局加载。
    if (Platform.isWindows) {
      unawaited(_load());
    }
  }

  @override
  void dispose() {
    _q.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _err = null;
    });
    try {
      final res = await widget.backend.recommendRooms(_site, 1);
      if (!mounted) return;
      setState(() => _items = res.items);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Future<void> _search() async {
    final kw = _q.text.trim();
    if (kw.isEmpty) return;
    setState(() {
      _loading = true;
      _err = null;
    });
    try {
      final res = await widget.backend.searchRooms(_site, kw, 1);
      if (!mounted) return;
      setState(() => _items = res.items);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Widget _windowsSiteTabs() {
    return fluent.Row(
      children: [
        for (final e in _sites.entries)
          fluent.Padding(
            padding: const EdgeInsets.only(right: 8),
            child: fluent.ToggleButton(
              checked: _site == e.key,
              onChanged: (_) {
                setState(() => _site = e.key);
                unawaited(_load());
              },
              child: fluent.Text(e.value),
            ),
          ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    if (Platform.isWindows) {
      return fluent.ScaffoldPage(
        header: fluent.PageHeader(title: const fluent.Text('首页')),
        content: fluent.Padding(
          padding: const EdgeInsets.all(12),
          child: fluent.Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _windowsSiteTabs(),
              const fluent.SizedBox(height: 10),
              fluent.Row(
                children: [
                  fluent.Expanded(
                    child: fluent.TextBox(
                      controller: _q,
                      placeholder: '搜索直播间',
                      onSubmitted: (_) => _search(),
                    ),
                  ),
                  const fluent.SizedBox(width: 8),
                  fluent.IconButton(
                    icon: const Icon(fluent.FluentIcons.search),
                    onPressed: _loading ? null : _search,
                  ),
                  fluent.IconButton(
                    icon: const Icon(fluent.FluentIcons.clear),
                    onPressed: _loading
                        ? null
                        : () {
                            _q.clear();
                            unawaited(_load());
                          },
                  ),
                  const fluent.SizedBox(width: 8),
                  fluent.Button(
                      onPressed: _loading ? null : _load,
                      child: const fluent.Text('刷新')),
                ],
              ),
              const fluent.SizedBox(height: 8),
              if (_err != null)
                fluent.InfoBar(
                  title: const fluent.Text('错误'),
                  content: fluent.Text(_err!),
                  severity: fluent.InfoBarSeverity.error,
                ),
              if (_loading) const fluent.ProgressRing(),
              fluent.Expanded(
                child: GridView.builder(
                  gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
                    maxCrossAxisExtent: 360,
                    mainAxisSpacing: 12,
                    crossAxisSpacing: 12,
                    // 稍微加高卡片，避免小窗口/高列数时文字区溢出。
                    childAspectRatio: 16 / 14.5,
                  ),
                  itemCount: _items.length,
                  itemBuilder: (context, i) {
                    final r = _items[i];
                    return RoomCard(
                      room: r,
                      onTap: () async {
                        await Navigator.of(context).push(
                          fluent.FluentPageRoute(
                            builder: (_) => LiveDecodePage(
                              backend: widget.backend,
                              initialInput: r.input,
                              autoDecode: true,
                            ),
                          ),
                        );
                      },
                    );
                  },
                ),
              ),
            ],
          ),
        ),
      );
    }

    final keys = _sites.keys.toList(growable: false);
    return DefaultTabController(
      length: keys.length,
      child: Builder(
        builder: (context) {
          return Scaffold(
            appBar: AppBar(
              titleSpacing: 8,
              title: TabBar(
                isScrollable: true,
                indicatorSize: TabBarIndicatorSize.label,
                tabs: [
                  for (final k in keys)
                    Tab(
                      child: Row(
                        children: [
                          Image.asset(
                            _siteIcons[k] ?? '',
                            width: 20,
                            height: 20,
                            errorBuilder: (context, _, __) =>
                                const SizedBox(width: 20, height: 20),
                          ),
                          const SizedBox(width: 8),
                          Text(_sites[k] ?? k),
                        ],
                      ),
                    ),
                ],
              ),
              actions: [
                IconButton(
                  tooltip: '搜索/解析',
                  icon: const Icon(Icons.search),
                  onPressed: () {
                    Navigator.of(context).push(
                      MaterialPageRoute(
                          builder: (_) =>
                              LiveDecodePage(backend: widget.backend)),
                    );
                  },
                ),
              ],
            ),
            body: TabBarView(
              children: [
                for (final k in keys)
                  HomeRoomsTab(
                    backend: widget.backend,
                    site: k,
                  ),
              ],
            ),
          );
        },
      ),
    );
  }
}
