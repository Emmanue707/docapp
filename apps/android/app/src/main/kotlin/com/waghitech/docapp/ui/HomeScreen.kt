package com.waghitech.docapp.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.waghitech.docapp.R
import com.waghitech.docapp.core.RecentEntry

@Composable
fun HomeScreen(
    recent: List<RecentEntry>,
    onOpen: () -> Unit,
    onRecent: (RecentEntry) -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Button(onClick = onOpen, modifier = Modifier.fillMaxWidth()) {
            Text(stringResource(R.string.open_file))
        }
        Text(stringResource(R.string.recent_files), style = MaterialTheme.typography.titleMedium)
        if (recent.isEmpty()) {
            Text(stringResource(R.string.no_recent), style = MaterialTheme.typography.bodyMedium)
        } else {
            LazyColumn(modifier = Modifier.fillMaxSize()) {
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