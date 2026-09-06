package app.operit

import android.content.Context
import android.graphics.Bitmap
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager
import android.hardware.display.VirtualDisplay
import android.media.Image
import android.media.ImageReader
import android.os.Handler
import android.os.HandlerThread
import android.util.DisplayMetrics
import android.view.WindowManager
import java.io.File

/** Each named legacy UI agent owns an actual offscreen Android display. */
object LegacyVirtualDisplays {
    private class Session(val name: String, val reader: ImageReader) {
        lateinit var display: VirtualDisplay
        var frame: Image? = null
        var closed = false
        val lock = Object()
    }
    private val sessions = mutableMapOf<String, Session>()
    private val imageThread by lazy { HandlerThread("operit-legacy-displays").apply { start() } }
    private val imageHandler by lazy { Handler(imageThread.looper) }

    @JvmStatic
    @Synchronized
    fun ensure(context: Context, agentId: String): Int {
        require(agentId.isNotBlank() && agentId != "default") { "A named agent is required for a virtual display" }
        sessions[agentId]?.let { return it.display.display.displayId }
        val windowManager = context.getSystemService(Context.WINDOW_SERVICE) as WindowManager
        val displayManager = context.getSystemService(Context.DISPLAY_SERVICE) as DisplayManager
        val metrics = DisplayMetrics()
        @Suppress("DEPRECATION")
        windowManager.defaultDisplay.getRealMetrics(metrics)
        val reader = ImageReader.newInstance(metrics.widthPixels, metrics.heightPixels, PixelFormat.RGBA_8888, 3)
        val session = Session(agentId, reader)
        reader.setOnImageAvailableListener({ source ->
            synchronized(session.lock) {
                if (!session.closed) {
                    val latest = source.acquireLatestImage()
                    if (latest != null) {
                        session.frame?.close()
                        session.frame = latest
                        session.lock.notifyAll()
                    }
                }
            }
        }, imageHandler)
        try {
            // OWN_CONTENT_ONLY prevents mirroring the user's primary display and needs no capture bypass.
            val flags = DisplayManager.VIRTUAL_DISPLAY_FLAG_PUBLIC or
                DisplayManager.VIRTUAL_DISPLAY_FLAG_PRESENTATION or DisplayManager.VIRTUAL_DISPLAY_FLAG_OWN_CONTENT_ONLY
            session.display = requireNotNull(displayManager.createVirtualDisplay(
                "Operit Agent $agentId", metrics.widthPixels, metrics.heightPixels,
                metrics.densityDpi, reader.surface, flags,
            )) { "Android did not create the agent display" }
            sessions[agentId] = session
            return session.display.display.displayId
        } catch (error: Throwable) {
            synchronized(session.lock) { session.closed = true; session.frame?.close(); session.frame = null }
            reader.close()
            throw error
        }
    }

    @JvmStatic
    @Synchronized
    fun resolveDisplayId(agentId: String): Int? = sessions[agentId]?.display?.display?.displayId

    @JvmStatic
    fun capture(context: Context, displayId: Int): String {
        val session = synchronized(this) {
            sessions.values.firstOrNull { it.display.display.displayId == displayId }
                ?: error("No virtual agent owns display $displayId")
        }
        val bitmap = synchronized(session.lock) {
            val deadline = android.os.SystemClock.uptimeMillis() + 3000
            while (session.frame == null && !session.closed) {
                val remaining = deadline - android.os.SystemClock.uptimeMillis()
                if (remaining <= 0) break
                session.lock.wait(remaining)
            }
            val frame = session.frame ?: error("Virtual display has no frame; start an activity on display $displayId first")
            val plane = frame.planes[0]
            val width = frame.width
            val height = frame.height
            val paddedWidth = width + (plane.rowStride - plane.pixelStride * width) / plane.pixelStride
            val padded = Bitmap.createBitmap(paddedWidth, height, Bitmap.Config.ARGB_8888)
            plane.buffer.rewind()
            padded.copyPixelsFromBuffer(plane.buffer)
            if (paddedWidth == width) padded else Bitmap.createBitmap(padded, 0, 0, width, height).also { padded.recycle() }
        }
        try {
            val directory = File(context.cacheDir, "legacy-ui").apply { mkdirs() }
            val output = File(directory, "display-${displayId}-${System.nanoTime()}.png")
            output.outputStream().use { require(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) { "Cannot encode agent screenshot" } }
            return output.absolutePath
        } finally { bitmap.recycle() }
    }

    @JvmStatic
    @Synchronized
    fun release(agentId: String): Boolean {
        val session = sessions.remove(agentId) ?: return false
        synchronized(session.lock) {
            session.closed = true
            session.frame?.close()
            session.frame = null
            session.lock.notifyAll()
        }
        session.reader.setOnImageAvailableListener(null, null)
        session.display.release()
        session.reader.close()
        return true
    }
}
