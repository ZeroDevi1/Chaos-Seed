package com.zerodevi1.chaos_seed.core.model

import kotlinx.serialization.Serializable

@Serializable
data class LiveDirSubCategory(
    val id: String,
    val parentId: String,
    val name: String,
    val pic: String? = null,
)

@Serializable
data class LiveDirCategory(
    val id: String,
    val name: String,
    val children: List<LiveDirSubCategory> = emptyList(),
)

@Serializable
data class LiveDirRoomCard(
    val site: String,
    val roomId: String,
    val input: String,
    val title: String,
    val cover: String? = null,
    val userName: String? = null,
    val online: Long? = null,
)

@Serializable
data class LiveDirRoomListResult(
    val hasMore: Boolean,
    val items: List<LiveDirRoomCard> = emptyList(),
)

