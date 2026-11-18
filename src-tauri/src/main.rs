// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("__NV_DISABLE_EXPLICIT_SYNC").is_none() {
        // NVIDIA + WebKitGTK on Wayland: with explicit sync on, the overlay either
        // crashes ("Missing acquire timeline") or repaints only partially.
        // Only NVIDIA's driver reads this variable, so it's harmless elsewhere.
        // SAFETY: first thing in main, before any other thread exists.
        unsafe { std::env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1") };
    }
    dictator_lib::run()
}
