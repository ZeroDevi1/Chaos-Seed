import 'dart:async';
import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/models/live.dart';
import '../widgets/material_error_card.dart';
import 'live_player_page.dart';

class LiveDecodePage extends StatefulWidget {
  const LiveDecodePage({
    super.key,
    required this.backend,
    this.initialInput,
    this.autoDecode = false,
  });

  final ChaosBackend backend;
  final String? initialInput;
  final bool autoDecode;

  @override
  State<LiveDecodePage> createState() => _LiveDecodePageState();
}

class _LiveDecodePageState extends State<LiveDecodePage> {
  late final TextEditingController _input =
      TextEditingController(text: (widget.initialInput ?? '').trim());

  bool _loading = false;
  String? _err;
  LivestreamDecodeManifestResult? _man;

  @override
  void initState() {
    super.initState();
    if (widget.autoDecode && _input.text.trim().isNotEmpty) {
      unawaited(_decode());
    }
  }

  @override
  void dispose() {
    _input.dispose();
    super.dispose();
  }

  Future<void> _decode() async {
    final s = _input.text.trim();
    if (s.isEmpty) return;
    setState(() {
      _loading = true;
      _err = null;
      _man = null;
    });
    try {
      final man = await widget.backend.decodeManifest(s);
      if (!mounted) return;
      setState(() => _man = man);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Future<void> _openVariant(LivestreamVariant v) async {
    final s = _input.text.trim();
    if (s.isEmpty) return;
    final variants = _man?.variants ?? const <LivestreamVariant>[];

    if (Platform.isWindows) {
      await Navigator.of(context).push(
        fluent.FluentPageRoute(
          builder: (_) => LivePlayerPage(
            backend: widget.backend,
            input: s,
            variants: variants,
            initialVariantId: v.id,
          ),
        ),
      );
      return;
    }

    await Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => LivePlayerPage(
          backend: widget.backend,
          input: s,
          variants: variants,
          initialVariantId: v.id,
        ),
      ),
    );
  }

  Widget _buildVariantList() {
    final man = _man;
    if (man == null) {
      return Text(
        '请输入直播间地址并点击“解析”。',
        style: TextStyle(color: Platform.isWindows ? null : Colors.grey),
      );
    }

    final variants = [...man.variants]
      ..sort((a, b) => b.quality.compareTo(a.quality));

    if (Platform.isWindows) {
      return GridView.builder(
        gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
          maxCrossAxisExtent: 260,
          mainAxisSpacing: 10,
          crossAxisSpacing: 10,
          childAspectRatio: 2.6,
        ),
        itemCount: variants.length,
        itemBuilder: (context, i) {
          final v = variants[i];
          return fluent.Button(
            onPressed: _loading ? null : () => _openVariant(v),
            child: fluent.Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                fluent.Text(v.label,
                    style: const fluent.TextStyle(fontWeight: FontWeight.w600)),
                fluent.Text('quality=${v.quality}',
                    style: const fluent.TextStyle(fontSize: 12)),
              ],
            ),
          );
        },
      );
    }

    return ListView.builder(
      itemCount: variants.length,
      itemBuilder: (context, i) {
        final v = variants[i];
        return Card(
          child: ListTile(
            title: Text(v.label,
                style: const TextStyle(fontWeight: FontWeight.w600)),
            subtitle: Text('清晰度：${v.quality}'),
            trailing: const Icon(Icons.play_arrow),
            onTap: _loading ? null : () => _openVariant(v),
          ),
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final man = _man;
    if (Platform.isWindows) {
      return fluent.ScaffoldPage(
        header: fluent.PageHeader(title: const fluent.Text('直播')),
        content: fluent.Padding(
          padding: const EdgeInsets.all(12),
          child: fluent.Column(
            children: [
              fluent.Row(children: [
                fluent.Expanded(
                  child: fluent.TextBox(
                    controller: _input,
                    placeholder: '输入直播间地址',
                    onSubmitted: (_) => _decode(),
                  ),
                ),
                const fluent.SizedBox(width: 8),
                fluent.Button(
                    onPressed: _loading ? null : _decode,
                    child: const fluent.Text('解析')),
              ]),
              const fluent.SizedBox(height: 8),
              if (_err != null)
                fluent.InfoBar(
                  title: const fluent.Text('错误'),
                  content: fluent.Text(_err!),
                  severity: fluent.InfoBarSeverity.error,
                ),
              if (_loading) const fluent.ProgressRing(),
              if (man != null) ...[
                const fluent.SizedBox(height: 8),
                fluent.InfoBar(
                  title: fluent.Text(
                      man.info.title.isEmpty ? '已解析' : man.info.title),
                  content: fluent.Text(
                      '${man.site}:${man.roomId}  变体数=${man.variants.length}'),
                  severity: fluent.InfoBarSeverity.info,
                ),
              ],
              const fluent.SizedBox(height: 8),
              fluent.Expanded(child: _buildVariantList()),
            ],
          ),
        ),
      );
    }

    return Scaffold(
      appBar: AppBar(title: const Text('链接解析')),
      body: ListView(
        padding: const EdgeInsets.all(12),
        children: [
          Card(
            child: Theme(
              data:
                  Theme.of(context).copyWith(dividerColor: Colors.transparent),
              child: ExpansionTile(
                title: const Text('直播间解析'),
                initiallyExpanded: true,
                childrenPadding: const EdgeInsets.fromLTRB(12, 0, 12, 12),
                children: [
                  TextField(
                    controller: _input,
                    minLines: 2,
                    maxLines: 3,
                    textInputAction: TextInputAction.go,
                    decoration: InputDecoration(
                      border: const OutlineInputBorder(),
                      hintText: '输入或粘贴哔哩哔哩/虎牙/斗鱼直播链接或 roomId',
                      contentPadding: const EdgeInsets.all(12),
                      enabledBorder: OutlineInputBorder(
                        borderSide:
                            BorderSide(color: Colors.grey.withAlpha(50)),
                      ),
                    ),
                    onSubmitted: (_) => _decode(),
                  ),
                  const SizedBox(height: 8),
                  SizedBox(
                    width: double.infinity,
                    child: TextButton.icon(
                      onPressed: _loading ? null : _decode,
                      icon: const Icon(Icons.play_circle_outline),
                      label: const Text('解析'),
                    ),
                  ),
                ],
              ),
            ),
          ),
          if (_err != null)
            MaterialErrorCard(
                message: _err!, onDismiss: () => setState(() => _err = null)),
          if (_loading) const LinearProgressIndicator(),
          if (man != null) ...[
            Card(
              child: ListTile(
                title: Text(man.info.title.isEmpty ? '已解析' : man.info.title),
                subtitle: Text(
                    '${man.site}:${man.roomId}  清晰度选项=${man.variants.length}'),
              ),
            ),
            ...([...man.variants]
                  ..sort((a, b) => b.quality.compareTo(a.quality)))
                .map(
              (v) => Card(
                child: ListTile(
                  title: Text(v.label,
                      style: const TextStyle(fontWeight: FontWeight.w600)),
                  subtitle: Text('清晰度：${v.quality}'),
                  trailing: const Icon(Icons.play_arrow),
                  onTap: _loading ? null : () => _openVariant(v),
                ),
              ),
            ),
            const SizedBox(height: 12),
          ],
          if (man == null) ...[
            const SizedBox(height: 8),
            Card(
              child: ListTile(
                leading: const Icon(Icons.info_outline),
                title: const Text('提示'),
                subtitle: const Text('解析后会显示清晰度列表；点击即可进入播放页。'),
              ),
            ),
          ],
        ],
      ),
    );
  }
}
