package com.zerodevi1.chaos_seed.player.engine

import android.content.Context
import android.view.Surface
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.withContext

class Media3ExoEngine(appContext: Context) : PlayerEngine {
    private val httpFactory = DefaultHttpDataSource.Factory()
    private val player: ExoPlayer

    private val _state = MutableStateFlow(PlayerState())
    override val state: StateFlow<PlayerState> = _state

    init {
        val dsFactory = DefaultDataSource.Factory(appContext, httpFactory)
        val msFactory = DefaultMediaSourceFactory(dsFactory)
        player = ExoPlayer.Builder(appContext)
            .setMediaSourceFactory(msFactory)
            .build()

        player.addListener(
            object : Player.Listener {
                override fun onPlaybackStateChanged(playbackState: Int) {
                    val buffering = playbackState == Player.STATE_BUFFERING
                    _state.value = _state.value.copy(buffering = buffering)
                }

                override fun onIsPlayingChanged(isPlaying: Boolean) {
                    _state.value = _state.value.copy(playing = isPlaying, error = null)
                }

                override fun onVideoSizeChanged(videoSize: androidx.media3.common.VideoSize) {
                    _state.value = _state.value.copy(
                        videoWidth = videoSize.width,
                        videoHeight = videoSize.height,
                    )
                }

                override fun onPlayerError(error: PlaybackException) {
                    _state.value = _state.value.copy(error = error.message ?: error.errorCodeName)
                }
            },
        )
    }

    override suspend fun open(url: String, headers: Map<String, String>) {
        withContext(Dispatchers.Main.immediate) {
            httpFactory.setDefaultRequestProperties(headers)
            player.setMediaItem(MediaItem.fromUri(url))
            player.prepare()
            player.playWhenReady = true
        }
    }

    override suspend fun play() {
        withContext(Dispatchers.Main.immediate) { player.play() }
    }

    override suspend fun pause() {
        withContext(Dispatchers.Main.immediate) { player.pause() }
    }

    override suspend fun setVolume(volume0to100: Int) {
        val v = volume0to100.coerceIn(0, 100) / 100.0f
        withContext(Dispatchers.Main.immediate) { player.volume = v }
    }

    override fun attachSurface(surface: Surface) {
        player.setVideoSurface(surface)
    }

    override fun detachSurface() {
        player.clearVideoSurface()
    }

    override fun release() {
        player.release()
    }
}

