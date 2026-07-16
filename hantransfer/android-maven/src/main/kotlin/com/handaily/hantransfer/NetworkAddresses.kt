package com.handaily.hantransfer

import android.net.nsd.NsdServiceInfo
import android.util.Log

internal object NetworkAddresses {
    private const val TAG = "HantransferNsd"

    fun pickReachableHost(info: NsdServiceInfo, attrs: Map<String, ByteArray>): String? {
        decodeTxt(attrs, "lan_ip")?.let { lan ->
            if (isPrivateIpv4(lan)) return lan
        }
        val resolved = info.host?.hostAddress ?: return null
        if (isPrivateIpv4(resolved)) return resolved
        if (isPrivateIpv6(resolved)) return resolved
        if (resolved.contains(':')) {
            Log.w(TAG, "Using global IPv6 $resolved; prefer PC restart to publish lan_ip")
            return resolved
        }
        return resolved
    }

    fun decodeTxt(attrs: Map<String, ByteArray>, key: String): String? {
        return attrs[key]?.toString(Charsets.UTF_8)?.trim()?.takeIf { it.isNotEmpty() }
    }

    fun isPrivateIpv4(host: String): Boolean {
        if (host.startsWith("192.168.") || host.startsWith("10.")) return true
        if (!host.startsWith("172.")) return false
        val second = host.split('.').getOrNull(1)?.toIntOrNull() ?: return false
        return second in 16..31
    }

    fun isPrivateIpv6(host: String): Boolean {
        val h = host.lowercase()
        return h.startsWith("fe80:") || h.startsWith("fc") || h.startsWith("fd")
    }
}
