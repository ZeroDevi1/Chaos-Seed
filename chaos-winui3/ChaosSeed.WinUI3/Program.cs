using System.Runtime.InteropServices;
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

        Application.Start(_ => new App());
    }
}
