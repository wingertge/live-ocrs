// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{borrow::BorrowMut, env, ops::Deref, sync::Arc};

use device_query::{DeviceEvents as _, DeviceState};
use live_ocrs::{
    app::LiveOcr, capture::CaptureState, dict, toggle, update_hover, view::Definitions, OcrState,
};
use parking_lot::RwLock;
use rapidocr::{ExecutionProvider, RapidOCRBuilder};
use tauri::{
    async_runtime::{channel, spawn},
    AppHandle, GlobalShortcutManager, Manager,
};
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

fn main() {
    tracing_subscriber::fmt()
        .with_span_events(FmtSpan::CLOSE)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let state = {
        let ocr = RapidOCRBuilder::new()
            .max_side_len(2048)
            .det_model("../../models/ch_PP-OCRv4_det_infer/ch_PP-OCRv4_det_infer.onnx")
            .rec_model(
                "../../models/ch_PP-OCRv4_rec_infer/ch_PP-OCRv4_rec_infer.onnx",
                "../../models/ppocr_keys_v1.txt",
            )
            .with_execution_providers([ExecutionProvider::Cuda])
            .build()
            .unwrap();
        let dict_path = env::current_dir().unwrap().join("../../data/cedict.json");
        println!("Dict Path: {dict_path:?}");
        let state = LiveOcr {
            capture_state: Arc::new(CaptureState { ocr }),
            enabled: false,
            hovering: None,
            definitions: Definitions::new(dict::load(dict_path)),
            monitor: None,
            tooltip_window: None,
        };
        Arc::new(RwLock::new(state))
    };

    tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            let mut global_shortcuts = app.global_shortcut_manager();
            let handle = app.handle();
            global_shortcuts.register("alt+x", move || {
                let state = handle.state::<OcrState>().deref().clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let action = {
                        let mut state = state.write();
                        toggle(state.borrow_mut())
                    };

                    match action {
                        live_ocrs::Action::UpdateOcr(ocr_result) => {
                            log::info!("OCR Result: {ocr_result:?}");
                            state.write().definitions.ocr_strings = ocr_result;
                        }
                        live_ocrs::Action::CloseTooltip => {
                            // TODO
                        }
                        live_ocrs::Action::None => {}
                    }
                });
            })?;
            {
                let app = app.handle();
                let state = app.state::<OcrState>().deref().clone();
                spawn(track_cursor(state, app));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
    loop {
        let position = rx.recv().await.unwrap();
        if position != last_position {
            last_position = position;
            let update = {
                log::info!("Cursor Position changed: {position:?}");
                let mut state = state.write();
                update_hover(state.borrow_mut(), position)
            };

            if let Some(definitions) = update {
                app.emit_all("definitions-changed", definitions).unwrap();
            }
        }
    }
}
