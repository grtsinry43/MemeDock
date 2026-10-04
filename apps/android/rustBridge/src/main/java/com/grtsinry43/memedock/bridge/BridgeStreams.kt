package com.grtsinry43.memedock.bridge

import com.grtsinry43.memedock.bridge.generated.Notification
import com.grtsinry43.memedock.bridge.generated.SubscriptionHandle
import com.grtsinry43.memedock.bridge.generated.TaskProgressHandle
import com.grtsinry43.memedock.bridge.generated.TaskSnapshot
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow

/** Transfers ownership of this subscription to one Flow collection. */
fun SubscriptionHandle.asFlow(): Flow<Notification> = flow {
    try {
        while (true) {
            val event = next()
            if (event is Notification.Closed) break
            emit(event)
        }
    } finally {
        try { unsubscribe() } finally { close() }
    }
}

/** Transfers ownership of this progress subscription to one Flow collection. */
fun TaskProgressHandle.asFlow(): Flow<TaskSnapshot> = flow {
    try {
        while (true) emit(next() ?: break)
    } finally {
        try { unsubscribe() } finally { close() }
    }
}
