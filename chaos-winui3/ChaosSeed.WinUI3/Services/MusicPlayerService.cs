using System;
using System.Collections.ObjectModel;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using ChaosSeed.WinUI3.Models.Music;
using ChaosSeed.WinUI3.Services.MusicBackends;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Media;
using Windows.Media.Core;
using Windows.Media.Playback;
using Windows.Storage.Streams;

namespace ChaosSeed.WinUI3.Services;

public sealed class MusicPlaylistItemVm
{
    public MusicPlaylistItemVm(MusicQueueItem item)
    {
        Item = item ?? throw new ArgumentNullException(nameof(item));
        Cover = MusicUiUtil.TryCreateBitmap(Item.Track.CoverUrl);
    }

    public MusicQueueItem Item { get; }
    public string Key => Item.Key;
    public MusicTrack Track => Item.Track;
    public string RequestedQualityId => Item.RequestedQualityId;
    public BitmapImage? Cover { get; }

    public string Title => Track.Title;

    public string Subtitle
    {
        get
        {
            var artist = Track.Artists is null ? "" : string.Join(" / ", Track.Artists.Where(s => !string.IsNullOrWhiteSpace(s)));
            var album = (Track.Album ?? "").Trim();
            return string.IsNullOrWhiteSpace(album) ? artist : $"{artist} · {album}";
        }
    }
}

public sealed class MusicPlayerService
{
    public static MusicPlayerService Instance => _instance.Value;
    private static readonly Lazy<MusicPlayerService> _instance = new(() => new MusicPlayerService());

    private readonly DispatcherQueue _dq;
    private readonly MediaPlayer _player;
    private readonly IMusicBackend _backend;
    private readonly SemaphoreSlim _playGate = new(1, 1);

    private readonly MusicQueueState _queue = new();
    public ObservableCollection<MusicPlaylistItemVm> Playlist { get; } = new();

    private MusicLoopMode _loopMode = MusicLoopMode.Single;

    private MusicPlayerService()
    {
        _dq = DispatcherQueue.GetForCurrentThread();
        _backend = MusicBackendFactory.Create();

        _player = new MediaPlayer
        {
            AudioCategory = MediaPlayerAudioCategory.Media,
            IsLoopingEnabled = false,
        };

        try
        {
            var smtc = _player.SystemMediaTransportControls;
            smtc.IsEnabled = true;
            smtc.IsPlayEnabled = true;
            smtc.IsPauseEnabled = true;
            smtc.IsStopEnabled = true;
            smtc.IsNextEnabled = true;
            smtc.IsPreviousEnabled = true;
            smtc.ButtonPressed += (sender, e) =>
            {
                _ = sender;
                try
                {
                    switch (e.Button)
                    {
                        case SystemMediaTransportControlsButton.Play:
                            Resume();
                            break;
                        case SystemMediaTransportControlsButton.Pause:
                            Pause();
                            break;
                        case SystemMediaTransportControlsButton.Stop:
                            StopKeepOpen();
                            break;
                        case SystemMediaTransportControlsButton.Next:
                            _ = NextAsync(CancellationToken.None);
                            break;
                        case SystemMediaTransportControlsButton.Previous:
                            _ = PrevAsync(CancellationToken.None);
                            break;
                    }
                }
                catch
                {
                    // ignore
                }
            };
        }
        catch
        {
            // ignore
        }

        _player.MediaEnded += (sender, e) =>
        {
            _ = sender;
            _ = e;
            try
            {
                _ = HandleMediaEndedAsync();
            }
            catch
            {
                // ignore
            }
        };
        _player.MediaFailed += (_, _) => StopInternal(keepOpen: false);

        try
        {
            _player.PlaybackSession.PlaybackStateChanged += (_, _) =>
            {
                var playing = _player.PlaybackSession.PlaybackState == MediaPlaybackState.Playing;
                if (IsPlaying == playing)
                {
                    return;
                }

                IsPlaying = playing;
                try
                {
                    var smtc = _player.SystemMediaTransportControls;
                    smtc.PlaybackStatus = playing ? MediaPlaybackStatus.Playing : MediaPlaybackStatus.Paused;
                }
                catch
                {
                    // ignore
                }

                RaiseChanged();
            };
        }
        catch
        {
            // ignore
        }
    }

    public event EventHandler? Changed;
    public event EventHandler? PlaylistChanged;

    public MediaPlayer Player => _player;

    public bool IsOpen { get; private set; }
    public bool IsPlaying { get; private set; }

    public int CurrentIndex => _queue.CurrentIndex;

    public MusicPlaylistItemVm? CurrentItem =>
        (CurrentIndex >= 0 && CurrentIndex < Playlist.Count) ? Playlist[CurrentIndex] : null;

    public MusicTrack? Track => CurrentItem?.Track;

    public string? CurrentKey => CurrentItem?.Key;

    public MusicLoopMode LoopMode
    {
        get => _loopMode;
        set
        {
            if (_loopMode == value)
            {
                return;
            }
            _loopMode = value;
            RaiseChanged();
        }
    }

    public double Volume
    {
        get => _player.Volume;
        set
        {
            var v = Math.Clamp(value, 0.0, 1.0);
            if (Math.Abs(_player.Volume - v) < 0.0001)
            {
                return;
            }
            _player.Volume = v;
            RaiseChanged();
        }
    }

    public (TimeSpan position, TimeSpan duration) GetTimeline()
    {
        try
        {
            var s = _player.PlaybackSession;
            return (s.Position, s.NaturalDuration);
        }
        catch
        {
            return (TimeSpan.Zero, TimeSpan.Zero);
        }
    }

    public async Task EnqueueAsync(MusicTrack track, string requestedQualityId, CancellationToken ct)
    {
        if (track is null) throw new ArgumentNullException(nameof(track));
        ct.ThrowIfCancellationRequested();

        var item = new MusicQueueItem(track, requestedQualityId);
        var changed = _queue.Enqueue(item, out var idx);
        if (changed)
        {
            EnqueueToObservable(idx, new MusicPlaylistItemVm(item));
            RaisePlaylistChanged();
        }
    }

    public async Task PlayNowAsync(MusicTrack track, string requestedQualityId, CancellationToken ct)
    {
        if (track is null) throw new ArgumentNullException(nameof(track));

        ct.ThrowIfCancellationRequested();

        var item = new MusicQueueItem(track, requestedQualityId);
        _queue.PlayNow(item, out var idx, out var inserted);
        if (inserted)
        {
            EnqueueToObservable(idx, new MusicPlaylistItemVm(item), insert: true);
            RaisePlaylistChanged();
        }

        await PlayAtAsync(idx, ct);
    }

    public async Task PlayAtAsync(int index, CancellationToken ct)
    {
        ct.ThrowIfCancellationRequested();

        if (!_queue.TrySetCurrentIndex(index))
        {
            return;
        }

        var vm = CurrentItem;
        if (vm is null)
        {
            return;
        }

        IsOpen = true;
        RaiseChanged();

        await _playGate.WaitAsync(ct);
        try
        {
            await EnsureConfigAppliedAsync(ct);
            var url = await ResolvePlayUrlAsync(vm.Track, vm.RequestedQualityId, ct);
            StartPlayback(url, vm.Track);
        }
        finally
        {
            _playGate.Release();
        }
    }

    public async Task NextAsync(CancellationToken ct)
    {
        if (_queue.TryGetNextIndex(LoopMode, out var idx))
        {
            await PlayAtAsync(idx, ct);
        }
    }

    public async Task PrevAsync(CancellationToken ct)
    {
        if (_queue.TryGetPrevIndex(LoopMode, out var idx))
        {
            await PlayAtAsync(idx, ct);
        }
    }

    public void TogglePlayPause()
    {
        if (!IsOpen)
        {
            return;
        }
        if (IsPlaying)
        {
            Pause();
        }
        else
        {
            Resume();
        }
    }

    public void Resume()
    {
        if (!IsOpen)
        {
            return;
        }
        try
        {
            _player.Play();
        }
        catch
        {
            // ignore
        }
        IsPlaying = true;
        RaiseChanged();
    }

    public void Pause()
    {
        if (!IsOpen)
        {
            return;
        }
        try
        {
            _player.Pause();
        }
        catch
        {
            // ignore
        }
        IsPlaying = false;
        RaiseChanged();
    }

    public void StopKeepOpen()
    {
        if (!IsOpen)
        {
            return;
        }

        try
        {
            _player.Pause();
            _player.PlaybackSession.Position = TimeSpan.Zero;
        }
        catch
        {
            // ignore
        }

        IsPlaying = false;
        RaiseChanged();
    }

    public void Stop()
    {
        StopInternal(keepOpen: false);
    }

    public void SeekToSeconds(double seconds)
    {
        if (!IsOpen)
        {
            return;
        }
        if (double.IsNaN(seconds) || double.IsInfinity(seconds))
        {
            return;
        }

        try
        {
            if (seconds < 0) seconds = 0;
            _player.PlaybackSession.Position = TimeSpan.FromSeconds(seconds);
        }
        catch
        {
            // ignore
        }
    }

    public void RemoveAt(int index)
    {
        if (_queue.RemoveAt(index, out var removedCurrent))
        {
            if (index >= 0 && index < Playlist.Count)
            {
                Playlist.RemoveAt(index);
            }

            if (removedCurrent)
            {
                StopKeepOpen();
                if (_queue.CurrentIndex >= 0)
                {
                    _ = PlayAtAsync(_queue.CurrentIndex, CancellationToken.None);
                }
                else
                {
                    StopInternal(keepOpen: false);
                }
            }

            RaisePlaylistChanged();
            RaiseChanged();
        }
    }

    public void ClearPlaylist()
    {
        _queue.Clear();
        Playlist.Clear();
        StopInternal(keepOpen: false);
        RaisePlaylistChanged();
    }

    private void EnqueueToObservable(int index, MusicPlaylistItemVm vm, bool insert = false)
    {
        try
        {
            if (insert)
            {
                if (index < 0 || index > Playlist.Count)
                {
                    Playlist.Add(vm);
                }
                else
                {
                    Playlist.Insert(index, vm);
                }
                return;
            }

            if (index >= 0 && index == Playlist.Count)
            {
                Playlist.Add(vm);
            }
            else
            {
                Playlist.Add(vm);
            }
        }
        catch
        {
            Playlist.Add(vm);
        }
    }

    private async Task HandleMediaEndedAsync()
    {
        if (!IsOpen)
        {
            return;
        }

        if (LoopMode == MusicLoopMode.Single)
        {
            try
            {
                _player.PlaybackSession.Position = TimeSpan.Zero;
                _player.Play();
                IsPlaying = true;
                RaiseChanged();
            }
            catch
            {
                StopInternal(keepOpen: true);
            }
            return;
        }

        if (LoopMode == MusicLoopMode.All)
        {
            try
            {
                await NextAsync(CancellationToken.None);
            }
            catch
            {
                StopInternal(keepOpen: true);
            }
            return;
        }

        // Off
        StopKeepOpen();
    }

    private void StartPlayback(string url, MusicTrack track)
    {
        try
        {
            StopInternal(keepOpen: true);

            var mediaSource = MediaSource.CreateFromUri(new Uri(url));
            var item = new MediaPlaybackItem(mediaSource);
            TryApplySmtcMetadata(item, track);
            _player.Source = item;
            _player.Play();
            IsPlaying = true;
            RaiseChanged();
        }
        catch
        {
            StopInternal(keepOpen: false);
        }
    }

    private void StopInternal(bool keepOpen)
    {
        try
        {
            _player.Pause();
            _player.Source = null;
        }
        catch
        {
            // ignore
        }

        IsPlaying = false;

        if (!keepOpen)
        {
            IsOpen = false;
            try
            {
                var du = _player.SystemMediaTransportControls.DisplayUpdater;
                du.ClearAll();
                du.Update();
            }
            catch
            {
                // ignore
            }
        }

        RaiseChanged();
    }

    private async Task EnsureConfigAppliedAsync(CancellationToken ct)
    {
        var s = SettingsService.Instance.Current;
        var netease = (s.NeteaseBaseUrls ?? "")
            .Split(';', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Select(x => x.Trim().TrimEnd('/'))
            .Where(x => !string.IsNullOrWhiteSpace(x))
            .ToArray();

        var cfg = new MusicProviderConfig
        {
            KugouBaseUrl = string.IsNullOrWhiteSpace(s.KugouBaseUrl) ? null : s.KugouBaseUrl.Trim().TrimEnd('/'),
            NeteaseBaseUrls = netease,
            NeteaseAnonymousCookieUrl = string.IsNullOrWhiteSpace(s.NeteaseAnonymousCookieUrl) ? null : s.NeteaseAnonymousCookieUrl.Trim(),
        };

        await _backend.ConfigSetAsync(cfg, ct);
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

    private async Task<string> ResolvePlayUrlAsync(MusicTrack track, string requestedQualityId, CancellationToken ct)
    {
        var desired = (requestedQualityId ?? "").Trim();
        var candidates = new[]
        {
            desired,
            "mp3_320",
            "mp3_192",
            "mp3_128",
            "flac",
        }
        .Select(x => (x ?? "").Trim())
        .Where(x => !string.IsNullOrWhiteSpace(x))
        .Distinct(StringComparer.Ordinal)
        .ToArray();

        Exception? last = null;
        foreach (var q in candidates)
        {
            try
            {
                var res = await _backend.TrackPlayUrlAsync(
                    new MusicTrackPlayUrlParams
                    {
                        Service = (track.Service ?? "").Trim(),
                        TrackId = (track.Id ?? "").Trim(),
                        QualityId = q,
                        Auth = BuildAuthFromSettings(),
                    },
                    ct
                );
                if (!string.IsNullOrWhiteSpace(res.Url))
                {
                    return res.Url;
                }
            }
            catch (Exception ex)
            {
                last = ex;
            }
        }

        throw last ?? new InvalidOperationException("empty play url");
    }

    private void TryApplySmtcMetadata(MediaPlaybackItem item, MusicTrack track)
    {
        try
        {
            var props = item.GetDisplayProperties();
            props.Type = MediaPlaybackType.Music;
            props.MusicProperties.Title = (track.Title ?? "").Trim();
            props.MusicProperties.Artist = string.Join(" / ", (track.Artists ?? Array.Empty<string>()).Where(s => !string.IsNullOrWhiteSpace(s)).Select(s => s.Trim()));
            props.MusicProperties.AlbumTitle = (track.Album ?? "").Trim();

            var cover = (track.CoverUrl ?? "").Trim();
            if (Uri.TryCreate(cover, UriKind.Absolute, out var coverUri))
            {
                props.Thumbnail = RandomAccessStreamReference.CreateFromUri(coverUri);
            }

            item.ApplyDisplayProperties(props);
        }
        catch
        {
            // ignore
        }

        try
        {
            var smtc = _player.SystemMediaTransportControls;
            smtc.IsEnabled = true;
            smtc.IsPlayEnabled = true;
            smtc.IsPauseEnabled = true;
            smtc.IsStopEnabled = true;
            smtc.IsNextEnabled = true;
            smtc.IsPreviousEnabled = true;

            var du = smtc.DisplayUpdater;
            du.Type = MediaPlaybackType.Music;
            du.MusicProperties.Title = (track.Title ?? "").Trim();
            du.MusicProperties.Artist = string.Join(" / ", (track.Artists ?? Array.Empty<string>()).Where(s => !string.IsNullOrWhiteSpace(s)).Select(s => s.Trim()));
            du.MusicProperties.AlbumTitle = (track.Album ?? "").Trim();

            var cover = (track.CoverUrl ?? "").Trim();
            if (Uri.TryCreate(cover, UriKind.Absolute, out var coverUri))
            {
                du.Thumbnail = RandomAccessStreamReference.CreateFromUri(coverUri);
            }

            du.Update();
        }
        catch
        {
            // ignore
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

    private void RaisePlaylistChanged()
    {
        try
        {
            PlaylistChanged?.Invoke(this, EventArgs.Empty);
        }
        catch
        {
            // ignore
        }
    }
}
