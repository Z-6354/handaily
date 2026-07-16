package com.handaily.hantransfer

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import java.io.File

data class AzurlaneFile(
    val uri: Uri?,
    val file: File?,
    val displayName: String,
    val relativePath: String,
    val category: String,
)

class SafFolderStore(context: Context) {
    private val prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun savedTreeUri(): Uri? = prefs.getString(KEY_TREE_URI, null)?.let(Uri::parse)

    fun savedDirectPath(): File? =
        prefs.getString(KEY_DIRECT_PATH, null)?.let(::File)

    fun savedDocumentId(): String? =
        prefs.getString(KEY_DOCUMENT_ID, null)?.takeIf { it.isNotBlank() }

    fun savedMtProvider(): Pair<String, String>? {
        val authority = prefs.getString(KEY_MT_AUTHORITY, null)?.takeIf { it.isNotBlank() } ?: return null
        val docId = prefs.getString(KEY_MT_DOCUMENT_ID, null)?.takeIf { it.isNotBlank() } ?: return null
        return authority to docId
    }

    fun savedTreeDocumentId(): String? =
        prefs.getString(KEY_TREE_DOC_ID, null)?.takeIf { it.isNotBlank() }

    fun saveTreeUri(uri: Uri, documentId: String? = null) {
        val docId = documentId?.takeIf { it.isNotBlank() }
            ?: DocumentsContract.getTreeDocumentId(uri)
        prefs.edit()
            .putString(KEY_TREE_URI, uri.toString())
            .putString(KEY_TREE_DOC_ID, docId)
            .remove(KEY_DIRECT_PATH)
            .remove(KEY_DOCUMENT_ID)
            .remove(KEY_MT_AUTHORITY)
            .remove(KEY_MT_DOCUMENT_ID)
            .apply()
    }

    fun saveDirectPath(dir: File) {
        prefs.edit()
            .putString(KEY_DIRECT_PATH, dir.absolutePath)
            .remove(KEY_TREE_URI)
            .remove(KEY_TREE_DOC_ID)
            .remove(KEY_DOCUMENT_ID)
            .remove(KEY_MT_AUTHORITY)
            .remove(KEY_MT_DOCUMENT_ID)
            .apply()
    }

    fun saveDocumentRoot(documentId: String, displayPath: File) {
        prefs.edit()
            .putString(KEY_DOCUMENT_ID, documentId)
            .putString(KEY_DIRECT_PATH, displayPath.absolutePath)
            .remove(KEY_TREE_URI)
            .remove(KEY_TREE_DOC_ID)
            .remove(KEY_MT_AUTHORITY)
            .remove(KEY_MT_DOCUMENT_ID)
            .apply()
    }

    fun saveMtProviderRoot(authority: String, documentId: String, displayPath: File) {
        prefs.edit()
            .putString(KEY_MT_AUTHORITY, authority)
            .putString(KEY_MT_DOCUMENT_ID, documentId)
            .putString(KEY_DIRECT_PATH, displayPath.absolutePath)
            .remove(KEY_TREE_URI)
            .remove(KEY_TREE_DOC_ID)
            .remove(KEY_DOCUMENT_ID)
            .apply()
    }

    fun clearTreeUri() {
        prefs.edit()
            .remove(KEY_TREE_URI)
            .remove(KEY_TREE_DOC_ID)
            .apply()
    }

    companion object {
        const val PREFS = "hantransfer_azurlane"
        const val KEY_TREE_URI = "assetbundles_tree_uri"
        const val KEY_TREE_DOC_ID = "assetbundles_tree_doc_id"
        const val KEY_DIRECT_PATH = "assetbundles_direct_path"
        const val KEY_DOCUMENT_ID = "assetbundles_document_id"
        const val KEY_MT_AUTHORITY = "mt_provider_authority"
        const val KEY_MT_DOCUMENT_ID = "mt_provider_document_id"

        fun buildAzDocumentUri(documentId: String): Uri? {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return null
            return runCatching {
                DocumentsContract.buildDocumentUri(
                    StorageAccess.DOCUMENT_AUTHORITY,
                    documentId,
                )
            }.getOrNull()
        }

        fun buildAzLive2dPickerUri(): Uri? =
            buildAzDocumentUri("primary:Android/data/com.bilibili.azurlane/files/AssetBundles/live2d")

        fun buildAzAssetBundlesPickerUri(): Uri? =
            buildAzDocumentUri("primary:Android/data/com.bilibili.azurlane/files/AssetBundles")

        /** 优先定位到 Android/data，系统不允许时退回 Android 根目录。 */
        fun buildAzInitialUri(): Uri? {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return null
            return buildAzDataInitialUri() ?: runCatching {
                DocumentsContract.buildTreeDocumentUri(
                    StorageAccess.DOCUMENT_AUTHORITY,
                    "primary:Android",
                )
            }.getOrNull()
        }

        fun buildAzDataInitialUri(): Uri? {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return null
            return runCatching {
                DocumentsContract.buildTreeDocumentUri(
                    StorageAccess.DOCUMENT_AUTHORITY,
                    "primary:Android/data",
                )
            }.getOrNull()
        }
    }
}

internal object TreeDocuments {
    private val PROJECTION = arrayOf(
        DocumentsContract.Document.COLUMN_DOCUMENT_ID,
        DocumentsContract.Document.COLUMN_DISPLAY_NAME,
        DocumentsContract.Document.COLUMN_MIME_TYPE,
    )

    fun rootName(context: Context, treeUri: Uri): String {
        val rootId = DocumentsContract.getTreeDocumentId(treeUri)
        return queryName(context, treeUri, rootId) ?: treeUri.lastPathSegment.orEmpty()
    }

    fun queryName(context: Context, treeUri: Uri, documentId: String): String? {
        val uri = DocumentsContract.buildDocumentUriUsingTree(treeUri, documentId)
        context.contentResolver.query(uri, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)
            ?.use { cursor ->
                if (cursor.moveToFirst()) {
                    return cursor.getString(0)
                }
            }
        return null
    }

    fun listChildren(context: Context, treeUri: Uri, parentId: String): List<ChildEntry> {
        val childrenUri = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentId)
        val out = mutableListOf<ChildEntry>()
        context.contentResolver.query(childrenUri, PROJECTION, null, null, null)?.use { cursor ->
            val idCol = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
            val nameCol = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_DISPLAY_NAME)
            val mimeCol = cursor.getColumnIndex(DocumentsContract.Document.COLUMN_MIME_TYPE)
            while (cursor.moveToNext()) {
                val id = cursor.getString(idCol) ?: continue
                val name = cursor.getString(nameCol) ?: continue
                val mime = cursor.getString(mimeCol).orEmpty()
                val isDir = mime == DocumentsContract.Document.MIME_TYPE_DIR
                val docUri = DocumentsContract.buildDocumentUriUsingTree(treeUri, id)
                out += ChildEntry(id, name, mime, isDir, docUri)
            }
        }
        return out
    }

    data class ChildEntry(
        val documentId: String,
        val name: String,
        val mimeType: String,
        val isDirectory: Boolean,
        val uri: Uri,
    )
}

object AssetScanner {
    fun rootName(context: Context, treeUri: Uri): String =
        TreeDocuments.rootName(context, treeUri)

    fun describeTree(context: Context, treeUri: Uri, rootDocumentId: String? = null): String? {
        val docId = rootDocumentId ?: DocumentsContract.getTreeDocumentId(treeUri)
        val name = TreeDocuments.queryName(context, treeUri, docId)
            ?: TreeDocuments.rootName(context, treeUri)
        if (name.isEmpty()) {
            return "授权目录已失效，请重新选择 AssetBundles"
        }
        if (!name.contains("AssetBundles", ignoreCase = true)) {
            return "当前目录不是 AssetBundles（$name），请重新选择"
        }
        return null
    }

    fun describeDirectPath(context: Context, dir: File): String? {
        if (!StorageAccess.pathIsDirectory(context, dir)) return "AssetBundles 目录不存在，请重新授权"
        if (!dir.name.contains("AssetBundles", ignoreCase = true) && !dir.name.equals("live2d", ignoreCase = true)) {
            return "当前目录不是 live2d 或 AssetBundles（${dir.name}），请重新授权"
        }
        return null
    }

    fun listFiles(context: Context, treeUri: Uri, categoryFilter: String?): List<AzurlaneFile> {
        val rootId = DocumentsContract.getTreeDocumentId(treeUri)
        return listFiles(context, treeUri, rootId, categoryFilter)
    }

    fun listFiles(
        context: Context,
        treeUri: Uri,
        rootDocumentId: String,
        categoryFilter: String?,
    ): List<AzurlaneFile> {
        val out = mutableListOf<AzurlaneFile>()
        walkChildren(context, treeUri, rootDocumentId, "", categoryFilter, out)
        return out.sortedBy { it.relativePath }
    }

    fun listFilesFromPath(context: Context, root: File, categoryFilter: String?): List<AzurlaneFile> {
        if (!StorageAccess.pathIsDirectory(context, root)) return emptyList()
        val out = mutableListOf<AzurlaneFile>()
        walkFile(context, root, root, "", categoryFilter, out)
        return out.sortedBy { it.relativePath }
    }

    fun listFilesFromDocumentId(context: Context, rootDocumentId: String, categoryFilter: String?): List<AzurlaneFile> {
        val out = mutableListOf<AzurlaneFile>()
        walkDocumentChildren(context, rootDocumentId, "", categoryFilter, out)
        return out.sortedBy { it.relativePath }
    }

    fun listFilesFromMtProvider(
        context: Context,
        authority: String,
        rootDocumentId: String,
        categoryFilter: String?,
    ): List<AzurlaneFile> {
        val out = mutableListOf<AzurlaneFile>()
        walkMtDocumentChildren(context, authority, rootDocumentId, "", categoryFilter, out)
        return out.sortedBy { it.relativePath }
    }

    private fun walkFile(
        context: Context,
        root: File,
        node: File,
        prefix: String,
        categoryFilter: String?,
        out: MutableList<AzurlaneFile>,
    ) {
        val children = StorageAccess.listDirectoryEntriesForScan(context, node) ?: return
        for (child in children) {
            val file = File(child.absolutePath)
            val path = if (prefix.isEmpty()) child.name else "$prefix/${child.name}"
            if (child.isDirectory) {
                walkFile(context, root, file, path, categoryFilter, out)
                continue
            }
            val category = categoryFromPath(path)
            if (categoryFilter == null || category == categoryFilter) {
                val uri = when {
                    child.mtAuthority != null && child.documentId != null ->
                        MtDataProviderAccess.documentUri(child.mtAuthority, child.documentId)
                    child.documentId != null -> StorageAccess.documentUri(child.documentId)
                    else -> null
                }
                val readableFile = StorageAccess.resolveReadableFile(file)
                out += AzurlaneFile(
                    uri = uri,
                    file = readableFile.takeIf { runCatching { it.isFile && it.canRead() }.getOrDefault(false) },
                    displayName = child.name,
                    relativePath = path,
                    category = category,
                )
            }
        }
    }

    private fun walkMtDocumentChildren(
        context: Context,
        authority: String,
        parentDocumentId: String,
        prefix: String,
        categoryFilter: String?,
        out: MutableList<AzurlaneFile>,
    ) {
        val children = MtDataProviderAccess.listChildren(context, authority, parentDocumentId) ?: return
        for (child in children) {
            val path = if (prefix.isEmpty()) child.name else "$prefix/${child.name}"
            val docId = child.documentId
            if (child.isDirectory) {
                if (docId != null) walkMtDocumentChildren(context, authority, docId, path, categoryFilter, out)
                continue
            }
            if (docId == null) continue
            val category = categoryFromPath(path)
            if (categoryFilter == null || category == categoryFilter) {
                out += AzurlaneFile(
                    uri = MtDataProviderAccess.documentUri(authority, docId),
                    file = null,
                    displayName = child.name,
                    relativePath = path,
                    category = category,
                )
            }
        }
    }

    private fun walkDocumentChildren(
        context: Context,
        parentId: String,
        prefix: String,
        categoryFilter: String?,
        out: MutableList<AzurlaneFile>,
    ) {
        val children = StorageAccess.listViaDocumentProvider(context, parentId) ?: return
        for (child in children) {
            val path = if (prefix.isEmpty()) child.name else "$prefix/${child.name}"
            val docId = child.documentId
            if (child.isDirectory) {
                if (docId != null) walkDocumentChildren(context, docId, path, categoryFilter, out)
                continue
            }
            if (docId == null) continue
            val category = categoryFromPath(path)
            if (categoryFilter == null || category == categoryFilter) {
                out += AzurlaneFile(
                    uri = StorageAccess.documentUri(docId),
                    file = null,
                    displayName = child.name,
                    relativePath = path,
                    category = category,
                )
            }
        }
    }

    private fun walkChildren(
        context: Context,
        treeUri: Uri,
        parentId: String,
        prefix: String,
        categoryFilter: String?,
        out: MutableList<AzurlaneFile>,
    ) {
        for (child in TreeDocuments.listChildren(context, treeUri, parentId)) {
            val path = if (prefix.isEmpty()) child.name else "$prefix/${child.name}"
            if (child.isDirectory) {
                walkChildren(context, treeUri, child.documentId, path, categoryFilter, out)
                continue
            }
            val category = categoryFromPath(path)
            if (categoryFilter == null || category == categoryFilter) {
                out += AzurlaneFile(child.uri, null, child.name, path, category)
            }
        }
    }

    private fun categoryFromPath(relative: String): String {
        val lower = relative.lowercase()
        return when {
            lower.contains("/live2d/") || lower.startsWith("live2d/") -> "live2d"
            lower.contains("/spinepainting/") || lower.startsWith("spinepainting/") -> "spinepainting"
            else -> "custom"
        }
    }
}
