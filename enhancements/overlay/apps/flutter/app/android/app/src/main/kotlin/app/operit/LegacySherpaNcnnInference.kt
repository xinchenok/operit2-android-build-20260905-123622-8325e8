package app.operit

import android.os.Looper
import com.k2fsa.sherpa.ncnn.DecoderConfig
import com.k2fsa.sherpa.ncnn.FeatureExtractorConfig
import com.k2fsa.sherpa.ncnn.ModelConfig
import com.k2fsa.sherpa.ncnn.RecognizerConfig
import com.k2fsa.sherpa.ncnn.SherpaNcnn
import java.io.File
import java.io.RandomAccessFile
import org.json.JSONObject

/** Executes the same bilingual NCNN transducer model used by Operit 1. */
internal object LegacySherpaNcnnInference {
    private var loadedDirectory: String? = null

    // Serialize model lifetimes so concurrent microphone requests cannot each
    // allocate another full transducer. This is called by the native Core worker.
    @Synchronized
    fun transcribe(requestJson: String): String {
        check(Looper.myLooper() != Looper.getMainLooper()) { "NCNN recognition must run on a worker" }
        val request = JSONObject(requestJson)
        require(JSONObject(request.getString("optionsJson")).length() == 0) {
            "Sherpa NCNN does not accept undeclared inference options"
        }
        val tag = JSONObject(request.getString("driverJson"))
        require(tag.length() == 1 && tag.has("SherpaNcnnStreamingTransducer")) {
            "Sherpa NCNN requires its native transducer driver"
        }
        val driver = tag.getJSONObject("SherpaNcnnStreamingTransducer")
        val modelRoot = File(request.getString("modelDirectory")).canonicalFile
        require(modelRoot.isDirectory) { "Sherpa NCNN model directory is missing" }
        fun modelFile(key: String): String {
            val relative = driver.getString(key)
            require(relative.isNotBlank()) { "Sherpa NCNN model path is empty: $key" }
            val file = File(modelRoot, relative).canonicalFile
            require(file.toPath().startsWith(modelRoot.toPath()) && file.isFile && file.length() > 0) {
                "Sherpa NCNN model file is missing or outside its directory: $key"
            }
            return file.absolutePath
        }
        val model = ModelConfig(
            encoderParam = modelFile("encoderParam"), encoderBin = modelFile("encoderBin"),
            decoderParam = modelFile("decoderParam"), decoderBin = modelFile("decoderBin"),
            joinerParam = modelFile("joinerParam"), joinerBin = modelFile("joinerBin"),
            tokens = modelFile("tokens"),
            numThreads = Runtime.getRuntime().availableProcessors().coerceIn(1, 4), useGPU = false,
        )
        val audio = File(request.getString("audioPath")).canonicalFile
        require(audio.isFile) { "NCNN input WAV file is missing" }
        // Validate the input before creating native model objects.
        RandomAccessFile(audio, "r").use { input ->
            val pcm = inspectWave(input)
            loadEngine(File(request.getString("engineLibraryDirectory")))
            val recognizer = SherpaNcnn(RecognizerConfig(
                featConfig = FeatureExtractorConfig(sampleRate = 16000.0f, featureDim = 80),
                modelConfig = model,
                decoderConfig = DecoderConfig(method = "greedy_search", numActivePaths = 4),
                enableEndpoint = false,
            ))
            try {
                input.seek(pcm.first)
                var remaining = pcm.second
                val buffer = ByteArray(16000) // 500 ms of mono PCM16, bounded for long recordings.
                while (remaining > 0) {
                    val count = minOf(remaining, buffer.size.toLong()).toInt()
                    input.readFully(buffer, 0, count)
                    val samples = FloatArray(count / 2) { i ->
                        val bits = (buffer[i * 2].toInt() and 255) or (buffer[i * 2 + 1].toInt() shl 8)
                        bits.toShort() / 32768.0f
                    }
                    recognizer.acceptSamples(samples)
                    while (recognizer.isReady()) recognizer.decode()
                    remaining -= count
                }
                // Supply the encoder right context before draining the final tokens.
                recognizer.acceptSamples(FloatArray(4800))
                recognizer.inputFinished()
                while (recognizer.isReady()) recognizer.decode()
                return JSONObject()
                    .put("text", recognizer.text.trim())
                    .put("resultJson", JSONObject().put("engine", "sherpa-ncnn")
                        .put("version", "2.1.15").put("sampleRate", 16000).toString())
                    .toString()
            } finally {
                recognizer.release()
            }
        }
    }

    private fun loadEngine(rawDirectory: File) {
        val directory = rawDirectory.canonicalFile
        require(directory.isDirectory) { "Sherpa NCNN engine directory is missing" }
        val path = directory.absolutePath
        val previous = loadedDirectory
        if (previous != null) {
            require(previous == path) { "Restart the application before switching the loaded NCNN engine" }
            return
        }
        for (name in listOf("libncnn.so", "libsherpa-ncnn-jni.so")) {
            val library = File(directory, name)
            require(library.isFile && library.length() > 0) { "Sherpa NCNN engine library is missing: $name" }
            System.load(library.absolutePath)
        }
        loadedDirectory = path
    }

    /** Reads RIFF chunk metadata without reading an entire recording into memory. */
    private fun inspectWave(input: RandomAccessFile): Pair<Long, Long> {
        require(input.length() >= 44) { "NCNN WAV is truncated" }
        require(fourcc(input) == "RIFF") { "NCNN audio must be RIFF WAV" }
        val riffSize = u32(input)
        require(riffSize + 8 <= input.length()) { "NCNN WAV RIFF length exceeds the file" }
        require(fourcc(input) == "WAVE") { "NCNN audio must be WAVE" }
        var formatFound = false
        var data: Pair<Long, Long>? = null
        val end = riffSize + 8
        while (input.filePointer + 8 <= end) {
            val kind = fourcc(input)
            val size = u32(input)
            val start = input.filePointer
            require(size <= end - start) { "NCNN WAV chunk is truncated" }
            when (kind) {
                "fmt " -> {
                    require(!formatFound && size >= 16) { "NCNN WAV format chunk is invalid" }
                    val format = u16(input)
                    val channels = u16(input)
                    val sampleRate = u32(input)
                    val byteRate = u32(input)
                    val blockAlign = u16(input)
                    val bits = u16(input)
                    require(format == 1 && channels == 1 && sampleRate == 16000L && bits == 16 &&
                        blockAlign == 2 && byteRate == 32000L) {
                        "Sherpa NCNN requires 16 kHz mono PCM16 WAV, matching the application recorder"
                    }
                    formatFound = true
                }
                "data" -> {
                    require(data == null && size > 0 && size % 2 == 0L) { "NCNN WAV data chunk is invalid" }
                    data = start to size
                }
            }
            input.seek(start + size + size % 2)
        }
        require(formatFound && data != null) { "NCNN WAV is missing PCM format or audio data" }
        return data
    }
    private fun fourcc(input: RandomAccessFile): String = ByteArray(4).also { input.readFully(it) }.toString(Charsets.US_ASCII)
    private fun u16(input: RandomAccessFile): Int = input.readUnsignedByte() or (input.readUnsignedByte() shl 8)
    private fun u32(input: RandomAccessFile): Long = u16(input).toLong() or (u16(input).toLong() shl 16)
}
