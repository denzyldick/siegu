package io.denzyl.siegu

import android.app.Activity
import android.app.WallpaperManager
import android.graphics.BitmapFactory
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke
import java.io.File

@TauriPlugin
class WallpaperPlugin(private val activity: Activity): Plugin(activity) {
    @Command
    fun setWallpaper(invoke: Invoke) {
        val path = invoke.getString("path")
        if (path == null) {
            invoke.reject("Missing path argument")
            return
        }

        try {
            val wallpaperManager = WallpaperManager.getInstance(activity)
            val bitmap = BitmapFactory.decodeFile(path)

            if (bitmap == null) {
                invoke.reject("Failed to decode image: file not found or unsupported format")
                return
            }

            wallpaperManager.setBitmap(bitmap)

            val ret = JSObject()
            ret.put("success", true)
            invoke.resolve(ret)
        } catch (e: SecurityException) {
            invoke.reject("Permission denied: ${e.message}")
        } catch (e: Exception) {
            invoke.reject(e.message ?: "Unknown error setting wallpaper")
        }
    }
}
