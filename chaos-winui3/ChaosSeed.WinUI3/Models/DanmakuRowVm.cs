using System.ComponentModel;
using System.Runtime.CompilerServices;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media.Imaging;

namespace ChaosSeed.WinUI3.Models;

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

    public string UserLabel => $"{User}:";

    public string MessageText
    {
        get
        {
            if (_emote is not null && IsImagePlaceholderText(Text))
            {
                return "";
            }
            return Text;
        }
    }

    public string DisplayText
    {
        get
        {
            if (_emote is not null && IsImagePlaceholderText(Text))
            {
                return $"{User}:";
            }
            return $"{User}: {Text}";
        }
    }

    public Visibility EmoteVisibility => _emote is null ? Visibility.Collapsed : Visibility.Visible;

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
            OnPropertyChanged(nameof(DisplayText));
            OnPropertyChanged(nameof(MessageText));
            OnPropertyChanged(nameof(EmoteVisibility));
        }
    }

    public static bool IsImagePlaceholderText(string? text)
    {
        var t = (text ?? "").Trim();
        if (t.Length == 0)
        {
            return true;
        }
        return string.Equals(t, "[图片]", StringComparison.OrdinalIgnoreCase)
            || string.Equals(t, "[image]", StringComparison.OrdinalIgnoreCase)
            || string.Equals(t, "[img]", StringComparison.OrdinalIgnoreCase);
    }

    private void OnPropertyChanged([CallerMemberName] string? name = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(name));
    }
}

