plugins { `kotlin-dsl` }
dependencies { implementation(libs.plugins.android.library.map { "com.android.tools.build:gradle:${it.version}" }) }
gradlePlugin {
    plugins {
        register("rustBridge") {
            id = "com.grtsinry43.memedock.rust-bridge"
            implementationClass = "MemeDockRustPlugin"
        }
    }
}
