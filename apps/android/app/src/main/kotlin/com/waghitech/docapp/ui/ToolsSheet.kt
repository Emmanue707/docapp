package com.waghitech.docapp.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.waghitech.docapp.R
import com.waghitech.docapp.core.UriFiles

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ToolsSheet(
    visible: Boolean,
    mime: String,
    onDismiss: () -> Unit,
    onTool: (DocTool) -> Unit,
) {
    if (!visible) {
        return
    }
    val items =
        buildList {
            if (UriFiles.isPdf(mime)) {
                add(DocTool.PdfToWord)
                add(DocTool.Merge)
                add(DocTool.Split)
                add(DocTool.Rotate)
                add(DocTool.Delete)
                add(DocTool.Reorder)
                add(DocTool.Crop)
                add(DocTool.AddText)
                add(DocTool.AddImage)
                add(DocTool.Sign)
                add(DocTool.Ocr)
                add(DocTool.ExtractText)
            } else if (UriFiles.isImage(mime)) {
                add(DocTool.ImagesToPdf)
                add(DocTool.Ocr)
            } else if (mime.contains("opendocument.text")) {
                add(DocTool.OdtToWord)
            } else if (mime.contains("wordprocessingml")) {
                add(DocTool.WordToOdt)
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
                val label = toolLabel(tool)
                Column(
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .clickable {
                                onTool(tool)
                                onDismiss()
                            }
                            .padding(8.dp)
                            .semantics { contentDescription = label },
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Icon(
                        painter = painterResource(R.drawable.ic_tools),
                        contentDescription = null,
                    )
                    Text(label, modifier = Modifier.padding(top = 4.dp))
                }
            }
        }
    }
}