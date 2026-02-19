using System.Collections.ObjectModel;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices.WindowsRuntime;
using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Models.Music;
using ChaosSeed.WinUI3.Services;
using ChaosSeed.WinUI3.Services.Downloads;
using ChaosSeed.WinUI3.Services.MusicBackends;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Imaging;
using Newtonsoft.Json.Linq;
using Windows.Storage.Pickers;
using Windows.Storage.Streams;
using VirtualKey = global::Windows.System.VirtualKey;
using WinRT.Interop;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class MusicPage : Page
{
    private readonly IMusicBackend _backend;
    private CancellationTokenSource? _loginPollCts;
    private readonly DispatcherQueue _dq;
    private bool _restoringState;

    private int _searchPage = 1;
    private const int SearchPageSize = 20;

    public ObservableCollection<MusicTrackVm> TrackResults { get; } = new();
    public ObservableCollection<MusicAlbumVm> AlbumResults { get; } = new();
    public ObservableCollection<MusicArtistVm> ArtistResults { get; } = new();
    public ObservableCollection<MusicTrackVm> DetailTrackResults { get; } = new();
    public ObservableCollection<MusicAlbumVm> DetailAlbumResults { get; } = new();
    public ObservableCollection<DownloadSessionVm> ActiveDownloadSessions => MusicDownloadManagerService.Instance.ActiveSessions;

    private MusicAlbum? _detailAlbum;
    private MusicArtist? _detailArtist;

    public MusicPage()
    {
        InitializeComponent();

        Loaded += (_, _) =>
        {
            SettingsService.Instance.SettingsChanged += OnSettingsChanged;
            UpdateLoginStatusFromSettings();
            MusicPlayerService.Instance.Changed += OnPreviewPlayerChanged;
            UpdatePreviewFlagsFromService();
            TryRestoreSearchState();
        };
        Unloaded += (_, _) =>
        {
            SettingsService.Instance.SettingsChanged -= OnSettingsChanged;
            MusicPlayerService.Instance.Changed -= OnPreviewPlayerChanged;
            SaveSearchState();
        };

        _dq = DispatcherQueue.GetForCurrentThread();
        _backend = MusicBackendFactory.Create();

        BackendBar.IsOpen = true;
        BackendBar.Severity = InfoBarSeverity.Informational;
        BackendBar.Title = $"Backend: {_backend.Name}";
        BackendBar.Message = _backend.InitNotice;

        ServiceCombo.SelectedIndex = 1; // default to QQ
        SearchModeCombo.SelectedIndex = 0;
        DefaultQualityCombo.SelectedIndex = 0;

        LoadSettingsToUi();
        UpdateLoginPanels();
        UpdateOutDirText();
    }

    private static bool IsQqLoggedIn(QqMusicCookie? c)
    {
        return c is not null
               && !string.IsNullOrWhiteSpace(c.Musickey)
               && !string.IsNullOrWhiteSpace(c.Musicid)
               && c.LoginType is not null;
    }

    private static bool IsKugouLoggedIn(KugouUserInfo? u)
    {
        return u is not null
               && !string.IsNullOrWhiteSpace(u.Userid)
               && !string.IsNullOrWhiteSpace(u.Token);
    }

    private void LoadSettingsToUi()
    {
        var s = SettingsService.Instance.Current;
        QqLoginStatusText.Text = IsQqLoggedIn(s.QqMusicCookie)
            ? (string.IsNullOrWhiteSpace(s.QqMusicCookie?.Nick) ? "已登录" : $"已登录：{s.QqMusicCookie!.Nick}")
            : "未登录";
        KugouLoginStatusText.Text = IsKugouLoggedIn(s.KugouUserInfo)
            ? $"已登录：{s.KugouUserInfo!.Userid}"
            : "未登录";
        UpdateLoginSummaryText(s);
    }

    private void OnSettingsChanged(object? sender, EventArgs e)
    {
        _ = sender;
        _ = e;
        try
        {
            UpdateLoginStatusFromSettings();
        }
        catch
        {
            // ignore
        }
    }

    private void UpdateLoginStatusFromSettings()
    {
        var s = SettingsService.Instance.Current;
        QqLoginStatusText.Text = IsQqLoggedIn(s.QqMusicCookie)
            ? (string.IsNullOrWhiteSpace(s.QqMusicCookie?.Nick) ? "已登录" : $"已登录：{s.QqMusicCookie!.Nick}")
            : "未登录";
        KugouLoginStatusText.Text = IsKugouLoggedIn(s.KugouUserInfo)
            ? $"已登录：{s.KugouUserInfo!.Userid}"
            : "未登录";
        UpdateLoginSummaryText(s);
    }

    private void UpdateLoginSummaryText(AppSettings s)
    {
        var qq = IsQqLoggedIn(s.QqMusicCookie)
            ? (string.IsNullOrWhiteSpace(s.QqMusicCookie?.Nick) ? "QQ: 已登录" : $"QQ: {s.QqMusicCookie!.Nick}")
            : "QQ: 未登录";
        var kg = IsKugouLoggedIn(s.KugouUserInfo) ? $"酷狗: {s.KugouUserInfo!.Userid}" : "酷狗: 未登录";
        LoginSummaryText.Text = $"{qq} | {kg}";
    }

    private void UpdateOutDirText()
    {
        var s = SettingsService.Instance.Current;
        OutDirText.Text = string.IsNullOrWhiteSpace(s.MusicLastOutDir) ? "-" : s.MusicLastOutDir!;
    }

    private static string GetSelectedComboTag(ComboBox combo, string fallback)
    {
        return combo.SelectedItem is ComboBoxItem cbi && cbi.Tag is string tag && !string.IsNullOrWhiteSpace(tag)
            ? tag
            : fallback;
    }

    private void UpdateLoginPanels()
    {
        var svc = GetSelectedComboTag(ServiceCombo, "qq");
        QqLoginPanel.Visibility = (svc == "qq" || svc == "all") ? Visibility.Visible : Visibility.Collapsed;
        KugouLoginPanel.Visibility = (svc == "kugou" || svc == "all") ? Visibility.Visible : Visibility.Collapsed;
    }

    private MusicProviderConfig BuildProviderConfigFromUi()
    {
        var s = SettingsService.Instance.Current;
        var netease = (s.NeteaseBaseUrls ?? "")
            .Split(';', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Select(x => x.Trim().TrimEnd('/'))
            .Where(x => !string.IsNullOrWhiteSpace(x))
            .ToArray();

        return new MusicProviderConfig
        {
            KugouBaseUrl = string.IsNullOrWhiteSpace(s.KugouBaseUrl) ? null : s.KugouBaseUrl.Trim().TrimEnd('/'),
            NeteaseBaseUrls = netease,
            NeteaseAnonymousCookieUrl = string.IsNullOrWhiteSpace(s.NeteaseAnonymousCookieUrl) ? null : s.NeteaseAnonymousCookieUrl.Trim(),
        };
    }

    private MusicAuthState BuildAuthFromSettings()
    {
        var s = SettingsService.Instance.Current;
        return new MusicAuthState
        {
            Qq = s.QqMusicCookie,
            Kugou = s.KugouUserInfo,
            NeteaseCookie = null,
        };
    }

    private async Task EnsureConfigAppliedAsync(CancellationToken ct)
    {
        var cfg = BuildProviderConfigFromUi();
        await _backend.ConfigSetAsync(cfg, ct);
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

    private async Task<string> EnsureOutDirAsync(CancellationToken ct)
    {
        var s = SettingsService.Instance.Current;
        if (!string.IsNullOrWhiteSpace(s.MusicLastOutDir))
        {
            return s.MusicLastOutDir!;
        }

        var picked = await PickOutDirAsync(ct);
        if (string.IsNullOrWhiteSpace(picked))
        {
            throw new InvalidOperationException("未选择输出目录");
        }

        SettingsService.Instance.Update(x => x.MusicLastOutDir = picked);
        UpdateOutDirText();
        return picked!;
    }

    private async Task<string?> GetOutDirForDownloadAsync(CancellationToken ct)
    {
        var s = SettingsService.Instance.Current;
        if (s.MusicAskOutDirEachTime)
        {
            var picked = await PickOutDirAsync(ct);
            if (string.IsNullOrWhiteSpace(picked))
            {
                return null;
            }

            SettingsService.Instance.Update(x => x.MusicLastOutDir = picked);
            UpdateOutDirText();
            return picked;
        }

        return await EnsureOutDirAsync(ct);
    }

    private string GetDefaultQualityTag()
    {
        return GetSelectedComboTag(DefaultQualityCombo, "best");
    }

    private string GetRequestedQualityId()
    {
        var tag = GetDefaultQualityTag();
        // "best" means: try FLAC first, then fall back per-track.
        return string.Equals(tag, "best", StringComparison.Ordinal) ? "flac" : tag;
    }

    private static string ChooseBestQualityId(MusicQuality[] qualities, string fallback)
    {
        if (qualities.Length == 0)
        {
            return fallback;
        }

        // Prefer the user's default if available. Otherwise avoid returning a value not present in the dropdown,
        // which would make ComboBox show an empty selection.
        var fb = (fallback ?? "").Trim();
        if (!string.IsNullOrWhiteSpace(fb))
        {
            var hit = qualities.FirstOrDefault(x => string.Equals((x.Id ?? "").Trim(), fb, StringComparison.Ordinal));
            if (!string.IsNullOrWhiteSpace(hit?.Id) || (hit is not null && hit.Id is not null))
            {
                return hit!.Id ?? "";
            }
        }

        // best-effort, daemon will do final fallback per-track.
        var order = new[] { "flac", "mp3_320", "mp3_192", "mp3_128" };
        foreach (var q in order)
        {
            var hit = qualities.FirstOrDefault(x => string.Equals((x.Id ?? "").Trim(), q, StringComparison.Ordinal));
            if (!string.IsNullOrWhiteSpace(hit?.Id) || (hit is not null && hit.Id is not null))
            {
                return hit!.Id ?? "";
            }
        }

        var best = qualities
            .Where(x => !string.IsNullOrWhiteSpace(x.Id))
            .OrderByDescending(x => x.BitrateKbps ?? 0u)
            .FirstOrDefault();
        if (!string.IsNullOrWhiteSpace(best?.Id))
        {
            return best!.Id;
        }

        // Last resort: pick any existing id (even empty string) so ComboBox can still select an item.
        return qualities[0].Id ?? "";
    }

    private static string InferQualityId(MusicQuality q)
    {
        var label = (q.Label ?? "").Trim();
        var format = (q.Format ?? "").Trim().ToLowerInvariant();
        var bitrate = q.BitrateKbps ?? 0u;

        if (q.Lossless || format.Contains("flac") || label.Contains("FLAC", StringComparison.OrdinalIgnoreCase))
        {
            return "flac";
        }

        if (label.Contains("320", StringComparison.OrdinalIgnoreCase) || bitrate >= 320)
        {
            return "mp3_320";
        }

        if (label.Contains("192", StringComparison.OrdinalIgnoreCase) || bitrate >= 192)
        {
            return "mp3_192";
        }

        if (label.Contains("128", StringComparison.OrdinalIgnoreCase) || bitrate >= 128)
        {
            return "mp3_128";
        }

        if (format.Contains("mp3"))
        {
            return "mp3_128";
        }

        return "";
    }

    private static MusicQuality[] NormalizeQualitiesForUi(MusicQuality[]? qualities)
    {
        if (qualities is null || qualities.Length == 0)
        {
            return new[]
            {
                new MusicQuality
                {
                    Id = "best",
                    Label = "最高（自动）",
                    Format = "",
                    BitrateKbps = null,
                    Lossless = false,
                },
            };
        }

        foreach (var q in qualities)
        {
            if (q is null)
            {
                continue;
            }

            q.Id = (q.Id ?? "").Trim();
            q.Label = (q.Label ?? "").Trim();
            q.Format = (q.Format ?? "").Trim();

            if (string.IsNullOrWhiteSpace(q.Id))
            {
                var inferred = InferQualityId(q);
                if (!string.IsNullOrWhiteSpace(inferred))
                {
                    q.Id = inferred;
                }
            }

            if (string.IsNullOrWhiteSpace(q.Label))
            {
                q.Label = q.Id switch
                {
                    "flac" => "FLAC",
                    "mp3_320" => "MP3 320",
                    "mp3_192" => "MP3 192",
                    "mp3_128" => "MP3 128",
                    "best" => "最高（自动）",
                    _ => q.Label ?? "",
                };
            }

            if (string.IsNullOrWhiteSpace(q.Format))
            {
                q.Format = q.Id switch
                {
                    "flac" => "flac",
                    "mp3_320" => "mp3",
                    "mp3_192" => "mp3",
                    "mp3_128" => "mp3",
                    _ => q.Format ?? "",
                };
            }
        }

        // De-dup by id (prefer higher bitrate / lossless).
        var map = new Dictionary<string, MusicQuality>(StringComparer.Ordinal);
        foreach (var q in qualities)
        {
            var id = (q?.Id ?? "").Trim();
            if (string.IsNullOrWhiteSpace(id))
            {
                continue;
            }

            if (!map.TryGetValue(id, out var existing))
            {
                map[id] = q!;
                continue;
            }

            var eb = existing.BitrateKbps ?? 0u;
            var qb = q!.BitrateKbps ?? 0u;
            if (q.Lossless && !existing.Lossless)
            {
                map[id] = q;
            }
            else if (qb > eb)
            {
                map[id] = q;
            }
        }

        if (map.Count == 0)
        {
            return new[]
            {
                new MusicQuality
                {
                    Id = "best",
                    Label = "最高（自动）",
                    Format = "",
                    BitrateKbps = null,
                    Lossless = false,
                },
            };
        }

        var order = new[] { "flac", "mp3_320", "mp3_192", "mp3_128" };
        var outList = new List<MusicQuality>();
        foreach (var id in order)
        {
            if (map.TryGetValue(id, out var q))
            {
                outList.Add(q);
                map.Remove(id);
            }
        }
        outList.AddRange(map.Values.OrderByDescending(x => x.BitrateKbps ?? 0u));
        return outList.ToArray();
    }

    private static void NormalizeTrackForUi(MusicTrack t)
    {
        if (t is null)
        {
            return;
        }

        t.Service = (t.Service ?? "").Trim();
        t.Id = (t.Id ?? "").Trim();
        t.Title = (t.Title ?? "").Trim();
        t.Album = string.IsNullOrWhiteSpace(t.Album) ? null : t.Album.Trim();
        t.CoverUrl = string.IsNullOrWhiteSpace(t.CoverUrl) ? null : t.CoverUrl.Trim();

        t.Qualities = NormalizeQualitiesForUi(t.Qualities);
    }

    private string ResolveQualityIdForTrackVm(MusicTrackVm vm)
    {
        var desired = (vm.SelectedQualityId ?? "").Trim();
        if (string.Equals(desired, "best", StringComparison.Ordinal))
        {
            return GetRequestedQualityId();
        }
        if (!string.IsNullOrWhiteSpace(desired))
        {
            var hit = vm.Qualities.FirstOrDefault(x => string.Equals((x.Id ?? "").Trim(), desired, StringComparison.Ordinal));
            if (hit is not null)
            {
                return hit.Id ?? "";
            }
        }

        return ChooseBestQualityId(vm.Qualities ?? Array.Empty<MusicQuality>(), GetRequestedQualityId());
    }

    private void OnDefaultQualityChanged(object sender, SelectionChangedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (_restoringState)
        {
            return;
        }

        // Keep current per-track selection stable unless it becomes invalid (e.g. user selects FLAC but track has MP3 only).
        var fallback = GetRequestedQualityId();
        foreach (var vm in EnumerateAllTrackVms())
        {
            if (vm.Qualities.Length == 0)
            {
                continue;
            }

            var desired = (vm.SelectedQualityId ?? "").Trim();
            if (!string.IsNullOrWhiteSpace(desired)
                && vm.Qualities.Any(x => string.Equals((x.Id ?? "").Trim(), desired, StringComparison.Ordinal)))
            {
                continue;
            }
            vm.SelectedQualityId = ChooseBestQualityId(vm.Qualities, fallback);
        }
    }

    private void SetInfoBar(InfoBar bar, InfoBarSeverity severity, string title, string? message)
    {
        bar.Severity = severity;
        bar.Title = title;
        bar.Message = message;
        bar.IsOpen = true;
    }

    private void ClearInfoBar(InfoBar bar)
    {
        bar.IsOpen = false;
        bar.Title = "";
        bar.Message = null;
    }

    private void UpdatePagingUi(bool enabled, bool canPrev, bool canNext, string? hint = null)
    {
        var show = enabled || !string.IsNullOrWhiteSpace(hint);
        PagingPanel.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
        PrevPageBtn.IsEnabled = enabled && canPrev;
        NextPageBtn.IsEnabled = enabled && canNext;
        PageText.Text = $"第 {_searchPage} 页";
        PagingHintText.Text = hint ?? "";
    }

    private void ResetPaging()
    {
        _searchPage = 1;
        UpdatePagingUi(enabled: false, canPrev: false, canNext: false);
    }

    private async Task SetQrAsync(MusicLoginQr qr)
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

    private void OnServiceChanged(object sender, SelectionChangedEventArgs e)
    {
        UpdateLoginPanels();
        if (_restoringState)
        {
            return;
        }
        ResetPaging();
    }

    private void OnSearchModeChanged(object sender, SelectionChangedEventArgs e)
    {
        var mode = GetSelectedComboTag(SearchModeCombo, "track");
        ApplySearchModeVisibility(mode);
        if (_restoringState)
        {
            return;
        }
        if (e is not null)
        {
            ResetPaging();
        }
    }

    private void ApplySearchModeVisibility(string mode)
    {
        TracksList.Visibility = mode == "track" ? Visibility.Visible : Visibility.Collapsed;
        AlbumsList.Visibility = mode == "album" ? Visibility.Visible : Visibility.Collapsed;
        ArtistsList.Visibility = mode == "artist" ? Visibility.Visible : Visibility.Collapsed;
    }

    private static void SelectComboByTag(ComboBox combo, string tag, int fallbackIndex = 0)
    {
        if (combo.Items is null || combo.Items.Count == 0)
        {
            combo.SelectedIndex = fallbackIndex;
            return;
        }

        for (var i = 0; i < combo.Items.Count; i++)
        {
            if (combo.Items[i] is ComboBoxItem item
                && item.Tag is string t
                && string.Equals(t, tag, StringComparison.Ordinal))
            {
                combo.SelectedIndex = i;
                return;
            }
        }
        combo.SelectedIndex = fallbackIndex;
    }

    private void SaveSearchState()
    {
        try
        {
            var state = new MusicSearchState
            {
                ServiceTag = GetSelectedComboTag(ServiceCombo, "qq"),
                SearchModeTag = GetSelectedComboTag(SearchModeCombo, "track"),
                Keyword = (KeywordBox.Text ?? "").Trim(),
                DefaultQualityTag = GetSelectedComboTag(DefaultQualityCombo, "best"),
                SearchPage = _searchPage,
                PagingEnabled = PagingPanel.Visibility == Visibility.Visible && (GetSelectedComboTag(ServiceCombo, "qq") != "all"),
                CanPrev = PrevPageBtn.IsEnabled,
                CanNext = NextPageBtn.IsEnabled,
                PagingHint = (PagingHintText.Text ?? "").Trim(),
                Tracks = TrackResults.Select(x => x.Track).ToArray(),
                TrackQualityByKey = TrackResults.ToDictionary(x => $"{x.Track.Service}:{x.Track.Id}", x => x.SelectedQualityId, StringComparer.Ordinal),
                Albums = AlbumResults.Select(x => x.Album).ToArray(),
                Artists = ArtistResults.Select(x => x.Artist).ToArray(),
                DetailVisible = DetailBorder.Visibility == Visibility.Visible,
                DetailAlbum = _detailAlbum,
                DetailArtist = _detailArtist,
                DetailTracks = DetailTrackResults.Select(x => x.Track).ToArray(),
                DetailTrackQualityByKey = DetailTrackResults.ToDictionary(x => $"{x.Track.Service}:{x.Track.Id}", x => x.SelectedQualityId, StringComparer.Ordinal),
                DetailAlbums = DetailAlbumResults.Select(x => x.Album).ToArray(),
            };
            MusicSearchStateService.Instance.Save(state);
        }
        catch
        {
            // ignore
        }
    }

    private void TryRestoreSearchState()
    {
        var state = MusicSearchStateService.Instance.State;
        if (state is null)
        {
            return;
        }

        _restoringState = true;
        try
        {
            SelectComboByTag(ServiceCombo, state.ServiceTag, fallbackIndex: 1);
            SelectComboByTag(SearchModeCombo, state.SearchModeTag, fallbackIndex: 0);
            SelectComboByTag(DefaultQualityCombo, state.DefaultQualityTag, fallbackIndex: 0);
            KeywordBox.Text = state.Keyword ?? "";
            _searchPage = state.SearchPage <= 0 ? 1 : state.SearchPage;

            TrackResults.Clear();
            foreach (var t in state.Tracks ?? Array.Empty<MusicTrack>())
            {
                NormalizeTrackForUi(t);
                var key = $"{t.Service}:{t.Id}";
                string sel;
                if (state.TrackQualityByKey is not null
                    && state.TrackQualityByKey.TryGetValue(key, out var desired)
                    && !string.IsNullOrWhiteSpace(desired))
                {
                    desired = desired.Trim();
                    var hit = (t.Qualities ?? Array.Empty<MusicQuality>())
                        .FirstOrDefault(x => string.Equals((x.Id ?? "").Trim(), desired, StringComparison.Ordinal));
                    sel = hit?.Id ?? ChooseBestQualityId(t.Qualities ?? Array.Empty<MusicQuality>(), GetRequestedQualityId());
                }
                else
                {
                    sel = ChooseBestQualityId(t.Qualities ?? Array.Empty<MusicQuality>(), GetRequestedQualityId());
                }
                TrackResults.Add(MusicTrackVm.From(t, sel));
            }

            AlbumResults.Clear();
            foreach (var a in state.Albums ?? Array.Empty<MusicAlbum>())
            {
                AlbumResults.Add(MusicAlbumVm.From(a));
            }

            ArtistResults.Clear();
            foreach (var a in state.Artists ?? Array.Empty<MusicArtist>())
            {
                ArtistResults.Add(MusicArtistVm.From(a));
            }

            ApplySearchModeVisibility(state.SearchModeTag);
            UpdatePagingUi(
                enabled: state.PagingEnabled,
                canPrev: state.CanPrev,
                canNext: state.CanNext,
                hint: string.IsNullOrWhiteSpace(state.PagingHint) ? null : state.PagingHint
            );

            _detailAlbum = state.DetailAlbum;
            _detailArtist = state.DetailArtist;
            DetailTrackResults.Clear();
            foreach (var t in state.DetailTracks ?? Array.Empty<MusicTrack>())
            {
                NormalizeTrackForUi(t);
                var key = $"{t.Service}:{t.Id}";
                string sel;
                if (state.DetailTrackQualityByKey is not null
                    && state.DetailTrackQualityByKey.TryGetValue(key, out var desired)
                    && !string.IsNullOrWhiteSpace(desired))
                {
                    desired = desired.Trim();
                    var hit = (t.Qualities ?? Array.Empty<MusicQuality>())
                        .FirstOrDefault(x => string.Equals((x.Id ?? "").Trim(), desired, StringComparison.Ordinal));
                    sel = hit?.Id ?? ChooseBestQualityId(t.Qualities ?? Array.Empty<MusicQuality>(), GetRequestedQualityId());
                }
                else
                {
                    sel = ChooseBestQualityId(t.Qualities ?? Array.Empty<MusicQuality>(), GetRequestedQualityId());
                }
                DetailTrackResults.Add(MusicTrackVm.From(t, sel));
            }
            DetailAlbumResults.Clear();
            foreach (var a in state.DetailAlbums ?? Array.Empty<MusicAlbum>())
            {
                DetailAlbumResults.Add(MusicAlbumVm.From(a));
            }

            if (state.DetailVisible && _detailAlbum is not null)
            {
                DetailTitleText.Text = $"专辑：{_detailAlbum.Title}";
                DetailDownloadBtn.Content = "下载整张专辑";
                DetailBorder.Visibility = Visibility.Visible;
                DetailTracksList.Visibility = Visibility.Visible;
                DetailAlbumsList.Visibility = Visibility.Collapsed;
            }
            else if (state.DetailVisible && _detailArtist is not null)
            {
                DetailTitleText.Text = $"歌手：{_detailArtist.Name}";
                DetailDownloadBtn.Content = "下载该歌手全部";
                DetailBorder.Visibility = Visibility.Visible;
                DetailTracksList.Visibility = Visibility.Collapsed;
                DetailAlbumsList.Visibility = Visibility.Visible;
            }
            else
            {
                DetailBorder.Visibility = Visibility.Collapsed;
            }
        }
        finally
        {
            _restoringState = false;
        }
    }

    private async void OnPickOutDirClicked(object sender, RoutedEventArgs e)
    {
        try
        {
            var picked = await PickOutDirAsync(CancellationToken.None);
            if (string.IsNullOrWhiteSpace(picked))
            {
                return;
            }
            SettingsService.Instance.Update(s => s.MusicLastOutDir = picked);
            UpdateOutDirText();
        }
        catch (Exception ex)
        {
            SetInfoBar(SearchBar, InfoBarSeverity.Error, "选择目录失败", ex.Message);
        }
    }

    private async void OnKeywordBoxKeyDown(object sender, KeyRoutedEventArgs e)
    {
        _ = sender;
        if (e.Key != VirtualKey.Enter)
        {
            return;
        }
        e.Handled = true;
        _searchPage = 1;
        await SearchAsync(reset: true);
    }

    private async void OnPrevPageClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (_searchPage <= 1)
        {
            return;
        }
        _searchPage -= 1;
        await SearchAsync(reset: false);
    }

    private async void OnNextPageClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _searchPage += 1;
        await SearchAsync(reset: false);
    }

    private async void OnSearchClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        _searchPage = 1;
        await SearchAsync(reset: true);
    }

    private async Task SearchAsync(bool reset)
    {
        ClearInfoBar(SearchBar);
        try
        {
            await EnsureConfigAppliedAsync(CancellationToken.None);

            var svc = GetSelectedComboTag(ServiceCombo, "qq");
            var keyword = (KeywordBox.Text ?? "").Trim();
            if (string.IsNullOrWhiteSpace(keyword))
            {
                SetInfoBar(SearchBar, InfoBarSeverity.Warning, "请输入关键词", null);
                ResetPaging();
                return;
            }

            if (reset)
            {
                _searchPage = 1;
            }

            var mode = GetSelectedComboTag(SearchModeCombo, "track");
            string[] services;
            if (svc == "all")
            {
                var list = new List<string> { "qq" };
                if (!string.IsNullOrWhiteSpace(SettingsService.Instance.Current.KugouBaseUrl))
                {
                    list.Add("kugou");
                }
                list.Add("netease");
                list.Add("kuwo");
                services = list.ToArray();
            }
            else
            {
                services = new[] { svc };
            }

            var pagingEnabled = svc != "all";
            if (!pagingEnabled)
            {
                _searchPage = 1;
            }

            if (mode == "track")
            {
                TrackResults.Clear();
                AlbumResults.Clear();
                ArtistResults.Clear();

                var seen = new HashSet<string>(StringComparer.Ordinal);
                var anyPageFull = false;
                foreach (var sv in services)
                {
                    MusicTrack[]? res = null;
                    try
                    {
                        res = await _backend.SearchTracksAsync(
                            new MusicSearchParams
                            {
                                Service = sv,
                                Keyword = keyword,
                                Page = (uint)(pagingEnabled ? _searchPage : 1),
                                PageSize = (uint)SearchPageSize,
                            },
                            CancellationToken.None
                        );
                    }
                    catch (Exception ex)
                    {
                        SetInfoBar(SearchBar, InfoBarSeverity.Warning, "部分来源搜索失败", ex.Message);
                    }

                    if (pagingEnabled && (res?.Length ?? 0) >= SearchPageSize)
                    {
                        anyPageFull = true;
                    }

                    foreach (var t in res ?? Array.Empty<MusicTrack>())
                    {
                        NormalizeTrackForUi(t);
                        var key = $"{t.Service}:{t.Id}";
                        if (!seen.Add(key))
                        {
                            continue;
                        }
                        var sel = ChooseBestQualityId(t.Qualities ?? Array.Empty<MusicQuality>(), GetRequestedQualityId());
                        TrackResults.Add(MusicTrackVm.From(t, sel));
                    }
                }

                UpdatePagingUi(
                    enabled: pagingEnabled,
                    canPrev: pagingEnabled && _searchPage > 1,
                    canNext: pagingEnabled && anyPageFull,
                    hint: pagingEnabled ? null : "“全部”模式暂不支持翻页"
                );
            }
            else if (mode == "album")
            {
                TrackResults.Clear();
                AlbumResults.Clear();
                ArtistResults.Clear();

                var seen = new HashSet<string>(StringComparer.Ordinal);
                var anyPageFull = false;
                foreach (var sv in services)
                {
                    MusicAlbum[]? res = null;
                    try
                    {
                        res = await _backend.SearchAlbumsAsync(
                            new MusicSearchParams
                            {
                                Service = sv,
                                Keyword = keyword,
                                Page = (uint)(pagingEnabled ? _searchPage : 1),
                                PageSize = (uint)SearchPageSize,
                            },
                            CancellationToken.None
                        );
                    }
                    catch (Exception ex)
                    {
                        SetInfoBar(SearchBar, InfoBarSeverity.Warning, "部分来源搜索失败", ex.Message);
                    }

                    if (pagingEnabled && (res?.Length ?? 0) >= SearchPageSize)
                    {
                        anyPageFull = true;
                    }

                    foreach (var a in res ?? Array.Empty<MusicAlbum>())
                    {
                        var key = $"{a.Service}:{a.Id}";
                        if (!seen.Add(key))
                        {
                            continue;
                        }
                        AlbumResults.Add(MusicAlbumVm.From(a));
                    }
                }

                UpdatePagingUi(
                    enabled: pagingEnabled,
                    canPrev: pagingEnabled && _searchPage > 1,
                    canNext: pagingEnabled && anyPageFull,
                    hint: pagingEnabled ? null : "“全部”模式暂不支持翻页"
                );
            }
            else
            {
                TrackResults.Clear();
                AlbumResults.Clear();
                ArtistResults.Clear();

                var seen = new HashSet<string>(StringComparer.Ordinal);
                var anyPageFull = false;
                foreach (var sv in services)
                {
                    MusicArtist[]? res = null;
                    try
                    {
                        res = await _backend.SearchArtistsAsync(
                            new MusicSearchParams
                            {
                                Service = sv,
                                Keyword = keyword,
                                Page = (uint)(pagingEnabled ? _searchPage : 1),
                                PageSize = (uint)SearchPageSize,
                            },
                            CancellationToken.None
                        );
                    }
                    catch (Exception ex)
                    {
                        SetInfoBar(SearchBar, InfoBarSeverity.Warning, "部分来源搜索失败", ex.Message);
                    }

                    if (pagingEnabled && (res?.Length ?? 0) >= SearchPageSize)
                    {
                        anyPageFull = true;
                    }

                    foreach (var a in res ?? Array.Empty<MusicArtist>())
                    {
                        var key = $"{a.Service}:{a.Id}";
                        if (!seen.Add(key))
                        {
                            continue;
                        }
                        ArtistResults.Add(MusicArtistVm.From(a));
                    }
                }

                UpdatePagingUi(
                    enabled: pagingEnabled,
                    canPrev: pagingEnabled && _searchPage > 1,
                    canNext: pagingEnabled && anyPageFull,
                    hint: pagingEnabled ? null : "“全部”模式暂不支持翻页"
                );
            }

            OnSearchModeChanged(SearchModeCombo, null!);
        }
        catch (Exception ex)
        {
            SetInfoBar(SearchBar, InfoBarSeverity.Error, "搜索失败", ex.Message);
            UpdatePagingUi(enabled: false, canPrev: false, canNext: false);
        }
    }

    private async void OnQqLoginQqClicked(object sender, RoutedEventArgs e)
        => await StartQqLoginAsync("qq");

    private async void OnQqLoginWechatClicked(object sender, RoutedEventArgs e)
        => await StartQqLoginAsync("wechat");

    private async Task StartQqLoginAsync(string loginType)
    {
        ClearInfoBar(SearchBar);
        _loginPollCts?.Cancel();
        _loginPollCts = new CancellationTokenSource();
        var ct = _loginPollCts.Token;

        try
        {
            await EnsureConfigAppliedAsync(ct);
            var qr = await _backend.QqLoginQrCreateAsync(loginType, ct);
            await SetQrAsync(qr);
            LoginExpander.IsExpanded = true;
            QrPanel.Visibility = Visibility.Visible;
            QrHintText.Text = "已生成二维码：请扫码并确认登录。";
            QqLoginStatusText.Text = "登录中...";

            while (!ct.IsCancellationRequested)
            {
                var res = await _backend.QqLoginQrPollAsync(qr.SessionId, ct);
                var state = (res.State ?? "").Trim();
                QrHintText.Text = BuildLoginHint("qq", loginType, state, res.Message);

                if (string.Equals(state, "done", StringComparison.OrdinalIgnoreCase) && res.Cookie is not null)
                {
                    SettingsService.Instance.Update(s => s.QqMusicCookie = res.Cookie);
                    QqLoginStatusText.Text = res.Cookie.Nick is null ? "已登录" : $"已登录：{res.Cookie.Nick}";
                    SetInfoBar(SearchBar, InfoBarSeverity.Success, "QQ 登录成功", null);
                    QrPanel.Visibility = Visibility.Collapsed;
                    QrImage.Source = null;
                    return;
                }
                if (string.Equals(state, "refuse", StringComparison.OrdinalIgnoreCase))
                {
                    QqLoginStatusText.Text = "未登录";
                    SetInfoBar(SearchBar, InfoBarSeverity.Warning, "已取消登录", res.Message);
                    QrPanel.Visibility = Visibility.Collapsed;
                    QrImage.Source = null;
                    return;
                }
                if (string.Equals(state, "timeout", StringComparison.OrdinalIgnoreCase))
                {
                    QqLoginStatusText.Text = "未登录";
                    SetInfoBar(SearchBar, InfoBarSeverity.Warning, "登录超时", res.Message);
                    QrPanel.Visibility = Visibility.Collapsed;
                    QrImage.Source = null;
                    return;
                }
                if (string.Equals(state, "other", StringComparison.OrdinalIgnoreCase))
                {
                    QqLoginStatusText.Text = "未登录";
                    SetInfoBar(SearchBar, InfoBarSeverity.Warning, "登录失败", res.Message ?? "unknown state");
                    QrPanel.Visibility = Visibility.Collapsed;
                    QrImage.Source = null;
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
            SetInfoBar(SearchBar, InfoBarSeverity.Error, "QQ 登录失败", ex.Message);
            QrPanel.Visibility = Visibility.Collapsed;
            QrImage.Source = null;
        }
    }

    private async void OnKugouLoginQqClicked(object sender, RoutedEventArgs e)
        => await StartKugouLoginAsync("qq");

    private async void OnKugouLoginWechatClicked(object sender, RoutedEventArgs e)
        => await StartKugouLoginAsync("wechat");

    private async Task StartKugouLoginAsync(string loginType)
    {
        ClearInfoBar(SearchBar);
        _loginPollCts?.Cancel();
        _loginPollCts = new CancellationTokenSource();
        var ct = _loginPollCts.Token;

        try
        {
            await EnsureConfigAppliedAsync(ct);
            var qr = await _backend.KugouLoginQrCreateAsync(loginType, ct);
            await SetQrAsync(qr);
            LoginExpander.IsExpanded = true;
            QrPanel.Visibility = Visibility.Visible;
            QrHintText.Text = "已生成二维码：请扫码并确认登录。";
            KugouLoginStatusText.Text = "登录中...";

            while (!ct.IsCancellationRequested)
            {
                var res = await _backend.KugouLoginQrPollAsync(qr.SessionId, ct);
                var state = (res.State ?? "").Trim();
                QrHintText.Text = BuildLoginHint("kugou", loginType, state, res.Message);

                if (string.Equals(state, "done", StringComparison.OrdinalIgnoreCase) && res.KugouUser is not null)
                {
                    SettingsService.Instance.Update(s => s.KugouUserInfo = res.KugouUser);
                    KugouLoginStatusText.Text = $"已登录：{res.KugouUser.Userid}";
                    SetInfoBar(SearchBar, InfoBarSeverity.Success, "酷狗登录成功", null);
                    QrPanel.Visibility = Visibility.Collapsed;
                    QrImage.Source = null;
                    return;
                }
                if (string.Equals(state, "refuse", StringComparison.OrdinalIgnoreCase))
                {
                    KugouLoginStatusText.Text = "未登录";
                    SetInfoBar(SearchBar, InfoBarSeverity.Warning, "已取消登录", res.Message);
                    QrPanel.Visibility = Visibility.Collapsed;
                    QrImage.Source = null;
                    return;
                }
                if (string.Equals(state, "timeout", StringComparison.OrdinalIgnoreCase))
                {
                    KugouLoginStatusText.Text = "未登录";
                    SetInfoBar(SearchBar, InfoBarSeverity.Warning, "登录超时", res.Message);
                    QrPanel.Visibility = Visibility.Collapsed;
                    QrImage.Source = null;
                    return;
                }
                if (string.Equals(state, "other", StringComparison.OrdinalIgnoreCase))
                {
                    KugouLoginStatusText.Text = "未登录";
                    SetInfoBar(SearchBar, InfoBarSeverity.Warning, "登录失败", res.Message ?? "unknown state");
                    QrPanel.Visibility = Visibility.Collapsed;
                    QrImage.Source = null;
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
            SetInfoBar(SearchBar, InfoBarSeverity.Error, "酷狗登录失败", ex.Message);
            QrPanel.Visibility = Visibility.Collapsed;
            QrImage.Source = null;
        }
    }

    private static string BuildLoginHint(string provider, string loginType, string state, string? message)
    {
        if (!string.IsNullOrWhiteSpace(message))
        {
            return message!;
        }

        var lt = (loginType ?? "").Trim().ToLowerInvariant();
        var st = (state ?? "").Trim().ToLowerInvariant();
        return st switch
        {
            "scan" => lt == "wechat" ? "等待扫码..." : "已扫码，等待确认...",
            "confirm" => "已确认，正在登录...",
            "done" => "登录成功",
            "timeout" => "登录超时",
            "refuse" => "已取消",
            _ => $"登录中...({provider}/{st})",
        };
    }

    private async void OnQqRefreshCookieClicked(object sender, RoutedEventArgs e)
    {
        ClearInfoBar(SearchBar);
        try
        {
            await EnsureConfigAppliedAsync(CancellationToken.None);
            var s = SettingsService.Instance.Current;
            if (s.QqMusicCookie is null)
            {
                SetInfoBar(SearchBar, InfoBarSeverity.Warning, "未登录", "请先扫码登录 QQ 音乐");
                return;
            }
            var outCookie = await _backend.QqRefreshCookieAsync(s.QqMusicCookie, CancellationToken.None);
            SettingsService.Instance.Update(x => x.QqMusicCookie = outCookie);
            QqLoginStatusText.Text = outCookie.Nick is null ? "已登录" : $"已登录：{outCookie.Nick}";
            SetInfoBar(SearchBar, InfoBarSeverity.Success, "Cookie 已刷新", null);
        }
        catch (Exception ex)
        {
            SetInfoBar(SearchBar, InfoBarSeverity.Error, "刷新 Cookie 失败", ex.Message);
        }
    }

    private async void OnDownloadTrackClicked(object sender, RoutedEventArgs e)
    {
        if (sender is not Button btn || btn.Tag is not MusicTrackVm vm)
        {
            return;
        }
        try
        {
            await EnsureConfigAppliedAsync(CancellationToken.None);
            var outDir = await GetOutDirForDownloadAsync(CancellationToken.None);
            if (string.IsNullOrWhiteSpace(outDir))
            {
                SetInfoBar(DownloadBar, InfoBarSeverity.Informational, "已取消", null);
                return;
            }

            var s = SettingsService.Instance.Current;
            var options = new MusicDownloadOptions
            {
                QualityId = ResolveQualityIdForTrackVm(vm),
                OutDir = outDir,
                PathTemplate = string.IsNullOrWhiteSpace(s.MusicPathTemplate) ? null : s.MusicPathTemplate,
                Overwrite = s.MusicDownloadOverwrite,
                Concurrency = (uint)Math.Clamp(s.MusicDownloadConcurrency, 1, 16),
                Retries = (uint)Math.Clamp(s.MusicDownloadRetries, 0, 10),
            };

            var target = new JObject
            {
                ["type"] = "track",
                ["track"] = JObject.FromObject(vm.Track),
            };

            var start = new MusicDownloadStartParams
            {
                Config = BuildProviderConfigFromUi(),
                Auth = BuildAuthFromSettings(),
                Target = target,
                Options = options,
            };

            var trackArtist = vm.Track.Artists is null ? "" : string.Join(" / ", vm.Track.Artists.Where(x => !string.IsNullOrWhiteSpace(x)));
            var meta = new DownloadSessionMeta
            {
                TargetType = "track",
                Service = vm.Track.Service,
                Title = vm.Track.Title,
                Artist = string.IsNullOrWhiteSpace(trackArtist) ? null : trackArtist,
                Album = vm.Track.Album,
                CoverUrl = vm.Track.CoverUrl,
                PrefetchedTracks = new[] { vm.Track },
            };

            var sid = await MusicDownloadManagerService.Instance.StartAsync(start, meta, CancellationToken.None);
            SetInfoBar(DownloadBar, InfoBarSeverity.Informational, "下载已开始", sid);
        }
        catch (Exception ex)
        {
            SetInfoBar(DownloadBar, InfoBarSeverity.Error, "下载失败", ex.Message);
        }
    }

    private async void OnAddToPlaylistClicked(object sender, RoutedEventArgs e)
    {
        _ = e;
        if (sender is not Button btn || btn.Tag is not MusicTrackVm vm)
        {
            return;
        }

        try
        {
            await MusicPlayerService.Instance.EnqueueAsync(vm.Track, ResolveQualityIdForTrackVm(vm), CancellationToken.None);
            SetInfoBar(SearchBar, InfoBarSeverity.Success, "已加入播放列表", vm.Title);
        }
        catch (Exception ex)
        {
            SetInfoBar(SearchBar, InfoBarSeverity.Error, "加入失败", ex.Message);
        }
    }

    private void OnOpenDownloadsPageClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;

        try
        {
            var win = App.MainWindowInstance;
            var nav = win?.NavigationElement;
            if (nav is null)
            {
                return;
            }

            NavigationViewItem? FindItem(string tag)
            {
                foreach (var x in nav.MenuItems)
                {
                    if (x is NavigationViewItem nvi && string.Equals(nvi.Tag as string, tag, StringComparison.Ordinal))
                    {
                        return nvi;
                    }
                }
                foreach (var x in nav.FooterMenuItems)
                {
                    if (x is NavigationViewItem nvi && string.Equals(nvi.Tag as string, tag, StringComparison.Ordinal))
                    {
                        return nvi;
                    }
                }
                return null;
            }

            var item = FindItem("downloads");
            if (item is not null)
            {
                nav.SelectedItem = item;
            }
        }
        catch
        {
            // ignore
        }
    }

    private async Task StartAlbumDownloadAsync(MusicAlbum alb)
    {
        try
        {
            await EnsureConfigAppliedAsync(CancellationToken.None);

            MusicTrack[] prefetched;
            try
            {
                prefetched = await _backend.AlbumTracksAsync(
                    new MusicAlbumTracksParams { Service = alb.Service, AlbumId = alb.Id },
                    CancellationToken.None
                );
                foreach (var t in prefetched)
                {
                    NormalizeTrackForUi(t);
                    if (t.Album is null)
                    {
                        t.Album = alb.Title;
                    }
                }
            }
            catch
            {
                prefetched = Array.Empty<MusicTrack>();
            }

            var outDir = await GetOutDirForDownloadAsync(CancellationToken.None);
            if (string.IsNullOrWhiteSpace(outDir))
            {
                SetInfoBar(DownloadBar, InfoBarSeverity.Informational, "已取消", null);
                return;
            }

            var s = SettingsService.Instance.Current;

            var options = new MusicDownloadOptions
            {
                QualityId = GetRequestedQualityId(),
                OutDir = outDir,
                PathTemplate = string.IsNullOrWhiteSpace(s.MusicPathTemplate) ? null : s.MusicPathTemplate,
                Overwrite = s.MusicDownloadOverwrite,
                Concurrency = (uint)Math.Clamp(s.MusicDownloadConcurrency, 1, 16),
                Retries = (uint)Math.Clamp(s.MusicDownloadRetries, 0, 10),
            };

            var target = new JObject
            {
                ["type"] = "album",
                ["service"] = alb.Service,
                ["albumId"] = alb.Id,
            };

            var start = new MusicDownloadStartParams
            {
                Config = BuildProviderConfigFromUi(),
                Auth = BuildAuthFromSettings(),
                Target = target,
                Options = options,
            };

            var meta = new DownloadSessionMeta
            {
                TargetType = "album",
                Service = alb.Service,
                Title = alb.Title,
                Artist = alb.Artist,
                Album = alb.Title,
                CoverUrl = alb.CoverUrl,
                PrefetchedTracks = prefetched,
            };

            var sid = await MusicDownloadManagerService.Instance.StartAsync(start, meta, CancellationToken.None);
            SetInfoBar(DownloadBar, InfoBarSeverity.Informational, "专辑下载已开始", sid);
        }
        catch (Exception ex)
        {
            SetInfoBar(DownloadBar, InfoBarSeverity.Error, "下载失败", ex.Message);
        }
    }

    private async Task StartArtistAllDownloadAsync(MusicArtist a)
    {
        try
        {
            await EnsureConfigAppliedAsync(CancellationToken.None);
            var outDir = await GetOutDirForDownloadAsync(CancellationToken.None);
            if (string.IsNullOrWhiteSpace(outDir))
            {
                SetInfoBar(DownloadBar, InfoBarSeverity.Informational, "已取消", null);
                return;
            }

            var s = SettingsService.Instance.Current;

            var options = new MusicDownloadOptions
            {
                QualityId = GetRequestedQualityId(),
                OutDir = outDir,
                PathTemplate = string.IsNullOrWhiteSpace(s.MusicPathTemplate) ? null : s.MusicPathTemplate,
                Overwrite = s.MusicDownloadOverwrite,
                Concurrency = (uint)Math.Clamp(s.MusicDownloadConcurrency, 1, 16),
                Retries = (uint)Math.Clamp(s.MusicDownloadRetries, 0, 10),
            };

            var target = new JObject
            {
                ["type"] = "artist_all",
                ["service"] = a.Service,
                ["artistId"] = a.Id,
            };

            var start = new MusicDownloadStartParams
            {
                Config = BuildProviderConfigFromUi(),
                Auth = BuildAuthFromSettings(),
                Target = target,
                Options = options,
            };

            var meta = new DownloadSessionMeta
            {
                TargetType = "artist_all",
                Service = a.Service,
                Title = a.Name,
                Artist = a.Name,
                Album = null,
                CoverUrl = a.CoverUrl,
                PrefetchedTracks = null,
            };

            var sid = await MusicDownloadManagerService.Instance.StartAsync(start, meta, CancellationToken.None);
            SetInfoBar(DownloadBar, InfoBarSeverity.Informational, "歌手全量下载已开始", sid);
        }
        catch (Exception ex)
        {
            SetInfoBar(DownloadBar, InfoBarSeverity.Error, "下载失败", ex.Message);
        }
    }

    private async void OnDownloadAlbumClicked(object sender, RoutedEventArgs e)
    {
        if (sender is not Button btn || btn.Tag is not MusicAlbumVm vm)
        {
            return;
        }
        await StartAlbumDownloadAsync(vm.Album);
    }

    private async void OnDownloadArtistAllClicked(object sender, RoutedEventArgs e)
    {
        if (sender is not Button btn || btn.Tag is not MusicArtistVm vm)
        {
            return;
        }
        await StartArtistAllDownloadAsync(vm.Artist);
    }

    private async void OnOpenAlbumClicked(object sender, RoutedEventArgs e)
    {
        if (sender is not Button btn || btn.Tag is not MusicAlbumVm vm)
        {
            return;
        }
        var alb = vm.Album;

        ClearInfoBar(SearchBar);
        try
        {
            await EnsureConfigAppliedAsync(CancellationToken.None);

            var tracks = await _backend.AlbumTracksAsync(
                new MusicAlbumTracksParams { Service = alb.Service, AlbumId = alb.Id },
                CancellationToken.None
            );

            _detailAlbum = alb;
            _detailArtist = null;

            DetailTrackResults.Clear();
            foreach (var t in tracks ?? Array.Empty<MusicTrack>())
            {
                NormalizeTrackForUi(t);
                var sel = ChooseBestQualityId(t.Qualities ?? Array.Empty<MusicQuality>(), GetRequestedQualityId());
                DetailTrackResults.Add(MusicTrackVm.From(t, sel));
            }
            DetailAlbumResults.Clear();

            DetailTitleText.Text = $"专辑：{alb.Title}";
            DetailDownloadBtn.Content = "下载整张专辑";
            DetailBorder.Visibility = Visibility.Visible;
            DetailTracksList.Visibility = Visibility.Visible;
            DetailAlbumsList.Visibility = Visibility.Collapsed;
        }
        catch (Exception ex)
        {
            SetInfoBar(SearchBar, InfoBarSeverity.Error, "加载专辑曲目失败", ex.Message);
        }
    }

    private async void OnOpenArtistClicked(object sender, RoutedEventArgs e)
    {
        if (sender is not Button btn || btn.Tag is not MusicArtistVm vm)
        {
            return;
        }
        var a = vm.Artist;

        ClearInfoBar(SearchBar);
        try
        {
            await EnsureConfigAppliedAsync(CancellationToken.None);

            var albums = await _backend.ArtistAlbumsAsync(
                new MusicArtistAlbumsParams { Service = a.Service, ArtistId = a.Id },
                CancellationToken.None
            );

            _detailAlbum = null;
            _detailArtist = a;

            DetailAlbumResults.Clear();
            foreach (var alb in albums ?? Array.Empty<MusicAlbum>())
            {
                DetailAlbumResults.Add(MusicAlbumVm.From(alb));
            }
            DetailTrackResults.Clear();

            DetailTitleText.Text = $"歌手：{a.Name}";
            DetailDownloadBtn.Content = "下载该歌手全部";
            DetailBorder.Visibility = Visibility.Visible;
            DetailTracksList.Visibility = Visibility.Collapsed;
            DetailAlbumsList.Visibility = Visibility.Visible;
        }
        catch (Exception ex)
        {
            SetInfoBar(SearchBar, InfoBarSeverity.Error, "加载歌手专辑失败", ex.Message);
        }
    }

    private async void OnDetailDownloadClicked(object sender, RoutedEventArgs e)
    {
        if (_detailAlbum is not null)
        {
            await StartAlbumDownloadAsync(_detailAlbum);
            return;
        }
        if (_detailArtist is not null)
        {
            await StartArtistAllDownloadAsync(_detailArtist);
            return;
        }
        SetInfoBar(SearchBar, InfoBarSeverity.Warning, "无可下载对象", null);
    }

    private void OnCloseDetailClicked(object sender, RoutedEventArgs e)
    {
        DetailBorder.Visibility = Visibility.Collapsed;
        DetailTrackResults.Clear();
        DetailAlbumResults.Clear();
        _detailAlbum = null;
        _detailArtist = null;
    }

    private async void OnCancelDownloadClicked(object sender, RoutedEventArgs e)
    {
        if (sender is not Button btn || btn.Tag is not DownloadSessionVm vm)
        {
            return;
        }
        try
        {
            await MusicDownloadManagerService.Instance.CancelAsync(vm.SessionId, CancellationToken.None);
            SetInfoBar(DownloadBar, InfoBarSeverity.Informational, "已取消", vm.SessionId);
        }
        catch (Exception ex)
        {
            SetInfoBar(DownloadBar, InfoBarSeverity.Error, "取消失败", ex.Message);
        }
    }

    private async void OnPreviewTrackClicked(object sender, RoutedEventArgs e)
    {
        if (sender is not Button btn || btn.Tag is not MusicTrackVm vm)
        {
            return;
        }

        try
        {
            await TogglePlayAsync(vm, CancellationToken.None);
        }
        catch (Exception ex)
        {
            SetInfoBar(SearchBar, InfoBarSeverity.Error, "试听失败", ex.Message);
        }
    }

    private async Task TogglePlayAsync(MusicTrackVm vm, CancellationToken ct)
    {
        var key = $"{vm.Track.Service}:{vm.Track.Id}";
        var svc = MusicPlayerService.Instance;
        if (svc.IsOpen && string.Equals(svc.CurrentKey, key, StringComparison.Ordinal))
        {
            svc.TogglePlayPause();
            UpdatePreviewFlagsFromService();
            return;
        }

        await svc.PlayNowAsync(vm.Track, ResolveQualityIdForTrackVm(vm), ct);
        UpdatePreviewFlagsFromService();
    }

    private void OnPreviewPlayerChanged(object? sender, EventArgs e)
    {
        _ = sender;
        _ = e;
        _dq.TryEnqueue(UpdatePreviewFlagsFromService);
    }

    private void UpdatePreviewFlagsFromService()
    {
        var svc = MusicPlayerService.Instance;
        var key = svc.CurrentKey;
        foreach (var vm in EnumerateAllTrackVms())
        {
            var k = $"{vm.Track.Service}:{vm.Track.Id}";
            vm.IsPreviewPlaying =
                svc.IsOpen
                && svc.IsPlaying
                && !string.IsNullOrWhiteSpace(key)
                && string.Equals(k, key, StringComparison.Ordinal);
        }
    }

    private IEnumerable<MusicTrackVm> EnumerateAllTrackVms()
    {
        foreach (var x in TrackResults) yield return x;
        foreach (var x in DetailTrackResults) yield return x;
    }
}

public sealed class MusicTrackVm : INotifyPropertyChanged
{
    private MusicTrackVm(MusicTrack track, string selectedQualityId, BitmapImage? cover, string serviceText)
    {
        Track = track ?? throw new ArgumentNullException(nameof(track));
        _selectedQualityId = selectedQualityId;
        Cover = cover;
        ServiceText = serviceText;
    }

    public static MusicTrackVm From(MusicTrack track, string selectedQualityId)
        => new(track, selectedQualityId, MusicUiUtil.TryCreateBitmap(track.CoverUrl), MusicUiUtil.ServiceToText(track.Service));

    public MusicTrack Track { get; }

    public string Title => Track.Title;
    public BitmapImage? Cover { get; }
    public string ServiceText { get; }

    public string Subtitle
    {
        get
        {
            var artist = Track.Artists is null ? "" : string.Join(" / ", Track.Artists.Where(s => !string.IsNullOrWhiteSpace(s)));
            var album = string.IsNullOrWhiteSpace(Track.Album) ? "" : Track.Album;
            if (!string.IsNullOrWhiteSpace(artist) && !string.IsNullOrWhiteSpace(album))
            {
                return $"{artist} · {album}";
            }
            return string.IsNullOrWhiteSpace(artist) ? album : artist;
        }
    }

    public MusicQuality[] Qualities => Track.Qualities ?? Array.Empty<MusicQuality>();

    private string _selectedQualityId;
    public string SelectedQualityId
    {
        get => _selectedQualityId;
        set
        {
            if (string.Equals(_selectedQualityId, value, StringComparison.Ordinal))
            {
                return;
            }
            _selectedQualityId = value;
            OnPropertyChanged();
        }
    }

    private bool _isPreviewPlaying;
    public bool IsPreviewPlaying
    {
        get => _isPreviewPlaying;
        set
        {
            if (_isPreviewPlaying == value)
            {
                return;
            }
            _isPreviewPlaying = value;
            OnPropertyChanged();
            OnPropertyChanged(nameof(PreviewBtnText));
        }
    }

    public string PreviewBtnText => IsPreviewPlaying ? "暂停" : "试听";

    public event PropertyChangedEventHandler? PropertyChanged;

    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}

public sealed class MusicAlbumVm
{
    private MusicAlbumVm(MusicAlbum album, BitmapImage? cover, string serviceText)
    {
        Album = album ?? throw new ArgumentNullException(nameof(album));
        Cover = cover;
        ServiceText = serviceText;
    }

    public static MusicAlbumVm From(MusicAlbum album)
        => new(album, MusicUiUtil.TryCreateBitmap(album.CoverUrl), MusicUiUtil.ServiceToText(album.Service));

    public MusicAlbum Album { get; }

    public string Title => Album.Title;
    public BitmapImage? Cover { get; }
    public string ServiceText { get; }

    public string Subtitle
    {
        get
        {
            var artist = (Album.Artist ?? "").Trim();
            var svc = ServiceText;
            var cnt = Album.TrackCount is null ? "" : $" · {Album.TrackCount.Value}首";
            if (!string.IsNullOrWhiteSpace(artist))
            {
                return $"{artist} · {svc}{cnt}";
            }
            return $"{svc}{cnt}";
        }
    }
}

public sealed class MusicArtistVm
{
    private MusicArtistVm(MusicArtist artist, BitmapImage? cover, string serviceText)
    {
        Artist = artist ?? throw new ArgumentNullException(nameof(artist));
        Cover = cover;
        ServiceText = serviceText;
    }

    public static MusicArtistVm From(MusicArtist artist)
        => new(artist, MusicUiUtil.TryCreateBitmap(artist.CoverUrl), MusicUiUtil.ServiceToText(artist.Service));

    public MusicArtist Artist { get; }

    public string Name => Artist.Name;
    public BitmapImage? Cover { get; }
    public string ServiceText { get; }

    public string Subtitle
    {
        get
        {
            var svc = ServiceText;
            var cnt = Artist.AlbumCount is null ? "" : $" · {Artist.AlbumCount.Value}张专辑";
            return $"{svc}{cnt}";
        }
    }
}

internal static class MusicUiUtil
{
    public static string ServiceToText(string? service)
    {
        var s = (service ?? "").Trim().ToLowerInvariant();
        return s switch
        {
            "qq" => "[QQ]",
            "kugou" => "[酷狗]",
            "netease" => "[网易]",
            "kuwo" => "[酷我]",
            _ => string.IsNullOrWhiteSpace(s) ? "" : $"[{s}]",
        };
    }

    public static BitmapImage? TryCreateBitmap(string? url)
    {
        if (string.IsNullOrWhiteSpace(url))
        {
            return null;
        }
        try
        {
            var s = url.Trim();
            if (s.StartsWith("//"))
            {
                s = "https:" + s;
            }
            if (!Uri.TryCreate(s, UriKind.Absolute, out var u))
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
