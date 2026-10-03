package app.stillnote

import app.stillnote.data.*
import app.stillnote.domain.*
import java.io.File
import java.io.IOException
import java.util.UUID
import kotlinx.serialization.json.Json
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class StoreTest {
    @get:Rule val temporary = TemporaryFolder()

    private fun path() = File(temporary.root, "journal.json")

    private fun note(j: Journal, text: String) =
        j.addEntry(parseDate("2026-10-03"), Log.Daily, Kind.Note, text, UUID.randomUUID())

    private fun rejected(action: () -> Unit) = assertThrows(Exception::class.java, action)

    @Test
    fun ac07_ac08_invalidSurrogateEscapesAndDeepJsonProtectOriginal() {
        val fixture = javaClass.getResource("/desktop-v1.json")!!.readText()
        val invalids =
            listOf("\\uD800", "\\uDC00", "\\uD800\\u0041", "\\uD800\\uD800").map { escaped ->
                fixture.replace("\"text\": \"읽은 책\"", "\"text\": \"$escaped\"")
            } +
                fixture.replaceFirst(
                    "{",
                    "{\"unknown\":" + "[".repeat(150) + "0" + "]".repeat(150) + ",",
                )
        invalids.forEach { invalid ->
            assertNotEquals(fixture, invalid)
            path().writeText(invalid)
            val store = JournalStore(path())
            rejected { store.load() }
            rejected { store.save(Journal()) }
            assertEquals(invalid, path().readText())
        }
        val valid = fixture.replace("\"text\": \"읽은 책\"", "\"text\": \"\\uD83D\\uDCDA\"")
        val decoded = JournalCodec.decode(valid)
        assertEquals("📚", decoded.entries[2].text)
        assertEquals(decoded, JournalCodec.decode(JournalCodec.encode(decoded)))
    }

    @Test
    fun ac07_ac08_duplicateJsonKeysIncludingEscapesProtectOriginal() {
        val fixture = javaClass.getResource("/desktop-v1.json")!!.readText()
        val invalids =
            listOf(
                fixture.replace("\"version\": 1", "\"version\": 2, \"version\": 1"),
                fixture.replace("\"version\": 1", "\"\\u0076ersion\": 2, \"version\": 1"),
                fixture.replace("\"text\": \"읽은 책\"", "\"text\": \"hidden\", \"text\": \"읽은 책\""),
                fixture.replace(
                    "\"status\": \"Scheduled\"",
                    "\"status\": \"Open\", \"status\": \"Scheduled\"",
                ),
                fixture.replace(
                    "\"id\": \"00000000-0000-0000-0000-000000000003\"",
                    "\"id\": \"00000000-0000-0000-0000-000000000099\", \"id\": \"00000000-0000-0000-0000-000000000003\"",
                ),
                fixture.replace("\"name\": \"독서 📚\"", "\"name\": \"hidden\", \"name\": \"독서 📚\""),
                fixture.replace(
                    "\"Collection\": \"00000000-0000-0000-0000-000000000064\"",
                    "\"Collection\": \"00000000-0000-0000-0000-000000000099\", \"Collection\": \"00000000-0000-0000-0000-000000000064\"",
                ),
            )
        invalids.forEach { invalid ->
            assertNotEquals(fixture, invalid)
            path().writeText(invalid)
            val store = JournalStore(path())
            rejected { store.load() }
            rejected { store.save(Journal()) }
            assertEquals(invalid, path().readText())
        }
        val settingsFile = File(temporary.root, "settings.json")
        listOf(
                "{\"language\":\"english\",\"language\":\"korean\",\"theme\":\"system\"}",
                "{\"language\":\"korean\",\"theme\":\"dark\",\"theme\":\"system\"}",
                "{\"\\u006canguage\":\"english\",\"language\":\"korean\",\"theme\":\"system\"}",
            )
            .forEach { invalid ->
                settingsFile.writeText(invalid)
                val store = SettingsStore(settingsFile)
                rejected { store.load() }
                rejected { store.save(Settings()) }
                assertEquals(invalid, settingsFile.readText())
            }
    }

    @Test
    fun ac08_malformedUtf8InsideValidJsonTextNeverReplacesOriginal() {
        val json = JournalCodec.encode(note(Journal(), "bad"))
        val bytes =
            json.substringBefore("bad").toByteArray() +
                byteArrayOf(0xc3.toByte(), 0x28) +
                json.substringAfter("bad").toByteArray()
        path().writeBytes(bytes)
        val store = JournalStore(path())
        rejected { store.load() }
        rejected { store.save(Journal()) }
        assertArrayEquals(bytes, path().readBytes())
        val file = File(temporary.root, "settings.json")
        file.writeBytes(byteArrayOf(0xc3.toByte(), 0x28))
        val settings = SettingsStore(file)
        rejected { settings.load() }
        rejected { settings.save(Settings()) }
        assertArrayEquals(byteArrayOf(0xc3.toByte(), 0x28), file.readBytes())
    }

    @Test
    fun ac01_ac07_firstLoadIsBlankAndNeverWritesSeeds() {
        val file = path()
        val session = StoreSession(JournalStore(file))
        assertEquals(Journal(), session.journal)
        assertFalse(file.exists())
        assertEquals(emptyList<String>(), temporary.root.listFiles()!!.map { it.name })
    }

    @Test
    fun ac07_rustGeneratedFixtureExactSemanticRoundtripAndDurableRestart() {
        val fixture = javaClass.getResource("/desktop-v1.json")!!.readText()
        val journal = JournalCodec.decode(fixture)
        assertEquals(6, journal.entries.size)
        assertEquals("독서 📚", journal.collections.single().name)
        assertEquals(Log.Collection(UUID(0, 100)), journal.entries[2].log)
        assertEquals("한글 café 📝", journal.entries[0].text)
        assertEquals(parseDate("0001-01-01"), journal.entries[4].date)
        assertEquals(parseDate("9999-12-31"), journal.entries[5].date)
        assertEquals(UUID(0, 2), journal.entries[0].migratedTo)
        assertEquals(UUID(0, 1), journal.entries[1].migratedFrom)
        val encoded = JournalCodec.encode(journal)
        assertEquals(Json.parseToJsonElement(fixture), Json.parseToJsonElement(encoded))
        val store = JournalStore(path())
        store.load()
        store.save(journal)
        assertEquals(journal, JournalStore(path()).load())
        // Durable output for the separate real desktop-model Rust validation command.
        val destination = System.getProperty("stillnote.compatibility.output")
        if (destination != null) File(destination).writeText(encoded)
    }

    @Test
    fun ac08_backupEqualsPreviousDurableSnapshot() {
        val session = StoreSession(JournalStore(path()))
        session.transact { note(it, "first") }
        val first = path().readBytes()
        session.transact { note(it, "second") }
        assertArrayEquals(first, session.store.backupPath.readBytes())
        assertEquals(
            listOf("first", "second"),
            StoreSession(JournalStore(path())).journal.entries.map { it.text },
        )
    }

    @Test
    fun ac08_corruptLoadAndInvalidSemanticsProtectOriginal() {
        listOf(
                "{broken",
                "{\"version\":2,\"entries\":[],\"collections\":[]}",
                "{\"version\":1,\"entries\":[],\"collections\":[{\"id\":\"00000000-0000-0000-0000-000000000000\",\"name\":\"bad\"}]}",
            )
            .forEach { invalid ->
                val file = path()
                file.writeText(invalid)
                val store = JournalStore(file)
                rejected { store.load() }
                rejected { store.save(Journal()) }
                assertEquals(invalid, file.readText())
                assertFalse(store.backupPath.exists())
            }
    }

    @Test
    fun ac07_ac08_wrongJsonTypesAndNonCanonicalUuidAreProtected() {
        val fixture = javaClass.getResource("/desktop-v1.json")!!.readText()
        val invalids =
            listOf(
                fixture.replace("\"version\": 1", "\"version\": \"1\""),
                fixture.replace("\"text\": \"읽은 책\"", "\"text\": 123"),
                fixture.replace("\"important\": true", "\"important\": \"true\""),
                fixture.replace("\"name\": \"독서 📚\"", "\"name\": false"),
                fixture.replace("00000000-0000-0000-0000-000000000003", "1-1-1-1-1"),
            )
        invalids.forEach { invalid ->
            assertNotEquals("Fixture mutation did not alter source", fixture, invalid)
            path().writeText(invalid)
            val store = JournalStore(path())
            rejected { store.load() }
            rejected { store.save(Journal()) }
            assertEquals(invalid, path().readText())
        }
    }

    @Test
    fun ac08_injectedReadFailureProtectsCurrentModelAndOriginal() {
        var fail = false
        val access =
            object : FileAccess() {
                override fun read(file: File): ByteArray? {
                    if (fail) throw IOException("injected unreadable file")
                    return super.read(file)
                }
            }
        val session = StoreSession(JournalStore(path(), access))
        session.transact { note(it, "original") }
        val bytes = path().readBytes()
        val model = session.journal
        fail = true
        rejected { session.transact { note(it, "uncommitted") } }
        assertArrayEquals(bytes, path().readBytes())
        assertEquals(model, session.journal)
        val store = JournalStore(path(), access)
        rejected { store.load() }
        rejected { store.save(Journal()) }
        assertArrayEquals(bytes, path().readBytes())
    }

    @Test
    fun ac08_externalChangesAndDeletionProtectBytesAndMemory() {
        val session = StoreSession(JournalStore(path()))
        session.transact { note(it, "original") }
        val original = session.journal
        path().writeText("external edit")
        rejected { session.transact { note(it, "should fail") } }
        assertEquals(original, session.journal)
        assertEquals("external edit", path().readText())
        path().delete()
        rejected { session.transact { note(it, "also fails") } }
        assertFalse(path().exists())
        assertEquals(original, session.journal)
    }

    @Test
    fun ac08_faultsAtBackupAndReplacementKeepOriginalAndModel() {
        listOf("journal.json.bak", "journal.json").forEachIndexed { index, failedName ->
            val folder = temporary.newFolder("fault-$index")
            val file = File(folder, "journal.json")
            var fail = false
            val access =
                object : FileAccess() {
                    override fun replace(file: File, bytes: ByteArray) {
                        if (fail && file.name == failedName) throw IOException("injected failure")
                        super.replace(file, bytes)
                    }
                }
            val session = StoreSession(JournalStore(file, access))
            session.transact { note(it, "original") }
            val bytes = file.readBytes()
            val model = session.journal
            fail = true
            rejected { session.transact { note(it, "uncommitted") } }
            assertArrayEquals(bytes, file.readBytes())
            assertEquals(model, session.journal)
            assertFalse(folder.listFiles()!!.any { it.name.endsWith(".tmp") })
        }
    }

    @Test
    fun ac08_realBackupPathFailureKeepsBytesAndMemory() {
        val session = StoreSession(JournalStore(path()))
        session.transact { note(it, "original") }
        val bytes = path().readBytes()
        val model = session.journal
        session.store.backupPath.mkdirs()
        File(session.store.backupPath, "keep").writeText("block replacement")
        rejected { session.transact { note(it, "uncommitted") } }
        assertArrayEquals(bytes, path().readBytes())
        assertEquals(model, session.journal)
        assertFalse(temporary.root.listFiles()!!.any { it.name.endsWith(".tmp") })
    }

    @Test
    fun ac08_operationFailureAndSerializedConcurrentWrites() {
        val session = StoreSession(JournalStore(path()))
        session.transact { note(it, "original") }
        val bytes = path().readBytes()
        rejected {
            session.transact {
                note(it, "discard")
                throw IOException("operation failed")
            }
        }
        assertArrayEquals(bytes, path().readBytes())
        assertEquals(1, session.journal.entries.size)
        val errors = java.util.concurrent.ConcurrentLinkedQueue<Throwable>()
        val threads =
            (1..12).map { index ->
                Thread {
                    try {
                        session.transact { note(it, "thread $index") }
                    } catch (e: Throwable) {
                        errors.add(e)
                    }
                }
            }
        threads.forEach { it.start() }
        threads.forEach { it.join() }
        assertTrue(errors.toString(), errors.isEmpty())
        assertEquals(13, session.journal.entries.size)
        assertEquals(session.journal, JournalStore(path()).load())
    }

    @Test
    fun ac09_settingsDefaultsRoundtripIsolationAndCorruption() {
        val settingsFile = File(temporary.root, "settings.json")
        val store = SettingsStore(settingsFile)
        assertEquals(Settings(Language.Korean, ThemeMode.System), store.load())
        assertFalse(settingsFile.exists())
        store.save(Settings(Language.English, ThemeMode.Dark))
        assertEquals(
            Json.parseToJsonElement("{\"language\":\"english\",\"theme\":\"dark\"}"),
            Json.parseToJsonElement(settingsFile.readText()),
        )
        assertEquals(Settings(Language.English, ThemeMode.Dark), SettingsStore(settingsFile).load())
        assertFalse(path().exists())
        settingsFile.writeText("{corrupt settings}")
        val corrupt = SettingsStore(settingsFile)
        rejected { corrupt.load() }
        rejected { corrupt.save(Settings()) }
        assertEquals("{corrupt settings}", settingsFile.readText())
        val session = StoreSession(JournalStore(path()))
        session.transact { note(it, "journal still works") }
        assertEquals("journal still works", JournalStore(path()).load().entries.single().text)
    }

    @Test
    fun ac09_settingsInvalidEnumsExternalConflictAndReplacementFailure() {
        val file = File(temporary.root, "settings.json")
        listOf(
                "{\"language\":\"Klingon\",\"theme\":\"system\"}",
                "{\"language\":\"korean\",\"theme\":\"Purple\"}",
                "{\"language\":false,\"theme\":\"system\"}",
            )
            .forEach { invalid ->
                file.writeText(invalid)
                val store = SettingsStore(file)
                rejected { store.load() }
                rejected { store.save(Settings()) }
                assertEquals(invalid, file.readText())
            }
        file.delete()
        val store = SettingsStore(file)
        store.load()
        store.save(Settings())
        file.writeText("external")
        rejected { store.save(Settings(Language.English, ThemeMode.Light)) }
        assertEquals("external", file.readText())
        file.delete()
        val fault =
            SettingsStore(
                file,
                object : FileAccess() {
                    override fun replace(file: File, bytes: ByteArray) {
                        throw IOException("settings failure")
                    }
                },
            )
        fault.load()
        rejected { fault.save(Settings()) }
        assertFalse(file.exists())
    }
}
