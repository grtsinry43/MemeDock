plugins { alias(libs.plugins.kotlin.jvm) }
val repository = rootProject.layout.projectDirectory.dir("../..")
java {
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
}
kotlin {
    compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_11) }
    sourceSets.main { kotlin.srcDir(project(":rustBridge").layout.buildDirectory.dir("generated/uniffi/host")) }
    sourceSets.main { kotlin.srcDir(project(":rustBridge").layout.projectDirectory.dir("src/main/java")) }
}
dependencies {
    implementation(libs.jna)
    implementation(libs.kotlinx.coroutines.core)
    testImplementation(libs.junit)
}
tasks.named("compileKotlin") { dependsOn(":rustBridge:generateHostBindings") }
tasks.test {
    dependsOn(":rustBridge:buildRustHost")
    // The Kotlin ABI can stay unchanged while native behavior changes.
    inputs.files(repository.file("target/debug/libmemedock_ffi.so"),
        repository.file("target/debug/memedock-ffi-fixtures"))
        .withPropertyName("nativeContractArtifacts")
        .withPathSensitivity(PathSensitivity.NONE)
    systemProperty("jna.library.path", repository.dir("target/debug").asFile.absolutePath)
    systemProperty("memedock.fixtureBinary", repository.file("target/debug/memedock-ffi-fixtures").asFile.absolutePath)
    jvmArgs("--enable-native-access=ALL-UNNAMED")
}
