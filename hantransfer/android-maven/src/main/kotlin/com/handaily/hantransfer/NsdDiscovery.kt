package com.handaily.hantransfer

import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import java.util.concurrent.Executor

class NsdDiscovery(
    context: Context,
    private val onUpdate: () -> Unit,
) {
    private val appContext = context.applicationContext
    private val mainHandler = Handler(Looper.getMainLooper())
    private val mainExecutor = Executor { runnable -> mainHandler.post(runnable) }
    private val nsdManager = appContext.getSystemService(Context.NSD_SERVICE) as? NsdManager
    private val wifiManager = appContext.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager

    private var multicastLock: WifiManager.MulticastLock? = null
    private var discoveryListener: NsdManager.DiscoveryListener? = null
    private var discoveryActive = false
    private var retryCount = 0
    private val pendingResolve = mutableSetOf<String>()
    private val resolved = linkedMapOf<String, DiscoveredDevice>()
    private val serviceCallbacks = linkedMapOf<String, NsdManager.ServiceInfoCallback>()

    fun devices(): List<DiscoveredDevice> = resolved.values.sortedBy { it.name }

    fun isActive(): Boolean = discoveryActive

    fun start(clearExisting: Boolean = false) {
        mainHandler.post { startOnMainThread(clearExisting) }
    }

    fun stop() {
        mainHandler.post { stopOnMainThread() }
    }

    private fun startOnMainThread(clearExisting: Boolean) {
        if (discoveryActive && !clearExisting) return

        val nsd = nsdManager
        if (nsd == null) {
            Log.w(TAG, "NsdManager unavailable")
            emitUpdate()
            return
        }

        if (clearExisting) {
            resolved.clear()
            pendingResolve.clear()
            emitUpdate()
        }

        if (discoveryActive) {
            stopOnMainThread()
        }

        acquireMulticastLock()

        val listener = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(serviceType: String) {
                discoveryActive = true
                retryCount = 0
                Log.d(TAG, "discovery started: $serviceType")
            }
            override fun onServiceFound(info: NsdServiceInfo) {
                val key = info.serviceName
                if (key.isNullOrBlank()) return
                if (pendingResolve.contains(key) || resolved.containsKey(key)) return
                pendingResolve.add(key)
                Log.d(TAG, "service found: $key type=${info.serviceType}")
                resolveService(nsd, info, key)
            }
            override fun onServiceLost(info: NsdServiceInfo) {
                resolved.remove(info.serviceName)
                pendingResolve.remove(info.serviceName)
                unregisterCallback(info.serviceName)
                emitUpdate()
            }
            override fun onDiscoveryStopped(serviceType: String) {
                discoveryActive = false
            }
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                discoveryActive = false
                Log.w(TAG, "onStartDiscoveryFailed: $errorCode")
                scheduleRetry(clearExisting)
            }
            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) = Unit
        }
        discoveryListener = listener
        runCatching {
            nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, listener)
        }.onFailure {
            discoveryActive = false
            Log.w(TAG, "discoverServices failed", it)
            scheduleRetry(clearExisting)
        }
    }

    @Suppress("DEPRECATION")
    private fun resolveService(nsd: NsdManager, info: NsdServiceInfo, serviceName: String) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            val callback = object : NsdManager.ServiceInfoCallback {
                override fun onServiceUpdated(serviceInfo: NsdServiceInfo) {
                    pendingResolve.remove(serviceName)
                    unregisterCallback(serviceName)
                    handleResolved(serviceInfo)
                }
                override fun onServiceLost() {
                    pendingResolve.remove(serviceName)
                    unregisterCallback(serviceName)
                    resolved.remove(serviceName)
                    emitUpdate()
                }
                override fun onServiceInfoCallbackRegistrationFailed(errorCode: Int) {
                    Log.w(TAG, "ServiceInfoCallback failed for $serviceName: $errorCode")
                    pendingResolve.remove(serviceName)
                    unregisterCallback(serviceName)
                }
                override fun onServiceInfoCallbackUnregistered() {
                    serviceCallbacks.remove(serviceName)
                }
            }
            serviceCallbacks[serviceName] = callback
            runCatching {
                nsd.registerServiceInfoCallback(info, mainExecutor, callback)
            }.onFailure {
                serviceCallbacks.remove(serviceName)
                pendingResolve.remove(serviceName)
                Log.w(TAG, "registerServiceInfoCallback failed for $serviceName", it)
            }
            return
        }

        runCatching {
            nsd.resolveService(info, createResolveListener(serviceName))
        }.onFailure {
            pendingResolve.remove(serviceName)
            Log.w(TAG, "resolveService failed for $serviceName", it)
        }
    }

    private fun unregisterCallback(serviceName: String?) {
        if (serviceName.isNullOrBlank()) return
        val nsd = nsdManager ?: return
        val callback = serviceCallbacks.remove(serviceName) ?: return
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            runCatching { nsd.unregisterServiceInfoCallback(callback) }
        }
    }

    private fun scheduleRetry(clearExisting: Boolean) {
        if (retryCount >= 5) {
            emitUpdate()
            return
        }
        retryCount += 1
        val delayMs = 1500L * retryCount
        mainHandler.postDelayed({ startOnMainThread(clearExisting) }, delayMs)
    }

    private fun stopOnMainThread() {
        val nsd = nsdManager ?: return
        serviceCallbacks.keys.toList().forEach { unregisterCallback(it) }
        serviceCallbacks.clear()
        discoveryListener?.let { runCatching { nsd.stopServiceDiscovery(it) } }
        discoveryListener = null
        discoveryActive = false
        releaseMulticastLock()
    }

    private fun createResolveListener(serviceName: String) = object : NsdManager.ResolveListener {
        override fun onResolveFailed(info: NsdServiceInfo, errorCode: Int) {
            Log.w(TAG, "onResolveFailed for $serviceName: $errorCode")
            pendingResolve.remove(serviceName)
        }
        override fun onServiceResolved(info: NsdServiceInfo) {
            handleResolved(info)
        }
    }

    private fun handleResolved(info: NsdServiceInfo) {
        val serviceName = info.serviceName
        pendingResolve.remove(serviceName)
        val port = info.port
        if (port <= 0) return
        val attrs = info.attributes ?: emptyMap()
        val host = NetworkAddresses.pickReachableHost(info, attrs)
        if (host == null) {
            Log.w(TAG, "no reachable host for $serviceName")
            return
        }
        Log.i(TAG, "resolved $serviceName -> $host:$port")
        resolved[serviceName] = DiscoveredDevice(
            serviceName = serviceName,
            name = NetworkAddresses.decodeTxt(attrs, "name") ?: serviceName,
            platform = NetworkAddresses.decodeTxt(attrs, "platform") ?: "unknown",
            version = NetworkAddresses.decodeTxt(attrs, "version") ?: "?",
            deviceId = NetworkAddresses.decodeTxt(attrs, "device_id") ?: "",
            features = NetworkAddresses.decodeTxt(attrs, "features")
                ?.split(',')?.map { it.trim() }?.filter { it.isNotEmpty() }
                ?: emptyList(),
            host = host,
            port = port,
        )
        emitUpdate()
    }

    private fun emitUpdate() {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            onUpdate()
        } else {
            mainHandler.post { onUpdate() }
        }
    }

    private fun acquireMulticastLock() {
        val wifi = wifiManager ?: return
        runCatching {
            multicastLock = wifi.createMulticastLock("hantransfer-mdns").apply {
                setReferenceCounted(true)
                acquire()
            }
        }.onFailure {
            Log.w(TAG, "multicast lock failed", it)
        }
    }

    private fun releaseMulticastLock() {
        multicastLock?.let {
            runCatching { if (it.isHeld) it.release() }
        }
        multicastLock = null
    }

    companion object {
        private const val TAG = "HantransferNsd"
        const val SERVICE_TYPE = "_hantransfer._tcp."
    }
}
