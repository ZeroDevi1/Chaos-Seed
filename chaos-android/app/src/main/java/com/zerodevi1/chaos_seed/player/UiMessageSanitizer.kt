package com.zerodevi1.chaos_seed.player

private val URL_REGEX = Regex("https?://\\S+", RegexOption.IGNORE_CASE)
private val WHITESPACE_REGEX = Regex("\\s+")

fun sanitizeForSnackbar(raw: String): String {
    val noNewlines = raw.replace('\n', ' ').replace('\r', ' ')
    val withoutUrls = URL_REGEX.replace(noNewlines, "")
    return WHITESPACE_REGEX.replace(withoutUrls, " ").trim()
}

