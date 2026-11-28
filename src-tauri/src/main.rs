mod commands;
mod state;

fn main() {
    let Some(platform) = ttops_core::platform::native() else {
        eprintln!("TTops supports macOS and Windows only");
        return;
    };
    tauri::Builder::default()
        .manage(state::AppState::new(platform))
        .invoke_handler(tauri::generate_handler![
            commands::get_volume_summary,
            commands::start_scan,
            commands::cancel_scan,
            commands::get_scan,
            commands::preview_cleanup,
            commands::run_cleanup
        ])
        .run(tauri::generate_context!())
        .expect("run TTops");
}
