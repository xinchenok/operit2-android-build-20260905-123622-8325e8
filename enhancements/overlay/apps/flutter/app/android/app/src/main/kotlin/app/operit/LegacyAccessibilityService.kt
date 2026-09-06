package app.operit

import android.accessibilityservice.AccessibilityService
import android.view.accessibility.AccessibilityEvent
import android.os.Build
import android.view.Display
import java.util.concurrent.ConcurrentHashMap

/** Enabled only through Android's user-controlled accessibility settings. */
class LegacyAccessibilityService : AccessibilityService() {
    companion object {
        @Volatile var current: LegacyAccessibilityService? = null
            private set
    }
    private val activities = ConcurrentHashMap<Int, String>()

    fun activityNameForDisplay(displayId: Int): String = activities[displayId].orEmpty()

    override fun onServiceConnected() {
        super.onServiceConnected()
        current = this
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        if (event?.eventType == AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED) {
            var displayId = Display.DEFAULT_DISPLAY
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                val all = windowsOnAllDisplays
                for (index in 0 until all.size()) {
                    if (all.valueAt(index).any { it.id == event.windowId }) {
                        displayId = all.keyAt(index)
                        break
                    }
                }
            }
            activities[displayId] = event.className?.toString().orEmpty()
        }
    }

    override fun onInterrupt() = Unit

    override fun onDestroy() {
        if (current === this) current = null
        super.onDestroy()
    }
}
