use std::{any::TypeId, sync::Arc};

use device_query::{DeviceEvents as _, DeviceQuery, DeviceState, Keycode, MouseState};
use iced::{
    futures::SinkExt,
    subscription, theme,
    widget::{text, Column, Row},
    Application, Color, Command, Element, Point, Subscription, Theme,
};
use rapidocr::RapidOCRBuilder;
use tokio::sync::mpsc;
use xcap::Monitor;

use crate::{
    capture::CaptureState,
    character::Block,
    dict::{self, Dictionary, DictionaryEntry, Pinyin, Tone},
    find_closest_char, longest_meaningful_string,
};

pub struct LiveOcr {
    pub capture_state: Arc<CaptureState>,
    pub dict: Dictionary,
    pub ocr_strings: Vec<Block>,
    pub enabled: bool,
    pub hovering: Option<(String, usize)>,
    pub definitions: Vec<DictionaryEntry>,
}

#[derive(Debug, Clone)]
pub enum OcrMessage {
    OcrChanged(Vec<Block>),
    CursorMoved(iced::Point),
    Toggled,
}

fn pinyin_color(tone: Tone) -> Color {
    match tone {
        Tone::First => Color::new(227. / 255., 0.0, 0.0, 1.0),
        Tone::Second => Color::new(2. / 255., 179. / 255., 28. / 255., 1.0),
        Tone::Third => Color::new(21. / 255., 16. / 255., 240. / 255., 1.0),
        Tone::Fourth => Color::new(137. / 255., 0., 191. / 255., 1.0),
        Tone::Fifth => Color::new(119. / 255., 119. / 255., 119. / 255., 1.0),
        Tone::None => Color::WHITE,
    }
}

fn view_pinyin(pinyin: &[Pinyin]) -> Element<OcrMessage> {
    let elements = pinyin.iter().map(|it| {
        text(it.syllable.to_owned())
            .size(16)
            .style(theme::Text::Color(pinyin_color(it.tone)))
            .into()
    });
    Row::with_children(elements).into()
}

impl LiveOcr {
    fn view_definitions(&self) -> Vec<Element<OcrMessage>> {
        let definition_text = self.definitions.iter().flat_map(|definition| {
            let header = text(definition.simplified.to_owned()).size(20);
            let pinyin = view_pinyin(&definition.pinyin);
            let body = definition
                .translations
                .iter()
                .map(|translation| text(translation).size(16).into());
            vec![header.into(), pinyin].into_iter().chain(body)
        });
        vec![
            text(if self.enabled {
                "Definitions: "
            } else {
                "Disabled"
            })
            .size(24)
            .into(),
            text("").size(16).into(),
        ]
        .into_iter()
        .chain(definition_text)
        .collect()
    }
}

impl Application for LiveOcr {
    fn view(&self) -> Element<OcrMessage> {
        /*         let hovering = self.hovering.iter().flat_map(|(str, char)| {
            vec![
                Text::new(format!("Text: {str}")).size(24).into(),
                Text::new(format!("Char: {}", str.chars().nth(*char).unwrap()))
                    .size(24)
                    .into(),
            ]
        });
        let text_elements = self
            .ocr_strings
            .iter()
            .map(|(text, _)| Text::new(text).size(16).into());
        let text_elements = vec![Text::new("OCR lines:").size(24).into()]
            .into_iter()
            .chain(text_elements)
            .chain(vec![Text::new("Hovering: ").size(24).into()])
            .chain(hovering)
            .chain(self.view_definitions()); */
        Column::with_children(self.view_definitions())
            .padding(32)
            .into()
    }

    fn update(&mut self, message: OcrMessage) -> Command<OcrMessage> {
        match message {
            OcrMessage::OcrChanged(ocr_result) => {
                self.ocr_strings = ocr_result;
                Command::none()
            }
            OcrMessage::Toggled => {
                log::info!("Toggled");
                self.enabled = !self.enabled;
                if self.enabled {
                    self.ocr_strings.clear();
                    let device_state = DeviceState::new();
                    let MouseState {
                        coords: (cursor_x, cursor_y),
                        ..
                    } = device_state.get_mouse();
                    let monitor = Monitor::from_point(cursor_x, cursor_y).unwrap();
                    Command::perform(self.capture_state.clone().capture(monitor), |ocr_state| {
                        OcrMessage::OcrChanged(ocr_state)
                    })
                } else {
                    self.hovering = None;
                    self.definitions = Vec::new();
                    Command::none()
                }
            }
            OcrMessage::CursorMoved(position) => {
                if self.enabled && !self.ocr_strings.is_empty() {
                    let point = geo::point!(x: position.x, y: position.y);
                    let (closest_string, closest_char, closest_distance) =
                        find_closest_char(&self.ocr_strings, point);
                    if closest_distance < 5.0 {
                        if let Some((prev_str, prev_char)) = &self.hovering {
                            if closest_string == prev_str && closest_char == *prev_char {
                                return Command::none();
                            }
                        }
                        self.hovering = Some((closest_string.to_owned(), closest_char));
                        let longest_string =
                            longest_meaningful_string(closest_string, closest_char);
                        self.definitions = self.dict.matches(&longest_string);
                    }
                }
                Command::none()
            }
        }
    }

    type Message = OcrMessage;

    fn new(_flags: ()) -> (Self, Command<OcrMessage>) {
        let ocr = RapidOCRBuilder::new().build().unwrap();
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
                ocr_strings: Vec::new(),
                enabled: false,
                hovering: None,
                dict: dict::load("data/cedict.json"),
                definitions: Vec::new(),
            },
            Command::none(),
        )
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        workaround_event_subscription()
    }

    fn title(&self) -> String {
        "OCR".into()
    }

    fn theme(&self) -> Self::Theme {
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
            log::info!("Key Up: {key:?}");
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
