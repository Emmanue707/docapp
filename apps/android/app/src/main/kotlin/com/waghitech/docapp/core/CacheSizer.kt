package com.waghitech.docapp.core

import android.app.ActivityManager
import android.content.Context

object CacheSizer {
    fun bitmapCacheMaxBytes(context: Context): Int {
        val am = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        val memoryClassKb = am.memoryClass
        return (memoryClassKb * 1024L / 8).toInt().coerceAtLeast(4 * 1024 * 1024)
    }
}