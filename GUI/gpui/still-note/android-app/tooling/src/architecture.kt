import java.io.File
import org.yaml.snakeyaml.Yaml

/** Lightweight source guard for inward dependencies; module compilation guards app -> core. */
fun runArchitecture(args: Array<String>) {
    require(args.isEmpty()) { "Usage: kotlin run -m tooling -- architecture" }
    val root = File(System.getProperty("user.dir"))
    require(File(root, "project.yaml").isFile) { "Run from android-app." }
    val violations = mutableListOf<String>()
    val yaml = Yaml()
    fun dependencies(module: String): List<*> =
        (yaml.load<Map<String, Any>>(File(root, "$module/module.yaml").readText())["dependencies"]
            as? List<*>) ?: emptyList<Any>()
    check(dependencies("core").isEmpty()) { "core must not depend on adapters or presentation" }
    check(dependencies("presentation") == listOf("//core")) {
        "presentation must depend only on core"
    }
    check(dependencies("app").containsAll(listOf("//core", "//presentation"))) {
        "app must wire core and presentation"
    }
    for (path in
        listOf("core/src/main/kotlin", "presentation/src/main/kotlin", "app/src/app/stillnote")) {
        File(root, path)
            .walkTopDown()
            .filter { it.extension == "kt" }
            .forEach { file ->
                val source = file.readText()
                val pkg = Regex("(?m)^package ([\\w.]+)").find(source)?.groupValues?.get(1) ?: ""
                val references =
                    Regex(
                            "(?:app\\.stillnote|androidx?|kotlinx|java\\.io|java\\.net|java\\.nio\\.file)\\.[\\w.]+"
                        )
                        .findAll(source)
                        .map { it.value }
                        .toList()
                val forbidden =
                    when {
                        path.startsWith("core") || path.startsWith("presentation") ->
                            references.filter {
                                it.startsWith("android") ||
                                    it.startsWith("kotlinx") ||
                                    it.startsWith("java.io.") ||
                                    it.startsWith("java.net.") ||
                                    it.startsWith("java.nio.file.") ||
                                    listOf("data", "di", "ui").any { layer ->
                                        it.startsWith("app.stillnote.$layer.")
                                    } ||
                                    (path.startsWith("core") &&
                                        it.startsWith("app.stillnote.presentation.")) ||
                                    (pkg.startsWith("app.stillnote.domain") &&
                                        it.startsWith("app.stillnote.application."))
                            }
                        pkg.startsWith("app.stillnote.presentation") ->
                            references.filter {
                                listOf("data", "di", "ui").any { layer ->
                                    it.startsWith("app.stillnote.$layer.")
                                }
                            }
                        pkg.startsWith("app.stillnote.data") ->
                            references.filter {
                                listOf("di", "presentation", "ui").any { layer ->
                                    it.startsWith("app.stillnote.$layer.")
                                }
                            }
                        pkg.startsWith("app.stillnote.ui") ->
                            references.filter {
                                listOf("data", "di").any { layer ->
                                    it.startsWith("app.stillnote.$layer.")
                                } ||
                                    (file.name != "StillnoteRoute.kt" &&
                                        (it.startsWith("androidx.lifecycle.") ||
                                            it == "app.stillnote.presentation.JournalViewModel")) ||
                                    (file.name !in
                                        setOf("StillnoteScreen.kt", "StillnoteRoute.kt") &&
                                        (it.startsWith("app.stillnote.application.") ||
                                            it ==
                                                "app.stillnote.presentation.JournalScreenActions"))
                            }
                        else -> emptyList()
                    }
                forbidden.distinct().forEach {
                    violations += "${file.relativeTo(root)}: forbidden reference $it"
                }
                if (
                    (path.startsWith("core") || path.startsWith("presentation")) &&
                        Regex(
                                "\\b(randomUUID|currentTimeMillis|nanoTime)\\s*\\(|\\b(LocalDate|LocalDateTime|Instant|Clock)\\.\\b(now|systemUTC|systemDefaultZone)\\s*\\("
                            )
                            .containsMatchIn(source)
                ) {
                    violations +=
                        "${file.relativeTo(root)}: identity/clock effects must be supplied by adapters"
                }
            }
    }
    check(violations.isEmpty()) { violations.joinToString("\n") }
    println("Architecture dependency rules checked.")
}
