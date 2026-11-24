package com.waghitech.docapp

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Scaffold
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.waghitech.docapp.core.DocEngine
import com.waghitech.docapp.core.ModelDownloader
import com.waghitech.docapp.core.RecentEntry
import com.waghitech.docapp.core.RecentStore
import com.waghitech.docapp.core.UriFiles
import com.waghitech.docapp.ui.DocTool
import com.waghitech.docapp.ui.HomeScreen
import com.waghitech.docapp.ui.ToolsSheet
import com.waghitech.docapp.ui.ViewerScreen
import com.waghitech.docapp.ui.theme.DocappTheme
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import uniffi.docsuite_ffi.CancelHandle
import uniffi.docsuite_ffi.ProgressListener
import uniffi.docsuite_ffi.PdfRectFfi
import java.io.File

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            DocappTheme {
                DocappRoot(
                    onPersistUri = { uri -> contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) },
                )
            }
        }
        intent?.let { handleIncoming(it) }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIncoming(intent)
    }

    private fun handleIncoming(intent: Intent) {
        // compose state picks this up via launch effect reading activity intent
        setIntent(intent)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DocappRoot(onPersistUri: (Uri) -> Unit) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val activity = context as ComponentActivity
    val scope = rememberCoroutineScope()
    val store = remember { RecentStore(context) }
    val engine = remember { DocEngine() }
    val models = remember { ModelDownloader(context) }
    var recent by remember { mutableStateOf(store.list()) }
    var screen by remember { mutableStateOf<Screen>(Screen.Home) }
    var workFile by remember { mutableStateOf<File?>(null) }
    var displayName by remember { mutableStateOf("") }
    var mime by remember { mutableStateOf("application/pdf") }
    var sourceUri by remember { mutableStateOf<Uri?>(null) }
    var toolsOpen by remember { mutableStateOf(false) }
    var visiblePage by remember { mutableIntStateOf(1) }
    var pageCount by remember { mutableIntStateOf(0) }
    var busy by remember { mutableStateOf(false) }
    var progress by remember { mutableFloatStateOf(0f) }
    var status by remember { mutableStateOf("") }
    var cancelHandle by remember { mutableStateOf<CancelHandle?>(null) }
    var workJob by remember { mutableStateOf<Job?>(null) }
    var dialog by remember { mutableStateOf<String?>(null) }
    var handwritingOcr by remember { mutableStateOf(false) }
    var signPassword by remember { mutableStateOf("") }
    var signP12 by remember { mutableStateOf<File?>(null) }

    fun openUri(uri: Uri) {
        val resolver = context.contentResolver
        try {
            onPersistUri(uri)
        } catch (_: SecurityException) {
        }
        val name = UriFiles.displayName(resolver, uri)
        val type = UriFiles.mime(resolver, uri)
        val file = UriFiles.copyToCache(context, uri, name)
        workFile = file
        displayName = name
        mime = type
        sourceUri = uri
        screen = Screen.Viewer
        visiblePage = 1
        pageCount = 0
        store.add(RecentEntry(uri.toString(), name, type, System.currentTimeMillis()))
        recent = store.list()
        if (UriFiles.isPdf(type)) {
            scope.launch {
                engine.pages(file.absolutePath).onSuccess { pageCount = it }
            }
        }
    }

    val openDoc =
        rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
            if (uri != null) {
                openUri(uri)
            }
        }

    val pickSignP12 =
        rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
            if (uri == null) {
                return@rememberLauncherForActivityResult
            }
            try {
                onPersistUri(uri)
            } catch (_: SecurityException) {
            }
            val name = UriFiles.displayName(context.contentResolver, uri)
            signP12 = UriFiles.copyToCache(context, uri, name)
        }

    val saveDoc =
        rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/pdf")) { uri ->
            val src = workFile ?: return@rememberLauncherForActivityResult
            if (uri == null) {
                return@rememberLauncherForActivityResult
            }
            scope.launch {
                runCatching { UriFiles.copyFromPath(context, src, uri) }
            }
        }

    LaunchedEffect(Unit) {
        val intent = activity.intent
        val uri = intent?.data
        if (uri != null) {
            openUri(uri)
        }
    }

    val progressListener =
        remember {
            object : ProgressListener {
                override fun `onProgress`(done: ULong, total: ULong, stage: String) {
                    if (total > 0uL) {
                        progress = done.toFloat() / total.toFloat()
                    }
                    status = stage
                }
            }
        }

    fun runJob(block: suspend () -> Unit) {
        workJob?.cancel()
        val cancel = CancelHandle()
        cancelHandle = cancel
        busy = true
        progress = 0f
        status = context.getString(R.string.working)
        workJob =
            scope.launch {
                try {
                    block()
                } finally {
                    busy = false
                    cancelHandle = null
                }
            }
    }

    Scaffold(
        topBar = {
            if (screen is Screen.Viewer) {
                TopAppBar(
                    title = {
                        Column {
                            Text(displayName)
                            if (pageCount > 0) {
                                Text(
                                    stringResource(R.string.page_of, visiblePage, pageCount),
                                )
                            }
                        }
                    },
                    navigationIcon = {
                        TextButton(onClick = { screen = Screen.Home }) {
                            Text("Back")
                        }
                    },
                    actions = {
                        IconButton(onClick = { toolsOpen = true }) {
                            androidx.compose.material3.Icon(
                                painter = painterResource(R.drawable.ic_tools),
                                contentDescription = stringResource(R.string.tools),
                            )
                        }
                    },
                )
            }
        },
    ) { pad ->
        Box(Modifier.fillMaxSize().padding(pad)) {
            when (screen) {
                Screen.Home ->
                    HomeScreen(
                        recent = recent,
                        onOpen = {
                            openDoc.launch(arrayOf("application/pdf", "image/*"))
                        },
                        onRecent = { entry ->
                            openUri(Uri.parse(entry.uri))
                        },
                    )
                Screen.Viewer -> {
                    val file = workFile
                    if (file != null) {
                        ViewerScreen(
                            workFile = file,
                            mime = mime,
                            onVisiblePage = { visiblePage = it },
                        )
                    }
                }
            }
            if (busy) {
                Column(Modifier.fillMaxWidth().padding(16.dp)) {
                    LinearProgressIndicator(progress = { progress }, modifier = Modifier.fillMaxWidth())
                    Text(status)
                    TextButton(
                        onClick = {
                            cancelHandle?.cancel()
                            workJob?.cancel()
                        },
                    ) {
                        Text(stringResource(R.string.cancel))
                    }
                }
            }
        }
    }

    ToolsSheet(
        visible = toolsOpen,
        isPdf = UriFiles.isPdf(mime),
        onDismiss = { toolsOpen = false },
        onTool = { tool ->
            val src = workFile ?: return@ToolsSheet
            when (tool) {
                DocTool.Save -> saveDoc.launch(displayName)
                DocTool.Share -> {
                    val uri = sourceUri
                    if (uri != null) {
                        val share = Intent(Intent.ACTION_SEND).apply {
                            type = mime
                            putExtra(Intent.EXTRA_STREAM, uri)
                            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                        }
                        context.startActivity(Intent.createChooser(share, null))
                    }
                }
                DocTool.ImagesToPdf -> {
                    if (!UriFiles.isImage(mime)) {
                        return@ToolsSheet
                    }
                    runJob {
                        val cancel = cancelHandle ?: return@runJob
                        val out = File(src.parentFile, "${src.nameWithoutExtension}.pdf")
                        engine.imagesToPdf(src.absolutePath, out.absolutePath, cancel, progressListener)
                            .onSuccess {
                                workFile = out
                                mime = "application/pdf"
                                displayName = out.name
                            }
                    }
                }
                DocTool.Rotate -> {
                    runJob {
                        val cancel = cancelHandle ?: return@runJob
                        val out = engine.workCopy(src, "rot")
                        val pages = (1..pageCount.coerceAtLeast(1)).toList()
                        engine.rotatePdf(src.absolutePath, pages, 90, out.absolutePath, cancel, progressListener)
                            .onSuccess { workFile = out }
                    }
                }
                DocTool.ConvertWord -> {
                    runJob {
                        val cancel = cancelHandle ?: return@runJob
                        val out = File(src.parentFile, "${src.nameWithoutExtension}.docx")
                        engine.toDocx(src.absolutePath, out.absolutePath, false, cancel, progressListener)
                    }
                }
                DocTool.Sign -> {
                    if (!UriFiles.isPdf(mime)) {
                        return@ToolsSheet
                    }
                    dialog = "sign"
                }
                DocTool.Ocr -> {
                    if (!models.ready()) {
                        dialog = "models"
                        return@ToolsSheet
                    }
                    dialog = "ocr_opts"
                }
                else -> dialog = "Soon: ${tool.name}"
            }
        },
    )

    if (dialog == "ocr_opts" && workFile != null) {
        val src = workFile!!
        AlertDialog(
            onDismissRequest = { dialog = null },
            title = { Text(stringResource(R.string.ocr)) },
            text = {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
                ) {
                    Checkbox(
                        checked = handwritingOcr,
                        onCheckedChange = { handwritingOcr = it },
                    )
                    Text(stringResource(R.string.handwriting_ocr))
                }
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        dialog = null
                        if (handwritingOcr && !models.handwritingReady()) {
                            dialog = "handwriting_models"
                            return@TextButton
                        }
                        val file = src
                        runJob {
                            val cancel = cancelHandle ?: return@runJob
                            val hw = handwritingOcr
                            if (UriFiles.isPdf(mime)) {
                                val out = engine.workCopy(file, "search")
                                engine.searchable(
                                    file.absolutePath,
                                    out.absolutePath,
                                    models.modelsDir.absolutePath,
                                    hw,
                                    cancel,
                                    progressListener,
                                ).onSuccess { workFile = out }
                            } else {
                                engine.ocr(
                                    file.absolutePath,
                                    models.modelsDir.absolutePath,
                                    hw,
                                    cancel,
                                    progressListener,
                                )
                            }
                        }
                    },
                ) { Text(stringResource(R.string.run_ocr)) }
            },
            dismissButton = {
                TextButton(onClick = { dialog = null }) {
                    Text(stringResource(R.string.cancel))
                }
            },
        )
    } else if (dialog == "handwriting_models") {
        var dlProgress by remember { mutableFloatStateOf(0f) }
        AlertDialog(
            onDismissRequest = { dialog = null },
            title = { Text(stringResource(R.string.download_handwriting_model)) },
            text = {
                LinearProgressIndicator(progress = { dlProgress }, modifier = Modifier.fillMaxWidth())
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        scope.launch {
                            models.downloadHandwriting { done, total ->
                                dlProgress =
                                    if (total > 0) {
                                        done.toFloat() / total.toFloat()
                                    } else {
                                        0f
                                    }
                            }
                            dialog = "ocr_opts"
                        }
                    },
                ) { Text(stringResource(R.string.download_handwriting_model)) }
            },
            dismissButton = {
                TextButton(onClick = { dialog = null }) {
                    Text(stringResource(R.string.cancel))
                }
            },
        )
    } else if (dialog == "models") {
        var dlProgress by remember { mutableFloatStateOf(0f) }
        AlertDialog(
            onDismissRequest = { dialog = null },
            title = { Text(stringResource(R.string.download_models)) },
            text = {
                Column {
                    LinearProgressIndicator(progress = { dlProgress }, modifier = Modifier.fillMaxWidth())
                }
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        scope.launch {
                            models.download { done, total ->
                                dlProgress =
                                    if (total > 0) {
                                        done.toFloat() / total.toFloat()
                                    } else {
                                        0f
                                    }
                            }
                            dialog = null
                        }
                    },
                ) { Text(stringResource(R.string.download_models)) }
            },
            dismissButton = {
                TextButton(onClick = { dialog = null }) {
                    Text(stringResource(R.string.cancel))
                }
            },
        )
    } else if (dialog == "sign") {
        AlertDialog(
            onDismissRequest = { dialog = null },
            title = { Text(stringResource(R.string.sign_digital_title)) },
            text = {
                Column {
                    TextButton(onClick = { pickSignP12.launch(arrayOf("*/*")) }) {
                        Text(stringResource(R.string.sign_pick_p12))
                    }
                    signP12?.let { Text(it.name) }
                    OutlinedTextField(
                        value = signPassword,
                        onValueChange = { signPassword = it },
                        label = { Text(stringResource(R.string.sign_password)) },
                        visualTransformation = PasswordVisualTransformation(),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        val src = workFile
                        val p12 = signP12
                        if (src == null || p12 == null) {
                            return@TextButton
                        }
                        dialog = null
                        runJob {
                            val cancel = cancelHandle ?: return@runJob
                            val out = engine.workCopy(src, "cms")
                            engine.digitalSignPdf(
                                src.absolutePath,
                                out.absolutePath,
                                p12.absolutePath,
                                signPassword,
                                cancel,
                                progressListener,
                            ).onSuccess { workFile = out }
                        }
                    },
                ) { Text(stringResource(R.string.sign_apply)) }
            },
            dismissButton = {
                TextButton(onClick = { dialog = null }) {
                    Text(stringResource(R.string.cancel))
                }
            },
        )
    } else if (dialog != null) {
        AlertDialog(
            onDismissRequest = { dialog = null },
            title = { Text(dialog ?: "") },
            confirmButton = {
                TextButton(onClick = { dialog = null }) { Text("OK") }
            },
        )
    }
}

private sealed interface Screen {
    data object Home : Screen
    data object Viewer : Screen
}