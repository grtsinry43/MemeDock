plugins { alias(libs.plugins.android.application) }

android {
    namespace = "com.grtsinry43.memedock.sharetest"
    compileSdk = 37
    defaultConfig {
        applicationId = "com.grtsinry43.memedock.sharetest"
        minSdk = 28
        targetSdk = 37
        versionCode = 1
        versionName = "test"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
}
androidComponents { beforeVariants(selector().withBuildType("release")) { it.enable = false } }
dependencies { implementation(libs.androidx.core.ktx) }
