use std::sync::Arc;

use geo::{BoundingRect, Rect};
use image::Rgb;
use rapidocr::{DetectionOptions, RapidOCR};
use tokio::task::spawn_blocking;
use xcap::Monitor;

use crate::{character::detect_char_boxes, draw_outline_geo};

pub struct CaptureState {
    pub ocr: RapidOCR,
    pub monitor: Monitor,
}

impl CaptureState {
    pub async fn capture(self: Arc<Self>) -> Vec<(String, Vec<(usize, Rect<f32>)>)> {
        let image = self.monitor.capture_image().unwrap();
        image.save("screen.png").unwrap();
        let image = image.into();
        spawn_blocking(move || {
            let detection_result = self
                .ocr
                .detect(&image, DetectionOptions::default())
                .unwrap();
            for result in &detection_result {
                log::debug!(
                    "[Text: {}, Bounds: {:?}]",
                    result.text.text,
                    result.bounds.rect.bounding_rect().unwrap()
                );
            }
            let char_boxes = detect_char_boxes(&image, &detection_result);
            let mut image = image.to_rgb8();
            for (_, contour) in char_boxes.iter().flat_map(|it| &it.1) {
                draw_outline_geo(&mut image, *contour, Rgb([255, 0, 0]))
            }
            image.save("boundaries.png").unwrap();
            char_boxes
        })
        .await
        .unwrap()
    }
}
