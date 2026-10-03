import com.facebook.ktfmt.format.Formatter
import java.io.File

fun runFormat(args: Array<String>) {
    val root = File(System.getProperty("user.dir"))
    require(File(root, "project.yaml").isFile) { "Run from android-app." }
    require(args.isEmpty() || args.toList() == listOf("--write")) {
        "Usage: format.main.kts [--write]"
    }
    val write = args.isNotEmpty()
    val paths = listOf("app/src", "app/test", "app/instrumentedTest", "core/src", "tooling/src")
    var failures = 0
    for (path in paths) {
        File(root, path)
            .walkTopDown()
            .filter { it.isFile && (it.extension == "kt" || it.extension == "kts") }
            .forEach { file ->
                val original = file.readText()
                val formatted = Formatter.format(Formatter.KOTLINLANG_FORMAT, original)
                if (original.replace("\r\n", "\n") != formatted) {
                    if (write) file.writeText(formatted)
                    else {
                        println("Needs formatting: ${file.relativeTo(root)}")
                        failures++
                    }
                }
            }
    }
    check(failures == 0) { "$failures Kotlin files need formatting. Run with --write." }
    println("Kotlin formatting checked (ktfmt 0.54).")
}
