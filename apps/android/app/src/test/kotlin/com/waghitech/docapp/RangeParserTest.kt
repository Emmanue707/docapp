package com.waghitech.docapp

import com.waghitech.docapp.core.RangeParser
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class RangeParserTest {
    @Test
    fun delete_expands_ranges() {
        val pages = RangeParser.parseDeleteInput("1-2,4", 5).getOrThrow()
        assertEquals(listOf(1, 2, 4), pages)
    }

    @Test
    fun delete_rejects_out_of_range() {
        assertTrue(RangeParser.parseDeleteInput("6", 5).isFailure)
    }

    @Test
    fun reorder_validates_permutation() {
        val order = RangeParser.parseReorderInput("3,1,2", 3).getOrThrow()
        assertEquals(listOf(3, 1, 2), order)
    }

    @Test
    fun reorder_rejects_duplicates() {
        assertTrue(RangeParser.parseReorderInput("1,1,2", 3).isFailure)
    }
}