import sys


def replace(content: str, old: str, new: str, label: str) -> str:
    if old not in content:
        raise SystemExit(
            f"ERROR: could not find expected code in ffmpeg-sys-next build.rs ({label}). "
            "The dependency version may have changed; update this patch script."
        )
    return content.replace(old, new)


path = sys.argv[1]
with open(path) as f:
    content = f.read()

if "android_cc_dir" in content:
    print("ffmpeg-sys-next build.rs already patched, skipping")
    sys.exit(0)

# ffmpeg-sys-next 8.1.0 / 9.0.0 resolves llvm-nm / llvm-strip relative to the CC path
# using `android_cc_path.join("..")`, which appends `..` to the clang FILE path
# instead of its parent directory. `canonicalize()` then fails and the build
# panics with "failed to resolve a path to android nm". Resolve tools next to
# clang instead, falling back to PATH.
old = """        for tool in ["nm", "strip"] {
            configure.arg(format!(
                "--{tool}={}",
                android_cc_path
                    .join("..")
                    .join(format!("llvm-{tool}"))
                    .canonicalize()
                    .unwrap_or_else(|_| panic!("failed to resolve a path to android {}", tool))
                    .display()
            ));
        }"""
new = """        let android_cc_dir = android_cc_path.parent().unwrap_or(android_cc_path);
        for tool in ["nm", "strip"] {
            let tool_path = android_cc_dir.join(format!("llvm-{tool}"));
            let resolved = if tool_path.exists() {
                tool_path.canonicalize().unwrap_or(tool_path)
            } else {
                PathBuf::from(format!("llvm-{}", tool))
            };
            configure.arg(format!("--{tool}={}", resolved.display()));
        }"""
content = replace(content, old, new, "android nm/strip tool resolution")

with open(path, "w") as f:
    f.write(content)
print("Patched ffmpeg-sys-next build.rs successfully")
