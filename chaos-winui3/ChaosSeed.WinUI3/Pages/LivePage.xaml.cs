using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Numerics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices.WindowsRuntime;
using System.Threading;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using ChaosSeed.WinUI3.Services.LiveBackends;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Media.Animation;
using StreamJsonRpc;
using Windows.Storage.Streams;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class LivePage : Page
{
    private enum LiveMode
    {
        Parse = 0,
        Select = 1,
        Playing = 2,
    }

    private enum PlayUiState
    {
        Idle = 0,
        Opening = 1,
        Playing = 2,
        Failed = 3,
    }

    private sealed class LastPlayRequest
    {
        public string Input { get; init; } = "";
        public string VariantId { get; init; } = "";
        public object? CoverRef { get; init; }
        public int PlaySeq { get; init; }
    }

    private const int DanmakuDefaultWidthPx = 180;
    private const int SplitterWidthPx = 6;
    private const int DanmakuMinWidthPx = 110;
    private const int DanmakuMaxWidthPx = 320;
    private static readonly PlayOpenOptions _playOptions = new()
    {
        OpenTimeout = TimeSpan.FromSeconds(15),
        RetryPerUrl = 1,
        RetryDelay = TimeSpan.FromMilliseconds(300),
    };
    public ObservableCollection<DanmakuRowVm> Rows { get; } = new();
    public ObservableCollection<LiveVariantCardVm> VariantCards { get; } = new();

    private readonly DispatcherQueue _dq = DispatcherQueue.GetForCurrentThread();
    private readonly SemaphoreSlim _imageSem = new(4, 4);
    private readonly FlyleafPlayerService? _player;
    private readonly string? _playerUnavailableMsg;
    private readonly ILiveBackend _backend;

    private string? _sessionId;
    private bool _danmakuExpanded = false;
    private int _danmakuWidthPx = DanmakuDefaultWidthPx;
    private CancellationTokenSource? _danmakuAnimCts;
    private CancellationTokenSource? _danmakuToggleHideCts;
    private LiveMode _mode = LiveMode.Parse;
    private LivestreamDecodeManifestResult? _manifest;
    private string? _lastInput;
    private CancellationTokenSource? _playCts;
    private CancellationTokenSource? _decodeCts;
    private int _playSeq;
    private PlayUiState _playUiState = PlayUiState.Idle;
    private LastPlayRequest? _lastPlayRequest;

    public string OverviewTitle => _manifest?.Info?.Title ?? "";
    public string OverviewStreamer => $"主播：{(_manifest?.Info?.Name ?? "-")}";
    public string OverviewStatus => $"状态：{(_manifest?.Info?.IsLiving == true ? "直播中" : "未开播/离线")}";
    public string OverviewRoom => _manifest is null ? "" : $"房间：{_manifest.Site}:{_manifest.RoomId}";

    public LivePage()
    {
        InitializeComponent();
        FlyleafHost.Loaded += OnFlyleafHostLoaded;
        _backend = LiveBackendFactory.Create();
        _backend.DanmakuMessageReceived += OnDanmakuMessage;
        _player = TryInitPlayer(out _playerUnavailableMsg);
        BindFlyleafHostPlayer(_player);
        Bindings.Update();

        if (!string.IsNullOrWhiteSpace(_backend.InitNotice))
        {
            if (_backend is ErrorLiveBackend)
            {
                ShowParseError(_backend.InitNotice!);
            }
            else
            {
                ShowParseInfo(_backend.InitNotice!);
            }
        }
    }

    private Task RunOnUiAsync(Action action)
    {
        if (_dq.HasThreadAccess)
        {
            action();
            return Task.CompletedTask;
        }

        var tcs = new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously);
        var ok = _dq.TryEnqueue(() =>
        {
            try
            {
                action();
                tcs.TrySetResult(null);
            }
            catch (Exception ex)
            {
                tcs.TrySetException(ex);
            }
        });
        if (!ok)
        {
            tcs.TrySetException(new InvalidOperationException("failed to enqueue UI action"));
        }

        return tcs.Task;
    }

    private Task RunOnUiAsync(Func<Task> action)
    {
        if (_dq.HasThreadAccess)
        {
            return action();
        }

        var tcs = new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously);
        var ok = _dq.TryEnqueue(async () =>
        {
            try
            {
                await action();
                tcs.TrySetResult(null);
            }
            catch (Exception ex)
            {
                tcs.TrySetException(ex);
            }
        });
        if (!ok)
        {
            tcs.TrySetException(new InvalidOperationException("failed to enqueue UI async action"));
        }

        return tcs.Task;
    }

    private FlyleafPlayerService? TryInitPlayer(out string? unavailableMsg)
    {
        unavailableMsg = null;
        if (!string.IsNullOrWhiteSpace(App.FlyleafInitError))
        {
            unavailableMsg =
                $"Flyleaf 初始化失败（通常是 FFmpeg DLL 缺失）：{App.FlyleafInitError}\n" +
                "请在 Windows 上运行：cargo xtask build-winui3 --release（或执行 scripts/fetch_ffmpeg_win.ps1）";
            ShowParseError(unavailableMsg);
            return null;
        }

        try
        {
            var p = new FlyleafPlayerService(_dq);
            p.Error += (_, msg) => _dq.TryEnqueue(() =>
            {
                try { ShowPlayerError(msg); } catch { }
            });
            p.Info += (_, msg) => _dq.TryEnqueue(() =>
            {
                try { ShowPlayerInfo(msg); } catch { }
            });
            BindFlyleafHostPlayer(p);
            return p;
        }
        catch (Exception ex)
        {
            unavailableMsg =
                $"Flyleaf 初始化失败（通常是 FFmpeg DLL 缺失）：{ex.Message}\n" +
                "请在 Windows 上运行：cargo xtask build-winui3 --release（或执行 scripts/fetch_ffmpeg_win.ps1）";
            ShowParseError(unavailableMsg);
            return null;
        }
    }

    private void OnFlyleafHostLoaded(object sender, RoutedEventArgs e)
    {
        BindFlyleafHostPlayer();
    }

    private void BindFlyleafHostPlayer(FlyleafPlayerService? service = null)
    {
        var s = service ?? _player;
        if (s is null)
        {
            return;
        }

        if (ReferenceEquals(FlyleafHost.Player, s.Player))
        {
            return;
        }

        try
        {
            FlyleafHost.Player = s.Player;
        }
        catch (Exception ex)
        {
            ShowParseError($"绑定播放器失败：{ex.Message}");
        }
    }

    private void UnbindFlyleafHostPlayer()
    {
        try { FlyleafHost.Player = null; } catch { }
    }

    protected override void OnNavigatedTo(Microsoft.UI.Xaml.Navigation.NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ShowParsePanelOnly();
        if (e.Parameter is string input)
        {
            InputBox.Text = input;
            _ = DecodeAndShowAsync(input);
        }
        UpdateDanmakuPane();
    }

    protected override async void OnNavigatedFrom(Microsoft.UI.Xaml.Navigation.NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        try
        {
            Interlocked.Increment(ref _playSeq);
            _backend.DanmakuMessageReceived -= OnDanmakuMessage;

            try { _playCts?.Cancel(); } catch { }
            _playCts?.Dispose();
            _playCts = null;

            _danmakuToggleHideCts?.Cancel();
            _danmakuToggleHideCts?.Dispose();
            _danmakuToggleHideCts = null;

            _danmakuAnimCts?.Cancel();
            _danmakuAnimCts?.Dispose();
            _danmakuAnimCts = null;

            _decodeCts?.Cancel();
            _decodeCts?.Dispose();
            _decodeCts = null;
            _lastPlayRequest = null;

            await RunOnUiAsync(() =>
            {
                try { _player?.Stop(); } catch { }
                UnbindFlyleafHostPlayer();
            });

            if (_sessionId is not null)
            {
                try { await _backend.CloseLiveAsync(_sessionId, CancellationToken.None); } catch { }
                _sessionId = null;
            }

            await RunOnUiAsync(() =>
            {
                try { _player?.Dispose(); } catch { }
            });

            await Task.Run(() =>
            {
                try { _backend.Dispose(); } catch { }
            });
        }
        catch
        {
            // ignore - page is being torn down
        }
    }

    private void ShowParsePanelOnly()
    {
        _manifest = null;
        VariantCards.Clear();
        _lastInput = null;
        _lastPlayRequest = null;
        SetMode(LiveMode.Parse);
        ParseStatusBar.IsOpen = false;
        PlayerStatusBar.IsOpen = false;
        Bindings.Update();

        ManifestOverview.Visibility = Visibility.Collapsed;
        EmptyHint.Text = "请先在上方输入直播间地址并点击解析。";
        EmptyHint.Visibility = Visibility.Visible;
    }

    private async void OnParseClicked(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        try
        {
            var input = (InputBox.Text ?? "").Trim();
            if (string.IsNullOrWhiteSpace(input))
            {
                ShowParseError("请输入直播间地址。");
                return;
            }

            await DecodeAndShowAsync(input);
        }
        catch (Exception ex)
        {
            try { ShowParseError(ex.Message); } catch { }
        }
    }

    private async Task DecodeAndShowAsync(string input)
    {
        _decodeCts?.Cancel();
        _decodeCts?.Dispose();
        _decodeCts = new CancellationTokenSource(TimeSpan.FromSeconds(20));
        var ct = _decodeCts.Token;

        await RunOnUiAsync(() =>
        {
            ParseBtn.IsEnabled = false;
            InputBox.IsEnabled = false;
            ShowParseInfo("解析中…");
        });

        try
        {
            var man = await _backend.DecodeManifestAsync(input, ct);
            await RunOnUiAsync(() =>
            {
                _manifest = man;
                _lastInput = man.RawInput;

                VariantCards.Clear();
                foreach (var v in man.Variants ?? Array.Empty<StreamVariant>())
                {
                    VariantCards.Add(LiveVariantCardVm.From(man, v));
                }

                SetMode(LiveMode.Select);

                if (VariantCards.Count == 0)
                {
                    EmptyHint.Text = "解析成功，但没有返回可用线路/清晰度。";
                    EmptyHint.Visibility = Visibility.Visible;
                }
                else
                {
                    EmptyHint.Visibility = Visibility.Collapsed;
                }
                ManifestOverview.Visibility = Visibility.Visible;
                Bindings.Update();

                if (_player is null && !string.IsNullOrWhiteSpace(_playerUnavailableMsg))
                {
                    ShowParseError(_playerUnavailableMsg);
                }
                else
                {
                    ParseStatusBar.IsOpen = false;
                }
            });
        }
        catch (OperationCanceledException)
        {
            var hint = TryGetDaemonLogPath();
            var msg = "解析超时/已取消，请重试。";
            if (!string.IsNullOrWhiteSpace(hint))
            {
                msg += $"\n（daemon 日志：{hint}）";
            }
            await RunOnUiAsync(() =>
            {
                ShowParseError(msg);
                SetMode(LiveMode.Parse);
            });
        }
        catch (Exception ex)
        {
            var hint = TryGetDaemonLogPath();
            var msg = ex.Message;
            if (!string.IsNullOrWhiteSpace(hint))
            {
                msg += $"\n（daemon 日志：{hint}）";
            }
            await RunOnUiAsync(() =>
            {
                ShowParseError(msg);
                SetMode(LiveMode.Parse);
            });
        }
        finally
        {
            await RunOnUiAsync(() =>
            {
                ParseBtn.IsEnabled = true;
                InputBox.IsEnabled = true;
            });
        }
    }

    private void ShowParseError(string msg)
    {
        ParseStatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error;
        ParseStatusBar.Title = "失败";
        ParseStatusBar.Message = msg;
        ParseStatusBar.IsOpen = true;
    }

    private void ShowParseInfo(string msg)
    {
        ParseStatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Informational;
        ParseStatusBar.Title = "提示";
        ParseStatusBar.Message = msg;
        ParseStatusBar.IsOpen = true;
    }

    private async void OnVariantItemClick(object sender, ItemClickEventArgs e)
    {
        try
        {
            if (e.ClickedItem is not LiveVariantCardVm vm)
            {
                return;
            }

            Image? sourceCover = null;
            try
            {
                if (VariantGrid.ContainerFromItem(vm) is GridViewItem gvi &&
                    gvi.ContentTemplateRoot is FrameworkElement root)
                {
                    sourceCover = root.FindName("CardCover") as Image;
                }
            }
            catch
            {
                // ignore
            }

            await BeginPlayFromCardAsync(vm, sourceCover);
        }
        catch (Exception ex)
        {
            try { ShowPlayerError(ex.Message); } catch { }
        }
    }

    private async Task BeginPlayFromCardAsync(LiveVariantCardVm vm, Image? sourceCover)
    {
        if (_player is null)
        {
            ShowParseError(_playerUnavailableMsg ?? "播放组件未初始化（Flyleaf/FFmpeg 依赖缺失或初始化失败）。");
            return;
        }

        var input = (_lastInput ?? (InputBox.Text ?? "")).Trim();
        if (string.IsNullOrWhiteSpace(input))
        {
            ShowParseError("请输入直播间地址。");
            return;
        }

        await BeginPlayAsync(input, vm.VariantId, sourceCover?.Source, sourceCover);
    }

    private async Task BeginPlayAsync(
        string input,
        string variantId,
        object? coverRef,
        Image? animationSourceCover
    )
    {
        if (_player is null)
        {
            throw new InvalidOperationException("播放器未初始化");
        }

        var playSeq = Interlocked.Increment(ref _playSeq);

        _playCts?.Cancel();
        _playCts?.Dispose();
        _playCts = new CancellationTokenSource();
        var ct = _playCts.Token;

        ConnectedAnimation? anim = null;
        if (animationSourceCover?.Source is not null)
        {
            try
            {
                anim = ConnectedAnimationService.GetForCurrentView().PrepareToAnimate("liveHeroCover", animationSourceCover);
            }
            catch
            {
                anim = null;
            }
        }

        _lastPlayRequest = new LastPlayRequest
        {
            Input = input,
            VariantId = variantId,
            CoverRef = coverRef,
            PlaySeq = playSeq,
        };

        VariantGrid.IsEnabled = false;
        _danmakuExpanded = false;
        _danmakuWidthPx = Math.Clamp(_danmakuWidthPx, DanmakuMinWidthPx, DanmakuMaxWidthPx);
        SetMode(LiveMode.Playing);
        BindFlyleafHostPlayer();
        SetPlayUiState(PlayUiState.Opening, null);
        PlayerStatusBar.IsOpen = false;
        ShowPlayerInfo("正在打开直播…");

        if (coverRef is Microsoft.UI.Xaml.Media.ImageSource src)
        {
            HeroCover.Source = src;
            HeroCover.Visibility = Visibility.Visible;
            HeroCover.Opacity = 1;
        }
        else
        {
            HeroCover.Source = null;
            HeroCover.Visibility = Visibility.Collapsed;
        }

        if (anim is not null)
        {
            try
            {
                anim.TryStart(HeroCover);
            }
            catch
            {
                // ignore
            }
        }

        try
        {
            await StopCurrentAsync();

            var res = await OpenLiveWithRetryAsync(input, variantId, ct);
            if (playSeq != _playSeq || ct.IsCancellationRequested)
            {
                try { await _backend.CloseLiveAsync(res.SessionId, CancellationToken.None); } catch { }
                return;
            }

            if (!string.IsNullOrWhiteSpace(variantId) &&
                !string.Equals(variantId.Trim(), (res.VariantId ?? "").Trim(), StringComparison.Ordinal))
            {
                ShowPlayerInfo($"线路回执不一致：请求={variantId}，返回={res.VariantId}");
            }

            var shortUrl = (res.Url ?? "").Trim();
            if (shortUrl.Length > 96)
            {
                shortUrl = shortUrl[..96] + "...";
            }
            ShowPlayerInfo($"回执线路：{(string.IsNullOrWhiteSpace(res.VariantId) ? "-" : res.VariantId)}，备链数：{res.BackupUrls?.Length ?? 0}\n{shortUrl}");

            await RunOnUiAsync(() =>
            {
                if (playSeq != _playSeq)
                {
                    return;
                }
                _sessionId = res.SessionId;
                Rows.Clear();
            });

            await _player.PlayAsync(
                res.Site ?? "",
                res.Url ?? "",
                res.BackupUrls ?? Array.Empty<string>(),
                res.Referer,
                res.UserAgent,
                ct,
                _playOptions
            );
            if (playSeq != _playSeq || ct.IsCancellationRequested)
            {
                return;
            }

            SetPlayUiState(PlayUiState.Playing, null);
            await FadeOutHeroCoverAsync();
        }
        catch (OperationCanceledException)
        {
            if (playSeq == _playSeq && !ct.IsCancellationRequested)
            {
                await StopCurrentAsync();
                await RunOnUiAsync(() =>
                {
                    if (playSeq != _playSeq)
                    {
                        return;
                    }
                    SetPlayUiState(PlayUiState.Failed, "打开直播超时/已取消，请点击重试。");
                    ShowPlayerError("打开直播超时/已取消，请点击重试。");
                    VariantGrid.IsEnabled = true;
                });
            }
        }
        catch (RemoteInvocationException ex)
        {
            await StopCurrentAsync();
            await RunOnUiAsync(() =>
            {
                if (playSeq != _playSeq)
                {
                    return;
                }
                var msg = BuildRemoteInvokeMessage(ex);
                SetPlayUiState(PlayUiState.Failed, msg);
                ShowPlayerError(msg);
                VariantGrid.IsEnabled = true;
            });
        }
        catch (Exception ex)
        {
            await StopCurrentAsync();
            await RunOnUiAsync(() =>
            {
                if (playSeq != _playSeq)
                {
                    return;
                }
                SetPlayUiState(PlayUiState.Failed, ex.Message);
                ShowPlayerError(ex.Message);
                VariantGrid.IsEnabled = true;
            });
        }
        finally
        {
            if (playSeq == _playSeq && _mode == LiveMode.Playing)
            {
                await RunOnUiAsync(() => VariantGrid.IsEnabled = true);
            }

            if (playSeq == _playSeq && _playUiState != PlayUiState.Playing)
            {
                await RunOnUiAsync(() =>
                {
                    HeroCover.Visibility = Visibility.Collapsed;
                    HeroCover.Source = null;
                    HeroCover.Opacity = 1;
                });
            }
        }
    }

    private async Task<LiveOpenResult> OpenLiveWithRetryAsync(
        string input,
        string variantId,
        CancellationToken ct
    )
    {
        Exception? last = null;
        const int totalAttempts = 2;

        for (var attempt = 1; attempt <= totalAttempts; attempt++)
        {
            try
            {
                ct.ThrowIfCancellationRequested();
                ShowPlayerInfo($"会话打开[{attempt}/{totalAttempts}]…");
                using var openCts = CancellationTokenSource.CreateLinkedTokenSource(ct);
                openCts.CancelAfter(_playOptions.OpenTimeout);
                return await _backend.OpenLiveAsync(input, variantId, openCts.Token);
            }
            catch (OperationCanceledException ex) when (!ct.IsCancellationRequested)
            {
                last = ex;
                ShowPlayerInfo($"会话打开超时[{attempt}/{totalAttempts}]");
            }
            catch (Exception ex)
            {
                last = ex;
                ShowPlayerInfo($"会话打开失败[{attempt}/{totalAttempts}]：{ex.Message}");
            }

            if (attempt < totalAttempts)
            {
                await Task.Delay(_playOptions.RetryDelay, ct);
            }
        }

        throw last ?? new Exception("live.open failed");
    }

    private string BuildRemoteInvokeMessage(RemoteInvocationException ex)
    {
        var msg = $"RPC 调用失败：{ex.Message}";
        var hint = TryGetDaemonLogPath();
        if (!string.IsNullOrWhiteSpace(hint))
        {
            msg += $"\n（daemon 日志：{hint}）";
        }
        return msg;
    }

    private Task FadeOutHeroCoverAsync()
    {
        return RunOnUiAsync(() =>
        {
            if (HeroCover.Visibility != Visibility.Visible)
            {
                return;
            }

            try
            {
                var sb = new Storyboard();
                var da = new DoubleAnimation
                {
                    To = 0,
                    Duration = new Duration(TimeSpan.FromMilliseconds(180)),
                    EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
                };
                Storyboard.SetTarget(da, HeroCover);
                Storyboard.SetTargetProperty(da, "Opacity");
                sb.Children.Add(da);
                sb.Completed += (_, _) =>
                {
                    HeroCover.Visibility = Visibility.Collapsed;
                    HeroCover.Source = null;
                    HeroCover.Opacity = 1;
                };
                sb.Begin();
            }
            catch
            {
                HeroCover.Visibility = Visibility.Collapsed;
                HeroCover.Source = null;
                HeroCover.Opacity = 1;
            }
        });
    }

    private async void OnRetryPlayClicked(object sender, RoutedEventArgs e)
    {
        try
        {
            var req = _lastPlayRequest;
            if (_mode != LiveMode.Playing || _playUiState != PlayUiState.Failed || req is null)
            {
                return;
            }

            await BeginPlayAsync(req.Input, req.VariantId, req.CoverRef, null);
        }
        catch (Exception ex)
        {
            ShowPlayerError($"重试失败：{ex.Message}");
        }
    }

    private async void OnBackToSelectFromFailedClicked(object sender, RoutedEventArgs e)
    {
        await ReturnToSelectAsync();
    }

    private async void OnBackToSelect(object sender, RoutedEventArgs e)
    {
        await ReturnToSelectAsync();
    }

    private async Task ReturnToSelectAsync()
    {
        try
        {
            Interlocked.Increment(ref _playSeq);
            try
            {
                _playCts?.Cancel();
            }
            catch
            {
                // ignore
            }

            var stopTask = StopCurrentAsync();
            _lastPlayRequest = null;
            SetMode(LiveMode.Select);
            await stopTask;
        }
        catch (Exception ex)
        {
            try { ShowPlayerError($"返回失败：{ex.Message}"); } catch { }
            try { SetMode(LiveMode.Select); } catch { }
        }
    }

    private async Task StopCurrentAsync()
    {
        await RunOnUiAsync(() =>
        {
            try
            {
                _player?.Stop();
            }
            catch
            {
                // ignore
            }
        });

        var sid = _sessionId;
        _sessionId = null;

        if (sid is not null)
        {
            try { await _backend.CloseLiveAsync(sid, CancellationToken.None); } catch { }
        }

        try
        {
            await RunOnUiAsync(() => Rows.Clear());
        }
        catch
        {
            // ignore
        }
    }

    private async void OnToggleDanmaku(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        try
        {
            if (_danmakuExpanded && _mode == LiveMode.Playing)
            {
                // Best-effort: persist current splitter width before collapsing.
                try
                {
                    var w = (int)Math.Round(DanmakuCol.ActualWidth);
                    _danmakuWidthPx = Math.Clamp(w, DanmakuMinWidthPx, DanmakuMaxWidthPx);
                }
                catch
                {
                    // ignore
                }
            }

            _danmakuExpanded = !_danmakuExpanded;
            await AnimateDanmakuPaneAsync(_danmakuExpanded);
        }
        catch
        {
            // ignore
        }
    }

    private void HideDanmakuToggleImmediately()
    {
        _danmakuToggleHideCts?.Cancel();
        _danmakuToggleHideCts?.Dispose();
        _danmakuToggleHideCts = null;

        try
        {
            DanmakuToggleBtn.Opacity = 0;
            DanmakuToggleBtn.IsHitTestVisible = false;
        }
        catch
        {
            // ignore
        }
    }

    private void ShowDanmakuToggle()
    {
        _danmakuToggleHideCts?.Cancel();
        _danmakuToggleHideCts?.Dispose();
        _danmakuToggleHideCts = null;

        try
        {
            DanmakuToggleBtn.Opacity = 1;
            DanmakuToggleBtn.IsHitTestVisible = true;
        }
        catch
        {
            // ignore
        }
    }

    private async Task HideDanmakuToggleWithDelayAsync(int delayMs = 300)
    {
        try
        {
            _danmakuToggleHideCts?.Cancel();
            _danmakuToggleHideCts?.Dispose();
            _danmakuToggleHideCts = new CancellationTokenSource();
            var ct = _danmakuToggleHideCts.Token;

            await Task.Delay(delayMs, ct);

            if (ct.IsCancellationRequested || _mode != LiveMode.Playing)
            {
                return;
            }

            _dq.TryEnqueue(() =>
            {
                try
                {
                    if (_mode != LiveMode.Playing)
                    {
                        return;
                    }
                    DanmakuToggleBtn.Opacity = 0;
                    DanmakuToggleBtn.IsHitTestVisible = false;
                }
                catch
                {
                    // ignore
                }
            });
        }
        catch
        {
            // ignore - best effort UI
        }
    }

    private void OnDanmakuToggleHotZonePointerEntered(
        object sender,
        Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e
    )
    {
        if (_mode != LiveMode.Playing)
        {
            return;
        }
        ShowDanmakuToggle();
    }

    private void OnDanmakuToggleHotZonePointerExited(
        object sender,
        Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e
    )
    {
        if (_mode != LiveMode.Playing)
        {
            return;
        }
        _ = HideDanmakuToggleWithDelayAsync();
    }

    private void OnDanmakuTogglePointerEntered(
        object sender,
        Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e
    )
    {
        if (_mode != LiveMode.Playing)
        {
            return;
        }
        ShowDanmakuToggle();
    }

    private void OnDanmakuTogglePointerExited(
        object sender,
        Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e
    )
    {
        if (_mode != LiveMode.Playing)
        {
            return;
        }
        _ = HideDanmakuToggleWithDelayAsync();
    }

    private void UpdateDanmakuToggleIcon()
    {
        try
        {
            DanmakuToggleIcon.Glyph = _danmakuExpanded ? "\uE76B" : "\uE76C"; // ChevronLeft / ChevronRight
        }
        catch
        {
            // ignore
        }
    }

    private void UpdateDanmakuPane()
    {
        if (_mode != LiveMode.Playing)
        {
            DanmakuCol.Width = new GridLength(0);
            SplitterCol.Width = new GridLength(0);
            DanmakuPane.Visibility = Visibility.Collapsed;
            DanmakuSplitter.Visibility = Visibility.Collapsed;
            DanmakuToggleHotZone.Visibility = Visibility.Collapsed;
            HideDanmakuToggleImmediately();
            return;
        }

        DanmakuToggleHotZone.Visibility = Visibility.Visible;
        if (_danmakuExpanded)
        {
            DanmakuCol.Width = new GridLength(_danmakuWidthPx);
            SplitterCol.Width = new Microsoft.UI.Xaml.GridLength(SplitterWidthPx);
            DanmakuPane.Visibility = Microsoft.UI.Xaml.Visibility.Visible;
            DanmakuSplitter.Visibility = Microsoft.UI.Xaml.Visibility.Visible;
            UpdateDanmakuToggleIcon();
        }
        else
        {
            DanmakuCol.Width = new Microsoft.UI.Xaml.GridLength(0);
            SplitterCol.Width = new Microsoft.UI.Xaml.GridLength(0);
            DanmakuPane.Visibility = Microsoft.UI.Xaml.Visibility.Collapsed;
            DanmakuSplitter.Visibility = Microsoft.UI.Xaml.Visibility.Collapsed;
            UpdateDanmakuToggleIcon();
        }
    }

    private async Task AnimateDanmakuPaneAsync(bool expand)
    {
        if (_mode != LiveMode.Playing)
        {
            UpdateDanmakuPane();
            return;
        }

        _danmakuAnimCts?.Cancel();
        _danmakuAnimCts?.Dispose();
        _danmakuAnimCts = new CancellationTokenSource();
        var ct = _danmakuAnimCts.Token;

        var fromWidth = (int)Math.Round(DanmakuCol.ActualWidth);
        var toWidth = expand ? _danmakuWidthPx : 0;

        if (expand)
        {
            await RunOnUiAsync(() =>
            {
                DanmakuPane.Visibility = Visibility.Visible;
                DanmakuPane.Opacity = 0;
                DanmakuPane.Translation = new Vector3(12, 0, 0);
                DanmakuSplitter.Visibility = Visibility.Visible;
                DanmakuSplitter.IsHitTestVisible = false;
                SplitterCol.Width = new GridLength(SplitterWidthPx);
            });
        }
        else
        {
            await RunOnUiAsync(() =>
            {
                DanmakuSplitter.IsHitTestVisible = false;
            });
        }

        const int durationMs = 220;
        const int frameMs = 16;
        var frames = Math.Max(10, durationMs / frameMs);

        for (var i = 0; i <= frames; i++)
        {
            ct.ThrowIfCancellationRequested();
            var t = (double)i / frames;
            var e = 1.0 - Math.Pow(1.0 - t, 3); // easeOutCubic
            var w = (int)Math.Round(fromWidth + (toWidth - fromWidth) * e);
            w = Math.Clamp(w, 0, DanmakuMaxWidthPx);

            await RunOnUiAsync(() =>
            {
                DanmakuCol.Width = new GridLength(w);
                SplitterCol.Width = new GridLength(w == 0 ? 0 : SplitterWidthPx);

                var p = expand ? e : (1.0 - e);
                DanmakuPane.Opacity = p;
                DanmakuPane.Translation = new Vector3((float)((1.0 - p) * 12.0), 0, 0);
            });

            await Task.Delay(frameMs, ct);
        }

        await RunOnUiAsync(() =>
        {
            DanmakuSplitter.IsHitTestVisible = true;
            UpdateDanmakuToggleIcon();
            if (!expand)
            {
                DanmakuPane.Visibility = Visibility.Collapsed;
                DanmakuPane.Opacity = 1;
                DanmakuPane.Translation = Vector3.Zero;
                DanmakuSplitter.Visibility = Visibility.Collapsed;
            }
            else
            {
                DanmakuPane.Opacity = 1;
                DanmakuPane.Translation = Vector3.Zero;
                DanmakuSplitter.Visibility = Visibility.Visible;
            }
        });
    }

    private void SetMode(LiveMode mode)
    {
        _mode = mode;

        ParsePanel.Visibility = mode == LiveMode.Playing ? Visibility.Collapsed : Visibility.Visible;
        SelectPane.Visibility = mode == LiveMode.Playing ? Visibility.Collapsed : Visibility.Visible;
        PlayerPane.Visibility = mode == LiveMode.Playing ? Visibility.Visible : Visibility.Collapsed;
        if (mode == LiveMode.Playing)
        {
            HideDanmakuToggleImmediately();
            ShowDanmakuToggle();
            _ = HideDanmakuToggleWithDelayAsync(1500);
        }
        else
        {
            SetPlayUiState(PlayUiState.Idle, null);
        }
        UpdateDanmakuPane();
    }

    private void SetPlayUiState(PlayUiState state, string? failureMessage)
    {
        _playUiState = state;
        PlayerOpeningOverlay.Visibility = state == PlayUiState.Opening
            ? Visibility.Visible
            : Visibility.Collapsed;
        PlayerFailedOverlay.Visibility = state == PlayUiState.Failed
            ? Visibility.Visible
            : Visibility.Collapsed;

        if (state == PlayUiState.Failed)
        {
            PlayerFailedMessage.Text = string.IsNullOrWhiteSpace(failureMessage)
                ? "未知错误"
                : failureMessage;
            RetryPlayBtn.IsEnabled = _lastPlayRequest is not null;
        }
        else
        {
            PlayerFailedMessage.Text = "";
            RetryPlayBtn.IsEnabled = false;
        }
    }

    private void OnVariantCardPointerEntered(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (sender is not Border b)
        {
            return;
        }
        try
        {
            b.Translation = new Vector3(0, -2, 0);
            b.Shadow = new Microsoft.UI.Xaml.Media.ThemeShadow();
        }
        catch
        {
            // ignore
        }
    }

    private void OnVariantCardPointerExited(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (sender is not Border b)
        {
            return;
        }
        try
        {
            b.Translation = Vector3.Zero;
            b.Shadow = null;
        }
        catch
        {
            // ignore
        }
    }

    private void OnVariantCardPointerPressed(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (sender is not Border b)
        {
            return;
        }
        try
        {
            b.Translation = new Vector3(0, 0, 0);
            b.Opacity = 0.96;
        }
        catch
        {
            // ignore
        }
    }

    private void OnVariantCardPointerReleased(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (sender is not Border b)
        {
            return;
        }
        try
        {
            b.Opacity = 1;
        }
        catch
        {
            // ignore
        }
    }

    private void ShowPlayerError(string msg)
    {
        PlayerStatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error;
        PlayerStatusBar.Title = "播放失败";
        PlayerStatusBar.Message = msg;
        PlayerStatusBar.IsOpen = true;
    }

    private void ShowPlayerInfo(string msg)
    {
        PlayerStatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Informational;
        PlayerStatusBar.Title = "播放器";
        PlayerStatusBar.Message = msg;
        PlayerStatusBar.IsOpen = true;
    }

    private void OnDanmakuMessage(object? sender, DanmakuMessage msg)
    {
        var sid = _sessionId;
        if (sid is null || msg.SessionId != sid)
        {
            return;
        }

        _dq.TryEnqueue(() =>
        {
            try
            {
                if (_sessionId != sid)
                {
                    return;
                }

                var row = new DanmakuRowVm(msg.User, msg.Text);
                Rows.Add(row);
                if (Rows.Count > 5000)
                {
                    Rows.RemoveAt(0);
                }
                if (_danmakuExpanded)
                {
                    try
                    {
                        DanmakuList.ScrollIntoView(row);
                    }
                    catch
                    {
                        // ignore
                    }
                }

                if (!string.IsNullOrWhiteSpace(msg.ImageUrl))
                {
                    _ = TryLoadEmoteAsync(sid, row, msg.ImageUrl!);
                }
            }
            catch
            {
                // ignore
            }
        });
    }

    private async Task TryLoadEmoteAsync(string sid, DanmakuRowVm row, string url)
    {
        if (_sessionId != sid)
        {
            return;
        }

        await _imageSem.WaitAsync();
        try
        {
            if (_sessionId != sid)
            {
                return;
            }

            var res = await _backend.FetchDanmakuImageAsync(sid, url, CancellationToken.None);
            if (string.IsNullOrWhiteSpace(res.Base64))
            {
                return;
            }

            var bytes = Convert.FromBase64String(res.Base64);
            using var ms = new InMemoryRandomAccessStream();
            await ms.WriteAsync(bytes.AsBuffer());
            ms.Seek(0);

            await RunOnUiAsync(async () =>
            {
                if (_sessionId != sid)
                {
                    return;
                }
                var bmp = new BitmapImage();
                ms.Seek(0);
                await bmp.SetSourceAsync(ms);
                row.Emote = bmp;
            });
        }
        catch
        {
            // ignore
        }
        finally
        {
            _imageSem.Release();
        }
    }

    private string? TryGetDaemonLogPath()
    {
        try
        {
            if (_backend is DaemonLiveBackend)
            {
                return DaemonClient.Instance.DaemonLogPath;
            }
        }
        catch
        {
            // ignore
        }

        return null;
    }
}

public sealed class LiveVariantCardVm
{
    public string VariantId { get; set; } = "";
    public string VariantText { get; set; } = "";
    public string Title { get; set; } = "";
    public string Streamer { get; set; } = "";
    public string StatusText { get; set; } = "";
    public BitmapImage? Cover { get; set; }

    public static LiveVariantCardVm From(LivestreamDecodeManifestResult man, StreamVariant v)
    {
        var cover = TryCreateBitmap(man.Info?.Cover);
        var status = man.Info?.IsLiving == true ? "直播中" : "未开播/离线";
        var streamer = string.IsNullOrWhiteSpace(man.Info?.Name) ? "主播：-" : $"主播：{man.Info!.Name}";
        var quality = v.Quality != 0 ? $"（{v.Quality}）" : "";
        return new LiveVariantCardVm
        {
            VariantId = v.Id ?? "",
            VariantText = $"清晰度：{(v.Label ?? "-")}{quality}",
            Title = man.Info?.Title ?? "-",
            Streamer = streamer,
            StatusText = $"状态：{status}",
            Cover = cover,
        };
    }

    private static BitmapImage? TryCreateBitmap(string? url)
    {
        if (string.IsNullOrWhiteSpace(url))
        {
            return null;
        }

        try
        {
            if (!Uri.TryCreate(url.Trim(), UriKind.Absolute, out var u))
            {
                return null;
            }
            return new BitmapImage(u);
        }
        catch
        {
            return null;
        }
    }
}

public sealed class DanmakuRowVm : INotifyPropertyChanged
{
    public event PropertyChangedEventHandler? PropertyChanged;

    public DanmakuRowVm(string user, string text)
    {
        User = user;
        Text = text;
    }

    public string User { get; }
    public string Text { get; }

    public string DisplayText => $"{User}: {Text}";

    private BitmapImage? _emote;
    public BitmapImage? Emote
    {
        get => _emote;
        set
        {
            if (ReferenceEquals(_emote, value))
            {
                return;
            }
            _emote = value;
            OnPropertyChanged();
        }
    }

    private void OnPropertyChanged([CallerMemberName] string? name = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
    }
}
