import 'dart:async';

import 'package:flutter/material.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/models/live_directory.dart';
import '../live/live_decode_page.dart';
import '../widgets/material_error_card.dart';
import '../widgets/room_card.dart';

class CategoryRoomsPage extends StatefulWidget {
  const CategoryRoomsPage({
    super.key,
    required this.backend,
    required this.site,
    required this.parentId,
    required this.parentName,
    required this.sub,
  });

  final ChaosBackend backend;
  final String site;
  final String parentId;
  final String parentName;
  final LiveDirSubCategory sub;

  @override
  State<CategoryRoomsPage> createState() => _CategoryRoomsPageState();
}

class _CategoryRoomsPageState extends State<CategoryRoomsPage> {
  bool _loading = false;
  bool _loadingMore = false;
  String? _err;

  List<LiveDirRoomCard> _items = const [];
  int _page = 1;
  bool _hasMore = true;

  late final ScrollController _sc = ScrollController()..addListener(_onScroll);

  @override
  void initState() {
    super.initState();
    unawaited(_load(reset: true));
  }

  @override
  void dispose() {
    _sc.removeListener(_onScroll);
    _sc.dispose();
    super.dispose();
  }

  void _onScroll() {
    if (!_hasMore || _loading || _loadingMore) return;
    if (!_sc.hasClients) return;
    final pos = _sc.position;
    if (pos.maxScrollExtent <= 0) return;
    if (pos.pixels >= pos.maxScrollExtent - 400) {
      unawaited(_loadMore());
    }
  }

  Future<void> _load({required bool reset}) async {
    setState(() {
      if (reset) {
        _loading = true;
        _loadingMore = false;
      } else {
        _loadingMore = true;
      }
      _err = null;
    });

    try {
      final page = reset ? 1 : _page;
      final res = await widget.backend.categoryRooms(
        widget.site,
        widget.parentId,
        widget.sub.id,
        page,
      );
      if (!mounted) return;
      setState(() {
        _items = reset ? res.items : [..._items, ...res.items];
        _hasMore = res.hasMore && res.items.isNotEmpty;
        if (reset) {
          _page = 2;
        } else if (_hasMore) {
          _page += 1;
        }
      });
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _loadingMore = false;
      });
    }
  }

  Future<void> _loadMore() => _load(reset: false);

  @override
  Widget build(BuildContext context) {
    final w = MediaQuery.sizeOf(context).width;
    var crossAxisCount = (w / 200).floor();
    if (crossAxisCount < 2) crossAxisCount = 2;

    final title =
        widget.sub.name.trim().isEmpty ? '分类' : widget.sub.name.trim();

    return Scaffold(
      appBar: AppBar(
        title: Text(title),
      ),
      body: RefreshIndicator(
        onRefresh: () => _load(reset: true),
        child: CustomScrollView(
          controller: _sc,
          slivers: [
            SliverPadding(
              padding: const EdgeInsets.all(12),
              sliver: SliverToBoxAdapter(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      widget.parentName,
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                    const SizedBox(height: 6),
                    if (_err != null)
                      MaterialErrorCard(
                        message: _err!,
                        onDismiss: () => setState(() => _err = null),
                      ),
                    if (_loading) const LinearProgressIndicator(),
                    const SizedBox(height: 8),
                  ],
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
                  childAspectRatio: 16 / 14.5,
                ),
                delegate: SliverChildBuilderDelegate(
                  (context, i) {
                    final r = _items[i];
                    return RoomCard(
                      room: r,
                      onTap: () async {
                        await Navigator.of(context).push(
                          MaterialPageRoute(
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
                  childCount: _items.length,
                ),
              ),
            ),
            SliverPadding(
              padding: const EdgeInsets.all(12),
              sliver: SliverToBoxAdapter(
                child: _loadingMore
                    ? const Padding(
                        padding: EdgeInsets.symmetric(vertical: 12),
                        child: Center(child: CircularProgressIndicator()),
                      )
                    : (!_hasMore && _items.isNotEmpty)
                        ? Padding(
                            padding: const EdgeInsets.symmetric(vertical: 12),
                            child: Center(
                              child: Text(
                                '没有更多了',
                                style: Theme.of(context).textTheme.bodySmall,
                              ),
                            ),
                          )
                        : const SizedBox(height: 12),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
