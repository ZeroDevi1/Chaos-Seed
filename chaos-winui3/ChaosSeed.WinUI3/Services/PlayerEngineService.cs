using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.WindowsRuntime;
using ChaosSeed.WinUI3.Models;
using LibVLCSharp.Shared;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Media.Core;
using Windows.Media.Playback;

namespace ChaosSeed.WinUI3.Services;

public sealed class PlayerEngineService : IDisposable
{
    private readonly DispatcherQueue _dq;
    private readonly MediaPlayerElement _systemPlayerElement;
    private readonly Image _vlcImage;
    private readonly ISystemMediaSourceFactory _systemMediaSourceFactory;

    private Windows.Media.Playback.MediaPlayer? _systemPlayer;

    private LibVLC? _vlc;
    private LibVLCSharp.Shared.MediaPlayer? _vlcPlayer;
    private Media? _vlcMedia;

    private readonly object _videoGate = new();
    private IntPtr _videoBuffer = IntPtr.Zero;
    private int _videoBufferSize;
    private uint _videoWidth;
    private uint _videoHeight;
    private uint _videoPitch;
    private byte[]? _managedCopy;
    private WriteableBitmap? _bitmap;

    private volatile bool _frameQueued;
    private long _lastFrameTicks;

    public event EventHandler<string>? Error;
    public event EventHandler<string>? Info;

    public PlayerEngineService(
        DispatcherQueue dispatcherQueue,
        MediaPlayerElement systemPlayerElement,
        Image vlcImage,
        ISystemMediaSourceFactory? systemMediaSourceFactory = null
    )
    {
        _dq = dispatcherQueue;
        _systemPlayerElement = systemPlayerElement;
        _vlcImage = vlcImage;
        _systemMediaSourceFactory = systemMediaSourceFactory ?? new DefaultSystemMediaSourceFactory();
    }

    public void Play(PlayerEngine engine, string url, string? referer, string? userAgent)
    {
        Stop();

        if (engine == PlayerEngine.System)
        {
            PlaySystem(url);
            return;
        }

        PlayVlc(url, referer, userAgent);
    }

    public void Stop()
    {
        try
        {
            _systemPlayer?.Pause();
        }
        catch { }

        try
        {
            _systemPlayerElement.SetMediaPlayer(null);
        }
        catch { }

        try
        {
            _systemPlayer?.Dispose();
        }
        catch { }
        finally
        {
            _systemPlayer = null;
        }

        try
        {
            _vlcPlayer?.Stop();
        }
        catch { }

        try
        {
            _vlcMedia?.Dispose();
        }
        catch { }
        finally
        {
            _vlcMedia = null;
        }

        try
        {
            _vlcPlayer?.Dispose();
        }
        catch { }
        finally
        {
            _vlcPlayer = null;
        }

        lock (_videoGate)
        {
            if (_videoBuffer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(_videoBuffer);
                _videoBuffer = IntPtr.Zero;
            }
            _videoBufferSize = 0;
            _videoWidth = 0;
            _videoHeight = 0;
            _videoPitch = 0;
            _managedCopy = null;
            _bitmap = null;
        }

        _dq.TryEnqueue(() =>
        {
            _systemPlayerElement.Visibility = Visibility.Collapsed;
            _vlcImage.Visibility = Visibility.Collapsed;
            _vlcImage.Source = null;
        });
    }

    private void PlaySystem(string url)
    {
        _dq.TryEnqueue(() =>
        {
            _vlcImage.Visibility = Visibility.Collapsed;
            _systemPlayerElement.Visibility = Visibility.Visible;
        });

        _systemPlayer?.Dispose();
        _systemPlayer = new Windows.Media.Playback.MediaPlayer();
        _systemPlayer.MediaOpened += (_, _) => Info?.Invoke(this, "系统播放器已打开媒体。");
        _systemPlayer.MediaFailed += (_, args) => Error?.Invoke(this, $"系统播放器失败：{args.Error} / {args.ExtendedErrorCode}");
        _systemPlayer.Source = _systemMediaSourceFactory.Create(url);
        _systemPlayerElement.SetMediaPlayer(_systemPlayer);
        _systemPlayer.Play();
        Info?.Invoke(this, $"系统播放器加载：{url}");
    }

    private void PlayVlc(string url, string? referer, string? userAgent)
    {
        _dq.TryEnqueue(() =>
        {
            _systemPlayerElement.Visibility = Visibility.Collapsed;
            _vlcImage.Visibility = Visibility.Visible;
        });

        try
        {
            Core.Initialize();
        }
        catch
        {
            // ignore - Core.Initialize may not be required in some setups
        }

        _vlc ??= new LibVLC();
        _vlcPlayer?.Dispose();
        _vlcPlayer = new LibVLCSharp.Shared.MediaPlayer(_vlc);
        _vlcPlayer.EncounteredError += (_, _) => Error?.Invoke(this, "VLC 播放遇到错误。");

        _vlcPlayer.SetVideoFormatCallbacks(VideoFormat, VideoCleanup);
        _vlcPlayer.SetVideoCallbacks(VideoLock, VideoUnlock, VideoDisplay);

        _vlcMedia?.Dispose();
        _vlcMedia = new Media(_vlc, new Uri(url));
        _vlcMedia.AddOption(":network-caching=300");
        _vlcMedia.AddOption(":live-caching=300");
        if (!string.IsNullOrWhiteSpace(referer))
        {
            _vlcMedia.AddOption($":http-referrer={referer}");
        }
        if (!string.IsNullOrWhiteSpace(userAgent))
        {
            _vlcMedia.AddOption($":http-user-agent={userAgent}");
        }

        var ok = _vlcPlayer.Play(_vlcMedia);
        if (!ok)
        {
            Error?.Invoke(this, "VLC 播放启动失败（Play 返回 false）。");
        }
        else
        {
            Info?.Invoke(this, $"VLC 播放加载：{url}");
        }
    }

    private uint VideoFormat(ref IntPtr opaque, IntPtr chroma, ref uint width, ref uint height, ref uint pitches, ref uint lines)
    {
        try
        {
            var fourcc = new byte[] { (byte)'R', (byte)'V', (byte)'3', (byte)'2' };
            Marshal.Copy(fourcc, 0, chroma, fourcc.Length);

            var pitch = width * 4;
            pitches = pitch;
            lines = height;

            lock (_videoGate)
            {
                _videoWidth = width;
                _videoHeight = height;
                _videoPitch = pitch;
                var size = checked((int)(pitch * height));
                if (_videoBuffer != IntPtr.Zero)
                {
                    Marshal.FreeHGlobal(_videoBuffer);
                    _videoBuffer = IntPtr.Zero;
                }
                _videoBuffer = Marshal.AllocHGlobal(size);
                _videoBufferSize = size;
                _managedCopy = new byte[size];
                _bitmap = null;
            }

            _dq.TryEnqueue(() =>
            {
                lock (_videoGate)
                {
                    if (_videoWidth == 0 || _videoHeight == 0)
                    {
                        return;
                    }
                    _bitmap = new WriteableBitmap((int)_videoWidth, (int)_videoHeight);
                    _vlcImage.Source = _bitmap;
                }
            });

            return 1;
        }
        catch (Exception ex)
        {
            Error?.Invoke(this, $"VLC 视频格式初始化失败：{ex.Message}");
            return 0;
        }
    }

    private void VideoCleanup(ref IntPtr opaque)
    {
        lock (_videoGate)
        {
            if (_videoBuffer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(_videoBuffer);
                _videoBuffer = IntPtr.Zero;
            }
            _videoBufferSize = 0;
            _managedCopy = null;
            _bitmap = null;
        }

        opaque = IntPtr.Zero;
    }

    private IntPtr VideoLock(IntPtr opaque, IntPtr planes)
    {
        lock (_videoGate)
        {
            if (_videoBuffer == IntPtr.Zero)
            {
                return IntPtr.Zero;
            }
            Marshal.WriteIntPtr(planes, _videoBuffer);
            return IntPtr.Zero;
        }
    }

    private void VideoUnlock(IntPtr opaque, IntPtr picture, IntPtr planes)
    {
        // no-op
    }

    private void VideoDisplay(IntPtr opaque, IntPtr picture)
    {
        var now = Stopwatch.GetTimestamp();
        var last = Interlocked.Read(ref _lastFrameTicks);
        // Throttle to ~30fps on UI copy.
        if (last != 0)
        {
            var dt = (now - last) / (double)Stopwatch.Frequency;
            if (dt < (1.0 / 30.0))
            {
                return;
            }
        }
        Interlocked.Exchange(ref _lastFrameTicks, now);

        if (_frameQueued)
        {
            return;
        }
        _frameQueued = true;

        _dq.TryEnqueue(() =>
        {
            try
            {
                WriteFrameToBitmap();
            }
            finally
            {
                _frameQueued = false;
            }
        });
    }

    private void WriteFrameToBitmap()
    {
        WriteableBitmap? bmp;
        byte[]? buf;
        IntPtr src;
        int len;

        lock (_videoGate)
        {
            bmp = _bitmap;
            buf = _managedCopy;
            src = _videoBuffer;
            len = _videoBufferSize;
        }

        if (bmp is null || buf is null || src == IntPtr.Zero || len <= 0)
        {
            return;
        }

        try
        {
            Marshal.Copy(src, buf, 0, len);
            using var s = bmp.PixelBuffer.AsStream();
            s.Position = 0;
            s.Write(buf, 0, len);
            bmp.Invalidate();
        }
        catch (Exception ex)
        {
            Error?.Invoke(this, $"VLC 渲染失败：{ex.Message}");
        }
    }

    public void Dispose()
    {
        Stop();
        try
        {
            _vlc?.Dispose();
        }
        catch { }
        finally
        {
            _vlc = null;
        }
    }
}
