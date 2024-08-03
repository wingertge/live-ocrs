use std::sync::Arc;

use capture::CaptureState;
use character::Block;
use device_query::{DeviceQuery as _, DeviceState, MouseState};
use dict::{Dictionary, DictionaryEntry};
use geo::{Coord, EuclideanDistance as _, LineString, Polygon, Rect};
use image::{Rgb, RgbImage};
use imageproc::point::Point;
use ordered_float::OrderedFloat;
use parking_lot::RwLock;
use unicode_blocks::{is_cjk, CJK_SYMBOLS_AND_PUNCTUATION, HALFWIDTH_AND_FULLWIDTH_FORMS};
use xcap::Monitor;

pub mod capture;
pub mod character;
pub mod dict;

pub struct Definitions {
    pub dict: Dictionary,
    pub ocr_strings: Vec<Block>,
    pub definitions: Vec<DictionaryEntry>,
}

impl Definitions {
    pub fn new(dict: Dictionary) -> Self {
        Self {
            dict,
            ocr_strings: Vec::new(),
            definitions: Vec::new(),
        }
    }

    pub fn update(&mut self, text: &str) {
        self.definitions = self.dict.matches(text);
    }
}

pub struct LiveOcr {
    pub capture_state: Arc<CaptureState>,
    pub enabled: bool,
    pub definitions: Definitions,
    pub hovering: Option<(String, usize, Rect<f32>)>,
    pub monitor: Option<Monitor>,
}

pub fn to_geo_poly(points: &[Point<i32>]) -> Polygon<f32> {
    let points = points
        .iter()
        .map(|point| Coord {
            x: point.x as f32,
            y: point.y as f32,
        })
        .collect();
    Polygon::new(LineString::new(points), vec![])
}

pub fn draw_outline_geo(image: &mut RgbImage, b_box: geo::Rect<f32>, color: Rgb<u8>) {
    let min_x = (b_box.min().x.round() as u32).clamp(0, image.width() - 1);
    let min_y = (b_box.min().y.round() as u32).clamp(0, image.height() - 1);
    let max_x = (b_box.max().x.round() as u32).clamp(0, image.width() - 1);
    let max_y = (b_box.max().y.round() as u32).clamp(0, image.height() - 1);

    for y in min_y..max_y {
        image.put_pixel(min_x, y, color);
        image.put_pixel(max_x, y, color);
    }

    for x in min_x..max_x {
        image.put_pixel(x, min_y, color);
        image.put_pixel(x, max_y, color);
    }
}

pub fn longest_meaningful_string(text: &str, from: usize) -> String {
    text.chars()
        .skip(from)
        .take_while(|ch| {
            is_cjk(*ch)
                && ![CJK_SYMBOLS_AND_PUNCTUATION, HALFWIDTH_AND_FULLWIDTH_FORMS]
                    .contains(&unicode_blocks::find_unicode_block(*ch).unwrap())
        })
        .collect()
}

pub fn find_closest_char(
    ocr_strings: &[Block],
    cursor: geo::Point<f32>,
) -> (String, usize, f32, Rect<f32>) {
    ocr_strings
        .iter()
        .map(|(text, chars)| {
            let (closest_char, closest_distance, closest_rect) = chars
                .iter()
                .map(|(ch, rect)| (*ch, OrderedFloat(rect.euclidean_distance(&cursor)), *rect))
                .min_by_key(|(_, distance, _)| *distance)
                .unwrap_or((
                    0,
                    OrderedFloat(f32::INFINITY),
                    Rect::new(Coord::zero(), Coord::zero()),
                ));
            (text.as_str(), closest_char, closest_distance, closest_rect)
        })
        .min_by_key(|(_, _, distance, _)| *distance)
        .map(|(a, b, c, d)| (a.to_string(), b, *c, d))
        .unwrap()
}

pub type OcrState = Arc<RwLock<LiveOcr>>;

pub fn update_hover(
    state: &mut LiveOcr,
    position: (i32, i32),
) -> Option<(Option<Rect<f32>>, Vec<DictionaryEntry>)> {
    let point = geo::point!(x: position.0 as f32, y: position.1 as f32);
    let (closest_string, closest_char, closest_distance, closest_rect) =
        find_closest_char(&state.definitions.ocr_strings, point);

    if closest_distance < 5.0 {
        if let Some((prev_str, prev_char, _)) = &state.hovering {
            if &closest_string == prev_str && closest_char == *prev_char {
                return None;
            }
        }
        state.hovering = Some((closest_string.to_owned(), closest_char, closest_rect));
        let longest_string = longest_meaningful_string(&closest_string, closest_char);
        state.definitions.update(&longest_string);
        Some((Some(closest_rect), state.definitions.definitions.clone()))
    } else if state.hovering.is_some() {
        state.definitions.definitions.clear();
        state.hovering.take();

        Some((None, Vec::new()))
    } else {
        None
    }
}

pub enum Action {
    UpdateOcr,
    CloseTooltip,
    None,
}

pub fn toggle(state: &mut LiveOcr) -> Action {
    log::info!("Toggled");
    state.enabled = !state.enabled;
    if state.enabled {
        state.definitions.ocr_strings.clear();
        let device_state = DeviceState::new();
        let MouseState {
            coords: (cursor_x, cursor_y),
            ..
        } = device_state.get_mouse();
        let monitor = Monitor::from_point(cursor_x, cursor_y).unwrap();
        let ocr_state = state.capture_state.clone().capture(&monitor);
        state.monitor = Some(monitor);
        state.definitions.ocr_strings = ocr_state;
        update_hover(state, device_state.get_mouse().coords);
        Action::UpdateOcr
    } else {
        state.hovering = None;
        state.monitor = None;
        state.definitions.definitions.clear();
        Action::CloseTooltip
    }
}
