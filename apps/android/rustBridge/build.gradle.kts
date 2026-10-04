plugins {
    alias(libs.plugins.android.library)
    id("com.grtsinry43.memedock.rust-bridge")
}
android {
    namespace = "com.grtsinry43.memedock.bridge"
    compileSdk = 37
    defaultConfig {
        minSdk = 28
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        consumerProguardFiles("src/main/keepRules/bridge.keep")
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
}
dependencies {
    compileOnly(libs.androidx.annotation)
    api("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")
    api(libs.kotlinx.coroutines.core)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
}
