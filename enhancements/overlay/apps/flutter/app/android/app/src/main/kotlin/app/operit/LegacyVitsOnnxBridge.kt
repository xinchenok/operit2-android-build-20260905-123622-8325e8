// Adapted from AAswordman/Operit VitsVoiceProvider, commit 962f922f0e902084f7683116a9e0543c60c51ff1.
// Retains its lexicon/direct-symbol/token-ID frontends, tensor options and PCM conversion.
package app.operit

import ai.onnxruntime.OnnxJavaType
import ai.onnxruntime.OnnxTensor
import ai.onnxruntime.OrtEnvironment
import ai.onnxruntime.OrtSession
import ai.onnxruntime.TensorInfo
import android.content.Context
import java.io.BufferedInputStream
import java.io.File
import java.io.FileInputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.DoubleBuffer
import java.nio.FloatBuffer
import java.nio.IntBuffer
import java.nio.LongBuffer
import java.security.MessageDigest
import java.util.Locale
import java.util.zip.ZipInputStream
import kotlin.math.roundToInt
import org.json.JSONArray
import org.json.JSONObject

/** Actual legacy ONNX inference; audio playback/pause/stop belong to Operit2's existing service. */
object LegacyVitsOnnxBridge {
    private val extractionLock = Any()

    @JvmStatic
    fun inspect(context: Context, configJson: String): String = synchronized(extractionLock) {
        LegacyVitsEngine(context.applicationContext, JSONObject(configJson)).use { it.describe().toString() }
    }

    @JvmStatic
    fun synthesize(context: Context, requestJson: String): String {
        val request = JSONObject(requestJson)
        val engine = synchronized(extractionLock) {
            LegacyVitsEngine(context.applicationContext, request.getJSONObject("config"))
        }
        return engine.use {
            val output = File(request.getString("outputPath")).canonicalFile
            require(output.parentFile?.isDirectory == true && !output.exists()) { "VITS output path is not available" }
            val extra = request.optJSONObject("extraParams") ?: JSONObject()
            val parameters = extra.keys().asSequence().associateWith { key -> extra.getString(key) }
            val samples = it.generate(request.getString("text"), request.getDouble("speed").toFloat(), parameters)
            writeWave(output, samples, it.sampleRate)
            JSONObject().put("audioPath", output.absolutePath).put("outputFormat", "wav")
                .put("sampleRate", it.sampleRate).put("sampleCount", samples.size).toString()
        }
    }

    private fun writeWave(file: File, samples: ShortArray, sampleRate: Int) {
        require(samples.size <= (Int.MAX_VALUE - 44) / 2) { "VITS audio exceeds WAV size limit" }
        val bytes = samples.size * 2
        val header = ByteBuffer.allocate(44).order(ByteOrder.LITTLE_ENDIAN)
        header.put("RIFF".toByteArray(Charsets.US_ASCII)).putInt(36 + bytes)
        header.put("WAVEfmt ".toByteArray(Charsets.US_ASCII)).putInt(16).putShort(1).putShort(1)
        header.putInt(sampleRate).putInt(sampleRate * 2).putShort(2).putShort(16)
        header.put("data".toByteArray(Charsets.US_ASCII)).putInt(bytes)
        try {
            file.outputStream().use { out ->
                out.write(header.array())
                val buffer = ByteBuffer.allocate(8192).order(ByteOrder.LITTLE_ENDIAN)
                for (sample in samples) {
                    if (buffer.remaining() < 2) { out.write(buffer.array(), 0, buffer.position()); buffer.clear() }
                    buffer.putShort(sample)
                }
                out.write(buffer.array(), 0, buffer.position())
            }
        } catch (error: Throwable) { file.delete(); throw error }
    }
}

private class VitsException(message: String, cause: Throwable? = null) : IllegalArgumentException(message, cause)
private fun message(template: String, vararg args: Any): String = String.format(Locale.ROOT, template, *args)

private class LegacyVitsEngine(private val context: Context, json: JSONObject) : AutoCloseable {
    private data class PackageConfig(val packagePath: String, val speakerId: String, val options: Map<String, String>)
    private val config = PackageConfig(json.getString("packagePath"), json.optString("speakerId"),
        (json.optJSONObject("options") ?: JSONObject()).let { options ->
            options.keys().asSequence().associateWith { key -> options.getString(key) }
        })
    private val env = OrtEnvironment.getEnvironment()
    private val session: OrtSession
    private val runtimeConfig: RuntimeConfig
    private val bindings: InputBindings
    private val packageFiles: PackageFiles
    private val currentSpeakerId = config.speakerId.trim()
    val sampleRate: Int get() = runtimeConfig.sampleRate

    init {
        packageFiles = resolvePackageFiles(resolvePackageRoot(config.packagePath))
        runtimeConfig = parseRuntimeConfig(packageFiles)
        require(runtimeConfig.sampleRate in 8000..384000) { "Invalid VITS sample rate" }
        val threads = optionalInt("threads") ?: 1
        require(threads in 1..64) { "VITS threads must be between 1 and 64" }
        session = OrtSession.SessionOptions().use { options ->
            options.setIntraOpNumThreads(threads)
            options.setInterOpNumThreads(1)
            options.setOptimizationLevel(OrtSession.SessionOptions.OptLevel.ALL_OPT)
            env.createSession(packageFiles.modelFile.absolutePath, options)
        }
        try {
            bindings = resolveInputBindings(session)
            val known = setOfNotNull(bindings.idsInputName, bindings.lengthInputName, bindings.scalesInputName, bindings.sidInputName)
            require(known == session.inputNames.toSet()) { "VITS has additional unbound model inputs: ${session.inputNames - known}" }
            require(session.outputNames.isNotEmpty()) { "VITS model has no audio output" }
            if (bindings.sidInputName != null) {
                val speaker = currentSpeakerId.toLongOrNull() ?: throw VitsException("VITS speaker ID is required for this model")
                require(speaker >= 0 && (runtimeConfig.speakerCount == null || speaker < runtimeConfig.speakerCount!!)) { "VITS speaker ID is out of range" }
            }
            if (bindings.scalesInputName != null) {
                val values = listOf(runtimeConfig.noiseScale, runtimeConfig.lengthScale, runtimeConfig.noiseW)
                require(values.all { it != null && it.isFinite() && it >= 0 }) { "VITS scales require valid noise_scale, length_scale and noise_w" }
                require(runtimeConfig.lengthScale!! > 0) { "VITS length_scale must be positive" }
            }
            if (optionalString("text_mode").lowercase(Locale.ROOT) !in setOf("token_ids", "phoneme_ids")) {
                require(runtimeConfig.frontend.lowercase(Locale.ROOT) in setOf("lexicon", "direct_symbols")) { "Unsupported legacy VITS frontend: ${runtimeConfig.frontend}" }
                if (runtimeConfig.frontend.equals("lexicon", true)) require(runtimeConfig.lexicon != null) { "Raw-text VITS requires a lexicon" }
            }
            if (runtimeConfig.addBlank) require(runtimeConfig.blankId != null) { "VITS add_blank requires a blank token ID" }
        } catch (error: Throwable) { session.close(); throw error }
    }

    fun describe(): JSONObject = JSONObject().put("packageRoot", packageFiles.root.absolutePath)
        .put("model", packageFiles.modelFile.absolutePath).put("sampleRate", sampleRate)
        .put("frontend", runtimeConfig.frontend).put("supportsSpeed", bindings.scalesInputName != null)
        .put("speakerCount", runtimeConfig.speakerCount ?: JSONObject.NULL)
        .put("inputNames", JSONArray(session.inputNames.toList())).put("outputNames", JSONArray(session.outputNames.toList()))

    fun generate(text: String, speed: Float, extraParams: Map<String, String>): ShortArray {
        require(text.isNotBlank() && speed.isFinite() && speed > 0) { "VITS text and speed are invalid" }
        if (bindings.scalesInputName == null) require(speed == 1f) { "This VITS model has no scales input and cannot change speech speed" }
        val ids = tokenize(text, runtimeConfig, extraParams)
        require(ids.isNotEmpty()) { "VITS tokenizer produced no token IDs" }
        return runModel(session, runtimeConfig, bindings, ids, speed).also { require(it.isNotEmpty()) { "VITS generated no audio samples" } }
    }
    override fun close() { session.close() }
    private companion object { const val PACKAGE_MANIFEST = "operit-vits-tts.json" }

    private data class RuntimeConfig(
        val sampleRate: Int,
        val tokenMap: Map<String, List<Long>>,
        val lexicon: PackageLexicon?,
        val frontend: String,
        val addBlank: Boolean,
        val blankId: Long?,
        val bosIds: List<Long>,
        val eosIds: List<Long>,
        val noiseScale: Float?,
        val lengthScale: Float?,
        val noiseW: Float?,
        val speakerCount: Int?
    )

    private data class PackageFiles(
        val root: File,
        val modelFile: File,
        val configFile: File,
        val lexiconFile: File?
    )

    private data class InputBindings(
        val idsInputName: String,
        val lengthInputName: String?,
        val scalesInputName: String?,
        val sidInputName: String?
    )

    private class PackageLexicon(
        private val entries: Map<String, LongArray>,
        private val tokenMap: Map<String, List<Long>>
    ) {
        private val maxEntryChars = entries.keys.maxOfOrNull { it.codePointCount(0, it.length) } ?: 1

        fun tokenize(text: String): LongArray {
            val ids = ArrayList<Long>()
            var index = 0
            while (index < text.length) {
                val codePoint = text.codePointAt(index)
                val charCount = Character.charCount(codePoint)
                val symbol = String(Character.toChars(codePoint))

                if (Character.isWhitespace(codePoint)) {
                    tokenMap[" "]?.let { ids.addAll(it) }
                    index += charCount
                    continue
                }

                if (isAsciiWordChar(codePoint)) {
                    val start = index
                    index += charCount
                    while (index < text.length) {
                        val next = text.codePointAt(index)
                        if (!isAsciiWordChar(next)) break
                        index += Character.charCount(next)
                    }
                    val word = text.substring(start, index).lowercase(Locale.ROOT)
                    val wordIds = entries[word]
                        ?: throw IllegalArgumentException("unknown word: $word")
                    ids.addAll(wordIds.toList())
                    continue
                }

                val lexiconMatch = longestEntryAt(text, index)
                if (lexiconMatch != null) {
                    ids.addAll(lexiconMatch.second.toList())
                    index += lexiconMatch.first.length
                    continue
                }

                val mapped = tokenMap[symbol]
                    ?: throw IllegalArgumentException("unknown symbol: $symbol")
                ids.addAll(mapped)
                index += charCount
            }
            return ids.toLongArray()
        }

        private fun longestEntryAt(text: String, start: Int): Pair<String, LongArray>? {
            var end = start
            var count = 0
            val pieces = ArrayList<String>()
            while (end < text.length && count < maxEntryChars) {
                val cp = text.codePointAt(end)
                val part = String(Character.toChars(cp))
                pieces.add(part)
                end += Character.charCount(cp)
                count++
            }

            for (size in pieces.size downTo 1) {
                val candidate = pieces.take(size).joinToString("").lowercase(Locale.ROOT)
                val ids = entries[candidate]
                if (ids != null) return candidate to ids
            }
            return null
        }

        private fun isAsciiWordChar(codePoint: Int): Boolean {
            return codePoint in 'a'.code..'z'.code ||
                codePoint in 'A'.code..'Z'.code ||
                codePoint in '0'.code..'9'.code ||
                codePoint == '\''.code ||
                codePoint == '-'.code
        }
    }

    private fun runModel(
        activeSession: OrtSession,
        activeConfig: RuntimeConfig,
        bindings: InputBindings,
        ids: LongArray,
        effectiveRate: Float
    ): ShortArray {
        val toClose = ArrayList<AutoCloseable>()
        val inputs = LinkedHashMap<String, OnnxTensor>()

        try {
            val idsInfo = tensorInfo(activeSession, bindings.idsInputName)
            val idsShape = shapeForValues(idsInfo, ids.size, bindings.idsInputName)
            val idsTensor = createIntegerTensor(bindings.idsInputName, ids, idsShape, idsInfo)
            toClose.add(idsTensor)
            inputs[bindings.idsInputName] = idsTensor

            bindings.lengthInputName?.let { name ->
                val lengthInfo = tensorInfo(activeSession, name)
                val lengthShape = shapeForValues(lengthInfo, 1, name)
                val lengthTensor = createIntegerTensor(
                    name = name,
                    values = longArrayOf(ids.size.toLong()),
                    shape = lengthShape,
                    info = lengthInfo
                )
                toClose.add(lengthTensor)
                inputs[name] = lengthTensor
            }

            bindings.scalesInputName?.let { name ->
                val noiseScale = activeConfig.noiseScale
                    ?: throw VitsException(message(VitsErrors.vits_tts_error_scales_not_set, "noise_scale"))
                val lengthScale = activeConfig.lengthScale
                    ?: throw VitsException(message(VitsErrors.vits_tts_error_scales_not_set, "length_scale"))
                val noiseW = activeConfig.noiseW
                    ?: throw VitsException(message(VitsErrors.vits_tts_error_scales_not_set, "noise_w"))
                val scales = floatArrayOf(noiseScale, lengthScale / effectiveRate.coerceAtLeast(0.01f), noiseW)
                val scalesInfo = tensorInfo(activeSession, name)
                val scalesShape = shapeForValues(scalesInfo, scales.size, name)
                val scalesTensor = createFloatTensor(name, scales, scalesShape, scalesInfo)
                toClose.add(scalesTensor)
                inputs[name] = scalesTensor
            }

            bindings.sidInputName?.let { name ->
                val speaker = currentSpeakerId.toLongOrNull()
                    ?: throw VitsException(message(VitsErrors.vits_tts_error_speaker_required))
                val sidInfo = tensorInfo(activeSession, name)
                val sidShape = shapeForValues(sidInfo, 1, name)
                val sidTensor = createIntegerTensor(name, longArrayOf(speaker), sidShape, sidInfo)
                toClose.add(sidTensor)
                inputs[name] = sidTensor
            }

            activeSession.run(inputs).use { result ->
                val firstOutput = result.get(0).value
                return extractPcm16(firstOutput)
            }
        } finally {
            for (i in toClose.indices.reversed()) {
                try {
                    toClose[i].close()
                } catch (_: Exception) {
                }
            }
        }
    }

    private fun resolvePackageRoot(raw: String): File {
        val packagePath = normalizeLocalPath(raw)
        if (packagePath.isBlank()) {
            throw VitsException(message(VitsErrors.vits_tts_error_package_path_not_set))
        }

        val source = File(packagePath)
        if (!source.exists()) {
            throw VitsException(message(VitsErrors.vits_tts_error_package_file_not_found, packagePath))
        }

        if (source.isDirectory) return source
        if (!source.isFile || !source.extension.equals("zip", ignoreCase = true)) {
            throw VitsException(message(VitsErrors.vits_tts_error_package_path_invalid, packagePath))
        }

        return extractZipPackage(source)
    }

    private fun extractZipPackage(zipFile: File): File {
        val signature = "${zipFile.absolutePath}|${zipFile.length()}|${zipFile.lastModified()}"
        val packageDir = File(context.filesDir, "vits_tts_packages")
        val target = File(packageDir, sha256(signature).take(16))
        val marker = File(target, ".source")
        if (target.isDirectory && marker.isFile && marker.readText(Charsets.UTF_8) == signature) {
            return target
        }

        if (target.exists()) {
            target.deleteRecursively()
        }
        if (!target.mkdirs() && !target.isDirectory) {
            throw VitsException(message(VitsErrors.vits_tts_error_package_extract_failed))
        }

        val canonicalTarget = target.canonicalFile
        try {
            ZipInputStream(BufferedInputStream(FileInputStream(zipFile))).use { zip ->
                var entry = zip.nextEntry
                val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
                while (entry != null) {
                    val entryName = entry.name.replace('\\', '/')
                    val outFile = File(target, entryName).canonicalFile
                    if (!outFile.path.startsWith(canonicalTarget.path + File.separator)) {
                        throw VitsException(message(VitsErrors.vits_tts_error_package_zip_entry_unsafe, entry.name))
                    }

                    if (entry.isDirectory) {
                        outFile.mkdirs()
                    } else {
                        outFile.parentFile?.mkdirs()
                        outFile.outputStream().use { out ->
                            while (true) {
                                val read = zip.read(buffer)
                                if (read < 0) break
                                out.write(buffer, 0, read)
                            }
                        }
                    }
                    zip.closeEntry()
                    entry = zip.nextEntry
                }
            }
        } catch (e: VitsException) {
            throw e
        } catch (e: Exception) {
            throw VitsException(message(VitsErrors.vits_tts_error_package_zip_read_failed), cause = e)
        }

        marker.writeText(signature, Charsets.UTF_8)
        return target
    }

    private fun resolvePackageFiles(root: File): PackageFiles {
        val files = root.walkTopDown().filter { it.isFile }.toList()
        val manifest = files.firstOrNull { it.name == PACKAGE_MANIFEST }?.let {
            JSONObject(it.readText(Charsets.UTF_8))
        }

        val modelFile = manifestPath(root, manifest, "model")
            ?: optionPath(root, "model_path")
            ?: singleCandidate(
                files.filter { it.extension.equals("onnx", ignoreCase = true) },
                VitsErrors.vits_tts_error_package_model_not_found,
                VitsErrors.vits_tts_error_package_model_ambiguous
            )

        val configFile = manifestPath(root, manifest, "config")
            ?: optionPath(root, "config_path")
            ?: File(modelFile.absolutePath + ".json").takeIf { it.isFile }
            ?: singleCandidate(
                files.filter { it.extension.equals("json", ignoreCase = true) && isTtsConfigJson(it) },
                VitsErrors.vits_tts_error_package_config_not_found,
                VitsErrors.vits_tts_error_package_config_ambiguous
            )

        val lexiconFile = manifestPath(root, manifest, "lexicon")
            ?: optionPath(root, "lexicon_path")
            ?: optionalSingleCandidate(
                files.filter { it.name.equals("lexicon.txt", ignoreCase = true) },
                VitsErrors.vits_tts_error_package_lexicon_ambiguous
            )

        return PackageFiles(root, modelFile, configFile, lexiconFile)
    }

    private fun parseRuntimeConfig(packageFiles: PackageFiles): RuntimeConfig {
        val root = try {
            JSONObject(packageFiles.configFile.readText(Charsets.UTF_8))
        } catch (e: Exception) {
            throw VitsException(message(VitsErrors.vits_tts_error_config_parse_failed), cause = e)
        }

        val sampleRate = optionalInt("sample_rate")
            ?: root.optJSONObject("audio")?.optionalInt("sample_rate")
            ?: root.optionalInt("sample_rate")
            ?: throw VitsException(message(VitsErrors.vits_tts_error_sample_rate_not_set))

        if (sampleRate <= 0) {
            throw VitsException(message(VitsErrors.vits_tts_error_sample_rate_not_set))
        }

        val tokenSource = firstTokenMapObject(root)
        val tokenSourceObject = tokenSource?.second
        val tokenMap = tokenSourceObject?.let { parseTokenMap(it) }
            ?: firstTokenArray(root)?.let { parseTokenArray(it) }
            ?: emptyMap()

        if (tokenMap.isEmpty()) {
            throw VitsException(message(VitsErrors.vits_tts_error_tokenize_failed, "token map is empty"))
        }

        val inference = root.optJSONObject("inference")
        val blankId = optionalLong("blank_token_id")
            ?: tokenSourceObject?.let { readFirstTokenId(it, "_") }
            ?: tokenSourceObject?.let { readFirstTokenId(it, "") }
            ?: tokenSourceObject?.let { readFirstTokenId(it, "<blank>") }
        val addBlank = optionalBool("add_blank")
            ?: optionalBool("interleave_blank")
            ?: (tokenSource?.first == "phoneme_id_map" && blankId != null)
        val frontend = optionalString("frontend").ifBlank { "lexicon" }
        val lexicon = packageFiles.lexiconFile?.let { loadLexicon(it, tokenMap) }

        return RuntimeConfig(
            sampleRate = sampleRate,
            tokenMap = tokenMap,
            lexicon = lexicon,
            frontend = frontend,
            addBlank = addBlank,
            blankId = blankId,
            bosIds = optionalIds("bos_token_ids")
                ?: optionalLong("bos_token_id")?.let { listOf(it) }
                ?: tokenSourceObject?.let { readTokenIds(it, "^") }.orEmpty(),
            eosIds = optionalIds("eos_token_ids")
                ?: optionalLong("eos_token_id")?.let { listOf(it) }
                ?: tokenSourceObject?.let { readTokenIds(it, "\$") }.orEmpty(),
            noiseScale = optionalFloat("noise_scale") ?: inference?.optionalFloat("noise_scale"),
            lengthScale = optionalFloat("length_scale") ?: inference?.optionalFloat("length_scale"),
            noiseW = optionalFloat("noise_w") ?: inference?.optionalFloat("noise_w"),
            speakerCount = optionalInt("speaker_count") ?: root.optionalInt("num_speakers")
        )
    }

    private fun tokenize(
        text: String,
        activeConfig: RuntimeConfig,
        extraParams: Map<String, String>
    ): LongArray {
        val directIds = listOf("token_ids", "phoneme_ids", "ids")
            .firstNotNullOfOrNull { key -> extraParams[key]?.takeIf { it.isNotBlank() } }
        if (directIds != null) {
            return parseIds(directIds, "extra token ids").toLongArray()
        }

        val textMode = optionalString("text_mode")
        if (textMode.equals("token_ids", ignoreCase = true) || textMode.equals("phoneme_ids", ignoreCase = true)) {
            return parseIds(text, "text token ids").toLongArray()
        }

        val bodyIds = when (activeConfig.frontend.lowercase(Locale.ROOT)) {
            "lexicon" -> {
                val lexicon = activeConfig.lexicon
                    ?: throw VitsException(message(VitsErrors.vits_tts_error_raw_text_requires_lexicon))
                try {
                    lexicon.tokenize(text)
                } catch (e: IllegalArgumentException) {
                    throw VitsException(message(VitsErrors.vits_tts_error_tokenize_failed, e.message.orEmpty()), cause = e)
                }
            }
            "direct_symbols" -> tokenizeSymbolsByMap(text, activeConfig.tokenMap)
            else -> throw VitsException(message(VitsErrors.vits_tts_error_frontend_unsupported, activeConfig.frontend))
        }

        if (bodyIds.isEmpty()) return bodyIds

        val ids = ArrayList<Long>()
        ids.addAll(activeConfig.bosIds)
        ids.addAll(bodyIds.toList())
        ids.addAll(activeConfig.eosIds)

        if (activeConfig.addBlank && ids.isNotEmpty()) {
            val blank = activeConfig.blankId
                ?: throw VitsException(message(VitsErrors.vits_tts_error_blank_token_not_set))
            val withBlank = ArrayList<Long>(ids.size * 2 - 1)
            ids.forEachIndexed { index, id ->
                if (index > 0) {
                    withBlank.add(blank)
                }
                withBlank.add(id)
            }
            return withBlank.toLongArray()
        }

        return ids.toLongArray()
    }

    private fun loadLexicon(file: File, tokenMap: Map<String, List<Long>>): PackageLexicon {
        val entries = LinkedHashMap<String, LongArray>()
        try {
            file.readLines(Charsets.UTF_8).forEachIndexed { index, line ->
                val trimmed = line.trim()
                if (trimmed.isEmpty()) return@forEachIndexed
                val parts = trimmed.split(Regex("\\s+"))
                if (parts.size < 2) {
                    throw VitsException(message(VitsErrors.vits_tts_error_lexicon_parse_failed, index + 1))
                }
                val word = parts.first().lowercase(Locale.ROOT)
                if (entries.containsKey(word)) {
                    throw VitsException(message(VitsErrors.vits_tts_error_lexicon_duplicate_word, word))
                }
                val ids = ArrayList<Long>()
                parts.drop(1).forEach { token ->
                    val tokenIds = tokenMap[token]
                        ?: throw VitsException(message(VitsErrors.vits_tts_error_lexicon_unknown_token, token))
                    ids.addAll(tokenIds)
                }
                entries[word] = ids.toLongArray()
            }
        } catch (e: VitsException) {
            throw e
        } catch (e: Exception) {
            throw VitsException(message(VitsErrors.vits_tts_error_lexicon_parse_failed, 0), cause = e)
        }
        return PackageLexicon(entries, tokenMap)
    }

    private fun resolveInputBindings(activeSession: OrtSession): InputBindings {
        val inputNames = activeSession.inputNames.toSet()
        val idsInput = resolveInputName(
            inputNames = inputNames,
            explicitOptionKeys = listOf("ids_input", "input_ids_name"),
            candidates = listOf("input", "input_ids", "ids", "text", "x"),
            requiredLabel = "input ids"
        )
        val lengthInput = resolveOptionalInputName(
            inputNames = inputNames,
            explicitOptionKeys = listOf("length_input", "input_lengths_name"),
            candidates = listOf("input_lengths", "text_lengths", "lengths", "x_lengths")
        )
        val scalesInput = resolveOptionalInputName(
            inputNames = inputNames,
            explicitOptionKeys = listOf("scales_input", "scales_name"),
            candidates = listOf("scales")
        )
        val sidInput = resolveOptionalInputName(
            inputNames = inputNames,
            explicitOptionKeys = listOf("sid_input", "speaker_input"),
            candidates = listOf("sid", "speaker_id", "speaker")
        )

        return InputBindings(
            idsInputName = idsInput,
            lengthInputName = lengthInput,
            scalesInputName = scalesInput,
            sidInputName = sidInput
        )
    }

    private fun resolveInputName(
        inputNames: Set<String>,
        explicitOptionKeys: List<String>,
        candidates: List<String>,
        requiredLabel: String
    ): String {
        explicitOptionKeys.firstNotNullOfOrNull { key ->
            config.options[key]?.trim()?.takeIf { it.isNotBlank() }
        }?.let { explicit ->
            if (explicit !in inputNames) {
                throw VitsException(message(VitsErrors.vits_tts_error_input_not_found, explicit))
            }
            return explicit
        }

        return candidates.firstOrNull { it in inputNames }
            ?: throw VitsException(
                message(
                    VitsErrors.vits_tts_error_input_name_not_resolved,
                    requiredLabel,
                    inputNames.joinToString()
                )
            )
    }

    private fun resolveOptionalInputName(
        inputNames: Set<String>,
        explicitOptionKeys: List<String>,
        candidates: List<String>
    ): String? {
        explicitOptionKeys.firstNotNullOfOrNull { key ->
            config.options[key]?.trim()?.takeIf { it.isNotBlank() }
        }?.let { explicit ->
            if (explicit !in inputNames) {
                throw VitsException(message(VitsErrors.vits_tts_error_input_not_found, explicit))
            }
            return explicit
        }
        return candidates.firstOrNull { it in inputNames }
    }

    private fun tensorInfo(activeSession: OrtSession, name: String): TensorInfo {
        return activeSession.inputInfo[name]?.info as? TensorInfo
            ?: throw VitsException(message(VitsErrors.vits_tts_error_tensor_info_missing, name))
    }

    private fun createIntegerTensor(
        name: String,
        values: LongArray,
        shape: LongArray,
        info: TensorInfo
    ): OnnxTensor {
        return when (info.type) {
            OnnxJavaType.INT64 -> OnnxTensor.createTensor(env, LongBuffer.wrap(values), shape)
            OnnxJavaType.INT32 -> {
                val intValues = IntArray(values.size) { index ->
                    val value = values[index]
                    if (value < Int.MIN_VALUE || value > Int.MAX_VALUE) {
                        throw VitsException(message(VitsErrors.vits_tts_error_integer_out_of_range, name))
                    }
                    value.toInt()
                }
                OnnxTensor.createTensor(env, IntBuffer.wrap(intValues), shape)
            }
            else -> throw VitsException(
                message(VitsErrors.vits_tts_error_unsupported_input_type, name, info.type.name)
            )
        }
    }

    private fun createFloatTensor(
        name: String,
        values: FloatArray,
        shape: LongArray,
        info: TensorInfo
    ): OnnxTensor {
        return when (info.type) {
            OnnxJavaType.FLOAT -> OnnxTensor.createTensor(env, FloatBuffer.wrap(values), shape)
            OnnxJavaType.DOUBLE -> {
                val doubleValues = DoubleArray(values.size) { values[it].toDouble() }
                OnnxTensor.createTensor(env, DoubleBuffer.wrap(doubleValues), shape)
            }
            else -> throw VitsException(
                message(VitsErrors.vits_tts_error_unsupported_input_type, name, info.type.name)
            )
        }
    }

    private fun shapeForValues(info: TensorInfo, valueCount: Int, inputName: String): LongArray {
        val shape = info.shape ?: return longArrayOf(valueCount.toLong())
        if (shape.isEmpty()) {
            if (valueCount != 1) {
                throw VitsException(message(VitsErrors.vits_tts_error_shape_unsupported, inputName))
            }
            return longArrayOf()
        }

        val normalized = shape.copyOf()
        val unknownIndices = ArrayList<Int>()
        var knownProduct = 1L
        normalized.forEachIndexed { index, dim ->
            if (dim <= 0) {
                unknownIndices.add(index)
            } else {
                knownProduct *= dim
            }
        }

        if (unknownIndices.isNotEmpty()) {
            if (knownProduct <= 0 || valueCount.toLong() % knownProduct != 0L) {
                throw VitsException(message(VitsErrors.vits_tts_error_shape_unsupported, inputName))
            }
            unknownIndices.dropLast(1).forEach { index ->
                normalized[index] = 1L
            }
            normalized[unknownIndices.last()] = valueCount.toLong() / knownProduct
        }

        val product = normalized.fold(1L) { acc, dim -> acc * dim }
        if (product != valueCount.toLong()) {
            throw VitsException(message(VitsErrors.vits_tts_error_shape_unsupported, inputName))
        }
        return normalized
    }

    private fun extractPcm16(value: Any?): ShortArray {
        flattenFloats(value).takeIf { it.isNotEmpty() }?.let { return floatsToPcm16(it) }
        flattenDoubles(value).takeIf { it.isNotEmpty() }?.let { return doublesToPcm16(it) }
        flattenShorts(value).takeIf { it.isNotEmpty() }?.let { return it }
        flattenInts(value).takeIf { it.isNotEmpty() }?.let { ints ->
            return ShortArray(ints.size) { index ->
                ints[index].coerceIn(Short.MIN_VALUE.toInt(), Short.MAX_VALUE.toInt()).toShort()
            }
        }
        flattenBytes(value).takeIf { it.isNotEmpty() }?.let { bytes ->
            if (bytes.size % 2 != 0) {
                throw VitsException(message(VitsErrors.vits_tts_error_output_unsupported))
            }
            val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
            return ShortArray(bytes.size / 2) { buffer.short }
        }
        throw VitsException(message(VitsErrors.vits_tts_error_output_unsupported))
    }

    private fun floatsToPcm16(samples: FloatArray): ShortArray {
        return ShortArray(samples.size) { index ->
            val sample = samples[index]
            val safeSample = if (sample.isFinite()) sample.coerceIn(-1f, 1f) else 0f
            (safeSample * Short.MAX_VALUE).roundToInt().toShort()
        }
    }

    private fun doublesToPcm16(samples: DoubleArray): ShortArray {
        return ShortArray(samples.size) { index ->
            val sample = samples[index]
            val safeSample = if (sample.isFinite()) sample.coerceIn(-1.0, 1.0) else 0.0
            (safeSample * Short.MAX_VALUE).roundToInt().toShort()
        }
    }

    private fun flattenFloats(value: Any?): FloatArray {
        val result = ArrayList<Float>()
        fun walk(v: Any?) {
            when (v) {
                is FloatArray -> v.forEach { result.add(it) }
                is Array<*> -> v.forEach { walk(it) }
            }
        }
        walk(value)
        return result.toFloatArray()
    }

    private fun flattenDoubles(value: Any?): DoubleArray {
        val result = ArrayList<Double>()
        fun walk(v: Any?) {
            when (v) {
                is DoubleArray -> v.forEach { result.add(it) }
                is Array<*> -> v.forEach { walk(it) }
            }
        }
        walk(value)
        return result.toDoubleArray()
    }

    private fun flattenShorts(value: Any?): ShortArray {
        val result = ArrayList<Short>()
        fun walk(v: Any?) {
            when (v) {
                is ShortArray -> v.forEach { result.add(it) }
                is Array<*> -> v.forEach { walk(it) }
            }
        }
        walk(value)
        return result.toShortArray()
    }

    private fun flattenInts(value: Any?): IntArray {
        val result = ArrayList<Int>()
        fun walk(v: Any?) {
            when (v) {
                is IntArray -> v.forEach { result.add(it) }
                is LongArray -> v.forEach { result.add(it.coerceIn(Int.MIN_VALUE.toLong(), Int.MAX_VALUE.toLong()).toInt()) }
                is Array<*> -> v.forEach { walk(it) }
            }
        }
        walk(value)
        return result.toIntArray()
    }

    private fun flattenBytes(value: Any?): ByteArray {
        val result = ArrayList<Byte>()
        fun walk(v: Any?) {
            when (v) {
                is ByteArray -> v.forEach { result.add(it) }
                is Array<*> -> v.forEach { walk(it) }
            }
        }
        walk(value)
        return result.toByteArray()
    }

    private fun firstTokenMapObject(root: JSONObject): Pair<String, JSONObject>? {
        listOf("phoneme_id_map", "token_id_map", "tokens", "vocab").forEach { key ->
            root.optJSONObject(key)?.let { return key to it }
        }
        root.optJSONObject("model")?.optJSONObject("vocab")?.let { return "model.vocab" to it }
        return null
    }

    private fun firstTokenArray(root: JSONObject): JSONArray? {
        root.optJSONArray("symbols")?.let { return it }
        root.optJSONObject("model")?.optJSONArray("symbols")?.let { return it }
        return null
    }

    private fun parseTokenMap(obj: JSONObject): Map<String, List<Long>> {
        val result = LinkedHashMap<String, List<Long>>()
        val keys = obj.keys()
        while (keys.hasNext()) {
            val key = keys.next()
            if (key == "_" || key.isEmpty() || key == "<blank>" || key == "^" || key == "\$") {
                continue
            }
            val ids = parseJsonIds(obj.opt(key), key)
            if (ids.isNotEmpty()) {
                result[key] = ids
            }
        }
        return result
    }

    private fun parseTokenArray(arr: JSONArray): Map<String, List<Long>> {
        val result = LinkedHashMap<String, List<Long>>()
        for (i in 0 until arr.length()) {
            val symbol = arr.optString(i, "")
            if (symbol.isNotEmpty() && symbol != "_" && symbol != "<blank>" && symbol != "^" && symbol != "\$") {
                result[symbol] = listOf(i.toLong())
            }
        }
        return result
    }

    private fun readFirstTokenId(obj: JSONObject, key: String): Long? {
        return readTokenIds(obj, key).firstOrNull()
    }

    private fun readTokenIds(obj: JSONObject, key: String): List<Long> {
        if (!obj.has(key)) return emptyList()
        return parseJsonIds(obj.opt(key), key)
    }

    private fun parseJsonIds(value: Any?, label: String): List<Long> {
        return when (value) {
            is Number -> listOf(value.toLong())
            is String -> listOf(parseLong(value, label))
            is JSONArray -> buildList {
                for (i in 0 until value.length()) {
                    add(parseJsonId(value.opt(i), "$label[$i]"))
                }
            }
            else -> emptyList()
        }
    }

    private fun parseJsonId(value: Any?, label: String): Long {
        return when (value) {
            is Number -> value.toLong()
            is String -> parseLong(value, label)
            else -> throw VitsException(message(VitsErrors.vits_tts_error_config_invalid_id, label))
        }
    }

    private fun parseIds(raw: String, label: String): List<Long> {
        return raw.split(',', ';', ' ', '\n', '\t')
            .map { it.trim() }
            .filter { it.isNotBlank() }
            .map { parseLong(it, label) }
    }

    private fun parseLong(raw: String, label: String): Long {
        return raw.trim().toLongOrNull()
            ?: throw VitsException(message(VitsErrors.vits_tts_error_config_invalid_id, label))
    }

    private fun optionalIds(key: String): List<Long>? {
        val raw = config.options[key]?.trim()?.takeIf { it.isNotBlank() } ?: return null
        return parseIds(raw, key)
    }

    private fun optionalString(key: String): String {
        return config.options[key]?.trim().orEmpty()
    }

    private fun optionalLong(key: String): Long? {
        val raw = config.options[key]?.trim()?.takeIf { it.isNotBlank() } ?: return null
        return raw.toLongOrNull()
            ?: throw VitsException(message(VitsErrors.vits_tts_error_config_invalid_id, key))
    }

    private fun optionalInt(key: String): Int? {
        val raw = config.options[key]?.trim()?.takeIf { it.isNotBlank() } ?: return null
        return raw.toIntOrNull()
            ?: throw VitsException(message(VitsErrors.vits_tts_error_config_invalid_int, key))
    }

    private fun optionalFloat(key: String): Float? {
        val raw = config.options[key]?.trim()?.takeIf { it.isNotBlank() } ?: return null
        return raw.toFloatOrNull()
            ?: throw VitsException(message(VitsErrors.vits_tts_error_config_invalid_float, key))
    }

    private fun optionalBool(key: String): Boolean? {
        val raw = config.options[key]?.trim()?.takeIf { it.isNotBlank() } ?: return null
        return when (raw.lowercase(Locale.ROOT)) {
            "true", "1", "yes", "y" -> true
            "false", "0", "no", "n" -> false
            else -> throw VitsException(message(VitsErrors.vits_tts_error_config_invalid_bool, key))
        }
    }

    private fun JSONObject.optionalInt(key: String): Int? {
        if (!has(key)) return null
        val value = opt(key)
        return when (value) {
            is Number -> value.toInt()
            is String -> value.trim().toIntOrNull()
            else -> null
        }
    }

    private fun JSONObject.optionalFloat(key: String): Float? {
        if (!has(key)) return null
        val value = opt(key)
        return when (value) {
            is Number -> value.toFloat()
            is String -> value.trim().toFloatOrNull()
            else -> null
        }
    }

    private fun manifestPath(root: File, manifest: JSONObject?, key: String): File? {
        val raw = manifest?.optString(key, "")?.trim()?.takeIf { it.isNotBlank() } ?: return null
        return resolveRelativeFile(root, raw)
    }

    private fun optionPath(root: File, key: String): File? {
        val raw = optionalString(key).takeIf { it.isNotBlank() } ?: return null
        return resolveRelativeFile(root, raw)
    }

    private fun resolveRelativeFile(root: File, raw: String): File {
        val file = File(raw)
        if (file.isAbsolute) {
            throw VitsException(message(VitsErrors.vits_tts_error_package_path_unsafe, raw))
        }

        val canonicalRoot = root.canonicalFile
        val resolved = File(root, raw).canonicalFile
        if (!resolved.path.startsWith(canonicalRoot.path + File.separator)) {
            throw VitsException(message(VitsErrors.vits_tts_error_package_path_unsafe, raw))
        }
        if (!resolved.isFile) {
            throw VitsException(message(VitsErrors.vits_tts_error_package_file_not_found, raw))
        }
        return resolved
    }

    private fun singleCandidate(files: List<File>, missingRes: String, ambiguousRes: String): File {
        if (files.isEmpty()) throw VitsException(message(missingRes))
        if (files.size > 1) throw VitsException(message(ambiguousRes, files.joinToString { it.name }))
        return files.single()
    }

    private fun optionalSingleCandidate(files: List<File>, ambiguousRes: String): File? {
        if (files.isEmpty()) return null
        if (files.size > 1) throw VitsException(message(ambiguousRes, files.joinToString { it.name }))
        return files.single()
    }

    private fun isTtsConfigJson(file: File): Boolean {
        val text = file.readText(Charsets.UTF_8)
        return text.contains("\"phoneme_id_map\"") ||
            text.contains("\"token_id_map\"") ||
            text.contains("\"symbols\"")
    }

    private fun normalizeLocalPath(raw: String): String {
        return raw.trim().removePrefix("file://")
    }

    private fun tokenizeSymbolsByMap(text: String, tokenMap: Map<String, List<Long>>): LongArray {
        val sortedKeys = tokenMap.keys
            .filter { it.isNotEmpty() }
            .sortedByDescending { it.length }
        val result = ArrayList<Long>()
        var index = 0
        while (index < text.length) {
            val matched = sortedKeys.firstOrNull { key ->
                text.regionMatches(index, key, 0, key.length, ignoreCase = false)
            } ?: throw VitsException(
                message(
                    VitsErrors.vits_tts_error_unknown_token,
                    printableSymbol(String(Character.toChars(text.codePointAt(index))))
                )
            )
            result.addAll(tokenMap.getValue(matched))
            index += matched.length
        }
        return result.toLongArray()
    }

    private fun sha256(raw: String): String {
        val digest = MessageDigest.getInstance("SHA-256").digest(raw.toByteArray(Charsets.UTF_8))
        return digest.joinToString("") { "%02x".format(it) }
    }

    private fun printableSymbol(symbol: String): String {
        return when (symbol) {
            "\n" -> "\\n"
            "\r" -> "\\r"
            "\t" -> "\\t"
            " " -> "space"
            else -> symbol
        }
    }


}

private object VitsErrors {
    const val vits_tts_error_blank_token_not_set = "VITS/Piper TTS 配置要求插入 blank token，但没有设置 blank_token_id。"
    const val vits_tts_error_config_invalid_bool = "VITS/Piper TTS 参数不是布尔值：%1\$s"
    const val vits_tts_error_config_invalid_float = "VITS/Piper TTS 参数不是数字：%1\$s"
    const val vits_tts_error_config_invalid_id = "VITS/Piper TTS 配置中的 id 无效：%1\$s"
    const val vits_tts_error_config_invalid_int = "VITS/Piper TTS 参数不是整数：%1\$s"
    const val vits_tts_error_config_parse_failed = "VITS/Piper TTS 配置文件解析失败"
    const val vits_tts_error_frontend_unsupported = "VITS/Piper TTS 不支持当前 frontend：%1\$s"
    const val vits_tts_error_input_name_not_resolved = "VITS/Piper TTS 无法识别 %1\$s 输入名，模型输入包括：%2\$s"
    const val vits_tts_error_input_not_found = "VITS/Piper TTS 模型中找不到输入：%1\$s"
    const val vits_tts_error_integer_out_of_range = "VITS/Piper TTS 输入 %1\$s 的 token id 超出 INT32 范围。"
    const val vits_tts_error_lexicon_duplicate_word = "VITS/Piper TTS lexicon.txt 中存在重复词条：%1\$s"
    const val vits_tts_error_lexicon_parse_failed = "VITS/Piper TTS lexicon.txt 解析失败，行号：%1\$d"
    const val vits_tts_error_lexicon_unknown_token = "VITS/Piper TTS lexicon.txt 中的 token 不在配置映射里：%1\$s"
    const val vits_tts_error_output_unsupported = "VITS/Piper TTS 输出格式不受支持，仅支持 float/PCM 张量。"
    const val vits_tts_error_package_config_ambiguous = "VITS/Piper TTS 模型包中找到多个配置 JSON，请在 manifest 或 options 中指定 config_path：%1\$s"
    const val vits_tts_error_package_config_not_found = "VITS/Piper TTS 模型包中没有找到可识别的配置 JSON。"
    const val vits_tts_error_package_extract_failed = "VITS/Piper TTS 模型包解压失败"
    const val vits_tts_error_package_file_not_found = "VITS/Piper TTS 模型包文件不存在：%1\$s"
    const val vits_tts_error_package_lexicon_ambiguous = "VITS/Piper TTS 模型包中找到多个 lexicon.txt，请在 manifest 或 options 中指定 lexicon_path：%1\$s"
    const val vits_tts_error_package_model_ambiguous = "VITS/Piper TTS 模型包中找到多个 .onnx 模型文件，请在 manifest 或 options 中指定 model_path：%1\$s"
    const val vits_tts_error_package_model_not_found = "VITS/Piper TTS 模型包中没有找到 .onnx 模型文件。"
    const val vits_tts_error_package_path_invalid = "VITS/Piper TTS 模型包路径无效，仅支持 .zip 文件或目录：%1\$s"
    const val vits_tts_error_package_path_not_set = "VITS/Piper TTS 模型包路径未设置，请填写本地 .zip 模型包或已解压目录路径。"
    const val vits_tts_error_package_path_unsafe = "VITS/Piper TTS 模型包路径必须指向包内文件：%1\$s"
    const val vits_tts_error_package_zip_entry_unsafe = "VITS/Piper TTS 模型包包含不安全路径：%1\$s"
    const val vits_tts_error_package_zip_read_failed = "VITS/Piper TTS 模型包读取失败"
    const val vits_tts_error_raw_text_requires_lexicon = "VITS/Piper TTS 当前配置需要 lexicon.txt 才能把原始文本转换为 token。请把 lexicon.txt 放入模型包，或把 frontend 设为 direct_symbols，或把 text_mode 设为 token_ids/phoneme_ids。"
    const val vits_tts_error_sample_rate_not_set = "VITS/Piper TTS sample_rate 未设置，请在配置 JSON 或参数中填写。"
    const val vits_tts_error_scales_not_set = "VITS/Piper TTS 模型需要 scales 输入，请在参数中填写 %1\$s。"
    const val vits_tts_error_shape_unsupported = "VITS/Piper TTS 输入 %1\$s 的 shape 与当前数据长度不匹配。"
    const val vits_tts_error_speaker_required = "VITS/Piper TTS 模型需要 speaker id，请填写数字 Speaker ID。"
    const val vits_tts_error_tensor_info_missing = "VITS/Piper TTS 输入缺少 TensorInfo：%1\$s"
    const val vits_tts_error_tokenize_failed = "VITS/Piper TTS 文本转 token 失败：%1\$s"
    const val vits_tts_error_unknown_token = "VITS/Piper TTS 配置中找不到字符或 phoneme 的 id：%1\$s"
    const val vits_tts_error_unsupported_input_type = "VITS/Piper TTS 输入 %1\$s 的类型不受支持：%2\$s"
}
