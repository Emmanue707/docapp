package com.waghitech.docapp

import com.waghitech.docapp.core.ToolMime
import com.waghitech.docapp.ui.DocTool
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ToolMimeTest {
    @Test
    fun merge_allows_multi() {
        assertTrue(ToolMime.multiSelect(DocTool.Merge))
    }

    @Test
    fun rotate_single_pdf_mime() {
        assertArrayEquals(arrayOf("application/pdf"), ToolMime.openTypes(DocTool.Rotate))
    }

    @Test
    fun test_tag_slug() {
        assertEquals("tool_pdf_word", ToolMime.testTag(DocTool.PdfToWord))
        assertEquals("tool_merge", ToolMime.testTag(DocTool.Merge))
    }

    @Test
    fun output_mime_from_extension() {
        assertEquals("application/pdf", ToolMime.outputMime("x.pdf"))
        assertEquals("text/plain", ToolMime.outputMime("notes.txt"))
    }

    @Test
    fun split_not_multi() {
        assertFalse(ToolMime.multiSelect(DocTool.Split))
    }
}