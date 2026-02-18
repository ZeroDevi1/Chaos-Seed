using System;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Threading;
using System.Threading.Tasks;
using ChaosSeed.WinUI3.Services.Downloads;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class DownloadsPage : Page
{
    private readonly DispatcherQueue _dq = DispatcherQueue.GetForCurrentThread();
    private readonly MusicDownloadManagerService _mgr = MusicDownloadManagerService.Instance;

    public ObservableCollection<DownloadSessionListItemVm> Sessions { get; } = new();
    public ObservableCollection<DownloadJobListItemVm> Jobs { get; } = new();

    private DownloadSessionRow? _selected;

    public DownloadsPage()
    {
        InitializeComponent();

        Loaded += (_, _) =>
        {
            _mgr.Changed += OnManagerChanged;
            Refresh();
        };

        Unloaded += (_, _) => _mgr.Changed -= OnManagerChanged;

        ApplySelectionState();
    }

    private void OnManagerChanged(object? sender, EventArgs e)
    {
        _ = sender;
        _ = e;
        _dq.TryEnqueue(Refresh);
    }

    private void Refresh()
    {
        Sessions.Clear();
        foreach (var s in _mgr.ListSessions(limit: 200))
        {
            Sessions.Add(DownloadSessionListItemVm.From(s));
        }

        HintText.Text = $"共 {Sessions.Count} 条";

        if (_selected is not null)
        {
            var next = Sessions.Select(x => x.Row).FirstOrDefault(x => string.Equals(x.SessionId, _selected.SessionId, StringComparison.Ordinal));
            _selected = next;
        }

        ApplySelectionState();
        RefreshJobs();
    }

    private void RefreshJobs()
    {
        Jobs.Clear();
        if (_selected is null)
        {
            return;
        }

        foreach (var j in _mgr.ListJobs(_selected.SessionId, limit: 5000))
        {
            Jobs.Add(DownloadJobListItemVm.From(j));
        }
    }

    private void ApplySelectionState()
    {
        if (_selected is null)
        {
            DetailTitleText.Text = "-";
            DetailStateText.Text = "";
            DetailMetaText.Text = "";
            CancelBtn.IsEnabled = false;
            OpenDirBtn.IsEnabled = false;
            DeleteBtn.IsEnabled = false;
            return;
        }

        DetailTitleText.Text = DownloadSessionListItemVm.BuildTitle(_selected);
        DetailStateText.Text = DownloadSessionListItemVm.InferState(_selected);
        DetailMetaText.Text = $"{_selected.OutDir} · {_selected.QualityId}";

        CancelBtn.IsEnabled = !_selected.Done;
        OpenDirBtn.IsEnabled = !string.IsNullOrWhiteSpace(_selected.OutDir);
        DeleteBtn.IsEnabled = true;
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

    private void OnRefreshClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ClearInfo();
        Refresh();
    }

    private void OnSessionSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        _ = sender;
        _ = e;
        if (SessionsList.SelectedItem is not DownloadSessionListItemVm vm)
        {
            _selected = null;
            Jobs.Clear();
            ApplySelectionState();
            return;
        }

        _selected = vm.Row;
        ApplySelectionState();
        RefreshJobs();
    }

    private async void OnCancelClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ClearInfo();

        if (_selected is null)
        {
            return;
        }

        try
        {
            await _mgr.CancelAsync(_selected.SessionId, CancellationToken.None);
            SetInfo(InfoBarSeverity.Informational, "已取消", _selected.SessionId);
        }
        catch (Exception ex)
        {
            SetInfo(InfoBarSeverity.Error, "取消失败", ex.Message);
        }
    }

    private void OnDeleteClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ClearInfo();

        if (_selected is null)
        {
            return;
        }

        try
        {
            _mgr.DeleteSession(_selected.SessionId);
            _selected = null;
            Refresh();
            SetInfo(InfoBarSeverity.Informational, "已删除记录", null);
        }
        catch (Exception ex)
        {
            SetInfo(InfoBarSeverity.Error, "删除失败", ex.Message);
        }
    }

    private void OnOpenDirClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        ClearInfo();

        var dir = (_selected?.OutDir ?? "").Trim();
        if (string.IsNullOrWhiteSpace(dir))
        {
            return;
        }

        try
        {
            Process.Start(new ProcessStartInfo("explorer.exe", dir) { UseShellExecute = true });
        }
        catch (Exception ex)
        {
            SetInfo(InfoBarSeverity.Error, "打开失败", ex.Message);
        }
    }
}

public sealed class DownloadSessionListItemVm : INotifyPropertyChanged
{
    private DownloadSessionListItemVm(DownloadSessionRow row)
    {
        Row = row;
    }

    public static DownloadSessionListItemVm From(DownloadSessionRow row)
        => new(row);

    public DownloadSessionRow Row { get; }

    public string Title => BuildTitle(Row);
    public string Totals => $"Total={Row.Total} Done={Row.DoneCount} Failed={Row.Failed} Skipped={Row.Skipped} Canceled={Row.Canceled}";
    public string State => InferState(Row);
    public string StartedAtText => MusicDownloadDb.FormatUnixMs(Row.StartedAtUnixMs);

    public static string BuildTitle(DownloadSessionRow row)
    {
        var title = (row.Title ?? "").Trim();
        var artist = (row.Artist ?? "").Trim();
        if (string.IsNullOrWhiteSpace(title))
        {
            title = row.SessionId;
        }
        return string.IsNullOrWhiteSpace(artist) ? title : $"{title} - {artist}";
    }

    public static string InferState(DownloadSessionRow row)
    {
        if (row.Done)
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

        if (row.DoneCount > 0 || row.Failed > 0 || row.Skipped > 0)
        {
            return "running";
        }

        return "pending";
    }

    public event PropertyChangedEventHandler? PropertyChanged;
    private void OnPropertyChanged([CallerMemberName] string? name = null)
        => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
}

public sealed class DownloadJobListItemVm
{
    private DownloadJobListItemVm(DownloadJobRow row)
    {
        Row = row;
    }

    public static DownloadJobListItemVm From(DownloadJobRow row)
        => new(row);

    public DownloadJobRow Row { get; }

    public string IndexText => $"#{Row.JobIndex.ToString(CultureInfo.InvariantCulture)}";
    public string State => (Row.State ?? "").Trim();

    public string Title =>
        string.IsNullOrWhiteSpace(Row.TrackTitle) ? (Row.TrackId ?? "-") : Row.TrackTitle!;

    public string Sub
    {
        get
        {
            var a = (Row.TrackArtists ?? "").Trim();
            var alb = (Row.TrackAlbum ?? "").Trim();
            if (!string.IsNullOrWhiteSpace(a) && !string.IsNullOrWhiteSpace(alb))
            {
                return $"{a} · {alb}";
            }
            return string.IsNullOrWhiteSpace(a) ? alb : a;
        }
    }

    public string Error => (Row.Error ?? "").Trim();
}

