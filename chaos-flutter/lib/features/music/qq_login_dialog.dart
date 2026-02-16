import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';

import '../../core/backend/chaos_backend.dart';

enum _QqLoginType {
  qq('qq', 'QQ'),
  wechat('wechat', '微信');

  const _QqLoginType(this.id, this.label);
  final String id;
  final String label;
}

class QqLoginDialog extends StatefulWidget {
  const QqLoginDialog({super.key, required this.backend});

  final ChaosBackend backend;

  @override
  State<QqLoginDialog> createState() => _QqLoginDialogState();
}

class _QqLoginDialogState extends State<QqLoginDialog> {
  _QqLoginType _type = _QqLoginType.qq;
  bool _loading = false;
  String? _err;

  // QR
  String? _sessionId;
  String? _mime;
  String? _base64;
  String _state = 'init';
  Timer? _pollTimer;

  @override
  void initState() {
    super.initState();
    unawaited(_createQrAndStartPoll());
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    super.dispose();
  }

  Future<void> _createQrAndStartPoll() async {
    if (!Platform.isAndroid &&
        !Platform.isWindows &&
        !Platform.isLinux &&
        !Platform.isMacOS) return;
    setState(() {
      _loading = true;
      _err = null;
      _sessionId = null;
      _mime = null;
      _base64 = null;
      _state = 'init';
    });

    try {
      final qr = await widget.backend.qqLoginQrCreate(_type.id);
      if (!mounted) return;
      setState(() {
        _sessionId = qr.sessionId;
        _mime = qr.mime;
        _base64 = qr.base64;
        _state = 'scan';
      });

      _pollTimer?.cancel();
      _pollTimer = Timer.periodic(
          const Duration(seconds: 2), (_) => unawaited(_pollOnce()));
    } catch (e) {
      if (!mounted) return;
      setState(() => _err = e.toString());
    } finally {
      if (!mounted) return;
      setState(() => _loading = false);
    }
  }

  Future<void> _pollOnce() async {
    final sid = _sessionId;
    if (sid == null || sid.trim().isEmpty) return;
    try {
      final r = await widget.backend.qqLoginQrPoll(sid);
      if (!mounted) return;
      setState(() => _state = r.state);

      if (r.cookie != null && (r.state == 'done' || r.state == 'Done')) {
        _pollTimer?.cancel();
        final cookieJson = jsonEncode(r.cookie!.toJson());
        if (!mounted) return;
        Navigator.of(context).pop(cookieJson);
      }
    } catch (e) {
      // 轮询失败不要直接退出，允许临时网络抖动。
      if (!mounted) return;
      setState(() => _err = e.toString());
    }
  }

  String _stateLabel(String raw) {
    switch (raw) {
      case 'scan':
        return '等待扫码';
      case 'confirm':
        return '请在手机确认登录';
      case 'done':
        return '登录成功';
      case 'timeout':
        return '二维码已过期';
      case 'refuse':
        return '已拒绝登录';
      default:
        return '状态：$raw';
    }
  }

  @override
  Widget build(BuildContext context) {
    final bytes = (_base64 == null || _base64!.trim().isEmpty)
        ? null
        : base64Decode(_base64!);

    return AlertDialog(
      title: const Text('QQ 音乐扫码登录'),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              children: [
                const Text('登录方式：'),
                const SizedBox(width: 8),
                DropdownButton<_QqLoginType>(
                  value: _type,
                  items: _QqLoginType.values
                      .map((t) =>
                          DropdownMenuItem(value: t, child: Text(t.label)))
                      .toList(),
                  onChanged: _loading
                      ? null
                      : (v) {
                          if (v == null || v == _type) return;
                          setState(() => _type = v);
                          unawaited(_createQrAndStartPoll());
                        },
                ),
                const Spacer(),
                IconButton(
                  tooltip: '刷新二维码',
                  onPressed: _loading ? null : _createQrAndStartPoll,
                  icon: const Icon(Icons.refresh),
                ),
              ],
            ),
            const SizedBox(height: 12),
            if (_err != null)
              Container(
                width: double.infinity,
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: Theme.of(context).colorScheme.errorContainer,
                  borderRadius: BorderRadius.circular(12),
                ),
                child: Text(
                  _err!,
                  style: TextStyle(
                      color: Theme.of(context).colorScheme.onErrorContainer),
                ),
              ),
            if (_loading) ...[
              const SizedBox(height: 12),
              const LinearProgressIndicator(),
            ],
            const SizedBox(height: 12),
            if (bytes != null)
              ClipRRect(
                borderRadius: BorderRadius.circular(12),
                child: Container(
                  color: Colors.white,
                  padding: const EdgeInsets.all(12),
                  child: Image.memory(bytes,
                      width: 240, height: 240, fit: BoxFit.contain),
                ),
              )
            else
              const SizedBox(
                  height: 240, child: Center(child: Text('二维码生成中...'))),
            const SizedBox(height: 12),
            Align(
              alignment: Alignment.centerLeft,
              child: Text(
                _stateLabel(_state),
                style: Theme.of(context).textTheme.bodyMedium,
              ),
            ),
            if (_mime != null && _mime!.trim().isNotEmpty)
              Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  '格式：$_mime',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(null),
          child: const Text('取消'),
        ),
      ],
    );
  }
}
