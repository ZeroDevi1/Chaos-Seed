using System.Text.Json;
using ChaosSeed.WinUI3.Chaos;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Models.Ffi;

namespace ChaosSeed.WinUI3.Services.LyricsBackends;

public sealed class FfiLyricsBackend : ILyricsBackend
{
    private static readonly JsonSerializerOptions _jsonOptions = new(JsonSerializerDefaults.Web)
    {
        PropertyNameCaseInsensitive = true,
    };

    private readonly SemaphoreSlim _ffiGate = new(1, 1);

    public string Name => "FFI";

    public string? InitNotice => null;

    public async Task<NowPlayingSnapshot> SnapshotNowPlayingAsync(
        bool includeThumbnail,
        int maxThumbBytes,
        int maxSessions,
        CancellationToken ct
    )
    {
        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();

            var inc = includeThumbnail ? (byte)1 : (byte)0;
            var maxThumb = (uint)Math.Clamp(maxThumbBytes, 1, 2_500_000);
            var maxSess = (uint)Math.Clamp(maxSessions, 1, 128);

            var json = await Task.Run(() =>
            {
                var p = ChaosFfi.chaos_now_playing_snapshot_json(inc, maxThumb, maxSess);
                var s = ChaosFfi.TakeString(p);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "now playing snapshot failed"));
                }
                return s!;
            }, ct);

            var snap = JsonSerializer.Deserialize<FfiNowPlayingSnapshot>(json, _jsonOptions)
                       ?? throw new InvalidOperationException("invalid now playing json");

            return MapNowPlaying(snap);
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public async Task<LyricsSearchResult[]> SearchLyricsAsync(LyricsSearchParams p, CancellationToken ct)
    {
        if (p is null)
        {
            throw new ArgumentNullException(nameof(p));
        }

        var title = (p.Title ?? "").Trim();
        if (string.IsNullOrWhiteSpace(title))
        {
            throw new ArgumentException("empty title", nameof(p));
        }

        await _ffiGate.WaitAsync(ct);
        try
        {
            ct.ThrowIfCancellationRequested();

            var album = string.IsNullOrWhiteSpace(p.Album) ? null : p.Album!.Trim();
            var artist = string.IsNullOrWhiteSpace(p.Artist) ? null : p.Artist!.Trim();
            var durationMs = p.DurationMs is null
                ? 0u
                : (p.DurationMs.Value > uint.MaxValue ? uint.MaxValue : (uint)p.DurationMs.Value);

            var limit = p.Limit is null ? 10u : p.Limit.Value;
            if (limit < 1u) limit = 1u;
            if (limit > 50u) limit = 50u;

            var strict = p.StrictMatch == true ? (byte)1 : (byte)0;
            var timeoutRaw = p.TimeoutMs is null ? 8000ul : p.TimeoutMs.Value;
            if (timeoutRaw < 1ul) timeoutRaw = 1ul;
            if (timeoutRaw > 60000ul) timeoutRaw = 60000ul;
            var timeoutMs = (uint)timeoutRaw;
            var servicesCsv = p.Services is null || p.Services.Length == 0
                ? null
                : string.Join(",", p.Services.Where(s => !string.IsNullOrWhiteSpace(s)).Select(s => s.Trim()));

            var json = await Task.Run(() =>
            {
                var pJson = ChaosFfi.chaos_lyrics_search_json(
                    title,
                    album,
                    artist,
                    durationMs,
                    limit,
                    strict,
                    servicesCsv,
                    timeoutMs
                );
                var s = ChaosFfi.TakeString(pJson);
                if (string.IsNullOrWhiteSpace(s))
                {
                    var err = ChaosFfi.TakeLastErrorJson();
                    throw new InvalidOperationException(FormatFfiError(err, "lyrics search failed"));
                }
                return s!;
            }, ct);

            var items = JsonSerializer.Deserialize<List<FfiLyricsSearchResult>>(json, _jsonOptions)
                        ?? new List<FfiLyricsSearchResult>();

            return items.Select(MapLyricsItem).ToArray();
        }
        finally
        {
            _ffiGate.Release();
        }
    }

    public void Dispose()
    {
        _ffiGate.Dispose();
    }

    private static NowPlayingSnapshot MapNowPlaying(FfiNowPlayingSnapshot snap)
    {
        return new NowPlayingSnapshot
        {
            Supported = snap.Supported,
            NowPlaying = snap.NowPlaying is null ? null : MapNowPlayingSession(snap.NowPlaying),
            Sessions = (snap.Sessions ?? new List<FfiNowPlayingSession>()).Select(MapNowPlayingSession).ToArray(),
            PickedAppId = snap.PickedAppId,
            RetrievedAtUnixMs = snap.RetrievedAtUnixMs,
        };
    }

    private static NowPlayingSession MapNowPlayingSession(FfiNowPlayingSession s)
    {
        return new NowPlayingSession
        {
            AppId = s.AppId ?? "",
            IsCurrent = s.IsCurrent,
            PlaybackStatus = s.PlaybackStatus ?? "",
            Title = s.Title,
            Artist = s.Artist,
            AlbumTitle = s.AlbumTitle,
            PositionMs = s.PositionMs,
            DurationMs = s.DurationMs,
            Genres = (s.Genres ?? new List<string>()).Where(x => !string.IsNullOrWhiteSpace(x)).ToArray(),
            SongId = s.SongId,
            Thumbnail = s.Thumbnail is null ? null : new NowPlayingThumbnail { Mime = s.Thumbnail.Mime, Base64 = s.Thumbnail.Base64 },
            Error = s.Error,
        };
    }

    private static LyricsSearchResult MapLyricsItem(FfiLyricsSearchResult x)
    {
        return new LyricsSearchResult
        {
            Service = (x.Service ?? "").Trim(),
            ServiceToken = x.ServiceToken ?? "",
            Title = x.Title,
            Artist = x.Artist,
            Album = x.Album,
            DurationMs = x.DurationMs,
            MatchPercentage = x.MatchPercentage,
            Quality = x.Quality,
            Matched = x.Matched,
            HasTranslation = x.HasTranslation,
            HasInlineTimetags = x.HasInlineTimetags,
            LyricsOriginal = x.LyricsOriginal ?? "",
            LyricsTranslation = x.LyricsTranslation,
            Debug = null,
        };
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
