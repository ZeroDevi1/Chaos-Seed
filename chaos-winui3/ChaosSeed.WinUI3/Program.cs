using System.Threading;
using System.Runtime.InteropServices;
using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Dispatching;
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
        ComWrappersSupport.InitializeComWrappers();

        // For WinUI 3 unpackaged apps: validate Windows App Runtime and dependencies before starting XAML.
        XamlCheckProcessRequirements();

        // Best-effort: register AppUserModelID + Start Menu shortcut so SMTC/GSMTC can show app name/icon.
        ShellAppIdentityService.TryEnsureChaosSeedIdentity();

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
