package com.waghitech.docapp.core

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject

data class RecentEntry(
    val uri: String,
    val name: String,
    val mime: String,
    val openedAt: Long,
)

class RecentStore(context: Context) {
    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun list(): List<RecentEntry> {
        val raw = prefs.getString(KEY, null) ?: return emptyList()
        val arr = JSONArray(raw)
        val out = mutableListOf<RecentEntry>()
        for (i in 0 until arr.length()) {
            val obj = arr.getJSONObject(i)
            out.add(
                RecentEntry(
                    uri = obj.getString("uri"),
                    name = obj.getString("name"),
                    mime = obj.getString("mime"),
                    openedAt = obj.getLong("openedAt"),
                ),
            )
        }
        return out.sortedByDescending { it.openedAt }
    }

    fun add(entry: RecentEntry) {
        val current = list().filter { it.uri != entry.uri }.toMutableList()
        current.add(0, entry)
        val trimmed = current.take(MAX)
        val arr = JSONArray()
        for (e in trimmed) {
            arr.put(
                JSONObject()
                    .put("uri", e.uri)
                    .put("name", e.name)
                    .put("mime", e.mime)
                    .put("openedAt", e.openedAt),
            )
        }
        prefs.edit().putString(KEY, arr.toString()).apply()
    }

    companion object {
        private const val PREFS = "docapp_recent"
        private const val KEY = "items"
        private const val MAX = 20
    }
}