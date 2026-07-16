package com.handaily.hantransfer

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.ContentValues
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.provider.Settings
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

object ApkUpdateManager {
    private val http = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(300, TimeUnit.SECONDS)
        .writeTimeout(300, TimeUnit.SECONDS)
        .build()

    private val checkHttp = OkHttpClient.Builder()
        .connectTimeout(8, TimeUnit.SECONDS)
        .readTimeout(15, TimeUnit.SECONDS)
        .build()

    private val INSTALLER_PACKAGES = listOf(
        "com.miui.packageinstaller",
        "com.android.packageinstaller",
        "com.google.android.packageinstaller",
        "com.lbe.security.miui",
    )

    fun localBuildCode(activity: Activity): Int =
        runCatching { activity.getString(R.string.app_version_code).toInt() }.getOrDefault(0)

    fun localDisplay(activity: Activity): String =
        runCatching { activity.getString(R.string.app_version_display) }.getOrDefault("0.1.0")

    fun needsInstallPermission(activity: Activity): Boolean {
        return Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            !activity.packageManager.canRequestPackageInstalls()
    }

    fun openInstallPermissionSettings(activity: Activity) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        activity.startActivity(
            Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES).apply {
                data = Uri.parse("package:${activity.packageName}")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            },
        )
    }

    fun checkUpdate(activity: Activity, device: DiscoveredDevice): JSONObject {
        val localBuild = localBuildCode(activity)
        val localDisplay = localDisplay(activity)
        val baseUrl = PcEndpoint.resolveApiBase(device)
        val request = Request.Builder()
            .url("$baseUrl/api/v1/app/release")
            .get()
            .build()
        return runCatching {
            checkHttp.newCall(request).execute().use { resp ->
                val text = resp.body?.string().orEmpty()
                if (!resp.isSuccessful) {
                    return@use JSONObject()
                        .put("ok", false)
                        .put("local_build", localBuild)
                        .put("local_display", localDisplay)
                        .put("error", parseError(text, resp.code) ?: "检查更新失败 (${resp.code})")
                }
                val data = parseApiData(text)
                val remoteBuild = data.optInt("build", 0)
                val updateAvailable = remoteBuild > localBuild
                JSONObject()
                    .put("ok", true)
                    .put("local_build", localBuild)
                    .put("local_display", localDisplay)
                    .put("remote_build", remoteBuild)
                    .put("remote_display", data.optString("display"))
                    .put("filename", data.optString("filename"))
                    .put("size", data.optLong("size"))
                    .put("download_url", data.optString("download_url", "/api/v1/app/release/download"))
                    .put("update_available", updateAvailable)
            }
        }.getOrElse { e ->
            JSONObject()
                .put("ok", false)
                .put("local_build", localBuild)
                .put("local_display", localDisplay)
                .put("error", networkErrorMessage(e, baseUrl))
        }
    }

    private fun parseApiData(text: String): JSONObject {
        val root = JSONObject(text)
        return root.optJSONObject("data") ?: root
    }

    private fun networkErrorMessage(e: Throwable, baseUrl: String): String {
        val msg = e.message.orEmpty()
        return when {
            msg.contains("Failed to connect", ignoreCase = true) ||
                msg.contains("Connection refused", ignoreCase = true) ||
                msg.contains("ECONNREFUSED", ignoreCase = true) ->
                "无法连接 PC（$baseUrl）。请确认 hantransfer 已启动且在同一 WiFi"
            msg.contains("timeout", ignoreCase = true) ||
                msg.contains("timed out", ignoreCase = true) ->
                "连接 PC 超时。请确认防火墙已放行 7822 端口"
            msg.contains("Unable to resolve host", ignoreCase = true) ||
                msg.contains("UnknownHost", ignoreCase = true) ->
                "无法解析 PC 地址。请在「设备」页重新选择或手动输入 IP"
            msg.isNotBlank() -> "检查更新失败：$msg"
            else -> "检查更新失败，请确认 PC 端 hantransfer 已启动"
        }
    }

    fun downloadAndInstall(
        activity: Activity,
        device: DiscoveredDevice,
        downloadPath: String,
        filename: String,
        onProgress: (Long, Long) -> Unit,
    ) {
        val apk = downloadApk(activity, device, downloadPath, filename, onProgress)
        if (needsInstallPermission(activity)) {
            openInstallPermissionSettings(activity)
            error(
                "APK 已下载（${apk.length() / 1024} KB）。\n" +
                    "请允许「安装未知应用」后，再点「下载并安装」重试",
            )
        }
        launchInstallUi(activity, apk)
    }

    fun clearCache(activity: Activity): JSONObject {
        var bytes = 0L
        var files = 0
        activity.cacheDir.listFiles()?.forEach { f ->
            if (f.isFile && (f.name.startsWith("shizuku-") || f.name.startsWith("upload-"))) {
                bytes += f.length()
                if (f.delete()) files++
            }
        }
        updatesDir(activity).listFiles()?.forEach { f ->
            if (f.isFile) {
                bytes += f.length()
                if (f.delete()) files++
            }
        }
        val prefs = activity.getSharedPreferences(PREFS, Activity.MODE_PRIVATE)
        val cachedPath = prefs.getString(KEY_CACHED_APK, null)
        if (!cachedPath.isNullOrBlank()) {
            val cached = File(cachedPath)
            if (!cached.isFile) {
                prefs.edit().remove(KEY_CACHED_APK).remove(KEY_MIRROR_PATH).apply()
            }
        }
        return JSONObject()
            .put("ok", true)
            .put("files", files)
            .put("bytes", bytes)
            .put("message", if (files > 0) "已清理 ${formatBytes(bytes)}" else "暂无缓存可清理")
    }

    private fun formatBytes(bytes: Long): String {
        if (bytes < 1024) return "$bytes B"
        if (bytes < 1024 * 1024) return "${bytes / 1024} KB"
        return String.format("%.1f MB", bytes / (1024.0 * 1024.0))
    }

    private fun downloadApk(
        activity: Activity,
        device: DiscoveredDevice,
        downloadPath: String,
        filename: String,
        onProgress: (Long, Long) -> Unit,
    ): File {
        val baseUrl = PcEndpoint.resolveApiBase(device)
        val url = if (downloadPath.startsWith("http")) {
            downloadPath
        } else {
            "$baseUrl$downloadPath"
        }
        val request = Request.Builder().url(url).get().build()
        http.newCall(request).execute().use { resp ->
            if (!resp.isSuccessful) {
                error(parseError(resp.body?.string().orEmpty(), resp.code) ?: "下载失败 (${resp.code})")
            }
            val body = resp.body ?: error("下载内容为空")
            val total = body.contentLength().coerceAtLeast(0L)
            val out = updatesDir(activity).apply { mkdirs() }
            val safeName = filename.ifBlank { "hantransfer-update.apk" }
                .replace(Regex("[^a-zA-Z0-9._-]"), "_")
            val apk = File(out, safeName)
            body.byteStream().use { input ->
                FileOutputStream(apk).use { output ->
                    val buf = ByteArray(65536)
                    var done = 0L
                    while (true) {
                        val n = input.read(buf)
                        if (n <= 0) break
                        output.write(buf, 0, n)
                        done += n
                        onProgress(done, if (total > 0L) total else done)
                    }
                }
            }
            if (apk.length() < 10_000L) error("下载文件过小，可能不是有效 APK")
            val mirror = mirrorToDownloads(activity, apk)
            rememberCachedApk(activity, apk, mirror)
            return apk
        }
    }

    private fun mirrorToDownloads(activity: Activity, apk: File): String? {
        mirrorViaMediaStore(activity, apk)?.let { return it }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && !Environment.isExternalStorageManager()) {
            return null
        }
        return runCatching {
            val downloads = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
            val dir = File(downloads, "hantransfer").apply { mkdirs() }
            val mirror = File(dir, apk.name)
            apk.inputStream().use { input ->
                mirror.outputStream().use { output -> input.copyTo(output) }
            }
            mirror.absolutePath
        }.getOrNull()
    }

    private fun mirrorViaMediaStore(activity: Activity, apk: File): String? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) return null
        return runCatching {
            val resolver = activity.contentResolver
            val values = ContentValues().apply {
                put(MediaStore.Downloads.DISPLAY_NAME, apk.name)
                put(MediaStore.Downloads.MIME_TYPE, "application/vnd.android.package-archive")
                put(MediaStore.Downloads.RELATIVE_PATH, "${Environment.DIRECTORY_DOWNLOADS}/hantransfer")
                put(MediaStore.Downloads.IS_PENDING, 1)
            }
            val collection = MediaStore.Downloads.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
            val uri = resolver.insert(collection, values) ?: return null
            resolver.openOutputStream(uri)?.use { out ->
                apk.inputStream().copyTo(out)
            }
            values.clear()
            values.put(MediaStore.Downloads.IS_PENDING, 0)
            resolver.update(uri, values, null, null)
            "Download/hantransfer/${apk.name}"
        }.getOrNull()
    }

    private fun updatesDir(activity: Activity): File =
        File(activity.getExternalFilesDir(null), "updates")

    private fun cachedApkFile(activity: Activity): File? {
        val prefs = activity.getSharedPreferences(PREFS, Activity.MODE_PRIVATE)
        val path = prefs.getString(KEY_CACHED_APK, null)?.takeIf { it.isNotBlank() } ?: return null
        val file = File(path)
        return file.takeIf { it.isFile }
    }

    private fun rememberCachedApk(activity: Activity, apk: File, mirrorPath: String? = null) {
        activity.getSharedPreferences(PREFS, Activity.MODE_PRIVATE)
            .edit()
            .putString(KEY_CACHED_APK, apk.absolutePath)
            .putString(KEY_MIRROR_PATH, mirrorPath)
            .apply()
    }

    private fun launchInstallUi(activity: Activity, apkFile: File) {
        if (!apkFile.isFile) error("安装包不存在：${apkFile.absolutePath}")
        val uri = ApkFileProvider.uriFor(activity, apkFile)
        val errorRef = AtomicReference<String?>(null)
        val latch = CountDownLatch(1)
        activity.runOnUiThread {
            try {
                startInstallIntent(activity, uri)
            } catch (e: Exception) {
                errorRef.set(e.message ?: "无法打开安装界面")
            } finally {
                latch.countDown()
            }
        }
        if (!latch.await(10, TimeUnit.SECONDS)) {
            error("打开安装界面超时。请点「下载并安装」重试")
        }
        errorRef.get()?.let { error(it) }
    }

    private fun startInstallIntent(activity: Activity, uri: Uri) {
        val base = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, "application/vnd.android.package-archive")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        // MIUI: prefer chooser so user can pick MT/文件管理器 if com.miui.packageinstaller fails
        if (isXiaomiFamily()) {
            activity.startActivity(Intent.createChooser(base, "安装 hantransfer"))
            return
        }
        for (pkg in INSTALLER_PACKAGES) {
            val targeted = Intent(base).setPackage(pkg)
            if (targeted.resolveActivity(activity.packageManager) != null) {
                activity.startActivity(targeted)
                return
            }
        }
        try {
            activity.startActivity(base)
            return
        } catch (_: ActivityNotFoundException) {
            // fall through
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            val install = Intent(Intent.ACTION_INSTALL_PACKAGE).apply {
                setDataAndType(uri, "application/vnd.android.package-archive")
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            try {
                activity.startActivity(install)
                return
            } catch (_: ActivityNotFoundException) {
                // fall through
            }
        }
        activity.startActivity(Intent.createChooser(base, "安装 hantransfer"))
    }

    private fun isXiaomiFamily(): Boolean {
        val brand = Build.BRAND.orEmpty()
        val manufacturer = Build.MANUFACTURER.orEmpty()
        return brand.equals("Xiaomi", ignoreCase = true) ||
            brand.equals("Redmi", ignoreCase = true) ||
            brand.equals("POCO", ignoreCase = true) ||
            manufacturer.equals("Xiaomi", ignoreCase = true)
    }

    private fun parseError(text: String, code: Int): String? {
        val parsed = runCatching {
            val root = JSONObject(text)
            val err = root.optJSONObject("error")
            Pair(err?.optString("code"), err?.optString("message"))
        }.getOrNull()
        if (code == 404 || parsed?.first == "APK_NOT_FOUND") {
            return "PC 端未找到 APK。请在电脑上运行 npm run hantransfer:apk，并确认 hantransfer 已启动"
        }
        return parsed?.second?.takeIf { it.isNotBlank() }
    }

    private const val PREFS = "hantransfer_update"
    private const val KEY_CACHED_APK = "cached_apk_path"
    private const val KEY_MIRROR_PATH = "mirror_apk_path"
}
