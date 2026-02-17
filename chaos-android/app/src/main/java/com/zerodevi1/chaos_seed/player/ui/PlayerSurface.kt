package com.zerodevi1.chaos_seed.player.ui

import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.viewinterop.AndroidView

@Composable
fun PlayerSurface(
    modifier: Modifier = Modifier,
    onSurfaceReady: (Surface) -> Unit,
    onSurfaceDestroyed: () -> Unit,
) {
    val ctx = LocalContext.current
    val onReady = rememberUpdatedState(onSurfaceReady)
    val onDestroyed = rememberUpdatedState(onSurfaceDestroyed)

    DisposableEffect(Unit) {
        onDispose { onDestroyed.value() }
    }

    AndroidView(
        modifier = modifier,
        factory = {
            SurfaceView(ctx).apply {
                holder.addCallback(
                    object : SurfaceHolder.Callback {
                        override fun surfaceCreated(holder: SurfaceHolder) {
                            onReady.value(holder.surface)
                        }

                        override fun surfaceChanged(
                            holder: SurfaceHolder,
                            format: Int,
                            width: Int,
                            height: Int,
                        ) {
                            onReady.value(holder.surface)
                        }

                        override fun surfaceDestroyed(holder: SurfaceHolder) {
                            onDestroyed.value()
                        }
                    },
                )
            }
        },
    )
}
