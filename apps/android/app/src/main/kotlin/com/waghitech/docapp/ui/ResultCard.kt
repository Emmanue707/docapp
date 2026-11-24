package com.waghitech.docapp.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Card
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.waghitech.docapp.R

data class SavedOutput(
    val label: String,
    val mime: String,
    val canOpenInApp: Boolean,
)

@Composable
fun ResultCard(
    output: SavedOutput,
    onOpen: () -> Unit,
    onShare: () -> Unit,
    onViewInApp: (() -> Unit)?,
    onDismiss: () -> Unit,
) {
    Card(
        modifier =
            Modifier
                .fillMaxWidth()
                .padding(16.dp)
                .testTag("result_card"),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(output.label, modifier = Modifier.testTag("result_card_text"))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = onOpen) {
                    Text(stringResource(R.string.open_output))
                }
                TextButton(onClick = onShare) {
                    Text(stringResource(R.string.share))
                }
                if (onViewInApp != null) {
                    TextButton(onClick = onViewInApp) {
                        Text(stringResource(R.string.open_in_app))
                    }
                }
                TextButton(onClick = onDismiss) {
                    Text(stringResource(R.string.dismiss))
                }
            }
        }
    }
}