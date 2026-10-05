package com.grtsinry43.memedock

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.grtsinry43.memedock.ui.MemeDockApp
import com.grtsinry43.memedock.app.MemeDockApplication

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val container = (application as MemeDockApplication).container
        setContent { MemeDockApp(container) }
    }
}
