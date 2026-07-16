package com.handaily.hantransfer

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import java.io.File

class ApkFileProvider : ContentProvider() {
    override fun onCreate(): Boolean = true

    override fun query(
        uri: Uri,
        projection: Array<out String>?,
        selection: String?,
        selectionArgs: Array<out String>?,
        sortOrder: String?,
    ): Cursor? = null

    override fun getType(uri: Uri): String = APK_MIME

    override fun insert(uri: Uri, values: ContentValues?): Uri? = null

    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0

    override fun update(
        uri: Uri,
        values: ContentValues?,
        selection: String?,
        selectionArgs: Array<out String>?,
    ): Int = 0

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        val ctx = requireNotNull(context) { "provider not ready" }
        val file = resolveFile(ctx, uri)
        return ParcelFileDescriptor.open(file, ParcelFileDescriptor.MODE_READ_ONLY)
    }

    companion object {
        private const val APK_MIME = "application/vnd.android.package-archive"

        fun uriFor(context: Context, file: File): Uri {
            val updatesRoot = updatesRoot(context)
            val canonical = file.canonicalFile
            require(canonical.path.startsWith(updatesRoot.path)) { "APK must live under app updates dir" }
            return Uri.Builder()
                .scheme("content")
                .authority("${context.packageName}.fileprovider")
                .appendPath(canonical.name)
                .build()
        }

        private fun updatesRoot(context: Context): File =
            File(context.getExternalFilesDir(null), "updates").canonicalFile

        private fun resolveFile(context: Context, uri: Uri): File {
            val name = uri.lastPathSegment?.trim().orEmpty()
            require(name.isNotEmpty() && !name.contains("..") && !name.contains('/')) { "invalid apk uri" }
            val file = File(updatesRoot(context), name).canonicalFile
            val root = updatesRoot(context)
            require(file.path.startsWith(root.path)) { "invalid apk path" }
            require(file.isFile) { "apk not found" }
            return file
        }
    }
}
