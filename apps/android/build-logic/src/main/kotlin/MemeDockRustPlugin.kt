import com.android.build.api.artifact.SingleArtifact
import com.android.build.api.dsl.ApplicationExtension
import com.android.build.api.dsl.LibraryExtension
import com.android.build.api.variant.AndroidComponentsExtension
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.artifacts.VersionCatalogsExtension

class MemeDockRustPlugin : Plugin<Project> {
    override fun apply(project: Project) = with(project) {
        val versions = extensions.getByType(VersionCatalogsExtension::class.java).named("libs")
        val ndkVersion = versions.findVersion("ndk").get().requiredVersion
        val cargoNdkVersion = versions.findVersion("cargoNdk").get().requiredVersion
        val root = rootProject.layout.projectDirectory.dir("../..")
        val sourceInputs = fileTree(root) {
            include("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/**", "crates/**/*.rs", "crates/**/Cargo.toml", "crates/ffi/*.toml", "tools/**/*.rs", "tools/**/Cargo.toml")
            exclude("**/target/**")
        }
        fun abis(release: Boolean): List<String> {
            val selected = providers.gradleProperty("memedock.abis").orNull?.split(',')?.map { it.trim() }
                ?: if (release) listOf("arm64-v8a") else listOf("arm64-v8a", "x86_64")
            require(selected.isNotEmpty() && selected.distinct().size == selected.size && selected.all { it in listOf("arm64-v8a", "x86_64") }) { "memedock.abis must contain arm64-v8a and/or x86_64" }
            return selected
        }
        plugins.withId("com.android.library") {
            val android = extensions.getByType(LibraryExtension::class.java)
            android.ndkVersion = ndkVersion
            val components = extensions.getByType(AndroidComponentsExtension::class.java)
            val host = tasks.register("buildRustHost", RustHostBuildTask::class.java) {
                repository.set(root); sources.from(sourceInputs)
                this.host.set(providers.systemProperty("os.name").zip(providers.systemProperty("os.arch")) { a, b -> "$a/$b" })
                artifacts.from(root.file("target/debug/libmemedock_ffi.so"), root.file("target/debug/memedock-bindgen"), root.file("target/debug/memedock-ffi-fixtures"))
            }
            tasks.register("generateHostBindings", GenerateBindingsTask::class.java) {
                dependsOn(host); repository.set(root)
                generator.set(root.file("target/debug/memedock-bindgen")); configuration.set(root.file("crates/ffi/uniffi-host.toml"))
                libraries.from(root.file("target/debug/libmemedock_ffi.so"))
                outputDirectory.set(layout.buildDirectory.dir("generated/uniffi/host"))
            }
            val tool = tasks.register("prepareCargoNdk", CargoNdkToolTask::class.java) {
                repository.set(root); version.set(cargoNdkVersion)
                installation.set(root.dir("target/tools/cargo-ndk-$cargoNdkVersion"))
            }
            components.onVariants { variant ->
                val release = variant.buildType == "release"
                val selected = abis(release)
                val suffix = variant.name.replaceFirstChar { it.uppercaseChar() }
                val native = tasks.register("buildRust$suffix", RustAndroidBuildTask::class.java) {
                    dependsOn(tool); repository.set(root); sources.from(sourceInputs)
                    cargoNdk.set(tool.flatMap { it.installation.file("bin/cargo-ndk") })
                    sdk.set(components.sdkComponents.sdkDirectory); this.ndkVersion.set(ndkVersion)
                    this.abis.set(selected); this.release.set(release)
                    outputDirectory.set(layout.buildDirectory.dir("generated/jniLibs/${variant.name}"))
                }
                val generated = tasks.register("generate${suffix}Bindings", GenerateBindingsTask::class.java) {
                    dependsOn(native, host); repository.set(root)
                    generator.set(root.file("target/debug/memedock-bindgen")); configuration.set(root.file("crates/ffi/uniffi.toml"))
                    libraries.from(native.flatMap { task -> task.outputDirectory.map { output -> selected.map { output.file("$it/libmemedock_ffi.so").asFile } } })
                    outputDirectory.set(layout.buildDirectory.dir("generated/uniffi/${variant.name}"))
                }
                variant.sources.kotlin?.addGeneratedSourceDirectory(generated) { it.outputDirectory }
                variant.sources.jniLibs?.addGeneratedSourceDirectory(native) { it.outputDirectory }
                tasks.register("verify${suffix}NativeAlignment", VerifyNativeAlignmentTask::class.java) {
                    libraries.from(generated.map { it.libraries }); dependsOn(generated)
                }
            }
        }
        plugins.withId("com.android.application") {
            val android = extensions.getByType(ApplicationExtension::class.java)
            android.ndkVersion = ndkVersion
            android.buildTypes.configureEach { ndk.abiFilters.addAll(abis(name == "release")) }
            val components = extensions.getByType(AndroidComponentsExtension::class.java)
            val buildToolsVersion = android.buildToolsVersion
            components.onVariants { variant ->
                val suffix = variant.name.replaceFirstChar { it.uppercaseChar() }
                val verify = tasks.register("verify${suffix}ApkAlignment", VerifyApkAlignmentTask::class.java) {
                    this.abis.set(abis(variant.buildType == "release"))
                    apkDirectory.set(variant.artifacts.get(SingleArtifact.APK))
                    zipalign.set(components.sdkComponents.sdkDirectory.map { it.file("build-tools/$buildToolsVersion/zipalign") })
                }
                tasks.register("verify${suffix}Bridge") {
                    group = "verification"
                    dependsOn(verify, ":rustBridge:verify${suffix}NativeAlignment", ":ffiContractTests:test")
                }
            }
        }
    }
}
