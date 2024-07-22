use std::sync::Arc;

use geo::{BoundingRect, Rect};
use image::{DynamicImage, Rgb};
use rapidocr::{DetectionOptions, RapidOCR};
use tokio::task::spawn_blocking;
use xcap::Monitor;

use crate::{character::detect_char_boxes, draw_outline_geo};

pub struct CaptureState {
    pub ocr: RapidOCR,
}

impl CaptureState {
    pub async fn capture(
        self: Arc<Self>,
        monitor: Monitor,
    ) -> Vec<(String, Vec<(usize, Rect<f32>)>)> {
        let image = monitor.capture_image().unwrap();
        image.save("screen.png").unwrap();
        let image = image.into();
        spawn_blocking(move || {
            let boxes = do_ocr(&self.ocr, &image);
            image.to_luma8().save("screen_gray.png").unwrap();
            let mut image = image.to_rgb8();
            for (_, contour) in boxes.iter().flat_map(|it| &it.1) {
                draw_outline_geo(&mut image, *contour, Rgb([255, 0, 0]))
            }
            image.save("boundaries.png").unwrap();
            boxes
        })
        .await
        .unwrap()
    }
}

pub fn do_ocr(ocr: &RapidOCR, image: &DynamicImage) -> Vec<(String, Vec<(usize, Rect<f32>)>)> {
    let detection_result = ocr.detect(&image, DetectionOptions::default()).unwrap();
    for result in &detection_result {
        log::debug!(
            "[Text: {}, Bounds: {:?}]",
            result.text.text,
            result.bounds.rect.bounding_rect().unwrap()
        );
    }
    let char_boxes = detect_char_boxes(&image, &detection_result);
    char_boxes
}
