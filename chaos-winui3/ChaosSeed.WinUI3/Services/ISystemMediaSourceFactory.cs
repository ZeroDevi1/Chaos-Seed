using Windows.Media.Core;

namespace ChaosSeed.WinUI3.Services;

public interface ISystemMediaSourceFactory
{
    MediaSource Create(string url);
}

public sealed class DefaultSystemMediaSourceFactory : ISystemMediaSourceFactory
{
    public MediaSource Create(string url) => MediaSource.CreateFromUri(new Uri(url));
}

