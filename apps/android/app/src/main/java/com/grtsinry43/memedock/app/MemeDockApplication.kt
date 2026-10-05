package com.grtsinry43.memedock.app

import android.app.Application

class MemeDockApplication : Application() {
    lateinit var container: AppContainer
        private set
    override fun onCreate() {
        super.onCreate()
        container = AppContainer(this)
    }
}
