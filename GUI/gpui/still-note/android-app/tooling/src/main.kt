fun main(args: Array<String>) {
    require(args.isNotEmpty()) { "Usage: kotlin run -m tooling -- format [--write]|lint|ui" }
    when (args[0]) {
        "format" -> runFormat(args.drop(1).toTypedArray())
        "lint",
        "ui" -> runAndroidChecks(args)
        else -> error("Unknown verification command: ${args[0]}")
    }
}
