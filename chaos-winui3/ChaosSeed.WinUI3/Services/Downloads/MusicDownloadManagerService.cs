using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
using ChaosSeed.WinUI3.Models.Music;
using ChaosSeed.WinUI3.Services.MusicBackends;
using Microsoft.UI.Dispatching;

namespace ChaosSeed.WinUI3.Services.Downloads;

public sealed class DownloadSessionMeta
{
    public string TargetType { get; init; } = "";
    public string? Service { get; init; }
    public string? Title { get; init; }
    public string? Artist { get; init; }
    public string? Album { get; init; }
    public string? CoverUrl { get; init; }

    public MusicTrack[]? PrefetchedTracks { get; init; }
}

public sealed class MusicDownloadManagerService
{
    public static MusicDownloadManagerService Instance => _instance.Value;
    private static readonly Lazy<MusicDownloadManagerService> _instance = new(() => new MusicDownloadManagerService());

    private readonly DispatcherQueue _dq;
    private readonly IMusicBackend _backend;
    private readonly MusicDownloadDb _db;

    private readonly ConcurrentDictionary<string, CancellationTokenSource> _pollCts = new(StringComparer.Ordinal);

    public ObservableCollection<DownloadSessionVm> ActiveSessions { get; } = new();

    public event EventHandler? Changed;

    private MusicDownloadManagerService()
    {
        _dq = DispatcherQueue.GetForCurrentThread();
        _backend = MusicBackendFactory.Create();
        _db = new MusicDownloadDb(MusicDownloadDb.GetDefaultDbPath());
    }

    public MusicDownloadDb Db => _db;

    public async Task<string> StartAsync(MusicDownloadStartParams start, DownloadSessionMeta meta, CancellationToken ct)
    {
        if (start is null) throw new ArgumentNullException(nameof(start));
        meta ??= new DownloadSessionMeta();

        ct.ThrowIfCancellationRequested();

        try
        {
            await _backend.ConfigSetAsync(start.Config, ct);
        }
        catch
        {
            // ignore
        }

        var now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
        var res = await _backend.DownloadStartAsync(start, ct);
        var sessionId = (res.SessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sessionId))
        {
            throw new InvalidOperationException("empty sessionId");
        }

        var row = new DownloadSessionRow
        {
            SessionId = sessionId,
            StartedAtUnixMs = now,
            LastUpdateUnixMs = now,
            Done = false,
            TargetType = (meta.TargetType ?? "").Trim(),
            Service = meta.Service,
            Title = meta.Title,
            Artist = meta.Artist,
            Album = meta.Album,
            CoverUrl = meta.CoverUrl,
            OutDir = (start.Options?.OutDir ?? "").Trim(),
            QualityId = (start.Options?.QualityId ?? "").Trim(),
            PathTemplate = start.Options?.PathTemplate,
            Overwrite = start.Options?.Overwrite == true,
            Concurrency = (int)(start.Options?.Concurrency ?? 0),
            Retries = (int)(start.Options?.Retries ?? 0),
            Total = 0,
            DoneCount = 0,
            Failed = 0,
            Skipped = 0,
            Canceled = 0,
        };

        _db.UpsertSession(row);

        var prefetched = meta.PrefetchedTracks ?? Array.Empty<MusicTrack>();
        if (prefetched.Length > 0)
        {
            var jobs = prefetched.Select((t, i) => new DownloadJobRow
            {
                SessionId = sessionId,
                JobIndex = i,
                TrackId = t.Id,
                TrackTitle = t.Title,
                TrackArtists = t.Artists is null ? null : string.Join(" / ", t.Artists.Where(x => !string.IsNullOrWhiteSpace(x))),
                TrackAlbum = t.Album,
                State = "pending",
                Path = null,
                Bytes = null,
                Error = null,
            });
            _db.UpsertJobs(sessionId, jobs);
        }

        EnsureActiveSessionVm(row);
        StartPolling(sessionId);
        return sessionId;
    }

    public async Task CancelAsync(string sessionId, CancellationToken ct)
    {
        var sid = (sessionId ?? "").Trim();
        if (sid.Length == 0)
        {
            return;
        }

        try
        {
            await _backend.CancelDownloadAsync(sid, ct);
        }
        catch
        {
            // ignore
        }

        try
        {
            var st = await _backend.DownloadStatusAsync(sid, ct);
            var now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            ApplyStatusToDbAndVm(sid, st, now);
        }
        catch
        {
            // ignore
        }

        StopPolling(sid);
        RaiseChanged();
    }

    public void StopPolling(string sessionId)
    {
        var sid = (sessionId ?? "").Trim();
        if (sid.Length == 0)
        {
            return;
        }

        if (_pollCts.TryRemove(sid, out var cts))
        {
            try { cts.Cancel(); } catch { }
            try { cts.Dispose(); } catch { }
        }
    }

    private void StartPolling(string sessionId)
    {
        var sid = (sessionId ?? "").Trim();
        if (sid.Length == 0)
        {
            return;
        }

        StopPolling(sid);
        var cts = new CancellationTokenSource();
        _pollCts[sid] = cts;
        var ct = cts.Token;

        _ = Task.Run(async () =>
        {
            try
            {
                while (!ct.IsCancellationRequested)
                {
                    MusicDownloadStatus st;
                    try
                    {
                        st = await _backend.DownloadStatusAsync(sid, ct);
                    }
                    catch
                    {
                        await Task.Delay(1200, ct);
                        continue;
                    }

                    var now = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
                    ApplyStatusToDbAndVm(sid, st, now);

                    RaiseChanged();

                    if (st.Done)
                    {
                        StopPolling(sid);
                        return;
                    }

                    await Task.Delay(800, ct);
                }
            }
            catch (OperationCanceledException)
            {
                // ignore
            }
            catch
            {
                // ignore
            }
        }, ct);
    }

    public IReadOnlyList<DownloadSessionRow> ListSessions(int limit = 200)
        => _db.ListSessions(limit);

    public IReadOnlyList<DownloadJobRow> ListJobs(string sessionId, int limit = 5000)
        => _db.ListJobs(sessionId, limit);

    public void DeleteSession(string sessionId)
    {
        StopPolling(sessionId);
        _db.DeleteSession(sessionId);
        _dq.TryEnqueue(() =>
        {
            var x = ActiveSessions.FirstOrDefault(s => string.Equals(s.SessionId, sessionId, StringComparison.Ordinal));
            if (x is not null)
            {
                ActiveSessions.Remove(x);
            }
        });
        RaiseChanged();
    }

    private void EnsureActiveSessionVm(DownloadSessionRow row)
    {
        _dq.TryEnqueue(() =>
        {
            var vm = ActiveSessions.FirstOrDefault(x => string.Equals(x.SessionId, row.SessionId, StringComparison.Ordinal));
            if (vm is null)
            {
                vm = new DownloadSessionVm(row);
                ActiveSessions.Insert(0, vm);
            }
            else
            {
                vm.Apply(row);
            }
        });
    }

    private void ApplyVmUpdate(DownloadSessionRow row)
    {
        var vm = ActiveSessions.FirstOrDefault(x => string.Equals(x.SessionId, row.SessionId, StringComparison.Ordinal));
        if (vm is null)
        {
            vm = new DownloadSessionVm(row);
            ActiveSessions.Insert(0, vm);
            return;
        }

        vm.Apply(row);
        if (row.Done)
        {
            // Keep it in ActiveSessions for quick access; DownloadsPage is the full history.
        }
    }

    private void RaiseChanged()
    {
        try
        {
            Changed?.Invoke(this, EventArgs.Empty);
        }
        catch
        {
            // ignore
        }
    }

    private void ApplyStatusToDbAndVm(string sid, MusicDownloadStatus st, long nowUnixMs)
    {
        var sessions = _db.ListSessions(limit: 200);
        var existing = sessions.FirstOrDefault(x => string.Equals(x.SessionId, sid, StringComparison.Ordinal));
        existing ??= new DownloadSessionRow
        {
            SessionId = sid,
            StartedAtUnixMs = nowUnixMs,
            LastUpdateUnixMs = nowUnixMs,
            OutDir = "",
            QualityId = "",
            TargetType = "",
        };

        var updated = existing with
        {
            LastUpdateUnixMs = nowUnixMs,
            Done = st.Done,
            Total = (int)st.Totals.Total,
            DoneCount = (int)st.Totals.Done,
            Failed = (int)st.Totals.Failed,
            Skipped = (int)st.Totals.Skipped,
            Canceled = (int)st.Totals.Canceled,
        };

        _db.UpsertSession(updated);

        var jobs = (st.Jobs ?? Array.Empty<MusicDownloadJobResult>())
            .Select(j => new DownloadJobRow
            {
                SessionId = sid,
                JobIndex = (int)j.Index,
                TrackId = j.TrackId,
                TrackTitle = null,
                TrackArtists = null,
                TrackAlbum = null,
                State = (j.State ?? "").Trim().ToLowerInvariant(),
                Path = j.Path,
                Bytes = j.Bytes is null ? null : (long?)j.Bytes.Value,
                Error = j.Error,
            });

        _db.UpsertJobs(sid, jobs);
        _dq.TryEnqueue(() => ApplyVmUpdate(updated));
    }
}

public sealed class DownloadSessionVm : INotifyPropertyChanged
{
    public DownloadSessionVm(DownloadSessionRow row)
    {
        SessionId = row.SessionId;
        Apply(row);
    }

    public string SessionId { get; }

    private string _title = "";
    public string Title
    {
        get => _title;
        private set
        {
            if (string.Equals(_title, value, StringComparison.Ordinal))
            {
                return;
            }
            _title = value;
            OnPropertyChanged();
        }
    }

    private string _state = "";
    public string State
    {
        get => _state;
        private set
        {
            if (string.Equals(_state, value, StringComparison.Ordinal))
            {
                return;
            }
            _state = value;
            OnPropertyChanged();
        }
    }

    private string _totals = "";
    public string Totals
    {
        get => _totals;
        private set
        {
            if (string.Equals(_totals, value, StringComparison.Ordinal))
            {
                return;
            }
            _totals = value;
            OnPropertyChanged();
        }
    }

    private bool _done;
    public bool Done
    {
        get => _done;
        private set
        {
            if (_done == value)
            {
                return;
            }
            _done = value;
            OnPropertyChanged();
        }
    }

    private long _startedAtUnixMs;
    public long StartedAtUnixMs
    {
        get => _startedAtUnixMs;
        private set
        {
            if (_startedAtUnixMs == value)
            {
                return;
            }
            _startedAtUnixMs = value;
            OnPropertyChanged();
            OnPropertyChanged(nameof(StartedAtText));
        }
    }

    public string StartedAtText => MusicDownloadDb.FormatUnixMs(StartedAtUnixMs);

    public void Apply(DownloadSessionRow row)
    {
        StartedAtUnixMs = row.StartedAtUnixMs;
        Done = row.Done;
        Title = BuildTitle(row);
        Totals = $"Total={row.Total} Done={row.DoneCount} Failed={row.Failed} Skipped={row.Skipped} Canceled={row.Canceled}";
        State = row.Done ? InferFinalState(row) : InferActiveState(row);
    }

    private static string BuildTitle(DownloadSessionRow row)
    {
        var title = (row.Title ?? "").Trim();
        var artist = (row.Artist ?? "").Trim();
        if (string.IsNullOrWhiteSpace(title))
        {
            title = row.SessionId;
        }

        return string.IsNullOrWhiteSpace(artist) ? title : $"{title} - {artist}";
    }

    private static string InferActiveState(DownloadSessionRow row)
    {
        if (row.Canceled > 0 && row.DoneCount == 0 && row.Failed == 0)
        {
            return "canceled";
        }

        if (row.DoneCount > 0 || row.Failed > 0 || row.Skipped > 0)
        {
            return "running";
        }

        return "pending";
    }

    private static string InferFinalState(DownloadSessionRow row)
    {
        if (row.Failed > 0)
        {
            return "failed";
        }

        if (row.Canceled > 0 && row.DoneCount == 0 && row.Failed == 0)
        {
            return "canceled";
        }

        return "done";
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}
