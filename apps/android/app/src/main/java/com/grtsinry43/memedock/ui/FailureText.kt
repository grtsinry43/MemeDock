package com.grtsinry43.memedock.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.grtsinry43.memedock.R

@Composable
fun failureText(code: String?): String = stringResource(when (code?.uppercase()) {
    "PERMISSION_DENIED" -> R.string.failure_permission
    "IO" -> R.string.failure_io
    "STORAGE_FULL" -> R.string.failure_full
    "UNSUPPORTED_FORMAT" -> R.string.failure_format
    "INVALID_IMAGE" -> R.string.failure_image
    "RESOURCE_LIMIT" -> R.string.failure_limit
    "BATCH_LIMIT" -> R.string.failure_batch
    "BUSY" -> R.string.failure_busy
    "BATCH_BUSY" -> R.string.error_batch_busy
    "NO_SHARE_TARGET" -> R.string.error_share_target
    "NOT_FOUND" -> R.string.failure_missing
    "CORRUPT_DATA", "UNSUPPORTED_SCHEMA", "DATABASE" -> R.string.failure_library
    else -> R.string.failure_unknown
})
