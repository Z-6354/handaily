package com.handaily.hantransfer

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import okhttp3.Call
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.MultipartBody
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.asRequestBody
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.RequestBody
import okio.BufferedSink
import org.json.JSONObject
import java.io.File
import java.io.InputStream
import java.io.FileOutputStream
import java.security.MessageDigest
import java.util.UUID
import java.util.concurrent.TimeUnit

class TransferClient(context: Context) {
    private val appContext = context.applicationContext
    private val prefs = appContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    /** Large file upload/download — long timeouts. */
    private val http = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(300, TimeUnit.SECONDS)
        .writeTimeout(300, TimeUnit.SECONDS)
        .build()

    /** Handshake / status / push-list — short timeouts so pool threads cannot stick for minutes. */
    private val apiHttp = OkHttpClient.Builder()
        .connectTimeout(4, TimeUnit.SECONDS)
        .readTimeout(10, TimeUnit.SECONDS)
        .writeTimeout(10, TimeUnit.SECONDS)
        .build()

    @Volatile
    private var shouldCancel: (() -> Boolean)? = null

    @Volatile
    private var onCall: ((Call) -> Unit)? = null

    fun bindUploadControl(shouldCancel: () -> Boolean, onCall: (Call) -> Unit) {
        this.shouldCancel = shouldCancel
        this.onCall = onCall
    }

    fun clearUploadControl() {
        shouldCancel = null
        onCall = null
    }

    private fun checkCancelled() {
        if (shouldCancel?.invoke() == true) throw TransferPausedException()
    }

    fun localDeviceId(): String {
        val existing = prefs.getString(KEY_DEVICE_ID, null)
        if (existing != null) return existing
        val created = UUID.randomUUID().toString()
        prefs.edit().putString(KEY_DEVICE_ID, created).apply()
        return created
    }

    fun localDeviceName(): String {
        return prefs.getString(KEY_DEVICE_NAME, null) ?: "HAN-PHONE".also {
            prefs.edit().putString(KEY_DEVICE_NAME, it).apply()
        }
    }

    fun ensureTrusted(device: DiscoveredDevice): HandshakeStatus {
        repeat(60) {
            when (val status = handshakeOnce(device)) {
                HandshakeStatus.TRUSTED, HandshakeStatus.REJECTED -> return status
                HandshakeStatus.PENDING -> Thread.sleep(2000)
                HandshakeStatus.ERROR -> Thread.sleep(1500)
            }
        }
        return HandshakeStatus.PENDING
    }

    fun handshakeOnce(device: DiscoveredDevice): HandshakeStatus {
        val base = apiBase(device)
        val body = JSONObject()
            .put("device_id", localDeviceId())
            .put("name", localDeviceName())
            .put("platform", "android")
            .put("version", appVersion())
            .toString()
            .toRequestBody("application/json".toMediaType())
        val request = Request.Builder()
            .url("$base/api/v1/handshake")
            .post(body)
            .build()
        return runCatching {
            apiHttp.newCall(request).execute().use { resp ->
                when (resp.code) {
                    200 -> HandshakeStatus.TRUSTED
                    202 -> HandshakeStatus.PENDING
                    403 -> HandshakeStatus.REJECTED
                    else -> HandshakeStatus.ERROR
                }
            }
        }.getOrDefault(HandshakeStatus.ERROR)
    }

    fun uploadUri(
        uri: Uri,
        device: DiscoveredDevice,
        type: String = "file",
        category: String? = null,
        relativePath: String? = null,
        onProgress: (Long, Long) -> Unit = { _, _ -> },
    ): String {
        val resolver = appContext.contentResolver
        val filename = queryDisplayName(uri) ?: "file.bin"
        val size = queryContentSize(uri).coerceAtLeast(0L)
        val hash = resolver.openInputStream(uri)?.use { sha256Stream(it) } ?: error("cannot read file")
        val fileBody = uriRequestBody(uri, size, onProgress)
        return uploadStream(
            filename = filename,
            size = size,
            hash = hash,
            fileBody = fileBody,
            device = device,
            type = type,
            category = category,
            relativePath = relativePath,
            onProgress = onProgress,
        )
    }

    fun uploadFile(
        file: File,
        filename: String,
        device: DiscoveredDevice,
        type: String = "file",
        category: String? = null,
        relativePath: String? = null,
        onProgress: (Long, Long) -> Unit = { _, _ -> },
    ): String {
        val size = file.length()
        val hash = sha256(file)
        val fileBody = fileRequestBody(file, size, onProgress)
        return uploadStream(
            filename = filename,
            size = size,
            hash = hash,
            fileBody = fileBody,
            device = device,
            type = type,
            category = category,
            relativePath = relativePath,
            onProgress = onProgress,
        )
    }

    private fun uploadStream(
        filename: String,
        size: Long,
        hash: String,
        fileBody: RequestBody,
        device: DiscoveredDevice,
        type: String,
        category: String?,
        relativePath: String?,
        onProgress: (Long, Long) -> Unit,
    ): String {
        val transferId = UUID.randomUUID().toString()
        val metadata = JSONObject()
            .put("filename", filename)
            .put("size", size)
            .put("hash", hash)
            .put("type", type)
            .put("source", localDeviceName())
        if (category != null) metadata.put("category", category)
        if (relativePath != null) metadata.put("relative_path", relativePath)

        // Same path + same size on PC → skip body transfer.
        checkDuplicateOnPc(device, metadata)?.let { existingPath ->
            onProgress(size, size)
            return "skipped:$existingPath"
        }

        val safeName = sanitizeFilename(filename)
        val multipart = MultipartBody.Builder()
            .setType(MultipartBody.FORM)
            .addFormDataPart(
                "metadata",
                null,
                metadata.toString().toRequestBody("application/json".toMediaType()),
            )
            .addFormDataPart(
                "file",
                safeName,
                fileBody,
            )
            .build()
        val base = apiBase(device)
        val request = Request.Builder()
            .url("$base/api/v1/files")
            .addHeader("X-Hantransfer-Device-ID", localDeviceId())
            .addHeader("X-Hantransfer-Transfer-ID", transferId)
            .post(multipart)
            .build()
        onProgress(0, size)
        checkCancelled()
        val call = http.newCall(request)
        onCall?.invoke(call)
        try {
            call.execute().use { resp ->
                checkCancelled()
                val text = resp.body?.string().orEmpty()
                if (!resp.isSuccessful) error(parseApiError(text, resp.code, "上传失败"))
                onProgress(size, size)
                val parsed = apiData(text)
                val status = parsed.optString("status", "")
                val saved = parsed.optString("path", "")
                if (saved.isBlank()) error("电脑未返回保存路径，请确认 PC 端 hantransfer 已启动")
                if (status == "pending_approval" || saved == "pending") return "pending:$transferId"
                if (status == "skipped") return "skipped:$saved"
                return saved
            }
        } catch (e: java.io.IOException) {
            if (shouldCancel?.invoke() == true) throw TransferPausedException()
            throw e
        }
    }

    /** Returns existing PC path when same name+size already exists; null otherwise. */
    private fun checkDuplicateOnPc(device: DiscoveredDevice, metadata: JSONObject): String? {
        val base = apiBase(device)
        val request = Request.Builder()
            .url("$base/api/v1/files/check")
            .addHeader("X-Hantransfer-Device-ID", localDeviceId())
            .post(metadata.toString().toRequestBody("application/json".toMediaType()))
            .build()
        return try {
            apiHttp.newCall(request).execute().use { resp ->
                if (!resp.isSuccessful) return null
                val parsed = apiData(resp.body?.string().orEmpty())
                if (parsed.optBoolean("exists", false) || parsed.optBoolean("skipped", false)) {
                    parsed.optString("path", "").takeIf { it.isNotBlank() }
                } else {
                    null
                }
            }
        } catch (_: Exception) {
            null
        }
    }

    private fun fileRequestBody(
        file: File,
        size: Long,
        onProgress: (Long, Long) -> Unit,
    ): RequestBody {
        val total = size.coerceAtLeast(0L)
        return object : RequestBody() {
            override fun contentType() = "application/octet-stream".toMediaType()

            override fun contentLength(): Long = if (total > 0L) total else -1L

            override fun writeTo(sink: BufferedSink) {
                var sent = 0L
                file.inputStream().use { input ->
                    val buffer = ByteArray(65536)
                    while (true) {
                        checkCancelled()
                        val read = input.read(buffer)
                        if (read <= 0) break
                        sink.write(buffer, 0, read)
                        sent += read
                        onProgress(sent, if (total > 0L) total else sent)
                    }
                }
            }
        }
    }

    private fun apiBase(device: DiscoveredDevice): String = PcEndpoint.resolveApiBase(device)

    private fun uriRequestBody(
        uri: Uri,
        size: Long,
        onProgress: (Long, Long) -> Unit,
    ): RequestBody {
        val resolver = appContext.contentResolver
        val total = size.coerceAtLeast(0L)
        return object : RequestBody() {
            override fun contentType() = "application/octet-stream".toMediaType()

            override fun contentLength(): Long = if (total > 0L) total else -1L

            override fun writeTo(sink: BufferedSink) {
                var sent = 0L
                resolver.openInputStream(uri)?.use { input ->
                    val buffer = ByteArray(65536)
                    while (true) {
                        checkCancelled()
                        val read = input.read(buffer)
                        if (read <= 0) break
                        sink.write(buffer, 0, read)
                        sent += read
                        onProgress(sent, if (total > 0L) total else sent)
                    }
                } ?: error("cannot read file")
            }
        }
    }

    fun listPushPending(device: DiscoveredDevice): List<PushItem> {
        val base = apiBase(device)
        val request = Request.Builder()
            .url("$base/api/v1/push/pending")
            .addHeader("X-Hantransfer-Device-ID", localDeviceId())
            .get()
            .build()
        apiHttp.newCall(request).execute().use { resp ->
            val text = resp.body?.string().orEmpty()
            if (!resp.isSuccessful) error("push pending failed ${resp.code}: $text")
            val root = JSONObject(text)
            if (root.has("ok") && !root.optBoolean("ok", true)) {
                val err = root.optJSONObject("error")
                error(err?.optString("message") ?: "request failed")
            }
            val arr = root.optJSONArray("data") ?: org.json.JSONArray()
            val list = mutableListOf<PushItem>()
            for (i in 0 until arr.length()) {
                val o = arr.getJSONObject(i)
                list.add(
                    PushItem(
                        pushId = o.getString("push_id"),
                        filename = o.getString("filename"),
                        size = o.getLong("size"),
                        source = o.optString("source", device.name),
                    ),
                )
            }
            return list
        }
    }

    fun downloadPush(
        device: DiscoveredDevice,
        item: PushItem,
        onProgress: (Long, Long) -> Unit = { _, _ -> },
    ): File {
        val dir = File(appContext.getExternalFilesDir(android.os.Environment.DIRECTORY_DOWNLOADS), "hantransfer")
        if (!dir.exists()) dir.mkdirs()
        existingSameSize(dir, item.filename, item.size)?.let { existing ->
            onProgress(item.size.coerceAtLeast(0L), item.size.coerceAtLeast(0L))
            ackPush(device, item.pushId)
            scanDownload(existing)
            return existing
        }

        val base = apiBase(device)
        val request = Request.Builder()
            .url("$base/api/v1/push/${item.pushId}/file")
            .addHeader("X-Hantransfer-Device-ID", localDeviceId())
            .get()
            .build()
        http.newCall(request).execute().use { resp ->
            if (!resp.isSuccessful) {
                val msg = when (resp.code) {
                    401 -> "未信任，请在电脑管理页允许连接"
                    403 -> "该文件不是发给本机的"
                    404 -> "文件不存在或已被接收"
                    else -> "下载失败 (${resp.code})"
                }
                error(msg)
            }
            val body = resp.body ?: error("empty push body")
            val total = item.size.coerceAtLeast(body.contentLength().coerceAtLeast(0L))
            val out = uniqueDestination(dir, item.filename)
            body.byteStream().use { input ->
                FileOutputStream(out).use { output ->
                    val buf = ByteArray(65536)
                    var readTotal = 0L
                    while (true) {
                        val n = input.read(buf)
                        if (n <= 0) break
                        output.write(buf, 0, n)
                        readTotal += n
                        onProgress(readTotal, total)
                    }
                }
            }
            if (item.size > 0L && out.length() < item.size) {
                out.delete()
                error("下载不完整 (${out.length()}/${item.size} 字节)")
            }
            ackPush(device, item.pushId)
            scanDownload(out)
            return out
        }
    }

    private fun existingSameSize(dir: File, filename: String, size: Long): File? {
        if (size <= 0L) return null
        val candidate = File(dir, filename)
        if (candidate.isFile && candidate.length() == size) return candidate
        return null
    }

    private fun uniqueDestination(dir: File, filename: String): File {
        var candidate = File(dir, filename)
        if (!candidate.exists()) return candidate
        // Same name but different size → keep unique copy; same size handled by existingSameSize.
        val dot = filename.lastIndexOf('.')
        val base = if (dot > 0) filename.substring(0, dot) else filename
        val ext = if (dot > 0) filename.substring(dot) else ""
        var n = 1
        while (candidate.exists()) {
            candidate = File(dir, "$base ($n)$ext")
            n++
        }
        return candidate
    }

    private fun scanDownload(file: File) {
        android.media.MediaScannerConnection.scanFile(
            appContext,
            arrayOf(file.absolutePath),
            null,
            null,
        )
    }

    fun ackPush(device: DiscoveredDevice, pushId: String) {
        val base = apiBase(device)
        val request = Request.Builder()
            .url("$base/api/v1/push/$pushId/ack")
            .addHeader("X-Hantransfer-Device-ID", localDeviceId())
            .post("".toRequestBody("application/json".toMediaType()))
            .build()
        http.newCall(request).execute().use { resp ->
            if (!resp.isSuccessful) error("push ack failed ${resp.code}")
        }
    }

    private fun apiData(text: String): JSONObject {
        val root = JSONObject(text)
        if (root.has("ok") && !root.optBoolean("ok", true)) {
            val err = root.optJSONObject("error")
            error(err?.optString("message") ?: "request failed")
        }
        return root.optJSONObject("data") ?: root
    }

    private fun parseApiError(text: String, code: Int, fallback: String): String {
        val parsed = runCatching {
            val root = JSONObject(text)
            val err = root.optJSONObject("error")
            err?.optString("message")?.takeIf { it.isNotBlank() }
        }.getOrNull()
        return when {
            code == 401 -> parsed ?: "电脑未信任此设备，请在管理页点允许"
            code == 403 -> parsed ?: "电脑已拒绝此设备"
            !parsed.isNullOrBlank() -> parsed
            else -> "$fallback (HTTP $code)"
        }
    }

    private fun sha256Stream(input: InputStream): String {
        val digest = MessageDigest.getInstance("SHA-256")
        val buf = ByteArray(8192)
        while (true) {
            val n = input.read(buf)
            if (n <= 0) break
            digest.update(buf, 0, n)
        }
        return "sha256:" + digest.digest().joinToString("") { "%02x".format(it) }
    }

    private fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buf = ByteArray(8192)
            while (true) {
                val n = input.read(buf)
                if (n <= 0) break
                digest.update(buf, 0, n)
            }
        }
        return "sha256:" + digest.digest().joinToString("") { "%02x".format(it) }
    }

    private fun appVersion(): String = runCatching {
        appContext.getString(R.string.app_version_display)
    }.getOrDefault("0.1.0")

    private fun sanitizeFilename(name: String): String {
        val trimmed = name.trim().ifEmpty { "file.bin" }
        val cleaned = trimmed
            .replace(Regex("[\"\\r\\n]"), "_")
            .replace(':', '_')
            .replace('/', '_')
            .replace('\\', '_')
        return cleaned.ifEmpty { "file.bin" }
    }

    private fun queryContentSize(uri: Uri): Long {
        appContext.contentResolver.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)
            ?.use { cursor ->
                if (cursor.moveToFirst()) {
                    val idx = cursor.getColumnIndex(OpenableColumns.SIZE)
                    if (idx >= 0 && !cursor.isNull(idx)) return cursor.getLong(idx)
                }
            }
        return -1L
    }

    private fun queryDisplayName(uri: Uri): String? {
        appContext.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
            ?.use { cursor ->
                if (cursor.moveToFirst()) {
                    val idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (idx >= 0) return cursor.getString(idx)
                }
            }
        return uri.lastPathSegment
    }

    companion object {
        private const val PREFS = "hantransfer"
        private const val KEY_DEVICE_ID = "device_id"
        private const val KEY_DEVICE_NAME = "device_name"
    }
}
