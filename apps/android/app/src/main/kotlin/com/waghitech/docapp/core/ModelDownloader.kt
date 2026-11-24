package com.waghitech.docapp.core

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.docsuite_ffi.detectionModelFilename
import uniffi.docsuite_ffi.detectionModelSha256
import uniffi.docsuite_ffi.detectionModelUrl
import uniffi.docsuite_ffi.modelsStatus
import uniffi.docsuite_ffi.recognitionModelFilename
import uniffi.docsuite_ffi.recognitionModelSha256
import uniffi.docsuite_ffi.recognitionModelUrl
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

class ModelDownloader(private val context: Context) {
    val modelsDir: File
        get() = File(context.filesDir, "models")

    fun ready(): Boolean {
        val st = modelsStatus(modelsDir.absolutePath)
        return st.`detectionPresent` && st.`recognitionPresent`
    }

    suspend fun download(
        onProgress: (done: Long, total: Long) -> Unit,
    ): Result<Unit> = withContext(Dispatchers.IO) {
        val dir = modelsDir
        if (!dir.exists() && !dir.mkdirs()) {
            return@withContext Result.failure(IllegalStateException("mkdir models"))
        }
        val tasks = listOf(
            Triple(detectionModelUrl(), detectionModelFilename(), detectionModelSha256()),
            Triple(recognitionModelUrl(), recognitionModelFilename(), recognitionModelSha256()),
        )
        var doneFiles = 0
        val totalFiles = tasks.size
        for ((url, name, sha) in tasks) {
            val dest = File(dir, name)
            if (dest.isFile && dest.inputStream().use { UriFiles.streamHash(it, sha) }) {
                doneFiles++
                onProgress(doneFiles.toLong(), totalFiles.toLong())
                continue
            }
            downloadOne(url, dest, sha, onProgress, doneFiles, totalFiles)
            doneFiles++
            onProgress(doneFiles.toLong(), totalFiles.toLong())
        }
        Result.success(Unit)
    }

    private fun downloadOne(
        url: String,
        dest: File,
        sha: String,
        onProgress: (done: Long, total: Long) -> Unit,
        fileIndex: Int,
        totalFiles: Int,
    ) {
        val tmp = File(dest.parentFile, "${dest.name}.part")
        val conn = URL(url).openConnection() as HttpURLConnection
        conn.connectTimeout = 30_000
        conn.readTimeout = 120_000
        conn.connect()
        val length = conn.contentLengthLong.coerceAtLeast(0L)
        conn.inputStream.use { input ->
            tmp.outputStream().use { output ->
                val buf = ByteArray(65536)
                var readTotal = 0L
                while (true) {
                    val n = input.read(buf)
                    if (n <= 0) {
                        break
                    }
                    output.write(buf, 0, n)
                    readTotal += n
                    if (length > 0) {
                        val frac = readTotal.toDouble() / length.toDouble()
                        val overall = fileIndex + frac
                        onProgress(overall.toLong(), totalFiles.toLong())
                    }
                }
            }
        }
        if (!tmp.inputStream().use { UriFiles.streamHash(it, sha) }) {
            tmp.delete()
            error("sha256 mismatch")
        }
        if (dest.exists()) {
            dest.delete()
        }
        if (!tmp.renameTo(dest)) {
            tmp.copyTo(dest, overwrite = true)
            tmp.delete()
        }
    }
}