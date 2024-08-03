// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    borrow::BorrowMut,
    env,
    error::Error,
    fs::{self, File},
    io,
    sync::{Arc, Mutex},
};

use device_query::{DeviceEvents as _, DeviceState};
use live_ocrs::{
    capture::CaptureState, dict, toggle, update_hover, Definitions, LiveOcr, OcrState,
};
use parking_lot::RwLock;
use rapidocr::{ExecutionProvider, RapidOCRBuilder};
use serde::{Deserialize, Serialize};
use tauri::{
    async_runtime::{channel, spawn, spawn_blocking},
    AppHandle, GlobalShortcutManager, LogicalSize, Manager, PhysicalPosition, State, Window,
    WindowBuilder, WindowUrl,
};
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt as _,
    EnvFilter,
};

fn main() {
    #[cfg(windows)]
    {
        use windows::Win32::System::Console::AllocConsole;
        let _ = unsafe { AllocConsole() };
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![content_size_changed])
        .setup(|app| {
            let log_dir = app.path_resolver().app_log_dir().unwrap();
            log::info!("Log Dir: {log_dir:?}");
            if !log_dir.exists() {
                fs::create_dir_all(&log_dir).unwrap();
            }
            let log_file = log_dir.join("log.txt");

            let subscriber = tracing_subscriber::fmt()
                .with_span_events(FmtSpan::CLOSE)
                .with_env_filter(EnvFilter::from_default_env())
                .finish()
                .with(
                    fmt::Layer::default().with_writer(Mutex::new(File::create(log_file).unwrap())),
                )
                .with(fmt::Layer::default().with_writer(io::stdout));

            tracing::subscriber::set_global_default(subscriber).unwrap();

            let app = app.handle();
            spawn_blocking(move || {
                let state = init_state(app.clone());
                if let Err(err) = &state {
                    log::error!("{err}");
                    app.exit(-1);
                }
                let state = state.unwrap();
                app.manage(state.clone());

                if let Some(splash) = app.get_window("splashscreen") {
                    splash.close().unwrap();
                }
                if let Some(main) = app.get_window("main") {
                    main.show().unwrap();
                }

                let mut global_shortcuts = app.global_shortcut_manager();
                {
                    let handle = app.clone();
                    let state = state.clone();
                    global_shortcuts
                        .register("alt+x", move || {
                            handle_toggle(handle.clone(), state.clone());
                        })
                        .unwrap();
                }

                {
                    let app = app.clone();
                    let state = state.clone();
                    spawn(track_cursor(state, app));
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
struct Rect {
    width: f32,
    height: f32,
}

#[tauri::command]
async fn content_size_changed(
    window: Window,
    state: State<'_, OcrState>,
    width: f32,
    height: f32,
) -> tauri::Result<()> {
    let state = state.read();
    if window.label() != "tooltip" {
        return Ok(());
    }
    if state.definitions.definitions.is_empty() {
        window.hide()?;
        return Ok(());
    }
    let width = width.ceil();
    let height = height.ceil();
    if let Some(((_, _, rect), monitor)) = state.hovering.as_ref().zip(state.monitor.as_ref()) {
        window.set_size(LogicalSize::new(width, height))?;
        let actual_size = window.inner_size()?;
        log::info!("Virtual size: ({width}, {height}), actual size: {actual_size:?}");
        let width = actual_size.width as f32;
        let height = actual_size.height as f32;
        let align_left = rect.min().x + width > monitor.x() as f32 + monitor.width() as f32;
        let align_top = rect.max().y + height > monitor.y() as f32 + monitor.height() as f32;
        let x = if align_left {
            rect.max().x - width as f32
        } else {
            rect.min().x
        };
        let y = if align_top {
            rect.min().y - height as f32
        } else {
            rect.max().y
        };
        window.set_position(PhysicalPosition::new(x, y))?;
        window.show()?;
    } else {
        window.hide()?;
    }

    Ok(())
}

fn handle_toggle(handle: AppHandle, state: OcrState) {
    spawn_blocking(move || {
        let ui_state = if state.read().enabled {
            "disabled"
        } else {
            "detecting"
        };
        handle.emit_to("main", "state-changed", ui_state).unwrap();
        let action = {
            let mut state = state.write();
            toggle(state.borrow_mut())
        };

        match action {
            live_ocrs::Action::UpdateOcr => {
                let strings: Vec<String> = state
                    .read()
                    .definitions
                    .ocr_strings
                    .iter()
                    .map(|it| it.0.clone())
                    .collect();
                handle.emit_to("main", "ocr-changed", strings).unwrap();
                let definitions = state.read().definitions.definitions.clone();
                let window =
                    WindowBuilder::new(&handle, "tooltip", WindowUrl::App("tooltip.html".into()))
                        .always_on_top(true)
                        .decorations(false)
                        .focused(false)
                        .visible(false)
                        .build()
                        .unwrap();
                window.set_ignore_cursor_events(true).unwrap();
                handle
                    .emit_to("tooltip", "definitions-changed", definitions)
                    .unwrap();
                handle.emit_to("main", "state-changed", "enabled").unwrap();
            }
            live_ocrs::Action::CloseTooltip => {
                handle
                    .emit_to("main", "ocr-changed", Vec::<String>::new())
                    .unwrap();
                if let Some(window) = handle.get_window("tooltip") {
                    window.close().unwrap();
                }
            }
            live_ocrs::Action::None => {}
        }
    });
}

fn init_state(app: AppHandle) -> Result<OcrState, Box<dyn Error>> {
    let paths = app.path_resolver();
    let cache_dir = paths.app_cache_dir().unwrap_or_else(|| ".cache".into());
    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir).unwrap();
    }
    let ocr = RapidOCRBuilder::new()
        .max_side_len(2048)
        .det_model(
            paths
                .resolve_resource("models/ch_PP-OCRv4_det_infer/ch_PP-OCRv4_det_infer.onnx")
                .ok_or("Det Model not found")?,
        )
        .rec_model(
            paths
                .resolve_resource("models/ch_PP-OCRv4_rec_infer/ch_PP-OCRv4_rec_infer.onnx")
                .ok_or("Rec Model not found")?,
            paths
                .resolve_resource("models/ppocr_keys_v1.txt")
                .ok_or("Keys not found")?,
        )
        .with_execution_providers([ExecutionProvider::TensorRT])
        .with_engine_cache_path(&cache_dir)
        .build()?;
    let dict_path = paths.resolve_resource("data/cedict.json").unwrap();
    println!("Dict Path: {dict_path:?}");
    let state = LiveOcr {
        capture_state: Arc::new(CaptureState { ocr }),
        enabled: false,
        hovering: None,
        definitions: Definitions::new(dict::load(dict_path, cache_dir.join("dict"))),
        monitor: None,
    };
    Ok(Arc::new(RwLock::new(state)))
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
                let mut state = state.write();
                update_hover(state.borrow_mut(), position)
            };

            if let Some((_, definitions)) = update {
                let tooltip = app.get_window("tooltip");
                if let Some(tooltip) = &tooltip {
                    tooltip.hide().unwrap();
                    /*                     if let Some(rect) = rect {
                        let position =
                            PhysicalPosition::new(rect.min().x as i32, rect.max().y as i32);
                        tooltip.set_position(position).unwrap()
                    } */
                }

                app.emit_to("tooltip", "definitions-changed", definitions)
                    .unwrap();
            }
        }
    }
}
