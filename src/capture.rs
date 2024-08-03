use std::sync::Arc;

use geo::{BoundingRect, Rect};
use image::DynamicImage;
use rapidocr::{DetectionOptions, RapidOCR};
use xcap::Monitor;

use crate::character::detect_char_boxes;

pub struct CaptureState {
    pub ocr: RapidOCR,
}

impl CaptureState {
    pub fn capture(self: Arc<Self>, monitor: &Monitor) -> Vec<(String, Vec<(usize, Rect<f32>)>)> {
        let image = monitor.capture_image().unwrap();
        #[cfg(feature = "debug")]
        image.save("screen.png").unwrap();
        let image = image.into();
        let boxes = do_ocr(&self.ocr, &image, monitor);
        #[cfg(feature = "debug")]
        {
            use crate::draw_outline_geo;
            use image::Rgb;

            image.to_luma8().save("screen_gray.png").unwrap();
            let mut image = image.to_rgb8();
            for (_, contour) in boxes.iter().flat_map(|it| &it.1) {
                draw_outline_geo(&mut image, *contour, Rgb([255, 0, 0]))
            }
            image.save("boundaries.png").unwrap();
        }

        boxes
    }
}

pub fn do_ocr(
    ocr: &RapidOCR,
    image: &DynamicImage,
    monitor: &Monitor,
) -> Vec<(String, Vec<(usize, Rect<f32>)>)> {
    let options = DetectionOptions {
        max_side_len: 2048,
        ..Default::default()
    };
    let detection_result = ocr.detect(&image, options).unwrap();
    for result in &detection_result {
        log::debug!(
            "[Text: {}, Bounds: {:?}]",
            result.text.text,
            result.bounds.rect.bounding_rect().unwrap()
        );
    }
    let char_boxes = detect_char_boxes(&image, &detection_result, monitor);
    char_boxes
}
