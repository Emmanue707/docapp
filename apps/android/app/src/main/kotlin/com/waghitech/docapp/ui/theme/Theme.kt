package com.waghitech.docapp.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

private val GreenSeed = Color(0xFF1B8A5A)

@Composable
fun DocappTheme(content: @Composable () -> Unit) {
    val context = LocalContext.current
    val dynamic = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
    val colors =
        if (dynamic) {
            if (isSystemInDarkTheme()) {
                dynamicDarkColorScheme(context)
            } else {
                dynamicLightColorScheme(context)
            }
        } else if (isSystemInDarkTheme()) {
            darkColorScheme(primary = GreenSeed)
        } else {
            lightColorScheme(primary = GreenSeed)
        }
    MaterialTheme(colorScheme = colors, content = content)
}