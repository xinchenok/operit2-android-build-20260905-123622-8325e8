package app.operit

import android.content.Context
import dalvik.system.DexClassLoader
import java.io.File
import java.lang.reflect.Array as ReflectArray
import java.lang.reflect.InvocationTargetException
import java.lang.reflect.Modifier
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong
import kotlin.coroutines.Continuation
import kotlin.coroutines.intrinsics.COROUTINE_SUSPENDED
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject

/** Calls real Android/JVM objects for the imported Operit packages. */
class LegacyJavaHost(private val context: Context) {
    private val objects = ConcurrentHashMap<String, Any>()
    private val sequence = AtomicLong()
    private val currentScope = ThreadLocal<String>()
    private class CodeScope {
        val loaders = mutableListOf<ClassLoader>()
        val artifacts = mutableListOf<JSONObject>()
    }
    private val codeScopes = ConcurrentHashMap<String, CodeScope>()
    private val artifactLock = Any()
    private fun codeScope(): CodeScope = codeScopes.getOrPut(currentScope.get()) { CodeScope() }

    fun call(requestJson: String): String = try {
        val request = JSONObject(requestJson)
        currentScope.set(request.getString("scope"))
        val operation = request.getString("operation")
        val result = when (operation) {
            "releaseScope" -> {
                val prefix = "${currentScope.get()}:"
                objects.keys.removeAll { it.startsWith(prefix) }
                codeScopes.remove(currentScope.get())
                JSONObject.NULL
            }
            "context" -> expose(context)
            "classExists" -> try { resolve(request.getString("className")); true } catch (_: ClassNotFoundException) { false }
            "new" -> {
                val cls = resolve(request.getString("className"))
                val args = arguments(request)
                val candidates = cls.constructors.mapNotNull { ctor ->
                    convertArguments(args, ctor.parameterTypes, ctor.isVarArgs)?.let { Pair(ctor, it) }
                }
                val match = candidates.minByOrNull { score(args, it.first.parameterTypes) }
                    ?: error("No matching constructor: ${cls.name}(${args.size} arguments)")
                expose(match.first.newInstance(*match.second))
            }
            "callStatic", "call", "callSuspendStatic", "callSuspend" -> {
                val isStatic = operation.endsWith("Static")
                val target = if (isStatic) null else instance(request.getString("handle"))
                val cls = if (isStatic) resolve(request.getString("className")) else target!!.javaClass
                val name = request.getString("member")
                val args = arguments(request)
                val suspendCall = operation.startsWith("callSuspend")
                val candidates = cls.methods.filter { method ->
                    method.name == name && (target != null || Modifier.isStatic(method.modifiers) || hasObjectInstance(cls)) &&
                        (!suspendCall || method.parameterTypes.lastOrNull() == Continuation::class.java)
                }.mapNotNull { method ->
                    val parameterTypes = if (suspendCall) method.parameterTypes.dropLast(1).toTypedArray() else method.parameterTypes
                    convertArguments(args, parameterTypes, method.isVarArgs)?.let { Pair(method, it) }
                }
                val match = candidates.minByOrNull { score(args, it.first.parameterTypes) }
                    ?: error("No matching method: ${cls.name}.$name(${args.size} arguments)")
                val receiver = target ?: if (Modifier.isStatic(match.first.modifiers)) null else cls.getField("INSTANCE").get(null)
                // Android framework methods may be declared by a hidden implementation class.
                val method = publicMethod(cls, match.first)
                val value = if (suspendCall) runBlocking {
                    suspendCoroutine<Any?> { continuation ->
                        try {
                            val value = method.invoke(receiver, *match.second, continuation)
                            if (value !== COROUTINE_SUSPENDED) continuation.resume(value)
                        } catch (error: InvocationTargetException) {
                            continuation.resumeWithException(error.targetException)
                        } catch (error: Throwable) {
                            continuation.resumeWithException(error)
                        }
                    }
                } else method.invoke(receiver, *match.second)
                encode(value)
            }
            "getStatic", "get", "setStatic", "set" -> {
                val isStatic = operation.endsWith("Static")
                val target = if (isStatic) null else instance(request.getString("handle"))
                val cls = if (isStatic) resolve(request.getString("className")) else target!!.javaClass
                val field = cls.getField(request.getString("member"))
                if (operation.startsWith("set")) {
                    field.set(target, convert(decode(request.opt("value")), field.type))
                    JSONObject.NULL
                } else encode(field.get(target))
            }
            "member" -> {
                val value = instance(request.getString("handle"))
                val name = request.getString("member")
                val field = value.javaClass.fields.firstOrNull { it.name == name }
                if (field != null) JSONObject().put("kind", "value").put("value", encode(field.get(value)))
                else {
                    require(value.javaClass.methods.any { it.name == name }) { "No member ${value.javaClass.name}.$name" }
                    JSONObject().put("kind", "method")
                }
            }
            "load" -> load(request)
            "artifacts" -> codeScope().let { scope -> synchronized(scope) { JSONArray(scope.artifacts) } }
            "release" -> { objects.remove(request.getString("handle")); JSONObject.NULL }
            else -> error("Unknown Java operation: $operation")
        }
        JSONObject().put("success", true).put("data", result).toString()
    } catch (error: Throwable) {
        val cause = if (error is InvocationTargetException) error.targetException else error
        JSONObject().put("success", false).put("message", "${cause.javaClass.simpleName}: ${cause.message}").toString()
    }

    private fun instance(handle: String): Any = objects[handle] ?: error("Java object was released: $handle")
    private fun hasObjectInstance(cls: Class<*>): Boolean = cls.fields.any { it.name == "INSTANCE" && Modifier.isStatic(it.modifiers) }
    private fun expose(value: Any): JSONObject {
        val handle = "${currentScope.get()}:${sequence.incrementAndGet()}"
        objects[handle] = value
        return JSONObject().put("__javaHandle", handle).put("__javaClass", value.javaClass.name)
    }
    private fun encode(value: Any?): Any = when (value) {
        null, Unit -> JSONObject.NULL
        is String, is Number, is Boolean -> value
        is Char -> value.toString()
        else -> if (value.javaClass.isArray) JSONArray().apply {
            for (index in 0 until ReflectArray.getLength(value)) put(encode(ReflectArray.get(value, index)))
        } else expose(value)
    }
    private fun decode(value: Any?): Any? = when (value) {
        null, JSONObject.NULL -> null
        is JSONObject -> if (value.has("__javaHandle")) instance(value.getString("__javaHandle")) else value.keys().asSequence().associateWith { decode(value.opt(it)) }
        is JSONArray -> (0 until value.length()).map { decode(value.opt(it)) }
        else -> value
    }
    private fun arguments(request: JSONObject): List<Any?> {
        val args = request.optJSONArray("args") ?: JSONArray()
        return (0 until args.length()).map { decode(args.opt(it)) }
    }
    private fun convert(value: Any?, type: Class<*>): Any? {
        if (value == null) { require(!type.isPrimitive); return null }
        if (type.isInstance(value)) return value
        if (type.isArray && value is List<*>) return ReflectArray.newInstance(type.componentType, value.size).apply {
            value.forEachIndexed { index, item -> ReflectArray.set(this, index, convert(item, type.componentType)) }
        }
        if (value is Number) return when (type) {
            java.lang.Byte.TYPE, java.lang.Byte::class.java -> value.toByte()
            java.lang.Short.TYPE, java.lang.Short::class.java -> value.toShort()
            java.lang.Integer.TYPE, java.lang.Integer::class.java -> value.toInt()
            java.lang.Long.TYPE, java.lang.Long::class.java -> value.toLong()
            java.lang.Float.TYPE, java.lang.Float::class.java -> value.toFloat()
            java.lang.Double.TYPE, java.lang.Double::class.java -> value.toDouble()
            else -> error("Number is incompatible with ${type.name}")
        }
        if (value is Boolean && type == java.lang.Boolean.TYPE) return value
        if (value is String && type == java.lang.Character.TYPE && value.length == 1) return value[0]
        if (value is String && type == CharSequence::class.java) return value
        if (type == Any::class.java) return value
        error("${value.javaClass.name} is incompatible with ${type.name}")
    }
    private fun convertArguments(args: List<Any?>, types: Array<Class<*>>, varargs: Boolean): Array<Any?>? = try {
        if (!varargs) {
            require(args.size == types.size)
            Array(types.size) { convert(args[it], types[it]) }
        } else {
            require(args.size >= types.size - 1)
            Array(types.size) { index ->
                if (index < types.size - 1) convert(args[index], types[index])
                else if (args.size == types.size && args[index] is List<*>) convert(args[index], types[index])
                else convert(args.drop(index), types[index])
            }
        }
    } catch (_: IllegalArgumentException) { null } catch (_: IllegalStateException) { null }
    private fun score(args: List<Any?>, types: Array<Class<*>>): Int = args.zip(types.toList()).sumOf { (value, type) ->
        if (value != null && type.isInstance(value)) 0 else if (value == null) 1 else if (value is Number && (type == java.lang.Integer.TYPE || type == java.lang.Long.TYPE)) 2 else 3
    }
    private fun publicMethod(cls: Class<*>, method: java.lang.reflect.Method): java.lang.reflect.Method {
        if (Modifier.isPublic(method.declaringClass.modifiers)) return method
        val queue = java.util.ArrayDeque<Class<*>>()
        cls.superclass?.let(queue::add)
        cls.interfaces.forEach(queue::add)
        while (queue.isNotEmpty()) {
            val candidate = queue.removeFirst()
            if (Modifier.isPublic(candidate.modifiers)) {
                try { return candidate.getMethod(method.name, *method.parameterTypes) } catch (_: NoSuchMethodException) { }
            }
            candidate.superclass?.let(queue::add)
            candidate.interfaces.forEach(queue::add)
        }
        return method
    }
    private fun resolve(name: String): Class<*> = codeScope().let { scope -> synchronized(scope) {
        for (loader in scope.loaders.asReversed()) {
            try { return@synchronized Class.forName(name, true, loader) } catch (_: ClassNotFoundException) { }
        }
        Class.forName(name, true, context.classLoader)
    } }
    private fun load(request: JSONObject): JSONObject = codeScope().let { scope -> synchronized(scope) {
        val loaders = scope.loaders
        val artifacts = scope.artifacts
        val input = File(request.getString("path")).canonicalFile
        require(input.isFile) { "Java artifact does not exist: $input" }
        val old = artifacts.firstOrNull { it.getString("path") == input.path }
        if (old != null) return@synchronized JSONObject(old.toString()).put("alreadyLoaded", true)
        val options = request.optJSONObject("options") ?: JSONObject()
        val prefixArray = options.optJSONArray("childFirstPrefixes") ?: JSONArray()
        val prefixes = (0 until prefixArray.length()).map { prefixArray.getString(it) }
        val optimized = File(context.codeCacheDir, "legacy-java").apply { mkdirs() }
        // Keep ToolPkg's resource writable for the next materialization; load a private immutable copy.
        val digest = java.security.MessageDigest.getInstance("SHA-256")
        input.inputStream().use { stream ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val count = stream.read(buffer)
                if (count < 0) break
                digest.update(buffer, 0, count)
            }
        }
        val hash = digest.digest().joinToString("") { "%02x".format(it.toInt() and 255) }
        val code = File(optimized, "$hash-${input.name}")
        synchronized(artifactLock) {
            // Separate JS scopes can load the same large runtime concurrently. Publish only
            // a complete immutable copy; a second loader must never see a half-written DEX.
            if (!code.isFile || code.length() != input.length()) {
                val staged = File.createTempFile("legacy-dex-", ".tmp", optimized)
                try {
                    java.io.FileOutputStream(staged).use { output ->
                        require(staged.setReadOnly()) { "Cannot protect Java artifact: $staged" }
                        input.inputStream().use { it.copyTo(output) }
                        output.fd.sync()
                    }
                    require(staged.renameTo(code)) { "Cannot publish Java artifact: $code" }
                } finally { staged.delete() }
            }
            require(code.setReadOnly() || !code.canWrite()) { "Cannot mark Java artifact read-only: $code" }
        }
        val parent = loaders.lastOrNull() ?: context.classLoader
        val nativePath = options.optString("nativeLibraryDir").takeIf { it.isNotBlank() }
        val loader = object : DexClassLoader(code.path, optimized.path, nativePath, parent) {
            override fun loadClass(name: String, resolve: Boolean): Class<*> {
                synchronized(this) {
                    findLoadedClass(name)?.let { return it }
                    if (prefixes.any(name::startsWith)) {
                        try { return findClass(name).also { if (resolve) resolveClass(it) } } catch (_: ClassNotFoundException) { }
                    }
                    return super.loadClass(name, resolve)
                }
            }
        }
        // Force actual DEX discovery; a plain JVM-only JAR is not executable on Android.
        if (input.extension != "dex") java.util.zip.ZipFile(input).use { zip ->
            require(zip.entries().asSequence().any { it.name.matches(Regex("classes[0-9]*\\.dex")) }) {
                "Java JAR contains no Android DEX code: $input"
            }
        }
        loaders.add(loader)
        JSONObject().put("index", artifacts.size).put("type", request.getString("type")).put("path", input.path)
            .put("childFirstPrefixes", prefixArray).put("alreadyLoaded", false).also { artifacts.add(it) }
    } }
}
