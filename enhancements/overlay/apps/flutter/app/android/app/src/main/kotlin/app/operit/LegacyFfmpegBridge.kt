package app.operit

import com.arthenica.ffmpegkit.FFmpegKit
import com.arthenica.ffmpegkit.FFmpegKitConfig
import com.arthenica.ffmpegkit.FFmpegSession
import com.arthenica.ffmpegkit.FFprobeKit
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** Real FFmpeg/FFprobe operations exposed to the enhanced legacy Java bridge. */
object LegacyFfmpegBridge {
    /** Executes argument text through FFmpegKit, preserving native exit status and logs. */
    @JvmStatic
    fun execute(command: String): String {
        require(command.isNotBlank()) { "FFmpeg command cannot be empty" }
        val startedAt = System.nanoTime()
        val arguments = arrayOf("-nostdin", *FFmpegKitConfig.parseArguments(command))
        val session = FFmpegKit.executeWithArguments(arguments)
        return result(session, command, startedAt).toString()
    }

    /** Returns the installed engine version and its actual compiled codec list. */
    @JvmStatic
    fun info(): String {
        val startedAt = System.nanoTime()
        val session = FFmpegKit.executeWithArguments(arrayOf("-nostdin", "-codecs"))
        val result = result(session, "-codecs", startedAt)
        result.put(
            "output",
            "FFmpegKit version: ${FFmpegKitConfig.getVersion()}\n" +
                "Build date: ${FFmpegKitConfig.getBuildDate()}\n\n" +
                result.getString("output"),
        )
        return result.toString()
    }

    /** Converts a media file using legacy options, then reads its real output metadata. */
    @JvmStatic
    fun convert(inputPath: String, outputPath: String, optionsJson: String): String {
        require(inputPath.isNotBlank() && outputPath.isNotBlank()) {
            "Input and output paths cannot be empty"
        }
        val inputFile = File(inputPath)
        val outputFile = File(outputPath)
        require(inputFile.isFile) { "Input file does not exist: $inputPath" }
        require(inputFile.canonicalFile != outputFile.canonicalFile) {
            "Input and output must be different files"
        }
        val options = if (optionsJson.isBlank()) JSONObject() else JSONObject(optionsJson)
        // Noninteractive conversions keep existing output files, matching legacy
        // FFmpeg's default rather than waiting forever for an overwrite answer.
        val arguments = mutableListOf("-nostdin", "-n", "-i", inputPath)
        option(options, "video_codec")?.let {
            arguments.addAll(listOf("-c:v", videoEncoder(it)))
        }
        option(options, "audio_codec")?.let {
            arguments.addAll(listOf("-c:a", audioEncoder(it)))
        }
        option(options, "resolution")?.let { arguments.addAll(listOf("-s", it)) }
        option(options, "bitrate")?.let { arguments.addAll(listOf("-b:v", it)) }
        option(options, "format")?.let { arguments.addAll(listOf("-f", it)) }
        arguments.add(outputPath)

        val startedAt = System.nanoTime()
        val session = FFmpegKit.executeWithArguments(arguments.toTypedArray())
        val result = result(session, FFmpegKitConfig.argumentsToString(arguments.toTypedArray()), startedAt)
        result.put("outputFile", outputPath)
        if (result.getInt("returnCode") == 0) {
            val mediaSession = FFprobeKit.getMediaInformation(outputPath)
            val media = mediaSession.mediaInformation?.allProperties
            result.put("probeReturnCode", mediaSession.returnCode?.value ?: JSONObject.NULL)
            if (media != null) {
                val streams = media.optJSONArray("streams") ?: JSONArray()
                val videoStreams = JSONArray()
                val audioStreams = JSONArray()
                for (index in 0 until streams.length()) {
                    val stream = streams.getJSONObject(index)
                    val type = stream.optString("codec_type")
                    if (type != "video" && type != "audio") {
                        continue
                    }
                    val converted = JSONObject()
                        .put("index", stream.getInt("index"))
                        .put("type", type)
                        .put("codec", stream.optString("codec_name", "unknown"))
                        .put("codecType", type)
                        .put("codecName", stream.optString("codec_name", "unknown"))
                    if (type == "video") {
                        if (stream.has("width") && stream.has("height")) {
                            converted.put("resolution", "${stream.getInt("width")}x${stream.getInt("height")}")
                        }
                        stream.optString("r_frame_rate").takeIf { it.isNotBlank() }?.let {
                            converted.put("frameRate", it)
                        }
                        videoStreams.put(converted)
                    } else {
                        stream.optString("sample_rate").takeIf { it.isNotBlank() }?.let {
                            converted.put("sampleRate", it)
                        }
                        if (stream.has("channels")) {
                            converted.put("channels", stream.getInt("channels"))
                        }
                        audioStreams.put(converted)
                    }
                }
                val format = media.optJSONObject("format") ?: JSONObject()
                result.put("videoStreams", videoStreams)
                result.put("audioStreams", audioStreams)
                result.put("mediaInformation", media)
                result.put(
                    "mediaInfo",
                    JSONObject()
                        .put("format", format.optString("format_name"))
                        .put("duration", format.optString("duration"))
                        .put("bitrate", format.optString("bit_rate"))
                        .put("videoStreams", videoStreams)
                        .put("audioStreams", audioStreams),
                )
            } else {
                // Encoding success and metadata probing are distinct operations.
                // Preserve the output and report the real probe failure separately.
                result.put("probeError", mediaSession.failStackTrace ?: mediaSession.output ?: "No media metadata returned")
            }
        }
        return result.toString()
    }

    private fun result(session: FFmpegSession, command: String, startedAt: Long): JSONObject {
        val returnCode = requireNotNull(session.returnCode) {
            session.failStackTrace ?: "FFmpeg did not produce a native return code"
        }.value
        val output = session.output ?: ""
        return JSONObject()
            .put("success", returnCode == 0)
            .put("command", command)
            .put("returnCode", returnCode)
            .put("output", output)
            .put("duration", (System.nanoTime() - startedAt) / 1_000_000)
            .put("videoStreams", JSONArray())
            .put("audioStreams", JSONArray())
    }

    private fun option(options: JSONObject, name: String): String? =
        if (options.isNull(name)) null else options.getString(name).trim().takeIf { it.isNotEmpty() }

    private fun videoEncoder(codec: String): String = when (codec.lowercase()) {
        "h264" -> "libx264"
        "hevc" -> "libx265"
        "vp8" -> "libvpx"
        "vp9" -> "libvpx-vp9"
        "av1", "libaom" -> "libaom-av1"
        else -> codec
    }

    private fun audioEncoder(codec: String): String = when (codec.lowercase()) {
        "mp3" -> "libmp3lame"
        "opus" -> "libopus"
        "vorbis" -> "libvorbis"
        "pcm", "wav" -> "pcm_s16le"
        else -> codec
    }
}
