use std::sync::Arc;

use app::LiveOcr;
use character::Block;
use device_query::{DeviceQuery as _, DeviceState, MouseState};
use dict::DictionaryEntry;
use geo::{Coord, EuclideanDistance as _, LineString, Polygon, Rect};
use image::{Rgb, RgbImage};
use imageproc::point::Point;
use ordered_float::OrderedFloat;
use parking_lot::RwLock;
use unicode_blocks::{is_cjk, CJK_SYMBOLS_AND_PUNCTUATION, HALFWIDTH_AND_FULLWIDTH_FORMS};
use xcap::Monitor;

pub mod app;
pub mod capture;
pub mod character;
pub mod dict;
pub mod view;

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

pub fn find_closest_char(ocr_strings: &[Block], cursor: geo::Point<f32>) -> (String, usize, f32) {
    ocr_strings
        .iter()
        .map(|(text, chars)| {
            let (closest_char, closest_distance) = chars
                .iter()
                .map(|(ch, rect)| (*ch, OrderedFloat(rect.euclidean_distance(&cursor))))
                .min_by_key(|(_, distance)| *distance)
                .unwrap_or((0, OrderedFloat(f32::INFINITY)));
            (text.as_str(), closest_char, closest_distance)
        })
        .min_by_key(|(_, _, distance)| *distance)
        .map(|(a, b, c)| (a.to_string(), b, *c))
        .unwrap()
}

pub mod native {
    use std::ffi::c_void;

    use windows::Win32::{
        Graphics::Gdi::HMONITOR,
        UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
    };
    use xcap::Monitor;

    pub fn get_scale_factor(monitor: &Monitor) -> f32 {
        let inner = HMONITOR(monitor.id() as *mut c_void);
        get_scale_factor_raw(inner)
    }

    pub fn get_scale_factor_raw(handle: HMONITOR) -> f32 {
        let mut dpi_x = 0;
        let mut dpi_y = 0;
        unsafe { GetDpiForMonitor(handle, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.unwrap();
        log::info!("DPI: {dpi_x}x{dpi_y}");
        let factor = (dpi_y as f32) / 96.0;
        log::info!("Scale factor: {factor}");

        factor
    }
}

pub type OcrState = Arc<RwLock<LiveOcr>>;

pub fn update_hover(state: &mut LiveOcr, position: (i32, i32)) -> Option<Vec<DictionaryEntry>> {
    let point = geo::point!(x: position.0 as f32, y: position.1 as f32);
    let (closest_string, closest_char, closest_distance) =
        find_closest_char(&state.definitions.ocr_strings, point);

    if closest_distance < 5.0 {
        if let Some((prev_str, prev_char)) = &state.hovering {
            if &closest_string == prev_str && closest_char == *prev_char {
                return None;
            }
        }
        state.hovering = Some((closest_string.to_owned(), closest_char));
        let longest_string = longest_meaningful_string(&closest_string, closest_char);
        state.definitions.update(&longest_string);
        Some(state.definitions.definitions.clone())
    } else {
        state.definitions.definitions.clear();
        Some(Vec::new())
    }
}

pub enum Action {
    UpdateOcr(Vec<(String, Vec<(usize, Rect<f32>)>)>),
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
        state.monitor = Some(monitor.clone());
        Action::UpdateOcr(state.capture_state.clone().capture(monitor))
    } else {
        state.hovering = None;
        state.monitor = None;
        state.definitions.definitions.clear();
        Action::CloseTooltip
    }
}
