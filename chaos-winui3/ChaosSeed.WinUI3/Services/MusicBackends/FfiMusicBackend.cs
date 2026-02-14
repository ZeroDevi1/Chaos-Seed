using System.Text.Json;
using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models.Music;
using Newtonsoft.Json;

namespace ChaosSeed.WinUI3.Services.MusicBackends;

public sealed class FfiMusicBackend : IMusicBackend
{
    private readonly SemaphoreSlim _ffiGate = new(1, 1);

    public string Name => "FFI";
    public string? InitNotice => null;

    public async Task ConfigSetAsync(MusicProviderConfig cfg, CancellationToken ct)
    {
        if (cfg is null) throw new ArgumentNullException(nameof(cfg));
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = JsonConvert.SerializeObject(cfg);
            await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_music_config_set_json(json);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "music.config.set failed"));
                }
            }, ct);
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public Task<MusicTrack[]> SearchTracksAsync(MusicSearchParams p, CancellationToken ct)
        => InvokeArrayAsync<MusicTrack>("searchTracks", json => ChaosFfi.chaos_music_search_tracks_json(json), p, ct);

    public Task<MusicAlbum[]> SearchAlbumsAsync(MusicSearchParams p, CancellationToken ct)
        => InvokeArrayAsync<MusicAlbum>("searchAlbums", json => ChaosFfi.chaos_music_search_albums_json(json), p, ct);

    public Task<MusicArtist[]> SearchArtistsAsync(MusicSearchParams p, CancellationToken ct)
        => InvokeArrayAsync<MusicArtist>("searchArtists", json => ChaosFfi.chaos_music_search_artists_json(json), p, ct);

    public Task<MusicTrack[]> AlbumTracksAsync(MusicAlbumTracksParams p, CancellationToken ct)
        => InvokeArrayAsync<MusicTrack>("albumTracks", json => ChaosFfi.chaos_music_album_tracks_json(json), p, ct);

    public Task<MusicAlbum[]> ArtistAlbumsAsync(MusicArtistAlbumsParams p, CancellationToken ct)
        => InvokeArrayAsync<MusicAlbum>("artistAlbums", json => ChaosFfi.chaos_music_artist_albums_json(json), p, ct);

    public Task<MusicTrackPlayUrlResult> TrackPlayUrlAsync(MusicTrackPlayUrlParams p, CancellationToken ct)
        => InvokeObjectAsync<MusicTrackPlayUrlResult>(
            "trackPlayUrl",
            json => ChaosFfi.chaos_music_track_play_url_json(json),
            p,
            ct
        );

    public async Task<MusicLoginQr> QqLoginQrCreateAsync(string loginType, CancellationToken ct)
    {
        var lt = (loginType ?? "").Trim();
        if (string.IsNullOrWhiteSpace(lt)) throw new ArgumentException("empty loginType", nameof(loginType));

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_music_qq_login_qr_create_json(lt);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "qq login qr create failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<MusicLoginQr>(json) ?? throw new InvalidOperationException("invalid MusicLoginQr json");
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<MusicLoginQrPollResult> QqLoginQrPollAsync(string sessionId, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid)) throw new ArgumentException("empty sessionId", nameof(sessionId));

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_music_qq_login_qr_poll_json(sid);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "qq login qr poll failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<MusicLoginQrPollResult>(json) ?? throw new InvalidOperationException("invalid MusicLoginQrPollResult json");
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<QqMusicCookie> QqRefreshCookieAsync(QqMusicCookie cookie, CancellationToken ct)
    {
        if (cookie is null) throw new ArgumentNullException(nameof(cookie));
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var cookieJson = JsonConvert.SerializeObject(cookie);
            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_music_qq_refresh_cookie_json(cookieJson);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "qq refresh cookie failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<QqMusicCookie>(json) ?? throw new InvalidOperationException("invalid QqMusicCookie json");
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<MusicLoginQr> KugouLoginQrCreateAsync(string loginType, CancellationToken ct)
    {
        var lt = (loginType ?? "").Trim();
        if (string.IsNullOrWhiteSpace(lt)) throw new ArgumentException("empty loginType", nameof(loginType));

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_music_kugou_login_qr_create_json(lt);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "kugou login qr create failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<MusicLoginQr>(json) ?? throw new InvalidOperationException("invalid MusicLoginQr json");
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<MusicLoginQrPollResult> KugouLoginQrPollAsync(string sessionId, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid)) throw new ArgumentException("empty sessionId", nameof(sessionId));

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_music_kugou_login_qr_poll_json(sid);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "kugou login qr poll failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<MusicLoginQrPollResult>(json) ?? throw new InvalidOperationException("invalid MusicLoginQrPollResult json");
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public Task<MusicDownloadStartResult> DownloadStartAsync(MusicDownloadStartParams p, CancellationToken ct)
        => InvokeObjectAsync<MusicDownloadStartResult>(
            "downloadStart",
            json => ChaosFfi.chaos_music_download_start_json(json),
            p,
            ct
        );

    public Task<MusicDownloadStatus> DownloadStatusAsync(string sessionId, CancellationToken ct)
        => InvokeSessionAsync<MusicDownloadStatus>(
            "downloadStatus",
            sid => ChaosFfi.chaos_music_download_status_json(sid),
            sessionId,
            ct
        );

    public Task CancelDownloadAsync(string sessionId, CancellationToken ct)
        => InvokeSessionAsync<OkReply>(
            "downloadCancel",
            sid => ChaosFfi.chaos_music_download_cancel_json(sid),
            sessionId,
            ct
        );

    public void Dispose()
    {
        _ffiGate.Dispose();
    }

    private async Task<T[]> InvokeArrayAsync<T>(
        string opName,
        Func<string, IntPtr> ffiFunc,
        object payload,
        CancellationToken ct
    )
    {
        if (payload is null) throw new ArgumentNullException(nameof(payload));
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = JsonConvert.SerializeObject(payload);
            var outJson = await Task.Run(() =>
            {
                var p = ffiFunc(json);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, $"music.{opName} failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<T[]>(outJson) ?? Array.Empty<T>();
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    private async Task<T> InvokeObjectAsync<T>(
        string opName,
        Func<string, IntPtr> ffiFunc,
        object payload,
        CancellationToken ct
    )
    {
        if (payload is null) throw new ArgumentNullException(nameof(payload));
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var json = JsonConvert.SerializeObject(payload);
            var outJson = await Task.Run(() =>
            {
                var p = ffiFunc(json);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, $"music.{opName} failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<T>(outJson) ?? throw new InvalidOperationException($"invalid {typeof(T).Name} json");
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    private async Task<T> InvokeSessionAsync<T>(
        string opName,
        Func<string, IntPtr> ffiFunc,
        string sessionId,
        CancellationToken ct
    )
    {
        var sid = (sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid)) throw new ArgumentException("empty sessionId", nameof(sessionId));

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();
            var outJson = await Task.Run(() =>
            {
                var p = ffiFunc(sid);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, $"music.{opName} failed"));
                }
                return s!;
            }, ct);
            return JsonConvert.DeserializeObject<T>(outJson) ?? throw new InvalidOperationException($"invalid {typeof(T).Name} json");
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    private static string FormatFfiError(string? errJson, string fallback)
    {
        if (string.IsNullOrWhiteSpace(errJson))
        {
            return fallback;
        }

        try
        {
            using var doc = JsonDocument.Parse(errJson);
            var root = doc.RootElement;
            var message = root.TryGetProperty("message", out var m) ? (m.GetString() ?? "") : "";
            var context = root.TryGetProperty("context", out var c) ? (c.GetString() ?? "") : "";

            message = message.Trim();
            context = context.Trim();

            if (!string.IsNullOrWhiteSpace(message) && !string.IsNullOrWhiteSpace(context))
            {
                return $"{message}\n{context}";
            }

            if (!string.IsNullOrWhiteSpace(message))
            {
                return message;
            }

            if (!string.IsNullOrWhiteSpace(context))
            {
                return context;
            }
        }
        catch
        {
            // ignore
        }

        return errJson.Trim();
    }
}
