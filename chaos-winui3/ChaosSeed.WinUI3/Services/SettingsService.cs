using System.IO;
using System.Text.Json;
using ChaosSeed.WinUI3.Models;
using Windows.Storage;

namespace ChaosSeed.WinUI3.Services;

public sealed class SettingsService
{
    public static SettingsService Instance => _instance.Value;
    private static readonly Lazy<SettingsService> _instance = new(() => new SettingsService());

    private const string SettingsKey = "chaosSeed.settings.v1";
    private readonly object _gate = new();
    private readonly ISettingsStore _store;

    public event EventHandler? SettingsChanged;

    public AppSettings Current { get; private set; } = new();

    private SettingsService()
    {
        _store = TryCreateApplicationDataStore() ?? new FileSettingsStore(GetDefaultSettingsPath());
        Reload();
    }

    public void Reload()
    {
        lock (_gate)
        {
            var json = _store.TryLoad(SettingsKey);
            if (!string.IsNullOrWhiteSpace(json))
            {
                try
                {
                    Current = JsonSerializer.Deserialize<AppSettings>(json!) ?? new AppSettings();
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
        _store.Save(SettingsKey, json);
    }

    private static ISettingsStore? TryCreateApplicationDataStore()
    {
        try
        {
            _ = ApplicationData.Current.LocalSettings.Values;
            return new ApplicationDataSettingsStore();
        }
        catch
        {
            return null;
        }
    }

    private static string GetDefaultSettingsPath()
    {
        var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        return Path.Combine(root, "ChaosSeed.WinUI3", "settings.v1.json");
    }

    private interface ISettingsStore
    {
        string? TryLoad(string key);
        void Save(string key, string json);
    }

    private sealed class ApplicationDataSettingsStore : ISettingsStore
    {
        public string? TryLoad(string key)
        {
            try
            {
                var values = ApplicationData.Current.LocalSettings.Values;
                if (values.TryGetValue(key, out var raw) && raw is string json)
                {
                    return json;
                }
            }
            catch
            {
                // ignore (no app identity / runtime not available)
            }
            return null;
        }

        public void Save(string key, string json)
        {
            try
            {
                ApplicationData.Current.LocalSettings.Values[key] = json;
            }
            catch
            {
                // ignore (no app identity / runtime not available)
            }
        }
    }

    private sealed class FileSettingsStore : ISettingsStore
    {
        private readonly string _path;

        public FileSettingsStore(string path)
        {
            _path = path;
        }

        public string? TryLoad(string key)
        {
            try
            {
                if (!File.Exists(_path))
                {
                    return null;
                }

                return File.ReadAllText(_path);
            }
            catch
            {
                return null;
            }
        }

        public void Save(string key, string json)
        {
            try
            {
                var dir = Path.GetDirectoryName(_path);
                if (!string.IsNullOrWhiteSpace(dir))
                {
                    Directory.CreateDirectory(dir);
                }

                var tmp = _path + ".tmp";
                File.WriteAllText(tmp, json);
                File.Copy(tmp, _path, overwrite: true);
                File.Delete(tmp);
            }
            catch
            {
                // ignore
            }
        }
    }
}
