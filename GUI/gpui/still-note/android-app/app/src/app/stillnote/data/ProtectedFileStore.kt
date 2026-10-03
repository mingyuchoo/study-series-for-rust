package app.stillnote.data

import app.stillnote.application.RepositoryException
import java.io.File

/** Owns the durable baseline and file effects, independently of the stored model. */
internal class ProtectedFileStore<T>(
    private val path: File,
    private val access: FileAccess,
    private val errorPrefix: String,
    private val empty: () -> T,
    private val decode: (String) -> T,
    private val encode: (T) -> String,
    private val backup: File? = null,
) {
    private var baseline: ByteArray? = null
    private var loaded = false

    @Synchronized
    fun load(): T {
        loaded = false
        try {
            val bytes = access.read(path)
            val value =
                bytes?.let { decode(it.decodeToString(throwOnInvalidSequence = true)) } ?: empty()
            baseline = bytes
            loaded = true
            return value
        } catch (e: Exception) {
            throw RepositoryException("${errorPrefix}_load", e)
        }
    }

    @Synchronized
    fun save(value: T, validate: (T) -> Unit = {}) {
        if (!loaded) throw RepositoryException("${errorPrefix}_protected")
        validate(value)
        try {
            val current = access.read(path)
            if (!sameBytes(current, baseline)) throw RepositoryException("${errorPrefix}_conflict")
            val bytes = encode(value).toByteArray(Charsets.UTF_8)
            if (current != null && backup != null) access.replace(backup, current)
            access.replace(path, bytes)
            baseline = bytes
        } catch (e: RepositoryException) {
            throw e
        } catch (e: Exception) {
            throw RepositoryException("${errorPrefix}_save", e)
        }
    }
}
