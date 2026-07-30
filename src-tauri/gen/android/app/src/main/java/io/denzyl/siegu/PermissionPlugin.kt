package io.denzyl.siegu

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

object PermissionCallback {
    var pendingCamera: ((Boolean) -> Unit)? = null
}

@TauriPlugin
class PermissionPlugin(private val activity: Activity): Plugin(activity) {
    @Command
    fun requestCamera(invoke: Invoke) {
        if (ContextCompat.checkSelfPermission(activity, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
            invoke.resolve(JSObject().apply { put("granted", true) })
            return
        }

        PermissionCallback.pendingCamera = { granted ->
            invoke.resolve(JSObject().apply { put("granted", granted) })
        }

        ActivityCompat.requestPermissions(
            activity,
            arrayOf(Manifest.permission.CAMERA),
            456
        )
    }
}
