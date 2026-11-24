package com.waghitech.docapp.ui

import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Article
import androidx.compose.material.icons.outlined.CallMerge
import androidx.compose.material.icons.outlined.Crop
import androidx.compose.material.icons.outlined.DeleteOutline
import androidx.compose.material.icons.outlined.Description
import androidx.compose.material.icons.outlined.Draw
import androidx.compose.material.icons.outlined.Image
import androidx.compose.material.icons.outlined.Layers
import androidx.compose.material.icons.outlined.PictureAsPdf
import androidx.compose.material.icons.outlined.RotateRight
import androidx.compose.material.icons.outlined.SwapVert
import androidx.compose.material.icons.outlined.TextFields
import androidx.compose.material.icons.outlined.TextSnippet
import androidx.compose.material.icons.outlined.TouchApp
import androidx.compose.ui.graphics.vector.ImageVector

fun toolIcon(tool: DocTool): ImageVector =
    when (tool) {
        DocTool.PdfToWord -> Icons.Outlined.Description
        DocTool.Merge -> Icons.Outlined.CallMerge
        DocTool.Split -> Icons.Outlined.Layers
        DocTool.Rotate -> Icons.Outlined.RotateRight
        DocTool.Delete -> Icons.Outlined.DeleteOutline
        DocTool.Reorder -> Icons.Outlined.SwapVert
        DocTool.Crop -> Icons.Outlined.Crop
        DocTool.AddText -> Icons.Outlined.TextFields
        DocTool.AddImage -> Icons.Outlined.Image
        DocTool.Sign -> Icons.Outlined.Draw
        DocTool.Ocr -> Icons.Outlined.Article
        DocTool.ImagesToPdf -> Icons.Outlined.PictureAsPdf
        DocTool.OdtToWord -> Icons.Outlined.Description
        DocTool.WordToOdt -> Icons.Outlined.TextSnippet
        DocTool.ExtractText -> Icons.Outlined.TextFields
        DocTool.Share -> Icons.Outlined.TouchApp
        DocTool.Save -> Icons.Outlined.Description
    }