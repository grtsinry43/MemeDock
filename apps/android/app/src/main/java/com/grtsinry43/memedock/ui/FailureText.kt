package com.grtsinry43.memedock.ui

import android.content.ActivityNotFoundException
import androidx.annotation.StringRes
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import com.grtsinry43.memedock.R
import com.grtsinry43.memedock.data.library.LibraryFailure

@Composable
fun failureText(code: String?): String = stringResource(failureTextRes(code))

@StringRes
fun failureTextRes(code: String?): Int = when (code?.uppercase()) {
    "PERMISSION_DENIED" -> R.string.failure_permission
    "IO" -> R.string.failure_io
    "STORAGE_FULL" -> R.string.failure_full
    "UNSUPPORTED_FORMAT" -> R.string.failure_format
    "UNSUPPORTED_COLOR_PROFILE" -> R.string.failure_color_profile
    "NO_SAVE_TARGET" -> R.string.failure_save_target
    "INVALID_IMAGE" -> R.string.failure_image
    "RESOURCE_LIMIT" -> R.string.failure_limit
    "BATCH_LIMIT" -> R.string.failure_batch
    "BUSY" -> R.string.failure_busy
    "BATCH_BUSY" -> R.string.error_batch_busy
    "NO_SHARE_TARGET" -> R.string.error_share_target
    "NOT_FOUND" -> R.string.failure_missing
    "CONFLICT" -> R.string.failure_changed
    "ENTITY_DELETED" -> R.string.failure_deleted
    "INVALID_INPUT" -> R.string.failure_input
    "CORRUPT_DATA", "UNSUPPORTED_SCHEMA", "DATABASE" -> R.string.failure_library
    else -> R.string.failure_unknown
}

/** Maps library and platform errors from output and management work to failure codes. */
fun failureCode(error: Exception): String = when (error) {
    is LibraryFailure -> error.reason
    is ActivityNotFoundException -> "NO_SHARE_TARGET"
    is SecurityException -> "PERMISSION_DENIED"
    is java.io.IOException -> "IO"
    else -> "INTERNAL"
}
