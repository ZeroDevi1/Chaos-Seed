import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'lsp_framing.dart';

class JsonRpcNotification {
  final String method;
  final dynamic params;
  const JsonRpcNotification(this.method, this.params);
}

class JsonRpcLspClient {
  JsonRpcLspClient({
    required this.executable,
    required this.args,
    required this.authToken,
  });

  final String executable;
  final List<String> args;
  final String authToken;

  Process? _proc;
  late final LspFrameDecoder _decoder = LspFrameDecoder();
  late final StreamSubscription<List<int>> _stdoutSub;

  final _pending = <int, Completer<dynamic>>{};
  int _nextId = 1;

  final _notifCtrl = StreamController<JsonRpcNotification>.broadcast();
  Stream<JsonRpcNotification> get notifications => _notifCtrl.stream;

  Future<void> start() async {
    if (_proc != null) return;
    _proc = await Process.start(
      executable,
      [...args, '--stdio', '--auth-token', authToken],
      mode: ProcessStartMode.detachedWithStdio,
      runInShell: false,
    );

    _stdoutSub = _proc!.stdout.listen(_onStdoutBytes, onDone: () {
      _failAll(StateError('daemon stdout closed'));
    });
    _proc!.stderr.listen((_) {
      // reserved for logs; ignore here.
    });

    // Must authenticate.
    await invoke('daemon.ping', {'authToken': authToken});
  }

  void _onStdoutBytes(List<int> chunk) {
    for (final frame in _decoder.push(chunk)) {
      try {
        final obj = jsonDecode(utf8.decode(frame));
        if (obj is! Map) continue;
        final m = obj.cast<String, dynamic>();
        final id = m['id'];
        if (id != null) {
          final c = _pending.remove(_asIntId(id));
          if (c == null) continue;
          if (m['error'] != null) {
            c.completeError(m['error']);
          } else {
            c.complete(m['result']);
          }
          continue;
        }
        final method = m['method'];
        if (method is String) {
          _notifCtrl.add(JsonRpcNotification(method, m['params']));
        }
      } catch (_) {
        // ignore malformed frames
      }
    }
  }

  int _asIntId(dynamic id) {
    if (id is int) return id;
    if (id is num) return id.toInt();
    if (id is String) return int.tryParse(id) ?? 0;
    return 0;
  }

  Future<dynamic> invoke(String method, Map<String, dynamic> params) async {
    await start();
    final id = _nextId++;
    final c = Completer<dynamic>();
    _pending[id] = c;
    final payload = jsonEncode({
      'jsonrpc': '2.0',
      'id': id,
      'method': method,
      'params': params,
    });
    _proc!.stdin.add(encodeLspFrame(payload));
    await _proc!.stdin.flush();
    return c.future;
  }

  void _failAll(Object err) {
    for (final c in _pending.values) {
      if (!c.isCompleted) c.completeError(err);
    }
    _pending.clear();
  }

  Future<void> dispose() async {
    final p = _proc;
    _proc = null;
    if (p == null) return;
    try {
      p.kill(ProcessSignal.sigterm);
    } catch (_) {}
    await _stdoutSub.cancel();
    _failAll(StateError('daemon disposed'));
    await _notifCtrl.close();
  }
}
