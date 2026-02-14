using ChaosSeed.WinUI3.Services;
using Xunit;

namespace ChaosSeed.WinUI3.NativeOverlay.Tests;

public sealed class UpdateServiceVersionTests
{
    [Fact]
    public void NormalizeVersion_strips_v_prefix_and_trims()
    {
        Assert.Equal("0.4.0", UpdateService.NormalizeVersion("v0.4.0"));
        Assert.Equal("0.4.0", UpdateService.NormalizeVersion("  0.4.0  "));
        Assert.Equal("1.2.3", UpdateService.NormalizeVersion("V1.2.3"));
        Assert.Equal("", UpdateService.NormalizeVersion(""));
    }

    [Fact]
    public void CompareVersionStrings_compares_numeric_parts()
    {
        Assert.True(UpdateService.CompareVersionStrings("0.4.0", "0.4.1") < 0);
        Assert.True(UpdateService.CompareVersionStrings("0.10.0", "0.9.9") > 0);
        Assert.Equal(0, UpdateService.CompareVersionStrings("1.0.0", "1.0.0"));
        Assert.Equal(0, UpdateService.CompareVersionStrings("1.0", "1.0.0"));
        Assert.Equal(0, UpdateService.CompareVersionStrings("0.4.0-beta", "0.4.0"));
    }
}
