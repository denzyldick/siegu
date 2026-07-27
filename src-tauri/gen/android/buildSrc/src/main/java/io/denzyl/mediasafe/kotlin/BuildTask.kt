import java.io.File
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.TaskAction

open class BuildTask : DefaultTask() {
    @Input
    var rootDirRel: String? = null
    @Input
    var target: String? = null
    @Input
    var release: Boolean? = null

    @TaskAction
    fun assemble() {
        val rootDirRel = rootDirRel ?: throw GradleException("rootDirRel cannot be null")
        val target = target ?: throw GradleException("target cannot be null")
        val release = release ?: throw GradleException("release cannot be null")

        val rootDir = File(project.projectDir, rootDirRel)
        val profile = if (release) "release" else "debug"
        val ndkTarget = "$target-linux-android"
        val abi = when (target) {
            "aarch64" -> "arm64-v8a"
            "x86_64" -> "x86_64"
            else -> throw GradleException("Unsupported target: $target")
        }

        logger.lifecycle("Building Rust lib for $target ($abi) in $profile mode")

        project.exec {
            workingDir(rootDir)
            executable("cargo")
            args(
                mutableListOf(
                    "ndk", "-t", ndkTarget, "-P", "24",
                    "build", "--features", "tauri/custom-protocol tauri/custom-protocol",
                    "--lib"
                ).apply {
                    if (release) add("--release")
                }
            )
        }.assertNormalExitValue()

        val soFile = rootDir.resolve("target/$ndkTarget/$profile/libsiegu_lib.so")
        val jniLibsDir = project.file("app/src/main/jniLibs/$abi")

        if (soFile.exists()) {
            jniLibsDir.mkdirs()
            soFile.copyTo(File(jniLibsDir, "libsiegu_lib.so"), overwrite = true)
            logger.lifecycle("Copied ${soFile.name} to ${jniLibsDir.absolutePath}")
        } else {
            throw GradleException("Built .so not found at: ${soFile.absolutePath}")
        }
    }
}
