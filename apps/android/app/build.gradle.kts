import java.io.File
import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

fun signingFromEnvOrFile(): Map<String, String>? {
    val keystore = System.getenv("DOCAPP_KEYSTORE")?.takeIf { it.isNotBlank() }
    val storePass = System.getenv("DOCAPP_KEYSTORE_PASSWORD")?.takeIf { it.isNotBlank() }
    val alias = System.getenv("DOCAPP_KEY_ALIAS")?.takeIf { it.isNotBlank() }
    val keyPass = System.getenv("DOCAPP_KEY_PASSWORD")?.takeIf { it.isNotBlank() }
    if (keystore != null && storePass != null && alias != null && keyPass != null) {
        return mapOf(
            "storeFile" to keystore,
            "storePassword" to storePass,
            "keyAlias" to alias,
            "keyPassword" to keyPass,
        )
    }
    val propsPath = System.getenv("DOCAPP_KEYSTORE_PROPS")?.takeIf { it.isNotBlank() }
        ?: rootProject.file("keystore.properties").takeIf { it.isFile }?.absolutePath
    if (propsPath == null) {
        return null
    }
    val propsFile = File(propsPath)
    if (!propsFile.isFile) {
        return null
    }
    val props = Properties()
    propsFile.inputStream().use { props.load(it) }
    val storeFile = props.getProperty("storeFile")?.takeIf { it.isNotBlank() }
    val storePassword = props.getProperty("storePassword")?.takeIf { it.isNotBlank() }
    val keyAlias = props.getProperty("keyAlias")?.takeIf { it.isNotBlank() }
    val keyPassword = props.getProperty("keyPassword")?.takeIf { it.isNotBlank() }
    if (storeFile == null || storePassword == null || keyAlias == null || keyPassword == null) {
        return null
    }
    return mapOf(
        "storeFile" to storeFile,
        "storePassword" to storePassword,
        "keyAlias" to keyAlias,
        "keyPassword" to keyPassword,
    )
}

val releaseSigning = signingFromEnvOrFile()

android {
    namespace = "com.waghitech.docapp"
    compileSdk = 35
    buildToolsVersion = "35.0.0"

    defaultConfig {
        applicationId = "com.waghitech.docapp"
        minSdk = 26
        targetSdk = 35
        versionCode = 4
        versionName = "0.1.3"
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    if (releaseSigning != null) {
        signingConfigs {
            create("release") {
                val path = releaseSigning["storeFile"]!!
                storeFile =
                    if (File(path).isAbsolute) {
                        file(path)
                    } else {
                        rootProject.file(path)
                    }
                storePassword = releaseSigning["storePassword"]
                keyAlias = releaseSigning["keyAlias"]
                keyPassword = releaseSigning["keyPassword"]
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            if (releaseSigning != null) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }

    splits {
        abi {
            isEnable = true
            reset()
            include("arm64-v8a", "x86_64")
            isUniversalApk = true
        }
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging {
        resources.excludes += setOf(
            "kotlin/**",
            "META-INF/*.version",
            "META-INF/**/*.kotlin_module",
            "DebugProbesKt.bin",
        )
    }

    lint {
        abortOnError = true
        warningsAsErrors = false
        lintConfig = file("lint.xml")
    }

    testOptions {
        unitTests.isReturnDefaultValues = true
    }

}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(composeBom)
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    implementation("net.java.dev.jna:jna:5.15.0@aar")
    testImplementation("junit:junit:4.13.2")
    testImplementation("androidx.test:core:1.6.1")
    testImplementation("org.robolectric:robolectric:4.14.1")
}