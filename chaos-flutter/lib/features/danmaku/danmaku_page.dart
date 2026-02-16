import 'dart:async';
import 'dart:io';

import 'package:fluent_ui/fluent_ui.dart' as fluent;
import 'package:flutter/material.dart';

import '../../core/backend/chaos_backend.dart';
import '../../core/models/danmaku.dart';

class DanmakuPage extends StatefulWidget {
  const DanmakuPage({super.key, required this.backend});
  final ChaosBackend backend;

  @override
  State<DanmakuPage> createState() => _DanmakuPageState();
}

class _DanmakuPageState extends State<DanmakuPage> {
  final _input = TextEditingController();
  bool _loading = false;
  String? _err;

  String? _sessionId;
  StreamSubscription<DanmakuMessage>? _sub;
  final List<DanmakuMessage> _tail = [];

  @override
  void dispose() {
    _sub?.cancel();
    final sid = _sessionId;
    if (sid != null) {
      widget.backend.closeLive(sid).ignore();
    }
    _input.dispose();
    super.dispose();
  }

  Future<void> _connect() async {
    final s = _input.text.trim();
    if (s.isEmpty) return;
    setState(() {
      _loading = true;
      _err = null;
      _tail.clear();
    });
    try {
      // Best-effort: open live to establish a danmaku session, but we don't start playback here.
      await widget.backend.decodeManifest(s);
      final res = await widget.backend.openLive(s);
      _sub?.cancel();
      _sessionId = res.sessionId;
      _sub = widget.backend.danmakuStream(res.sessionId).listen((m) {
        setState(() {
          _tail.add(m);
          if (_tail.length > 500) {
            _tail.removeRange(0, _tail.length - 500);
          }
        });
      });
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
        header: fluent.PageHeader(title: const fluent.Text('弹幕')),
        content: fluent.Padding(
          padding: const EdgeInsets.all(12),
          child: fluent.Column(
            children: [
              fluent.Row(children: [
                fluent.Expanded(
                  child: fluent.TextBox(
                    controller: _input,
                    placeholder: '输入直播间地址（用于连接弹幕）',
                    onSubmitted: (_) => _connect(),
                  ),
                ),
                const fluent.SizedBox(width: 8),
                fluent.Button(
                    onPressed: _loading ? null : _connect,
                    child: const fluent.Text('连接')),
              ]),
              const fluent.SizedBox(height: 8),
              if (_err != null)
                fluent.InfoBar(
                  title: const fluent.Text('错误'),
                  content: fluent.Text(_err!),
                  severity: fluent.InfoBarSeverity.error,
                ),
              if (_loading) const fluent.ProgressRing(),
              fluent.Expanded(
                child: fluent.ListView(
                  children: [
                    for (final m in _tail.reversed.take(200))
                      fluent.Text('${m.user}: ${m.text}'),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    }

    return Scaffold(
      appBar: AppBar(title: const Text('弹幕')),
      body: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          children: [
            TextField(
              controller: _input,
              decoration: const InputDecoration(labelText: '直播间地址'),
              onSubmitted: (_) => _connect(),
            ),
            const SizedBox(height: 8),
            FilledButton(
                onPressed: _loading ? null : _connect, child: const Text('连接')),
            if (_err != null)
              Text(_err!, style: const TextStyle(color: Colors.red)),
            if (_loading) const LinearProgressIndicator(),
            Expanded(
              child: ListView(
                children: [
                  for (final m in _tail.reversed.take(200))
                    Text('${m.user}: ${m.text}')
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

extension _Ignore on Future<void> {
  void ignore() {}
}
