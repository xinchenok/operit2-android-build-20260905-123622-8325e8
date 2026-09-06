package app.operit

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.app.ActivityOptions
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.Path
import android.graphics.Rect
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.system.Os
import android.system.ErrnoException
import android.os.Process
import android.view.Display
import android.view.KeyEvent
import android.view.accessibility.AccessibilityNodeInfo
import app.operit.core.tools.system.AndroidPrivilegedCommandExecutor
import app.operit.core.tools.system.AndroidPrivilegedCommandResult
import app.operit.core.tools.system.AndroidPrivilegedCommandTarget
import java.io.File
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import org.json.JSONArray
import org.json.JSONObject

/** Android implementations used by the enhanced legacy package bridge. */
object LegacyAndroidTools {
    private val mainHandler = Handler(Looper.getMainLooper())
    private val screenshotExecutor = Executors.newSingleThreadExecutor()

    @JvmStatic
    fun execute(context: Context, requestJson: String): String {
        return try {
            check(Looper.myLooper() != Looper.getMainLooper()) {
                "Legacy Android tools must run on the package worker thread"
            }
            val request = JSONObject(requestJson)
            val data: Any = when (val action = request.getString("action")) {
                "shell" -> shell(context, request)
                "resolveFilePath" -> resolveFilePath(context, request)
                "intent" -> launchIntent(context, request, false)
                "sendBroadcast" -> launchIntent(context, request, true)
                "captureScreenshot" -> captureScreenshot(context, request.optInt("displayId", Display.DEFAULT_DISPLAY))
                "getPageInfo" -> pageInfo(requireAccessibility(context), request.optInt("displayId", Display.DEFAULT_DISPLAY))
                "tap", "longPress", "swipe" -> gesture(context, request, action)
                "clickElement" -> clickElement(context, request)
                "setText" -> setText(context, request)
                "pressKey" -> pressKey(context, request)
                "authorizeAccessibility" -> {
                    openAccessibilitySettings(context)
                    JSONObject().put("enabled", LegacyAccessibilityService.current != null)
                }
                else -> throw IllegalArgumentException("Unknown legacy Android action: $action")
            }
            JSONObject().put("success", true).put("data", data).toString()
        } catch (error: Exception) {
            JSONObject().put("success", false)
                .put("error", error.message ?: error.javaClass.simpleName).toString()
        }
    }

    /** Resolves Linux paths against the installed PRoot and its actual host binds. */
    private fun resolveFilePath(context: Context, request: JSONObject): String {
        val path = request.getString("path")
        when (val environment = request.optString("environment", "android")) {
            "android" -> return path
            "linux" -> Unit
            else -> throw IllegalArgumentException("Unknown file environment: $environment")
        }
        require(path.startsWith("/")) { "Linux file paths must be absolute" }
        val rootfs = Os.getenv("OPERIT_ANDROID_ROOTFS_DIR")
            ?: throw IllegalStateException("Android Linux runtime has not been initialized")
        check(File(rootfs).isDirectory) { "Installed Android Linux rootfs is missing" }
        val user = Process.myUid() / 100000
        val storage = "/storage/emulated/$user"
        val binds = mutableListOf(
            "/sdcard" to storage,
            storage to storage,
            "/data/user/$user/${context.packageName}" to context.applicationInfo.dataDir,
            "/data/data/${context.packageName}" to context.applicationInfo.dataDir,
            context.filesDir.absolutePath to context.filesDir.absolutePath,
            "/dev/pts" to "/dev/pts",
            "/dev/fd" to "/proc/self/fd",
            "/dev/stdin" to "/proc/self/fd/0",
            "/dev/stdout" to "/proc/self/fd/1",
            "/dev/stderr" to "/proc/self/fd/2",
            "/dev" to "/dev", "/proc" to "/proc", "/sys" to "/sys",
            "/data/local/tmp" to "/data/local/tmp",
        )
        for (key in listOf("OPERIT_ANDROID_RUNTIME_ROOT", "OPERIT_ANDROID_WORKSPACE_ROOT")) {
            Os.getenv(key)?.let { binds.add(it to it) }
        }
        binds.sortByDescending { it.first.length }
        fun normalized(value: String): String =
            "/" + value.split('/').filter { it.isNotEmpty() && it != "." }.joinToString("/")
        fun publicPath(physical: String): String {
            if (physical == "/sdcard" || physical.startsWith("/sdcard/") ||
                physical == "/data" || physical.startsWith("/data/")) return physical
            if (physical == storage || physical.startsWith("$storage/")) return "/sdcard" + physical.removePrefix(storage)
            return "/mnt/android/root" + physical
        }
        fun boundPath(guestPath: String): String {
            val bind = binds.firstOrNull { guestPath == it.first || guestPath.startsWith(it.first + "/") }
            return if (bind != null) bind.second + guestPath.removePrefix(bind.first)
                else rootfs.trimEnd('/') + guestPath
        }
        var guest = normalized(path)
        var followedLinks = 0
        resolve@ while (true) {
            val parts = guest.trimStart('/').split('/').filter { it.isNotEmpty() }
            for (index in parts.indices) {
                if (parts[index] == "..") {
                    // Resolve a preceding symlink before interpreting its parent.
                    // Lexically collapsing link/.. first can overwrite another file.
                    val parent = parts.take(index).dropLast(1)
                    guest = "/" + (parent + parts.drop(index + 1)).joinToString("/")
                    continue@resolve
                }
                val prefix = "/" + parts.take(index + 1).joinToString("/")
                val physicalPrefix = boundPath(prefix)
                if (Regex("^/proc/(self|thread-self|[0-9]+)/fd(/|$)").containsMatchIn(physicalPrefix)) {
                    // Proc fd links can refer to pipes or deleted files; opening the
                    // descriptor is meaningful, reopening its readlink text is not.
                    return publicPath(boundPath(guest))
                }
                val target = try { Os.readlink(physicalPrefix) } catch (_: ErrnoException) { null }
                if (target != null) {
                    followedLinks += 1
                    check(followedLinks <= 40) { "Too many Linux symbolic links while resolving $path" }
                    val parent = "/" + parts.take(index).joinToString("/")
                    val tail = parts.drop(index + 1).joinToString("/")
                    guest = normalized((if (target.startsWith("/")) target else "$parent/$target") + "/" + tail)
                    continue@resolve
                }
            }
            return publicPath(boundPath(guest))
        }
    }

    private fun shell(context: Context, request: JSONObject): JSONObject {
        val command = request.getString("command")
        require(command.isNotBlank()) { "shell command must not be blank" }
        val timeout = request.optLong("timeoutMs", 60_000L).coerceAtLeast(1L)
        val requested = request.optString("target", "auto")
        val target = when (requested) {
            "root", "root_exec", "root_libsu" -> {
                check(AndroidPrivilegeAuthorization.isRootAuthorized(context)) {
                    "请先在 Operit2 的 Android 权限设置中授权 Root"
                }
                AndroidPrivilegedCommandTarget.RootExec
            }
            "shizuku" -> {
                check(AndroidPrivilegeAuthorization.isShizukuAuthorized()) {
                    "请先启动并授权 Shizuku"
                }
                AndroidPrivilegedCommandTarget.Shizuku
            }
            "shell", "normal" -> null
            "auto" -> when {
                AndroidPrivilegeAuthorization.isRootAuthorized(context) -> AndroidPrivilegedCommandTarget.RootExec
                AndroidPrivilegeAuthorization.isShizukuAuthorized() -> AndroidPrivilegedCommandTarget.Shizuku
                else -> null
            }
            else -> throw IllegalArgumentException("Unknown shell target: $requested")
        }
        val result = if (target != null) {
            AndroidPrivilegedCommandExecutor.execute(target, command, timeout)
        } else {
            val process = ProcessBuilder("/system/bin/sh", "-c", command).start()
            val workers = Executors.newFixedThreadPool(2)
            try {
                val stdout = workers.submit<ByteArray> { process.inputStream.use { it.readBytes() } }
                val stderr = workers.submit<ByteArray> { process.errorStream.use { it.readBytes() } }
                check(process.waitFor(timeout, TimeUnit.MILLISECONDS)) {
                    process.destroyForcibly()
                    "shell command timed out after $timeout ms"
                }
                AndroidPrivilegedCommandResult(
                    stdout.get(5, TimeUnit.SECONDS), stderr.get(5, TimeUnit.SECONDS), process.exitValue(),
                )
            } finally {
                if (process.isAlive) process.destroyForcibly()
                workers.shutdownNow()
            }
        }
        return JSONObject().put("command", command)
            .put("output", result.stdoutText() + result.stderrText())
            .put("stdout", result.stdoutText()).put("stderr", result.stderrText())
            .put("exitCode", result.exitCode)
    }

    private fun launchIntent(context: Context, request: JSONObject, forceBroadcast: Boolean): JSONObject {
        val options = request.optJSONObject("options") ?: request
        val type = if (forceBroadcast) "broadcast" else options.optString("type", "activity")
        val action = options.optString("intentAction", options.optString("action", ""))
            .takeUnless { it == "intent" || it == "sendBroadcast" }.orEmpty()
        val uriText = options.optString("uri", options.optString("data", ""))
        val intent = if (uriText.startsWith("intent:")) Intent.parseUri(uriText, Intent.URI_INTENT_SCHEME) else Intent()
        if (action.isNotBlank()) intent.action = action
        if (uriText.isNotBlank() && !uriText.startsWith("intent:")) intent.data = Uri.parse(uriText)
        val mimeType = options.optString("mimeType", options.optString("mime_type", ""))
        if (mimeType.isNotBlank()) intent.setDataAndType(intent.data, mimeType)
        val packageName = options.optString("package_name", options.optString("packageName", options.optString("package", "")))
        if (packageName.isNotBlank()) {
            intent.setPackage(packageName)
            if (intent.action.isNullOrBlank() && uriText.isBlank() && options.optString("component", "").isBlank()) {
                intent.action = Intent.ACTION_MAIN
                intent.addCategory(Intent.CATEGORY_LAUNCHER)
            }
        }
        val component = options.optString("component", "")
        if (component.isNotBlank()) {
            intent.component = ComponentName.unflattenFromString(component)
                ?: if (packageName.isNotBlank()) ComponentName(packageName, component) else null
            require(intent.component != null) { "Invalid Android component: $component" }
        }
        val categories = options.optJSONArray("categories")
        if (categories != null) for (index in 0 until categories.length()) intent.addCategory(categories.getString(index))
        intent.addFlags(options.optInt("flags", 0))
        val extras = options.optJSONObject("extras") ?: JSONObject()
        for (key in extras.keys()) putExtra(intent, key, extras.get(key))
        when (type) {
            "activity" -> {
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                val displayId = options.optInt("displayId", request.optInt("displayId", Display.DEFAULT_DISPLAY))
                if (displayId != Display.DEFAULT_DISPLAY) {
                    check(Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) { "Virtual display activities require Android 8 or later" }
                    val activityOptions = ActivityOptions.makeBasic().apply { launchDisplayId = displayId }
                    context.startActivity(intent, activityOptions.toBundle())
                } else context.startActivity(intent)
            }
            "broadcast" -> context.sendBroadcast(intent)
            "service" -> context.startService(intent)
            else -> throw IllegalArgumentException("Unknown intent type: $type")
        }
        return JSONObject().put("action", intent.action.orEmpty()).put("uri", uriText)
            .put("package_name", packageName).put("component", component)
            .put("flags", intent.flags).put("extras_count", extras.length())
            .put("type", type).put("result", "dispatched")
    }

    private fun putExtra(intent: Intent, key: String, value: Any) {
        when (value) {
            JSONObject.NULL -> intent.putExtra(key, null as String?)
            is Boolean -> intent.putExtra(key, value)
            is Int -> intent.putExtra(key, value)
            is Long -> intent.putExtra(key, value)
            is Number -> intent.putExtra(key, value.toDouble())
            is String -> intent.putExtra(key, value)
            is JSONArray -> {
                val values = (0 until value.length()).map { value.get(it) }
                when {
                    values.all { it is String } -> intent.putExtra(key, values.map { it as String }.toTypedArray())
                    values.all { it is Int } -> intent.putExtra(key, values.map { it as Int }.toIntArray())
                    values.all { it is Number } -> intent.putExtra(key, values.map { (it as Number).toDouble() }.toDoubleArray())
                    else -> intent.putExtra(key, value.toString())
                }
            }
            is JSONObject -> intent.putExtra(key, value.toString())
            else -> throw IllegalArgumentException("Unsupported intent extra: $key")
        }
    }

    private fun requireAccessibility(context: Context): LegacyAccessibilityService {
        val service = LegacyAccessibilityService.current
        if (service != null) return service
        openAccessibilitySettings(context)
        throw IllegalStateException("请在刚打开的无障碍设置中启用 Operit2 增强版 UI 自动操作服务，然后重试")
    }

    private fun openAccessibilitySettings(context: Context) {
        context.startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }

    private fun displayRoot(service: LegacyAccessibilityService, displayId: Int): AccessibilityNodeInfo {
        if (displayId == Display.DEFAULT_DISPLAY) {
            return service.rootInActiveWindow ?: throw IllegalStateException("当前屏幕没有可读取的无障碍窗口")
        }
        check(Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) { "Virtual screen UI inspection requires Android 11 or later" }
        val windows = service.windowsOnAllDisplays.get(displayId).orEmpty()
        for (window in windows.sortedByDescending { it.isActive || it.isFocused }) {
            window.root?.let { return it }
        }
        throw IllegalStateException("虚拟屏幕 $displayId 暂无可读取窗口；请先在该屏幕启动目标应用")
    }

    private fun pageInfo(service: LegacyAccessibilityService, displayId: Int): JSONObject {
        val root = displayRoot(service, displayId)
        return try {
            JSONObject().put("packageName", root.packageName?.toString().orEmpty())
                .put("activityName", service.activityNameForDisplay(displayId)).put("uiElements", nodeJson(root, 0))
                .put("displayId", displayId)
        } finally {
            @Suppress("DEPRECATION")
            root.recycle()
        }
    }

    private fun nodeJson(node: AccessibilityNodeInfo, depth: Int): JSONObject {
        val rect = Rect().also { node.getBoundsInScreen(it) }
        val children = JSONArray()
        if (depth < 80) for (index in 0 until node.childCount) {
            val child = node.getChild(index) ?: continue
            try { if (child.isVisibleToUser) children.put(nodeJson(child, depth + 1)) }
            finally { @Suppress("DEPRECATION") child.recycle() }
        }
        return JSONObject().put("className", node.className?.toString().orEmpty())
            .put("text", node.text?.toString().orEmpty())
            .put("contentDesc", node.contentDescription?.toString().orEmpty())
            .put("resourceId", node.viewIdResourceName.orEmpty())
            .put("bounds", "[${rect.left},${rect.top}][${rect.right},${rect.bottom}]")
            .put("isClickable", node.isClickable).put("isEditable", node.isEditable)
            .put("isEnabled", node.isEnabled).put("children", children)
    }

    private fun <T> withSelectedNode(service: LegacyAccessibilityService, selector: JSONObject, displayId: Int, block: (AccessibilityNodeInfo) -> T): T {
        val root = displayRoot(service, displayId)
        val nodes = mutableListOf(root)
        try {
            var cursor = 0
            var matched = 0
            val wanted = selector.optInt("index", 0).coerceAtLeast(0)
            while (cursor < nodes.size) {
                val node = nodes[cursor++]
                if (node.isVisibleToUser && matches(node, selector)) {
                    if (matched++ == wanted) return block(node)
                }
                for (index in 0 until node.childCount) node.getChild(index)?.let { nodes.add(it) }
            }
            throw IllegalStateException("当前页面未找到符合条件的控件: $selector")
        } finally {
            for (node in nodes) { @Suppress("DEPRECATION") node.recycle() }
        }
    }

    private fun matches(node: AccessibilityNodeInfo, selector: JSONObject): Boolean {
        val partial = selector.optBoolean("partialMatch", false)
        fun stringMatch(key: String, actual: String?): Boolean {
            if (!selector.has(key)) return true
            val expected = selector.getString(key)
            return if (partial) actual.orEmpty().contains(expected) else actual.orEmpty() == expected
        }
        return stringMatch("resourceId", node.viewIdResourceName) &&
            stringMatch("className", node.className?.toString()) &&
            stringMatch("text", node.text?.toString()) &&
            stringMatch("contentDesc", node.contentDescription?.toString()) &&
            (!selector.has("isClickable") || node.isClickable == selector.getBoolean("isClickable")) &&
            (!selector.optBoolean("editableOnly", false) || node.isEditable) &&
            (!selector.optBoolean("focusedOnly", false) || node.isFocused)
    }

    private fun clickElement(context: Context, request: JSONObject): JSONObject {
        val selector = request.optJSONObject("selector") ?: request.optJSONObject("params") ?: request
        val displayId = request.optInt("displayId", Display.DEFAULT_DISPLAY)
        if (selector.has("bounds")) {
            val values = Regex("-?\\d+").findAll(selector.getString("bounds")).map { it.value.toInt() }.toList()
            require(values.size == 4) { "bounds must be [left,top][right,bottom]" }
            return gesture(context, JSONObject().put("x", (values[0] + values[2]) / 2)
                .put("y", (values[1] + values[3]) / 2).put("displayId", displayId), "tap")
        }
        val service = requireAccessibility(context)
        return withSelectedNode(service, selector, displayId) { node ->
            val rect = Rect().also { node.getBoundsInScreen(it) }
            var clicked = node.performAction(AccessibilityNodeInfo.ACTION_CLICK)
            if (!clicked) {
                var parent = node.parent
                while (parent != null) {
                    val current = parent
                    try {
                        if (current.isClickable && current.performAction(AccessibilityNodeInfo.ACTION_CLICK)) {
                            clicked = true
                            break
                        }
                        parent = current.parent
                    } finally { @Suppress("DEPRECATION") current.recycle() }
                }
            }
            if (!clicked) dispatchGesture(service, rect.centerX().toFloat(), rect.centerY().toFloat(), rect.centerX().toFloat(), rect.centerY().toFloat(), 80, displayId)
            actionResult("clickElement", "已点击当前页面控件").put("elementId", node.viewIdResourceName.orEmpty())
        }
    }

    private fun setText(context: Context, request: JSONObject): JSONObject {
        val service = requireAccessibility(context)
        val selector = JSONObject().put("editableOnly", true)
        val displayId = request.optInt("displayId", Display.DEFAULT_DISPLAY)
        val resourceId = request.optString("resourceId", "")
        if (resourceId.isNotEmpty()) selector.put("resourceId", resourceId)
        else selector.put("focusedOnly", true)
        val text = request.getString("text")
        fun apply(node: AccessibilityNodeInfo): JSONObject {
            val args = Bundle().apply { putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text) }
            check(node.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args)) { "当前控件拒绝设置文字" }
            return actionResult("setText", "已设置输入框文字").put("elementId", node.viewIdResourceName.orEmpty())
        }
        return try { withSelectedNode(service, selector, displayId, ::apply) }
        catch (error: IllegalStateException) {
            if (resourceId.isNotEmpty() || !error.message.orEmpty().startsWith("当前页面未找到")) throw error
            selector.remove("focusedOnly")
            withSelectedNode(service, selector, displayId, ::apply)
        }
    }

    private fun gesture(context: Context, request: JSONObject, action: String): JSONObject {
        val sx = request.optDouble("x", request.optDouble("startX", 0.0)).toFloat()
        val sy = request.optDouble("y", request.optDouble("startY", 0.0)).toFloat()
        val ex = if (action == "swipe") request.getDouble("endX").toFloat() else sx
        val ey = if (action == "swipe") request.getDouble("endY").toFloat() else sy
        val duration = request.optLong("duration", if (action == "longPress") 650L else if (action == "swipe") 300L else 80L)
        dispatchGesture(requireAccessibility(context), sx, sy, ex, ey, duration.coerceIn(1L, 60_000L), request.optInt("displayId", Display.DEFAULT_DISPLAY))
        return actionResult(action, "已完成屏幕操作").put("coordinates", JSONArray().put(sx.toDouble()).put(sy.toDouble()))
    }

    private fun dispatchGesture(service: AccessibilityService, sx: Float, sy: Float, ex: Float, ey: Float, duration: Long, displayId: Int) {
        val latch = CountDownLatch(1)
        var completed = false
        val path = Path().apply { moveTo(sx, sy); if (sx != ex || sy != ey) lineTo(ex, ey) }
        val builder = GestureDescription.Builder().addStroke(GestureDescription.StrokeDescription(path, 0, duration))
        if (displayId != Display.DEFAULT_DISPLAY) {
            check(Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) { "Virtual screen gestures require Android 11 or later" }
            builder.setDisplayId(displayId)
        }
        val gesture = builder.build()
        var failure: Exception? = null
        mainHandler.post {
            try {
                if (!service.dispatchGesture(gesture, object : AccessibilityService.GestureResultCallback() {
                    override fun onCompleted(gestureDescription: GestureDescription?) { completed = true; latch.countDown() }
                    override fun onCancelled(gestureDescription: GestureDescription?) { latch.countDown() }
                }, mainHandler)) latch.countDown()
            } catch (error: Exception) {
                failure = error
                latch.countDown()
            }
        }
        val returned = latch.await(duration + 5_000L, TimeUnit.MILLISECONDS)
        failure?.let { throw IllegalStateException(it.message, it) }
        check(returned && completed) { "Android 取消或拒绝了屏幕手势" }
    }

    private fun pressKey(context: Context, request: JSONObject): JSONObject {
        val key = request.get("keyCode").toString().uppercase().removePrefix("KEYCODE_")
        val global = when (key) {
            "BACK", "4" -> AccessibilityService.GLOBAL_ACTION_BACK
            "HOME", "3" -> AccessibilityService.GLOBAL_ACTION_HOME
            "APP_SWITCH", "RECENTS", "187" -> AccessibilityService.GLOBAL_ACTION_RECENTS
            "NOTIFICATIONS" -> AccessibilityService.GLOBAL_ACTION_NOTIFICATIONS
            "QUICK_SETTINGS" -> AccessibilityService.GLOBAL_ACTION_QUICK_SETTINGS
            "POWER_DIALOG" -> AccessibilityService.GLOBAL_ACTION_POWER_DIALOG
            else -> null
        }
        val displayId = request.optInt("displayId", Display.DEFAULT_DISPLAY)
        if (global != null && displayId == Display.DEFAULT_DISPLAY) {
            check(requireAccessibility(context).performGlobalAction(global)) { "Android 拒绝了系统按键操作" }
        } else {
            val code = key.toIntOrNull() ?: KeyEvent.keyCodeFromString("KEYCODE_$key")
            require(code != KeyEvent.KEYCODE_UNKNOWN) { "Unknown Android key: $key" }
            check(AndroidPrivilegeAuthorization.isRootAuthorized(context) || AndroidPrivilegeAuthorization.isShizukuAuthorized()) {
                "此按键需要已授权的 Root 或 Shizuku"
            }
            val result = shell(context, JSONObject().put("command", "/system/bin/input -d $displayId keyevent $code"))
            check(result.getInt("exitCode") == 0) { result.getString("output") }
        }
        return actionResult("pressKey", "已发送按键 $key")
    }

    private fun captureScreenshot(context: Context, displayId: Int): String {
        if (displayId != Display.DEFAULT_DISPLAY) return LegacyVirtualDisplays.capture(context, displayId)
        val service = LegacyAccessibilityService.current
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R && service != null) {
            val result = arrayOfNulls<String>(1)
            val error = arrayOfNulls<Throwable>(1)
            val latch = CountDownLatch(1)
            service.takeScreenshot(Display.DEFAULT_DISPLAY, screenshotExecutor, object : AccessibilityService.TakeScreenshotCallback {
                override fun onSuccess(screenshot: AccessibilityService.ScreenshotResult) {
                    try {
                        val wrapped = Bitmap.wrapHardwareBuffer(screenshot.hardwareBuffer, screenshot.colorSpace)
                            ?: throw IllegalStateException("Android screenshot buffer is empty")
                        val bitmap = requireNotNull(wrapped.copy(Bitmap.Config.ARGB_8888, false)) { "Android screenshot bitmap conversion failed" }
                        wrapped.recycle()
                        try {
                            val directory = File(context.cacheDir, "legacy-screenshots").apply { mkdirs() }
                            val file = File(directory, "${UUID.randomUUID()}.png")
                            file.outputStream().use { check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)) }
                            result[0] = file.absolutePath
                        } finally { bitmap.recycle() }
                    } catch (failure: Throwable) { error[0] = failure }
                    finally { screenshot.hardwareBuffer.close(); latch.countDown() }
                }
                override fun onFailure(errorCode: Int) {
                    error[0] = IllegalStateException("Android screenshot failed: $errorCode")
                    latch.countDown()
                }
            })
            check(latch.await(10, TimeUnit.SECONDS)) { "Android screenshot timed out" }
            error[0]?.let { throw IllegalStateException(it.message, it) }
            return requireNotNull(result[0])
        }
        val activity = MainActivity.currentActivity()
            ?: throw IllegalStateException("请打开 Operit2 后授权截屏，或启用无障碍截图服务")
        return JSONObject(activity.handleRuntimeHostRequest("systemCaptureScreenshot", "{}")).getString("path")
    }

    private fun actionResult(type: String, description: String): JSONObject =
        JSONObject().put("actionType", type).put("actionDescription", description)
}
