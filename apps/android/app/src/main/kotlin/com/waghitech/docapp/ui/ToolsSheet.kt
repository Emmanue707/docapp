package com.waghitech.docapp.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.waghitech.docapp.R

enum class DocTool {
    Merge,
    Split,
    Rotate,
    Delete,
    Reorder,
    Crop,
    AddText,
    AddImage,
    Sign,
    ConvertWord,
    ImagesToPdf,
    Ocr,
    Share,
    Save,
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ToolsSheet(
    visible: Boolean,
    isPdf: Boolean,
    onDismiss: () -> Unit,
    onTool: (DocTool) -> Unit,
) {
    if (!visible) {
        return
    }
    val items =
        buildList {
            if (isPdf) {
                add(DocTool.Merge)
                add(DocTool.Split)
                add(DocTool.Rotate)
                add(DocTool.Delete)
                add(DocTool.Reorder)
                add(DocTool.Crop)
                add(DocTool.AddText)
                add(DocTool.AddImage)
                add(DocTool.Sign)
                add(DocTool.ConvertWord)
                add(DocTool.Ocr)
            } else {
                add(DocTool.ImagesToPdf)
                add(DocTool.Ocr)
            }
            add(DocTool.Share)
            add(DocTool.Save)
        }
    ModalBottomSheet(onDismissRequest = onDismiss) {
        LazyVerticalGrid(
            columns = GridCells.Fixed(3),
            modifier = Modifier.fillMaxWidth().padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            items(items) { tool ->
                Column(
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .clickable {
                                onTool(tool)
                                onDismiss()
                            }
                            .padding(8.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Icon(
                        painter = painterResource(R.drawable.ic_tools),
                        contentDescription = null,
                    )
                    Text(label(tool), modifier = Modifier.padding(top = 4.dp))
                }
            }
        }
    }
}

@Composable
private fun label(tool: DocTool): String {
    return when (tool) {
        DocTool.Merge -> stringResource(R.string.merge)
        DocTool.Split -> stringResource(R.string.split)
        DocTool.Rotate -> stringResource(R.string.rotate)
        DocTool.Delete -> stringResource(R.string.delete_pages)
        DocTool.Reorder -> stringResource(R.string.reorder)
        DocTool.Crop -> stringResource(R.string.crop)
        DocTool.AddText -> stringResource(R.string.add_text)
        DocTool.AddImage -> stringResource(R.string.add_image)
        DocTool.Sign -> stringResource(R.string.sign)
        DocTool.ConvertWord -> stringResource(R.string.convert_word)
        DocTool.ImagesToPdf -> stringResource(R.string.images_to_pdf)
        DocTool.Ocr -> stringResource(R.string.ocr)
        DocTool.Share -> stringResource(R.string.share)
        DocTool.Save -> stringResource(R.string.save)
    }
}