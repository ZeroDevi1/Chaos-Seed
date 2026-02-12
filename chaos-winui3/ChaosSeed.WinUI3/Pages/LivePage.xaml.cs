using System.Collections.ObjectModel;
using System.Collections.Generic;
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
using Microsoft.UI.Xaml.Controls.Primitives;
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
    private readonly Queue<string> _playerLogTail = new();
    private const int PlayerLogTailMaxLines = 6;
    private CancellationTokenSource? _backBtnHideCts;
    private DispatcherQueueTimer? _playerControlsHideTimer;
    private bool _playerPaused;
    private bool _volumeSync;
    private bool _inAppFullscreen;
    private XamlRoot? _fullscreenXamlRoot;
    private bool _systemFullscreenRequested;
    private bool _debugPlayerOverlay;

    private string? _sessionId;
    private string? _playingVariantId;
    private string? _playingVariantLabel;
    private long _emoteReq;
    private long _emoteOk;
    private long _emoteFail;
    private string? _lastEmoteErr;
    private long _lastEmoteDebugAtMs;
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
        InitPlayerControlsUi();
        FlyleafHost.Loaded += OnFlyleafHostLoaded;
        _backend = LiveBackendFactory.Create();
        _backend.DanmakuMessageReceived += OnDanmakuMessage;
        _player = TryInitPlayer(out _playerUnavailableMsg);
        BindFlyleafHostPlayer(_player);
        SettingsService.Instance.SettingsChanged += OnSettingsChanged;
        ApplyDebugUiFromSettings();
        HideBackToSelectImmediately();
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

    private void InitPlayerControlsUi()
    {
        try
        {
            PlayerTitleText.Text = "";
            QualityText.Text = "清晰度";
            VolumeSlider.Value = 100;
            _playerControlsHideTimer = _dq.CreateTimer();
            _playerControlsHideTimer.IsRepeating = false;
            _playerControlsHideTimer.Interval = TimeSpan.FromSeconds(10);
            _playerControlsHideTimer.Tick += (_, _) =>
            {
                try { HidePlayerControlsImmediately(); } catch { }
            };
            HidePlayerControlsImmediately();
        }
        catch
        {
            // ignore
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
            SettingsService.Instance.SettingsChanged -= OnSettingsChanged;

            try { _playCts?.Cancel(); } catch { }
            _playCts?.Dispose();
            _playCts = null;

            _backBtnHideCts?.Cancel();
            _backBtnHideCts?.Dispose();
            _backBtnHideCts = null;

            try { _playerControlsHideTimer?.Stop(); } catch { }
            _playerControlsHideTimer = null;

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
                try { ExitSystemFullscreenIfNeeded(); } catch { }
                try { ExitInAppFullscreenIfNeeded(); } catch { }
            });

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

        var preferSystemFullscreen = SettingsService.Instance.Current.LiveDefaultFullscreen;
        _systemFullscreenRequested = preferSystemFullscreen;

        ConnectedAnimation? anim = null;
        if (animationSourceCover?.Source is not null)
        {
            try
            {
                anim = ConnectedAnimationService.GetForCurrentView().PrepareToAnimate("liveHeroCover", animationSourceCover);
                try { anim.Configuration = new DirectConnectedAnimationConfiguration(); } catch { }
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
        if (preferSystemFullscreen)
        {
            EnterInAppFullscreenIfNeeded();
        }
        else
        {
            ExitInAppFullscreenIfNeeded();
            ExitSystemFullscreenIfNeeded();
        }
        SyncPlayerOverlayFromManifest();
        RebuildQualityFlyout(variantId);
        BindFlyleafHostPlayer();
        await EnsureFlyleafHostReadyAsync(ct);
        SetPlayUiState(PlayUiState.Opening, null);
        _playerLogTail.Clear();
        Interlocked.Exchange(ref _emoteReq, 0);
        Interlocked.Exchange(ref _emoteOk, 0);
        Interlocked.Exchange(ref _emoteFail, 0);
        Volatile.Write(ref _lastEmoteErr, null);
        Interlocked.Exchange(ref _lastEmoteDebugAtMs, 0);
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

        if (preferSystemFullscreen)
        {
            var configured = SettingsService.Instance.Current.LiveFullscreenDelayMs;
            configured = Math.Clamp(configured, 0, 2000);
            var delay = Math.Max(configured, anim is null ? 0 : 180);
            _ = EnterSystemFullscreenAfterDelayAsync(playSeq, delayMs: delay);
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
                _playingVariantId = (res.VariantId ?? "").Trim();
                _playingVariantLabel = (res.VariantLabel ?? "").Trim();
                SyncPlayerOverlayFromManifest(res);
                UpdateQualityUiFromPlayingVariant();
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
            SyncAudioUiFromPlayer();
            SetPausedUi(paused: false);
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

    private async Task EnsureFlyleafHostReadyAsync(CancellationToken ct)
    {
        if (_dq.HasThreadAccess && FlyleafHost.ActualWidth > 1 && FlyleafHost.ActualHeight > 1)
        {
            return;
        }

        var tcs = new TaskCompletionSource<object?>(TaskCreationOptions.RunContinuationsAsynchronously);

        RoutedEventHandler? onLoaded = null;
        SizeChangedEventHandler? onSizeChanged = null;

        void TryComplete()
        {
            try
            {
                if (FlyleafHost.ActualWidth > 1 && FlyleafHost.ActualHeight > 1)
                {
                    tcs.TrySetResult(null);
                }
            }
            catch
            {
                // ignore
            }
        }

        onLoaded = (_, _) => TryComplete();
        onSizeChanged = (_, _) => TryComplete();

        try
        {
            FlyleafHost.Loaded += onLoaded;
            FlyleafHost.SizeChanged += onSizeChanged;
        }
        catch
        {
            return;
        }

        CancellationTokenRegistration ctr = default;
        if (ct.CanBeCanceled)
        {
            ctr = ct.Register(() => tcs.TrySetCanceled(ct));
        }

        // Kick once in case we're already ready but caller is off-thread.
        _dq.TryEnqueue(TryComplete);

        try
        {
            var done = await Task.WhenAny(tcs.Task, Task.Delay(800, ct));
            if (ReferenceEquals(done, tcs.Task))
            {
                await tcs.Task;
            }
            // timeout: proceed best-effort (FlyleafHost may still render after layout completes)
        }
        finally
        {
            ctr.Dispose();
            try { FlyleafHost.Loaded -= onLoaded; } catch { }
            try { FlyleafHost.SizeChanged -= onSizeChanged; } catch { }
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
            ExitSystemFullscreenIfNeeded();
            ExitInAppFullscreenIfNeeded();
            HidePlayerControlsImmediately();
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
            _playingVariantId = null;
            _playingVariantLabel = null;
            _systemFullscreenRequested = false;
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
            HideBackToSelectImmediately();
            HideDanmakuToggleImmediately();
            ShowDanmakuToggle();
            _ = HideDanmakuToggleWithDelayAsync(1500);
            ShowPlayerControls();
        }
        else
        {
            SetPlayUiState(PlayUiState.Idle, null);
            HideBackToSelectImmediately();
            HidePlayerControlsImmediately();
            ExitInAppFullscreenIfNeeded();
            ExitSystemFullscreenIfNeeded();
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

        var canControl = state == PlayUiState.Playing;
        try
        {
            PlayPauseBtn.IsEnabled = canControl;
            MuteBtn.IsEnabled = canControl;
            VolumeSlider.IsEnabled = canControl;
            QualityBtn.IsEnabled = canControl && QualityFlyout.Items.Count > 0;
        }
        catch
        {
            // ignore
        }

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
        AppendPlayerLog(Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error, "播放失败", msg);
    }

    private void ShowPlayerInfo(string msg)
    {
        AppendPlayerLog(Microsoft.UI.Xaml.Controls.InfoBarSeverity.Informational, "播放器", msg);
    }

    private void AppendPlayerLog(
        Microsoft.UI.Xaml.Controls.InfoBarSeverity severity,
        string title,
        string msg
    )
    {
        try
        {
            if (!_debugPlayerOverlay)
            {
                return;
            }

            var lines = (msg ?? "").Replace("\r\n", "\n").Split('\n');
            foreach (var line in lines)
            {
                var trimmed = (line ?? "").Trim();
                if (trimmed.Length == 0)
                {
                    continue;
                }

                _playerLogTail.Enqueue(trimmed);
                while (_playerLogTail.Count > PlayerLogTailMaxLines)
                {
                    _playerLogTail.Dequeue();
                }
            }

            PlayerStatusBar.Severity = severity;
            PlayerStatusBar.Title = title;
            PlayerStatusBar.Message = string.Join("\n", _playerLogTail);
            PlayerStatusBar.IsOpen = true;
        }
        catch
        {
            // ignore
        }
    }

    private void OnSettingsChanged(object? sender, EventArgs e)
    {
        ApplyDebugUiFromSettings();
    }

    private void ApplyDebugUiFromSettings()
    {
        var enabled = SettingsService.Instance.Current.DebugPlayerOverlay;
        _debugPlayerOverlay = enabled;

        _dq.TryEnqueue(() =>
        {
            try
            {
                PlayerStatusBar.Visibility = enabled ? Visibility.Visible : Visibility.Collapsed;
                if (!enabled)
                {
                    PlayerStatusBar.IsOpen = false;
                    _playerLogTail.Clear();
                }
            }
            catch
            {
                // ignore
            }
        });
    }

    private void OnXamlRootChanged(XamlRoot sender, XamlRootChangedEventArgs args)
    {
        _ = args;
        if (!_inAppFullscreen)
        {
            return;
        }

        try { UpdateFullScreenPopupSize(); } catch { }
    }

    private void EnterInAppFullscreenIfNeeded()
    {
        if (_inAppFullscreen)
        {
            UpdateFullScreenPopupSize();
            return;
        }

        XamlRoot? root = null;
        try
        {
            root = (App.MainWindowInstance?.Content as FrameworkElement)?.XamlRoot ?? XamlRoot;
        }
        catch
        {
            root = XamlRoot;
        }

        try
        {
            if (root is not null && !ReferenceEquals(_fullscreenXamlRoot, root))
            {
                if (_fullscreenXamlRoot is not null)
                {
                    try { _fullscreenXamlRoot.Changed -= OnXamlRootChanged; } catch { }
                }
                _fullscreenXamlRoot = root;
                _fullscreenXamlRoot.Changed += OnXamlRootChanged;
            }
        }
        catch
        {
            // ignore
        }

        try
        {
            if (root is not null && !ReferenceEquals(FullScreenPopup.XamlRoot, root))
            {
                FullScreenPopup.XamlRoot = root;
            }
        }
        catch
        {
            // ignore
        }

        UpdateFullScreenPopupSize();

        try
        {
            if (ReferenceEquals(PlayerHost.Content, PlayerSurface))
            {
                PlayerHost.Content = null;
            }
        }
        catch
        {
            // ignore
        }

        try { FullScreenPlayerHost.Content = PlayerSurface; } catch { }
        try { FullScreenPopup.IsOpen = true; } catch { }
        _inAppFullscreen = true;
    }

    private void ExitInAppFullscreenIfNeeded()
    {
        if (!_inAppFullscreen)
        {
            try { FullScreenPopup.IsOpen = false; } catch { }
            return;
        }

        try { FullScreenPopup.IsOpen = false; } catch { }

        try
        {
            if (ReferenceEquals(FullScreenPlayerHost.Content, PlayerSurface))
            {
                FullScreenPlayerHost.Content = null;
            }
        }
        catch
        {
            // ignore
        }

        try { PlayerHost.Content = PlayerSurface; } catch { }

        try
        {
            if (_fullscreenXamlRoot is not null)
            {
                _fullscreenXamlRoot.Changed -= OnXamlRootChanged;
            }
        }
        catch
        {
            // ignore
        }

        _fullscreenXamlRoot = null;
        _inAppFullscreen = false;
    }

    private void UpdateFullScreenPopupSize()
    {
        try
        {
            var xr = _fullscreenXamlRoot ?? FullScreenPopupRoot.XamlRoot ?? XamlRoot;
            if (xr is null)
            {
                return;
            }

            FullScreenPopupRoot.Width = xr.Size.Width;
            FullScreenPopupRoot.Height = xr.Size.Height;
        }
        catch
        {
            // ignore
        }
    }

    private async Task EnterSystemFullscreenAfterDelayAsync(int playSeq, int delayMs)
    {
        try
        {
            if (delayMs > 0)
            {
                await Task.Delay(delayMs);
            }

            if (playSeq != _playSeq || _mode != LiveMode.Playing || !_systemFullscreenRequested)
            {
                return;
            }

            await RunOnUiAsync(() =>
            {
                if (playSeq != _playSeq || _mode != LiveMode.Playing || !_systemFullscreenRequested)
                {
                    return;
                }

                try
                {
                    if (App.MainWindowInstance?.IsSystemFullscreen != true)
                    {
                        App.MainWindowInstance?.TrySetSystemFullscreen(true);
                    }
                }
                catch
                {
                    // ignore
                }
            });
        }
        catch
        {
            // ignore
        }
    }

    private void ExitSystemFullscreenIfNeeded()
    {
        _systemFullscreenRequested = false;
        try
        {
            if (App.MainWindowInstance?.IsSystemFullscreen == true)
            {
                App.MainWindowInstance.TrySetSystemFullscreen(false);
            }
        }
        catch
        {
            // ignore
        }
    }

    private void OnPlayerSurfacePointerEntered(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (_mode != LiveMode.Playing)
        {
            return;
        }
        ShowPlayerControls();
    }

    private void OnPlayerSurfacePointerExited(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (_mode != LiveMode.Playing)
        {
            return;
        }

        try { RestartPlayerControlsHideTimer(TimeSpan.FromMilliseconds(350)); } catch { }
    }

    private void OnPlayerSurfacePointerMoved(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (_mode != LiveMode.Playing)
        {
            return;
        }
        ShowPlayerControls();
    }

    private void ShowPlayerControls()
    {
        if (_mode != LiveMode.Playing)
        {
            return;
        }

        try
        {
            PlayerControlsRoot.Opacity = 1;
            PlayerControlsRoot.IsHitTestVisible = true;
            RestartPlayerControlsHideTimer(TimeSpan.FromSeconds(10));
        }
        catch
        {
            // ignore
        }
    }

    private void HidePlayerControlsImmediately()
    {
        try { _playerControlsHideTimer?.Stop(); } catch { }

        try
        {
            PlayerControlsRoot.Opacity = 0;
            PlayerControlsRoot.IsHitTestVisible = false;
        }
        catch
        {
            // ignore
        }
    }

    private void RestartPlayerControlsHideTimer(TimeSpan delay)
    {
        if (_playerControlsHideTimer is null)
        {
            return;
        }

        try
        {
            _playerControlsHideTimer.Stop();
            _playerControlsHideTimer.Interval = delay;
            _playerControlsHideTimer.Start();
        }
        catch
        {
            // ignore
        }
    }

    private void OnPlayPauseClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ShowPlayerControls();

        if (_playUiState != PlayUiState.Playing || _player is null)
        {
            return;
        }

        try
        {
            if (_playerPaused)
            {
                _player.Player.Play();
                SetPausedUi(paused: false);
            }
            else
            {
                _player.Player.Pause();
                SetPausedUi(paused: true);
            }
        }
        catch
        {
            // ignore
        }
    }

    private void SetPausedUi(bool paused)
    {
        _playerPaused = paused;
        try { PlayPauseIcon.Symbol = paused ? Symbol.Play : Symbol.Pause; } catch { }
    }

    private void OnMuteClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ShowPlayerControls();

        if (_player is null)
        {
            return;
        }

        try
        {
            var muted = _player.Player.Audio.Mute;
            _player.Player.Audio.Mute = !muted;
            SetMuteUi(!muted);
        }
        catch
        {
            // ignore
        }
    }

    private void SetMuteUi(bool muted)
    {
        try { MuteIcon.Symbol = muted ? Symbol.Mute : Symbol.Volume; } catch { }
    }

    private void OnVolumeSliderValueChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        _ = sender;
        ShowPlayerControls();

        if (_volumeSync || _player is null)
        {
            return;
        }

        try
        {
            _player.Player.Audio.Volume = (int)Math.Round(e.NewValue);
        }
        catch
        {
            // ignore
        }
    }

    private void SyncAudioUiFromPlayer()
    {
        if (_player is null)
        {
            return;
        }

        try
        {
            var max = 100.0;
            try
            {
                max = Convert.ToDouble(_player.Player.Config.Audio.VolumeMax);
                if (double.IsNaN(max) || max <= 0)
                {
                    max = 100.0;
                }
            }
            catch
            {
                max = 100.0;
            }

            var vol = Convert.ToDouble(_player.Player.Audio.Volume);
            _volumeSync = true;
            VolumeSlider.Maximum = max;
            VolumeSlider.Value = Math.Clamp(vol, VolumeSlider.Minimum, VolumeSlider.Maximum);
            _volumeSync = false;

            SetMuteUi(_player.Player.Audio.Mute);
        }
        catch
        {
            _volumeSync = false;
        }
    }

    private void SyncPlayerOverlayFromManifest(LiveOpenResult? res = null)
    {
        try
        {
            var title = (res?.Title ?? _manifest?.Info?.Title ?? "").Trim();
            PlayerTitleText.Text = string.IsNullOrWhiteSpace(title) ? "-" : title;
        }
        catch
        {
            // ignore
        }
    }

    private void UpdateQualityUiFromPlayingVariant()
    {
        try
        {
            var id = (_playingVariantId ?? "").Trim();
            var label = (_playingVariantLabel ?? "").Trim();

            if (string.IsNullOrWhiteSpace(label))
            {
                var vars = _manifest?.Variants ?? Array.Empty<StreamVariant>();
                foreach (var v in vars)
                {
                    if (string.Equals((v.Id ?? "").Trim(), id, StringComparison.Ordinal))
                    {
                        label = (v.Label ?? "").Trim();
                        break;
                    }
                }
            }

            QualityText.Text = string.IsNullOrWhiteSpace(label) ? "清晰度" : label;

            foreach (var raw in QualityFlyout.Items)
            {
                if (raw is ToggleMenuFlyoutItem item && item.Tag is string tag)
                {
                    item.IsChecked = string.Equals(tag.Trim(), id, StringComparison.Ordinal);
                }
            }
        }
        catch
        {
            // ignore
        }
    }

    private void RebuildQualityFlyout(string? selectedVariantId)
    {
        try
        {
            QualityFlyout.Items.Clear();

            var vars = new List<StreamVariant>(_manifest?.Variants ?? Array.Empty<StreamVariant>());
            vars.Sort((a, b) =>
            {
                var qa = a.Quality;
                var qb = b.Quality;
                var cmp = qb.CompareTo(qa);
                if (cmp != 0)
                {
                    return cmp;
                }
                return string.CompareOrdinal(a.Label ?? "", b.Label ?? "");
            });

            var target = (selectedVariantId ?? _playingVariantId ?? "").Trim();

            foreach (var v in vars)
            {
                var id = (v.Id ?? "").Trim();
                if (string.IsNullOrWhiteSpace(id))
                {
                    continue;
                }

                var label = (v.Label ?? "").Trim();
                var text = string.IsNullOrWhiteSpace(label) ? id : label;
                if (v.Quality != 0)
                {
                    text += $"（{v.Quality}）";
                }

                var item = new ToggleMenuFlyoutItem
                {
                    Text = text,
                    Tag = id,
                    IsChecked = string.Equals(id, target, StringComparison.Ordinal),
                };
                item.Click += OnQualityFlyoutItemClick;
                QualityFlyout.Items.Add(item);
            }

            QualityBtn.IsEnabled = QualityFlyout.Items.Count > 0;
            foreach (var raw in QualityFlyout.Items)
            {
                if (raw is ToggleMenuFlyoutItem item && item.IsChecked)
                {
                    QualityText.Text = item.Text;
                    break;
                }
            }
        }
        catch
        {
            // ignore
        }
    }

    private async void OnQualityFlyoutItemClick(object sender, RoutedEventArgs e)
    {
        _ = e;
        ShowPlayerControls();

        if (_mode != LiveMode.Playing || _player is null)
        {
            return;
        }

        if (sender is not ToggleMenuFlyoutItem item || item.Tag is not string variantId)
        {
            return;
        }

        if (string.Equals(variantId.Trim(), (_playingVariantId ?? _lastPlayRequest?.VariantId ?? "").Trim(), StringComparison.Ordinal))
        {
            return;
        }

        var input = (_lastPlayRequest?.Input ?? _lastInput ?? (InputBox.Text ?? "")).Trim();
        if (string.IsNullOrWhiteSpace(input))
        {
            return;
        }

        try
        {
            await BeginPlayAsync(input, variantId.Trim(), coverRef: null, animationSourceCover: null);
        }
        catch (Exception ex)
        {
            try { ShowPlayerError($"切换清晰度失败：{ex.Message}"); } catch { }
        }
    }

    private void ShowBackToSelect()
    {
        _backBtnHideCts?.Cancel();
        _backBtnHideCts?.Dispose();
        _backBtnHideCts = null;

        try
        {
            BackToSelectBtn.Opacity = 1;
            BackToSelectBtn.IsHitTestVisible = true;
        }
        catch
        {
            // ignore
        }
    }

    private void HideBackToSelectImmediately()
    {
        _backBtnHideCts?.Cancel();
        _backBtnHideCts?.Dispose();
        _backBtnHideCts = null;

        try
        {
            BackToSelectBtn.Opacity = 0;
            BackToSelectBtn.IsHitTestVisible = false;
        }
        catch
        {
            // ignore
        }
    }

    private async Task HideBackToSelectWithDelayAsync(int delayMs = 350)
    {
        try
        {
            _backBtnHideCts?.Cancel();
            _backBtnHideCts?.Dispose();
            _backBtnHideCts = new CancellationTokenSource();
            var ct = _backBtnHideCts.Token;

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
                    BackToSelectBtn.Opacity = 0;
                    BackToSelectBtn.IsHitTestVisible = false;
                }
                catch
                {
                    // ignore
                }
            });
        }
        catch
        {
            // ignore
        }
    }

    private void OnBackHotZonePointerEntered(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (_mode != LiveMode.Playing)
        {
            return;
        }
        ShowBackToSelect();
    }

    private void OnBackHotZonePointerExited(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (_mode != LiveMode.Playing)
        {
            return;
        }
        _ = HideBackToSelectWithDelayAsync();
    }

    private void OnBackBtnPointerEntered(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (_mode != LiveMode.Playing)
        {
            return;
        }
        ShowBackToSelect();
    }

    private void OnBackBtnPointerExited(object sender, Microsoft.UI.Xaml.Input.PointerRoutedEventArgs e)
    {
        if (_mode != LiveMode.Playing)
        {
            return;
        }
        _ = HideBackToSelectWithDelayAsync();
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
                    Interlocked.Increment(ref _emoteReq);
                    EmitEmoteDebugIfDue();
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
                Interlocked.Increment(ref _emoteFail);
                Volatile.Write(ref _lastEmoteErr, "empty image reply");
                EmitEmoteDebugIfDue();
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

            Interlocked.Increment(ref _emoteOk);
            EmitEmoteDebugIfDue();
        }
        catch (Exception ex)
        {
            Interlocked.Increment(ref _emoteFail);
            var host = "";
            try
            {
                if (Uri.TryCreate(url?.Trim() ?? "", UriKind.Absolute, out var u) && !string.IsNullOrWhiteSpace(u.Host))
                {
                    host = u.Host.Trim();
                }
            }
            catch
            {
                host = "";
            }

            var msg = string.IsNullOrWhiteSpace(host) ? ex.Message : $"{host}: {ex.Message}";
            Volatile.Write(ref _lastEmoteErr, msg);
            EmitEmoteDebugIfDue();
        }
        finally
        {
            _imageSem.Release();
        }
    }

    private void EmitEmoteDebugIfDue()
    {
        try
        {
            if (!_debugPlayerOverlay)
            {
                return;
            }

            var now = Environment.TickCount64;
            var last = Interlocked.Read(ref _lastEmoteDebugAtMs);
            if (now - last < 2000)
            {
                return;
            }

            if (Interlocked.CompareExchange(ref _lastEmoteDebugAtMs, now, last) != last)
            {
                return;
            }

            var req = Interlocked.Read(ref _emoteReq);
            var ok = Interlocked.Read(ref _emoteOk);
            var fail = Interlocked.Read(ref _emoteFail);
            var err = Volatile.Read(ref _lastEmoteErr);

            var msg = $"[emote] req={req} ok={ok} fail={fail}";
            if (!string.IsNullOrWhiteSpace(err))
            {
                msg += $"\nlastErr={err}";
            }

            _dq.TryEnqueue(() =>
            {
                try { ShowPlayerInfo(msg); } catch { }
            });
        }
        catch
        {
            // ignore
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
