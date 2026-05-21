#[cfg(feature = "cpp")]
fn main() {
    let mut build = cc::Build::new();
    build.cpp(true).flag("-std=c++11").file("src/esaxx.cpp").include("src");
    #[cfg(target_os = "macos")]
    build.flag("-stdlib=libc++");
    build.compile("esaxx");
}

#[cfg(not(feature = "cpp"))]
fn main() {}
