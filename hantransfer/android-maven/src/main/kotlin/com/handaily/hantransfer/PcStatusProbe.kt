package com.handaily.hantransfer

import android.util.Log
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import java.util.concurrent.TimeUnit

object PcStatusProbe {
    private const val TAG = "HantransferProbe"

    private val http = OkHttpClient.Builder()
        .connectTimeout(4, TimeUnit.SECONDS)
        .readTimeout(4, TimeUnit.SECONDS)
        .build()

    fun probe(host: String, port: Int): DiscoveredDevice? {
        val trimmedHost = host.trim()
        if (trimmedHost.isEmpty() || port <= 0) return null
        val base = "http://${formatHost(trimmedHost)}:$port"
        val request = Request.Builder()
            .url("$base/api/v1/status")
            .get()
            .build()
        return runCatching {
            http.newCall(request).execute().use { resp ->
                if (!resp.isSuccessful) return@use null
                val raw = resp.body?.string().orEmpty()
                parseStatus(trimmedHost, port, raw)
            }
        }.getOrElse {
            Log.d(TAG, "probe failed for $trimmedHost:$port", it)
            null
        }
    }

    private fun parseStatus(fallbackHost: String, port: Int, raw: String): DiscoveredDevice? {
        val root = JSONObject(raw)
        if (!root.optBoolean("ok", false)) return null
        val data = root.optJSONObject("data") ?: return null
        val connectHost = data.optString("lan_ip").trim().takeIf { it.isNotEmpty() }
            ?: fallbackHost
        val name = data.optString("name").trim().takeIf { it.isNotEmpty() } ?: connectHost
        val deviceId = data.optString("device_id").trim()
        val serviceName = deviceId.ifEmpty { "$connectHost:$port" }
        val featuresRaw = data.optString("features").trim()
        val features = if (featuresRaw.isEmpty()) {
            emptyList()
        } else {
            featuresRaw.split(',').map { it.trim() }.filter { it.isNotEmpty() }
        }
        return DiscoveredDevice(
            serviceName = serviceName,
            name = name,
            platform = data.optString("platform").ifEmpty { "windows" },
            version = data.optString("version").ifEmpty { "?" },
            deviceId = deviceId,
            features = features,
            host = connectHost,
            port = data.optInt("port", port).takeIf { it > 0 } ?: port,
        )
    }

    private fun formatHost(host: String): String {
        return if (host.contains(':') && !host.startsWith('[')) "[$host]" else host
    }
}
