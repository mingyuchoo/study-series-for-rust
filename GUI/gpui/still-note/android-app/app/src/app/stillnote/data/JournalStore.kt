package app.stillnote.data

import app.stillnote.application.*
import app.stillnote.domain.*
import java.io.File

class JournalStore(val path: File, private val access: FileAccess = FileAccess()) :
    JournalRepository {
    val backupPath: File
        get() = File(path.parentFile, path.name + ".bak")

    private var baseline: ByteArray? = null
    private var loaded = false

    @Synchronized
    override fun load(): Journal {
        loaded = false
        try {
            val bytes = access.read(path)
            val journal =
                bytes?.let { JournalCodec.decode(it.decodeToString(throwOnInvalidSequence = true)) }
                    ?: Journal()
            baseline = bytes
            loaded = true
            return journal
        } catch (e: Exception) {
            throw RepositoryException("journal_load", e)
        }
    }

    @Synchronized
    override fun save(journal: Journal) {
        if (!loaded) throw RepositoryException("journal_protected")
        journal.validate()
        val current =
            try {
                access.read(path)
            } catch (e: Exception) {
                throw RepositoryException("journal_save", e)
            }
        if (!sameBytes(current, baseline)) throw RepositoryException("journal_conflict")
        val bytes = JournalCodec.encode(journal).toByteArray(Charsets.UTF_8)
        try {
            if (current != null) access.replace(backupPath, current)
            access.replace(path, bytes)
            baseline = bytes
        } catch (e: Exception) {
            throw RepositoryException("journal_save", e)
        }
    }
}
