using ChaosSeed.WinUI3.Models;
using ChaosSeed.WinUI3.Models.Tts;
using ChaosSeed.WinUI3.Services;
using ChaosSeed.WinUI3.Services.TtsBackends;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class TtsDebugPage : Page
{
    private TtsService? _tts;
    private ITtsBackend? _backend;

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
            SetBackendBoxFromSettings(s.TtsBackendMode);

            // 一些常用默认值（对齐 docs/tts_cosyvoice3_sft.md 示例）。
            SpeedBox.Value = 1.1;
            SeedBox.Value = 1986;
            TemperatureBox.Value = 1.0;
            TopPBox.Value = 0.75;
            TopKBox.Value = 20;
            WinSizeBox.Value = 10;
            TauRBox.Value = 1.0;
            GuideSepBox.Text = "。 ";

            // PyO3/PT 默认参数（对齐 uv run python tools/infer_sft.py 示例）。
            if (string.IsNullOrWhiteSpace(PythonWorkdirBox.Text))
            {
                PythonWorkdirBox.Text = Path.Combine(AppContext.BaseDirectory, "voicelab", "workflows", "cosyvoice");
            }
            if (string.IsNullOrWhiteSpace(ModelDirBox.Text))
            {
                // infer_sft.py 的 --model_dir 允许相对 workdir（更方便打包）。
                ModelDirBox.Text = "pretrained_models/Fun-CosyVoice3-0.5B-dream-sft";
            }
            if (string.IsNullOrWhiteSpace(LlmCkptBox.Text))
            {
                LlmCkptBox.Text = "exp/dream_sft/llm/torch_ddp/epoch_5_whole.pt";
            }
            if (string.IsNullOrWhiteSpace(FlowCkptBox.Text))
            {
                FlowCkptBox.Text = "exp/dream_sft/flow/torch_ddp/flow_avg.pt";
            }
            if (string.IsNullOrWhiteSpace(SpkIdBox.Text))
            {
                SpkIdBox.Text = "dream";
            }

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

    private void SetBackendBoxFromSettings(LiveBackendMode mode)
    {
        try
        {
            var want = mode switch
            {
                LiveBackendMode.Daemon => "daemon",
                LiveBackendMode.Ffi => "ffi",
                _ => "auto",
            };

            foreach (var it in BackendBox.Items)
            {
                if (it is ComboBoxItem cb && string.Equals(cb.Content?.ToString()?.Trim(), want, StringComparison.OrdinalIgnoreCase))
                {
                    BackendBox.SelectedItem = cb;
                    return;
                }
            }
        }
        catch
        {
            // ignore
        }
    }

    private static LiveBackendMode GetBackendModeFromBox(ComboBox box)
    {
        var s = (box.SelectedItem as ComboBoxItem)?.Content?.ToString()?.Trim()?.ToLowerInvariant();
        return s switch
        {
            "daemon" => LiveBackendMode.Daemon,
            "ffi" => LiveBackendMode.Ffi,
            _ => LiveBackendMode.Auto,
        };
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
            LlmCkpt = string.IsNullOrWhiteSpace(LlmCkptBox.Text) ? null : LlmCkptBox.Text.Trim(),
            FlowCkpt = string.IsNullOrWhiteSpace(FlowCkptBox.Text) ? null : FlowCkptBox.Text.Trim(),
            PythonWorkdir = string.IsNullOrWhiteSpace(PythonWorkdirBox.Text) ? null : PythonWorkdirBox.Text.Trim(),
            PythonInferScript = string.IsNullOrWhiteSpace(PythonInferScriptBox.Text) ? null : PythonInferScriptBox.Text.Trim(),
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
        var backendMode = GetBackendModeFromBox(BackendBox);
        SettingsService.Instance.Update(x => x.TtsBackendMode = backendMode);

        _backend?.Dispose();
        _backend = TtsBackendFactory.Create();
        _tts = TtsService.CreateWithDaemon(_backend, DaemonClient.Instance);

        if (!string.IsNullOrWhiteSpace(_backend.InitNotice))
        {
            ShowInfo("后端提示", _backend.InitNotice!);
        }

        ShowInfo("开始生成", $"已提交到 {_backend.Name}（tts.sft.start），正在等待状态推送/轮询…");

        _cts = new CancellationTokenSource();
        _sessionId = null;
        try
        {
            var ct = _cts.Token;

            // 优先使用 daemon 的 tts.sft.statusChanged 通知；若 daemon 不支持则自动回退到轮询。
            var progress = new Progress<TtsSftStatus>(st =>
            {
                try { StageText.Text = $"stage: {st.Stage ?? "-"} ({st.State})"; } catch { }
            });

            var (sid, meta, wav) = await (_tts ?? throw new InvalidOperationException("tts backend not initialized"))
                .SynthesizeSftToWavBytesAsync(
                    p,
                    progress,
                    pollInterval: TimeSpan.FromMilliseconds(250),
                    onSessionId: id => _sessionId = id,
                    ct
                );

            var clip = new TtsClip
            {
                SessionId = sid,
                Text = text,
                Mime = meta.Mime,
                WavBytes = wav,
                SampleRate = meta.SampleRate,
                Channels = meta.Channels,
                DurationMs = meta.DurationMs,
            };

            var autoPlay = AutoPlayToggle.IsChecked ?? true;
            await TtsPlayerService.Instance.OpenAsync(clip, autoPlay, ct);
            ShowInfo("生成完成", $"已缓存到内存（{wav.Length} bytes），可在底栏播放器中播放/保存。");
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
            try { _backend?.Dispose(); } catch { }
            _backend = null;
            _tts = null;
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
            await (_tts ?? throw new InvalidOperationException("tts backend not initialized")).CancelAsync(sid, CancellationToken.None);
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
