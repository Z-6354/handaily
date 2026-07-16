package com.handaily.hantransfer

import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import java.util.concurrent.TimeUnit

internal object PcEndpoint {
    private val probeHttp = OkHttpClient.Builder()
        .connectTimeout(5, TimeUnit.SECONDS)
        .readTimeout(8, TimeUnit.SECONDS)
        .build()

    fun resolveApiBase(device: DiscoveredDevice): String {
        val statusUrl = "${device.baseUrl}/api/v1/status"
        return runCatching {
            probeHttp.newCall(Request.Builder().url(statusUrl).get().build()).execute().use { resp ->
                if (!resp.isSuccessful) return@use device.baseUrl
                val data = parseApiData(resp.body?.string().orEmpty())
                val lanIp = data.optString("lan_ip").trim().takeIf { it.isNotEmpty() }
                val port = data.optInt("port", device.port).takeIf { it > 0 } ?: device.port
                if (lanIp != null) formatBaseUrl(lanIp, port) else device.baseUrl
            }
        }.getOrDefault(device.baseUrl)
    }

    fun formatBaseUrl(host: String, port: Int): String {
        val h = if (host.contains(':') && !host.startsWith('[')) "[$host]" else host
        return "http://$h:$port"
    }

    private fun parseApiData(text: String): JSONObject {
        val root = JSONObject(text)
        return root.optJSONObject("data") ?: root
    }
}
