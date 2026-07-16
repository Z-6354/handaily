package com.handaily.hantransfer

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.Settings
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.nio.file.Files

object StorageAccess {
    const val DOCUMENT_AUTHORITY = "com.android.externalstorage.documents"
    private const val ZWSP = "\u200b"

    val AZ_CN_PACKAGE = "com.bilibili.azurlane"

    fun azCnLive2dDir(): File = File(
        Environment.getExternalStorageDirectory(),
        "Android/data/$AZ_CN_PACKAGE/files/AssetBundles/live2d",
    )

    fun azCnAssetBundlesDir(): File = File(
        Environment.getExternalStorageDirectory(),
        "Android/data/$AZ_CN_PACKAGE/files/AssetBundles",
    )

    fun findAzLive2dDocumentId(context: Context): String? =
        fileToDocumentId(azCnLive2dDir())?.takeIf { documentExists(context, it) }

    val AZ_PACKAGES = listOf(
        AZ_CN_PACKAGE,
        "com.YoStarJP.AzurLane",
        "com.YoStarEN.AzurLane",
        "com.YoStarKR.AzurLane",
    )

    fun hasAllFilesAccess(context: Context): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Environment.isExternalStorageManager()
        } else {
            @Suppress("DEPRECATION")
            Environment.getExternalStorageState() == Environment.MEDIA_MOUNTED
        }
    }

    fun needsAllFilesAccess(): Boolean = Build.VERSION.SDK_INT >= Build.VERSION_CODES.R

    fun defaultBrowseRoot(): File = storageRoots().firstOrNull() ?: File("/storage/emulated/0")

    fun openAllFilesAccessSettings(context: Context) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return
        val pkgIntent = Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
            data = Uri.parse("package:${context.packageName}")
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        if (runCatching { context.startActivity(pkgIntent) }.isSuccess) return
        context.startActivity(
            Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            },
        )
    }

    fun ensureStartupPermissions(context: Context) {
        // 不在启动时自动跳转系统设置页，避免打断 WebView；由用户在设置页主动授权
    }

    fun probe(context: Context): JSONObject {
        val rootsArr = JSONArray()
        var anyDocumentOk = false
        var anyLive2dDocOk = false
        for (base in storageRoots()) {
            val data = File(base, "Android/data")
            val azPkgDir = File(base, "Android/data/com.bilibili.azurlane")
            val files = File(azPkgDir, "files")
            val ab = File(files, "AssetBundles")
            val live2d = File(ab, "live2d")
            val abDocOk = fileToDocumentId(ab)?.let { documentExists(context, it) } == true
            val live2dDocOk = fileToDocumentId(live2d)?.let { documentExists(context, it) } == true
            if (abDocOk) anyDocumentOk = true
            if (live2dDocOk) anyLive2dDocOk = true
            val sample = listDirNames(context, data, limit = 24)
            rootsArr.put(
                JSONObject()
                    .put("root", base.absolutePath)
                    .put("android_data_exists", pathExists(context, data))
                    .put("android_data_is_dir", pathIsDirectory(context, data))
                    .put("android_data_can_read", data.canRead())
                    .put("android_data_list_ok", sample != null)
                    .put("android_data_count", sample?.size ?: -1)
                    .put("sample_dirs", JSONArray().apply { sample?.forEach { put(it) } })
                    .put("az_pkg_exists", pathExists(context, azPkgDir))
                    .put("az_files_exists", pathExists(context, files))
                    .put("az_files_is_dir", pathIsDirectory(context, files))
                    .put("assetbundles_exists", pathExists(context, ab))
                    .put("assetbundles_is_dir", pathIsDirectory(context, ab))
                    .put("assetbundles_path", ab.absolutePath)
                    .put("assetbundles_document_ok", abDocOk)
                    .put("live2d_path", live2d.absolutePath)
                    .put("live2d_document_ok", live2dDocOk),
            )
        }
        val installed = installedAzPackages(context)
        val found = findAzAssetBundlesDir(context)?.absolutePath ?: ""
        val zeroWidthOk = zeroWidthBypassWorks(context)
        val mtProviders = MtDataProviderAccess.installedProviders(context)
        val summary = buildProbeSummary(
            context = context,
            installed = installed,
            found = found,
            anyDocumentOk = anyDocumentOk,
            anyLive2dDocOk = anyLive2dDocOk,
            zeroWidthOk = zeroWidthOk,
            mtProviders = mtProviders,
        )
        return JSONObject()
            .put("all_files_access", hasAllFilesAccess(context))
            .put("zero_width_bypass", zeroWidthOk)
            .put("mt_providers", JSONArray(mtProviders))
            .put("installed_az_packages", JSONArray(installed))
            .put("found_assetbundles", found)
            .put("summary", summary.optString("text"))
            .put("recommendation", summary.optString("recommendation"))
            .put("roots", rootsArr)
    }

    private fun buildProbeSummary(
        context: Context,
        installed: List<String>,
        found: String,
        anyDocumentOk: Boolean,
        anyLive2dDocOk: Boolean,
        zeroWidthOk: Boolean,
        mtProviders: List<String>,
    ): JSONObject {
        val lines = mutableListOf<String>()
        var recommendation = ""
        when {
            found.isNotBlank() -> {
                lines += "✓ 已找到 AssetBundles"
                lines += found
                recommendation = "点「发送 live2d」即可"
            }
            anyLive2dDocOk -> {
                lines += "✓ 系统文档接口可访问 live2d 目录"
                recommendation = "点「重新扫描」后发送 live2d"
            }
            anyDocumentOk -> {
                lines += "△ 可访问 AssetBundles，但 live2d 可能为空"
                recommendation = "请先打开碧蓝航线下载皮肤资源"
            }
            zeroWidthOk -> {
                lines += "✓ 零宽字符路径可用（与 MT 管理器同款技巧）"
                lines += "✓ 已安装：${installed.joinToString()}"
                recommendation = "点「重新扫描」或浏览内部存储进入 live2d"
            }
            mtProviders.isNotEmpty() -> {
                lines += "✓ 检测到 MT 本地存储：${mtProviders.joinToString()}"
                lines += "（游戏 APK 已注入文件提供器）"
                recommendation = "点「重新扫描」后发送 live2d"
            }
            installed.isEmpty() -> {
                lines += "✗ 未检测到碧蓝航线安装包"
                lines += "（若已安装，请更新 App 后重试）"
                recommendation = "确认游戏已安装并打开过一次"
            }
            else -> {
                lines += "✓ 已安装：${installed.joinToString()}"
                lines += "✗ 无法读取 Android/data"
                lines += "请开启「所有文件访问」后返回碧蓝页"
                recommendation = "设置 → 所有文件访问 → 开启 hantransfer"
            }
        }
        return JSONObject()
            .put("text", lines.joinToString("\n"))
            .put("recommendation", recommendation)
    }

    fun browseDir(context: Context, path: String?, safTreeUri: Uri? = null): JSONObject {
        if (!hasAllFilesAccessForBrowse()) {
            return JSONObject()
                .put("ok", false)
                .put("error", "需要开启「所有文件访问」才能浏览内部存储")
                .put("need_files_access", true)
        }
        val target = resolveBrowseTarget(path) ?: return JSONObject()
            .put("ok", false)
            .put("error", "路径无效或不在允许范围内")
        if (isRestrictedAndroidPath(target)) {
            return browseRestrictedAndroidDir(context, target)
        }
        if (!pathExists(context, target)) {
            return JSONObject().put("ok", false).put("error", "目录不存在：${target.absolutePath}")
        }
        if (!pathIsDirectory(context, target)) {
            return JSONObject().put("ok", false).put("error", "不是目录：${target.name}")
        }
        val parent = target.parentFile?.takeIf { isPathAllowed(it) }?.absolutePath ?: ""
        var entries = listDirectoryEntries(context, target)
        var via = "file"
        if (entries.isNullOrEmpty()) {
            syntheticKnownChildren(context, target)?.let {
                entries = it
                via = "known"
            }
        }
        if (entries.isNullOrEmpty() && safTreeUri != null) {
            listDirectoryEntriesViaSaf(context, safTreeUri, target)?.let {
                entries = it
                via = "saf"
            }
        }
        return buildBrowseSuccess(context, target, parent, entries, via)
    }

    private fun browseRestrictedAndroidDir(context: Context, target: File): JSONObject {
        val parent = target.parentFile?.takeIf { isPathAllowed(it) }?.absolutePath ?: ""
        var entries: List<DirEntry>? = null
        var via = "mt_provider"
        resolveMtDocument(context, target)?.let { (authority, docId) ->
            entries = MtDataProviderAccess.listChildren(context, authority, docId)
        }
        if (entries.isNullOrEmpty()) {
            entries = listViaZeroWidthBypass(target)
            via = "zero_width"
        }
        if (entries.isNullOrEmpty()) {
            fileToDocumentId(target)?.let { docId ->
                entries = listViaDocumentProvider(context, docId)
                via = "document"
            }
        }
        if (entries.isNullOrEmpty()) {
            syntheticKnownChildren(context, target)?.let {
                entries = it
                via = "known"
            }
        }
        if (entries.isNullOrEmpty() && fileToDocumentId(target)?.let { documentExists(context, it) } == true) {
            entries = emptyList()
            via = "document"
        }
        if (entries != null) {
            return buildBrowseSuccess(context, target, parent, entries, via)
        }
        return JSONObject()
            .put("ok", false)
            .put("path", target.absolutePath)
            .put("shortcuts", JSONArray())
            .put(
                "error",
                "无法读取 live2d 目录。\n请确认已开启「所有文件访问」且碧蓝航线已下载皮肤资源",
            )
            .put("need_files_access", !hasAllFilesAccess(context))
    }

    private fun buildBrowseSuccess(
        context: Context,
        target: File,
        parent: String,
        entries: List<DirEntry>?,
        via: String,
    ): JSONObject {
        val finalEntries = entries ?: return JSONObject()
            .put("ok", false)
            .put("path", target.absolutePath)
            .put("need_files_access", !hasAllFilesAccess(context))
            .put("shortcuts", JSONArray())
            .put("error", buildBrowseListError(context, target))
        val arr = JSONArray()
        finalEntries.sortedWith(compareBy({ !it.isDirectory }, { it.name.lowercase() })).forEach { entry ->
            arr.put(
                JSONObject()
                    .put("name", entry.name)
                    .put("path", entry.absolutePath)
                    .put("is_dir", entry.isDirectory)
                    .put("size", if (entry.isDirectory) 0L else entry.size)
                    .put("document_id", entry.documentId.orEmpty()),
            )
        }
        return JSONObject()
            .put("ok", true)
            .put("path", target.absolutePath)
            .put("parent", parent)
            .put("via", via)
            .put("shortcuts", JSONArray())
            .put("entries", arr)
    }

    private fun isRestrictedAndroidPath(file: File): Boolean {
        val path = runCatching { file.absolutePath }.getOrDefault("")
        return path.contains("/Android/data") ||
            path.contains("/Android/obb") ||
            (file.name == "data" && file.parentFile?.name == "Android") ||
            (file.name == "Android" && file.parentFile?.let { isPathAllowed(it) && !it.absolutePath.contains("/Android") } == true)
    }

    fun azBrowseShortcuts(context: Context): JSONArray {
        val arr = JSONArray()
        val seen = linkedSetOf<String>()
        fun add(label: String, path: String) {
            if (!seen.add(path)) return
            arr.put(JSONObject().put("label", label).put("path", path))
        }
        for (base in storageRoots()) {
            for (pkg in installedPackages(context)) {
                val ab = File(base, "Android/data/$pkg/files/AssetBundles")
                if (pathIsDirectory(context, ab)) {
                    add("AssetBundles", ab.absolutePath)
                    val live2d = File(ab, "live2d")
                    if (pathIsDirectory(context, live2d)) add("Live2D", live2d.absolutePath)
                    val spine = File(ab, "spinepainting")
                    if (pathIsDirectory(context, spine)) add("Spine", spine.absolutePath)
                    val files = File(base, "Android/data/$pkg/files")
                    if (pathIsDirectory(context, files)) add("碧蓝 files", files.absolutePath)
                }
            }
        }
        return arr
    }

    fun installedAzPackages(context: Context): List<String> {
        val found = linkedSetOf<String>()
        found += installedPackages(context)
        if (hasAllFilesAccessForBrowse()) {
            for (base in storageRoots()) {
                val dataRoot = File(base, "Android/data")
                if (!pathIsDirectory(context, dataRoot)) continue
                listDirectoryEntries(context, dataRoot)?.forEach { dir ->
                    if (!dir.isDirectory) return@forEach
                    val name = dir.name.lowercase()
                    if (name.contains("azurlane") || name.contains("azur")) {
                        found += dir.name
                    }
                }
            }
        }
        return found.sorted()
    }

    fun findAzAssetBundlesDir(context: Context): File? {
        MtDataProviderAccess.findAssetBundles(context)?.let { return it.displayPath }
        findAzDocumentId(context)?.let { docId ->
            return documentIdToFile(docId)
        }
        for (base in storageRoots()) {
            for (pkg in installedPackages(context)) {
                assetBundlesUnder(context, base, pkg)?.let { return it }
            }
            for (pkg in AZ_PACKAGES) {
                assetBundlesUnder(context, base, pkg)?.let { return it }
            }
            scanAndroidData(context, base)?.let { return it }
            deepFindAssetBundles(context, base)?.let { return it }
        }
        return null
    }

    fun findAzDocumentId(context: Context): String? {
        for (pkg in installedPackages(context)) {
            for (base in storageRoots()) {
                val ab = File(base, "Android/data/$pkg/files/AssetBundles")
                fileToDocumentId(ab)?.let { docId ->
                    if (documentExists(context, docId)) return docId
                }
            }
        }
        for (pkg in AZ_PACKAGES) {
            for (base in storageRoots()) {
                val ab = File(base, "Android/data/$pkg/files/AssetBundles")
                fileToDocumentId(ab)?.let { docId ->
                    if (documentExists(context, docId)) return docId
                }
            }
        }
        return null
    }

    fun fileToDocumentId(file: File): String? {
        val targetPath = runCatching { file.canonicalPath }.getOrNull() ?: return null
        for (root in storageRoots()) {
            val rootPath = runCatching { root.canonicalPath }.getOrNull() ?: continue
            if (!targetPath.startsWith(rootPath)) continue
            val relative = targetPath.removePrefix(rootPath).trimStart('/')
            return "primary:$relative"
        }
        return null
    }

    fun documentIdToFile(documentId: String, base: File = defaultBrowseRoot()): File {
        val relative = documentId.removePrefix("primary:").trimStart('/')
        return File(base, relative)
    }

    fun documentUri(documentId: String): Uri =
        DocumentsContract.buildDocumentUri(DOCUMENT_AUTHORITY, documentId)

    fun documentExists(context: Context, documentId: String): Boolean {
        return runCatching {
            context.contentResolver.openFileDescriptor(documentUri(documentId), "r")?.use { true } ?: false
        }.getOrDefault(false) ||
            listViaDocumentProvider(context, documentId)?.isNotEmpty() == true
    }

    internal fun listViaDocumentProvider(context: Context, documentId: String): List<DirEntry>? {
        val childrenUri = DocumentsContract.buildChildDocumentsUri(DOCUMENT_AUTHORITY, documentId)
        val projection = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
        )
        val out = mutableListOf<DirEntry>()
        runCatching {
            context.contentResolver.query(childrenUri, projection, null, null, null)?.use { cursor ->
                val idCol = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
                val nameCol = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_DISPLAY_NAME)
                val mimeCol = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_MIME_TYPE)
                while (cursor.moveToNext()) {
                    val id = cursor.getString(idCol) ?: continue
                    val name = cursor.getString(nameCol) ?: continue
                    val mime = cursor.getString(mimeCol).orEmpty()
                    val isDir = mime == DocumentsContract.Document.MIME_TYPE_DIR
                    val path = documentIdToFile(id).absolutePath
                    out += DirEntry(name, path, isDir, 0L, id)
                }
            }
        }
        return out.takeIf { it.isNotEmpty() }
    }

    fun explainAzMissing(context: Context): String {
        if (!isPackageInstalled(context.packageManager, AZ_CN_PACKAGE)) {
            return "未检测到碧蓝航线（国服），请先安装并打开游戏"
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && !hasAllFilesAccess(context)) {
            return "请开启 hantransfer「所有文件访问」"
        }
        return "无法读取 live2d 目录，请确认游戏已下载皮肤资源"
    }

    fun buildPrimaryStorageUri(): Uri? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return null
        return runCatching {
            DocumentsContract.buildDocumentUri(
                "com.android.externalstorage.documents",
                "primary:",
            )
        }.getOrNull()
    }

    internal fun listDirectoryEntriesForScan(context: Context, dir: File): List<DirEntry>? =
        listDirectoryEntries(context, dir)

    fun isPathAllowed(file: File): Boolean {
        return runCatching {
            val canonical = file.canonicalPath
            storageRoots().any { root ->
                runCatching { canonical.startsWith(root.canonicalPath) }.getOrDefault(false)
            }
        }.getOrDefault(false)
    }

    private fun hasAllFilesAccessForBrowse(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Environment.isExternalStorageManager()
        } else {
            true
        }
    }

    private fun resolveBrowseTarget(path: String?): File? {
        val target = if (path.isNullOrBlank()) defaultBrowseRoot() else File(path)
        return target.takeIf { isPathAllowed(it) }
    }

    private fun installedPackages(context: Context): List<String> {
        val pm = context.packageManager
        return AZ_PACKAGES.filter { isPackageInstalled(pm, it) }
    }

    fun pathExists(context: Context, file: File): Boolean {
        if (isRestrictedAndroidPath(file)) {
            resolveMtDocument(context, file)?.let { (authority, docId) ->
                if (MtDataProviderAccess.documentExists(context, authority, docId)) return true
            }
            if (pathExistsViaZeroWidth(file)) return true
            fileToDocumentId(file)?.let { if (documentExists(context, it)) return true }
            return false
        }
        if (runCatching { file.exists() }.getOrDefault(false)) return true
        fileToDocumentId(file)?.let { if (documentExists(context, it)) return true }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R && hasAllFilesAccessForBrowse()) {
            if (shellPathType(file.absolutePath) != null) return true
        }
        return false
    }

    fun pathIsDirectory(context: Context, file: File): Boolean {
        if (isRestrictedAndroidPath(file)) {
            resolveMtDocument(context, file)?.let { (authority, docId) ->
                if (MtDataProviderAccess.documentExists(context, authority, docId)) return true
            }
            if (pathIsDirectoryViaZeroWidth(file)) return true
            fileToDocumentId(file)?.let { if (documentExists(context, it)) return true }
            return false
        }
        if (runCatching { file.isDirectory }.getOrDefault(false)) return true
        fileToDocumentId(file)?.let { if (documentExists(context, it)) return true }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R && hasAllFilesAccessForBrowse()) {
            if (shellPathIsDirectory(file.absolutePath)) return true
        }
        return false
    }

    private fun resolveMtDocument(context: Context, file: File): Pair<String, String>? {
        val resolved = MtDataProviderAccess.resolveMtDocument(file) ?: return null
        val pkg = resolved.second.substringBefore('/')
        if (!MtDataProviderAccess.isAvailable(context, pkg)) return null
        return resolved
    }

    internal data class DirEntry(
        val name: String,
        val absolutePath: String,
        val isDirectory: Boolean,
        val size: Long,
        val documentId: String? = null,
        val mtAuthority: String? = null,
    )

    private fun isAndroidDataRoot(dir: File): Boolean =
        dir.name == "data" && dir.parentFile?.name == "Android"

    private fun isAzPackageDir(dir: File): Boolean {
        val parent = dir.parentFile ?: return false
        return parent.name == "data" && dir.name.contains('.')
    }

    private fun isAzFilesDir(dir: File): Boolean =
        dir.name == "files" && dir.parentFile?.name?.contains("azur", ignoreCase = true) == true

    private fun isAssetBundlesDir(dir: File): Boolean =
        dir.name.equals("AssetBundles", ignoreCase = true)

    private fun syntheticKnownChildren(context: Context, dir: File): List<DirEntry>? {
        when {
            isAndroidDataRoot(dir) -> return syntheticAndroidDataEntries(context, dir)
            isAzPackageDir(dir) -> return syntheticSubdirs(context, dir, listOf("files", "cache"))
            isAzFilesDir(dir) -> return syntheticSubdirs(context, dir, listOf("AssetBundles"))
            isAssetBundlesDir(dir) -> return syntheticSubdirs(context, dir, listOf("live2d", "spinepainting", "painting"))
        }
        return null
    }

    private fun syntheticAndroidDataEntries(context: Context, dataRoot: File): List<DirEntry>? {
        val entries = linkedSetOf<DirEntry>()
        for (pkg in installedPackages(context)) addDirEntryIfExists(context, entries, dataRoot, pkg)
        for (pkg in AZ_PACKAGES) addDirEntryIfExists(context, entries, dataRoot, pkg)
        return entries.takeIf { it.isNotEmpty() }?.toList()
    }

    private fun addDirEntryIfExists(
        context: Context,
        out: MutableSet<DirEntry>,
        parent: File,
        name: String,
    ) {
        val child = File(parent, name)
        if (pathIsDirectory(context, child)) {
            out += DirEntry(name, child.absolutePath, true, 0L, fileToDocumentId(child))
        }
    }

    private fun syntheticSubdirs(context: Context, parent: File, names: List<String>): List<DirEntry>? {
        val entries = names.mapNotNull { name ->
            val child = File(parent, name)
            if (pathIsDirectory(context, child)) {
                DirEntry(name, child.absolutePath, true, 0L, fileToDocumentId(child))
            } else {
                null
            }
        }
        return entries.takeIf { it.isNotEmpty() }
    }

    private fun listDirectoryEntries(context: Context, dir: File): List<DirEntry>? {
        if (isRestrictedAndroidPath(dir)) {
            resolveMtDocument(context, dir)?.let { (authority, docId) ->
                MtDataProviderAccess.listChildren(context, authority, docId)?.let {
                    if (it.isNotEmpty()) return it
                }
            }
            listViaZeroWidthBypass(dir)?.let { if (it.isNotEmpty()) return it }
            fileToDocumentId(dir)?.let { docId ->
                listViaDocumentProvider(context, docId)?.let { return it }
            }
            return syntheticKnownChildren(context, dir)
        }
        fileToDocumentId(dir)?.let { docId ->
            listViaDocumentProvider(context, docId)?.let { if (it.isNotEmpty()) return it }
        }
        val viaFile = listViaFileApi(context, dir)
        if (viaFile != null && viaFile.isNotEmpty()) return viaFile
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            listViaNio(context, dir)?.let { if (it.isNotEmpty()) return it }
        }
        fileToDocumentId(dir)?.let { listViaDocumentProvider(context, it) }?.let { return it }
        return viaFile ?: listViaNio(context, dir)
    }

    private fun shouldRetryShellForEmpty(context: Context, dir: File, entries: List<DirEntry>): Boolean {
        if (entries.isNotEmpty()) return false
        if (!dir.absolutePath.contains("/Android/data")) return false
        if (hasAllFilesAccessForBrowse() && shellPathIsDirectory(dir.absolutePath)) return true
        fileToDocumentId(dir)?.let { if (documentExists(context, it)) return true }
        return false
    }

    private fun entryFromFile(context: Context, child: File): DirEntry =
        DirEntry(
            name = child.name,
            absolutePath = child.absolutePath,
            isDirectory = pathIsDirectory(context, child),
            size = if (pathIsDirectory(context, child)) 0L else runCatching { child.length() }.getOrDefault(0L),
            documentId = fileToDocumentId(child),
        )

    private fun listViaFileApi(context: Context, dir: File): List<DirEntry>? {
        val children = dir.listFiles() ?: return null
        return children.map { entryFromFile(context, it) }
    }

    private fun listViaNio(context: Context, dir: File): List<DirEntry>? =
        runCatching {
            Files.newDirectoryStream(dir.toPath()).use { stream ->
                stream.map { entryFromFile(context, it.toFile()) }.toList()
            }
        }.getOrNull()

    private fun listViaShell(path: String): List<DirEntry>? {
        val commands = listOf(
            arrayOf("/system/bin/ls", "-1", path),
            arrayOf("/system/bin/ls", path),
            arrayOf("sh", "-c", "ls -1 ${shellQuote(path)}"),
        )
        for (cmd in commands) {
            val names = execShellLines(*cmd) ?: continue
            if (names.isEmpty() && !shellPathIsDirectory(path)) continue
            val entries = names.mapNotNull { name -> entryFromShell(path, name) }
            if (entries.isNotEmpty() || names.isEmpty()) return entries
        }
        return null
    }

    private fun entryFromShell(parentPath: String, name: String): DirEntry? {
        if (name.isBlank() || name == "." || name == "..") return null
        val file = File(parentPath, name)
        val type = shellPathType(file.absolutePath)
        val isDir = when (type) {
            "d" -> true
            "f" -> false
            else -> runCatching { file.isDirectory }.getOrDefault(false)
        }
        val size = if (isDir) {
            0L
        } else {
            runCatching { file.length() }.getOrElse {
                shellFileSize(file.absolutePath) ?: 0L
            }
        }
        return DirEntry(
            name = name,
            absolutePath = file.absolutePath,
            isDirectory = isDir,
            size = size,
        )
    }

    private fun execShellLines(vararg cmd: String): List<String>? {
        return runCatching {
            val process = ProcessBuilder(*cmd)
                .redirectErrorStream(true)
                .start()
            val lines = process.inputStream.bufferedReader().readLines()
            if (process.waitFor() != 0) return null
            lines.map { it.trim() }.filter { it.isNotEmpty() }
        }.getOrNull()
    }

    private fun shellPathType(path: String): String? {
        val lines = execShellLines(
            "sh",
            "-c",
            "if [ -d ${shellQuote(path)} ]; then echo d; elif [ -f ${shellQuote(path)} ]; then echo f; fi",
        ) ?: return null
        return lines.firstOrNull()
    }

    private fun shellPathIsDirectory(path: String): Boolean = shellPathType(path) == "d"

    private fun shellFileSize(path: String): Long? {
        val lines = execShellLines("sh", "-c", "stat -c %s ${shellQuote(path)} 2>/dev/null || wc -c < ${shellQuote(path)}")
        return lines?.firstOrNull()?.trim()?.toLongOrNull()
    }

    private fun shellQuote(value: String): String =
        "'" + value.replace("'", "'\\''") + "'"

    private fun listDirectoryEntriesViaSaf(context: Context, treeUri: Uri, target: File): List<DirEntry>? {
        val docId = fileToDocumentId(target) ?: return null
        val treeRootId = DocumentsContract.getTreeDocumentId(treeUri)
        if (!isDocumentUnderTree(docId, treeRootId)) return null
        val children = TreeDocuments.listChildren(context, treeUri, docId)
        if (children.isEmpty() && !safDocumentExists(context, treeUri, docId)) return null
        return children.map { child ->
            val childPath = File(target, child.name).absolutePath
            DirEntry(
                name = child.name,
                absolutePath = childPath,
                isDirectory = child.isDirectory,
                size = 0L,
            )
        }
    }

    private fun pathToPrimaryDocumentId(file: File): String? = fileToDocumentId(file)

    private fun isDocumentUnderTree(docId: String, treeRootId: String): Boolean {
        val doc = docId.removeSuffix("/")
        val root = treeRootId.removeSuffix("/")
        return doc == root || doc.startsWith("$root/")
    }

    private fun safDocumentExists(context: Context, treeUri: Uri, documentId: String): Boolean {
        val uri = DocumentsContract.buildDocumentUriUsingTree(treeUri, documentId)
        return runCatching {
            context.contentResolver.query(uri, arrayOf(DocumentsContract.Document.COLUMN_DOCUMENT_ID), null, null, null)
                ?.use { it.moveToFirst() } == true
        }.getOrDefault(false)
    }

    private fun buildBrowseListError(context: Context, target: File): String {
        val path = target.absolutePath
        if (isXiaomiFamily() && path.contains("/Android/data")) {
            return "此目录被小米系统限制，无法逐层浏览。\n" +
                "请用上方「Live2D」快捷按钮，或碧蓝页「从文件管理器选 live2d」"
        }
        if (!hasAllFilesAccess(context)) {
            return "无法列出目录内容，请先开启 hantransfer「所有文件访问」"
        }
        return "无法列出目录内容（可能被系统限制，部分机型需在设置中额外开启）"
    }

    private fun isXiaomiFamily(): Boolean {
        val brand = Build.BRAND.orEmpty()
        val manufacturer = Build.MANUFACTURER.orEmpty()
        return brand.equals("Xiaomi", ignoreCase = true) ||
            brand.equals("Redmi", ignoreCase = true) ||
            brand.equals("POCO", ignoreCase = true) ||
            manufacturer.equals("Xiaomi", ignoreCase = true)
    }

    private fun listDirNames(context: Context, dir: File, limit: Int): List<String>? {
        val entries = listDirectoryEntries(context, dir) ?: return null
        return entries.sortedBy { it.name.lowercase() }.take(limit).map { it.name }
    }

    private fun assetBundlesUnder(context: Context, base: File, pkg: String): File? {
        val dir = File(base, "Android/data/$pkg/files/AssetBundles")
        return dir.takeIf { pathIsDirectory(context, it) }
    }

    private fun scanAndroidData(context: Context, base: File): File? {
        for (pkg in AZ_PACKAGES) {
            assetBundlesUnder(context, base, pkg)?.let { return it }
        }
        val dataRoot = File(base, "Android/data")
        if (!pathIsDirectory(context, dataRoot)) return null
        for (pkgDir in listDirectoryEntries(context, dataRoot) ?: return null) {
            if (!pkgDir.isDirectory) continue
            val name = pkgDir.name.lowercase()
            if (!name.contains("azurlane") && !name.contains("azur")) continue
            val ab = File(pkgDir.absolutePath, "files/AssetBundles")
            if (pathIsDirectory(context, ab)) return ab
        }
        return null
    }

    private fun deepFindAssetBundles(context: Context, base: File): File? {
        val dataRoot = File(base, "Android/data")
        if (!pathIsDirectory(context, dataRoot)) return null
        for (pkgDir in listDirectoryEntries(context, dataRoot) ?: return null) {
            if (!pkgDir.isDirectory) continue
            walkForAssetBundles(context, File(pkgDir.absolutePath), 0)?.let { return it }
        }
        return null
    }

    private fun walkForAssetBundles(context: Context, dir: File, depth: Int): File? {
        if (depth > 10) return null
        if (dir.name.equals("AssetBundles", ignoreCase = true) && pathIsDirectory(context, dir)) return dir
        if (!pathIsDirectory(context, dir)) return null
        for (child in listDirectoryEntries(context, dir) ?: return null) {
            if (!child.isDirectory) continue
            walkForAssetBundles(context, File(child.absolutePath), depth + 1)?.let { return it }
        }
        return null
    }

    private fun isPackageInstalled(pm: PackageManager, packageName: String): Boolean {
        return runCatching {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                pm.getPackageInfo(packageName, PackageManager.PackageInfoFlags.of(0))
            } else {
                @Suppress("DEPRECATION")
                pm.getPackageInfo(packageName, 0)
            }
            true
        }.getOrDefault(false)
    }

    fun resolveReadableFile(file: File): File {
        if (runCatching { file.isFile && file.canRead() }.getOrDefault(false)) return file
        val bypass = toZeroWidthBypassPath(file) ?: return file
        return bypass.takeIf { runCatching { it.isFile && it.canRead() }.getOrDefault(false) } ?: file
    }

    fun zeroWidthBypassWorks(context: Context): Boolean {
        if (!hasAllFilesAccessForBrowse()) return false
        for (base in storageRoots()) {
            val data = File(base, "Android/data")
            if (pathIsDirectoryViaZeroWidth(data)) return true
        }
        return false
    }

    private fun toZeroWidthBypassPath(file: File): File? {
        val path = runCatching { file.absolutePath }.getOrNull() ?: return null
        if (!path.contains("/Android/data") && !path.contains("/Android/obb")) return null
        val bypass = path
            .replace("/Android/data", "/Android/${ZWSP}data")
            .replace("/Android/obb", "/Android/${ZWSP}obb")
        return if (bypass == path) null else File(bypass)
    }

    private fun canonicalDisplayPath(file: File): String =
        file.absolutePath
            .replace("/Android/${ZWSP}data", "/Android/data")
            .replace("/Android/${ZWSP}obb", "/Android/obb")

    private fun pathExistsViaZeroWidth(file: File): Boolean {
        val bypass = toZeroWidthBypassPath(file) ?: return false
        return runCatching { bypass.exists() }.getOrDefault(false)
    }

    private fun pathIsDirectoryViaZeroWidth(file: File): Boolean {
        val bypass = toZeroWidthBypassPath(file) ?: return false
        return runCatching { bypass.isDirectory }.getOrDefault(false)
    }

    private fun listViaZeroWidthBypass(dir: File): List<DirEntry>? {
        val bypassDir = toZeroWidthBypassPath(dir) ?: return null
        val children = runCatching { bypassDir.listFiles() }.getOrNull() ?: return null
        return children.map { child ->
            val displayPath = canonicalDisplayPath(child)
            DirEntry(
                name = child.name,
                absolutePath = displayPath,
                isDirectory = child.isDirectory,
                size = if (child.isDirectory) 0L else runCatching { child.length() }.getOrDefault(0L),
                documentId = fileToDocumentId(File(displayPath)),
            )
        }
    }

    fun storageRoots(): List<File> {
        val out = linkedSetOf<File>()
        Environment.getExternalStorageDirectory()?.let { out += it }
        for (path in listOf("/storage/emulated/0", "/sdcard", "/storage/self/primary")) {
            out += File(path)
        }
        return out.filter { it.isDirectory }
    }
}
