package app.stillnote.data

import app.stillnote.application.*
import app.stillnote.domain.*
import java.io.File

class SettingsStore(val path: File, private val access: FileAccess = FileAccess()) :
    SettingsRepository {
    private var baseline: ByteArray? = null
    private var loaded = false

    @Synchronized
    override fun load(): Settings {
        loaded = false
        try {
            val bytes = access.read(path)
            val s =
                if (bytes == null) Settings()
                else SettingsCodec.decode(bytes.decodeToString(throwOnInvalidSequence = true))

            baseline = bytes
            loaded = true
            return s
        } catch (e: Exception) {
            throw RepositoryException("settings_load", e)
        }
    }

    @Synchronized
    override fun save(settings: Settings) {
        if (!loaded) throw RepositoryException("settings_protected")
        try {
            if (!sameBytes(access.read(path), baseline))
                throw RepositoryException("settings_conflict")
            val bytes = SettingsCodec.encode(settings).toByteArray(Charsets.UTF_8)
            access.replace(path, bytes)
            baseline = bytes
        } catch (e: RepositoryException) {
            throw e
        } catch (e: Exception) {
            throw RepositoryException("settings_save", e)
        }
    }
}
