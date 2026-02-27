using ChaosSeed.WinUI3.Models.Tts;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class TtsDebugPage : Page
{
    private readonly TtsService _tts = new(DaemonClient.Instance);

    private CancellationTokenSource? _cts;
    private string? _sessionId;

    public TtsDebugPage()
    {
        InitializeComponent();
        Loaded += (_, _) => ApplyDefaultsFromSettings();
        Unloaded += (_, _) => { try { _cts?.Cancel(); } catch { } };
    }

    private void ApplyDefaultsFromSettings()
    {
        try
        {
            var s = SettingsService.Instance.Current;
            ModelDirBox.Text = (s.TtsCosyVoicePackDir ?? "").Trim();
            SpkIdBox.Text = (s.TtsLastSpkId ?? "").Trim();

            // 一些常用默认值（对齐 docs/tts_cosyvoice3_sft.md 示例）。
            SpeedBox.Value = 1.1;
            SeedBox.Value = 1986;
            TemperatureBox.Value = 1.0;
            TopPBox.Value = 0.75;
            TopKBox.Value = 20;
            WinSizeBox.Value = 10;
            TauRBox.Value = 1.0;
            GuideSepBox.Text = "。 ";

            if (string.IsNullOrWhiteSpace(InputTextBox.Text))
            {
                InputTextBox.Text = "你好";
            }
            if (string.IsNullOrWhiteSpace(PromptTextBox.Text))
            {
                PromptTextBox.Text = "<|endofprompt|>";
            }
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsDebugPage.ApplyDefaultsFromSettings", ex);
        }
    }

    private async void OnPickModelDirClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        try
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
            if (folder is null)
            {
                return;
            }

            ModelDirBox.Text = folder.Path;
            SettingsService.Instance.Update(x => x.TtsCosyVoicePackDir = folder.Path);
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsDebugPage.PickModelDir", ex);
            ShowError("选择目录失败", ex.Message);
        }
    }

    private async void OnGenerateClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;

        if (_cts is not null)
        {
            return;
        }

        var modelDir = (ModelDirBox.Text ?? "").Trim();
        var spkId = (SpkIdBox.Text ?? "").Trim();
        var text = (InputTextBox.Text ?? "").Trim();

        if (string.IsNullOrWhiteSpace(modelDir))
        {
            ShowError("参数错误", "modelDir 为空");
            return;
        }
        if (string.IsNullOrWhiteSpace(spkId))
        {
            ShowError("参数错误", "spkId 为空");
            return;
        }
        if (string.IsNullOrWhiteSpace(text))
        {
            ShowError("参数错误", "text 为空");
            return;
        }

        SettingsService.Instance.Update(x =>
        {
            x.TtsCosyVoicePackDir = modelDir;
            x.TtsLastSpkId = spkId;
        });

        var promptStrategy = (PromptStrategyBox.SelectedItem as ComboBoxItem)?.Content?.ToString()?.Trim();
        var guideSep = string.IsNullOrWhiteSpace(GuideSepBox.Text) ? null : GuideSepBox.Text.Trim();

        var p = new TtsSftStartParams
        {
            ModelDir = modelDir,
            SpkId = spkId,
            Text = text,
            PromptText = (PromptTextBox.Text ?? "").Trim(),
            PromptStrategy = string.IsNullOrWhiteSpace(promptStrategy) ? null : promptStrategy,
            GuideSep = guideSep,
            Speed = double.IsFinite(SpeedBox.Value) ? SpeedBox.Value : 1.0,
            Seed = (ulong)(double.IsFinite(SeedBox.Value) ? Math.Max(0, SeedBox.Value) : 1986),
            Temperature = double.IsFinite(TemperatureBox.Value) ? TemperatureBox.Value : 1.0,
            TopP = double.IsFinite(TopPBox.Value) ? TopPBox.Value : 0.75,
            TopK = (uint)(double.IsFinite(TopKBox.Value) ? Math.Max(1, TopKBox.Value) : 20),
            WinSize = (uint)(double.IsFinite(WinSizeBox.Value) ? Math.Max(1, WinSizeBox.Value) : 10),
            TauR = double.IsFinite(TauRBox.Value) ? TauRBox.Value : 1.0,
            TextFrontend = TextFrontendToggle.IsChecked ?? true,
        };

        GenerateBtn.IsEnabled = false;
        CancelBtn.IsEnabled = true;
        StageText.Text = "stage: starting";
        ShowInfo("开始生成", "已提交到 daemon（tts.sft.start），正在轮询状态…");

        _cts = new CancellationTokenSource();
        _sessionId = null;
        try
        {
            var ct = _cts.Token;
            var start = await _tts.StartSftAsync(p, ct);
            var sid = (start.SessionId ?? "").Trim();
            if (string.IsNullOrWhiteSpace(sid))
            {
                throw new InvalidOperationException("tts.sft.start returned empty sessionId");
            }
            _sessionId = sid;

            while (true)
            {
                ct.ThrowIfCancellationRequested();
                var st = await _tts.StatusAsync(sid, ct);
                StageText.Text = $"stage: {st.Stage ?? "-"} ({st.State})";
                if (!st.Done)
                {
                    await Task.Delay(TimeSpan.FromMilliseconds(250), ct);
                    continue;
                }

                if (!string.Equals(st.State, "done", StringComparison.OrdinalIgnoreCase))
                {
                    var err = (st.Error ?? "").Trim();
                    if (string.IsNullOrWhiteSpace(err))
                    {
                        err = $"tts job finished in state={st.State}";
                    }
                    throw new InvalidOperationException(err);
                }

                if (st.Result is null || string.IsNullOrWhiteSpace(st.Result.WavBase64))
                {
                    throw new InvalidOperationException("tts.sft.status returned done but result.wavBase64 is empty");
                }

                byte[] wav;
                try
                {
                    wav = Convert.FromBase64String(st.Result.WavBase64);
                }
                catch (FormatException ex)
                {
                    throw new InvalidOperationException("invalid base64 wav payload", ex);
                }

                var clip = new TtsClip
                {
                    SessionId = sid,
                    Text = text,
                    Mime = st.Result.Mime,
                    WavBytes = wav,
                    SampleRate = st.Result.SampleRate,
                    Channels = st.Result.Channels,
                    DurationMs = st.Result.DurationMs,
                };

                var autoPlay = AutoPlayToggle.IsChecked ?? true;
                await TtsPlayerService.Instance.OpenAsync(clip, autoPlay, ct);
                ShowInfo("生成完成", $"已缓存到内存（{wav.Length} bytes），可在底栏播放器中播放/保存。");
                break;
            }
        }
        catch (OperationCanceledException)
        {
            ShowInfo("已取消", "任务已取消");
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsDebugPage.Generate", ex);
            ShowError("生成失败", ex.Message);
        }
        finally
        {
            _cts?.Dispose();
            _cts = null;
            _sessionId = null;
            GenerateBtn.IsEnabled = true;
            CancelBtn.IsEnabled = false;
            StageText.Text = "stage: -";
        }
    }

    private async void OnCancelClicked(object sender, RoutedEventArgs e)
    {
        _ = sender;
        _ = e;
        try
        {
            _cts?.Cancel();
        }
        catch
        {
            // ignore
        }

        var sid = (_sessionId ?? "").Trim();
        if (string.IsNullOrWhiteSpace(sid))
        {
            return;
        }

        try
        {
            await _tts.CancelAsync(sid, CancellationToken.None);
        }
        catch (Exception ex)
        {
            AppLog.Exception("TtsDebugPage.Cancel", ex);
        }
    }

    private void ShowInfo(string title, string message)
    {
        StatusBar.Title = title;
        StatusBar.Message = message;
        StatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Informational;
        StatusBar.IsOpen = true;
    }

    private void ShowError(string title, string message)
    {
        StatusBar.Title = title;
        StatusBar.Message = message;
        StatusBar.Severity = Microsoft.UI.Xaml.Controls.InfoBarSeverity.Error;
        StatusBar.IsOpen = true;
    }
}
