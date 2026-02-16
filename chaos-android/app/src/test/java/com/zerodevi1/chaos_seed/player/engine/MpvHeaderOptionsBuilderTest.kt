package com.zerodevi1.chaos_seed.player.engine

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class MpvHeaderOptionsBuilderTest {
    @Test
    fun emptyHeaders() {
        val o = MpvHeaderOptionsBuilder.fromHeaders(emptyMap())
        assertNull(o.userAgent)
        assertNull(o.referer)
        assertNull(o.httpHeaderFields)
    }

    @Test
    fun extractsUserAgentAndReferer() {
        val o = MpvHeaderOptionsBuilder.fromHeaders(
            mapOf(
                "User-Agent" to "ua",
                "Referer" to "https://example.com",
            ),
        )
        assertEquals("ua", o.userAgent)
        assertEquals("https://example.com", o.referer)
        assertNull(o.httpHeaderFields)
    }

    @Test
    fun otherHeadersBecomeHttpHeaderFields() {
        val o = MpvHeaderOptionsBuilder.fromHeaders(
            linkedMapOf(
                "User-Agent" to "ua",
                "X-Test" to "1",
                "Accept" to "application/x-mpegURL",
            ),
        )
        assertEquals("ua", o.userAgent)
        assertNull(o.referer)
        assertEquals("X-Test: 1\nAccept: application/x-mpegURL", o.httpHeaderFields)
    }
}

