// JNI field/class ABI from k2-fsa/sherpa-ncnn v2.1.15 (Apache-2.0).
// Explicit release is adapted from Operit 1 to avoid retaining native models until GC.
package com.k2fsa.sherpa.ncnn

import android.content.res.AssetManager

data class FeatureExtractorConfig(
    var sampleRate: Float,
    var featureDim: Int,
)


data class ModelConfig(
    var encoderParam: String,
    var encoderBin: String,
    var decoderParam: String,
    var decoderBin: String,
    var joinerParam: String,
    var joinerBin: String,
    var tokens: String,
    var numThreads: Int = 1,
    var useGPU: Boolean = true, // If there is a GPU and useGPU true, we will use GPU
)

data class DecoderConfig(
    var method: String = "modified_beam_search", // valid values: greedy_search, modified_beam_search
    var numActivePaths: Int = 4, // used only by modified_beam_search
)

data class RecognizerConfig(
    var featConfig: FeatureExtractorConfig,
    var modelConfig: ModelConfig,
    var decoderConfig: DecoderConfig,
    var enableEndpoint: Boolean = true,
    var rule1MinTrailingSilence: Float = 2.4f,
    var rule2MinTrailingSilence: Float = 1.0f,
    var rule3MinUtteranceLength: Float = 30.0f,
    var hotwordsFile: String = "",
    var hotwordsScore: Float = 1.5f,
)

class SherpaNcnn(
    var config: RecognizerConfig,
    assetManager: AssetManager? = null,
) {
    private var ptr: Long

    init {
        if (assetManager != null) {
            ptr = newFromAsset(assetManager, config)
        } else {
            ptr = newFromFile(config)
        }
        require(ptr != 0L) { "Sherpa NCNN failed to create the recognizer" }
    }

    protected fun finalize() {
        release()
    }

    @Synchronized
    fun release() {
        val p = ptr
        if (p == 0L) return
        ptr = 0L
        delete(p)
    }

    @Synchronized
    fun acceptSamples(samples: FloatArray) =
        acceptWaveform(handle(), samples = samples, sampleRate = config.featConfig.sampleRate)

    @Synchronized
    fun isReady() = isReady(handle())

    @Synchronized
    fun decode() = decode(handle())

    @Synchronized
    fun inputFinished() = inputFinished(handle())
    @Synchronized
    fun isEndpoint(): Boolean = isEndpoint(handle())
    @Synchronized
    fun reset(recreate: Boolean = false) = reset(handle(), recreate = recreate)

    val text: String
        @Synchronized get() = getText(handle())

    private fun handle(): Long = ptr.also { check(it != 0L) { "Sherpa NCNN recognizer is released" } }

    private external fun newFromAsset(
        assetManager: AssetManager,
        config: RecognizerConfig,
    ): Long

    private external fun newFromFile(
        config: RecognizerConfig,
    ): Long

    private external fun delete(ptr: Long)
    private external fun acceptWaveform(ptr: Long, samples: FloatArray, sampleRate: Float)
    private external fun inputFinished(ptr: Long)
    private external fun isReady(ptr: Long): Boolean
    private external fun decode(ptr: Long)
    private external fun isEndpoint(ptr: Long): Boolean
    private external fun reset(ptr: Long, recreate: Boolean)
    private external fun getText(ptr: Long): String


}
