import org.gradle.api.DefaultTask
import org.gradle.api.file.ConfigurableFileCollection
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.tasks.*
import org.gradle.process.ExecOperations
import org.gradle.work.DisableCachingByDefault
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.zip.ZipFile
import javax.inject.Inject

private fun verifyElf(bytes: ByteArray, name: String) {
    check(bytes.size >= 64 && bytes[0] == 0x7f.toByte() && String(bytes, 1, 3, Charsets.US_ASCII) == "ELF") { "Invalid ELF: $name" }
    check(bytes[4] == 2.toByte() && bytes[5] == 1.toByte()) { "Expected little-endian ELF64: $name" }
    val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
    val offset = buffer.getLong(32)
    val entrySize = buffer.getShort(54).toInt() and 0xffff
    val count = buffer.getShort(56).toInt() and 0xffff
    check(entrySize >= 56 && offset >= 0 && offset + entrySize.toLong() * count <= bytes.size) { "Invalid ELF program headers: $name" }
    var loads = 0
    repeat(count) { index ->
        val header = Math.toIntExact(offset + entrySize.toLong() * index)
        if (buffer.getInt(header) == 1) {
            loads++
            val alignment = buffer.getLong(header + 48)
            check(alignment >= 16384 && alignment and (alignment - 1) == 0L) { "ELF LOAD alignment below 16 KiB: $name" }
            check((buffer.getLong(header + 16) - buffer.getLong(header + 8)) % 16384 == 0L) { "ELF LOAD offset incompatible with 16 KiB: $name" }
        }
    }
    check(loads > 0) { "ELF has no LOAD segments: $name" }
}

@DisableCachingByDefault(because = "Verification has no output artifact")
abstract class VerifyNativeAlignmentTask : DefaultTask() {
    @get:InputFiles @get:PathSensitive(PathSensitivity.RELATIVE) abstract val libraries: ConfigurableFileCollection
    @TaskAction fun verify() {
        check(libraries.files.isNotEmpty()) { "No native libraries" }
        libraries.forEach { verifyElf(it.readBytes(), it.path) }
        logger.lifecycle("Verified 16 KiB ELF alignment for ${libraries.files.size} Rust libraries")
    }
}

@DisableCachingByDefault(because = "Verifies packaged Rust and third-party native libraries")
abstract class VerifyApkAlignmentTask : DefaultTask() {
    @get:Inject abstract val exec: ExecOperations
    @get:InputDirectory @get:PathSensitive(PathSensitivity.RELATIVE) abstract val apkDirectory: DirectoryProperty
    @get:InputFile @get:PathSensitive(PathSensitivity.NONE) abstract val zipalign: RegularFileProperty
    @get:Input abstract val abis: ListProperty<String>
    @TaskAction fun verify() {
        val apks = apkDirectory.get().asFile.listFiles()?.filter { it.extension == "apk" }.orEmpty()
        check(apks.isNotEmpty()) { "No APKs to verify" }
        for (apk in apks) {
            ZipFile(apk).use { zip ->
                val entries = zip.entries().asSequence().filter { it.name.startsWith("lib/") && it.name.endsWith(".so") }.toList()
                val packagedAbis = entries.map { it.name.split('/')[1] }.toSet()
                check(packagedAbis == abis.get().toSet()) { "Unexpected APK ABIs: $packagedAbis" }
                for (abi in abis.get()) {
                    check(entries.any { it.name == "lib/$abi/libmemedock_ffi.so" }) { "Rust library missing for $abi" }
                    check(entries.any { it.name == "lib/$abi/libjnidispatch.so" }) { "JNA native library missing for $abi" }
                }
                for (entry in entries) zip.getInputStream(entry).use { verifyElf(it.readBytes(), entry.name) }
            }
            exec.exec { commandLine(zipalign.get().asFile, "-c", "-P", "16", "4", apk) }.assertNormalExitValue()
        }
        logger.lifecycle("Verified Rust/JNA ELF and APK 16 KiB alignment")
    }
}
