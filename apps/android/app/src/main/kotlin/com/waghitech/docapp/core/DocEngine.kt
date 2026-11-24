package com.waghitech.docapp.core

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.docsuite_ffi.CancelHandle
import uniffi.docsuite_ffi.PdfRectFfi
import uniffi.docsuite_ffi.ProgressListener
import uniffi.docsuite_ffi.cropPages
import uniffi.docsuite_ffi.deletePagesFfi
import uniffi.docsuite_ffi.extractTextLayer
import uniffi.docsuite_ffi.imageToPdfFfi
import uniffi.docsuite_ffi.makeSearchable
import uniffi.docsuite_ffi.merge
import uniffi.docsuite_ffi.ocrImage
import uniffi.docsuite_ffi.pageCount
import uniffi.docsuite_ffi.pdfToDocx
import uniffi.docsuite_ffi.reorderPages
import uniffi.docsuite_ffi.rotate
import uniffi.docsuite_ffi.split
import uniffi.docsuite_ffi.stampImage
import uniffi.docsuite_ffi.stampText
import uniffi.docsuite_ffi.signPdfFfi
import uniffi.docsuite_ffi.visibleSignatureFfi
import java.io.File

class DocEngine {
    suspend fun pages(path: String): Result<Int> = onIo { pageCount(path).toInt() }

    suspend fun mergePdfs(
        inputs: List<String>,
        output: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo { merge(inputs, output, cancel, progress) }

    suspend fun splitPdf(
        path: String,
        ranges: String,
        outDir: String,
        single: Boolean,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo { split(path, ranges, outDir, single, cancel, progress) }

    suspend fun rotatePdf(
        path: String,
        pages: List<Int>,
        degrees: Int,
        output: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo {
        rotate(path, pages.map { it.toUInt() }, degrees, output, cancel, progress)
    }

    suspend fun deletePdfPages(
        path: String,
        pages: List<Int>,
        output: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo {
        deletePagesFfi(path, pages.map { it.toUInt() }, output, cancel, progress)
    }

    suspend fun reorderPdf(
        path: String,
        order: List<Int>,
        output: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo {
        reorderPages(path, order.map { it.toUInt() }, output, cancel, progress)
    }

    suspend fun cropPdf(
        path: String,
        pages: List<Int>,
        rect: PdfRectFfi,
        expand: Boolean,
        output: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo {
        cropPages(path, pages.map { it.toUInt() }, rect, expand, output, cancel, progress)
    }

    suspend fun stampTextPdf(
        path: String,
        page: Int,
        x: Double,
        y: Double,
        size: Double,
        text: String,
        output: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo {
        stampText(path, page.toUInt(), x, y, size, text, output, cancel, progress)
    }

    suspend fun stampImagePdf(
        path: String,
        page: Int,
        rect: PdfRectFfi,
        imagePath: String,
        output: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo {
        stampImage(path, page.toUInt(), rect, imagePath, output, cancel, progress)
    }

    suspend fun digitalSignPdf(
        path: String,
        output: String,
        p12Path: String,
        password: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo {
        signPdfFfi(
            path,
            output,
            p12Path,
            password,
            null,
            null,
            null,
            null,
            null,
            cancel,
            progress,
        )
    }

    suspend fun signPdf(
        path: String,
        page: Int,
        rect: PdfRectFfi,
        name: String,
        reason: String,
        imagePath: String?,
        output: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo {
        visibleSignatureFfi(
            path,
            page.toUInt(),
            rect,
            name,
            reason,
            imagePath,
            output,
            cancel,
            progress,
        )
    }

    suspend fun imagesToPdf(
        input: String,
        output: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo { imageToPdfFfi(input, output, cancel, progress) }

    suspend fun toDocx(
        path: String,
        out: String,
        editable: Boolean,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo { pdfToDocx(path, out, editable, cancel, progress) }

    suspend fun ocr(
        path: String,
        modelsDir: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<String> = onIo { ocrImage(path, modelsDir, cancel, progress) }

    suspend fun searchable(
        path: String,
        out: String,
        modelsDir: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<Unit> = onIo { makeSearchable(path, out, modelsDir, cancel, progress) }

    suspend fun pageText(path: String, page: Int): Result<String> =
        onIo { extractTextLayer(path, page.toUInt()) }

    fun workCopy(src: File, suffix: String = "edit"): File {
        val parent = src.parentFile ?: error("no parent")
        val name = src.nameWithoutExtension
        val ext = src.extension
        val out = File(parent, "$name-$suffix.${ext.ifBlank { "pdf" }}")
        src.copyTo(out, overwrite = true)
        return out
    }

    private suspend fun <T> onIo(block: () -> T): Result<T> =
        withContext(Dispatchers.Default) {
            runCatching { block() }
        }
}