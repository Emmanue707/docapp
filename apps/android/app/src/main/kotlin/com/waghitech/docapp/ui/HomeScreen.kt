package com.waghitech.docapp.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.waghitech.docapp.R
import com.waghitech.docapp.core.RecentEntry
import com.waghitech.docapp.core.ToolMime

@Composable
fun HomeScreen(
    recent: List<RecentEntry>,
    tools: List<DocTool>,
    onOpen: () -> Unit,
    onRecent: (RecentEntry) -> Unit,
    onTool: (DocTool) -> Unit,
    working: Boolean = false,
) {
    Column(
        modifier = Modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 12.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Button(onClick = onOpen, modifier = Modifier.fillMaxWidth(), enabled = !working) {
            Text(stringResource(R.string.open_file))
        }
        if (working) {
            CircularProgressIndicator(modifier = Modifier.padding(vertical = 4.dp))
        }
        Text(stringResource(R.string.tools_home), style = MaterialTheme.typography.titleMedium)
        LazyVerticalGrid(
            columns = GridCells.Fixed(3),
            modifier = Modifier.fillMaxWidth().heightIn(max = 360.dp).padding(bottom = 4.dp),
            contentPadding = PaddingValues(bottom = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(tools) { tool ->
                val label = toolLabel(tool)
                Column(
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .clickable(enabled = !working) { onTool(tool) }
                            .padding(8.dp)
                            .testTag(ToolMime.testTag(tool))
                            .semantics { contentDescription = label },
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Icon(
                        imageVector = toolIcon(tool),
                        contentDescription = null,
                    )
                    Text(
                        label,
                        modifier = Modifier.padding(top = 4.dp),
                        style = MaterialTheme.typography.labelMedium,
                        maxLines = 2,
                    )
                }
            }
        }
        Text(stringResource(R.string.recent_files), style = MaterialTheme.typography.titleMedium)
        if (recent.isEmpty()) {
            Text(stringResource(R.string.no_recent), style = MaterialTheme.typography.bodyMedium)
        } else {
            LazyColumn(modifier = Modifier.weight(1f)) {
                items(recent, key = { it.uri }) { entry ->
                    Text(
                        text = entry.name,
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clickable { onRecent(entry) }
                                .padding(vertical = 12.dp),
                        style = MaterialTheme.typography.bodyLarge,
                    )
                }
            }
        }
    }
}

@Composable
fun toolLabel(tool: DocTool): String =
    when (tool) {
        DocTool.PdfToWord -> stringResource(R.string.pdf_to_word)
        DocTool.Merge -> stringResource(R.string.merge)
        DocTool.Split -> stringResource(R.string.split)
        DocTool.Rotate -> stringResource(R.string.rotate)
        DocTool.Delete -> stringResource(R.string.delete_pages)
        DocTool.Reorder -> stringResource(R.string.reorder)
        DocTool.Crop -> stringResource(R.string.crop)
        DocTool.AddText -> stringResource(R.string.add_text)
        DocTool.AddImage -> stringResource(R.string.add_image)
        DocTool.Sign -> stringResource(R.string.sign)
        DocTool.Ocr -> stringResource(R.string.make_searchable)
        DocTool.ImagesToPdf -> stringResource(R.string.images_to_pdf)
        DocTool.OdtToWord -> stringResource(R.string.odt_to_word)
        DocTool.WordToOdt -> stringResource(R.string.word_to_odt)
        DocTool.ExtractText -> stringResource(R.string.extract_text)
        DocTool.Share -> stringResource(R.string.share)
        DocTool.Save -> stringResource(R.string.save)
    }