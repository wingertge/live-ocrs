use character::Block;
use geo::{Coord, EuclideanDistance as _, LineString, Polygon};
use image::{Rgb, RgbImage};
use imageproc::point::Point;
use ordered_float::OrderedFloat;
use unicode_blocks::{is_cjk, CJK_SYMBOLS_AND_PUNCTUATION, HALFWIDTH_AND_FULLWIDTH_FORMS};

pub mod app;
pub mod capture;
pub mod character;
pub mod dict;

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

pub fn find_closest_char(ocr_strings: &[Block], cursor: geo::Point<f32>) -> (&str, usize, f32) {
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
        .map(|(a, b, c)| (a, b, *c))
        .unwrap()
}
