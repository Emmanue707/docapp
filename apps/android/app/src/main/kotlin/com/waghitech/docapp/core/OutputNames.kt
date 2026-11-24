package com.waghitech.docapp.core

object OutputNames {
    fun stem(displayName: String): String {
        val base = displayName.substringBeforeLast('.')
        return if (base.isBlank()) displayName else base
    }

    fun merged(stem: String): String = "$stem-merged.pdf"

    fun rotated(stem: String): String = "$stem-rotated.pdf"

    fun deleted(stem: String): String = "$stem-trimmed.pdf"

    fun reordered(stem: String): String = "$stem-reordered.pdf"

    fun cropped(stem: String): String = "$stem-cropped.pdf"

    fun stamped(stem: String): String = "$stem-stamped.pdf"

    fun signed(stem: String): String = "$stem-signed.pdf"

    fun searchable(stem: String): String = "$stem-searchable.pdf"

    fun docx(stem: String): String = "$stem.docx"

    fun odt(stem: String): String = "$stem.odt"

    fun extractText(stem: String): String = "$stem-text.txt"

    fun imagePdf(stem: String): String = "$stem.pdf"

    fun splitPart(stem: String, rangeLabel: String): String {
        val clean = rangeLabel.replace(Regex("\\s+"), "")
        return "$stem-p$clean.pdf"
    }

    fun ocrText(stem: String): String = "$stem-ocr.txt"

    fun unique(base: String, taken: Set<String>): String {
        if (base !in taken) {
            return base
        }
        val dot = base.lastIndexOf('.')
        val (head, ext) =
            if (dot > 0) {
                base.substring(0, dot) to base.substring(dot)
            } else {
                base to ""
            }
        var n = 2
        while (true) {
            val candidate = "$head-$n$ext"
            if (candidate !in taken) {
                return candidate
            }
            n++
        }
    }
}