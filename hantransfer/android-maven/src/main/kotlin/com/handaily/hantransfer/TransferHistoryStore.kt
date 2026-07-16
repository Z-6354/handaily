package com.handaily.hantransfer

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject

class TransferHistoryStore(context: Context) {
    private val prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun list(): List<TransferHistoryItem> = runCatching {
        val arr = JSONArray(prefs.getString(KEY_ITEMS, "[]") ?: "[]")
        buildList {
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                add(
                    TransferHistoryItem(
                        filename = o.optString("filename", "file"),
                        deviceName = o.optString("deviceName", ""),
                        path = o.optString("path", ""),
                        at = o.optLong("at", 0L),
                        type = o.optString("type", "file"),
                    ),
                )
            }
        }.sortedByDescending { it.at }
    }.getOrElse { emptyList() }

    fun append(item: TransferHistoryItem) {
        val current = JSONArray(prefs.getString(KEY_ITEMS, "[]") ?: "[]")
        val obj = JSONObject()
            .put("filename", item.filename)
            .put("deviceName", item.deviceName)
            .put("path", item.path)
            .put("at", item.at)
            .put("type", item.type)
        current.put(obj)
        val trimmed = JSONArray()
        val start = (current.length() - MAX_ITEMS).coerceAtLeast(0)
        for (i in start until current.length()) trimmed.put(current.getJSONObject(i))
        prefs.edit().putString(KEY_ITEMS, trimmed.toString()).apply()
    }

    companion object {
        private const val PREFS = "hantransfer_history"
        private const val KEY_ITEMS = "items"
        private const val MAX_ITEMS = 200
    }
}
