use std::{any::TypeId, env, sync::Arc};

use device_query::{DeviceEvents as _, DeviceQuery, DeviceState, Keycode, MouseState};
use iced::{
    futures::SinkExt,
    multi_window::Application,
    subscription,
    widget::{text, Column},
    window::{self, settings::PlatformSpecific, Position, Settings},
    Command, Element, Point, Size, Subscription, Theme,
};
use rapidocr::RapidOCRBuilder;
use tokio::{sync::mpsc, task::spawn_blocking};
use xcap::Monitor;

use crate::{
    capture::CaptureState, character::Block, dict, find_closest_char, longest_meaningful_string,
    native::get_scale_factor, view::Definitions,
};

pub struct LiveOcr {
    pub capture_state: Arc<CaptureState>,
    pub enabled: bool,
    pub definitions: Definitions,
    pub hovering: Option<(String, usize)>,
    pub monitor: Option<Monitor>,
    pub tooltip_window: Option<(window::Id, f32)>,
}

#[derive(Debug, Clone)]
pub enum OcrMessage {
    OcrChanged(Vec<Block>),
    CursorMoved(iced::Point),
    Toggled,
}

impl Application for LiveOcr {
    fn view(&self, id: window::Id) -> Element<OcrMessage> {
        if self
            .tooltip_window
            .map(|(tooltip_id, _)| tooltip_id == id)
            .unwrap_or_default()
        {
            Column::with_children(vec![text("Hello").size(40).into()]).into()
        } else {
            Column::with_children(if self.enabled {
                self.definitions.view()
            } else {
                vec![text("Disabled").size(24).into()]
            })
            .padding(32)
            .into()
        }
    }

    fn update(&mut self, message: OcrMessage) -> Command<OcrMessage> {
        match message {
            OcrMessage::OcrChanged(ocr_result) => {
                self.definitions.ocr_strings = ocr_result;
                let monitor = self
                    .monitor
                    .clone()
                    .unwrap_or_else(|| Monitor::all().unwrap().first().unwrap().clone());
                let dpi_scale = get_scale_factor(&monitor);
                let primary_scale = get_scale_factor(
                    Monitor::all()
                        .unwrap()
                        .iter()
                        .find(|it| it.is_primary())
                        .unwrap(),
                );
                log::info!(
                    "DPI Scale: {dpi_scale}, {}x{}",
                    monitor.width(),
                    monitor.height()
                );
                let (id, command) = iced::window::spawn(Settings {
                    size: Size::new(
                        monitor.width() as f32 / primary_scale,
                        monitor.height() as f32 / primary_scale,
                    ),
                    position: Position::Specific(Point::new(
                        monitor.x() as f32 / primary_scale,
                        monitor.y() as f32 / primary_scale,
                    )),
                    resizable: false,
                    decorations: false,
                    transparent: true,
                    platform_specific: PlatformSpecific {
                        drag_and_drop: false,
                        skip_taskbar: true,
                        parent: None,
                    },

                    ..Default::default()
                });
                self.tooltip_window = Some((id, dpi_scale));
                command
            }
            OcrMessage::Toggled => {
                log::info!("Toggled");
                self.enabled = !self.enabled;
                if self.enabled {
                    self.definitions.ocr_strings.clear();
                    let device_state = DeviceState::new();
                    let MouseState {
                        coords: (cursor_x, cursor_y),
                        ..
                    } = device_state.get_mouse();
                    let monitor = Monitor::from_point(cursor_x, cursor_y).unwrap();
                    self.monitor = Some(monitor.clone());
                    let capture = self.capture_state.clone();
                    Command::perform(
                        spawn_blocking(move || capture.capture(monitor)),
                        |ocr_state| OcrMessage::OcrChanged(ocr_state.unwrap()),
                    )
                } else {
                    let command = if let Some((window_id, _)) = self.tooltip_window.take() {
                        iced::window::close(window_id)
                    } else {
                        Command::none()
                    };

                    self.hovering = None;
                    self.monitor = None;
                    self.definitions.definitions.clear();
                    command
                }
            }
            OcrMessage::CursorMoved(position) => {
                if self.enabled && !self.definitions.ocr_strings.is_empty() {
                    let point = geo::point!(x: position.x, y: position.y);
                    let (closest_string, closest_char, closest_distance, _) =
                        find_closest_char(&self.definitions.ocr_strings, point);
                    if closest_distance < 5.0 {
                        if let Some((prev_str, prev_char)) = &self.hovering {
                            if &closest_string == prev_str && closest_char == *prev_char {
                                return Command::none();
                            }
                        }
                        self.hovering = Some((closest_string.to_owned(), closest_char));
                        let longest_string =
                            longest_meaningful_string(&closest_string, closest_char);
                        self.definitions.update(&longest_string);
                    }
                }
                Command::none()
            }
        }
    }

    type Message = OcrMessage;

    fn new(_flags: ()) -> (Self, Command<OcrMessage>) {
        let ocr = RapidOCRBuilder::new().max_side_len(2048).build().unwrap();
        let dict_path = env::current_dir().unwrap().join("data/cedict.json");
        log::info!("Dict Path: {dict_path:?}");
        /*         let image = image::open("Screenshot_5.png").unwrap();
               image.to_luma8().save("screen_gray.png").unwrap();
               let boxes = do_ocr(&ocr, &image);
               let mut image = image.to_rgb8();
               for (_, contour) in boxes.iter().flat_map(|it| &it.1) {
                   draw_outline_geo(&mut image, *contour, Rgb([255, 0, 0]))
               }
               image.save("boundaries.png").unwrap();
        */
        (
            Self {
                capture_state: Arc::new(CaptureState { ocr }),
                enabled: false,
                hovering: None,
                definitions: Definitions::new(dict::load(dict_path, ".cache")),
                monitor: None,
                tooltip_window: None,
            },
            Command::none(),
        )
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        workaround_event_subscription()
    }

    fn title(&self, _id: window::Id) -> String {
        "OCR".into()
    }

    fn theme(&self, _id: window::Id) -> Self::Theme {
        Theme::GruvboxDark
    }

    type Executor = iced::executor::Default;

    type Theme = Theme;

    type Flags = ();
}

fn workaround_event_subscription() -> Subscription<OcrMessage> {
    struct DeviceQuery;

    subscription::channel(TypeId::of::<DeviceQuery>(), 10, |mut out| async move {
        let device_state = DeviceState::new();
        let (tx, mut rx) = mpsc::channel(10);
        let send = tx.clone();
        let _guard = device_state.on_mouse_move(move |position| {
            send.blocking_send(OcrMessage::CursorMoved(Point {
                x: position.0 as f32,
                y: position.1 as f32,
            }))
            .unwrap();
        });
        let state = device_state.clone();
        let send = tx.clone();
        let _guard = device_state.on_key_up(move |key| {
            //log::info!("Key Up: {key:?}");
            if *key == Keycode::Z && state.get_keys().contains(&Keycode::LAlt) {
                send.blocking_send(OcrMessage::Toggled).unwrap();
            }
        });

        loop {
            let rcv = rx.recv().await.unwrap();
            out.send(rcv).await.unwrap();
        }
    })
}
