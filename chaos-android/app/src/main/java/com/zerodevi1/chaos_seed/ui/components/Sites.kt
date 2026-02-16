package com.zerodevi1.chaos_seed.ui.components

import androidx.annotation.DrawableRes
import com.zerodevi1.chaos_seed.R

data class LiveSite(
    val key: String,
    val label: String,
    @field:DrawableRes val iconRes: Int,
)

object LiveSites {
    val all: List<LiveSite> = listOf(
        LiveSite(key = "bili_live", label = "哔哩哔哩", iconRes = R.drawable.bilibili),
        LiveSite(key = "huya", label = "虎牙直播", iconRes = R.drawable.huya),
        LiveSite(key = "douyu", label = "斗鱼直播", iconRes = R.drawable.douyu),
    )
}
