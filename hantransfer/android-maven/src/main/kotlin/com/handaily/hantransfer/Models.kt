package com.handaily.hantransfer

data class DiscoveredDevice(
    val serviceName: String,
    val name: String,
    val platform: String,
    val version: String,
    val deviceId: String,
    val features: List<String>,
    val host: String,
    val port: Int,
) {
    val baseUrl: String
        get() {
            val h = if (host.contains(':') && !host.startsWith('[')) "[$host]" else host
            return "http://$h:$port"
        }
}

enum class HandshakeStatus { TRUSTED, PENDING, REJECTED, ERROR }

data class TransferHistoryItem(
    val filename: String,
    val deviceName: String,
    val path: String,
    val at: Long,
    val type: String,
)

data class PushItem(
    val pushId: String,
    val filename: String,
    val size: Long,
    val source: String,
)
