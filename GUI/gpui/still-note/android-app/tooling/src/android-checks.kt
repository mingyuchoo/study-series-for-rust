import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.gradle.tooling.GradleConnector
import org.yaml.snakeyaml.Yaml

fun runAndroidChecks(args: Array<String>) {
    // Kotlin Toolchain 0.13 has no public lint/instrumentation command. This bridge
    // checks its precompiled production jars; it never compiles production Kotlin.
    val root = File(System.getProperty("user.dir")).canonicalFile
    require(File(root, "project.yaml").isFile) { "Run from android-app." }
    require(args.size == 1 && args[0] in listOf("lint", "ui")) {
        "Usage: kotlin run -m tooling -- lint|ui"
    }
    val ui = args[0] == "ui"
    val yaml = Yaml().load<Map<String, Any>>(File(root, "app/module.yaml").readText())
    val settings = yaml["settings"] as Map<*, *>
    val android = settings["android"] as Map<*, *>
    val kotlin = settings["kotlin"] as Map<*, *>
    val serialization = kotlin["serialization"] as Map<*, *>
    val jdk =
        File(
            requireNotNull(System.getenv("JAVA_HOME")) {
                "Verification requires JAVA_HOME pointing to JDK 17."
            }
        )
    require(
        File(
                jdk,
                "bin/java" + if (System.getProperty("os.name").startsWith("Windows")) ".exe" else "",
            )
            .isFile
    )
    val sdk =
        File(
            requireNotNull(System.getenv("ANDROID_HOME")) { "Set ANDROID_HOME to the Android SDK." }
        )
    val adb =
        File(
            sdk,
            "platform-tools/adb" +
                if (System.getProperty("os.name").startsWith("Windows")) ".exe" else "",
        )
    fun adbOutput(vararg arguments: String): String {
        val process =
            ProcessBuilder(listOf(adb.absolutePath) + arguments).redirectErrorStream(true).start()
        val output = process.inputStream.bufferedReader().readText()
        check(process.waitFor() == 0) { output }
        return output.trim()
    }
    if (ui) {
        val serial = System.getenv("ANDROID_SERIAL") ?: ""
        require(Regex("emulator-[0-9]+").matches(serial)) {
            "Set ANDROID_SERIAL to the isolated API 36 emulator."
        }
        require(adbOutput("-s", serial, "shell", "getprop", "ro.kernel.qemu") == "1")
        require(adbOutput("-s", serial, "shell", "getprop", "ro.build.version.sdk") == "36")
        require(
            adbOutput(
                "-s",
                serial,
                "shell",
                "settings",
                "get",
                "secure",
                "show_ime_with_hard_keyboard",
            ) == "1"
        ) {
            "The verification emulator must show the software keyboard with a hardware keyboard attached."
        }
        adbOutput("-s", serial, "shell", "svc", "power", "stayon", "true")
        adbOutput("-s", serial, "shell", "input", "keyevent", "KEYCODE_WAKEUP")
        adbOutput("-s", serial, "shell", "wm", "dismiss-keyguard")
    }
    fun literal(value: Any?): String =
        "\"" +
            value.toString().replace("\\", "\\\\").replace("\"", "\\\"").replace("$", "\\$") +
            "\""
    fun path(value: String) = literal(File(root, value).invariantSeparatorsPath)
    val mainJar = File(root, "build/tasks/_app_jarAndroidDebug/app-jvm.jar")
    val coreJar = File(root, "build/tasks/_core_jarJvm/core-jvm.jar")
    val presentationJar = File(root, "build/tasks/_presentation_jarJvm/presentation-jvm.jar")
    require(mainJar.isFile && coreJar.isFile && presentationJar.isFile) {
        "Run kotlin build first."
    }
    val bridge = File(root, "build/android-checks").apply { mkdirs() }
    if (ui) {
        val serial = System.getenv("ANDROID_SERIAL")
        val apk = File(root, "build/tasks/_app_buildAndroidDebug/gradle-project-debug.apk")
        require(apk.isFile) { "Run kotlin build first." }
        println(adbOutput("-s", serial, "install", "-r", apk.absolutePath))
        val applicationId = android["applicationId"].toString()
        adbOutput("-s", serial, "shell", "am", "force-stop", applicationId)
        val launch =
            adbOutput(
                "-s",
                serial,
                "shell",
                "am",
                "start",
                "-W",
                "-n",
                "$applicationId/${android["namespace"]}.MainActivity",
            )
        check("Status: ok" in launch && "Error" !in launch) { launch }
        println("Kotlin CLI APK launch passed on $serial.")
        val results = File(bridge, "build/outputs/androidTest-results")
        check(
            results.canonicalPath.startsWith(File(bridge, "build").canonicalPath + File.separator)
        )
        if (results.exists()) check(results.deleteRecursively()) { "Cannot clear old UI results." }
    }
    File(bridge, "settings.gradle.kts")
        .writeText(
            """
pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }
dependencyResolutionManagement { repositories { google(); mavenCentral() } }
rootProject.name = "StillnoteVerification"
"""
                .trimIndent()
        )
    File(bridge, "gradle.properties")
        .writeText("android.useAndroidX=true\norg.gradle.jvmargs=-Xmx3072m -Dfile.encoding=UTF-8\n")
    val dependencies =
        (yaml["dependencies"] as List<*>)
            .mapNotNull { dependency ->
                when (dependency) {
                    is String ->
                        if (dependency.startsWith("//")) null
                        else "implementation(${literal(dependency)})"
                    is Map<*, *> -> "implementation(platform(${literal(dependency["bom"])}))"
                    else -> error("Unsupported dependency: $dependency")
                }
            }
            .joinToString("\n")
    val bom = (yaml["dependencies"] as List<*>).filterIsInstance<Map<*, *>>().single()["bom"]
    File(bridge, "build.gradle.kts")
        .writeText(
            """
plugins {
    id("com.android.application") version "8.13.2"
    id("org.jetbrains.kotlin.android") version ${literal(kotlin["version"])}
    id("org.jetbrains.kotlin.plugin.compose") version ${literal(kotlin["version"])}
}
android {
    namespace = ${literal(android["namespace"])}
    compileSdk = ${android["compileSdk"]}
    buildToolsVersion = ${literal(android["buildToolsVersion"])}
    defaultConfig {
        applicationId = "app.stillnote.verification"
        minSdk = ${android["minSdk"]}
        targetSdk = ${android["targetSdk"]}
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    sourceSets {
        getByName("main") {
            manifest.srcFile(${path("app/src/AndroidManifest.xml")})
            java.setSrcDirs(listOf(${path("app/src")}))
            res.setSrcDirs(listOf(${path("app/res")}))
        }
        getByName("androidTest").java.setSrcDirs(listOf(${path("app/instrumentedTest")}))
    }
    buildFeatures { compose = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    lint { checkReleaseBuilds = false }
}
dependencies {
    implementation(files(${literal(mainJar.invariantSeparatorsPath)}, ${literal(coreJar.invariantSeparatorsPath)}, ${literal(presentationJar.invariantSeparatorsPath)}))
    $dependencies
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:${serialization["version"]}")
    androidTestImplementation(platform(${literal(bom)}))
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test:rules:1.7.0")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
// Sources remain visible to lint. All production bytecode comes from Kotlin CLI.
tasks.matching { it.name == "compileDebugKotlin" }.configureEach { enabled = false }
"""
                .trimIndent()
        )
    GradleConnector.newConnector()
        .useGradleVersion("8.13")
        .forProjectDirectory(bridge)
        .connect()
        .use { connection ->
            connection
                .newBuild()
                .setJavaHome(jdk)
                .setEnvironmentVariables(System.getenv())
                .forTasks(if (ui) "connectedDebugAndroidTest" else "lintDebug")
                .withArguments("--console=plain")
                .setStandardOutput(System.out)
                .setStandardError(System.err)
                .run()
        }
    if (ui) {
        val reports =
            File(bridge, "build/outputs/androidTest-results")
                .walkTopDown()
                .filter { it.name.startsWith("TEST-") && it.extension == "xml" }
                .toList()
        check(reports.isNotEmpty()) { "No instrumentation results were produced." }
        val factory = DocumentBuilderFactory.newInstance()
        factory.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true)
        var total = 0
        for (report in reports) {
            val suite = factory.newDocumentBuilder().parse(report).documentElement
            total += suite.getAttribute("tests").toInt()
            check(
                suite.getAttribute("failures").toInt() == 0 &&
                    suite.getAttribute("errors").toInt() == 0 &&
                    suite.getAttribute("skipped").toInt() == 0
            ) {
                "UI test failures or skips in $report"
            }
        }
        check(total >= 14) { "Expected at least the existing 14 UI tests; ran $total." }
        println("$total instrumentation tests passed using Kotlin CLI production jars.")
    }
    println("Android ${args[0]} verification completed. Reports: ${File(bridge, "build/reports")}")
}
