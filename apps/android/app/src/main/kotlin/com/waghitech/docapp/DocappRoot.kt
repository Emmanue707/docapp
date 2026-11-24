package com.waghitech.docapp

import android.content.Intent
import android.net.Uri
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
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
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.waghitech.docapp.core.DocEngine
import com.waghitech.docapp.core.DownloadOutput
import com.waghitech.docapp.core.ModelDownloader
import com.waghitech.docapp.core.OutputNames
import com.waghitech.docapp.core.RecentEntry
import com.waghitech.docapp.core.RecentStore
import com.waghitech.docapp.core.ToolInput
import com.waghitech.docapp.core.ToolMime
import com.waghitech.docapp.core.ToolParams
import com.waghitech.docapp.core.ToolRunner
import com.waghitech.docapp.core.UriFiles
import com.waghitech.docapp.ui.DocTool
import com.waghitech.docapp.ui.HomeScreen
import com.waghitech.docapp.ui.ResultCard
import com.waghitech.docapp.ui.SavedOutput
import com.waghitech.docapp.ui.ToolsSheet
import com.waghitech.docapp.ui.ViewerScreen
import java.io.File
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import uniffi.docsuite_ffi.CancelHandle
import uniffi.docsuite_ffi.ProgressListener

private sealed interface Screen {
    data object Home : Screen
    data object Viewer : Screen
}

private data class QueuedTool(
    val tool: DocTool,
    val input: ToolInput,
    val pageCount: Int = 0,
)

private data class ResultState(
    val uri: Uri,
    val label: String,
    val mime: String,
    val cacheFile: File?,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DocappRoot(onPersistUri: (Uri) -> Unit) {
    val context = LocalContext.current
    val activity = context as ComponentActivity
    val scope = rememberCoroutineScope()
    val store = remember { RecentStore(context) }
    val engine = remember { DocEngine() }
    val models = remember { ModelDownloader(context) }
  val workDir = remember { File(context.cacheDir, "work").also { it.mkdirs() } }
    val runner = remember(workDir) { ToolRunner(engine, workDir, models.modelsDir) }
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
    var errorMsg by remember { mutableStateOf<String?>(null) }
    var result by remember { mutableStateOf<ResultState?>(null) }
    var pendingTool by remember { mutableStateOf<DocTool?>(null) }
    var queued by remember { mutableStateOf<QueuedTool?>(null) }
    var optDialog by remember { mutableStateOf<String?>(null) }
    var handwritingOcr by remember { mutableStateOf(false) }
    var signPassword by remember { mutableStateOf("") }
    var signP12 by remember { mutableStateOf<File?>(null) }
    var signName by remember { mutableStateOf("Signed") }
    var signReason by remember { mutableStateOf("") }
    var rangeText by remember { mutableStateOf("1") }
    var rotateDegrees by remember { mutableStateOf("90") }
    var reorderText by remember { mutableStateOf("") }
    var addTextValue by remember { mutableStateOf("") }
    var wordEditable by remember { mutableStateOf(false) }
    var stampImage by remember { mutableStateOf<File?>(null) }
    var mergeWith by remember { mutableStateOf<List<File>>(emptyList()) }

    fun fail(msg: String) {
        Log.e("Docapp", msg)
        errorMsg = msg
    }

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
                engine.pages(file.absolutePath).onSuccess { pageCount = it }.onFailure { fail(it.message ?: "pages") }
            }
        }
    }

    val openDoc =
        rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
            if (uri != null) {
                openUri(uri)
            }
        }

    val openToolOptions: (DocTool, ToolInput, Int) -> Unit = { tool, input, count ->
        when {
            tool.needsModels() && !models.ready() -> optDialog = "models"
            tool == DocTool.Ocr && handwritingOcr && !models.handwritingReady() -> optDialog = "handwriting_models"
            else -> {
                if (count > 0) {
                    reorderText = (1..count).joinToString(",")
                }
                promptOptions(tool, input, count) { optDialog = it }
            }
        }
    }

    val pickMulti =
        rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
            val tool = pendingTool
            pendingTool = null
            if (tool == null) {
                return@rememberLauncherForActivityResult
            }
            if (uris.isEmpty()) {
                fail(context.getString(R.string.empty_selection))
                return@rememberLauncherForActivityResult
            }
            val files = mergeWith.toMutableList()
            mergeWith = emptyList()
            var name = ""
            var type = ""
            uris.forEachIndexed { i, uri ->
                try {
                    onPersistUri(uri)
                } catch (_: SecurityException) {
                }
                val n = UriFiles.displayName(context.contentResolver, uri)
                val m = UriFiles.mime(context.contentResolver, uri)
                val cached = UriFiles.copyToCache(context, uri, n)
                if (files.isEmpty()) {
                    name = n
                    type = m
                }
                files.add(cached)
            }
            if (files.isEmpty()) {
                fail(context.getString(R.string.empty_selection))
                return@rememberLauncherForActivityResult
            }
            if (name.isBlank()) {
                name = files.first().name
                type = ToolMime.outputMime(name)
            }
            val input = ToolInput(files, name, type)
            scope.launch {
                val count =
                    if (UriFiles.isPdf(type)) {
                        engine.pages(files.first().absolutePath).getOrElse {
                            fail(it.message ?: "pages")
                            return@launch
                        }
                    } else {
                        0
                    }
                queued = QueuedTool(tool, input, count)
                openToolOptions(tool, input, count)
            }
        }

    val pickSingle =
        rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
            val tool = pendingTool
            pendingTool = null
            if (tool == null) {
                if (uri != null) {
                    openUri(uri)
                }
                return@rememberLauncherForActivityResult
            }
            if (uri == null) {
                return@rememberLauncherForActivityResult
            }
            try {
                onPersistUri(uri)
            } catch (_: SecurityException) {
            }
            val name = UriFiles.displayName(context.contentResolver, uri)
            val type = UriFiles.mime(context.contentResolver, uri)
            val file = UriFiles.copyToCache(context, uri, name)
            val input = ToolInput(listOf(file), name, type)
            scope.launch {
                val count =
                    if (UriFiles.isPdf(type)) {
                        engine.pages(file.absolutePath).getOrElse {
                            fail(it.message ?: "pages")
                            return@launch
                        }
                    } else {
                        0
                    }
                queued = QueuedTool(tool, input, count)
                openToolOptions(tool, input, count)
            }
        }

    val pickStampImage =
        rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
            if (uri == null) {
                return@rememberLauncherForActivityResult
            }
            try {
                onPersistUri(uri)
            } catch (_: SecurityException) {
            }
            val name = UriFiles.displayName(context.contentResolver, uri)
            stampImage = UriFiles.copyToCache(context, uri, name)
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
                    .onFailure { fail(it.message ?: "save") }
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
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Throwable) {
                    fail(e.message ?: e.toString())
                } finally {
                    busy = false
                    cancelHandle = null
                }
            }
    }

    suspend fun saveProduced(tool: DocTool, input: ToolInput, params: ToolParams) {
        val cancel = cancelHandle ?: return
        runner.run(tool, input, params, cancel, progressListener)
            .onFailure { fail(it.message ?: "failed") }
            .onSuccess { produced ->
                val saved = mutableListOf<Pair<Uri, String>>()
                val taken = mutableSetOf<String>()
                for (p in produced) {
                    val name = OutputNames.unique(p.publicName, taken)
                    taken.add(name)
                    val uri =
                        DownloadOutput.save(context, p.file, name, p.mime)
                            .getOrElse {
                                fail(it.message ?: "save")
                                return
                            }
                    saved.add(uri to name)
                }
                if (saved.isEmpty()) {
                    fail(context.getString(R.string.empty_selection))
                    return
                }
                val primary = saved.last()
                val label =
                    if (saved.size == 1) {
                        context.getString(R.string.saved_to, "${DownloadOutput.FOLDER_LABEL}/${primary.second}")
                    } else {
                        context.getString(R.string.saved_many, saved.size, DownloadOutput.FOLDER_LABEL)
                    }
                val openCache =
                    if (ToolMime.outputMime(primary.second) == "application/pdf") {
                        produced.lastOrNull()?.file
                    } else {
                        null
                    }
                result =
                    ResultState(
                        uri = primary.first,
                        label = label,
                        mime = ToolMime.outputMime(primary.second),
                        cacheFile = openCache,
                    )
            }
    }

    fun startHomeTool(tool: DocTool) {
        pendingTool = tool
        val types = ToolMime.openTypes(tool)
        if (ToolMime.multiSelect(tool)) {
            pickMulti.launch(types)
        } else {
            pickSingle.launch(types)
        }
    }

    fun startViewerTool(tool: DocTool) {
        val src = workFile ?: return
        val input = ToolInput(listOf(src), displayName, mime)
        queued = QueuedTool(tool, input, pageCount)
        when (tool) {
            DocTool.Save -> saveDoc.launch(displayName)
            DocTool.Share -> {
                val uri = sourceUri
                if (uri != null) {
                    val share =
                        Intent(Intent.ACTION_SEND).apply {
                            type = mime
                            putExtra(Intent.EXTRA_STREAM, uri)
                            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                        }
                    context.startActivity(Intent.createChooser(share, null))
                }
            }
            DocTool.Merge -> {
                mergeWith = listOf(src)
                pendingTool = tool
                pickMulti.launch(ToolMime.openTypes(tool))
            }
            DocTool.ImagesToPdf -> {
                pendingTool = tool
                pickMulti.launch(ToolMime.openTypes(tool))
            }
            DocTool.Ocr -> {
                if (!models.ready()) {
                    optDialog = "models"
                    return
                }
                optDialog = "ocr_opts"
            }
            DocTool.Sign -> optDialog = "sign"
            DocTool.AddImage -> optDialog = "stamp_image_pick"
            else -> promptOptions(tool, input, pageCount) { optDialog = it }
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
                                Text(stringResource(R.string.page_of, visiblePage, pageCount))
                            }
                        }
                    },
                    navigationIcon = {
                        TextButton(onClick = { screen = Screen.Home }) {
                            Text(stringResource(R.string.back))
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
                        tools = ToolMime.homeTools(),
                        onOpen = { openDoc.launch(arrayOf("application/pdf", "image/*", "*/*")) },
                        onRecent = { entry -> openUri(Uri.parse(entry.uri)) },
                        onTool = { startHomeTool(it) },
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
            val rs = result
            if (rs != null) {
                ResultCard(
                    output =
                        SavedOutput(
                            label = rs.label,
                            mime = rs.mime,
                            canOpenInApp = rs.cacheFile != null && UriFiles.isPdf(rs.mime),
                        ),
                    onOpen = {
                        val view =
                            Intent(Intent.ACTION_VIEW).apply {
                                setDataAndType(rs.uri, rs.mime)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            }
                        context.startActivity(view)
                    },
                    onShare = {
                        val share =
                            Intent(Intent.ACTION_SEND).apply {
                                type = rs.mime
                                putExtra(Intent.EXTRA_STREAM, rs.uri)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            }
                        context.startActivity(Intent.createChooser(share, null))
                    },
                    onViewInApp =
                        if (rs.cacheFile != null && UriFiles.isPdf(rs.mime)) {
                            {
                                workFile = rs.cacheFile
                                displayName = rs.cacheFile.name
                                mime = "application/pdf"
                                screen = Screen.Viewer
                                result = null
                                scope.launch {
                                    rs.cacheFile?.let { f ->
                                        engine.pages(f.absolutePath).onSuccess { pageCount = it }
                                    }
                                }
                            }
                        } else {
                            null
                        },
                    onDismiss = { result = null },
                )
            }
        }
    }

    ToolsSheet(
        visible = toolsOpen,
        mime = mime,
        onDismiss = { toolsOpen = false },
        onTool = { startViewerTool(it) },
    )

    optionDialogs(
        optDialog = optDialog,
        onDismiss = { optDialog = null },
        rangeText = rangeText,
        onRange = { rangeText = it },
        rotateDegrees = rotateDegrees,
        onRotateDegrees = { rotateDegrees = it },
        reorderText = reorderText,
        onReorder = { reorderText = it },
        addTextValue = addTextValue,
        onAddText = { addTextValue = it },
        wordEditable = wordEditable,
        onWordEditable = { wordEditable = it },
        handwritingOcr = handwritingOcr,
        onHandwriting = { handwritingOcr = it },
        signName = signName,
        onSignName = { signName = it },
        signReason = signReason,
        onSignReason = { signReason = it },
        signPassword = signPassword,
        onSignPassword = { signPassword = it },
        signP12 = signP12,
        onPickP12 = { pickSignP12.launch(arrayOf("*/*")) },
        stampImage = stampImage,
        onPickStamp = { pickStampImage.launch(arrayOf("image/*")) },
        onDownloadModels = {
            scope.launch {
                models.download { _, _ -> }
                optDialog = "ocr_opts"
            }
        },
        onDownloadHandwriting = {
            scope.launch {
                models.downloadHandwriting { _, _ -> }
                optDialog = "ocr_opts"
            }
        },
        onConfirm = { key ->
            val q = queued ?: return@optionDialogs
            if (key == "ocr_opts" && handwritingOcr && !models.handwritingReady()) {
                optDialog = "handwriting_models"
                return@optionDialogs
            }
            optDialog = null
            val params = paramsFromDialog(key, q, q.pageCount, rangeText, rotateDegrees, reorderText, addTextValue, wordEditable, handwritingOcr, signName, signReason, signP12, signPassword, stampImage, visiblePage)
            if (params == null) {
                fail(context.getString(R.string.missing_options))
                return@optionDialogs
            }
            runJob {
                saveProduced(q.tool, q.input, params)
            }
        },
    )

    val err = errorMsg
    if (err != null) {
        AlertDialog(
            onDismissRequest = { errorMsg = null },
            title = { Text(stringResource(R.string.error_title)) },
            text = { Text(err) },
            confirmButton = {
                TextButton(onClick = { errorMsg = null }) { Text("OK") }
            },
        )
    }
}

private fun DocTool.needsModels(): Boolean = this == DocTool.Ocr

private fun promptOptions(
    tool: DocTool,
    input: ToolInput,
    pageCount: Int,
    set: (String) -> Unit,
) {
    val d =
        when (tool) {
            DocTool.Split -> "split_opts"
            DocTool.Rotate -> "rotate_opts"
            DocTool.Delete -> "delete_opts"
            DocTool.Reorder -> "reorder_opts"
            DocTool.Crop -> "crop_opts"
            DocTool.AddText -> "text_opts"
            DocTool.AddImage -> "stamp_image_opts"
            DocTool.PdfToWord -> "word_opts"
            DocTool.Sign -> "sign"
            DocTool.Ocr -> "ocr_opts"
            DocTool.Merge,
            DocTool.ImagesToPdf,
            -> "run_merge"
            DocTool.OdtToWord,
            DocTool.WordToOdt,
            DocTool.ExtractText,
            -> "run_merge"
            else -> "run_merge"
        }
    if (tool == DocTool.Reorder && pageCount > 0) {
        // caller should set reorderText
    }
    set(d)
}

private fun paramsFromDialog(
    key: String,
    q: QueuedTool,
    pageCount: Int,
    rangeText: String,
    rotateDegrees: String,
    reorderText: String,
    addText: String,
    wordEditable: Boolean,
    handwriting: Boolean,
    signName: String,
    signReason: String,
    signP12: File?,
    signPassword: String,
    stampImage: File?,
    visiblePage: Int,
): ToolParams? =
    when (key) {
        "run_merge" -> ToolParams.None
        "split_opts" -> ToolParams.Split(rangeText)
        "rotate_opts" -> {
            val deg = rotateDegrees.toIntOrNull() ?: return null
            ToolParams.Rotate(deg, rangeText.ifBlank { null }, pageCount)
        }
        "delete_opts" -> ToolParams.Delete(rangeText, pageCount)
        "reorder_opts" -> ToolParams.Reorder(reorderText, pageCount)
        "crop_opts" -> ToolParams.Crop(visiblePage.coerceAtLeast(1), ToolRunner.defaultCropRect())
        "text_opts" ->
            if (addText.isBlank()) {
                null
            } else {
                ToolParams.AddText(visiblePage.coerceAtLeast(1), addText)
            }
        "stamp_image_opts" -> {
            val img = stampImage ?: return null
            ToolParams.AddImage(visiblePage.coerceAtLeast(1), img, ToolRunner.defaultStampRect())
        }
        "word_opts" -> ToolParams.PdfToWord(wordEditable)
        "ocr_opts" -> ToolParams.Ocr(handwriting)
        "sign_visible" ->
            ToolParams.SignVisible(
                visiblePage.coerceAtLeast(1),
                signName,
                signReason,
                stampImage,
                ToolRunner.defaultSignRect(),
            )
        "sign_digital" -> {
            val p12 = signP12 ?: return null
            ToolParams.SignDigital(p12, signPassword)
        }
        else -> ToolParams.None
    }

@Composable
private fun optionDialogs(
    optDialog: String?,
    onDismiss: () -> Unit,
    rangeText: String,
    onRange: (String) -> Unit,
    rotateDegrees: String,
    onRotateDegrees: (String) -> Unit,
    reorderText: String,
    onReorder: (String) -> Unit,
    addTextValue: String,
    onAddText: (String) -> Unit,
    wordEditable: Boolean,
    onWordEditable: (Boolean) -> Unit,
    handwritingOcr: Boolean,
    onHandwriting: (Boolean) -> Unit,
    signName: String,
    onSignName: (String) -> Unit,
    signReason: String,
    onSignReason: (String) -> Unit,
    signPassword: String,
    onSignPassword: (String) -> Unit,
    signP12: File?,
    onPickP12: () -> Unit,
    stampImage: File?,
    onPickStamp: () -> Unit,
    onDownloadModels: () -> Unit,
    onDownloadHandwriting: () -> Unit,
    onConfirm: (String) -> Unit,
) {
    when (optDialog) {
        "split_opts" ->
            textOptionDialog(
                title = stringResource(R.string.split),
                label = stringResource(R.string.page_ranges),
                value = rangeText,
                onValue = onRange,
                onDismiss = onDismiss,
                onConfirm = { onConfirm("split_opts") },
            )
        "rotate_opts" ->
            columnOptionDialog(
                title = stringResource(R.string.rotate),
                onDismiss = onDismiss,
                onConfirm = { onConfirm("rotate_opts") },
            ) {
                OutlinedTextField(
                    value = rotateDegrees,
                    onValueChange = onRotateDegrees,
                    label = { Text(stringResource(R.string.degrees)) },
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = rangeText,
                    onValueChange = onRange,
                    label = { Text(stringResource(R.string.page_ranges_optional)) },
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        "delete_opts" ->
            textOptionDialog(
                title = stringResource(R.string.delete_pages),
                label = stringResource(R.string.page_ranges),
                value = rangeText,
                onValue = onRange,
                onDismiss = onDismiss,
                onConfirm = { onConfirm("delete_opts") },
            )
        "reorder_opts" ->
            textOptionDialog(
                title = stringResource(R.string.reorder),
                label = stringResource(R.string.page_order),
                value = reorderText,
                onValue = onReorder,
                onDismiss = onDismiss,
                onConfirm = { onConfirm("reorder_opts") },
            )
        "text_opts" ->
            textOptionDialog(
                title = stringResource(R.string.add_text),
                label = stringResource(R.string.text_to_add),
                value = addTextValue,
                onValue = onAddText,
                onDismiss = onDismiss,
                onConfirm = { onConfirm("text_opts") },
            )
        "word_opts" ->
            columnOptionDialog(
                title = stringResource(R.string.pdf_to_word),
                onDismiss = onDismiss,
                onConfirm = { onConfirm("word_opts") },
            ) {
                Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                    Checkbox(checked = wordEditable, onCheckedChange = onWordEditable)
                    Text(stringResource(R.string.editable))
                }
                Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                    Checkbox(checked = !wordEditable, onCheckedChange = { onWordEditable(!it) })
                    Text(stringResource(R.string.exact_look))
                }
            }
        "ocr_opts" ->
            columnOptionDialog(
                title = stringResource(R.string.make_searchable),
                onDismiss = onDismiss,
                onConfirm = { onConfirm("ocr_opts") },
            ) {
                Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                    Checkbox(checked = handwritingOcr, onCheckedChange = onHandwriting)
                    Text(stringResource(R.string.handwriting_ocr))
                }
            }
        "stamp_image_pick" ->
            columnOptionDialog(
                title = stringResource(R.string.add_image),
                onDismiss = onDismiss,
                onConfirm = {
                    onDismiss()
                    if (stampImage != null) {
                        onConfirm("stamp_image_opts")
                    }
                },
            ) {
                TextButton(onClick = onPickStamp) {
                    Text(stringResource(R.string.pick_image))
                }
                stampImage?.let { Text(it.name) }
            }
        "sign" ->
            columnOptionDialog(
                title = stringResource(R.string.sign),
                onDismiss = onDismiss,
                onConfirm = { onConfirm("sign_visible") },
            ) {
                OutlinedTextField(
                    value = signName,
                    onValueChange = onSignName,
                    label = { Text(stringResource(R.string.sign_name)) },
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = signReason,
                    onValueChange = onSignReason,
                    label = { Text(stringResource(R.string.sign_reason)) },
                    modifier = Modifier.fillMaxWidth(),
                )
                TextButton(onClick = onPickStamp) {
                    Text(stringResource(R.string.pick_image_optional))
                }
                stampImage?.let { Text(it.name) }
                TextButton(onClick = onPickP12) {
                    Text(stringResource(R.string.sign_pick_p12))
                }
                signP12?.let { Text(it.name) }
                OutlinedTextField(
                    value = signPassword,
                    onValueChange = onSignPassword,
                    label = { Text(stringResource(R.string.sign_password)) },
                    visualTransformation = PasswordVisualTransformation(),
                    modifier = Modifier.fillMaxWidth(),
                )
                if (signP12 != null) {
                    TextButton(onClick = { onConfirm("sign_digital") }) {
                        Text(stringResource(R.string.sign_apply))
                    }
                }
            }
        "models" ->
            AlertDialog(
                onDismissRequest = onDismiss,
                title = { Text(stringResource(R.string.download_models)) },
                confirmButton = {
                    TextButton(onClick = onDownloadModels) {
                        Text(stringResource(R.string.download_models))
                    }
                },
                dismissButton = {
                    TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
                },
            )
        "handwriting_models" ->
            AlertDialog(
                onDismissRequest = onDismiss,
                title = { Text(stringResource(R.string.download_handwriting_model)) },
                confirmButton = {
                    TextButton(onClick = onDownloadHandwriting) {
                        Text(stringResource(R.string.download_handwriting_model))
                    }
                },
                dismissButton = {
                    TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) }
                },
            )
        "run_merge" -> {
            LaunchedEffect(Unit) {
                onConfirm("run_merge")
                onDismiss()
            }
        }
        "crop_opts" -> {
            LaunchedEffect(Unit) {
                onConfirm("crop_opts")
                onDismiss()
            }
        }
    }
}

@Composable
private fun textOptionDialog(
    title: String,
    label: String,
    value: String,
    onValue: (String) -> Unit,
    onDismiss: () -> Unit,
    onConfirm: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = {
            OutlinedTextField(
                value = value,
                onValueChange = onValue,
                label = { Text(label) },
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = { TextButton(onClick = onConfirm) { Text(stringResource(R.string.run_tool)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun columnOptionDialog(
    title: String,
    onDismiss: () -> Unit,
    onConfirm: () -> Unit,
    content: @Composable () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { Column { content() } },
        confirmButton = { TextButton(onClick = onConfirm) { Text(stringResource(R.string.run_tool)) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}