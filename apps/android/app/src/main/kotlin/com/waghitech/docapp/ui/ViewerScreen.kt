package com.waghitech.docapp.ui

import android.graphics.BitmapFactory
import android.net.Uri
import androidx.compose.foundation.Image
import androidx.compose.foundation.gestures.rememberTransformableState
import androidx.compose.foundation.gestures.transformable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.waghitech.docapp.R
import com.waghitech.docapp.core.CacheSizer
import com.waghitech.docapp.core.PdfRenderHelper
import com.waghitech.docapp.core.UriFiles
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

@Composable
fun ViewerScreen(
    workFile: File,
    mime: String,
    onVisiblePage: (Int) -> Unit,
) {
  if (UriFiles.isImage(mime)) {
        ImageViewer(workFile)
    } else {
        PdfViewer(workFile, onVisiblePage)
    }
}

@Composable
private fun ImageViewer(file: File) {
    var scale by remember { mutableFloatStateOf(1f) }
    var offsetX by remember { mutableFloatStateOf(0f) }
    var offsetY by remember { mutableFloatStateOf(0f) }
    val bitmap = remember(file) { BitmapFactory.decodeFile(file.absolutePath) }
    val transformState =
        rememberTransformableState { zoom, pan, _ ->
            scale = (scale * zoom).coerceIn(1f, 6f)
            offsetX += pan.x
            offsetY += pan.y
        }
    Box(
        modifier =
            Modifier
                .fillMaxSize()
                .transformable(transformState),
        contentAlignment = Alignment.Center,
    ) {
        if (bitmap != null) {
            Image(
                bitmap = bitmap.asImageBitmap(),
                contentDescription = null,
                modifier =
                    Modifier.graphicsLayer {
                        scaleX = scale
                        scaleY = scale
                        translationX = offsetX
                        translationY = offsetY
                    },
                contentScale = ContentScale.Fit,
            )
        }
    }
}

@Composable
private fun PdfViewer(
    file: File,
    onVisiblePage: (Int) -> Unit,
) {
    val context = LocalContext.current
    val screenWidthDp = LocalConfiguration.current.screenWidthDp
    val widthPx = with(LocalDensity.current) { screenWidthDp.dp.roundToPx() }
    val cacheBytes = remember { CacheSizer.bitmapCacheMaxBytes(context) }
    val helper = remember(file) { PdfRenderHelper(file, cacheBytes) }
    DisposableEffect(helper) {
        onDispose { helper.close() }
    }
    val bitmaps = remember { mutableStateMapOf<Int, android.graphics.Bitmap>() }
    var loading by remember { mutableStateOf(false) }
    val listState = rememberLazyListState()
    LaunchedEffect(listState.firstVisibleItemIndex) {
        val page = listState.firstVisibleItemIndex + 1
        onVisiblePage(page)
    }
    Column(modifier = Modifier.fillMaxSize()) {
        if (loading) {
            LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
        }
        LazyColumn(state = listState, modifier = Modifier.fillMaxSize()) {
            itemsIndexed((0 until helper.pageCount).toList(), key = { i, _ -> i }) { index, _ ->
                var scale by remember(index) { mutableFloatStateOf(1f) }
                val transformState =
                    rememberTransformableState { zoom, _, _ ->
                        scale = (scale * zoom).coerceIn(1f, 4f)
                    }
                LaunchedEffect(index, widthPx, scale) {
                    loading = true
                    val bmp =
                        withContext(Dispatchers.Default) {
                            helper.renderPage(index, widthPx, scale)
                        }
                    bitmaps[index] = bmp
                    loading = false
                }
                val bmp = bitmaps[index]
                Box(
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .padding(vertical = 8.dp)
                            .transformable(transformState),
                    contentAlignment = Alignment.Center,
                ) {
                    if (bmp != null) {
                        Image(
                            bitmap = bmp.asImageBitmap(),
                            contentDescription = null,
                            modifier = Modifier.fillMaxWidth(),
                            contentScale = ContentScale.FillWidth,
                        )
                    } else {
                        Text(
                            stringResource(R.string.working),
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
            }
        }
    }
}