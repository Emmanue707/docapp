package com.waghitech.docapp.core

import android.content.ContentValues
import android.content.Context
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import java.io.File

object DownloadOutput {
    const val FOLDER_LABEL = "Download/Docapp"

    fun save(
        context: Context,
        source: File,
        displayName: String,
        mime: String,
    ): Result<Uri> =
        runCatching {
            val resolver = context.contentResolver
            val name = displayName.replace(Regex("[^A-Za-z0-9._-]"), "_")
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val values =
                    ContentValues().apply {
                        put(MediaStore.MediaColumns.DISPLAY_NAME, name)
                        put(MediaStore.MediaColumns.MIME_TYPE, mime)
                        put(
                            MediaStore.MediaColumns.RELATIVE_PATH,
                            "${Environment.DIRECTORY_DOWNLOADS}/Docapp",
                        )
                        put(MediaStore.MediaColumns.IS_PENDING, 1)
                    }
                val uri =
                    resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
                        ?: error("MediaStore insert failed")
                resolver.openOutputStream(uri, "wt")?.use { out ->
                    source.inputStream().use { input -> input.copyTo(out) }
                } ?: error("cannot open output stream")
                values.clear()
                values.put(MediaStore.MediaColumns.IS_PENDING, 0)
                resolver.update(uri, values, null, null)
                uri
            } else {
                @Suppress("DEPRECATION")
                val root =
                    Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
                val dir = File(root, "Docapp")
                if (!dir.exists()) {
                    dir.mkdirs()
                }
                val target = File(dir, name)
                source.copyTo(target, overwrite = true)
                val values =
                    ContentValues().apply {
                        put(MediaStore.MediaColumns.DISPLAY_NAME, name)
                        put(MediaStore.MediaColumns.MIME_TYPE, mime)
                        @Suppress("DEPRECATION")
                        put(MediaStore.MediaColumns.DATA, target.absolutePath)
                    }
                val uri =
                    resolver.insert(MediaStore.Files.getContentUri("external"), values)
                        ?: Uri.fromFile(target)
                uri
            }
        }

    fun saveText(
        context: Context,
        text: String,
        displayName: String,
    ): Result<Uri> {
        val tmp = File.createTempFile("docapp-out-", ".txt", context.cacheDir)
        tmp.writeText(text)
        return save(context, tmp, displayName, "text/plain").also { tmp.delete() }
    }
}