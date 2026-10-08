import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.*
import org.gradle.process.ExecOperations
import org.gradle.work.DisableCachingByDefault
import java.io.File
import javax.inject.Inject

@DisableCachingByDefault(because = "Cargo owns its incremental cache; native outputs are host-specific")
abstract class RustHostBuildTask : DefaultTask() {
    @get:Inject abstract val exec: ExecOperations
    @get:Internal abstract val repository: DirectoryProperty
    @get:InputFiles @get:PathSensitive(PathSensitivity.RELATIVE) abstract val sources: ConfigurableFileCollection
    @get:Input abstract val host: Property<String>
    @get:OutputFiles abstract val artifacts: ConfigurableFileCollection
    @TaskAction fun build() {
        exec.exec {
            workingDir(repository)
            commandLine("cargo", "build", "--locked", "-p", "memedock-ffi", "-p", "memedock-bindgen", "-p", "memedock-ffi-fixtures")
        }.assertNormalExitValue()
    }
}

@DisableCachingByDefault(because = "Installs a versioned local build tool")
abstract class CargoNdkToolTask : DefaultTask() {
    @get:Inject abstract val exec: ExecOperations
    @get:Input abstract val version: Property<String>
    @get:Internal abstract val repository: DirectoryProperty
    @get:OutputDirectory abstract val installation: DirectoryProperty
    @TaskAction fun install() {
        val binary = installation.file("bin/cargo-ndk").get().asFile
        if (!binary.isFile) {
            exec.exec {
                workingDir(repository)
                commandLine("cargo", "install", "cargo-ndk", "--version", version.get(), "--locked", "--root", installation.get().asFile)
            }.assertNormalExitValue()
        }
        val output = java.io.ByteArrayOutputStream()
        exec.exec { environment("CARGO", "cargo"); commandLine(binary, "ndk", "--version"); standardOutput = output }.assertNormalExitValue()
        check(output.toString().trim() == "cargo-ndk ${version.get()}") { "Unexpected cargo-ndk version: $output" }
    }
}

@DisableCachingByDefault(because = "Cargo owns its incremental cache; uses the installed NDK")
abstract class RustAndroidBuildTask : DefaultTask() {
    @get:Inject abstract val exec: ExecOperations
    @get:Internal abstract val repository: DirectoryProperty
    @get:InputFiles @get:PathSensitive(PathSensitivity.RELATIVE) abstract val sources: ConfigurableFileCollection
    @get:InputFile @get:PathSensitive(PathSensitivity.NONE) abstract val cargoNdk: RegularFileProperty
    @get:Internal abstract val sdk: DirectoryProperty
    @get:Input abstract val ndkVersion: Property<String>
    @get:Input abstract val abis: ListProperty<String>
    @get:Input abstract val release: Property<Boolean>
    @get:OutputDirectory abstract val outputDirectory: DirectoryProperty
    @TaskAction fun build() {
        val ndk = sdk.dir("ndk/${ndkVersion.get()}").get().asFile
        check(ndk.resolve("source.properties").isFile) { "Install NDK ${ndkVersion.get()} with SDK Manager" }
        val targets = abis.get().map {
            when (it) { "arm64-v8a" -> "aarch64-linux-android"; "x86_64" -> "x86_64-linux-android"; else -> error("Unsupported ABI: $it") }
        }
        exec.exec {
            workingDir(repository)
            commandLine(listOf("rustup", "target", "add", "--toolchain", "1.94.1") + targets)
        }.assertNormalExitValue()
        // Remove outputs for deselected ABIs so stale libraries cannot enter APKs.
        outputDirectory.get().asFile.listFiles()?.filter { it.name !in abis.get() }?.forEach { check(it.deleteRecursively()) }
        val args = mutableListOf(cargoNdk.get().asFile.absolutePath, "ndk")
        for (abi in abis.get()) args.addAll(listOf("--target", abi))
        args.addAll(listOf("--platform", "28", "--output-dir", outputDirectory.get().asFile.absolutePath, "build", "--locked", "-p", "memedock-ffi"))
        if (release.get()) args.add("--release")
        exec.exec {
            workingDir(repository)
            environment("ANDROID_NDK_HOME", ndk.absolutePath)
            environment("CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS", "-C link-arg=-Wl,-z,max-page-size=16384")
            environment("CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS", "-C link-arg=-Wl,-z,max-page-size=16384")
            environment("CARGO", "cargo")
            commandLine(args)
        }.assertNormalExitValue()
    }
}

@DisableCachingByDefault(because = "Validates generator output against every selected ABI")
abstract class GenerateBindingsTask : DefaultTask() {
    @get:Inject abstract val exec: ExecOperations
    @get:Internal abstract val repository: DirectoryProperty
    @get:InputFile @get:PathSensitive(PathSensitivity.NONE) abstract val generator: RegularFileProperty
    @get:InputFile @get:PathSensitive(PathSensitivity.RELATIVE) abstract val configuration: RegularFileProperty
    @get:InputFiles @get:PathSensitive(PathSensitivity.RELATIVE) abstract val libraries: ConfigurableFileCollection
    @get:OutputDirectory abstract val outputDirectory: DirectoryProperty
    @TaskAction fun generate() {
        val binaries = libraries.files.sortedBy { it.absolutePath }
        check(binaries.isNotEmpty()) { "No native libraries to generate bindings from" }
        val output = outputDirectory.get().asFile
        check(output.deleteRecursively())
        var reference: Map<String, List<Byte>>? = null
        binaries.forEachIndexed { index, library ->
            val destination = if (index == 0) output else temporaryDir.resolve("abi-$index")
            check(destination.deleteRecursively())
            exec.exec {
                workingDir(repository)
                commandLine(generator.get().asFile, "generate", library, "--language", "kotlin", "--config", configuration.get().asFile, "--out-dir", destination, "--no-format")
            }.assertNormalExitValue()
            val contents = destination.walkTopDown().filter { it.isFile }.associate { it.relativeTo(destination).invariantSeparatorsPath to it.readBytes().toList() }
            check(contents.isNotEmpty()) { "Generator produced no Kotlin sources" }
            if (reference == null) reference = contents else check(reference == contents) { "UniFFI interface differs between selected ABIs: $library" }
        }
    }
}
