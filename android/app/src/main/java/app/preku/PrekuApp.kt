package app.preku

import android.app.Application
import app.preku.platform.Channels
import dagger.hilt.android.HiltAndroidApp

@HiltAndroidApp
class PrekuApp : Application() {
    override fun onCreate() {
        super.onCreate()
        Channels.create(this)
    }
}
