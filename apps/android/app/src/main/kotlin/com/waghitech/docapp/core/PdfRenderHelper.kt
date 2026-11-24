package com.waghitech.docapp.core

import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.Matrix
import android.graphics.pdf.PdfRenderer
import android.os.ParcelFileDescriptor
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import java.io.File
import java.util.LinkedHashMap

class PdfRenderHelper(
    file: File,
    cacheMaxBytes: Int,
) : AutoCloseable {
    private val pfd = ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
    private val renderer = PdfRenderer(pfd)
    private val mutex = Mutex()
  private val cache = object : LinkedHashMap<Int, Bitmap>(16, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Int, Bitmap>?): Boolean {
            var size = 0
            for (b in values) {
                size += b.byteCount
            }
            return size > cacheMaxBytes
        }
    }

    val pageCount: Int
        get() = renderer.pageCount

    suspend fun renderPage(index: Int, widthPx: Int, scale: Float): Bitmap = mutex.withLock {
        val key = index + (widthPx * 1000) + (scale * 100).toInt()
        cache[key]?.let { return@withLock it }
        val page = renderer.openPage(index)
        try {
            val baseW = (page.width * scale).toInt().coerceAtLeast(1)
            val baseH = (page.height * scale).toInt().coerceAtLeast(1)
            val w = widthPx.coerceAtMost(baseW).coerceAtLeast(1)
            val h = (baseH * (w.toFloat() / baseW.toFloat())).toInt().coerceAtLeast(1)
            val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
            bmp.eraseColor(Color.WHITE)
            val matrix = Matrix()
            val sx = w.toFloat() / page.width.toFloat()
            val sy = h.toFloat() / page.height.toFloat()
            matrix.setScale(sx, sy)
            page.render(bmp, null, matrix, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY)
            cache[key] = bmp
            bmp
        } finally {
            page.close()
        }
    }

    override fun close() {
        for (b in cache.values) {
            b.recycle()
        }
        cache.clear()
        renderer.close()
        pfd.close()
    }
}