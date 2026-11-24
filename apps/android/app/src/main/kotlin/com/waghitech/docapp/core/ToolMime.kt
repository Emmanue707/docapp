package com.waghitech.docapp.core

import com.waghitech.docapp.ui.DocTool

object ToolMime {
    fun openTypes(tool: DocTool): Array<String> =
        when (tool) {
            DocTool.Merge,
            DocTool.Split,
            DocTool.Rotate,
            DocTool.Delete,
            DocTool.Reorder,
            DocTool.Crop,
            DocTool.AddText,
            DocTool.AddImage,
            DocTool.Sign,
            DocTool.PdfToWord,
            DocTool.Ocr,
            DocTool.ExtractText,
            -> arrayOf("application/pdf")
            DocTool.ImagesToPdf -> arrayOf("image/*")
            DocTool.OdtToWord ->
                arrayOf(
                    "application/vnd.oasis.opendocument.text",
                    "application/octet-stream",
                )
            DocTool.WordToOdt ->
                arrayOf(
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                    "application/octet-stream",
                )
            DocTool.Share,
            DocTool.Save,
            -> arrayOf("*/*")
        }

    fun multiSelect(tool: DocTool): Boolean =
        tool == DocTool.Merge || tool == DocTool.ImagesToPdf

    fun testTag(tool: DocTool): String = "tool_${tagSlug(tool)}"

    private fun tagSlug(tool: DocTool): String =
        when (tool) {
            DocTool.PdfToWord -> "pdf_word"
            DocTool.ImagesToPdf -> "images_pdf"
            DocTool.OdtToWord -> "odt_word"
            DocTool.WordToOdt -> "word_odt"
            DocTool.ExtractText -> "extract_text"
            else -> tool.name.lowercase()
        }

    fun homeTools(): List<DocTool> =
        listOf(
            DocTool.PdfToWord,
            DocTool.Merge,
            DocTool.Split,
            DocTool.Rotate,
            DocTool.Delete,
            DocTool.Reorder,
            DocTool.Crop,
            DocTool.AddText,
            DocTool.AddImage,
            DocTool.Sign,
            DocTool.Ocr,
            DocTool.ImagesToPdf,
            DocTool.OdtToWord,
            DocTool.WordToOdt,
            DocTool.ExtractText,
        )

    fun outputMime(path: String): String {
        val lower = path.lowercase()
        return when {
            lower.endsWith(".pdf") -> "application/pdf"
            lower.endsWith(".docx") ->
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            lower.endsWith(".odt") -> "application/vnd.oasis.opendocument.text"
            lower.endsWith(".txt") -> "text/plain"
            else -> "application/octet-stream"
        }
    }
}