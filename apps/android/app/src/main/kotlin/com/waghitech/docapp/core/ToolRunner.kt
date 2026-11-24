package com.waghitech.docapp.core

import com.waghitech.docapp.ui.DocTool
import uniffi.docsuite_ffi.CancelHandle
import uniffi.docsuite_ffi.PdfRectFfi
import uniffi.docsuite_ffi.ProgressListener
import uniffi.docsuite_ffi.docxToOdt
import uniffi.docsuite_ffi.odtToDocx
import java.io.File

data class ToolInput(
    val files: List<File>,
    val displayName: String,
    val mime: String,
)

data class ProducedFile(
    val file: File,
    val publicName: String,
    val mime: String,
)

sealed class ToolParams {
    data object None : ToolParams()

    data class Split(val ranges: String, val single: Boolean = false) : ToolParams()

    data class Rotate(val degrees: Int, val pageSpec: String?, val pageCount: Int) : ToolParams()

    data class Delete(val pageSpec: String, val pageCount: Int) : ToolParams()

    data class Reorder(val orderSpec: String, val pageCount: Int) : ToolParams()

    data class Crop(val page: Int, val rect: PdfRectFfi) : ToolParams()

    data class AddText(
        val page: Int,
        val text: String,
        val x: Double = 72.0,
        val y: Double = 72.0,
        val size: Double = 14.0,
    ) : ToolParams()

    data class AddImage(val page: Int, val image: File, val rect: PdfRectFfi) : ToolParams()

    data class SignVisible(
        val page: Int,
        val name: String,
        val reason: String,
        val image: File?,
        val rect: PdfRectFfi,
    ) : ToolParams()

    data class SignDigital(val p12: File, val password: String) : ToolParams()

    data class PdfToWord(val editable: Boolean) : ToolParams()

    data class Ocr(val handwriting: Boolean) : ToolParams()
}

class ToolRunner(
    private val engine: DocEngine,
    private val workDir: File,
    private val modelsDir: File,
) {
    suspend fun run(
        tool: DocTool,
        input: ToolInput,
        params: ToolParams,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): Result<List<ProducedFile>> {
        val stem = OutputNames.stem(input.displayName)
        return runCatching {
            when (tool) {
                DocTool.Merge -> merge(input, stem, cancel, progress)
                DocTool.Split -> split(input, stem, params, cancel, progress)
                DocTool.Rotate -> rotate(input, stem, params, cancel, progress)
                DocTool.Delete -> delete(input, stem, params, cancel, progress)
                DocTool.Reorder -> reorder(input, stem, params, cancel, progress)
                DocTool.Crop -> crop(input, stem, params, cancel, progress)
                DocTool.AddText -> addText(input, stem, params, cancel, progress)
                DocTool.AddImage -> addImage(input, stem, params, cancel, progress)
                DocTool.Sign -> sign(input, stem, params, cancel, progress)
                DocTool.PdfToWord -> pdfToWord(input, stem, params, cancel, progress)
                DocTool.OdtToWord -> odtToWord(input, stem, cancel, progress)
                DocTool.WordToOdt -> wordToOdt(input, stem, cancel, progress)
                DocTool.ImagesToPdf -> imagesToPdf(input, stem, cancel, progress)
                DocTool.Ocr -> ocr(input, stem, params, cancel, progress)
                DocTool.ExtractText -> extractText(input, stem)
                DocTool.Share,
                DocTool.Save,
                -> error("not runnable")
            }
        }
    }

    private suspend fun merge(
        input: ToolInput,
        stem: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val name = OutputNames.merged(stem)
        val out = File(workDir, name)
        engine.mergePdfs(input.files.map { it.absolutePath }, out.absolutePath, cancel, progress)
            .getOrThrow()
        return listOf(ProducedFile(out, name, "application/pdf"))
    }

    private suspend fun split(
        input: ToolInput,
        stem: String,
        params: ToolParams,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val p = params as? ToolParams.Split ?: error("split params")
        val outDir = File(workDir, "split-${System.nanoTime()}")
        outDir.mkdirs()
        engine.splitPdf(
            input.files.single().absolutePath,
            p.ranges,
            outDir.absolutePath,
            p.single,
            cancel,
            progress,
        ).getOrThrow()
        val labels = p.ranges.split(',').map { it.trim() }.filter { it.isNotEmpty() }
        val parts = outDir.listFiles()?.filter { it.extension == "pdf" }?.sortedBy { it.name } ?: emptyList()
        return parts.mapIndexed { i, file ->
            val label = labels.getOrElse(i) { "part${i + 1}" }
            val name = OutputNames.splitPart(stem, label)
            ProducedFile(file, name, "application/pdf")
        }
    }

    private suspend fun rotate(
        input: ToolInput,
        stem: String,
        params: ToolParams,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val p = params as? ToolParams.Rotate ?: error("rotate params")
        val pages =
            if (p.pageSpec.isNullOrBlank()) {
                (1..p.pageCount).toList()
            } else {
                RangeParser.parseDeleteInput(p.pageSpec, p.pageCount).getOrThrow()
            }
        val name = OutputNames.rotated(stem)
        val out = File(workDir, name)
        engine.rotatePdf(
            input.files.single().absolutePath,
            pages,
            p.degrees,
            out.absolutePath,
            cancel,
            progress,
        ).getOrThrow()
        return listOf(ProducedFile(out, name, "application/pdf"))
    }

    private suspend fun delete(
        input: ToolInput,
        stem: String,
        params: ToolParams,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val p = params as? ToolParams.Delete ?: error("delete params")
        val pages = RangeParser.parseDeleteInput(p.pageSpec, p.pageCount).getOrThrow()
        val name = OutputNames.deleted(stem)
        val out = File(workDir, name)
        engine.deletePdfPages(
            input.files.single().absolutePath,
            pages,
            out.absolutePath,
            cancel,
            progress,
        ).getOrThrow()
        return listOf(ProducedFile(out, name, "application/pdf"))
    }

    private suspend fun reorder(
        input: ToolInput,
        stem: String,
        params: ToolParams,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val p = params as? ToolParams.Reorder ?: error("reorder params")
        val order = RangeParser.parseReorderInput(p.orderSpec, p.pageCount).getOrThrow()
        val name = OutputNames.reordered(stem)
        val out = File(workDir, name)
        engine.reorderPdf(
            input.files.single().absolutePath,
            order,
            out.absolutePath,
            cancel,
            progress,
        ).getOrThrow()
        return listOf(ProducedFile(out, name, "application/pdf"))
    }

    private suspend fun crop(
        input: ToolInput,
        stem: String,
        params: ToolParams,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val p = params as? ToolParams.Crop ?: error("crop params")
        val name = OutputNames.cropped(stem)
        val out = File(workDir, name)
        engine.cropPdf(
            input.files.single().absolutePath,
            listOf(p.page),
            p.rect,
            false,
            out.absolutePath,
            cancel,
            progress,
        ).getOrThrow()
        return listOf(ProducedFile(out, name, "application/pdf"))
    }

    private suspend fun addText(
        input: ToolInput,
        stem: String,
        params: ToolParams,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val p = params as? ToolParams.AddText ?: error("text params")
        val name = OutputNames.stamped(stem)
        val out = File(workDir, name)
        engine.stampTextPdf(
            input.files.single().absolutePath,
            p.page,
            p.x,
            p.y,
            p.size,
            p.text,
            out.absolutePath,
            cancel,
            progress,
        ).getOrThrow()
        return listOf(ProducedFile(out, name, "application/pdf"))
    }

    private suspend fun addImage(
        input: ToolInput,
        stem: String,
        params: ToolParams,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val p = params as? ToolParams.AddImage ?: error("image params")
        val name = OutputNames.stamped(stem)
        val out = File(workDir, name)
        engine.stampImagePdf(
            input.files.single().absolutePath,
            p.page,
            p.rect,
            p.image.absolutePath,
            out.absolutePath,
            cancel,
            progress,
        ).getOrThrow()
        return listOf(ProducedFile(out, name, "application/pdf"))
    }

    private suspend fun sign(
        input: ToolInput,
        stem: String,
        params: ToolParams,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val name = OutputNames.signed(stem)
        val out = File(workDir, name)
        when (params) {
            is ToolParams.SignDigital -> {
                engine.digitalSignPdf(
                    input.files.single().absolutePath,
                    out.absolutePath,
                    params.p12.absolutePath,
                    params.password,
                    cancel,
                    progress,
                ).getOrThrow()
            }
            is ToolParams.SignVisible -> {
                engine.signPdf(
                    input.files.single().absolutePath,
                    params.page,
                    params.rect,
                    params.name,
                    params.reason,
                    params.image?.absolutePath,
                    out.absolutePath,
                    cancel,
                    progress,
                ).getOrThrow()
            }
            else -> error("sign params")
        }
        return listOf(ProducedFile(out, name, "application/pdf"))
    }

    private suspend fun pdfToWord(
        input: ToolInput,
        stem: String,
        params: ToolParams,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val editable = (params as? ToolParams.PdfToWord)?.editable == true
        val name = OutputNames.docx(stem)
        val out = File(workDir, name)
        engine.toDocx(input.files.single().absolutePath, out.absolutePath, editable, cancel, progress)
            .getOrThrow()
        return listOf(
            ProducedFile(
                out,
                name,
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ),
        )
    }

    private suspend fun odtToWord(
        input: ToolInput,
        stem: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val name = OutputNames.docx(stem)
        val out = File(workDir, name)
        engine.odtToDocx(input.files.single().absolutePath, out.absolutePath, cancel, progress)
            .getOrThrow()
        return listOf(
            ProducedFile(
                out,
                name,
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ),
        )
    }

    private suspend fun wordToOdt(
        input: ToolInput,
        stem: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val name = OutputNames.odt(stem)
        val out = File(workDir, name)
        engine.docxToOdt(input.files.single().absolutePath, out.absolutePath, cancel, progress)
            .getOrThrow()
        return listOf(ProducedFile(out, name, "application/vnd.oasis.opendocument.text"))
    }

    private suspend fun imagesToPdf(
        input: ToolInput,
        stem: String,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val temps = mutableListOf<File>()
        input.files.forEachIndexed { i, file ->
            val part = File(workDir, "img-$i-${System.nanoTime()}.pdf")
            engine.imagesToPdf(file.absolutePath, part.absolutePath, cancel, progress).getOrThrow()
            temps.add(part)
        }
        val name = OutputNames.imagePdf(stem)
        val out = File(workDir, name)
        if (temps.size == 1) {
            temps[0].copyTo(out, overwrite = true)
        } else {
            engine.mergePdfs(temps.map { it.absolutePath }, out.absolutePath, cancel, progress)
                .getOrThrow()
        }
        temps.forEach { it.delete() }
        return listOf(ProducedFile(out, name, "application/pdf"))
    }

    private suspend fun ocr(
        input: ToolInput,
        stem: String,
        params: ToolParams,
        cancel: CancelHandle,
        progress: ProgressListener?,
    ): List<ProducedFile> {
        val handwriting = (params as? ToolParams.Ocr)?.handwriting == true
        return if (UriFiles.isPdf(input.mime)) {
            val name = OutputNames.searchable(stem)
            val out = File(workDir, name)
            engine.searchable(
                input.files.single().absolutePath,
                out.absolutePath,
                modelsDir.absolutePath,
                handwriting,
                cancel,
                progress,
            ).getOrThrow()
            listOf(ProducedFile(out, name, "application/pdf"))
        } else {
            val text =
                engine.ocr(
                    input.files.single().absolutePath,
                    modelsDir.absolutePath,
                    handwriting,
                    cancel,
                    progress,
                ).getOrThrow()
            val name = OutputNames.ocrText(stem)
            val out = File(workDir, name)
            out.writeText(text)
            listOf(ProducedFile(out, name, "text/plain"))
        }
    }

    private suspend fun extractText(
        input: ToolInput,
        stem: String,
    ): List<ProducedFile> {
        val path = input.files.single().absolutePath
        val count = engine.pages(path).getOrThrow()
        val sb = StringBuilder()
        for (p in 1..count) {
            if (p > 1) {
                sb.append("\n\n")
            }
            sb.append(engine.pageText(path, p).getOrThrow())
        }
        val name = OutputNames.extractText(stem)
        val out = File(workDir, name)
        out.writeText(sb.toString())
        return listOf(ProducedFile(out, name, "text/plain"))
    }

    companion object {
        fun defaultCropRect(): PdfRectFfi = PdfRectFfi(36.0, 36.0, 559.0, 806.0)

        fun defaultSignRect(): PdfRectFfi = PdfRectFfi(72.0, 600.0, 260.0, 720.0)

        fun defaultStampRect(): PdfRectFfi = PdfRectFfi(72.0, 72.0, 220.0, 160.0)
    }
}