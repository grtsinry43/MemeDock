package com.grtsinry43.memedock.ui.navigation

import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.runtime.toMutableStateList

sealed interface Destination {
    data object Home : Destination
    data class Sticker(val id: String) : Destination
    data class Collection(val id: String, val name: String) : Destination
    data class Tag(val id: String, val name: String) : Destination
    data object Collections : Destination
    data object Trash : Destination
    data object Backup : Destination
}

// Each destination is a tag followed by a fixed number of arguments, so names may contain any text.
val BackStackSaver = Saver<SnapshotStateList<Destination>, ArrayList<String>>(
    save = { stack ->
        ArrayList<String>().apply {
            stack.forEach { destination ->
                when (destination) {
                    Destination.Home -> add("home")
                    is Destination.Sticker -> { add("sticker"); add(destination.id) }
                    is Destination.Collection -> { add("collection"); add(destination.id); add(destination.name) }
                    is Destination.Tag -> { add("tag"); add(destination.id); add(destination.name) }
                    Destination.Collections -> add("collections")
                    Destination.Trash -> add("trash")
                    Destination.Backup -> add("backup")
                }
            }
        }
    },
    restore = { saved ->
        val values = saved.iterator()
        buildList {
            while (values.hasNext()) {
                add(when (val tag = values.next()) {
                    "home" -> Destination.Home
                    "sticker" -> Destination.Sticker(values.next())
                    "collection" -> Destination.Collection(values.next(), values.next())
                    "tag" -> Destination.Tag(values.next(), values.next())
                    "collections" -> Destination.Collections
                    "trash" -> Destination.Trash
                    "backup" -> Destination.Backup
                    else -> error("Unknown destination $tag")
                })
            }
        }.toMutableStateList()
    },
)
