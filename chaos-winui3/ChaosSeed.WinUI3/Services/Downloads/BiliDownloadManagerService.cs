using System.Collections.Concurrent;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Globalization;
using System.Linq;
using System.Runtime.CompilerServices;
using ChaosSeed.WinUI3.Models.Bili;
using ChaosSeed.WinUI3.Services.BiliBackends;
using Microsoft.UI.Dispatching;

namespace ChaosSeed.WinUI3.Services.Downloads;

public sealed class BiliDownloadManagerService
{
    public static BiliDownloadManagerService Instance => _instance.Value;
    private static readonly Lazy<BiliDownloadManagerService> _instance = new(() => new BiliDownloadManagerService());

    private readonly DispatcherQueue _dq;
    private readonly IBiliBackend _backend;

    private readonly ConcurrentDictionary<string, CancellationTokenSource> _pollCts = new(StringComparer.Ordinal);

    public ObservableCollection<BiliDownloadSessionVm> ActiveSessions { get; } = new();
    public BiliParseMemoryCache ParseMemory { get; } = new();

    public event EventHandler? Changed;

    private BiliDownloadManagerService()
    {
        _dq = DispatcherQueue.GetForCurrentThread();
        _backend = BiliBackendFactory.Create();
    }

    public IBiliBackend Backend => _backend;

    public async Task<string> StartAsync(BiliDownloadStartParams start, string? displayTitle, CancellationToken ct)
    {
        if (start is null) throw new ArgumentNullException(nameof(start));
        if (start.Options is null) throw new ArgumentException("missing options", nameof(start));

        ct.ThrowIfCancellationRequested();

        // Prefer BBDown-style task API (daemon/ffi share the same semantics).
        BiliAuthBundle? bundle = null;
        try
        {
            var a = start.Auth;
            var cookie = (a?.Cookie ?? "").Trim();
            var tv = (SettingsService.Instance.Current.BiliTvAccessToken ?? "").Trim();
            if (!string.IsNullOrWhiteSpace(cookie))
            {
                bundle = new BiliAuthBundle
                {
                    Web = new BiliWebAuth
                    {
                        Cookie = cookie,
                        RefreshToken = string.IsNullOrWhiteSpace(a?.RefreshToken) ? null : a!.RefreshToken!.Trim(),
                    }
                };
            }
            if (!string.IsNullOrWhiteSpace(tv))
            {
                bundle ??= new BiliAuthBundle();
                bundle.Tv = new BiliTvAuth { AccessToken = tv };
            }
        }
        catch
        {
            bundle = null;
        }

        var taskRes = await _backend.TaskAddAsync(new BiliTaskAddParams
        {
            Api = start.Api,
            Input = start.Input,
            Auth = bundle,
            Options = start.Options,
        }, ct);

        var sessionId = (taskRes.TaskId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sessionId))
        {
            throw new InvalidOperationException("empty taskId");
        }

        var vm = new BiliDownloadSessionVm
        {
            SessionId = sessionId,
            Input = (start.Input ?? "").Trim(),
            DisplayTitle = string.IsNullOrWhiteSpace(displayTitle) ? null : displayTitle!.Trim(),
            StartedAtUnixMs = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
        };

        _dq.TryEnqueue(() =>
        {
            ActiveSessions.Insert(0, vm);
            RaiseChanged();
        });

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
            await _backend.TaskCancelAsync(sid, ct);
        }
        catch
        {
            // best-effort
        }

        try
        {
            var st = (await _backend.TaskGetAsync(sid, ct)).Status;
            _dq.TryEnqueue(() =>
            {
                var vm = ActiveSessions.FirstOrDefault(x => string.Equals(x.SessionId, sid, StringComparison.Ordinal));
                vm?.UpdateFrom(st);
                RaiseChanged();
            });
        }
        catch
        {
            // ignore
        }

        StopPolling(sid);
    }

    public void Remove(string sessionId)
    {
        var sid = (sessionId ?? "").Trim();
        if (sid.Length == 0)
        {
            return;
        }

        StopPolling(sid);
        _dq.TryEnqueue(() =>
        {
            var vm = ActiveSessions.FirstOrDefault(x => string.Equals(x.SessionId, sid, StringComparison.Ordinal));
            if (vm is not null)
            {
                ActiveSessions.Remove(vm);
                RaiseChanged();
            }
        });
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
                    BiliDownloadStatus st;
                    try
                    {
                        st = (await _backend.TaskGetAsync(sid, ct)).Status;
                    }
                    catch
                    {
                        await Task.Delay(1200, ct);
                        continue;
                    }

                    _dq.TryEnqueue(() =>
                    {
                        var vm = ActiveSessions.FirstOrDefault(x => string.Equals(x.SessionId, sid, StringComparison.Ordinal));
                        vm?.UpdateFrom(st);
                        RaiseChanged();
                    });

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

    private void RaiseChanged()
    {
        try { Changed?.Invoke(this, EventArgs.Empty); } catch { }
    }
}

public sealed class BiliDownloadSessionVm : INotifyPropertyChanged
{
    public string SessionId { get; set; } = "";
    public string? Input { get; set; }
    public string? DisplayTitle { get; set; }
    public long StartedAtUnixMs { get; set; }

    private bool _done;
    private string _totalsText = "";

    public ObservableCollection<BiliDownloadJobVm> Jobs { get; } = new();

    public bool Done
    {
        get => _done;
        private set => SetField(ref _done, value);
    }

    public string Title
    {
        get
        {
            var t = (DisplayTitle ?? "").Trim();
            if (!string.IsNullOrWhiteSpace(t))
            {
                return t;
            }
            var input = (Input ?? "").Trim();
            return string.IsNullOrWhiteSpace(input) ? SessionId : input;
        }
    }

    public string StartedAtText
        => DateTimeOffset.FromUnixTimeMilliseconds(StartedAtUnixMs).LocalDateTime.ToString("yyyy-MM-dd HH:mm:ss", CultureInfo.InvariantCulture);

    public string TotalsText
    {
        get => _totalsText;
        private set => SetField(ref _totalsText, value);
    }

    public void UpdateFrom(BiliDownloadStatus st)
    {
        if (st is null)
        {
            return;
        }

        Done = st.Done;
        var t = st.Totals;
        TotalsText = $"Total={t.Total} Done={t.Done} Failed={t.Failed} Skipped={t.Skipped} Canceled={t.Canceled}";

        var byIndex = Jobs.ToDictionary(x => x.Index, x => x);
        var order = new List<BiliDownloadJobVm>();

        foreach (var j in st.Jobs ?? Array.Empty<BiliDownloadJobStatus>())
        {
            var idx = j.Index;
            if (!byIndex.TryGetValue(idx, out var vm))
            {
                vm = new BiliDownloadJobVm { Index = idx };
                Jobs.Add(vm);
            }
            vm.UpdateFrom(j);
            order.Add(vm);
        }

        // Keep a stable order by index.
        if (order.Count > 0)
        {
            var sorted = order.OrderBy(x => x.Index).ToArray();
            for (var i = 0; i < sorted.Length; i++)
            {
                var want = sorted[i];
                var cur = Jobs[i];
                if (!ReferenceEquals(cur, want))
                {
                    Jobs.Remove(want);
                    Jobs.Insert(i, want);
                }
            }
        }

        OnPropertyChanged(nameof(Title));
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    private void OnPropertyChanged(string name)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));

    private void SetField<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return;
        }
        field = value;
        if (name is not null)
        {
            OnPropertyChanged(name);
        }
    }
}

public sealed class BiliDownloadJobVm : INotifyPropertyChanged
{
    public uint Index { get; set; }

    private uint? _pageNumber;
    private string _title = "";
    private string _state = "";
    private string _phase = "";
    private double _progressValue;
    private string _progressText = "";
    private string? _error;

    public uint? PageNumber
    {
        get => _pageNumber;
        private set => SetField(ref _pageNumber, value);
    }

    public string Title
    {
        get => _title;
        private set => SetField(ref _title, value);
    }

    public string State
    {
        get => _state;
        private set => SetField(ref _state, value);
    }

    public string Phase
    {
        get => _phase;
        private set => SetField(ref _phase, value);
    }

    public double ProgressValue
    {
        get => _progressValue;
        private set => SetField(ref _progressValue, value);
    }

    public string ProgressText
    {
        get => _progressText;
        private set => SetField(ref _progressText, value);
    }

    public string? Error
    {
        get => _error;
        private set => SetField(ref _error, value);
    }

    public string IndexText => PageNumber is null ? $"#{Index}" : $"P{PageNumber} (#{Index})";

    public void UpdateFrom(BiliDownloadJobStatus st)
    {
        PageNumber = st.PageNumber;
        Title = (st.Title ?? "").Trim();
        State = (st.State ?? "").Trim();
        Phase = (st.Phase ?? "").Trim();

        var done = st.BytesDownloaded;
        var total = st.BytesTotal;
        var speed = st.SpeedBps;

        if (total is not null && total.Value > 0)
        {
            ProgressValue = Math.Clamp(done / (double)total.Value, 0.0, 1.0);
        }
        else
        {
            ProgressValue = 0.0;
        }

        var doneText = FormatBytes(done);
        var totalText = total is null ? "?" : FormatBytes(total.Value);
        var speedText = speed is null ? "" : $" · {FormatBytes(speed.Value)}/s";
        ProgressText = $"{doneText} / {totalText}{speedText} · {Phase}";

        Error = string.IsNullOrWhiteSpace(st.Error) ? null : st.Error.Trim();

        OnPropertyChanged(nameof(IndexText));
    }

    private static string FormatBytes(ulong bytes)
    {
        const double K = 1024.0;
        const double M = K * 1024.0;
        const double G = M * 1024.0;

        if (bytes >= (ulong)G) return $"{bytes / G:0.00} GB";
        if (bytes >= (ulong)M) return $"{bytes / M:0.00} MB";
        if (bytes >= (ulong)K) return $"{bytes / K:0.00} KB";
        return $"{bytes} B";
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    private void OnPropertyChanged(string name)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));

    private void SetField<T>(ref T field, T value, [CallerMemberName] string? name = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return;
        }
        field = value;
        if (name is not null)
        {
            OnPropertyChanged(name);
        }
    }
}
