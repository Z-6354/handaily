package com.handaily.hantransfer

import android.content.Context
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import java.io.File

/**
 * 读取已通过 MT 管理器「注入文件提供器」的游戏数据。
 * Authority: `{packageName}.MTDataFilesProvider`
 * DocumentId 示例: `com.bilibili.azurlane/android_data/files/AssetBundles/live2d/...`
 */
object MtDataProviderAccess {
    data class MtRoot(
        val authority: String,
        val packageName: String,
        val rootDocumentId: String,
        val displayPath: File,
    )

    private val PROJECTION = arrayOf(
        DocumentsContract.Document.COLUMN_DOCUMENT_ID,
        DocumentsContract.Document.COLUMN_DISPLAY_NAME,
        DocumentsContract.Document.COLUMN_MIME_TYPE,
        DocumentsContract.Document.COLUMN_SIZE,
    )

    fun authorityFor(packageName: String): String = "$packageName.MTDataFilesProvider"

    fun installedProviders(context: Context): List<String> =
        StorageAccess.installedAzPackages(context).filter { isAvailable(context, it) }

    fun isAvailable(context: Context, packageName: String): Boolean {
        val authority = authorityFor(packageName)
        return runCatching {
            context.contentResolver.query(
                DocumentsContract.buildRootsUri(authority),
                arrayOf(DocumentsContract.Root.COLUMN_ROOT_ID),
                null,
                null,
                null,
            )?.use { cursor -> cursor.moveToFirst() } == true
        }.getOrDefault(false)
    }

    fun findAssetBundles(context: Context): MtRoot? {
        for (pkg in StorageAccess.installedAzPackages(context)) {
            if (!isAvailable(context, pkg)) continue
            val authority = authorityFor(pkg)
            val docId = assetBundlesDocumentId(pkg)
            if (!documentExists(context, authority, docId)) continue
            val display = File(
                Environment.getExternalStorageDirectory(),
                "Android/data/$pkg/files/AssetBundles",
            )
            return MtRoot(authority, pkg, docId, display)
        }
        return null
    }

    fun assetBundlesDocumentId(packageName: String): String =
        "$packageName/android_data/files/AssetBundles"

    fun live2dDocumentId(packageName: String): String =
        "$packageName/android_data/files/AssetBundles/live2d"

    fun documentExists(context: Context, authority: String, documentId: String): Boolean {
        if (runCatching {
                context.contentResolver.openFileDescriptor(
                    DocumentsContract.buildDocumentUri(authority, documentId),
                    "r",
                )?.use { true } == true
            }.getOrDefault(false)
        ) {
            return true
        }
        return !listChildren(context, authority, documentId).isNullOrEmpty()
    }

    fun documentUri(authority: String, documentId: String): Uri =
        DocumentsContract.buildDocumentUri(authority, documentId)

    fun resolveMtDocument(file: File): Pair<String, String>? {
        val path = runCatching { file.canonicalPath }.getOrNull() ?: return null
        for (pkg in StorageAccess.AZ_PACKAGES) {
            val marker = "/Android/data/$pkg/"
            val idx = path.indexOf(marker)
            if (idx < 0) continue
            val suffix = path.substring(idx + marker.length)
            val authority = authorityFor(pkg)
            return authority to "$pkg/android_data/$suffix"
        }
        return null
    }

    internal fun listChildren(
        context: Context,
        authority: String,
        parentDocumentId: String,
    ): List<StorageAccess.DirEntry>? {
        val childrenUri = DocumentsContract.buildChildDocumentsUri(authority, parentDocumentId)
        val out = mutableListOf<StorageAccess.DirEntry>()
        runCatching {
            context.contentResolver.query(childrenUri, PROJECTION, null, null, null)?.use { cursor ->
                val idCol = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
                val nameCol = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_DISPLAY_NAME)
                val mimeCol = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_MIME_TYPE)
                val sizeCol = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_SIZE)
                while (cursor.moveToNext()) {
                    val id = cursor.getString(idCol) ?: continue
                    val name = cursor.getString(nameCol) ?: continue
                    val mime = cursor.getString(mimeCol).orEmpty()
                    val isDir = mime == DocumentsContract.Document.MIME_TYPE_DIR
                    val size = if (sizeCol >= 0) cursor.getLong(sizeCol) else 0L
                    val displayPath = mtDocumentIdToDisplayPath(id) ?: continue
                    out += StorageAccess.DirEntry(
                        name = name,
                        absolutePath = displayPath,
                        isDirectory = isDir,
                        size = if (isDir) 0L else size,
                        documentId = id,
                        mtAuthority = authority,
                    )
                }
            }
        }
        return out.takeIf { it.isNotEmpty() }
    }

    fun mtDocumentIdToDisplayPath(documentId: String): String? {
        val slash = documentId.indexOf('/')
        if (slash <= 0) return null
        val pkg = documentId.substring(0, slash)
        val rest = documentId.substring(slash + 1)
        if (!rest.startsWith("android_data")) return null
        val suffix = rest.removePrefix("android_data").trimStart('/')
        val base = File(Environment.getExternalStorageDirectory(), "Android/data/$pkg")
        return if (suffix.isEmpty()) base.absolutePath else File(base, suffix).absolutePath
    }
}
