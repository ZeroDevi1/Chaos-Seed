import 'dart:async';
import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/models/live_directory.dart';
import '../widgets/material_error_card.dart';
import '../widgets/network_image_view.dart';
import '../widgets/room_card.dart';
import '../live/live_decode_page.dart';
import 'category_rooms_page.dart';

class CategoriesPage extends StatefulWidget {
  const CategoriesPage({super.key, required this.backend});
  final ChaosBackend backend;

  @override
  State<CategoriesPage> createState() => _CategoriesPageState();
}

class _CategoriesPageState extends State<CategoriesPage> {
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
  bool _loading = false;
  String? _err;

  List<LiveDirCategory> _cats = const [];
  LiveDirSubCategory? _activeSub;
  String? _activeParentId;
  List<LiveDirRoomCard> _rooms = const [];
  int _page = 1;
  bool _hasMore = true;

  @override
  void initState() {
    super.initState();
    unawaited(_loadCategories());
  }

  @override
  void dispose() {
    super.dispose();
  }

  Future<void> _loadCategories() async {
    setState(() {
      _loading = true;
      _err = null;
      _cats = const [];
      _activeSub = null;
      _rooms = const [];
      _page = 1;
      _hasMore = true;
    });
    try {
      final cats = await widget.backend.categories(_site);
      if (!mounted) return;
      setState(() => _cats = cats);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Future<void> _selectSub(
      LiveDirCategory parent, LiveDirSubCategory sub) async {
    setState(() {
      _activeParentId = parent.id;
      _activeSub = sub;
      _rooms = const [];
      _page = 1;
      _hasMore = true;
    });
    await _loadMoreRooms();
  }

  Future<void> _loadMoreRooms() async {
    if (_loading || !_hasMore) return;
    final sub = _activeSub;
    if (sub == null) return;
    setState(() {
      _loading = true;
      _err = null;
    });
    try {
      final res = await widget.backend
          .categoryRooms(_site, _activeParentId, sub.id, _page);
      final next = [..._rooms, ...res.items];
      if (!mounted) return;
      setState(() {
        _rooms = next;
        _hasMore = res.hasMore && res.items.isNotEmpty;
        if (_hasMore) _page += 1;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Widget _buildCategoryCard(LiveDirCategory parent, LiveDirSubCategory sub) {
    return Card(
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: () async {
          await Navigator.of(context).push(
            MaterialPageRoute(
              builder: (_) => CategoryRoomsPage(
                backend: widget.backend,
                site: _site,
                parentId: parent.id,
                parentName: parent.name,
                sub: sub,
              ),
            ),
          );
        },
        child: Stack(
          fit: StackFit.expand,
          children: [
            NetworkImageView(url: sub.pic, borderRadius: 0),
            Positioned(
              left: 0,
              right: 0,
              bottom: 0,
              child: Container(
                decoration: BoxDecoration(
                  gradient: LinearGradient(
                    begin: Alignment.bottomCenter,
                    end: Alignment.topCenter,
                    colors: [
                      Colors.black.withOpacity(0.75),
                      Colors.transparent,
                    ],
                  ),
                ),
                padding: const EdgeInsets.fromLTRB(10, 14, 10, 10),
                child: Text(
                  sub.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                    color: Colors.white,
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
            ),
            Positioned(
              left: 10,
              top: 10,
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                decoration: BoxDecoration(
                  color: Colors.black.withOpacity(0.45),
                  borderRadius: BorderRadius.circular(999),
                ),
                child: Text(
                  parent.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                    color: Colors.white,
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _sitePicker() {
    const sites = <String>['bili_live', 'huya', 'douyu'];
    if (Platform.isWindows) {
      return fluent.Row(
        children: [
          for (final s in sites)
            fluent.Padding(
              padding: const EdgeInsets.only(right: 8),
              child: fluent.ToggleButton(
                checked: _site == s,
                onChanged: (_) {
                  setState(() => _site = s);
                  unawaited(_loadCategories());
                },
                child: fluent.Text(_sites[s] ?? s),
              ),
            ),
        ],
      );
    }
    return DropdownButton<String>(
      value: _site,
      items: sites
          .map((s) => DropdownMenuItem(value: s, child: Text(_sites[s] ?? s)))
          .toList(),
      onChanged: (v) {
        if (v == null) return;
        setState(() => _site = v);
        unawaited(_loadCategories());
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    if (Platform.isWindows) {
      return fluent.ScaffoldPage(
        header: fluent.PageHeader(
            title: const fluent.Text('分类'), commandBar: _sitePicker()),
        content: fluent.Padding(
          padding: const EdgeInsets.all(12),
          child: fluent.Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (_err != null)
                fluent.InfoBar(
                  title: const fluent.Text('错误'),
                  content: fluent.Text(_err!),
                  severity: fluent.InfoBarSeverity.error,
                ),
              if (_loading) const fluent.ProgressRing(),
              fluent.Expanded(
                child: fluent.Row(
                  children: [
                    fluent.Expanded(
                      child: fluent.ListView.builder(
                        itemCount: _cats.length,
                        itemBuilder: (context, i) {
                          final c = _cats[i];
                          return fluent.Expander(
                            header: fluent.Text(c.name),
                            content: fluent.Column(
                              children: [
                                for (final sub in c.children)
                                  fluent.ListTile.selectable(
                                    title: fluent.Text(sub.name),
                                    onPressed: () => _selectSub(c, sub),
                                  ),
                              ],
                            ),
                          );
                        },
                      ),
                    ),
                    const fluent.SizedBox(width: 12),
                    fluent.Expanded(
                      child: fluent.Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          fluent.Text(_activeSub == null
                              ? '请选择分类'
                              : '房间: ${_activeSub!.name}'),
                          const fluent.SizedBox(height: 8),
                          fluent.Expanded(
                            child: GridView.builder(
                              gridDelegate:
                                  const SliverGridDelegateWithMaxCrossAxisExtent(
                                maxCrossAxisExtent: 360,
                                mainAxisSpacing: 12,
                                crossAxisSpacing: 12,
                                childAspectRatio: 16 / 14.5,
                              ),
                              itemCount: _rooms.length,
                              itemBuilder: (context, i) {
                                final r = _rooms[i];
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
                          if (_hasMore)
                            fluent.Button(
                              onPressed: _loading ? null : _loadMoreRooms,
                              child: const fluent.Text('加载更多'),
                            ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    }

    const keys = <String>['bili_live', 'huya', 'douyu'];
    return DefaultTabController(
      length: keys.length,
      child: Builder(
        builder: (context) {
          final w = MediaQuery.sizeOf(context).width;
          var crossAxisCount = (w / 200).floor();
          if (crossAxisCount < 2) crossAxisCount = 2;

          return Scaffold(
            appBar: AppBar(
              titleSpacing: 8,
              title: TabBar(
                isScrollable: true,
                indicatorSize: TabBarIndicatorSize.label,
                onTap: (i) {
                  final k = keys[i];
                  if (k == _site) return;
                  setState(() => _site = k);
                  unawaited(_loadCategories());
                },
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
            ),
            body: RefreshIndicator(
              onRefresh: _loadCategories,
              child: CustomScrollView(
                slivers: [
                  SliverPadding(
                    padding: const EdgeInsets.all(12),
                    sliver: SliverToBoxAdapter(
                      child: Column(
                        children: [
                          if (_err != null)
                            MaterialErrorCard(
                              message: _err!,
                              onDismiss: () => setState(() => _err = null),
                            ),
                          if (_loading) const LinearProgressIndicator(),
                        ],
                      ),
                    ),
                  ),
                  for (final c in _cats) ...[
                    SliverPadding(
                      padding: const EdgeInsets.fromLTRB(12, 4, 12, 8),
                      sliver: SliverToBoxAdapter(
                        child: Text(
                          c.name,
                          style: Theme.of(context).textTheme.titleSmall,
                        ),
                      ),
                    ),
                    SliverPadding(
                      padding: const EdgeInsets.symmetric(horizontal: 12),
                      sliver: SliverGrid(
                        gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                          crossAxisCount: crossAxisCount,
                          mainAxisSpacing: 12,
                          crossAxisSpacing: 12,
                          childAspectRatio: 16 / 9.5,
                        ),
                        delegate: SliverChildBuilderDelegate(
                          (context, i) {
                            final sub = c.children[i];
                            return _buildCategoryCard(c, sub);
                          },
                          childCount: c.children.length,
                        ),
                      ),
                    ),
                    const SliverToBoxAdapter(child: SizedBox(height: 12)),
                  ],
                ],
              ),
            ),
          );
        },
      ),
    );
  }
}
