using ChaosSeed.WinUI3.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace ChaosSeed.WinUI3.Pages;

public sealed partial class HomePage : Page
{
    public HomePage()
    {
        InitializeComponent();
    }

    private async void OnGoClicked(object sender, RoutedEventArgs e)
    {
        var input = (InputBox.Text ?? "").Trim();
        if (string.IsNullOrWhiteSpace(input))
        {
            ShowError("请输入直播间地址。");
            return;
        }

        GoBtn.IsEnabled = false;
        InputBox.IsEnabled = false;
        ShowInfo("解析中...");
        try
        {
            var res = await DaemonClient.Instance.OpenLiveAsync(input);
            StatusBar.IsOpen = false;

            if (App.MainWindowInstance is not MainWindow mw)
            {
                ShowError("无法获取主窗口实例，无法跳转到直播页面。");
                return;
            }
            mw.NavigateToLive(res);
        }
        catch (Exception ex)
        {
            ShowError(ex.Message);
        }
        finally
        {
            GoBtn.IsEnabled = true;
            InputBox.IsEnabled = true;
        }
    }

    private void ShowError(string msg)
    {
        StatusBar.Severity = InfoBarSeverity.Error;
        StatusBar.Title = "失败";
        StatusBar.Message = msg;
        StatusBar.IsOpen = true;
    }

    private void ShowInfo(string msg)
    {
        StatusBar.Severity = InfoBarSeverity.Informational;
        StatusBar.Title = "提示";
        StatusBar.Message = msg;
        StatusBar.IsOpen = true;
    }
}
