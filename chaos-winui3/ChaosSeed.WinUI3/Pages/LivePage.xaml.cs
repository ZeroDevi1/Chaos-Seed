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
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Imaging;
using Microsoft.UI.Xaml.Media.Animation;
using StreamJsonRpc;
using Windows.Foundation;
using Windows.Storage.Streams;
using VirtualKey = global::Windows.System.VirtualKey;

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

    private readonly Microsoft.UI.Dispatching.DispatcherQueue _dq =
        Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
    private readonly SemaphoreSlim _imageSem = new(4, 4);
    private readonly FlyleafPlayerService? _player;
    private readonly string? _playerUnavailableMsg;
    private readonly ILiveBackend _backend;
    private readonly DanmakuOverlayEngine? _danmakuOverlayEngine;
    private readonly Queue<string> _playerLogTail = new();
    private const int PlayerLogTailMaxLines = 6;
    private CancellationTokenSource? _backBtnHideCts;
    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _playerControlsHideTimer;
    private bool _playerPaused;
    private bool _volumeSync;
    private bool _inAppFullscreen;
    private XamlRoot? _fullscreenXamlRoot;
    private FrameworkElement? _fullscreenRootElement;
    private bool _debugPlayerOverlay;
    private TransitionCollection? _playerPaneDefaultTransitions;
    private bool _danmakuOverlayUiInit;
    private Microsoft.UI.Dispatching.DispatcherQueueTimer? _danmakuOverlayPersistTimer;

    private Popup? FullScreenPopup => App.MainWindowInstance?.FullScreenPopupElement;
    private Grid? FullScreenPopupRoot => App.MainWindowInstance?.FullScreenPopupRootElement;
    private Grid? FullScreenBackdrop => App.MainWindowInstance?.FullScreenBackdropElement;
    private ContentControl? FullScreenPlayerHost => App.MainWindowInstance?.FullScreenPlayerHostElement;

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
    private CancellationTokenSource? _fullscreenAnimCts;
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
        try { _playerPaneDefaultTransitions = PlayerPane.Transitions; } catch { }
        InitPlayerControlsUi();
        FlyleafHost.Loaded += OnFlyleafHostLoaded;
        _backend = LiveBackendFactory.Create();
        _backend.DanmakuMessageReceived += OnDanmakuMessage;
        try
        {
            _danmakuOverlayEngine = new DanmakuOverlayEngine(_dq, DanmakuOverlayStage, _backend.FetchDanmakuImageAsync);
        }
        catch
        {
            // ignore overlay initialization failures
        }
        _player = TryInitPlayer(out _playerUnavailableMsg);
        BindFlyleafHostPlayer(_player);
        SettingsService.Instance.SettingsChanged += OnSettingsChanged;
        ApplyDebugUiFromSettings();
        ApplyDanmakuOverlayFromSettings();
        HideBackToSelectImmediately();
        Bindings.Update();
        Unloaded += (_, _) =>
        {
            try { _danmakuOverlayEngine?.SetActive(false); } catch { }
            try { _danmakuOverlayEngine?.Clear(); } catch { }
        };

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

    private void ApplyDanmakuOverlayFromSettings()
    {
        try
        {
            var s = SettingsService.Instance.Current;

            _danmakuOverlayUiInit = true;
            try
            {
                DanmakuOverlayBtn.IsChecked = s.DanmakuOverlayEnabled;

                DanmakuOverlayOpacitySlider.Value = Math.Clamp(s.DanmakuOverlayOpacity, 0.0, 1.0) * 100.0;
                DanmakuOverlayFontScaleSlider.Value = Math.Clamp(s.DanmakuOverlayFontScale, 0.5, 2.0) * 100.0;
                DanmakuOverlayDensitySlider.Value = Math.Clamp(s.DanmakuOverlayDensity, 0.0, 1.0) * 100.0;

                DanmakuOverlayAreaRadio.SelectedIndex = s.DanmakuOverlayArea switch
                {
                    DanmakuOverlayAreaMode.Quarter => 0,
                    DanmakuOverlayAreaMode.Half => 1,
                    DanmakuOverlayAreaMode.ThreeQuarter => 2,
                    _ => 3, // Full
                };

                UpdateDanmakuOverlaySettingLabels();
            }
            finally
            {
                _danmakuOverlayUiInit = false;
            }

            _danmakuOverlayEngine?.SetActive(_mode == LiveMode.Playing);
            _danmakuOverlayEngine?.ApplySettings(s);
        }
        catch
        {
            // ignore
        }
    }

    private void ApplyDanmakuOverlayPreviewFromUi()
    {
        try
        {
            var tmp = new AppSettings
            {
                DanmakuOverlayEnabled = DanmakuOverlayBtn.IsChecked == true,
                DanmakuOverlayOpacity = Math.Clamp(DanmakuOverlayOpacitySlider.Value / 100.0, 0.0, 1.0),
                DanmakuOverlayFontScale = Math.Clamp(DanmakuOverlayFontScaleSlider.Value / 100.0, 0.5, 2.0),
                DanmakuOverlayDensity = Math.Clamp(DanmakuOverlayDensitySlider.Value / 100.0, 0.0, 1.0),
                DanmakuOverlayArea = GetDanmakuOverlayAreaFromUi(),
            };

            _danmakuOverlayEngine?.SetActive(_mode == LiveMode.Playing);
            _danmakuOverlayEngine?.ApplySettings(tmp);
        }
        catch
        {
            // ignore
        }
    }

    private void EnsureDanmakuOverlayPersistTimer()
    {
        if (_danmakuOverlayPersistTimer is not null)
        {
            return;
        }

        _danmakuOverlayPersistTimer = _dq.CreateTimer();
        _danmakuOverlayPersistTimer.IsRepeating = false;
        _danmakuOverlayPersistTimer.Interval = TimeSpan.FromMilliseconds(250);
        _danmakuOverlayPersistTimer.Tick += (_, _) =>
        {
            try
            {
                PersistDanmakuOverlaySettingsFromUi();
            }
            catch
            {
                // ignore
            }
        };
    }

    private void SchedulePersistDanmakuOverlaySettings()
    {
        EnsureDanmakuOverlayPersistTimer();

        try { _danmakuOverlayPersistTimer!.Stop(); } catch { }
        try { _danmakuOverlayPersistTimer!.Start(); } catch { }
    }

    private void PersistDanmakuOverlaySettingsFromUi()
    {
        if (_danmakuOverlayUiInit)
        {
            return;
        }

        var enabled = DanmakuOverlayBtn.IsChecked == true;
        var opacity = Math.Clamp(DanmakuOverlayOpacitySlider.Value / 100.0, 0.0, 1.0);
        var fontScale = Math.Clamp(DanmakuOverlayFontScaleSlider.Value / 100.0, 0.5, 2.0);
        var density = Math.Clamp(DanmakuOverlayDensitySlider.Value / 100.0, 0.0, 1.0);
        var area = GetDanmakuOverlayAreaFromUi();

        SettingsService.Instance.Update(s =>
        {
            s.DanmakuOverlayEnabled = enabled;
            s.DanmakuOverlayOpacity = opacity;
            s.DanmakuOverlayFontScale = fontScale;
            s.DanmakuOverlayDensity = density;
            s.DanmakuOverlayArea = area;
        });
    }

    private DanmakuOverlayAreaMode GetDanmakuOverlayAreaFromUi()
    {
        try
        {
            if (DanmakuOverlayAreaRadio.SelectedItem is RadioButton rb && rb.Tag is string tag)
            {
                return tag switch
                {
                    "Quarter" => DanmakuOverlayAreaMode.Quarter,
                    "Half" => DanmakuOverlayAreaMode.Half,
                    "ThreeQuarter" => DanmakuOverlayAreaMode.ThreeQuarter,
                    _ => DanmakuOverlayAreaMode.Full,
                };
            }
        }
        catch
        {
            // ignore
        }

        return DanmakuOverlayAreaRadio.SelectedIndex switch
        {
            0 => DanmakuOverlayAreaMode.Quarter,
            1 => DanmakuOverlayAreaMode.Half,
            2 => DanmakuOverlayAreaMode.ThreeQuarter,
            _ => DanmakuOverlayAreaMode.Full,
        };
    }

    private void UpdateDanmakuOverlaySettingLabels()
    {
        try { DanmakuOverlayOpacityLabel.Text = $"{(int)Math.Round(DanmakuOverlayOpacitySlider.Value)}%"; } catch { }
        try { DanmakuOverlayFontScaleLabel.Text = $"{(int)Math.Round(DanmakuOverlayFontScaleSlider.Value)}%"; } catch { }
        try { DanmakuOverlayDensityLabel.Text = $"{(int)Math.Round(DanmakuOverlayDensitySlider.Value)}%"; } catch { }
    }

    private void OnDanmakuOverlayClicked(SplitButton sender, SplitButtonClickEventArgs args)
    {
        _ = sender;
        _ = args;

        if (_danmakuOverlayUiInit)
        {
            return;
        }

        var enabled = DanmakuOverlayBtn.IsChecked == true;
        // Click is the only reliable event on ToggleSplitButton across WinAppSDK versions.
        // The checked state is already updated by the time we get here.
        SettingsService.Instance.Update(s => s.DanmakuOverlayEnabled = enabled);
    }

    private void OnDanmakuOverlaySettingChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        _ = sender;
        _ = e;

        if (_danmakuOverlayUiInit)
        {
            return;
        }

        UpdateDanmakuOverlaySettingLabels();
        ApplyDanmakuOverlayPreviewFromUi();
        SchedulePersistDanmakuOverlaySettings();
    }

    private void OnDanmakuOverlayAreaChanged(object sender, SelectionChangedEventArgs e)
    {
        _ = sender;
        _ = e;

        if (_danmakuOverlayUiInit)
        {
            return;
        }

        ApplyDanmakuOverlayPreviewFromUi();
        SchedulePersistDanmakuOverlaySettings();
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
        try
        {
            // Restore rendering surface if we navigated away (page is cached).
            BindFlyleafHostPlayer();
        }
        catch
        {
            // ignore
        }

        // If we're already playing, keep the session/player alive and do not reset UI state.
        if (_mode == LiveMode.Playing)
        {
            try
            {
                SetMode(LiveMode.Playing);
                UpdateDanmakuPane();
                UpdateFullscreenButtonIcon();
            }
            catch
            {
                // ignore
            }
            return;
        }

        // Only reset to parse state when there's no cached manifest/results.
        if (_manifest is null && VariantCards.Count == 0 && string.IsNullOrWhiteSpace(_lastInput))
        {
            ShowParsePanelOnly();
        }
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
        // Keep playback alive across app navigation. Only "返回" inside the player stops/tears down.
        try
        {
            await RunOnUiAsync(() =>
            {
                try { ExitSystemFullscreenIfNeeded(); } catch { }
                try { ExitInAppFullscreenIfNeeded(); } catch { }
                try { ApplyContextLayerProgress(0); } catch { }
                try { UnbindFlyleafHostPlayer(); } catch { }
            });
        }
        catch
        {
            // ignore - best effort
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

    private async void OnParseInputKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key != VirtualKey.Enter)
        {
            return;
        }
        if (!ParseBtn.IsEnabled || !InputBox.IsEnabled)
        {
            return;
        }

        e.Handled = true;

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

        var wasPlaying = _mode == LiveMode.Playing;
        var autoFullscreenOnOpen = SettingsService.Instance.Current.LiveDefaultFullscreen && !wasPlaying;
        var keepFullscreen = wasPlaying && (_inAppFullscreen || (App.MainWindowInstance?.IsSystemFullscreen == true));

        var useHeroAnim = !wasPlaying && animationSourceCover?.Source is not null;
        ConnectedAnimation? heroAnimPrepared = null;
        if (useHeroAnim)
        {
            try
            {
                heroAnimPrepared = ConnectedAnimationService.GetForCurrentView().PrepareToAnimate("liveHeroCover", animationSourceCover);
                try { heroAnimPrepared.Configuration = new DirectConnectedAnimationConfiguration(); } catch { }
            }
            catch
            {
                useHeroAnim = false;
                heroAnimPrepared = null;
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
	        if (wasPlaying && _danmakuExpanded)
	        {
	            // Best-effort: persist current splitter width before reopening so a quality switch doesn't
	            // reset user layout.
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

	        if (!wasPlaying)
	        {
	            _danmakuExpanded = false;
	        }
	        _danmakuWidthPx = Math.Clamp(_danmakuWidthPx, DanmakuMinWidthPx, DanmakuMaxWidthPx);
	        if (!keepFullscreen)
	        {
	            ExitInAppFullscreenIfNeeded();
	            ExitSystemFullscreenIfNeeded();
        }

        // When we have a hero animation, avoid stacking a separate entrance transition that can make the
        // shared-element transition feel like a jump-cut.
        if (useHeroAnim)
        {
            try { PlayerPane.Transitions = null; } catch { }
        }

        SetMode(LiveMode.Playing);
        SyncPlayerOverlayFromManifest();
        RebuildQualityFlyout(variantId);
        BindFlyleafHostPlayer();
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

        if (useHeroAnim && HeroCover.Visibility == Visibility.Visible)
        {
            try { PlayerPane.UpdateLayout(); } catch { }
            try { HeroCover.UpdateLayout(); } catch { }
            ConnectedAnimation? animToStart = null;
            try { animToStart = ConnectedAnimationService.GetForCurrentView().GetAnimation("liveHeroCover"); } catch { }
            animToStart ??= heroAnimPrepared;
            if (animToStart is not null)
            {
                TryStartHeroCoverAnimation(animToStart, remainingRetries: 4);
            }
        }

	        if (useHeroAnim)
	        {
	            try { PlayerPane.Transitions = _playerPaneDefaultTransitions; } catch { }
	        }

	        try { await RunOnUiAsync(TryRefreshPlayerLayoutUnsafe); } catch { }
	        await EnsureFlyleafHostReadyAsync(ct);

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
                try { _danmakuOverlayEngine?.Clear(); } catch { }
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

            if (autoFullscreenOnOpen && playSeq == _playSeq && !ct.IsCancellationRequested)
            {
                try { await EnterFullscreenCompositeAsync(requestSystemFullscreen: true); } catch { }
            }
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

    private void TryStartHeroCoverAnimation(ConnectedAnimation heroAnim, int remainingRetries)
    {
        if (heroAnim is null || remainingRetries < 0)
        {
            return;
        }

        try
        {
            if (HeroCover.Visibility != Visibility.Visible)
            {
                return;
            }

            if (heroAnim.TryStart(HeroCover))
            {
                return;
            }
        }
        catch
        {
            // ignore
        }

        if (remainingRetries == 0)
        {
            return;
        }

        _dq.TryEnqueue(() => TryStartHeroCoverAnimation(heroAnim, remainingRetries - 1));
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

            // Ensure variant cards are clickable immediately even if the open flow is still unwinding.
            try { VariantGrid.IsEnabled = true; } catch { }

            var stopTask = StopCurrentAsync();
            _lastPlayRequest = null;
            _playingVariantId = null;
            _playingVariantLabel = null;
            SetMode(LiveMode.Select);
            try { VariantGrid.IsEnabled = true; } catch { }
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

        try
        {
            await RunOnUiAsync(() => _danmakuOverlayEngine?.Clear());
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
	            // Danmaku pane is on the right:
	            // - When expanded, show "collapse to right" icon.
	            // - When collapsed, show "expand to left" icon.
	            DanmakuToggleIcon.Glyph = _danmakuExpanded ? "\uE76C" : "\uE76B"; // ChevronRight / ChevronLeft
	        }
	        catch
	        {
	            // ignore
	        }
	    }

	    private void TryRefreshPlayerLayoutUnsafe()
	    {
	        try
	        {
	            PlayerSurface.InvalidateMeasure();
	            PlayerSurface.InvalidateArrange();
	        }
	        catch
	        {
	            // ignore
	        }

	        try
	        {
	            FlyleafHost.InvalidateMeasure();
	            FlyleafHost.InvalidateArrange();
	        }
	        catch
	        {
	            // ignore
	        }

	        try { PlayerSurface.UpdateLayout(); } catch { }
	        try { FlyleafHost.UpdateLayout(); } catch { }
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
        try
        {
            _danmakuOverlayEngine?.SetActive(mode == LiveMode.Playing);
            _danmakuOverlayEngine?.ApplySettings(SettingsService.Instance.Current);
        }
        catch
        {
            // ignore
        }
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
            DanmakuOverlayBtn.IsEnabled = state != PlayUiState.Idle;
            QualityBtn.IsEnabled = canControl && QualityFlyout.Items.Count > 0;
            FullscreenBtn.IsEnabled = state != PlayUiState.Idle;
            UpdateFullscreenButtonIcon();
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
        ApplyDanmakuOverlayFromSettings();
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

    private void OnFullscreenRootSizeChanged(object sender, SizeChangedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (!_inAppFullscreen)
        {
            return;
        }

        try { UpdateFullScreenPopupSize(); } catch { }
    }

    private void AttachFullscreenRootElement(FrameworkElement? root)
    {
        if (root is null)
        {
            return;
        }

        if (ReferenceEquals(_fullscreenRootElement, root))
        {
            return;
        }

        try
        {
            if (_fullscreenRootElement is not null)
            {
                _fullscreenRootElement.SizeChanged -= OnFullscreenRootSizeChanged;
            }
        }
        catch
        {
            // ignore
        }

        _fullscreenRootElement = root;
        try { _fullscreenRootElement.SizeChanged += OnFullscreenRootSizeChanged; } catch { }
    }

    private void DetachFullscreenRootElement()
    {
        try
        {
            if (_fullscreenRootElement is not null)
            {
                _fullscreenRootElement.SizeChanged -= OnFullscreenRootSizeChanged;
            }
        }
        catch
        {
            // ignore
        }

        _fullscreenRootElement = null;
    }

		    private void EnterInAppFullscreenIfNeeded()
		    {
		        var popup = FullScreenPopup;
		        var popupRoot = FullScreenPopupRoot;
		        var playerHost = FullScreenPlayerHost;
		        var backdrop = FullScreenBackdrop;
		        if (popup is null || popupRoot is null || playerHost is null)
		        {
		            return;
		        }

		        if (_inAppFullscreen)
		        {
		            UpdateFullScreenPopupSize();
		            return;
		        }

	        TryCloseNavPaneForFullscreen();

	        XamlRoot? root = null;
        FrameworkElement? rootElement = null;
	        try
	        {
            rootElement = App.MainWindowInstance?.Content as FrameworkElement;
	            root = rootElement?.XamlRoot ?? XamlRoot;
	        }
        catch
        {
            root = XamlRoot;
        }

        try { AttachFullscreenRootElement(rootElement); } catch { }

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
	            if (root is not null && !ReferenceEquals(popup.XamlRoot, root))
	            {
	                popup.XamlRoot = root;
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

	        try
	        {
	            if (backdrop is not null)
	            {
	                backdrop.Opacity = 1;
	            }
	            Canvas.SetLeft(playerHost, 0);
	            Canvas.SetTop(playerHost, 0);
	            playerHost.Width = popupRoot.Width;
	            playerHost.Height = popupRoot.Height;
	        }
	        catch
	        {
	            // ignore
	        }

	        try { playerHost.Content = PlayerSurface; } catch { }
	        try { popup.IsOpen = true; } catch { }
		        _inAppFullscreen = true;
		        try
		        {
		            ApplyContextLayerProgress(1);
	            UpdateFullscreenButtonIcon();
	        }
	        catch
	        {
	            // ignore
	        }
	    }

			    private void ExitInAppFullscreenIfNeeded()
			    {
			        var popup = FullScreenPopup;
			        var playerHost = FullScreenPlayerHost;

			        // Always clear any pending fullscreen animation state, even if we already think we're not fullscreen.
			        // Otherwise stale CTS can block resize/layout refresh after a system fullscreen toggle.
			        CancelFullscreenAnimation();
			        DetachFullscreenRootElement();

			        if (!_inAppFullscreen)
			        {
			            try
			            {
			                if (popup is not null)
			                {
			                    popup.IsOpen = false;
			                }
			            }
			            catch { }
			            return;
			        }

			        try
			        {
			            if (popup is not null)
			            {
			                popup.IsOpen = false;
			            }
			        }
			        catch { }

	        try
	        {
	            if (playerHost is not null && ReferenceEquals(playerHost.Content, PlayerSurface))
	            {
	                playerHost.Content = null;
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
        _fullscreenRootElement = null;
	        _inAppFullscreen = false;

	        try
	        {
	            ApplyContextLayerProgress(0);
	            UpdateFullscreenButtonIcon();
	        }
	        catch
	        {
	            // ignore
	        }
    }

		    private void UpdateFullScreenPopupSize()
		    {
		        var popup = FullScreenPopup;
		        var popupRoot = FullScreenPopupRoot;
		        var playerHost = FullScreenPlayerHost;
		        if (popup is null || popupRoot is null || playerHost is null)
		        {
		            return;
		        }

		        try
		        {
		            var targetRect = GetFullscreenTargetRect();
		            if (targetRect.Width <= 1 || targetRect.Height <= 1)
	            {
	                return;
	            }

		            try
		            {
		                popup.HorizontalOffset = 0;
		                popup.VerticalOffset = 0;
		            }
		            catch
		            {
		                // ignore
		            }

		            popupRoot.Width = targetRect.Width;
		            popupRoot.Height = targetRect.Height;

		            if (_inAppFullscreen && _fullscreenAnimCts is null)
		            {
		                Canvas.SetLeft(playerHost, 0);
		                Canvas.SetTop(playerHost, 0);
		                playerHost.Width = targetRect.Width;
		                playerHost.Height = targetRect.Height;
		                TryRefreshPlayerLayoutUnsafe();
		            }
		        }
		        catch
		        {
		            // ignore
		        }
		    }

    private void ExitSystemFullscreenIfNeeded()
    {
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

    private void CancelFullscreenAnimation(CancellationTokenSource? expected = null)
    {
        // When called without `expected`, cancels/disposes the current CTS (if any).
        // When called with `expected`, only clears the field if it still points to `expected`,
        // but always disposes `expected` so early-return paths don't leak CTS.
        CancellationTokenSource? cts = null;
        try
        {
            if (expected is null)
            {
                cts = _fullscreenAnimCts;
                _fullscreenAnimCts = null;
            }
            else if (ReferenceEquals(_fullscreenAnimCts, expected))
            {
                cts = expected;
                _fullscreenAnimCts = null;
            }
            else
            {
                cts = expected;
            }
        }
        catch
        {
            cts = expected;
        }

        try { cts?.Cancel(); } catch { }
        try { cts?.Dispose(); } catch { }
    }

    private static double EaseInOutCubic(double t)
    {
        t = Math.Clamp(t, 0, 1);
        if (t < 0.5)
        {
            return 4 * t * t * t;
        }
        return 1 - Math.Pow(-2 * t + 2, 3) / 2;
    }

    private int GetFullscreenAnimDurationMs()
    {
        var rate = SettingsService.Instance.Current.LiveFullscreenAnimRate;
        if (double.IsNaN(rate) || double.IsInfinity(rate) || rate <= 0)
        {
            rate = 1.0;
        }
        rate = Math.Clamp(rate, 0.25, 2.5);

        const double baseMs = 320;
        var ms = (int)Math.Round(baseMs / rate);
        return Math.Clamp(ms, 120, 900);
    }

    private FrameworkElement? TryGetFullscreenRootElement()
    {
        try
        {
            if (App.MainWindowInstance?.Content is FrameworkElement fe)
            {
                return fe;
            }
        }
        catch
        {
            // ignore
        }
        return null;
    }

    private static Rect GetElementBounds(FrameworkElement element, UIElement relativeTo)
    {
        var rect = new Rect(0, 0, element.ActualWidth, element.ActualHeight);
        return element.TransformToVisual(relativeTo).TransformBounds(rect);
    }

    private static Rect TryGetMainWindowBounds()
    {
        try
        {
            var win = App.MainWindowInstance;
            if (win is null)
            {
                return default;
            }

            var b = win.Bounds;
            if (b.Width > 1 && b.Height > 1)
            {
                return b;
            }
        }
        catch
        {
            // ignore
        }

        return default;
    }

	    private Rect GetFullscreenTargetRect()
	    {
	        try
	        {
	            var xr = _fullscreenXamlRoot ?? FullScreenPopupRoot?.XamlRoot ?? XamlRoot;
	            if (xr is not null && xr.Size.Width > 1 && xr.Size.Height > 1)
	            {
	                return new Rect(0, 0, xr.Size.Width, xr.Size.Height);
	            }
	        }
	        catch
	        {
	            // ignore
	        }

	        var b = TryGetMainWindowBounds();
	        if (b.Width > 1 && b.Height > 1)
	        {
	            return new Rect(0, 0, b.Width, b.Height);
	        }

	        double w;
	        double h;
	        try { w = FullScreenPopupRoot?.Width ?? 0; } catch { w = 0; }
	        try { h = FullScreenPopupRoot?.Height ?? 0; } catch { h = 0; }
	        if (double.IsNaN(w) || double.IsInfinity(w) || w <= 0)
	        {
	            try { w = FullScreenPopupRoot?.ActualWidth ?? 0; } catch { w = 0; }
	        }
	        if (double.IsNaN(h) || double.IsInfinity(h) || h <= 0)
	        {
	            try { h = FullScreenPopupRoot?.ActualHeight ?? 0; } catch { h = 0; }
	        }

	        return new Rect(0, 0, w, h);
	    }

    private static void TryCloseNavPaneForFullscreen()
    {
        try
        {
            var nav = App.MainWindowInstance?.NavigationElement;
            if (nav is null)
            {
                return;
            }
            nav.IsPaneOpen = false;
        }
        catch
        {
            // ignore
        }
    }

	    private Rect GetCurrentFullscreenHostRect()
	    {
	        var playerHost = FullScreenPlayerHost;
	        if (playerHost is null)
	        {
	            return default;
	        }

	        double x;
	        double y;
	        try { x = Canvas.GetLeft(playerHost); } catch { x = 0; }
	        try { y = Canvas.GetTop(playerHost); } catch { y = 0; }
	        if (double.IsNaN(x) || double.IsInfinity(x))
	        {
	            x = 0;
	        }
	        if (double.IsNaN(y) || double.IsInfinity(y))
	        {
	            y = 0;
	        }

	        var w = playerHost.Width;
	        var h = playerHost.Height;
	        if (double.IsNaN(w) || double.IsInfinity(w) || w <= 0)
	        {
	            w = playerHost.ActualWidth;
	        }
	        if (double.IsNaN(h) || double.IsInfinity(h) || h <= 0)
	        {
	            h = playerHost.ActualHeight;
	        }

	        return new Rect(x, y, w, h);
	    }

    private void ApplyContextLayerProgress(double progress)
    {
        progress = Math.Clamp(progress, 0, 1);
        var alpha = 1.0 - progress;
        var s = 1.0 - 0.04 * progress;

        var win = App.MainWindowInstance;
        if (win is null)
        {
            return;
        }

        try
        {
            var title = win.TitleBarElement;
            title.Opacity = alpha;
            title.CenterPoint = new Vector3((float)(title.ActualWidth / 2.0), (float)(title.ActualHeight / 2.0), 0);
            title.Scale = new Vector3((float)s, (float)s, 1);
            title.IsHitTestVisible = alpha > 0.02;
        }
        catch
        {
            // ignore
        }

        try
        {
            var nav = win.NavigationElement;
            nav.Opacity = alpha;
            nav.CenterPoint = new Vector3((float)(nav.ActualWidth / 2.0), (float)(nav.ActualHeight / 2.0), 0);
            nav.Scale = new Vector3((float)s, (float)s, 1);
            nav.IsHitTestVisible = alpha > 0.02;
        }
        catch
        {
            // ignore
        }
    }

    private void UpdateFullscreenButtonIcon()
    {
        try
        {
            var isFullscreen = _inAppFullscreen || (App.MainWindowInstance?.IsSystemFullscreen == true);
            FullscreenIcon.Symbol = isFullscreen ? Symbol.BackToWindow : Symbol.FullScreen;
            ToolTipService.SetToolTip(FullscreenBtn, isFullscreen ? "退出全屏" : "全屏");
        }
        catch
        {
            // ignore
        }
    }

    private async Task EnterFullscreenCompositeAsync(bool requestSystemFullscreen)
    {
        if (_mode != LiveMode.Playing)
        {
            return;
        }

        var popup = FullScreenPopup;
        var popupRoot = FullScreenPopupRoot;
        var backdrop = FullScreenBackdrop;
        var playerHost = FullScreenPlayerHost;
        if (popup is null || popupRoot is null || playerHost is null)
        {
            if (requestSystemFullscreen && App.MainWindowInstance?.IsSystemFullscreen != true)
            {
                try { App.MainWindowInstance?.TrySetSystemFullscreen(true); } catch { }
            }
            return;
        }

	        if (_inAppFullscreen)
	        {
	            if (requestSystemFullscreen && App.MainWindowInstance?.IsSystemFullscreen != true)
	            {
	                try { App.MainWindowInstance?.TrySetSystemFullscreen(true); } catch { }
	            }
	            TryCloseNavPaneForFullscreen();
	            ApplyContextLayerProgress(1);
	            UpdateFullscreenButtonIcon();
	            return;
	        }

	        CancelFullscreenAnimation();
	        var animCts = new CancellationTokenSource();
	        _fullscreenAnimCts = animCts;
	        var ct = animCts.Token;

	        TryCloseNavPaneForFullscreen();

	        var rootElement = TryGetFullscreenRootElement();
        if (rootElement is null)
        {
            EnterInAppFullscreenIfNeeded();
            if (requestSystemFullscreen && App.MainWindowInstance?.IsSystemFullscreen != true)
            {
                try { App.MainWindowInstance?.TrySetSystemFullscreen(true); } catch { }
                try { await Task.Delay(32, ct); } catch { }
                try { await RunOnUiAsync(() => { UpdateFullScreenPopupSize(); TryRefreshPlayerLayoutUnsafe(); }); } catch { }
            }
            CancelFullscreenAnimation(animCts);
            return;
        }

        Rect fromRect;
        try
        {
            fromRect = GetElementBounds(PlayerSurface, rootElement);
        }
        catch
        {
            fromRect = default;
        }

        if (fromRect.Width <= 1 || fromRect.Height <= 1)
        {
            EnterInAppFullscreenIfNeeded();
            if (requestSystemFullscreen && App.MainWindowInstance?.IsSystemFullscreen != true)
            {
                try { App.MainWindowInstance?.TrySetSystemFullscreen(true); } catch { }
                try { await Task.Delay(32, ct); } catch { }
                try { await RunOnUiAsync(() => { UpdateFullScreenPopupSize(); TryRefreshPlayerLayoutUnsafe(); }); } catch { }
            }
            CancelFullscreenAnimation(animCts);
            return;
        }

	        XamlRoot? root = null;
	        try { root = rootElement.XamlRoot ?? XamlRoot; } catch { root = XamlRoot; }

	        await RunOnUiAsync(() =>
	        {
            try { AttachFullscreenRootElement(rootElement); } catch { }

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
	                if (root is not null && !ReferenceEquals(popup.XamlRoot, root))
	                {
	                    popup.XamlRoot = root;
	                }
	            }
	            catch
	            {
	                // ignore
	            }

	            UpdateFullScreenPopupSize();

		            try
		            {
		                // Make fullscreen background opaque immediately to avoid showing the "hole" left behind
		                // when the player is temporarily removed from the normal layout during reparenting.
		                if (backdrop is not null)
		                {
		                    backdrop.Opacity = 1;
		                }
		                ApplyContextLayerProgress(0);

		                if (ReferenceEquals(PlayerHost.Content, PlayerSurface))
		                {
		                    PlayerHost.Content = null;
		                }
	                playerHost.Content = PlayerSurface;

	                Canvas.SetLeft(playerHost, fromRect.X);
	                Canvas.SetTop(playerHost, fromRect.Y);
		                playerHost.Width = fromRect.Width;
		                playerHost.Height = fromRect.Height;

		                popup.IsOpen = true;
		                _inAppFullscreen = true;
		                UpdateFullscreenButtonIcon();
		            }
	            catch
	            {
                // ignore
            }
        });

	        if (requestSystemFullscreen && App.MainWindowInstance?.IsSystemFullscreen != true)
	        {
	            try { App.MainWindowInstance?.TrySetSystemFullscreen(true); } catch { }
	            try { await Task.Delay(32, ct); } catch { }
	            try { await RunOnUiAsync(() => UpdateFullScreenPopupSize()); } catch { }
	        }

	        Rect toRect;
	        try
	        {
	            toRect = GetFullscreenTargetRect();
	        }
	        catch
	        {
	            toRect = new Rect(0, 0, popupRoot.Width, popupRoot.Height);
	        }

	        if (toRect.Width <= 1 || toRect.Height <= 1)
	        {
	            await RunOnUiAsync(() =>
	            {
		                try
		                {
		                    if (backdrop is not null)
		                    {
		                        backdrop.Opacity = 1;
		                    }
		                    ApplyContextLayerProgress(1);
		                    Canvas.SetLeft(playerHost, 0);
		                    Canvas.SetTop(playerHost, 0);
		                    playerHost.Width = popupRoot.Width;
		                    playerHost.Height = popupRoot.Height;
		                }
	                catch
	                {
	                    // ignore
                }
            });
            CancelFullscreenAnimation(animCts);
            return;
        }

        var durationMs = GetFullscreenAnimDurationMs();
        const int frameMs = 16;
        var frames = Math.Max(10, durationMs / frameMs);

	        try
	        {
	            for (var i = 0; i <= frames; i++)
	            {
                ct.ThrowIfCancellationRequested();
                if (_mode != LiveMode.Playing || !_inAppFullscreen)
                {
                    return;
                }

                var t = (double)i / frames;
                var e = EaseInOutCubic(t);

                var x = fromRect.X + (toRect.X - fromRect.X) * e;
                var y = fromRect.Y + (toRect.Y - fromRect.Y) * e;
                var w = fromRect.Width + (toRect.Width - fromRect.Width) * e;
                var h = fromRect.Height + (toRect.Height - fromRect.Height) * e;

	                await RunOnUiAsync(() =>
	                {
		                    try
		                    {
		                        Canvas.SetLeft(playerHost, x);
		                        Canvas.SetTop(playerHost, y);
		                        playerHost.Width = w;
		                        playerHost.Height = h;
		                        // Keep the backdrop fully opaque during the expansion so the app chrome / layout
		                        // never peeks through as "white space" around the player.
		                        if (backdrop is not null)
		                        {
		                            backdrop.Opacity = 1;
		                        }
		                        ApplyContextLayerProgress(e);
		                    }
	                    catch
	                    {
	                        // ignore
                    }
                });

                await Task.Delay(frameMs, ct);
            }
        }
        catch (OperationCanceledException)
        {
            try { await RunOnUiAsync(() => ApplyContextLayerProgress(_inAppFullscreen ? 1 : 0)); } catch { }
            return;
        }
        finally
        {
            CancelFullscreenAnimation(animCts);
        }

	        await RunOnUiAsync(() =>
	        {
	            try
	            {
	                Canvas.SetLeft(playerHost, 0);
	                Canvas.SetTop(playerHost, 0);
	                playerHost.Width = toRect.Width;
	                playerHost.Height = toRect.Height;
	                if (backdrop is not null)
	                {
	                    backdrop.Opacity = 1;
	                }
	                ApplyContextLayerProgress(1);
	                UpdateFullScreenPopupSize();
	                TryRefreshPlayerLayoutUnsafe();
	                UpdateFullscreenButtonIcon();
	            }
	            catch
	            {
	                // ignore
	            }
	        });
	    }

	    private async Task ExitFullscreenCompositeAsync()
	    {
	        if (!_inAppFullscreen)
	        {
	            ExitSystemFullscreenIfNeeded();
	            ApplyContextLayerProgress(0);
	            UpdateFullscreenButtonIcon();
	            return;
	        }

	        var popup = FullScreenPopup;
	        var popupRoot = FullScreenPopupRoot;
	        var backdrop = FullScreenBackdrop;
	        var playerHost = FullScreenPlayerHost;

	        CancelFullscreenAnimation();
	        var animCts = new CancellationTokenSource();
	        _fullscreenAnimCts = animCts;
	        var ct = animCts.Token;

	        var rootElement = TryGetFullscreenRootElement();
	        if (rootElement is null)
	        {
	            ExitSystemFullscreenIfNeeded();
	            ApplyContextLayerProgress(0);
	            ExitInAppFullscreenIfNeeded();
	            CancelFullscreenAnimation(animCts);
	            return;
	        }

        if (App.MainWindowInstance?.IsSystemFullscreen == true)
        {
            try { App.MainWindowInstance.TrySetSystemFullscreen(false); } catch { }
            try { await Task.Delay(16, ct); } catch { }
        }

        await RunOnUiAsync(() => UpdateFullScreenPopupSize());

	        Rect toRect = default;
	        await RunOnUiAsync(() =>
	        {
	            try
	            {
	                // Compute against the "normal" context layer (scale=1) to avoid mismatching the final reparent target.
	                ApplyContextLayerProgress(0);
	                toRect = GetElementBounds(PlayerHost, rootElement);
	                ApplyContextLayerProgress(1);
	            }
            catch
            {
                toRect = default;
            }
        });

        if (toRect.Width <= 1 || toRect.Height <= 1)
        {
            ExitInAppFullscreenIfNeeded();
            CancelFullscreenAnimation(animCts);
            return;
        }

	        var fromRect = GetCurrentFullscreenHostRect();
	        if (fromRect.Width <= 1 || fromRect.Height <= 1)
	        {
	            try
	            {
	                fromRect = GetFullscreenTargetRect();
	            }
	            catch
	            {
	                fromRect = new Rect(0, 0, popupRoot?.Width ?? 0, popupRoot?.Height ?? 0);
	            }
	        }

        var durationMs = GetFullscreenAnimDurationMs();
        const int frameMs = 16;
        var frames = Math.Max(10, durationMs / frameMs);

        try
        {
            for (var i = 0; i <= frames; i++)
            {
                ct.ThrowIfCancellationRequested();
                if (_mode != LiveMode.Playing || !_inAppFullscreen)
                {
                    return;
                }

                var t = (double)i / frames;
                var e = EaseInOutCubic(t);

                var x = fromRect.X + (toRect.X - fromRect.X) * e;
                var y = fromRect.Y + (toRect.Y - fromRect.Y) * e;
                var w = fromRect.Width + (toRect.Width - fromRect.Width) * e;
                var h = fromRect.Height + (toRect.Height - fromRect.Height) * e;

                var p = 1.0 - e;

	                await RunOnUiAsync(() =>
	                {
	                    try
	                    {
	                        if (playerHost is null || backdrop is null)
	                        {
	                            return;
	                        }
	                        Canvas.SetLeft(playerHost, x);
	                        Canvas.SetTop(playerHost, y);
	                        playerHost.Width = w;
	                        playerHost.Height = h;
	                        backdrop.Opacity = p;
	                        ApplyContextLayerProgress(p);
	                    }
	                    catch
	                    {
	                        // ignore
                    }
                });

                await Task.Delay(frameMs, ct);
            }
        }
        catch (OperationCanceledException)
        {
            try { await RunOnUiAsync(() => ApplyContextLayerProgress(_inAppFullscreen ? 1 : 0)); } catch { }
            return;
        }
        finally
        {
            CancelFullscreenAnimation(animCts);
        }

		        await RunOnUiAsync(() =>
		        {
		            try
		            {
		                ApplyContextLayerProgress(0);
	                if (backdrop is not null)
	                {
	                    backdrop.Opacity = 0;
	                }

		                if (playerHost is not null && ReferenceEquals(playerHost.Content, PlayerSurface))
		                {
		                    playerHost.Content = null;
		                }
		                PlayerHost.Content = PlayerSurface;

		                if (popup is not null)
		                {
		                    popup.IsOpen = false;
		                }

		                if (_fullscreenXamlRoot is not null)
		                {
		                    try { _fullscreenXamlRoot.Changed -= OnXamlRootChanged; } catch { }
		                }
	                _fullscreenXamlRoot = null;
	                _inAppFullscreen = false;

	                UpdateFullscreenButtonIcon();
	            }
            catch
            {
                // ignore
            }
        });
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

    private async void OnToggleFullscreenClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ShowPlayerControls();

        if (_mode != LiveMode.Playing)
        {
            return;
        }

        try
        {
            if (_inAppFullscreen)
            {
                await ExitFullscreenCompositeAsync();
            }
            else
            {
                await EnterFullscreenCompositeAsync(requestSystemFullscreen: true);
            }
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
                // List is visually inverted (bottom -> top). Insert newest at index 0 to appear at bottom.
                Rows.Insert(0, row);
                if (Rows.Count > 5000)
                {
                    Rows.RemoveAt(Rows.Count - 1);
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

                try
                {
                    _danmakuOverlayEngine?.Enqueue(msg);
                }
                catch
                {
                    // ignore
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
