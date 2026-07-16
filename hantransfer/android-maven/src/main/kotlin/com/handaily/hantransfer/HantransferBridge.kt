package com.handaily.hantransfer

import android.net.Uri
import android.os.Build
import android.content.pm.PackageManager
import android.webkit.JavascriptInterface
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.concurrent.Executors

class HantransferBridge(
    private val activity: MainActivity,
    private val runJs: (String) -> Unit,
) {
    /** Light IO: handshake, probe, browse, push-list, update check. */
    private val ioExecutor = Executors.newFixedThreadPool(3)

    /** Heavy transfer: upload / download — isolated so IO polls stay responsive. */
    private val transferExecutor = Executors.newFixedThreadPool(2)

    private val transfer = TransferClient(activity)
    private val history = TransferHistoryStore(activity)
    private val saf = SafFolderStore(activity)
    private val discovery = NsdDiscovery(activity) { notifyDevices() }
    private val httpDiscovered = linkedMapOf<String, DiscoveredDevice>()
    private val uploadSession = UploadSession()

    @Volatile
    private var selectedHost: String? = loadSavedHost(activity)
    @Volatile
    private var selectedPort: Int = loadSavedPort(activity)
    @Volatile
    private var selectedServiceName: String? = loadSavedServiceName(activity)

    fun onDestroy() {
        discovery.stop()
        ioExecutor.shutdownNow()
        transferExecutor.shutdownNow()
    }

    @Volatile
    private var lastAzLocateAtMs: Long = 0L

    @Volatile
    private var connecting = false

    @Volatile
    private var trustCheckInFlight = false

    @Volatile
    private var lastTrustCheckAtMs: Long = 0L

    @Volatile
    private var lastTrustJsStatus: String? = null

    @Volatile
    private var pushRefreshInFlight = false

    @Volatile
    private var cachedHandshakeKey: String? = null

    @Volatile
    private var cachedHandshakeStatus: HandshakeStatus? = null

    @Volatile
    private var cachedHandshakeAtMs: Long = 0L

    @Volatile
    private var lastPushPendingJson: String = "[]"

    fun onResume() {
        if (isFileOriginUrl()) {
            startDiscovery()
        }
        // Avoid stacking probes on top of autoConnect during cold start.
        if (!connecting) {
            ioExecutor.execute { probeSavedPcInternal(notify = true) }
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && StorageAccess.hasAllFilesAccess(activity)) {
            val now = System.currentTimeMillis()
            if (now - lastAzLocateAtMs > 60_000L) {
                lastAzLocateAtMs = now
                ioExecutor.execute { locateAzFolder(showToast = false) }
            }
        }
    }

    private fun handshakeStatus(device: DiscoveredDevice, force: Boolean = false): HandshakeStatus {
        val key = "${device.host}:${device.port}"
        val now = System.currentTimeMillis()
        if (!force) {
            val cached = cachedHandshakeStatus
            if (cached != null && cachedHandshakeKey == key && now - cachedHandshakeAtMs < 5_000L) {
                return cached
            }
        }
        val status = transfer.handshakeOnce(device)
        cachedHandshakeKey = key
        cachedHandshakeStatus = status
        cachedHandshakeAtMs = now
        return status
    }

    fun onStartup() {
        // Connection and page hooks run after WebView onPageFinished (__markNativeApp).
    }

    fun startDiscovery(clearExisting: Boolean = false) {
        if (clearExisting) {
            httpDiscovered.clear()
        }
        activity.runOnUiThread {
            discovery.start(clearExisting)
        }
    }

    @JavascriptInterface
    fun startDiscoveryFromJs() {
        startDiscovery(clearExisting = false)
    }

    @JavascriptInterface
    fun rescanDiscoveryFromJs() {
        startDiscovery(clearExisting = true)
        ioExecutor.execute {
            probeSavedPcInternal(notify = true)
        }
    }

    @JavascriptInterface
    fun probeSavedPcAsync() {
        ioExecutor.execute {
            probeSavedPcInternal(notify = true)
        }
    }

    @JavascriptInterface
    fun pauseSendAsync() {
        uploadSession.requestPause()
    }

    @JavascriptInterface
    fun connectToPcAsync(host: String, port: Int) {
        ioExecutor.execute {
            connectToHostInternal(host.trim(), port)
        }
    }

    @JavascriptInterface
    fun autoConnectSavedAsync() {
        ioExecutor.execute { autoConnectSavedInternal() }
    }

    @JavascriptInterface
    fun getDevicesJson(): String = devicesJson()

    @JavascriptInterface
    fun setSelectedDevice(host: String, port: Int, serviceName: String?) {
        val changed = host != selectedHost || port != selectedPort
        selectedHost = host
        selectedPort = port
        selectedServiceName = serviceName?.takeIf { it.isNotBlank() }
        saveSelectedDevice(host, port, selectedServiceName)
        if (changed) {
            lastTrustJsStatus = null
            cachedHandshakeKey = null
            cachedHandshakeStatus = null
            cachedHandshakeAtMs = 0L
        }
    }

    @JavascriptInterface
    fun syncSelectedDevice(host: String, port: Int, serviceName: String?) {
        setSelectedDevice(host, port, serviceName)
    }

    @JavascriptInterface
    fun getHistoryJson(): String = runCatching {
        val arr = JSONArray()
        history.list().forEach { h ->
            arr.put(
                JSONObject()
                    .put("filename", h.filename)
                    .put("deviceName", h.deviceName)
                    .put("path", h.path)
                    .put("at", h.at)
                    .put("type", h.type),
            )
        }
        arr.toString()
    }.getOrElse { "[]" }

    @JavascriptInterface
    fun appendHistory(json: String) {
        val o = JSONObject(json)
        history.append(
            TransferHistoryItem(
                filename = o.getString("filename"),
                deviceName = o.getString("deviceName"),
                path = o.optString("path", ""),
                at = o.optLong("at", System.currentTimeMillis()),
                type = o.optString("type", "file"),
            ),
        )
    }

    @JavascriptInterface
    fun pickAndSendFile() {
        activity.runOnUiThread {
            runJs("typeof openNativeBrowser === 'function' && openNativeBrowser()")
        }
    }

    /** Sync bridge must not block WebView — kick async and return a loading stub. */
    @JavascriptInterface
    fun browseDirJson(path: String?): String {
        browseDirAsync(path)
        return JSONObject()
            .put("ok", true)
            .put("loading", true)
            .put("path", path ?: "")
            .put("entries", JSONArray())
            .toString()
    }

    @JavascriptInterface
    fun browseDirAsync(path: String?) {
        ioExecutor.execute {
            val payload = StorageAccess.browseDir(activity, path, null).toString()
            runJs("onBrowseDirResult($payload)")
        }
    }

    @JavascriptInterface
    fun sendFilesByPathJson(pathsJson: String) {
        val device = selectedDevice() ?: run {
            progressJs("file", 0, 0, true, "未选择电脑")
            return
        }
        val paths = runCatching {
            val arr = JSONArray(pathsJson)
            buildList {
                for (i in 0 until arr.length()) add(arr.getString(i))
            }
        }.getOrElse {
            progressJs("file", 0, 0, true, "路径无效")
            return
        }
        if (paths.isEmpty()) {
            progressJs("file", 0, 0, true, "未选择文件")
            return
        }
        transferExecutor.execute {
            beginUploadBatch()
            var ok = 0
            var skipped = 0
            var fail = 0
            try {
                if (!guardTrustedForSend(device, SendChannel.FILE)) return@execute
                for ((index, rawPath) in paths.withIndex()) {
                    if (uploadSession.shouldStop()) {
                        finishUploadPaused(ok + skipped, paths.size)
                        return@execute
                    }
                    val file = File(rawPath)
                    if (!StorageAccess.isPathAllowed(file)) {
                        fail++
                        continue
                    }
                    try {
                        val readable = StorageAccess.resolveReadableFile(file)
                        val raw = if (readable.isFile && runCatching { readable.canRead() }.getOrDefault(false)) {
                            transfer.uploadFile(readable, file.name, device) { sent, total ->
                                progressJs(file.name, sent, total, false, null, index + 1, paths.size)
                            }
                        } else {
                            val docId = StorageAccess.fileToDocumentId(file)
                                ?.takeIf { StorageAccess.documentExists(activity, it) }
                            if (docId == null) {
                                fail++
                                continue
                            }
                            transfer.uploadUri(
                                StorageAccess.documentUri(docId),
                                device,
                            ) { sent, total ->
                                progressJs(file.name, sent, total, false, null, index + 1, paths.size)
                            }
                        }
                        val result = parseUploadResult(raw)
                        history.append(
                            TransferHistoryItem(file.name, device.name, result.path, System.currentTimeMillis(), "file"),
                        )
                        if (result.skipped) skipped++ else ok++
                    } catch (e: TransferPausedException) {
                        throw e
                    } catch (e: Exception) {
                        fail++
                        progressJs(file.name, 0, 0, false, e.message, index + 1, paths.size)
                    }
                }
                if (ok + skipped == 0) {
                    progressJs("file", 0, 0, true, if (fail > 0) "全部失败（$fail）" else "无法读取所选文件，请检查权限或路径")
                    return@execute
                }
                if (uploadSession.shouldStop()) {
                    finishUploadPaused(ok + skipped, paths.size)
                    return@execute
                }
                val summary = batchUploadSummary(ok, skipped, fail, paths.size)
                val err = if (fail > 0 && ok + skipped == 0) summary else null
                progressJs(summary, 1, 1, true, err, ok + skipped, paths.size)
            } catch (e: TransferPausedException) {
                finishUploadPaused(ok + skipped, paths.size, e.message)
            } catch (e: Exception) {
                progressJs("file", 0, 0, true, e.message)
            } finally {
                endUploadBatch()
            }
        }
    }

    @JavascriptInterface
    fun ensureTrustedAsync() {
        if (connecting) return
        val now = System.currentTimeMillis()
        if (trustCheckInFlight) return
        if (now - lastTrustCheckAtMs < 2_000L) return
        val device = selectedDevice() ?: run {
            trustJs("error", "未选择电脑")
            return
        }
        trustCheckInFlight = true
        lastTrustCheckAtMs = now
        ioExecutor.execute {
            try {
                when (handshakeStatus(device)) {
                    HandshakeStatus.TRUSTED -> trustJs("trusted")
                    HandshakeStatus.PENDING -> trustJs("pending", "等待电脑确认信任…")
                    HandshakeStatus.REJECTED -> trustJs("rejected", "电脑已拒绝此设备")
                    HandshakeStatus.ERROR -> trustJs("error", "无法连接电脑")
                }
            } finally {
                trustCheckInFlight = false
            }
        }
    }

    /**
     * Non-blocking trust snapshot for settings UI.
     * Uses recent handshake cache only — never does network on the JS bridge thread.
     */
    @JavascriptInterface
    fun probeTrustJson(): String {
        val device = selectedDevice()
            ?: return JSONObject()
                .put("trusted", false)
                .put("selected", false)
                .toString()
        val key = "${device.host}:${device.port}"
        val cached = cachedHandshakeStatus
        val fresh = cached != null && cachedHandshakeKey == key &&
            System.currentTimeMillis() - cachedHandshakeAtMs < 30_000L
        return when {
            fresh && cached == HandshakeStatus.TRUSTED -> JSONObject()
                .put("trusted", true)
                .put("selected", true)
                .put("host", device.host)
                .put("port", device.port)
                .toString()
            fresh && cached == HandshakeStatus.PENDING -> JSONObject()
                .put("trusted", false)
                .put("selected", true)
                .put("pending", true)
                .put("message", "等待电脑确认信任…")
                .toString()
            fresh && cached == HandshakeStatus.REJECTED -> JSONObject()
                .put("trusted", false)
                .put("selected", true)
                .put("message", "电脑已拒绝此设备")
                .toString()
            else -> JSONObject()
                .put("trusted", false)
                .put("selected", true)
                .put("pending", true)
                .put("message", "正在确认连接…")
                .toString()
        }
    }

    private fun trustJs(status: String, message: String? = null) {
        // Avoid flooding WebView with identical pending/trusted callbacks from poll storms.
        if (status == lastTrustJsStatus && (status == "trusted" || status == "pending")) {
            return
        }
        lastTrustJsStatus = status
        val payload = JSONObject().put("status", status)
        if (message != null) payload.put("message", message)
        runJs("onNativeTrustPayload($payload)")
    }

    /** Legacy sync entry — returns last cached list and refreshes asynchronously. */
    @JavascriptInterface
    fun pollPushPendingJson(): String {
        schedulePushRefresh(notify = false)
        return lastPushPendingJson
    }

    @JavascriptInterface
    fun pollPushPendingAsync() {
        schedulePushRefresh(notify = true)
    }

    private fun schedulePushRefresh(notify: Boolean) {
        if (pushRefreshInFlight || connecting) return
        pushRefreshInFlight = true
        ioExecutor.execute {
            try {
                refreshPushPendingInternal(notify = notify)
            } finally {
                pushRefreshInFlight = false
            }
        }
    }

    private fun refreshPushPendingInternal(notify: Boolean) {
        val device = selectedDevice()
        val json = if (device == null) {
            "[]"
        } else {
            runCatching {
                if (handshakeStatus(device) != HandshakeStatus.TRUSTED) return@runCatching "[]"
                val arr = JSONArray()
                transfer.listPushPending(device).forEach { item ->
                    arr.put(
                        JSONObject()
                            .put("push_id", item.pushId)
                            .put("filename", item.filename)
                            .put("size", item.size)
                            .put("source", item.source),
                    )
                }
                arr.toString()
            }.getOrElse { "[]" }
        }
        lastPushPendingJson = json
        if (notify) {
            dispatchJsRaw("onPushPendingResult", json)
        }
    }

    @JavascriptInterface
    fun downloadPush(pushId: String) {
        val device = selectedDevice() ?: run {
            receiveJs(name = "未选择电脑", batchDone = true, error = "未选择电脑")
            return
        }
        transferExecutor.execute {
            val pending = transfer.listPushPending(device)
            val item = pending.find { it.pushId == pushId }
            if (item == null) {
                receiveJs(name = "推送任务不存在", batchDone = true, error = "推送任务不存在")
                return@execute
            }
            downloadPushItems(device, listOf(item))
        }
    }

    @JavascriptInterface
    fun downloadPushBatch(pushIdsJson: String) {
        val device = selectedDevice() ?: run {
            receiveJs(name = "未选择电脑", batchDone = true, error = "未选择电脑")
            return
        }
        val ids = runCatching {
            val arr = JSONArray(pushIdsJson)
            buildSet {
                for (i in 0 until arr.length()) add(arr.getString(i))
            }
        }.getOrElse {
            receiveJs(name = "参数无效", batchDone = true, error = "参数无效")
            return
        }
        transferExecutor.execute {
            val pending = transfer.listPushPending(device).filter { it.pushId in ids }
            downloadPushItems(device, pending)
        }
    }

    @JavascriptInterface
    fun downloadAllPush() {
        val device = selectedDevice() ?: run {
            receiveJs(name = "未选择电脑", batchDone = true, error = "未选择电脑")
            return
        }
        transferExecutor.execute {
            val pending = transfer.listPushPending(device)
            downloadPushItems(device, pending)
        }
    }

    private fun downloadPushItems(device: DiscoveredDevice, items: List<PushItem>) {
        if (items.isEmpty()) {
            receiveJs(name = "暂无待接收文件", batchDone = true, batchTotal = 0)
            return
        }
        when (handshakeStatus(device)) {
            HandshakeStatus.TRUSTED -> Unit
            HandshakeStatus.PENDING -> {
                receiveJs(name = "等待电脑确认信任", batchDone = true, error = "等待电脑确认信任")
                return
            }
            HandshakeStatus.REJECTED -> {
                receiveJs(name = "电脑已拒绝连接", batchDone = true, error = "电脑已拒绝连接")
                return
            }
            HandshakeStatus.ERROR -> {
                receiveJs(name = "连接失败", batchDone = true, error = "无法连接电脑")
                return
            }
        }
        val totalBytes = items.sumOf { it.size.coerceAtLeast(0L) }.coerceAtLeast(1L)
        var completedBytes = 0L
        var ok = 0
        var fail = 0
        for ((index, item) in items.withIndex()) {
            try {
                val file = transfer.downloadPush(device, item) { sent, _ ->
                    receiveJs(
                        name = item.filename,
                        progress = (completedBytes + sent) to totalBytes,
                        index = index + 1,
                        batchTotal = items.size,
                        pushId = item.pushId,
                    )
                }
                completedBytes += item.size.coerceAtLeast(0L)
                history.append(
                    TransferHistoryItem(
                        item.filename,
                        item.source,
                        file.absolutePath,
                        System.currentTimeMillis(),
                        "push",
                    ),
                )
                receiveJs(
                    name = item.filename,
                    fileDone = true,
                    pushId = item.pushId,
                    index = index + 1,
                    batchTotal = items.size,
                )
                ok++
            } catch (e: Exception) {
                completedBytes += item.size.coerceAtLeast(0L)
                receiveJs(
                    name = item.filename,
                    fileDone = true,
                    error = e.message ?: "下载失败",
                    pushId = item.pushId,
                    index = index + 1,
                    batchTotal = items.size,
                )
                fail++
            }
        }
        val summary = when {
            fail > 0 && ok > 0 -> "完成 $ok/${items.size}，$fail 个失败"
            fail > 0 -> "全部失败"
            else -> "已接收 $ok 个文件"
        }
        receiveJs(
            name = summary,
            batchDone = true,
            batchTotal = items.size,
            successCount = ok,
            failCount = fail,
        )
    }

    @JavascriptInterface
    fun getAppInfoJson(): String = JSONObject()
        .put("version", runCatching { activity.getString(R.string.app_version_name) }.getOrDefault("0.1.0"))
        .put("build", runCatching { activity.getString(R.string.app_version_code) }.getOrDefault("0"))
        .put("display", runCatching { activity.getString(R.string.app_version_display) }.getOrDefault("0.1.0"))
        .put("native", true)
        .toString()

    @JavascriptInterface
    fun getStorageAccessJson(): String = JSONObject()
        .put("needed", StorageAccess.needsAllFilesAccess())
        .put("granted", StorageAccess.hasAllFilesAccess(activity))
        .toString()

    @JavascriptInterface
    fun getStartupStatusJson(): String = JSONObject()
        .put("files_access", StorageAccess.hasAllFilesAccess(activity))
        .put("install_allowed", !ApkUpdateManager.needsInstallPermission(activity))
        .put("az_ready", saf.savedDirectPath() != null || saf.savedTreeUri() != null || saf.savedDocumentId() != null)
        .toString()

    @JavascriptInterface
    fun requestInstallPermission() {
        activity.runOnUiThread { ApkUpdateManager.openInstallPermissionSettings(activity) }
    }

    @JavascriptInterface
    fun clearAppCacheAsync() {
        ioExecutor.execute {
            val payload = ApkUpdateManager.clearCache(activity)
            runJs("onClearCacheResult($payload)")
        }
    }

    @JavascriptInterface
    fun getSavedDeviceJson(): String {
        val host = loadSavedHost(activity) ?: return "{}"
        return JSONObject()
            .put("host", host)
            .put("port", loadSavedPort(activity))
            .put("name", host)
            .put("id", loadSavedServiceName(activity).orEmpty())
            .toString()
    }

    @JavascriptInterface
    fun checkAppUpdateAsync() {
        ioExecutor.execute {
            val payload = runCatching {
                val device = resolveUpdateDevice()
                    ?: return@runCatching JSONObject().put("ok", false).put("error", "未选择电脑")
                ApkUpdateManager.checkUpdate(activity, device)
            }.getOrElse {
                JSONObject().put("ok", false).put("error", it.message ?: "检查失败")
            }
            updateCheckJs(payload)
        }
    }

    /** Sync bridge must not block — schedule async check and return a stub. */
    @JavascriptInterface
    fun checkAppUpdateJson(): String {
        checkAppUpdateAsync()
        return JSONObject()
            .put("ok", true)
            .put("checking", true)
            .put("message", "正在检查更新…")
            .toString()
    }

    @JavascriptInterface
    fun downloadAppUpdate() {
        val device = resolveUpdateDevice() ?: run {
            updateJs(error = "未选择电脑", done = true)
            return
        }
        transferExecutor.execute {
            try {
                val info = ApkUpdateManager.checkUpdate(activity, device)
                if (!info.optBoolean("ok", false)) {
                    updateJs(error = info.optString("error", "检查更新失败"), done = true)
                    return@execute
                }
                if (!info.optBoolean("update_available", false)) {
                    updateJs(message = "已是最新版本", done = true)
                    return@execute
                }
                ApkUpdateManager.downloadAndInstall(
                    activity = activity,
                    device = device,
                    downloadPath = info.optString("download_url"),
                    filename = info.optString("filename", "hantransfer-update.apk"),
                ) { done, total ->
                    updateJs(progress = done to total)
                }
                updateJs(message = "已打开安装程序，请在系统界面确认安装", done = true)
            } catch (e: Exception) {
                val msg = e.message ?: "更新失败"
                val hint = if (msg.contains("安装未知应用") || msg.contains("已下载")) msg else msg
                updateJs(error = hint, done = true)
            }
        }
    }

    @JavascriptInterface
    fun requestAllFilesAccess() {
        activity.runOnUiThread { StorageAccess.openAllFilesAccessSettings(activity) }
    }

    private fun locateAzFolder(showToast: Boolean): JSONObject {
        val live2d = StorageAccess.azCnLive2dDir()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && !StorageAccess.hasAllFilesAccess(activity)) {
            if (showToast) {
                activity.runOnUiThread { StorageAccess.openAllFilesAccessSettings(activity) }
            }
            return JSONObject()
                .put("ok", false)
                .put("need_files_access", true)
                .put("message", "请开启 hantransfer「所有文件访问」")
        }
        if (StorageAccess.pathIsDirectory(activity, live2d)) {
            saf.saveDirectPath(live2d)
            return JSONObject()
                .put("ok", true)
                .put("path", live2d.absolutePath)
                .put("mode", "direct")
                .put("message", "已连接国服 live2d 目录")
        }
        StorageAccess.findAzLive2dDocumentId(activity)?.let { docId ->
            saf.saveDocumentRoot(docId, live2d)
            return JSONObject()
                .put("ok", true)
                .put("path", live2d.absolutePath)
                .put("mode", "document")
                .put("message", "已连接国服 live2d 目录")
        }
        val mtLive2dId = MtDataProviderAccess.live2dDocumentId(StorageAccess.AZ_CN_PACKAGE)
        val mtAuthority = MtDataProviderAccess.authorityFor(StorageAccess.AZ_CN_PACKAGE)
        if (MtDataProviderAccess.isAvailable(activity, StorageAccess.AZ_CN_PACKAGE) &&
            MtDataProviderAccess.documentExists(activity, mtAuthority, mtLive2dId)
        ) {
            saf.saveMtProviderRoot(mtAuthority, mtLive2dId, live2d)
            return JSONObject()
                .put("ok", true)
                .put("path", live2d.absolutePath)
                .put("mode", "mt_provider")
                .put("message", "已连接国服 live2d 目录")
        }
        if (showToast) {
            activity.runOnUiThread { StorageAccess.openAllFilesAccessSettings(activity) }
        }
        return JSONObject()
            .put("ok", false)
            .put("message", StorageAccess.explainAzMissing(activity))
    }

    private fun effectiveAzCategoryFilter(category: String, direct: File?): String? {
        if (direct?.name.equals("live2d", ignoreCase = true) && category == "live2d") return null
        return category
    }

    @JavascriptInterface
    fun loadAzLive2dAsync() {
        ioExecutor.execute {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && !StorageAccess.hasAllFilesAccess(activity)) {
                activity.runOnUiThread { StorageAccess.openAllFilesAccessSettings(activity) }
                val blocked = JSONObject()
                    .put("ok", false)
                    .put("need_files_access", true)
                    .put("error", "请开启 hantransfer「所有文件访问」后返回")
                runJs("onAzLive2dReady($blocked)")
                return@execute
            }
            runJs("onAzLive2dReady(${buildAzLive2dPayload()})")
        }
    }

    @JavascriptInterface
    fun sendAzSelectedJson(relativePathsJson: String) {
        val device = selectedDevice() ?: run {
            azStatus("请先选择电脑", error = true)
            return
        }
        val relPaths = runCatching {
            val arr = JSONArray(relativePathsJson)
            buildSet {
                for (i in 0 until arr.length()) add(arr.getString(i))
            }
        }.getOrElse {
            azStatus("路径无效", error = true)
            return
        }
        if (relPaths.isEmpty()) {
            azStatus("未选择文件", error = true)
            return
        }
        transferExecutor.execute {
            beginUploadBatch()
            var ok = 0
            var skipped = 0
            var fail = 0
            var batchTotal = relPaths.size
            try {
                if (!guardTrustedForSend(device, SendChannel.AZ)) return@execute
                val files = collectAzLive2dFiles().filter { f ->
                    f.relativePath in relPaths || f.file?.absolutePath in relPaths
                }
                if (files.isEmpty()) {
                    azStatus("未找到可发送的文件", error = true)
                    return@execute
                }
                batchTotal = files.size
                for ((index, f) in files.withIndex()) {
                    if (uploadSession.shouldStop()) {
                        finishUploadPaused(ok + skipped, files.size, channel = SendChannel.AZ)
                        return@execute
                    }
                    when (val outcome = uploadOneAzFile(device, f, index + 1, files.size)) {
                        is AzUploadOutcome.Ok -> {
                            history.append(
                                TransferHistoryItem(f.displayName, device.name, outcome.path, System.currentTimeMillis(), "azurlane_asset"),
                            )
                            if (outcome.skipped) skipped++ else ok++
                        }
                        is AzUploadOutcome.Fail -> fail++
                        AzUploadOutcome.SkipReadable -> fail++
                    }
                }
                if (ok + skipped == 0) {
                    azStatus(if (fail > 0) "全部失败（$fail）" else "无法读取所选文件，请检查权限", error = true)
                    return@execute
                }
                if (uploadSession.shouldStop()) {
                    finishUploadPaused(ok + skipped, files.size, channel = SendChannel.AZ)
                    return@execute
                }
                val summary = batchUploadSummary(ok, skipped, fail, files.size)
                progressJs(summary, 1, 1, true, null, ok + skipped, files.size)
            } catch (e: TransferPausedException) {
                finishUploadPaused(ok + skipped, batchTotal, e.message, SendChannel.AZ)
            } catch (e: Exception) {
                azStatus(e.message ?: "发送失败", error = true)
            } finally {
                endUploadBatch()
            }
        }
    }

    private fun collectAzLive2dFiles(): List<AzurlaneFile> {
        locateAzFolder(showToast = false)
        val direct = saf.savedDirectPath()
        val docId = saf.savedDocumentId()
        val mt = saf.savedMtProvider()
        val root = direct ?: StorageAccess.azCnLive2dDir()
        val filter = effectiveAzCategoryFilter("live2d", direct ?: root)
        return when {
            mt != null -> AssetScanner.listFilesFromMtProvider(activity, mt.first, mt.second, filter)
            docId != null -> AssetScanner.listFilesFromDocumentId(activity, docId, filter)
            direct != null -> AssetScanner.listFilesFromPath(activity, direct, filter)
            else -> emptyList()
        }
    }

    private fun buildAzLive2dPayload(): JSONObject {
        val live2d = StorageAccess.azCnLive2dDir()
        val direct = saf.savedDirectPath()
        val docId = saf.savedDocumentId()
        val mt = saf.savedMtProvider()
        if (direct == null && docId == null && mt == null) {
            return JSONObject()
                .put("ok", false)
                .put("error", StorageAccess.explainAzMissing(activity))
        }
        val files = collectAzLive2dFiles()
        val arr = JSONArray()
        for (f in files) {
            arr.put(
                JSONObject()
                    .put("name", f.displayName)
                    .put("relative", f.relativePath)
                    .put("path", f.file?.absolutePath ?: f.relativePath)
                    .put("size", f.file?.length() ?: 0L)
                    .put("is_dir", false),
            )
        }
        val via = when {
            mt != null -> "mt_provider"
            docId != null -> "document"
            else -> "file"
        }
        return JSONObject()
            .put("ok", true)
            .put("path", (direct ?: live2d).absolutePath)
            .put("via", via)
            .put("entries", arr)
    }

    @JavascriptInterface
    fun sendAzBatch(category: String) {
        val device = selectedDevice() ?: run {
            azStatus("请先选择电脑", error = true)
            return
        }
        transferExecutor.execute {
            beginUploadBatch()
            var ok = 0
            var skipped = 0
            var fail = 0
            var batchTotal = 0
            try {
                locateAzFolder(showToast = false)
                val direct = saf.savedDirectPath()
                val docId = saf.savedDocumentId()
                val mt = saf.savedMtProvider()
                if (direct == null && docId == null && mt == null) {
                    azStatus(StorageAccess.explainAzMissing(activity), error = true)
                    return@execute
                }
                if (!guardTrustedForSend(device, SendChannel.AZ)) return@execute
                val filter = effectiveAzCategoryFilter(category, direct)
                val files = when {
                    mt != null -> AssetScanner.listFilesFromMtProvider(
                        activity,
                        mt.first,
                        mt.second,
                        filter,
                    )
                    docId != null -> AssetScanner.listFilesFromDocumentId(activity, docId, filter)
                    direct != null -> AssetScanner.listFilesFromPath(activity, direct, filter)
                    else -> emptyList()
                }
                if (files.isEmpty()) {
                    azStatus("未找到可发送的文件（目录可能为空或未下载皮肤）", error = true)
                    return@execute
                }
                batchTotal = files.size
                for ((index, f) in files.withIndex()) {
                    if (uploadSession.shouldStop()) {
                        finishUploadPaused(ok + skipped, files.size, channel = SendChannel.AZ)
                        return@execute
                    }
                    when (val outcome = uploadOneAzFile(device, f, index + 1, files.size)) {
                        is AzUploadOutcome.Ok -> {
                            history.append(
                                TransferHistoryItem(f.displayName, device.name, outcome.path, System.currentTimeMillis(), "azurlane_asset"),
                            )
                            if (outcome.skipped) skipped++ else ok++
                        }
                        is AzUploadOutcome.Fail -> fail++
                        AzUploadOutcome.SkipReadable -> fail++
                    }
                }
                if (ok + skipped == 0) {
                    azStatus(if (fail > 0) "全部失败（$fail）" else "无法读取文件，请检查权限", error = true)
                    return@execute
                }
                if (uploadSession.shouldStop()) {
                    finishUploadPaused(ok + skipped, files.size, channel = SendChannel.AZ)
                    return@execute
                }
                val summary = batchUploadSummary(ok, skipped, fail, files.size)
                progressJs(summary, 1, 1, true, null, ok + skipped, files.size)
            } catch (e: TransferPausedException) {
                finishUploadPaused(ok + skipped, batchTotal, e.message, SendChannel.AZ)
            } catch (e: Exception) {
                azStatus(e.message ?: "发送失败", error = true)
            } finally {
                endUploadBatch()
            }
        }
    }

    fun onFilePicked(uri: Uri) {
        onFilesPicked(listOf(uri))
    }

    fun onFilesPicked(uris: List<Uri>) {
        val device = selectedDevice() ?: run {
            progressJs("file", 0, 0, true, "未选择电脑")
            return
        }
        if (uris.isEmpty()) return
        transferExecutor.execute {
            beginUploadBatch()
            var ok = 0
            var skipped = 0
            var fail = 0
            try {
                if (!guardTrustedForSend(device, SendChannel.FILE)) return@execute
                for ((index, uri) in uris.withIndex()) {
                    if (uploadSession.shouldStop()) {
                        finishUploadPaused(ok + skipped, uris.size)
                        return@execute
                    }
                    val name = uri.lastPathSegment ?: "file"
                    try {
                        val raw = transfer.uploadUri(uri, device) { sent, total ->
                            progressJs(name, sent, total, false, null, index + 1, uris.size)
                        }
                        val result = parseUploadResult(raw)
                        history.append(TransferHistoryItem(name, device.name, result.path, System.currentTimeMillis(), "file"))
                        if (result.skipped) skipped++ else ok++
                    } catch (e: TransferPausedException) {
                        throw e
                    } catch (e: Exception) {
                        fail++
                        progressJs(name, 0, 0, false, e.message, index + 1, uris.size)
                    }
                }
                if (ok + skipped == 0) {
                    progressJs("file", 0, 0, true, if (fail > 0) "全部失败（$fail）" else "发送失败")
                    return@execute
                }
                if (uploadSession.shouldStop()) {
                    finishUploadPaused(ok + skipped, uris.size)
                    return@execute
                }
                val summary = batchUploadSummary(ok, skipped, fail, uris.size)
                progressJs(summary, 1, 1, true, null, ok + skipped, uris.size)
            } catch (e: TransferPausedException) {
                finishUploadPaused(ok + skipped, uris.size, e.message)
            } catch (e: Exception) {
                progressJs("file", 0, 0, true, e.message)
            } finally {
                endUploadBatch()
            }
        }
    }

    private fun resolveUpdateDevice(): DiscoveredDevice? {
        selectedDevice()?.let { return it }
        val host = loadSavedHost(activity) ?: return null
        return DiscoveredDevice(
            serviceName = loadSavedServiceName(activity) ?: "$host:${loadSavedPort(activity)}",
            name = host,
            platform = "windows",
            version = "0.1.0",
            deviceId = "",
            features = emptyList(),
            host = host,
            port = loadSavedPort(activity),
        )
    }

    private fun selectedDevice(): DiscoveredDevice? {
        val list = allDevices()
        selectedServiceName?.let { name ->
            list.find { it.serviceName == name }?.let { return it }
        }
        val host = selectedHost
        if (host != null) {
            list.find { it.host == host && it.port == selectedPort }?.let { return it }
            return DiscoveredDevice(
                serviceName = selectedServiceName ?: "$host:$selectedPort",
                name = host,
                platform = "windows",
                version = "0.1.0",
                deviceId = "",
                features = emptyList(),
                host = host,
                port = selectedPort,
            )
        }
        if (list.isEmpty()) return null
        if (list.size == 1) return list[0]
        return list.firstOrNull()
    }

    private fun allDevices(): List<DiscoveredDevice> {
        val merged = linkedMapOf<String, DiscoveredDevice>()
        discovery.devices().forEach { merged["${it.host}:${it.port}"] = it }
        httpDiscovered.values.forEach { merged["${it.host}:${it.port}"] = it }
        return merged.values.sortedBy { it.name }
    }

    private fun autoConnectSavedInternal() {
        val host = loadSavedHost(activity)
        if (host.isNullOrBlank()) {
            probeSavedPcInternal(notify = true)
            return
        }
        connectToHostInternal(host, loadSavedPort(activity))
    }

    private fun connectToHostInternal(host: String, port: Int) {
        if (host.isEmpty() || port <= 0) {
            emitConnectResult(ok = false, error = "请输入有效的 PC IP")
            return
        }
        // Never silent-return: JS shows a sticky loading toast until onNativeConnectResult.
        if (connecting) {
            emitConnectResult(ok = false, error = "正在连接中，请稍候")
            return
        }
        connecting = true
        try {
            val device = PcStatusProbe.probe(host, port)
            if (device == null) {
                probeSavedPcInternal(notify = true)
                emitConnectResult(
                    ok = false,
                    error = "无法连接 $host:$port，请确认 PC 已运行 hantransfer 且在同一 WiFi",
                )
                return
            }
            httpDiscovered[device.serviceName] = device
            selectedHost = device.host
            selectedPort = device.port
            selectedServiceName = device.serviceName
            saveSelectedDevice(device.host, device.port, device.serviceName)
            activity.runOnUiThread { notifyDevices() }
            emitHandshakeResult(device)
        } finally {
            connecting = false
        }
    }

    private fun emitHandshakeResult(device: DiscoveredDevice) {
        when (handshakeStatus(device, force = true)) {
            HandshakeStatus.TRUSTED -> emitConnectResult(
                ok = true,
                pending = false,
                device = device,
                message = "已连接并已信任",
            )
            HandshakeStatus.PENDING -> emitConnectResult(
                ok = true,
                pending = true,
                device = device,
                message = "等待电脑确认信任…",
            )
            HandshakeStatus.REJECTED -> emitConnectResult(
                ok = false,
                device = device,
                error = "电脑已拒绝此设备",
            )
            HandshakeStatus.ERROR -> emitConnectResult(
                ok = false,
                device = device,
                error = "握手失败",
            )
        }
    }

    private fun dispatchJs(event: String, payload: JSONObject) {
        val quoted = JSONObject.quote(payload.toString())
        runJs("(function(){try{var p=JSON.parse($quoted);typeof $event==='function'&&$event(p);}catch(_){}})()")
    }

    private fun dispatchJsRaw(event: String, json: String) {
        val quoted = JSONObject.quote(json)
        runJs("(function(){try{var p=JSON.parse($quoted);typeof $event==='function'&&$event(p);}catch(_){}})()")
    }

    private fun probeSavedPcInternal(notify: Boolean): Boolean {
        val host = loadSavedHost(activity) ?: return false
        val port = loadSavedPort(activity)
        val device = PcStatusProbe.probe(host, port) ?: return false
        httpDiscovered[device.serviceName] = device
        if (notify) {
            activity.runOnUiThread { notifyDevices() }
        }
        return true
    }

    private fun isFileOriginUrl(): Boolean {
        return activity.webViewUrl()?.startsWith("file:") == true
    }

    private fun emitConnectResult(
        ok: Boolean,
        pending: Boolean = false,
        device: DiscoveredDevice? = null,
        message: String? = null,
        error: String? = null,
    ) {
        val payload = JSONObject()
            .put("ok", ok)
            .put("pending", pending)
        if (message != null) payload.put("message", message)
        if (error != null) payload.put("error", error)
        if (device != null) {
            payload.put(
                "device",
                JSONObject()
                    .put("id", device.serviceName)
                    .put("name", device.name)
                    .put("platform", device.platform)
                    .put("host", device.host)
                    .put("port", device.port),
            )
        }
        dispatchJs("onNativeConnectResult", payload)
    }

    private fun notifyDevices() {
        dispatchJsRaw("onDevicesUpdated", devicesJson())
    }

    private fun devicesJson(): String {
        val arr = JSONArray()
        allDevices().forEach { d ->
            arr.put(
                JSONObject()
                    .put("id", d.serviceName)
                    .put("name", d.name)
                    .put("platform", d.platform)
                    .put("host", d.host)
                    .put("port", d.port),
            )
        }
        return arr.toString()
    }

    private enum class SendChannel { FILE, AZ }

    private fun guardTrustedForSend(device: DiscoveredDevice, channel: SendChannel): Boolean {
        return when (handshakeStatus(device, force = true)) {
            HandshakeStatus.TRUSTED -> true
            HandshakeStatus.PENDING -> {
                val msg = "等待电脑确认信任"
                when (channel) {
                    SendChannel.FILE -> progressJs("file", 0, 0, true, msg)
                    SendChannel.AZ -> azStatus(msg, error = true)
                }
                false
            }
            HandshakeStatus.REJECTED -> {
                val msg = "电脑已拒绝此设备"
                when (channel) {
                    SendChannel.FILE -> progressJs("file", 0, 0, true, msg)
                    SendChannel.AZ -> azStatus(msg, error = true)
                }
                false
            }
            HandshakeStatus.ERROR -> {
                val msg = "无法连接电脑"
                when (channel) {
                    SendChannel.FILE -> progressJs("file", 0, 0, true, msg)
                    SendChannel.AZ -> azStatus(msg, error = true)
                }
                false
            }
        }
    }

    private fun beginUploadBatch() {
        uploadSession.reset()
        transfer.bindUploadControl(
            shouldCancel = { uploadSession.shouldStop() },
            onCall = { uploadSession.track(it) },
        )
    }

    private fun endUploadBatch() {
        transfer.clearUploadControl()
        uploadSession.clearCall()
    }

    private fun finishUploadPaused(
        completed: Int,
        total: Int,
        message: String? = null,
        channel: SendChannel = SendChannel.FILE,
    ) {
        val msg = message ?: "已暂停，已完成 $completed${if (total > 0) "/$total" else ""} 个文件"
        if (channel == SendChannel.AZ) {
            progressJs("已暂停", 0, 0, true, msg, paused = true, index = completed, batchTotal = total)
        } else {
            progressJs("已暂停", 0, 0, true, msg, paused = true, index = completed, batchTotal = total)
        }
    }

    private data class UploadParse(val path: String, val skipped: Boolean)

    private sealed class AzUploadOutcome {
        data class Ok(val path: String, val skipped: Boolean) : AzUploadOutcome()
        data class Fail(val message: String?) : AzUploadOutcome()
        data object SkipReadable : AzUploadOutcome()
    }

    private fun parseUploadResult(raw: String): UploadParse {
        return if (raw.startsWith("skipped:")) {
            UploadParse(raw.removePrefix("skipped:"), skipped = true)
        } else {
            UploadParse(raw, skipped = false)
        }
    }

    private fun batchUploadSummary(ok: Int, skipped: Int, fail: Int, total: Int): String {
        val parts = mutableListOf<String>()
        if (ok > 0) parts.add("已发送 $ok")
        if (skipped > 0) parts.add("跳过 $skipped 个重复")
        if (fail > 0) parts.add("失败 $fail")
        if (parts.isEmpty()) return "没有可发送的文件"
        return parts.joinToString("，") + "（共 $total）"
    }

    private fun uploadOneAzFile(
        device: DiscoveredDevice,
        f: AzurlaneFile,
        index: Int,
        batchTotal: Int,
    ): AzUploadOutcome {
        if (f.file == null && f.uri == null) return AzUploadOutcome.SkipReadable
        return try {
            val raw = when {
                f.file != null && runCatching { StorageAccess.resolveReadableFile(f.file).canRead() }.getOrDefault(false) ->
                    transfer.uploadFile(
                        file = StorageAccess.resolveReadableFile(f.file),
                        filename = f.displayName,
                        device = device,
                        type = "azurlane_asset",
                        category = f.category,
                        relativePath = f.relativePath,
                    ) { sent, total ->
                        progressJs(f.displayName, sent, total, false, null, index, batchTotal)
                    }
                f.uri != null -> transfer.uploadUri(
                    uri = f.uri,
                    device = device,
                    type = "azurlane_asset",
                    category = f.category,
                    relativePath = f.relativePath,
                ) { sent, total ->
                    progressJs(f.displayName, sent, total, false, null, index, batchTotal)
                }
                else -> return AzUploadOutcome.SkipReadable
            }
            val parsed = parseUploadResult(raw)
            AzUploadOutcome.Ok(parsed.path, parsed.skipped)
        } catch (e: TransferPausedException) {
            throw e
        } catch (e: Exception) {
            progressJs(f.displayName, 0, 0, false, e.message, index, batchTotal)
            AzUploadOutcome.Fail(e.message)
        }
    }

    private fun progressJs(
        name: String,
        sent: Long,
        total: Long,
        done: Boolean,
        error: String?,
        index: Int = 0,
        batchTotal: Int = 0,
        paused: Boolean = false,
    ) {
        val payload = JSONObject()
            .put("name", name)
            .put("sent", sent)
            .put("total", total)
            .put("done", done)
            .put("paused", paused)
        if (error != null) payload.put("error", error)
        if (batchTotal > 0) {
            payload.put("batch_total", batchTotal)
            if (index > 0) payload.put("index", index)
        }
        dispatchJs("onUploadProgressPayload", payload)
    }

    private fun azStatus(message: String, error: Boolean = false) {
        val payload = JSONObject()
            .put("message", message)
            .put("error", error)
        runJs("onAzStatusPayload($payload)")
    }

    private fun receiveJs(
        name: String,
        fileDone: Boolean = false,
        batchDone: Boolean = false,
        progress: Pair<Long, Long>? = null,
        error: String? = null,
        pushId: String? = null,
        index: Int = 0,
        batchTotal: Int = 0,
        successCount: Int = 0,
        failCount: Int = 0,
    ) {
        val payload = JSONObject().put("name", name)
        if (fileDone) payload.put("done", true)
        if (batchDone) payload.put("batch_done", true)
        if (error != null) payload.put("error", error)
        if (pushId != null) payload.put("push_id", pushId)
        if (batchTotal > 0) {
            payload.put("batch_total", batchTotal)
            if (index > 0) payload.put("index", index)
        }
        if (batchDone) {
            payload.put("success_count", successCount)
            payload.put("fail_count", failCount)
        }
        if (progress != null) {
            payload.put("sent", progress.first)
            payload.put("total", progress.second)
        }
        runJs("onReceiveProgressPayload($payload)")
    }

    private fun updateCheckJs(info: JSONObject) {
        runJs("onAppUpdateCheckResult($info)")
    }

    private fun updateJs(
        message: String? = null,
        error: String? = null,
        done: Boolean = false,
        progress: Pair<Long, Long>? = null,
    ) {
        val payload = JSONObject()
        if (message != null) payload.put("message", message)
        if (error != null) payload.put("error", error)
        if (done) payload.put("done", true)
        if (progress != null) {
            payload.put("sent", progress.first)
            payload.put("total", progress.second)
        }
        runJs("onAppUpdatePayload($payload)")
    }

    private fun saveSelectedDevice(host: String, port: Int, serviceName: String?) {
        activity.getSharedPreferences(PREFS_DEVICE, android.content.Context.MODE_PRIVATE)
            .edit()
            .putString(KEY_HOST, host)
            .putInt(KEY_PORT, port)
            .putString(KEY_SERVICE, serviceName)
            .apply()
    }

    companion object {
        private const val PREFS_DEVICE = "hantransfer_device"
        private const val KEY_HOST = "host"
        private const val KEY_PORT = "port"
        private const val KEY_SERVICE = "service_name"

        private fun loadSavedHost(activity: MainActivity): String? =
            activity.getSharedPreferences(PREFS_DEVICE, android.content.Context.MODE_PRIVATE)
                .getString(KEY_HOST, null)
                ?.takeIf { it.isNotBlank() }

        private fun loadSavedPort(activity: MainActivity): Int =
            activity.getSharedPreferences(PREFS_DEVICE, android.content.Context.MODE_PRIVATE)
                .getInt(KEY_PORT, 7822)

        private fun loadSavedServiceName(activity: MainActivity): String? =
            activity.getSharedPreferences(PREFS_DEVICE, android.content.Context.MODE_PRIVATE)
                .getString(KEY_SERVICE, null)
                ?.takeIf { it.isNotBlank() }
    }
}
