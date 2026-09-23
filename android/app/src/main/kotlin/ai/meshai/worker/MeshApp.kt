package ai.meshai.worker

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager

class MeshApp : Application() {
    override fun onCreate() {
        super.onCreate()
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "MeshAI worker", NotificationManager.IMPORTANCE_LOW).apply {
            description = "Shown while this phone is part of a mesh"
        })
    }
    companion object { const val CHANNEL = "meshai.worker" }
}
