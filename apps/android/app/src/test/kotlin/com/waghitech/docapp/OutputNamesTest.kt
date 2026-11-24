package com.waghitech.docapp

import com.waghitech.docapp.core.OutputNames
import org.junit.Assert.assertEquals
import org.junit.Test

class OutputNamesTest {
    @Test
    fun stem_strips_extension() {
        assertEquals("report", OutputNames.stem("report.pdf"))
    }

    @Test
    fun merged_name() {
        assertEquals("report-merged.pdf", OutputNames.merged("report"))
    }

    @Test
    fun split_part_label() {
        assertEquals("report-p1-3.pdf", OutputNames.splitPart("report", "1-3"))
    }

    @Test
    fun unique_avoids_collision() {
        val taken = setOf("a.pdf", "a-2.pdf")
        assertEquals("a-3.pdf", OutputNames.unique("a.pdf", taken))
    }
}