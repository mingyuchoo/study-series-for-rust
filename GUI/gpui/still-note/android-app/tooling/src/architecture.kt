import java.io.File

/** Lightweight source guard for inward dependencies; module compilation guards app -> core. */
fun runArchitecture(args: Array<String>) {
    require(args.isEmpty()) { "Usage: kotlin run -m tooling -- architecture" }
    val root = File(System.getProperty("user.dir"))
    require(File(root, "project.yaml").isFile) { "Run from android-app." }
    val violations = mutableListOf<String>()
    for (path in listOf("core/src/main/kotlin", "app/src/app/stillnote")) {
        File(root, path)
            .walkTopDown()
            .filter { it.extension == "kt" }
            .forEach { file ->
                val source = file.readText()
                val pkg = Regex("(?m)^package ([\\w.]+)").find(source)?.groupValues?.get(1) ?: ""
                val references =
                    Regex(
                            "(?:app\\.stillnote|androidx?|kotlinx|java\\.io|java\\.nio\\.file)\\.[\\w.]+"
                        )
                        .findAll(source)
                        .map { it.value }
                        .toList()
                val forbidden =
                    when {
                        path.startsWith("core") ->
                            references.filter {
                                it.startsWith("android") ||
                                    it.startsWith("kotlinx") ||
                                    it.startsWith("java.io.") ||
                                    it.startsWith("java.nio.file.") ||
                                    listOf("data", "di", "presentation", "ui").any { layer ->
                                        it.startsWith("app.stillnote.$layer.")
                                    } ||
                                    (pkg.endsWith("domain") &&
                                        it.startsWith("app.stillnote.application."))
                            }
                        pkg.endsWith("presentation") ->
                            references.filter {
                                listOf("data", "di", "ui").any { layer ->
                                    it.startsWith("app.stillnote.$layer.")
                                }
                            }
                        pkg.endsWith("data") ->
                            references.filter {
                                listOf("di", "presentation", "ui").any { layer ->
                                    it.startsWith("app.stillnote.$layer.")
                                }
                            }
                        pkg.endsWith("ui") ->
                            references.filter {
                                listOf("data", "di").any { layer ->
                                    it.startsWith("app.stillnote.$layer.")
                                } ||
                                    (file.name != "StillnoteRoute.kt" &&
                                        (it.startsWith("androidx.lifecycle.") ||
                                            it == "app.stillnote.presentation.JournalViewModel"))
                            }
                        else -> emptyList()
                    }
                forbidden.distinct().forEach {
                    violations += "${file.relativeTo(root)}: forbidden reference $it"
                }
                if (
                    path.startsWith("core") &&
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
