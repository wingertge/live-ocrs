use geo::{coord, BoundingRect, Rect, Scale, Translate};
use geo_clipper::{Clipper, EndType, JoinType};
use image::DynamicImage;
use imageproc::contours::{find_contours_with_threshold, BorderType};
use ordered_float::OrderedFloat;
use rapidocr::OcrResult;
use unicode_blocks::is_cjk;

use crate::to_geo_poly;

pub type Character = (usize, Rect<f32>);
pub type Characters = Vec<Character>;
pub type Block = (String, Characters);

pub fn detect_char_boxes(image: &DynamicImage, detection_results: &[OcrResult]) -> Vec<Block> {
    detection_results
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            let text = line.text.text.trim();
            text.chars().count() > 0 && text.chars().all(is_cjk)
        })
        .filter_map(|(i, line)| {
            println!(
                "{} is CJK: {}",
                line.text.text,
                line.text.text.trim().chars().all(is_cjk)
            );
            if line.text.text.chars().count() == 1 {
                return Some((
                    line.text.text.to_owned(),
                    vec![(0usize, line.bounds.rect.bounding_rect().unwrap())],
                ));
            }
            log::info!("Contouring {i}");
            let rect = line.bounds.rect.bounding_rect().unwrap();
            let image = image.crop_imm(
                rect.min().x as u32,
                rect.min().y as u32,
                rect.width() as u32,
                rect.height() as u32,
            );

            let gray_image = image.to_luma8();

            let mut bounds = find_contours_with_threshold::<i32>(&gray_image, 128)
                .into_iter()
                .filter(|contour| contour.border_type == BorderType::Outer)
                .map(|it| to_geo_poly(&it.points).bounding_rect().unwrap())
                .filter_map(|it| {
                    let poly =
                        it.to_polygon()
                            .offset(0.5, JoinType::Square, EndType::ClosedPolygon, 1.0);
                    poly.bounding_rect()
                })
                .collect::<Vec<_>>();

            if bounds.len() < 2 {
                log::info!("bounds too small");
                return None;
            }

            bounds.sort_by_cached_key(|it| OrderedFloat(it.min().x));

            let character_width = find_character_width(&bounds);
            if character_width == 0.0 {
                log::info!("No contours found for {}", line.text.text);
                return None;
            }
            log::info!("Character width: {character_width}");
            let line_rect = find_line_bounds(&bounds, character_width);
            log::info!("Detected line height: {}", line_rect.height());
            let letter_spacing = find_letter_spacing(&bounds, character_width, line_rect);
            log::info!("Detected character spacing: {letter_spacing}");

            Some((
                line.text.text.to_owned(),
                line.text
                    .text
                    .chars()
                    .enumerate()
                    .map(|(i, _)| {
                        let min_x =
                            line_rect.min().x + i as f32 * (letter_spacing + character_width);
                        let max_x = min_x + character_width;
                        (
                            i,
                            Rect::new(
                                coord![x: min_x, y: line_rect.min().y],
                                coord![x: max_x, y: line_rect.max().y],
                            )
                            .translate(rect.min().x, rect.min().y),
                        )
                    })
                    .collect(),
            ))
        })
        .collect()
}

fn find_line_bounds(bounds: &[Rect<f32>], char_width: f32) -> Rect<f32> {
    let min_y = *bounds
        .iter()
        .map(|it| OrderedFloat(it.min().y))
        .min()
        .unwrap();
    let min_x = *bounds
        .iter()
        .map(|it| OrderedFloat(it.min().x))
        .min()
        .unwrap();
    let max_height = *bounds
        .iter()
        .map(|it| OrderedFloat(it.height()))
        .max()
        .unwrap();
    let max_x = *bounds
        .iter()
        .map(|it| OrderedFloat(it.max().x))
        .max()
        .unwrap();
    Rect::new(
        coord![x: min_x, y: min_y],
        coord![x: max_x, y:  min_y + max_height.max(char_width)],
    )
}

fn find_character_width(bounds: &[Rect<f32>]) -> f32 {
    *bounds
        .iter()
        .map(|it| OrderedFloat(it.width()))
        .max()
        .unwrap_or(OrderedFloat(0.0))
}

fn find_letter_spacing(bounds: &[Rect<f32>], character_width: f32, line: Rect<f32>) -> f32 {
    // Expand bounds to full width & height
    let new_bounds = bounds
        .iter()
        .map(|rect| {
            let mut rect = rect.scale_xy(character_width / rect.width(), 1.0);
            rect.set_min((rect.min().x, line.min().y));
            rect.set_max((rect.max().x, line.max().y));
            rect
        })
        .collect::<Vec<_>>();

    let mut distances = new_bounds
        .windows(2)
        .map(|rects| OrderedFloat((rects[1].min().x - rects[0].max().x).abs()))
        .collect::<Vec<_>>();
    distances.sort_unstable();
    log::info!("Distances: {distances:?}");
    let median = *distances[distances.len() / 2];
    let filter_outliers = distances
        .into_iter()
        .map(|it| *it)
        .filter(|&it| it / median < 2.0 && it / median > 0.5)
        .collect::<Vec<_>>();
    let count = filter_outliers.len() as f32;
    filter_outliers.into_iter().sum::<f32>() / count
}
