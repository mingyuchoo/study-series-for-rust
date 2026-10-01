fn main() {
    // libgit2 uses the POSIX spellings close/read/write. On Windows ARM64,
    // MSVC provides these CRT compatibility symbols in oldnames.lib.
    if std::env::var("TARGET").as_deref() == Ok("aarch64-pc-windows-msvc") {
        println!("cargo:rustc-link-lib=oldnames");
    }
}
