using System;
using System.Diagnostics;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using System.Runtime.InteropServices;
using ChaosSeed.WinUI3.Cli;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Xaml;
using WinRT;

namespace ChaosSeed.WinUI3;

public static class Program
{
    [DllImport("Microsoft.ui.xaml.dll")]
    private static extern void XamlCheckProcessRequirements();

    [STAThread]
    public static async Task Main(string[] args)
    {
        AppDomain.CurrentDomain.UnhandledException += (_, e) =>
        {
            try
            {
                if (e.ExceptionObject is Exception ex)
                {
                    AppLog.Exception("Program.UnhandledException", ex);
                }
                else
                {
                    AppLog.Error($"Program.UnhandledException: {e.ExceptionObject}");
                }
            }
            catch { }
        };

        TaskScheduler.UnobservedTaskException += (_, e) =>
        {
            try { AppLog.Exception("Program.UnobservedTaskException", e.Exception); } catch { }
            try { e.SetObserved(); } catch { }
        };

#if DEBUG
        AppDomain.CurrentDomain.FirstChanceException += (_, e) =>
        {
            if (e.Exception is not COMException com)
            {
                return;
            }

            try
            {
                Debug.WriteLine($"[Program.COM] HRESULT=0x{com.HResult:X8} {com.Message}");
            }
            catch { }
        };
#endif

        AppLog.Info($"Startup begin; args=[{string.Join(" ", args ?? Array.Empty<string>())}]");
        AppLog.Info($"Process={Environment.ProcessPath} BaseDir={AppContext.BaseDirectory}");
        AppLog.Info($".NET={Environment.Version} OS={Environment.OSVersion} Arch={RuntimeInformation.ProcessArchitecture}");

        // 检查是否为 CLI 模式
        if (CliParser.IsCliMode(args))
        {
            AppLog.Info("Running in CLI mode");
            try
            {
                var exitCode = await RunCliModeAsync(args);
                Environment.Exit(exitCode);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"CLI Error: {ex.Message}");
                Environment.Exit(1);
            }
            return;
        }

        // GUI 模式初始化
        await InitializeGuiModeAsync();
    }

    /// <summary>
    /// 运行 CLI 模式
    /// </summary>
    private static async Task<int> RunCliModeAsync(string[] args)
    {
        // 注册取消键 (Ctrl+C)
        using var cts = new CancellationTokenSource();
        Console.CancelKeyPress += (_, e) =>
        {
            e.Cancel = true;
            cts.Cancel();
        };

        var runner = new CliRunner();
        return await runner.RunAsync(args, cts.Token);
    }

    /// <summary>
    /// 初始化 GUI 模式
    /// </summary>
    private static async Task InitializeGuiModeAsync()
    {
        // TTS（PyO3/PT）：为 zip 分发"解压即用"做一层默认环境注入。
        // 说明：FFI 路径下 Rust 引擎在本进程内运行，因此需要在首次调用 PyO3 初始化前设置这些环境变量。
        try
        {
            var baseDir = AppContext.BaseDirectory;
            var pyHome = Path.Combine(baseDir, "python");
            var venvRoot = Path.Combine(baseDir, ".venv");
            var venvSite = Path.Combine(venvRoot, "Lib", "site-packages");
            var workdir = Path.Combine(baseDir, "voicelab", "workflows", "cosyvoice");

            if (Directory.Exists(pyHome))
            {
                // 嵌入式 Python：强制指定标准库位置，避免 "No module named encodings"。
                if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("PYTHONHOME")))
                {
                    Environment.SetEnvironmentVariable("PYTHONHOME", pyHome);
                }
                if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("PYTHONPATH")))
                {
                    Environment.SetEnvironmentVariable("PYTHONPATH", $"{Path.Combine(pyHome, "Lib")};{Path.Combine(pyHome, "DLLs")}");
                }

                // 让 python310.dll/依赖 DLL 可被加载器找到。
                var path = Environment.GetEnvironmentVariable("PATH") ?? "";
                if (!path.Contains(pyHome, StringComparison.OrdinalIgnoreCase))
                {
                    Environment.SetEnvironmentVariable("PATH", $"{pyHome};{path}");
                }
            }

            if (Directory.Exists(venvSite) && string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("CHAOS_TTS_PY_VENV_SITE_PACKAGES")))
            {
                Environment.SetEnvironmentVariable("CHAOS_TTS_PY_VENV_SITE_PACKAGES", venvSite);
            }

            if (Directory.Exists(workdir) && string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("CHAOS_TTS_PY_WORKDIR")))
            {
                Environment.SetEnvironmentVariable("CHAOS_TTS_PY_WORKDIR", workdir);
            }

            // 默认使用打包内的 dream_sft 权重（用户可在 UI/配置中覆盖）。
            if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("CHAOS_TTS_PY_MODEL_DIR")))
            {
                Environment.SetEnvironmentVariable("CHAOS_TTS_PY_MODEL_DIR", "pretrained_models/Fun-CosyVoice3-0.5B-dream-sft");
            }
            if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("CHAOS_TTS_PY_LLM_CKPT")))
            {
                Environment.SetEnvironmentVariable("CHAOS_TTS_PY_LLM_CKPT", "exp/dream_sft/llm/torch_ddp/epoch_5_whole.pt");
            }
            if (string.IsNullOrWhiteSpace(Environment.GetEnvironmentVariable("CHAOS_TTS_PY_FLOW_CKPT")))
            {
                Environment.SetEnvironmentVariable("CHAOS_TTS_PY_FLOW_CKPT", "exp/dream_sft/flow/torch_ddp/flow_avg.pt");
            }
        }
        catch (Exception ex)
        {
            // best-effort：不因环境注入失败而阻断启动
            AppLog.Exception("Program.TtsPythonEnvInit", ex);
        }

        ComWrappersSupport.InitializeComWrappers();
        AppLog.Info("ComWrappers initialized");

        // For WinUI 3 unpackaged apps: validate Windows App Runtime and dependencies before starting XAML.
        AppLog.Info("Calling XamlCheckProcessRequirements...");
        XamlCheckProcessRequirements();
        AppLog.Info("XamlCheckProcessRequirements OK");

        // Best-effort: register AppUserModelID + Start Menu shortcut so SMTC/GSMTC can show app name/icon.
        ShellAppIdentityService.TryEnsureChaosSeedIdentity();
        AppLog.Info("Shell app identity ensured (best-effort)");

        Application.Start(p =>
        {
            var context = new Microsoft.UI.Dispatching.DispatcherQueueSynchronizationContext(
                Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread()
            );
            SynchronizationContext.SetSynchronizationContext(context);
            _ = p;
            new App();
        });
    }
}
