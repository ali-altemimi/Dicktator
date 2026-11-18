mod overlay;

use serde::Serialize;
use tauri::Emitter;

/// What the overlay shows. Sent to the frontend as the `state` event.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub status: &'static str, // "idle" | "recording" | "transcribing"
    pub text: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            overlay::create(app.handle())?;

            // Spike: fake some dictation so the overlay has something to show.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let words = "The results suggest a strong correlation between sleep \
                             quality and memory consolidation in older adults."
                    .split(' ');
                let mut text = String::new();
                for (i, w) in words.cycle().enumerate() {
                    let status = ["recording", "transcribing", "idle"][(i / 6) % 3];
                    text.push_str(w);
                    text.push(' ');
                    let _ = handle.emit("state", State { status, text: text.clone() });
                    std::thread::sleep(std::time::Duration::from_millis(400));
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
