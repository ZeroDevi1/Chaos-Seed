using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Numerics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices.WindowsRuntime;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Media.Animation;
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

    private const int DanmakuDefaultWidthPx = 280;
    private const int SplitterWidthPx = 6;
    private const int DanmakuMinWidthPx = 220;
    private const int DanmakuMaxWidthPx = 480;
    public ObservableCollection<DanmakuRowVm> Rows { get; } = new();
    public ObservableCollection<LiveVariantCardVm> VariantCards { get; } = new();

    private readonly DispatcherQueue _dq = DispatcherQueue.GetForCurrentThread();
    private readonly SemaphoreSlim _imageSem = new(4, 4);
    private readonly FlyleafPlayerService? _player;
    private readonly string? _playerUnavailableMsg;

    private string? _sessionId;
    private bool _danmakuExpanded = true;
    private int _danmakuWidthPx = DanmakuDefaultWidthPx;
    private CancellationTokenSource? _danmakuAnimCts;
    private LiveMode _mode = LiveMode.Parse;
    private LivestreamDecodeManifestResult? _manifest;
    private string? _lastInput;
    private CancellationTokenSource? _playCts;
    private CancellationTokenSource? _decodeCts;

    public string OverviewTitle => _manifest?.Info?.Title ?? "";
    public string OverviewStreamer => $"主播：{(_manifest?.Info?.Name ?? "-")}";
    public string OverviewStatus => $"状态：{(_manifest?.Info?.IsLiving == true ? "直播中" : "未开播/离线")}";
    public string OverviewRoom => _manifest is null ? "" : $"房间：{_manifest.Site}:{_manifest.RoomId}";

    public LivePage()
    {
        InitializeComponent();
        DaemonClient.Instance.DanmakuMessageReceived += OnDanmakuMessage;
        _player = TryInitPlayer(out _playerUnavailableMsg);
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
            p.Error += (_, msg) => _dq.TryEnqueue(() => ShowPlayerError(msg));
            p.Info += (_, msg) => _dq.TryEnqueue(() => ShowPlayerInfo(msg));
            FlyleafHost.Player = p.Player;
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
        DaemonClient.Instance.DanmakuMessageReceived -= OnDanmakuMessage;

        _decodeCts?.Cancel();
        _decodeCts?.Dispose();
        _decodeCts = null;

        if (_sessionId is not null)
        {
            try { await DaemonClient.Instance.CloseLiveAsync(_sessionId); } catch { }
        }

        _player?.Dispose();
    }

    private void ShowParsePanelOnly()
    {
        _manifest = null;
        VariantCards.Clear();
        _lastInput = null;
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
        var input = (InputBox.Text ?? "").Trim();
        if (string.IsNullOrWhiteSpace(input))
        {
            ShowParseError("请输入直播间地址。");
            return;
        }

        await DecodeAndShowAsync(input);
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
            var man = await DaemonClient.Instance.DecodeManifestAsync(input, ct);
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
            var hint = DaemonClient.Instance.DaemonLogPath;
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
            var hint = DaemonClient.Instance.DaemonLogPath;
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

        _playCts?.Cancel();
        _playCts = new CancellationTokenSource();
        var ct = _playCts.Token;

        ConnectedAnimation? anim = null;
        if (sourceCover?.Source is not null)
        {
            try
            {
                anim = ConnectedAnimationService.GetForCurrentView().PrepareToAnimate("liveHeroCover", sourceCover);
            }
            catch
            {
                anim = null;
            }
        }

        VariantGrid.IsEnabled = false;
        SetMode(LiveMode.Playing);
        PlayerStatusBar.IsOpen = false;
        ShowPlayerInfo("正在打开直播…");

        if (sourceCover?.Source is not null)
        {
            HeroCover.Source = sourceCover.Source;
            HeroCover.Visibility = Visibility.Visible;
            HeroCover.Opacity = 1;
        }
        else
        {
            HeroCover.Source = null;
            HeroCover.Visibility = Visibility.Collapsed;
        }

        UpdateLayout();
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

            using var openCts = CancellationTokenSource.CreateLinkedTokenSource(ct);
            openCts.CancelAfter(TimeSpan.FromSeconds(20));

            var res = await DaemonClient.Instance.OpenLiveAsync(input, vm.VariantId, openCts.Token);
            await RunOnUiAsync(() =>
            {
                _sessionId = res.SessionId;
                Rows.Clear();
            });

            await _player.PlayAsync(res.Site, res.Url, res.BackupUrls, res.Referer, res.UserAgent, ct);
            await FadeOutHeroCoverAsync();
        }
        catch (OperationCanceledException)
        {
            if (!ct.IsCancellationRequested)
            {
                await StopCurrentAsync();
                await RunOnUiAsync(() =>
                {
                    SetMode(LiveMode.Select);
                    ShowPlayerError("打开直播超时/已取消，请重试。");
                    VariantGrid.IsEnabled = true;
                });
            }
        }
        catch (Exception ex)
        {
            await StopCurrentAsync();
            await RunOnUiAsync(() =>
            {
                SetMode(LiveMode.Select);
                ShowPlayerError(ex.Message);
                VariantGrid.IsEnabled = true;
            });
        }
        finally
        {
            if (_mode == LiveMode.Playing)
            {
                await RunOnUiAsync(() => VariantGrid.IsEnabled = true);
            }

            // Best-effort: ensure the poster is not stuck visible if playback failed/canceled.
            if (_mode != LiveMode.Playing)
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

    private async void OnBackToSelect(object sender, RoutedEventArgs e)
    {
        await StopCurrentAsync();
        SetMode(LiveMode.Select);
    }

    private async Task StopCurrentAsync()
    {
        try
        {
            _player?.Stop();
        }
        catch
        {
            // ignore
        }

        if (_sessionId is not null)
        {
            try { await DaemonClient.Instance.CloseLiveAsync(_sessionId); } catch { }
            _sessionId = null;
        }
    }

    private async void OnToggleDanmaku(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
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

    private void UpdateDanmakuPane()
    {
        if (_mode != LiveMode.Playing)
        {
            DanmakuCol.Width = new GridLength(0);
            SplitterCol.Width = new GridLength(0);
            DanmakuPane.Visibility = Visibility.Collapsed;
            DanmakuSplitter.Visibility = Visibility.Collapsed;
            DanmakuToggleBtn.Visibility = Visibility.Collapsed;
            return;
        }

        DanmakuToggleBtn.Visibility = Visibility.Visible;
        if (_danmakuExpanded)
        {
            DanmakuCol.Width = new GridLength(_danmakuWidthPx);
            SplitterCol.Width = new Microsoft.UI.Xaml.GridLength(SplitterWidthPx);
            DanmakuPane.Visibility = Microsoft.UI.Xaml.Visibility.Visible;
            DanmakuSplitter.Visibility = Microsoft.UI.Xaml.Visibility.Visible;
            DanmakuToggleIcon.Symbol = Symbol.Back;
        }
        else
        {
            DanmakuCol.Width = new Microsoft.UI.Xaml.GridLength(0);
            SplitterCol.Width = new Microsoft.UI.Xaml.GridLength(0);
            DanmakuPane.Visibility = Microsoft.UI.Xaml.Visibility.Collapsed;
            DanmakuSplitter.Visibility = Microsoft.UI.Xaml.Visibility.Collapsed;
            DanmakuToggleIcon.Symbol = Symbol.Forward;
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
            DanmakuToggleIcon.Symbol = expand ? Symbol.Back : Symbol.Forward;
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
        UpdateDanmakuPane();
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
        if (_sessionId is null || msg.SessionId != _sessionId)
        {
            return;
        }

        _dq.TryEnqueue(async () =>
        {
            var row = new DanmakuRowVm(msg.User, msg.Text);
            Rows.Add(row);
            if (Rows.Count > 5000)
            {
                Rows.RemoveAt(0);
            }
            if (_danmakuExpanded)
            {
                DanmakuList.ScrollIntoView(row);
            }

            if (!string.IsNullOrWhiteSpace(msg.ImageUrl))
            {
                await TryLoadEmoteAsync(row, msg.ImageUrl!);
            }
        });
    }

    private async Task TryLoadEmoteAsync(DanmakuRowVm row, string url)
    {
        if (_sessionId is null)
        {
            return;
        }

        await _imageSem.WaitAsync();
        try
        {
            var res = await DaemonClient.Instance.FetchDanmakuImageAsync(_sessionId, url);
            if (string.IsNullOrWhiteSpace(res.Base64))
            {
                return;
            }

            var bytes = Convert.FromBase64String(res.Base64);
            using var ms = new InMemoryRandomAccessStream();
            await ms.WriteAsync(bytes.AsBuffer());
            ms.Seek(0);

            var bmp = new BitmapImage();
            await bmp.SetSourceAsync(ms);
            row.Emote = bmp;
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
