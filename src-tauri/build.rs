fn main() {
    // Windows: load vulkan-1.dll (used by whisper.cpp's GPU backend) on first
    // use, not at startup. A PC whose graphics driver has no Vulkan would
    // otherwise refuse to start Dictator at all, with only a cryptic
    // missing-DLL dialog. stt::Transcriber::load checks for the DLL first and
    // shows a proper message instead.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        println!("cargo:rustc-link-arg=/DELAYLOAD:vulkan-1.dll");
        println!("cargo:rustc-link-lib=delayimp");
    }
    tauri_build::build()
}
