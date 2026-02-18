using System.Linq;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices.WindowsRuntime;
using BiliPageModel = ChaosSeed.WinUI3.Models.Bili.BiliPage;
using ChaosSeed.WinUI3.Models.Bili;
using ChaosSeed.WinUI3.Services;
using ChaosSeed.WinUI3.Services.Downloads;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Storage.Pickers;
using Windows.Storage.Streams;
using WinRT.Interop;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class BiliPage : Page
{
    private readonly BiliDownloadManagerService _mgr = BiliDownloadManagerService.Instance;

    private CancellationTokenSource? _loginPollCts;

    public ObservableCollection<BiliDownloadSessionVm> Sessions => _mgr.ActiveSessions;
    public ObservableCollection<BiliParsedPageVm> ParsedPages { get; } = new();

    public BiliPage()
    {
        InitializeComponent();

        Loaded += (_, _) => InitFromSettings();
        Unloaded += (_, _) =>
        {
            try { _loginPollCts?.Cancel(); } catch { }
        };
    }

    private void InitFromSettings()
    {
        var s = SettingsService.Instance.Current;

        OutDirText.Text = string.IsNullOrWhiteSpace(s.BiliLastOutDir) ? "输出目录：-" : $"输出目录：{s.BiliLastOutDir}";
        SelectPageBox.Text = "ALL";
        DfnPriorityBox.Text = string.IsNullOrWhiteSpace(s.BiliDfnPriority) ? new Models.AppSettings().BiliDfnPriority : s.BiliDfnPriority;
        EncodingPriorityBox.Text = string.IsNullOrWhiteSpace(s.BiliEncodingPriority) ? "hevc,av1,avc" : s.BiliEncodingPriority;
        ConcurrencyBox.Value = Math.Clamp(s.BiliConcurrency, 1, 16);
        RetriesBox.Value = Math.Clamp(s.BiliRetries, 0, 10);
        DownloadSubtitleToggle.IsChecked = s.BiliDownloadSubtitle;
        SkipMuxToggle.IsChecked = s.BiliSkipMux;
        FilePatternBox.Text = string.IsNullOrWhiteSpace(s.BiliFilePattern) ? "<videoTitle>" : s.BiliFilePattern;
        MultiFilePatternBox.Text = string.IsNullOrWhiteSpace(s.BiliMultiFilePattern)
            ? "<videoTitle>/[P<pageNumberWithZero>]<pageTitle>"
            : s.BiliMultiFilePattern;

        FfmpegPathText.Text = string.IsNullOrWhiteSpace(s.FfmpegPath) ? "ffmpeg：-" : $"ffmpeg：{s.FfmpegPath}";

        UpdateLoginStatus();

        var notice = (_mgr.Backend.InitNotice ?? "").Trim();
        BackendHintText.Text = string.IsNullOrWhiteSpace(notice) ? "" : $"后端提示：{notice}";
    }

    private void UpdateLoginStatus()
    {
        var s = SettingsService.Instance.Current;
        var hasCookie = !string.IsNullOrWhiteSpace(s.BiliCookie);
        LoginStatusText.Text = hasCookie ? "已登录（Cookie 已保存）" : "未登录";
    }

    private void SetInfo(InfoBarSeverity sev, string title, string? msg)
    {
        InfoBar.Severity = sev;
        InfoBar.Title = title;
        InfoBar.Message = msg ?? "";
        InfoBar.IsOpen = true;
    }

    private void ClearInfo()
    {
        InfoBar.IsOpen = false;
        InfoBar.Title = "";
        InfoBar.Message = "";
    }

    private BiliAuthState? BuildAuthFromSettings()
    {
        var s = SettingsService.Instance.Current;
        var cookie = (s.BiliCookie ?? "").Trim();
        if (string.IsNullOrWhiteSpace(cookie))
        {
            return null;
        }

        return new BiliAuthState
        {
            Cookie = cookie,
            RefreshToken = string.IsNullOrWhiteSpace(s.BiliRefreshToken) ? null : s.BiliRefreshToken!.Trim(),
        };
    }

    private async Task SetQrAsync(BiliLoginQr qr)
    {
        if (string.IsNullOrWhiteSpace(qr.Base64))
        {
            QrImage.Source = null;
            return;
        }

        var bytes = Convert.FromBase64String(qr.Base64);
        using var stream = new InMemoryRandomAccessStream();
        await stream.WriteAsync(bytes.AsBuffer());
        stream.Seek(0);

        var bmp = new BitmapImage();
        await bmp.SetSourceAsync(stream);
        QrImage.Source = bmp;
    }

    private async void OnLoginQrClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        await StartLoginAsync();
    }

    private async Task StartLoginAsync()
    {
        ClearInfo();
        _loginPollCts?.Cancel();
        _loginPollCts = new CancellationTokenSource();
        var ct = _loginPollCts.Token;

        try
        {
            var qr = await _mgr.Backend.LoginQrCreateAsync(ct);
            await SetQrAsync(qr);

            QrPanel.Visibility = Visibility.Visible;
            QrHintText.Text = "已生成二维码：请扫码并确认登录。";
            UpdateLoginStatus();

            while (!ct.IsCancellationRequested)
            {
                var res = await _mgr.Backend.LoginQrPollAsync(qr.SessionId, ct);
                var state = (res.State ?? "").Trim().ToLowerInvariant();
                QrHintText.Text = state switch
                {
                    "scan" => "等待扫码…",
                    "confirm" => "已扫码，等待确认…",
                    "timeout" => "二维码已过期，请重新生成。",
                    _ => string.IsNullOrWhiteSpace(res.Message) ? $"登录状态：{state}" : res.Message,
                };

                if (string.Equals(state, "done", StringComparison.OrdinalIgnoreCase) && res.Auth is not null)
                {
                    var auth = res.Auth;
                    SettingsService.Instance.Update(s =>
                    {
                        s.BiliCookie = string.IsNullOrWhiteSpace(auth.Cookie) ? null : auth.Cookie!.Trim();
                        s.BiliRefreshToken = string.IsNullOrWhiteSpace(auth.RefreshToken) ? null : auth.RefreshToken!.Trim();
                    });
                    SetInfo(InfoBarSeverity.Success, "登录成功", null);
                    QrPanel.Visibility = Visibility.Collapsed;
                    QrImage.Source = null;
                    UpdateLoginStatus();
                    return;
                }

                if (string.Equals(state, "timeout", StringComparison.OrdinalIgnoreCase))
                {
                    SetInfo(InfoBarSeverity.Warning, "登录超时", res.Message);
                    QrPanel.Visibility = Visibility.Collapsed;
                    QrImage.Source = null;
                    UpdateLoginStatus();
                    return;
                }

                if (string.Equals(state, "other", StringComparison.OrdinalIgnoreCase))
                {
                    SetInfo(InfoBarSeverity.Warning, "登录失败", res.Message ?? "unknown");
                    QrPanel.Visibility = Visibility.Collapsed;
                    QrImage.Source = null;
                    UpdateLoginStatus();
                    return;
                }

                await Task.Delay(1000, ct);
            }
        }
        catch (OperationCanceledException)
        {
            // ignore
        }
        catch (Exception ex)
        {
            SetInfo(InfoBarSeverity.Error, "登录失败", ex.Message);
            QrPanel.Visibility = Visibility.Collapsed;
            QrImage.Source = null;
            UpdateLoginStatus();
        }
    }

    private async void OnRefreshCookieClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ClearInfo();

        try
        {
            var auth = BuildAuthFromSettings();
            if (auth is null)
            {
                SetInfo(InfoBarSeverity.Warning, "未登录", "请先扫码登录。");
                return;
            }

            var res = await _mgr.Backend.RefreshCookieAsync(new BiliRefreshCookieParams { Auth = auth }, CancellationToken.None);
            SettingsService.Instance.Update(s =>
            {
                s.BiliCookie = string.IsNullOrWhiteSpace(res.Auth.Cookie) ? null : res.Auth.Cookie!.Trim();
                s.BiliRefreshToken = string.IsNullOrWhiteSpace(res.Auth.RefreshToken) ? null : res.Auth.RefreshToken!.Trim();
            });
            SetInfo(InfoBarSeverity.Success, "Cookie 已刷新", null);
            UpdateLoginStatus();
        }
        catch (Exception ex)
        {
            SetInfo(InfoBarSeverity.Error, "刷新失败", ex.Message);
        }
    }

    private void OnClearLoginClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ClearInfo();
        SettingsService.Instance.Update(s =>
        {
            s.BiliCookie = null;
            s.BiliRefreshToken = null;
        });
        UpdateLoginStatus();
        SetInfo(InfoBarSeverity.Informational, "已清除登录信息", null);
    }

    private async void OnPickOutDirClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ClearInfo();

        try
        {
            var picked = await PickOutDirAsync(CancellationToken.None);
            if (string.IsNullOrWhiteSpace(picked))
            {
                return;
            }

            SettingsService.Instance.Update(s => s.BiliLastOutDir = picked);
            OutDirText.Text = $"输出目录：{picked}";
        }
        catch (Exception ex)
        {
            SetInfo(InfoBarSeverity.Error, "选择目录失败", ex.Message);
        }
    }

    private async Task<string?> PickOutDirAsync(CancellationToken ct)
    {
        var picker = new FolderPicker();
        picker.FileTypeFilter.Add("*");

        var win = App.MainWindowInstance;
        if (win is null)
        {
            throw new InvalidOperationException("MainWindow not ready");
        }

        InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(win));
        var folder = await picker.PickSingleFolderAsync();
        ct.ThrowIfCancellationRequested();
        return folder?.Path;
    }

    private async Task<string?> PickFfmpegPathAsync(CancellationToken ct)
    {
        var picker = new FileOpenPicker();
        picker.FileTypeFilter.Add(".exe");

        var win = App.MainWindowInstance;
        if (win is null)
        {
            throw new InvalidOperationException("MainWindow not ready");
        }

        InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(win));
        var file = await picker.PickSingleFileAsync();
        ct.ThrowIfCancellationRequested();
        return file?.Path;
    }

    private async void OnPickFfmpegClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ClearInfo();

        try
        {
            var picked = await PickFfmpegPathAsync(CancellationToken.None);
            if (string.IsNullOrWhiteSpace(picked))
            {
                return;
            }

            SettingsService.Instance.Update(s => s.FfmpegPath = picked);
            FfmpegPathText.Text = $"ffmpeg：{picked}";
        }
        catch (Exception ex)
        {
            SetInfo(InfoBarSeverity.Error, "选择失败", ex.Message);
        }
    }

    private async void OnParseClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ClearInfo();

        var input = (InputBox.Text ?? "").Trim();
        if (string.IsNullOrWhiteSpace(input))
        {
            SetInfo(InfoBarSeverity.Warning, "缺少输入", "请输入 BV/av 或视频链接。");
            return;
        }

        try
        {
            var res = await _mgr.Backend.ParseAsync(new BiliParseParams { Input = input, Auth = BuildAuthFromSettings() }, CancellationToken.None);
            var v = res.Videos.FirstOrDefault();
            if (v is null)
            {
                SetInfo(InfoBarSeverity.Warning, "解析失败", "未解析到视频。");
                return;
            }

            ParsedTitleText.Text = v.Title;

            ParsedPages.Clear();
            foreach (var p in v.Pages ?? Array.Empty<BiliPageModel>())
            {
                ParsedPages.Add(BiliParsedPageVm.From(p));
            }

            ParsedExpander.Visibility = Visibility.Visible;
            ParsedExpander.IsExpanded = true;
            SelectPageBox.Text = "ALL";
        }
        catch (Exception ex)
        {
            SetInfo(InfoBarSeverity.Error, "解析失败", ex.Message);
        }
    }

    private async void OnStartDownloadClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ClearInfo();

        var input = (InputBox.Text ?? "").Trim();
        if (string.IsNullOrWhiteSpace(input))
        {
            SetInfo(InfoBarSeverity.Warning, "缺少输入", "请输入 BV/av 或视频链接。");
            return;
        }

        try
        {
            var s = SettingsService.Instance.Current;

            var outDir = await GetOutDirForDownloadAsync(CancellationToken.None);
            if (string.IsNullOrWhiteSpace(outDir))
            {
                return;
            }

            var skipMux = SkipMuxToggle.IsChecked == true;
            var ffmpegPath = (s.FfmpegPath ?? "").Trim();
            if (!skipMux && string.IsNullOrWhiteSpace(ffmpegPath))
            {
                SetInfo(InfoBarSeverity.Warning, "缺少 ffmpeg", "未配置 ffmpeg.exe 路径，或勾选“跳过混流”。");
                return;
            }

            var selectPage = BuildSelectPage();
            var dfnPriority = (DfnPriorityBox.Text ?? "").Trim();
            var encodingPriority = (EncodingPriorityBox.Text ?? "").Trim();

            var concurrency = (uint)Math.Clamp((int)Math.Round(ConcurrencyBox.Value), 1, 16);
            var retries = (uint)Math.Clamp((int)Math.Round(RetriesBox.Value), 0, 10);

            var downloadSubtitle = DownloadSubtitleToggle.IsChecked == true;
            var filePattern = (FilePatternBox.Text ?? "").Trim();
            var multiFilePattern = (MultiFilePatternBox.Text ?? "").Trim();

            SettingsService.Instance.Update(x =>
            {
                x.BiliLastOutDir = outDir;
                x.BiliDfnPriority = dfnPriority;
                x.BiliEncodingPriority = encodingPriority;
                x.BiliConcurrency = (int)concurrency;
                x.BiliRetries = (int)retries;
                x.BiliDownloadSubtitle = downloadSubtitle;
                x.BiliSkipMux = skipMux;
                x.BiliFilePattern = filePattern;
                x.BiliMultiFilePattern = multiFilePattern;
            });

            var start = new BiliDownloadStartParams
            {
                Api = "web",
                Input = input,
                Auth = BuildAuthFromSettings(),
                Options = new BiliDownloadOptions
                {
                    OutDir = outDir,
                    SelectPage = selectPage,
                    DfnPriority = dfnPriority,
                    EncodingPriority = encodingPriority,
                    FilePattern = string.IsNullOrWhiteSpace(filePattern) ? "<videoTitle>" : filePattern,
                    MultiFilePattern = string.IsNullOrWhiteSpace(multiFilePattern)
                        ? "<videoTitle>/[P<pageNumberWithZero>]<pageTitle>"
                        : multiFilePattern,
                    DownloadSubtitle = downloadSubtitle,
                    SkipMux = skipMux,
                    Concurrency = concurrency,
                    Retries = retries,
                    FfmpegPath = skipMux ? "" : ffmpegPath,
                },
            };

            var title = string.IsNullOrWhiteSpace(ParsedTitleText.Text) ? null : ParsedTitleText.Text.Trim();
            var sid = await _mgr.StartAsync(start, title, CancellationToken.None);
            SetInfo(InfoBarSeverity.Success, "已开始", sid);
        }
        catch (Exception ex)
        {
            SetInfo(InfoBarSeverity.Error, "启动失败", ex.Message);
        }
    }

    private async Task<string?> GetOutDirForDownloadAsync(CancellationToken ct)
    {
        var s = SettingsService.Instance.Current;
        if (s.BiliAskOutDirEachTime)
        {
            var picked = await PickOutDirAsync(ct);
            if (string.IsNullOrWhiteSpace(picked))
            {
                return null;
            }

            SettingsService.Instance.Update(x => x.BiliLastOutDir = picked);
            OutDirText.Text = $"输出目录：{picked}";
            return picked;
        }

        if (!string.IsNullOrWhiteSpace(s.BiliLastOutDir))
        {
            return s.BiliLastOutDir;
        }

        var first = await PickOutDirAsync(ct);
        if (string.IsNullOrWhiteSpace(first))
        {
            throw new InvalidOperationException("未选择输出目录");
        }

        SettingsService.Instance.Update(x => x.BiliLastOutDir = first);
        OutDirText.Text = $"输出目录：{first}";
        return first;
    }

    private string BuildSelectPage()
    {
        var ui = (SelectPageBox.Text ?? "").Trim();
        if (ParsedPages.Count == 0)
        {
            return string.IsNullOrWhiteSpace(ui) ? "ALL" : ui;
        }

        var selected = ParsedPages.Where(x => x.IsSelected == true).Select(x => x.PageNumber).OrderBy(x => x).ToArray();
        if (selected.Length == 0)
        {
            return string.IsNullOrWhiteSpace(ui) ? "ALL" : ui;
        }
        if (selected.Length == ParsedPages.Count)
        {
            return "ALL";
        }

        return string.Join(",", selected);
    }

    private async void OnCancelSessionClicked(object sender, RoutedEventArgs e)
    {
        _ = e;
        ClearInfo();

        if (sender is not Button btn || btn.Tag is not BiliDownloadSessionVm vm)
        {
            return;
        }

        try
        {
            await _mgr.CancelAsync(vm.SessionId, CancellationToken.None);
            SetInfo(InfoBarSeverity.Informational, "已取消", vm.SessionId);
        }
        catch (Exception ex)
        {
            SetInfo(InfoBarSeverity.Error, "取消失败", ex.Message);
        }
    }

    private void OnRemoveSessionClicked(object sender, RoutedEventArgs e)
    {
        _ = e;
        ClearInfo();

        if (sender is not Button btn || btn.Tag is not BiliDownloadSessionVm vm)
        {
            return;
        }

        _mgr.Remove(vm.SessionId);
    }
}

public sealed class BiliParsedPageVm : INotifyPropertyChanged
{
    private bool? _isSelected = true;

    public uint PageNumber { get; set; }
    public string Cid { get; set; } = "";
    public string PageTitle { get; set; } = "";
    public uint? DurationS { get; set; }
    public string? Dimension { get; set; }

    public bool? IsSelected
    {
        get => _isSelected;
        set => SetField(ref _isSelected, value);
    }

    public string Title => $"P{PageNumber} {PageTitle}";
    public string Sub => DurationS is null ? Cid : $"{Cid} · {DurationS}s";

    public static BiliParsedPageVm From(BiliPageModel p)
        => new()
        {
            PageNumber = p.PageNumber,
            Cid = p.Cid,
            PageTitle = p.PageTitle,
            DurationS = p.DurationS,
            Dimension = p.Dimension,
            IsSelected = true,
        };

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
