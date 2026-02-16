import 'dart:async';
import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/backend/ffi_backend.dart';
import '../../core/models/subtitles.dart';
import '../widgets/material_error_card.dart';

class SubtitlesPage extends StatefulWidget {
  const SubtitlesPage({super.key, required this.backend});
  final ChaosBackend backend;

  @override
  State<SubtitlesPage> createState() => _SubtitlesPageState();
}

class _SubtitlesPageState extends State<SubtitlesPage> {
  final _q = TextEditingController();
  bool _loading = false;
  String? _err;
  List<ThunderSubtitleItem> _items = const [];

  @override
  void dispose() {
    _q.dispose();
    super.dispose();
  }

  Future<void> _search() async {
    final query = _q.text.trim();
    if (query.isEmpty) return;
    setState(() {
      _loading = true;
      _err = null;
      _items = const [];
    });
    try {
      final items = await widget.backend.subtitleSearch(SubtitleSearchParams(
        query: query,
        limit: 20,
        minScore: null,
        lang: null,
        timeoutMs: 10000,
      ));
      if (!mounted) return;
      setState(() => _items = items);
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Future<void> _download(ThunderSubtitleItem item) async {
    setState(() {
      _loading = true;
      _err = null;
    });
    try {
      final outDir = Platform.isAndroid
          ? await FfiChaosBackend.defaultAndroidDownloadDir()
          : Directory.current.path;
      final reply =
          await widget.backend.subtitleDownload(SubtitleDownloadParams(
        item: item,
        outDir: outDir,
        timeoutMs: 20000,
        retries: 2,
        overwrite: false,
      ));
      if (!mounted) return;
      final msg = '已下载: ${reply.path} (${reply.bytes} bytes)';
      if (Platform.isWindows) {
        await showDialog(
          context: context,
          builder: (_) => fluent.ContentDialog(
            title: const fluent.Text('字幕下载'),
            content: fluent.Text(msg),
            actions: [
              fluent.Button(
                  child: const fluent.Text('OK'),
                  onPressed: () => Navigator.pop(context))
            ],
          ),
        );
      } else {
        if (!mounted) return;
        ScaffoldMessenger.of(context)
            .showSnackBar(SnackBar(content: Text(msg)));
      }
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (Platform.isWindows) {
      return fluent.ScaffoldPage(
        header: fluent.PageHeader(title: const fluent.Text('字幕')),
        content: fluent.Padding(
          padding: const EdgeInsets.all(12),
          child: fluent.Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              fluent.Row(children: [
                fluent.Expanded(
                  child: fluent.TextBox(
                    controller: _q,
                    placeholder: '输入关键字',
                    onSubmitted: (_) => _search(),
                  ),
                ),
                const SizedBox(width: 8),
                fluent.Button(
                  onPressed: _loading ? null : _search,
                  child: const fluent.Text('搜索'),
                ),
              ]),
              const SizedBox(height: 12),
              if (_err != null)
                fluent.InfoBar(
                    title: const fluent.Text('错误'),
                    content: fluent.Text(_err!),
                    severity: fluent.InfoBarSeverity.error),
              if (_loading) const fluent.ProgressRing(),
              fluent.Expanded(
                child: fluent.ListView.builder(
                  itemCount: _items.length,
                  itemBuilder: (context, i) {
                    final it = _items[i];
                    final lang = it.languages.join(',');
                    return fluent.ListTile.selectable(
                      title: fluent.Text(it.name),
                      subtitle: fluent.Text(
                          'score=${it.score.toStringAsFixed(2)}  lang=$lang  ext=${it.ext}'),
                      trailing: fluent.Button(
                        onPressed: _loading ? null : () => _download(it),
                        child: const fluent.Text('下载'),
                      ),
                    );
                  },
                ),
              ),
            ],
          ),
        ),
      );
    }

    return Scaffold(
      appBar: AppBar(title: const Text('字幕')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          children: [
            Row(children: [
              Expanded(
                child: TextField(
                  controller: _q,
                  decoration: const InputDecoration(labelText: '关键字'),
                  onSubmitted: (_) => _search(),
                ),
              ),
              const SizedBox(width: 8),
              FilledButton(
                  onPressed: _loading ? null : _search,
                  child: const Text('搜索')),
            ]),
            const SizedBox(height: 12),
            if (_err != null)
              MaterialErrorCard(
                  message: _err!, onDismiss: () => setState(() => _err = null)),
            if (_loading) const LinearProgressIndicator(),
            Expanded(
              child: ListView.builder(
                itemCount: _items.length,
                itemBuilder: (context, i) {
                  final it = _items[i];
                  return Card(
                    child: ListTile(
                      title: Text(it.name,
                          maxLines: 2, overflow: TextOverflow.ellipsis),
                      subtitle: Text(
                          'score=${it.score.toStringAsFixed(2)}  ${it.languages.join(',')}  .${it.ext}'),
                      trailing: IconButton(
                        icon: const Icon(Icons.download),
                        onPressed: _loading ? null : () => _download(it),
                      ),
                    ),
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}
