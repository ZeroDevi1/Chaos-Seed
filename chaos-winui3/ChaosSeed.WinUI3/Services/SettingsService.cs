using System.Text.Json;
using ChaosSeed.WinUI3.Models;
using Windows.Storage;

namespace ChaosSeed.WinUI3.Services;

public sealed class SettingsService
{
    public static SettingsService Instance { get; } = new();

    private const string SettingsKey = "chaosSeed.settings.v1";
    private readonly object _gate = new();

    public event EventHandler? SettingsChanged;

    public AppSettings Current { get; private set; } = new();

    private SettingsService()
    {
        Reload();
    }

    public void Reload()
    {
        lock (_gate)
        {
            var values = ApplicationData.Current.LocalSettings.Values;
            if (values.TryGetValue(SettingsKey, out var raw) && raw is string json && !string.IsNullOrWhiteSpace(json))
            {
                try
                {
                    Current = JsonSerializer.Deserialize<AppSettings>(json) ?? new AppSettings();
                    return;
                }
                catch
                {
                    // ignore and reset to defaults
                }
            }

            Current = new AppSettings();
            PersistLocked();
        }
    }

    public void Update(Action<AppSettings> mutator)
    {
        lock (_gate)
        {
            mutator(Current);
            PersistLocked();
        }

        SettingsChanged?.Invoke(this, EventArgs.Empty);
    }

    private void PersistLocked()
    {
        var json = JsonSerializer.Serialize(Current);
        ApplicationData.Current.LocalSettings.Values[SettingsKey] = json;
    }
}

