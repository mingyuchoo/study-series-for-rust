package app.stillnote.data

import app.stillnote.application.JournalRepository
import app.stillnote.domain.Journal
import java.io.File

class JournalStore(path: File, access: FileAccess = FileAccess()) : JournalRepository {
    val backupPath: File = File(path.parentFile, path.name + ".bak")

    private val file =
        ProtectedFileStore(
            path,
            access,
            "journal",
            { Journal() },
            JournalCodec::decode,
            JournalCodec::encode,
            backupPath,
        )

    override fun load(): Journal = file.load()

    override fun save(journal: Journal) = file.save(journal, Journal::validate)
}
