using Windows.ApplicationModel;

namespace ChaosSeed.WinUI3.Services;

public static class AppIdentityService
{
    // Unpackaged apps do not have an AppX identity; Package.Current will throw.
    public static bool IsPackaged
    {
        get
        {
            try
            {
                _ = Package.Current.Id;
                return true;
            }
            catch
            {
                return false;
            }
        }
    }
}

