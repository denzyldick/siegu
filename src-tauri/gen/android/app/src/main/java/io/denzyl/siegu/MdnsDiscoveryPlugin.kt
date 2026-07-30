package io.denzyl.siegu

import android.app.Activity
import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Handler
import android.os.Looper
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@TauriPlugin
class MdnsDiscoveryPlugin(private val activity: Activity): Plugin(activity) {
    companion object {
        private const val SERVICE_TYPE = "_siegu._tcp."
        private const val DISCOVERY_TIMEOUT_MS = 3000L
        private const val RESOLVE_TIMEOUT_MS = 2000L
    }

    @Command
    fun ping(invoke: Invoke) {
        android.util.Log.i("siegu-mdns", "Ping received")
        invoke.resolve(JSObject().apply { put("ok", true) })
    }

    @Command
    fun discover(invoke: Invoke) {
        val args = invoke.getArgs()
        val timeoutSecs = maxOf(args.optInt("timeoutSecs", 2), 2)
        val ctx = activity as Context

        val nsdManager = ctx.getSystemService(Context.NSD_SERVICE) as? NsdManager ?: run {
            invoke.reject("NsdManager not available")
            return
        }

        val discovered = CopyOnWriteArrayList<NsdServiceInfo>()
        val latch = CountDownLatch(1)

        val listener = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(regType: String) {
                android.util.Log.i("siegu-mdns", "Discovery started: $regType")
            }
            override fun onServiceFound(service: NsdServiceInfo) {
                android.util.Log.i("siegu-mdns", "Found: ${service.serviceName}")
                discovered.add(service)
            }
            override fun onServiceLost(service: NsdServiceInfo) {}
            override fun onDiscoveryStopped(regType: String) {
                latch.countDown()
            }
            override fun onStartDiscoveryFailed(regType: String, errorCode: Int) {
                android.util.Log.e("siegu-mdns", "Discovery FAILED: $regType error=$errorCode")
                latch.countDown()
            }
            override fun onStopDiscoveryFailed(regType: String, errorCode: Int) {}
        }

        nsdManager.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, listener)

        Thread {
            latch.await(timeoutSecs * 1000L, TimeUnit.MILLISECONDS)

            val stopLatch = CountDownLatch(1)
            Handler(Looper.getMainLooper()).post {
                try { nsdManager.stopServiceDiscovery(listener) } catch (_: Exception) {}
                stopLatch.countDown()
            }
            stopLatch.await(500, TimeUnit.MILLISECONDS)

            // Resolve sequentially (typically 1 host)
            val hosts = JSONArray()
            for (svc in discovered) {
                val info = resolveService(nsdManager, svc)
                if (info != null) hosts.put(info)
            }

            android.util.Log.i("siegu-mdns", "Returning ${hosts.length()} hosts")
            invoke.resolve(JSObject().apply {
                put("hosts", hosts.toString())
            })
        }.start()
    }

    private fun resolveService(nsdManager: NsdManager, service: NsdServiceInfo): JSONObject? {
        val latch = CountDownLatch(1)
        var info: JSONObject? = null

        val listener = object : NsdManager.ResolveListener {
            override fun onServiceResolved(svc: NsdServiceInfo) {
                info = JSONObject().apply {
                    put("name", svc.serviceName ?: "")
                    put("ip", svc.host?.hostAddress ?: "")
                    put("port", svc.port)
                }
                latch.countDown()
            }
            override fun onResolveFailed(svc: NsdServiceInfo, errorCode: Int) {
                latch.countDown()
            }
        }

        Handler(Looper.getMainLooper()).post {
            nsdManager.resolveService(service, listener)
        }

        latch.await(RESOLVE_TIMEOUT_MS, TimeUnit.MILLISECONDS)
        return info
    }
}
