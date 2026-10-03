plugins {
    id("com.android.application") version "8.13.2" apply false
    id("org.jetbrains.kotlin.jvm") version "2.2.21" apply false
    id("org.jetbrains.kotlin.android") version "2.2.21" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.21" apply false
    id("org.jetbrains.kotlin.plugin.serialization") version "2.2.21" apply false
    id("com.diffplug.spotless") version "7.2.1"
}

spotless {
    kotlin {
        target("app/src/**/*.kt", "core/src/**/*.kt")
        ktfmt("0.54").kotlinlangStyle()
    }
    kotlinGradle {
        target("*.gradle.kts", "app/*.gradle.kts", "core/*.gradle.kts")
        ktfmt("0.54").kotlinlangStyle()
    }
}
