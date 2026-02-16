import 'dart:async';
import 'dart:ffi';
import 'dart:isolate';
import 'dart:io';

import 'package:ffi/ffi.dart';

import 'chaos_ffi_bindings.dart';

class FfiIsolateRunner {
  FfiIsolateRunner({DynamicLibrary? libraryOverride})
      : _libraryOverride = libraryOverride;

  final DynamicLibrary? _libraryOverride;

  Isolate? _isolate;
  SendPort? _toIsolate;
  late final ReceivePort _fromIsolate = ReceivePort();
  late final StreamSubscription _sub;

  final _pending = <int, Completer<dynamic>>{};
  int _nextId = 1;

  final _danmakuControllers = <String, StreamController<String>>{};

  Future<void> start() async {
    if (_toIsolate != null) return;

    final ready = Completer<SendPort>();
    _sub = _fromIsolate.listen((msg) {
      if (msg is SendPort) {
        if (!ready.isCompleted) ready.complete(msg);
        return;
      }
      if (msg is Map) {
        final m = msg.cast<String, dynamic>();
        final type = m['type'];
        if (type == 'resp') {
          final id = m['id'] as int;
          final ok = m['ok'] as bool;
          final c = _pending.remove(id);
          if (c == null) return;
          if (ok) {
            c.complete(m['result']);
          } else {
            c.completeError(ChaosFfiException(
              m['error'] as String? ?? 'ffi error',
              lastErrorJson: m['lastErrorJson'] as String?,
            ));
          }
          return;
        }
        if (type == 'danmaku') {
          final sid = (m['sessionId'] as String?) ?? '';
          final jsonEvents = (m['eventsJson'] as String?) ?? '[]';
          final ctrl = _danmakuControllers[sid];
          if (ctrl != null && !ctrl.isClosed) {
            ctrl.add(jsonEvents);
          }
          return;
        }
      }
    });

    _isolate = await Isolate.spawn(
      _entry,
      _IsolateInit(
        mainPort: _fromIsolate.sendPort,
        // We cannot send DynamicLibrary across isolates; only signal "override" via null.
        useOverride: _libraryOverride != null,
      ),
      debugName: 'chaos_ffi_isolate',
    );
    _toIsolate = await ready.future;

    if (_libraryOverride != null) {
      // For tests only: inject override by returning a "process" library in the isolate.
      // (Real overrides can't cross isolates safely.)
      await call('ffi.set_override', const {});
    }
  }

  Future<dynamic> call(String method, Map<String, dynamic> params) async {
    await start();
    final id = _nextId++;
    final c = Completer<dynamic>();
    _pending[id] = c;
    _toIsolate!.send({
      'type': 'req',
      'id': id,
      'method': method,
      'params': params,
    });
    return c.future;
  }

  Stream<String> danmakuEventsJson(String sessionId) {
    return _danmakuControllers.putIfAbsent(sessionId, () {
      return StreamController<String>.broadcast(onCancel: () {
        // no-op
      });
    }).stream;
  }

  Future<void> danmakuConnect(
      {required String sessionId, required String input}) async {
    await call('danmaku.connect', {'sessionId': sessionId, 'input': input});
  }

  Future<void> danmakuDisconnect({required String sessionId}) async {
    await call('danmaku.disconnect', {'sessionId': sessionId});
    final c = _danmakuControllers.remove(sessionId);
    if (c != null && !c.isClosed) {
      await c.close();
    }
  }

  Future<void> dispose() async {
    for (final sid in _danmakuControllers.keys.toList(growable: false)) {
      try {
        await danmakuDisconnect(sessionId: sid);
      } catch (_) {
        // ignore
      }
    }
    _pending.forEach((_, c) {
      if (!c.isCompleted) c.completeError(StateError('ffi runner disposed'));
    });
    _pending.clear();
    await _sub.cancel();
    _fromIsolate.close();
    _toIsolate = null;
    _isolate?.kill(priority: Isolate.immediate);
    _isolate = null;
  }

  // ---- isolate entry ----

  static void _entry(_IsolateInit init) {
    final recv = ReceivePort();
    init.mainPort.send(recv.sendPort);

    ChaosFfiBindings? ffi;
    final danmakuHandles = <String, Pointer<Void>>{};
    var pollBusy = false;

    List<String> windowsDylibCandidates(String fileName) {
      final exeDir = File(Platform.resolvedExecutable).parent;
      return <String>[
        // Next to the running executable (typical for packaged apps).
        File(exeDir.path + Platform.pathSeparator + fileName).path,
        // When running from repo root / chaos-flutter.
        Directory.current.uri.resolve(fileName).toFilePath(),
        Directory.current.uri.resolve('windows/deps/$fileName').toFilePath(),
        Directory.current.uri
            .resolve('chaos-flutter/windows/deps/$fileName')
            .toFilePath(),
        // When launched from build output.
        Directory.current.uri
            .resolve('build/windows/x64/runner/Debug/$fileName')
            .toFilePath(),
        Directory.current.uri
            .resolve('build/windows/x64/runner/Release/$fileName')
            .toFilePath(),
      ];
    }

    String? findWindowsDylibPath(String fileName) {
      final candidates = windowsDylibCandidates(fileName);
      for (final c in candidates) {
        if (File(c).existsSync()) return c;
      }
      return null;
    }

    void setDllDirectoryFor(String dllPath) {
      if (!Platform.isWindows) return;
      final dir = File(dllPath).parent.path;
      // Ensure dependent DLLs (if any) can also be resolved from the same folder.
      final k32 = DynamicLibrary.open('kernel32.dll');
      final setDllDir = k32.lookupFunction<Int32 Function(Pointer<Utf16>),
          int Function(Pointer<Utf16>)>('SetDllDirectoryW');
      final pDir = dir.toNativeUtf16();
      try {
        setDllDir(pDir);
      } finally {
        calloc.free(pDir);
      }
    }

    DynamicLibrary openLibrary() {
      // Library name:
      // - Windows: chaos_ffi.dll (opened by filename)
      // - Android/Linux/macOS: libchaos_ffi.so / .dylib
      if (Platform.isWindows) {
        final fileName = 'chaos_ffi.dll';
        final p = findWindowsDylibPath(fileName);
        if (p != null) {
          setDllDirectoryFor(p);
          return DynamicLibrary.open(p);
        }
        // Fallback: rely on PATH / executable directory.
        try {
          return DynamicLibrary.open(fileName);
        } catch (e) {
          final searched = windowsDylibCandidates(fileName).join('\n  - ');
          throw StateError('无法加载 $fileName：$e\n已尝试路径：\n  - $searched');
        }
      }
      if (Platform.isAndroid) {
        return DynamicLibrary.open('libchaos_ffi.so');
      }
      // Best-effort for dev (Linux/macOS future).
      try {
        return DynamicLibrary.open('libchaos_ffi.so');
      } catch (_) {
        return DynamicLibrary.process();
      }
    }

    ffi = ChaosFfiBindings(openLibrary());

    void sendResp(int id, dynamic result) {
      init.mainPort
          .send({'type': 'resp', 'id': id, 'ok': true, 'result': result});
    }

    void sendErr(int id, String err) {
      init.mainPort.send({
        'type': 'resp',
        'id': id,
        'ok': false,
        'error': err,
        'lastErrorJson': ffi?.takeLastErrorJson(),
      });
    }

    Future<void> pollAllDanmaku() async {
      if (pollBusy) return;
      pollBusy = true;
      try {
        for (final e in danmakuHandles.entries.toList(growable: false)) {
          final sid = e.key;
          final h = e.value;
          try {
            final s = ffi!.danmakuPollJson(h, 50);
            // Fast path: avoid JSON decode here; forward to main isolate.
            if (s != '[]') {
              init.mainPort
                  .send({'type': 'danmaku', 'sessionId': sid, 'eventsJson': s});
            }
          } catch (_) {
            // Ignore poll errors; next tick may recover. Disconnect is explicit.
          }
        }
      } finally {
        pollBusy = false;
      }
    }

    Timer? globalTimer;
    void ensureGlobalPollTimer() {
      globalTimer ??= Timer.periodic(const Duration(milliseconds: 120), (_) {
        pollAllDanmaku();
      });
    }

    void stopGlobalPollTimerIfIdle() {
      if (danmakuHandles.isEmpty) {
        globalTimer?.cancel();
        globalTimer = null;
      }
    }

    recv.listen((msg) async {
      if (msg is! Map) return;
      final m = msg.cast<String, dynamic>();
      if (m['type'] != 'req') return;
      final id = m['id'] as int;
      final method = m['method'] as String? ?? '';
      final params = (m['params'] as Map?)?.cast<String, dynamic>() ??
          const <String, dynamic>{};

      try {
        switch (method) {
          case 'subtitle.search':
            {
              final json = ffi!.subtitleSearchJson(
                query: params['query'] as String,
                limit: params['limit'] as int,
                minScore: params['minScore'] as double?,
                lang: params['lang'] as String?,
                timeoutMs: params['timeoutMs'] as int,
              );
              sendResp(id, json);
              return;
            }
          case 'subtitle.download':
            {
              final json = ffi!.subtitleDownloadJson(
                itemJson: params['itemJson'] as String,
                outDir: params['outDir'] as String,
                timeoutMs: params['timeoutMs'] as int,
                retries: params['retries'] as int,
                overwrite: params['overwrite'] as bool,
              );
              sendResp(id, json);
              return;
            }
          case 'live.decodeManifest':
            {
              final json =
                  ffi!.liveDecodeManifestJson(input: params['input'] as String);
              sendResp(id, json);
              return;
            }
          case 'live.resolveVariant2':
            {
              final json = ffi!.resolveVariant2Json(
                site: params['site'] as String,
                roomId: params['roomId'] as String,
                variantId: params['variantId'] as String,
              );
              sendResp(id, json);
              return;
            }
          case 'liveDir.categories':
            {
              final json = ffi!.liveDirCategoriesJson(params['site'] as String);
              sendResp(id, json);
              return;
            }
          case 'liveDir.recommendRooms':
            {
              final json = ffi!.liveDirRecommendRoomsJson(
                  params['site'] as String, params['page'] as int);
              sendResp(id, json);
              return;
            }
          case 'liveDir.categoryRooms':
            {
              final json = ffi!.liveDirCategoryRoomsJson(
                params['site'] as String,
                params['parentId'] as String?,
                params['categoryId'] as String,
                params['page'] as int,
              );
              sendResp(id, json);
              return;
            }
          case 'liveDir.searchRooms':
            {
              final json = ffi!.liveDirSearchRoomsJson(
                params['site'] as String,
                params['keyword'] as String,
                params['page'] as int,
              );
              sendResp(id, json);
              return;
            }
          case 'danmaku.connect':
            {
              final sid = params['sessionId'] as String;
              final input = params['input'] as String;
              // Replace existing session handle if present.
              final old = danmakuHandles.remove(sid);
              if (old != null) {
                try {
                  ffi!.danmakuDisconnect(old);
                } catch (_) {}
              }
              final h = ffi!.danmakuConnect(input);
              danmakuHandles[sid] = h;
              ensureGlobalPollTimer();
              sendResp(id, true);
              return;
            }
          case 'danmaku.disconnect':
            {
              final sid = params['sessionId'] as String;
              final h = danmakuHandles.remove(sid);
              if (h != null) {
                try {
                  ffi!.danmakuDisconnect(h);
                } catch (_) {}
              }
              stopGlobalPollTimerIfIdle();
              sendResp(id, true);
              return;
            }
          case 'nowPlaying.snapshot':
            {
              final json = ffi!.nowPlayingSnapshotJson(
                includeThumbnail: params['includeThumbnail'] as bool,
                maxThumbnailBytes: params['maxThumbnailBytes'] as int,
                maxSessions: params['maxSessions'] as int,
              );
              sendResp(id, json);
              return;
            }
          case 'lyrics.search':
            {
              final json = ffi!.lyricsSearchJson(
                title: params['title'] as String,
                album: params['album'] as String?,
                artist: params['artist'] as String?,
                durationMs: params['durationMs'] as int,
                limit: params['limit'] as int,
                strictMatch: params['strictMatch'] as bool,
                servicesCsv: params['servicesCsv'] as String?,
                timeoutMs: params['timeoutMs'] as int,
              );
              sendResp(id, json);
              return;
            }
          case 'music.config.set':
            {
              final json =
                  ffi!.musicConfigSetJson(params['configJson'] as String);
              sendResp(id, json);
              return;
            }
          case 'music.searchTracks':
            sendResp(
                id, ffi!.musicSearchTracksJson(params['paramsJson'] as String));
            return;
          case 'music.searchAlbums':
            sendResp(
                id, ffi!.musicSearchAlbumsJson(params['paramsJson'] as String));
            return;
          case 'music.searchArtists':
            sendResp(id,
                ffi!.musicSearchArtistsJson(params['paramsJson'] as String));
            return;
          case 'music.albumTracks':
            sendResp(
                id, ffi!.musicAlbumTracksJson(params['paramsJson'] as String));
            return;
          case 'music.artistAlbums':
            sendResp(
                id, ffi!.musicArtistAlbumsJson(params['paramsJson'] as String));
            return;
          case 'music.trackPlayUrl':
            sendResp(
                id, ffi!.musicTrackPlayUrlJson(params['paramsJson'] as String));
            return;
          case 'music.qq.loginQrCreate':
            sendResp(id,
                ffi!.musicQqLoginQrCreateJson(params['loginType'] as String));
            return;
          case 'music.qq.loginQrPoll':
            sendResp(
                id, ffi!.musicQqLoginQrPollJson(params['sessionId'] as String));
            return;
          case 'music.qq.refreshCookie':
            sendResp(id,
                ffi!.musicQqRefreshCookieJson(params['cookieJson'] as String));
            return;
          case 'music.kugou.loginQrCreate':
            sendResp(
                id,
                ffi!.musicKugouLoginQrCreateJson(
                    params['loginType'] as String));
            return;
          case 'music.kugou.loginQrPoll':
            sendResp(id,
                ffi!.musicKugouLoginQrPollJson(params['sessionId'] as String));
            return;
          case 'music.download.start':
            sendResp(id,
                ffi!.musicDownloadStartJson(params['paramsJson'] as String));
            return;
          case 'music.download.status':
            sendResp(id,
                ffi!.musicDownloadStatusJson(params['sessionId'] as String));
            return;
          case 'music.download.cancel':
            sendResp(id,
                ffi!.musicDownloadCancelJson(params['sessionId'] as String));
            return;
          case 'ffi.set_override':
            // no-op; overrides can't be injected across isolates in production.
            sendResp(id, true);
            return;
        }

        sendErr(id, 'unknown ffi method: $method');
      } catch (e) {
        sendErr(id, e.toString());
      }
    });
  }
}

class _IsolateInit {
  final SendPort mainPort;
  final bool useOverride;
  const _IsolateInit({required this.mainPort, required this.useOverride});
}
