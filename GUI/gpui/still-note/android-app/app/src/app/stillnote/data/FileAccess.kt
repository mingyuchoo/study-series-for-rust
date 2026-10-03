package app.stillnote.data

import java.io.File
import java.io.FileOutputStream
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.util.UUID

/** Atomic, baseline-aware file boundary; override replace for deterministic fault tests. */
open class FileAccess {
    open fun read(file: File): ByteArray? =
        try {
            Files.readAllBytes(file.toPath())
        } catch (_: java.nio.file.NoSuchFileException) {
            null
        }

    open fun replace(file: File, bytes: ByteArray) {
        file.parentFile?.mkdirs()
        val temp = File(file.parentFile, ".stillnote-${UUID.randomUUID()}.tmp")
        try {
            FileOutputStream(temp).use {
                it.write(bytes)
                it.fd.sync()
            }
            Files.move(
                temp.toPath(),
                file.toPath(),
                StandardCopyOption.ATOMIC_MOVE,
                StandardCopyOption.REPLACE_EXISTING,
            )
        } finally {
            temp.delete()
        }
    }
}
