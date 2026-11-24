package com.waghitech.docapp.core

import android.content.ContentResolver
import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import java.io.File
import java.io.InputStream

object UriFiles {
    fun displayName(resolver: ContentResolver, uri: Uri): String {
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                val idx = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (idx >= 0) {
                    val name = c.getString(idx)
                    if (!name.isNullOrBlank()) {
                        return name
                    }
                }
            }
        }
        return uri.lastPathSegment ?: "document"
    }

    fun mime(resolver: ContentResolver, uri: Uri): String {
        val t = resolver.getType(uri)
        if (!t.isNullOrBlank()) {
            return t
        }
        val ext = MimeTypeMap.getFileExtensionFromUrl(uri.toString())
        return MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext) ?: "application/octet-stream"
    }

    fun isPdf(mime: String): Boolean = mime == "application/pdf"

    fun isImage(mime: String): Boolean = mime.startsWith("image/")

    fun copyToCache(context: Context, uri: Uri, name: String): File {
        val dir = File(context.cacheDir, "work")
        if (!dir.exists()) {
            dir.mkdirs()
        }
        val safe = name.replace(Regex("[^A-Za-z0-9._-]"), "_")
        val out = File(dir, safe)
        context.contentResolver.openInputStream(uri).use { input ->
            requireNotNull(input) { "cannot open uri" }
            out.outputStream().use { output ->
                input.copyTo(output)
            }
        }
        return out
    }

    fun copyFromPath(context: Context, path: File, uri: Uri) {
        context.contentResolver.openOutputStream(uri, "wt").use { output ->
            requireNotNull(output) { "cannot write uri" }
            path.inputStream().use { input ->
                input.copyTo(output)
            }
        }
    }

    fun streamHash(input: InputStream, sha256: String): Boolean {
        val md = java.security.MessageDigest.getInstance("SHA-256")
        val buf = ByteArray(65536)
        while (true) {
            val n = input.read(buf)
            if (n <= 0) {
                break
            }
            md.update(buf, 0, n)
        }
        val hex = md.digest().joinToString("") { b -> "%02x".format(b) }
        return hex == sha256.lowercase()
    }
}