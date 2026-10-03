fun main(args: Array<String>) {
    val commandArgs = args.ifEmpty { arrayOf("format") }
    when (commandArgs[0]) {
        "format" -> runFormat(commandArgs.drop(1).toTypedArray())
        "lint",
        "ui" -> runAndroidChecks(commandArgs)
        else ->
            error(
                "Unknown verification command: ${commandArgs[0]}. " +
                    "Usage: kotlin run -m tooling -- format [--write]|lint|ui"
            )
    }
}
