use geo::{coord, BoundingRect, Intersects, Rect, Translate};
use geo_clipper::{Clipper, EndType, JoinType};
use image::DynamicImage;
#[cfg(feature = "debug")]
use image::Rgb;
use imageproc::{
    contours::{find_contours_with_threshold, BorderType},
    contrast::{threshold, ThresholdType},
};
use ordered_float::OrderedFloat;
use rapidocr::OcrResult;
use unicode_blocks::{
    find_unicode_block, is_cjk, CJK_SYMBOLS_AND_PUNCTUATION, HALFWIDTH_AND_FULLWIDTH_FORMS,
};

#[cfg(feature = "debug")]
use crate::draw_outline_geo;
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
            let text = strip_punctuation(&line.text.text);
            log::info!("Stripped string: {text}");
            let text_len = text.chars().count();
            let removed = line.text.text.chars().count() - text_len;
            println!("{} is CJK: {}", text, text.trim().chars().all(is_cjk));
            if text_len == 1 {
                return Some((
                    text,
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

            let mut gray_image = threshold(&image.to_luma8(), 128, ThresholdType::Binary);
            if gray_image.get_pixel(0, 0).0 == [255] {
                gray_image = threshold(&image.to_luma8(), 128, ThresholdType::BinaryInverted);
            }

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
            #[cfg(feature = "debug")]
            {
                let mut image = DynamicImage::ImageLuma8(gray_image).to_rgb8();
                for contour in bounds.iter() {
                    draw_outline_geo(&mut image, *contour, Rgb([255, 0, 0]))
                }
                image.save(format!("part_images/subimage{i}.png")).unwrap();
            }
            if bounds.len() < 2 {
                log::info!("bounds too small");
                return None;
            }

            bounds.sort_by_cached_key(|it| OrderedFloat(it.min().x));

            if removed > 0 {
                bounds = remove_overlap(bounds);
                log::debug!("New bounds len: {}, Text len: {text_len}", bounds.len());
                bounds.truncate(bounds.len() - removed);
            }

            let mut character_width = find_character_width(&bounds);
            if character_width == 0.0 {
                log::info!("No contours found for {}", line.text.text);
                return None;
            }
            log::info!("Character width: {character_width}");
            let line_rect = find_line_bounds(&bounds, character_width);
            log::info!("Detected line height: {}", line_rect.height());
            if character_width * text_len as f32 > line_rect.width() {
                let new_width = line_rect.width() / text_len as f32;
                log::warn!(
                    "Incorrect boxes: character boxes exceed line. Correcting by {}",
                    new_width / character_width
                );
                character_width = new_width;
            }

            let letter_spacing =
                (line_rect.width() - character_width * text_len as f32) / (text_len - 1) as f32;

            //let letter_spacing = find_letter_spacing(&bounds, character_width, line_rect);
            log::info!("Detected character spacing: {letter_spacing}");

            Some((
                text.clone(),
                text.chars()
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

/* fn find_letter_spacing(bounds: &[Rect<f32>], character_width: f32, line: Rect<f32>) -> f32 {
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
} */

fn strip_punctuation(text: &str) -> String {
    let text: String = text
        .chars()
        .rev()
        .skip_while(|it| {
            !is_cjk(*it)
                || [CJK_SYMBOLS_AND_PUNCTUATION, HALFWIDTH_AND_FULLWIDTH_FORMS]
                    .contains(&find_unicode_block(*it).unwrap())
        })
        .collect();
    text.chars().rev().collect()
}

fn remove_overlap(bounds: Vec<Rect<f32>>) -> Vec<Rect<f32>> {
    let mut new_bounds: Vec<Rect<f32>> = Vec::with_capacity(bounds.len());
    for bound in bounds {
        if let Some(other) = new_bounds.iter().find(|it| it.intersects(&bound)) {
            new_bounds.push(merge_rects(bound, *other));
        } else {
            new_bounds.push(bound);
        }
    }
    new_bounds
}

fn merge_rects(this: Rect<f32>, other: Rect<f32>) -> Rect<f32> {
    let min_x = this.min().x.min(other.min().x);
    let max_x = this.max().x.max(other.max().x);
    let min_y = this.min().y.min(other.min().y);
    let max_y = this.max().y.max(other.max().y);
    Rect::new(coord![x: min_x, y: min_y], coord![x: max_x, y: max_y])
}
