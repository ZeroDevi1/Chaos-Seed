package com.zerodevi1.chaos_seed.player

import org.junit.Assert.assertEquals
import org.junit.Test

class UiMessageSanitizerTest {
    @Test
    fun `removes single url`() {
        val raw = "切换线路失败 https://a.com/x?y=z"
        assertEquals("切换线路失败", sanitizeForSnackbar(raw))
    }

    @Test
    fun `removes multiple urls and normalizes whitespace`() {
        val raw = "第一行\n第二行 https://a.com/x  再来一个 https://b.com/y?q=1"
        assertEquals("第一行 第二行 再来一个", sanitizeForSnackbar(raw))
    }

    @Test
    fun `url only becomes empty`() {
        assertEquals("", sanitizeForSnackbar("https://a.com/x?y=z"))
    }
}

