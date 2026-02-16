package com.zerodevi1.chaos_seed.core.json

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull

fun JsonObject.pickElement(keys: List<String>): JsonElement? {
    for (k in keys) {
        val v = this[k]
        if (v != null && v !is JsonNull) return v
    }
    return null
}

fun JsonObject.pickString(keys: List<String>, fallback: String = ""): String {
    val v = pickElement(keys) ?: return fallback
    return (v as? JsonPrimitive)?.contentOrNull ?: fallback
}

fun JsonObject.pickStringOrNull(keys: List<String>): String? {
    val s = pickString(keys, fallback = "")
    return s.ifBlank { null }
}

fun JsonObject.pickIntOrNull(keys: List<String>): Int? {
    val v = pickElement(keys) ?: return null
    val p = v as? JsonPrimitive ?: return null
    return p.intOrNull ?: p.longOrNull?.toInt() ?: p.doubleOrNull?.toInt()
}

fun JsonObject.pickLongOrNull(keys: List<String>): Long? {
    val v = pickElement(keys) ?: return null
    val p = v as? JsonPrimitive ?: return null
    return p.longOrNull ?: p.intOrNull?.toLong() ?: p.doubleOrNull?.toLong()
}

fun JsonObject.pickBool(keys: List<String>, fallback: Boolean = false): Boolean {
    val v = pickElement(keys) ?: return fallback
    val p = v as? JsonPrimitive ?: return fallback
    return p.booleanOrNull ?: fallback
}

fun JsonObject.pickObject(keys: List<String>): JsonObject? {
    val v = pickElement(keys) ?: return null
    return runCatching { v.jsonObject }.getOrNull()
}

fun JsonObject.pickArray(keys: List<String>): JsonArray? {
    val v = pickElement(keys) ?: return null
    return runCatching { v.jsonArray }.getOrNull()
}
