import sys

path = sys.argv[1]
with open(path) as f:
    content = f.read()

old = 'let android_cc_dir = android_cc_path.parent().unwrap().join("..").join(format!("llvm-{tool}")).canonicalize()'
new = (
    'let android_cc_dir = android_cc_path.parent().unwrap_or(android_cc_path);\n'
    '            let tool_path = android_cc_dir.join(format!("llvm-{tool}"));\n'
    '            let resolved = if tool_path.exists() {\n'
    '                tool_path.canonicalize().unwrap_or(tool_path)\n'
    '            } else {\n'
    '                PathBuf::from(format!("llvm-{}", tool))\n'
    '            }'
)
content = content.replace(old, new)

old2 = 'configure.arg(format!("--{tool}={}", android_cc_dir.display()));'
new2 = 'configure.arg(format!("--{tool}={}", resolved.display()));'
content = content.replace(old2, new2)

with open(path, 'w') as f:
    f.write(content)
print('Patched ffmpeg-sys-next build.rs successfully')
