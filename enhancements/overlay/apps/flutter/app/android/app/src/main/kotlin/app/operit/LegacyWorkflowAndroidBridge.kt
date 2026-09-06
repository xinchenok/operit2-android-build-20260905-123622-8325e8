package app.operit

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Build
import android.os.Bundle
import android.system.Os
import java.io.File
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject

/** Restores explicit workflow intent listeners with the Android runtime. */
object LegacyWorkflowAndroidBridge {
    private const val PREFS = "legacy_workflow_android"
    private const val ACTIONS = "registered_actions"
    private var receiver: BroadcastReceiver? = null

    private fun actionsKey(): String = "$ACTIONS@${Os.getenv("OPERIT_ANDROID_RUNTIME_ROOT").orEmpty()}"

    @JvmStatic
    fun start(context: Context) {
        val application = context.applicationContext
        AndroidCoreRuntime.get(application).runBackground {
            try {
                val root = Os.getenv("OPERIT_ANDROID_RUNTIME_ROOT") ?: return@runBackground
                val state = File(root, "data/legacy_workflows/state.json")
                val actions = linkedSetOf<String>()
                if (state.isFile) {
                    val workflows = JSONObject(state.readText()).getJSONArray("workflows")
                    for (index in 0 until workflows.length()) {
                        val workflow = workflows.getJSONObject(index)
                        if (!workflow.optBoolean("enabled", true)) continue
                        val nodes = workflow.optJSONArray("nodes") ?: JSONArray(workflow.optString("nodes", "[]"))
                        for (nodeIndex in 0 until nodes.length()) {
                            val node = nodes.getJSONObject(nodeIndex)
                            if (node.optString("type") != "trigger" || node.optString("triggerType") != "intent") continue
                            val config = node.optJSONObject("triggerConfig") ?: continue
                            if (config.optBoolean("enabled", true)) {
                                config.optString("action").takeIf { it.isNotBlank() }?.let { actions.add(it) }
                            }
                        }
                    }
                }
                refresh(application, JSONObject().put("actions", JSONArray(actions.toList())).toString())
            } catch (error: Exception) {
                Log.e("LegacyWorkflowAndroidBridge", "Could not restore workflow actions from current runtime", error)
                try {
                    val saved = application.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                        .getString(actionsKey(), "[]") ?: "[]"
                    synchronized(this) { register(application, JSONArray(saved)) }
                } catch (restoreError: Exception) {
                    Log.e("LegacyWorkflowAndroidBridge", "Could not restore saved workflow actions", restoreError)
                }
            }
        }
    }

    @JvmStatic
    @Synchronized
    fun refresh(context: Context, actionsJson: String): String {
        val actions = JSONObject(actionsJson).getJSONArray("actions")
        register(context.applicationContext, actions)
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().putString(actionsKey(), actions.toString()).apply()
        return JSONObject().put("success", true).put("actions", actions).toString()
    }

    private fun register(context: Context, actions: JSONArray) {
        receiver?.let {
            try { context.unregisterReceiver(it) } catch (_: IllegalArgumentException) { }
        }
        receiver = null
        val filter = IntentFilter()
        for (index in 0 until actions.length()) {
            val action = actions.getString(index).trim()
            if (action.isNotEmpty() && action != LegacyWorkflowReceiver.EVENT_ACTION &&
                action != LegacyWorkflowReceiver.TASKER_ACTION && action != LegacyWorkflowReceiver.OLD_INTENT_ACTION) {
                filter.addAction(action)
            }
        }
        if (filter.countActions() == 0) return
        val listener = LegacyWorkflowReceiver()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.registerReceiver(listener, filter, Context.RECEIVER_EXPORTED)
        } else {
            @Suppress("DEPRECATION")
            context.registerReceiver(listener, filter)
        }
        receiver = listener
    }
}

/** Receives external, user-configured Tasker and Android workflow intents. */
class LegacyWorkflowReceiver : BroadcastReceiver() {
    companion object {
        const val EVENT_ACTION = "app.operit.LEGACY_WORKFLOW_EVENT"
        const val TASKER_ACTION = "app.operit.LEGACY_TASKER_EVENT"
        const val OLD_INTENT_ACTION = "com.ai.assistance.operit.TRIGGER_WORKFLOW"
    }

    override fun onReceive(context: Context, intent: Intent) {
        val action = intent.action ?: return
        val pending = goAsync()
        val host = AndroidCoreRuntime.get(context.applicationContext)
        host.runBackground {
            try {
                if (!host.restoreStorageRoots()) return@runBackground
                val runtimeHandle = host.ensureRuntimeHandle()
                val extras = bundleJson(intent.extras)
                val requestedTopic = intent.getStringExtra("topic").orEmpty()
                val topic = when {
                    requestedTopic == "legacy.workflow.intent" || requestedTopic == "legacy.workflow.tasker" -> requestedTopic
                    action == TASKER_ACTION -> "legacy.workflow.tasker"
                    else -> "legacy.workflow.intent"
                }
                val eventAction = if (action == EVENT_ACTION) intent.getStringExtra("action") ?: action else action
                val data = JSONObject().put("action", eventAction).put("extras", extras)
                    .put("text", intent.getStringExtra("text") ?: intent.getStringExtra("command") ?: "")
                    .put("uri", intent.dataString.orEmpty())
                extras.opt("params")?.let { data.put("params", it) }
                val event = JSONObject().put("topic", topic).put("data", data)
                    .put("domain", "host").put("source", "android.broadcast").put("platform", "android")
                    .put("occurredAtMillis", System.currentTimeMillis())
                // Keep goAsync active until Core accepts this exact event.
                RuntimeEvents.emit(runtimeHandle, event)
            } catch (error: Exception) {
                Log.e("LegacyWorkflowReceiver", "Workflow broadcast could not be delivered", error)
            } finally {
                pending.finish()
            }
        }
    }

    private fun bundleJson(bundle: Bundle?): JSONObject {
        val result = JSONObject()
        if (bundle == null) return result
        for (key in bundle.keySet()) {
            @Suppress("DEPRECATION")
            val value = bundle.get(key)
            result.put(key, jsonValue(value))
        }
        return result
    }

    private fun jsonValue(value: Any?): Any = when (value) {
        null -> JSONObject.NULL
        is String, is Boolean, is Number -> value
        is Bundle -> bundleJson(value)
        is Array<*> -> JSONArray(value.map { jsonValue(it) })
        is List<*> -> JSONArray(value.map { jsonValue(it) })
        is IntArray -> JSONArray(value.toList())
        is LongArray -> JSONArray(value.toList())
        is BooleanArray -> JSONArray(value.toList())
        else -> value.toString()
    }
}
