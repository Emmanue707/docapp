package com.waghitech.docapp

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.waghitech.docapp.core.RecentEntry
import com.waghitech.docapp.core.RecentStore
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class RecentStoreTest {
    private val context: Context = ApplicationProvider.getApplicationContext()

    @Test
    fun stores_and_trims_recents() {
        val store = RecentStore(context)
        val prefs = context.getSharedPreferences("docapp_recent", Context.MODE_PRIVATE)
        prefs.edit().clear().apply()
        val e1 =
            RecentEntry("content://a", "a.pdf", "application/pdf", 100)
        val e2 =
            RecentEntry("content://b", "b.pdf", "application/pdf", 200)
        store.add(e1)
        store.add(e2)
        val list = store.list()
        assertEquals(2, list.size)
        assertEquals("b.pdf", list[0].name)
    }
}