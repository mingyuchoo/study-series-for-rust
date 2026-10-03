package app.stillnote.data

import app.stillnote.application.SettingsRepository
import app.stillnote.domain.Settings
import java.io.File

class SettingsStore(path: File, access: FileAccess = FileAccess()) : SettingsRepository {
    private val file =
        ProtectedFileStore(
            path,
            access,
            "settings",
            { Settings() },
            SettingsCodec::decode,
            SettingsCodec::encode,
        )

    override fun load(): Settings = file.load()

    override fun save(settings: Settings) = file.save(settings)
}
