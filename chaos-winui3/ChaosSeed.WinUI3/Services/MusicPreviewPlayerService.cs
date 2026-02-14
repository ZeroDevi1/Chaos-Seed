using System;
using System.Linq;
using ChaosSeed.WinUI3.Models.Music;
using Windows.Media;
using Windows.Media.Core;
using Windows.Media.Playback;
using Windows.Storage.Streams;

namespace ChaosSeed.WinUI3.Services;

public sealed class MusicPreviewPlayerService
{
    public static MusicPreviewPlayerService Instance { get; } = new();

    private readonly MediaPlayer _player;

    private MusicPreviewPlayerService()
    {
        _player = new MediaPlayer
        {
            AudioCategory = MediaPlayerAudioCategory.Media,
            IsLoopingEnabled = true,
        };

        try
        {
            var smtc = _player.SystemMediaTransportControls;
            smtc.IsEnabled = true;
            smtc.IsPlayEnabled = true;
            smtc.IsPauseEnabled = true;
            smtc.IsStopEnabled = true;
        }
        catch
        {
            // ignore
        }

        _player.MediaEnded += (_, _) =>
        {
            if (!_player.IsLoopingEnabled)
            {
                StopInternal();
                return;
            }

            try
            {
                _player.PlaybackSession.Position = TimeSpan.Zero;
                _player.Play();
            }
            catch
            {
                StopInternal();
            }
        };
        _player.MediaFailed += (_, _) => StopInternal();

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

    public MediaPlayer Player => _player;

    public bool IsOpen { get; private set; }
    public bool IsPlaying { get; private set; }

    public string? CurrentKey { get; private set; }
    public MusicTrack? Track { get; private set; }
    public string? Url { get; private set; }

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

    public bool IsLooping
    {
        get => _player.IsLoopingEnabled;
        set
        {
            var v = value;
            if (_player.IsLoopingEnabled == v)
            {
                return;
            }
            _player.IsLoopingEnabled = v;
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

    public void Play(string key, MusicTrack track, string url)
    {
        if (string.IsNullOrWhiteSpace(key)) throw new ArgumentException("empty key", nameof(key));
        if (track is null) throw new ArgumentNullException(nameof(track));
        if (string.IsNullOrWhiteSpace(url)) throw new ArgumentException("empty url", nameof(url));

        StopInternal(keepOpen: true);

        CurrentKey = key;
        Track = track;
        Url = url;
        IsOpen = true;

        _player.IsLoopingEnabled = true;

        var mediaSource = MediaSource.CreateFromUri(new Uri(url));
        var item = new MediaPlaybackItem(mediaSource);
        TryApplySmtcMetadata(item, track);
        _player.Source = item;
        _player.Play();
        IsPlaying = true;
        RaiseChanged();
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
        _player.Play();
        IsPlaying = true;
        RaiseChanged();
    }

    public void Pause()
    {
        if (!IsOpen)
        {
            return;
        }
        _player.Pause();
        IsPlaying = false;
        RaiseChanged();
    }

    public void Stop()
    {
        StopInternal();
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

    private void StopInternal(bool keepOpen = false)
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
            CurrentKey = null;
            Track = null;
            Url = null;

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
}
