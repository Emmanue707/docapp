package com.waghitech.docapp

import android.app.Application
import uniffi.docsuite_ffi.version

class DocappApp : Application() {
    override fun onCreate() {
        super.onCreate()
        version()
    }
}