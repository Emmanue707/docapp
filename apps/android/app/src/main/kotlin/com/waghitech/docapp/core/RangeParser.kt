package com.waghitech.docapp.core

object RangeParser {
    fun parseDeleteInput(text: String, pageCount: Int): Result<List<Int>> {
        val parts = text.split(',').map { it.trim() }.filter { it.isNotEmpty() }
        if (parts.isEmpty()) {
            return Result.failure(IllegalArgumentException("empty"))
        }
        val pages = linkedSetOf<Int>()
        for (part in parts) {
            if (part.contains('-')) {
                val bounds = part.split('-', limit = 2)
                if (bounds.size != 2) {
                    return Result.failure(IllegalArgumentException("bad range"))
                }
                val start = bounds[0].toIntOrNull()
                    ?: return Result.failure(IllegalArgumentException("bad number"))
                val end = bounds[1].toIntOrNull()
                    ?: return Result.failure(IllegalArgumentException("bad number"))
                if (start < 1 || end < start || end > pageCount) {
                    return Result.failure(IllegalArgumentException("out of range"))
                }
                for (p in start..end) {
                    pages.add(p)
                }
            } else {
                val p = part.toIntOrNull()
                    ?: return Result.failure(IllegalArgumentException("bad number"))
                if (p < 1 || p > pageCount) {
                    return Result.failure(IllegalArgumentException("out of range"))
                }
                pages.add(p)
            }
        }
        return Result.success(pages.toList())
    }

    fun parseReorderInput(text: String, pageCount: Int): Result<List<Int>> {
        val nums = text.split(',').map { it.trim() }.filter { it.isNotEmpty() }
        if (nums.size != pageCount) {
            return Result.failure(IllegalArgumentException("need $pageCount pages"))
        }
        val order = mutableListOf<Int>()
        val seen = mutableSetOf<Int>()
        for (n in nums) {
            val p = n.toIntOrNull()
                ?: return Result.failure(IllegalArgumentException("bad number"))
            if (p < 1 || p > pageCount || !seen.add(p)) {
                return Result.failure(IllegalArgumentException("invalid order"))
            }
            order.add(p)
        }
        return Result.success(order)
    }
}