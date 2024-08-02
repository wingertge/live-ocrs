// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{borrow::BorrowMut, env, fs, sync::Arc};

use device_query::{DeviceEvents as _, DeviceState};
use live_ocrs::{
    app::LiveOcr, capture::CaptureState, dict, toggle, update_hover, view::Definitions, OcrState,
};
use parking_lot::RwLock;
use rapidocr::{ExecutionProvider, RapidOCRBuilder};
use tauri::{
    async_runtime::{channel, spawn},
    App, AppHandle, GlobalShortcutManager, Manager, PhysicalPosition, WindowBuilder, WindowUrl,
};
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

fn main() {
    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::CLOSE)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .setup(|app| {
            let state = init_state(app);
            app.manage(state.clone());

            let mut global_shortcuts = app.global_shortcut_manager();
            {
                let handle = app.handle();
                let state = state.clone();
                global_shortcuts.register("alt+x", move || {
                    handle_toggle(handle.clone(), state.clone());
                })?;
            }

            {
                let app = app.handle();
                let state = state.clone();
                spawn(track_cursor(state, app));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_toggle(handle: AppHandle, state: OcrState) {
    tauri::async_runtime::spawn_blocking(move || {
        let action = {
            let mut state = state.write();
            toggle(state.borrow_mut())
        };

        match action {
            live_ocrs::Action::UpdateOcr => {
                let definitions = state.read().definitions.definitions.clone();
                let window =
                    WindowBuilder::new(&handle, "tooltip", WindowUrl::App("index.html".into()))
                        .always_on_top(true)
                        .decorations(false)
                        .focused(false)
                        .visible(false)
                        .build()
                        .unwrap();
                window.set_ignore_cursor_events(true).unwrap();
                handle.emit_all("definitions-changed", definitions).unwrap();
            }
            live_ocrs::Action::CloseTooltip => {
                if let Some(window) = handle.get_window("tooltip") {
                    window.close().unwrap();
                }
            }
            live_ocrs::Action::None => {}
        }
    });
}

fn init_state(app: &App) -> OcrState {
    let cache_dir = app
        .path_resolver()
        .app_cache_dir()
        .unwrap_or_else(|| ".cache".into());
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir).unwrap();
    }
    let ocr = RapidOCRBuilder::new()
        .max_side_len(2048)
        .det_model("../../models/ch_PP-OCRv4_det_infer/ch_PP-OCRv4_det_infer.onnx")
        .rec_model(
            "../../models/ch_PP-OCRv4_rec_infer/ch_PP-OCRv4_rec_infer.onnx",
            "../../models/ppocr_keys_v1.txt",
        )
        .with_execution_providers([ExecutionProvider::TensorRT])
        .with_engine_cache_path(&cache_dir)
        .build()
        .unwrap();
    let dict_path = env::current_dir().unwrap().join("../../data/cedict.json");
    println!("Dict Path: {dict_path:?}");
    let state = LiveOcr {
        capture_state: Arc::new(CaptureState { ocr }),
        enabled: false,
        hovering: None,
        definitions: Definitions::new(dict::load(dict_path, &cache_dir)),
        monitor: None,
        tooltip_window: None,
    };
    Arc::new(RwLock::new(state))
}

async fn track_cursor(state: OcrState, app: AppHandle) {
    let (tx, mut rx) = channel(5);
    let device_state = DeviceState::new();
    let _guard = {
        let state = state.clone();
        device_state.on_mouse_move(move |position| {
            let enabled = {
                let state = state.read();
                state.enabled && !state.definitions.ocr_strings.is_empty()
            };
            if enabled {
                tx.blocking_send(*position).unwrap();
            }
        })
    };

    let mut last_position = (0, 0);
    let mut visible = false;
    loop {
        let position = rx.recv().await.unwrap();
        if position != last_position {
            last_position = position;

            let update = {
                let mut state = state.write();
                update_hover(state.borrow_mut(), position)
            };

            if let Some((rect, definitions)) = update {
                let tooltip = app.get_window("tooltip");
                let new_visible = !definitions.is_empty();
                if let Some(tooltip) = &tooltip {
                    if new_visible && !visible {
                        tooltip.show().unwrap();
                    } else if !new_visible && visible {
                        tooltip.hide().unwrap();
                    }
                    if let Some(rect) = rect {
                        let position =
                            PhysicalPosition::new(rect.min().x as i32, rect.max().y as i32);
                        tooltip.set_position(position).unwrap()
                    }
                }
                visible = new_visible;

                app.emit_all("definitions-changed", definitions).unwrap();
            }
        }
    }
}
