using System;
using System.Diagnostics;
using System.Threading;
using System.Threading.Tasks;
using System.Runtime.InteropServices;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Xaml;
using WinRT;

namespace ChaosSeed.WinUI3;

public static class Program
{
    [DllImport("Microsoft.ui.xaml.dll")]
    private static extern void XamlCheckProcessRequirements();

    [STAThread]
    public static void Main(string[] args)
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
