package com.waghitech.docapp

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.waghitech.docapp.core.CacheSizer
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class CacheSizerTest {
    @Test
    fun cache_bytes_positive() {
        val context: Context = ApplicationProvider.getApplicationContext()
        assertTrue(CacheSizer.bitmapCacheMaxBytes(context) >= 4 * 1024 * 1024)
    }
}