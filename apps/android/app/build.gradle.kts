plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.aboutLibraries)
    id("com.grtsinry43.memedock.rust-bridge")
}

val licenseDefinitions = layout.buildDirectory.file("generated/aboutLibraries/export/aboutlibraries.json")

aboutLibraries {
    // Builds must not depend on network access; licenses come from the dependency metadata.
    offlineMode = true
    collect { configPath = file("aboutlibraries") }
    export {
        variant = "release"
        outputFile = licenseDefinitions
    }
}

abstract class LicenseResources : DefaultTask() {
    @get:InputFile abstract val definitions: RegularFileProperty
    @get:OutputDirectory abstract val resources: DirectoryProperty

    @TaskAction
    fun copy() {
        val raw = resources.dir("raw").get().asFile.apply { deleteRecursively(); mkdirs() }
        definitions.get().asFile.copyTo(raw.resolve("aboutlibraries.json"), overwrite = true)
    }
}

// The 13.x Android integration needs the AppExtension that AGP 9 removed.
val licenseResources = tasks.register<LicenseResources>("licenseResources") {
    dependsOn("exportLibraryDefinitions")
    definitions = licenseDefinitions
}

androidComponents {
    onVariants { variant -> variant.sources.res?.addGeneratedSourceDirectory(licenseResources, LicenseResources::resources) }
}

android {
    namespace = "com.grtsinry43.memedock"
    compileSdk {
        version = release(37)
    }

    defaultConfig {
        applicationId = "com.grtsinry43.memedock"
        minSdk = 28
        targetSdk = 37
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            optimization {
                enable = false
            }
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    buildFeatures {
        compose = true
    }
}

dependencies {
    // The Android artifact contains only vectors; use our pinned Compose dependencies.
    implementation(libs.material.symbols) { isTransitive = false }
    implementation(libs.androidx.datastore.preferences)
    implementation(project(":rustBridge"))
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.animation)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.navigation3)
    implementation(libs.androidx.navigation3.runtime)
    implementation(libs.androidx.navigation3.ui)
    implementation(libs.haze)
    implementation(libs.reorderable)
    implementation(libs.aboutlibraries.core)
    implementation(libs.coil.compose)
    implementation(libs.coil.gif)
    implementation(libs.kotlinx.coroutines.core)
    testImplementation(libs.junit)
    testImplementation(libs.kotlinx.coroutines.test)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(libs.androidx.junit)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
    debugImplementation(libs.androidx.compose.ui.tooling)
}
