package com.zerodevi1.chaos_seed.ui.components

fun formatOnlineCount(n: Long?): String? {
    val v = n ?: return null
    if (v < 0) return null
    if (v < 10_000) return v.toString()
    val w = v / 10_000.0
    val fixed = if (w >= 10) w.toInt().toString() else String.format("%.1f", w)
    // Trim trailing ".0" in the small range.
    val s = fixed.removeSuffix(".0")
    return "${s}万"
}
