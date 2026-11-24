use std::path::{Path, PathBuf};
use std::sync::Mutex;

use docsuite_core::{CancelToken, Error, Result};
use ocrs::{ImageSource, OcrError};
use rten::Model;
use rten_imageproc::BoundingRect;
use rten_tensor::NdTensor;

use crate::ctc;
use crate::gray::GrayImage;
use crate::models::{self, ensure_handwriting, ensure_models};
use crate::ocrs_recognizer::OcrsRecognizer;
use crate::preprocess::{crop_gray, horizontal_projection_lines};
use crate::recognizer::Recognizer;
use crate::scorer::score_candidates;
use crate::types::{BBox, Candidate, Line, Word};

const IN_H: u32 = 32;
const IN_W: u32 = 128;
const TOP_N: usize = 5;

pub struct HtrRecognizer {
    ocrs: OcrsRecognizer,
    handwriting: PathBuf,
    engine: Mutex<Option<Model>>,
}

impl HtrRecognizer {
    pub fn with_dir(dir: &Path) -> Result<Self> {
        ensure_models(dir)?;
        ensure_handwriting(dir)?;
        let (htr, _) = models::handwriting_path(dir);
        Ok(Self {
            ocrs: OcrsRecognizer::with_dir(dir)?,
            handwriting: htr,
            engine: Mutex::new(None),
        })
    }

    fn model(&self) -> Result<std::sync::MutexGuard<'_, Option<Model>>> {
        self.engine
            .lock()
            .map_err(|_| Error::Ocr("htr model lock poisoned".to_owned()))
    }

    fn with_htr_model<R>(&self, f: impl FnOnce(&Model) -> Result<R>) -> Result<R> {
        let mut guard = self.model()?;
        if guard.is_none() {
            let m = Model::load_file(&self.handwriting).map_err(|e| Error::Ocr(e.to_string()))?;
            *guard = Some(m);
        }
        let model = guard
            .as_ref()
            .ok_or_else(|| Error::Ocr("htr model missing".to_owned()))?;
        f(model)
    }

    fn recognize_word(&self, patch: &GrayImage, cancel: &CancelToken) -> Result<Vec<Candidate>> {
        cancel.check()?;
        let tensor = word_tensor(patch)?;
        self.with_htr_model(|model| {
            let input_id = model
                .node_id("image")
                .map_err(|e| Error::Ocr(e.to_string()))?;
            let output_id = model
                .node_id("log_probs")
                .map_err(|e| Error::Ocr(e.to_string()))?;
            let inputs = vec![(input_id, tensor.into())];
            let [out] = model
                .run_n(inputs, [output_id], None)
                .map_err(|e| Error::Ocr(e.to_string()))?;
            let (shape, data) = out
                .into_shape_vec::<f32, 3>()
                .map_err(|e| Error::Ocr(e.to_string()))?;
            let seq_len = shape[0];
            let width = shape[2];
            let decoded = ctc::beam_top_n(&data, seq_len, width, TOP_N);
            if decoded.is_empty() {
                let g = ctc::greedy_decode(&data, seq_len, width);
                let raw = vec![Candidate {
                    text: g.text,
                    confidence: g.confidence,
                }];
                return Ok(raw);
            }
            let raw: Vec<Candidate> = decoded
                .into_iter()
                .map(|d| Candidate {
                    text: d.text,
                    confidence: d.confidence,
                })
                .collect();
            let ranked = score_candidates(&raw, None);
            let mut out = Vec::with_capacity(TOP_N);
            for s in ranked.into_iter().take(TOP_N) {
                out.push(Candidate {
                    text: s.text,
                    confidence: s.confidence,
                });
            }
            Ok(out)
        })
    }

    fn lines_from_detection(
        &self,
        image: &GrayImage,
        offset_x: f32,
        offset_y: f32,
        cancel: &CancelToken,
    ) -> Result<Vec<Line>> {
        let source = ImageSource::from_bytes(&image.pixels, (image.width, image.height))
            .map_err(|e| Error::Ocr(e.to_string()))?;
        cancel.check()?;
        self.ocrs.with_engine(|engine| {
            let input = engine.prepare_input(source).map_err(map_ocr_err)?;
            let words = engine.detect_words(&input).map_err(map_ocr_err)?;
            cancel.check()?;
            if words.is_empty() {
                return Ok(Vec::new());
            }
            let groups = engine.find_text_lines(&input, &words);
            let mut lines = Vec::new();
            for group in groups {
                let mut line_words = Vec::new();
                let mut x0 = f32::MAX;
                let mut y0 = f32::MAX;
                let mut x1 = f32::MIN;
                let mut y1 = f32::MIN;
                for rect in group {
                    let br = rect.bounding_rect();
                    x0 = x0.min(br.left());
                    y0 = y0.min(br.top());
                    x1 = x1.max(br.right());
                    y1 = y1.max(br.bottom());
                    let wb = BBox {
                        x0: br.left(),
                        y0: br.top(),
                        x1: br.right(),
                        y1: br.bottom(),
                    };
                    let patch = crop_gray(image, wb);
                    let cands = self.recognize_word(&patch, cancel)?;
                    if cands.is_empty() {
                        continue;
                    }
                    line_words.push(Word {
                        bbox: BBox {
                            x0: wb.x0 + offset_x,
                            y0: wb.y0 + offset_y,
                            x1: wb.x1 + offset_x,
                            y1: wb.y1 + offset_y,
                        },
                        candidates: cands,
                    });
                }
                if line_words.is_empty() {
                    continue;
                }
                lines.push(Line {
                    bbox: BBox {
                        x0: x0 + offset_x,
                        y0: y0 + offset_y,
                        x1: x1 + offset_x,
                        y1: y1 + offset_y,
                    },
                    words: line_words,
                });
            }
            Ok(lines)
        })
    }

    fn lines_from_projection(
        &self,
        image: &GrayImage,
        offset_x: f32,
        offset_y: f32,
        cancel: &CancelToken,
    ) -> Result<Vec<Line>> {
        let line_boxes = horizontal_projection_lines(image, 3);
        let mut out = Vec::new();
        for lb in line_boxes {
            cancel.check()?;
            let line_img = crop_gray(image, lb);
            let words = vertical_projection_words(&line_img, 4);
            let mut line_words = Vec::new();
            for wb in words {
                let patch = crop_gray(&line_img, wb);
                let cands = self.recognize_word(&patch, cancel)?;
                if cands.is_empty() {
                    continue;
                }
                line_words.push(Word {
                    bbox: BBox {
                        x0: wb.x0 + lb.x0 + offset_x,
                        y0: wb.y0 + lb.y0 + offset_y,
                        x1: wb.x1 + lb.x0 + offset_x,
                        y1: wb.y1 + lb.y0 + offset_y,
                    },
                    candidates: cands,
                });
            }
            if line_words.is_empty() {
                continue;
            }
            out.push(Line {
                bbox: BBox {
                    x0: lb.x0 + offset_x,
                    y0: lb.y0 + offset_y,
                    x1: lb.x1 + offset_x,
                    y1: lb.y1 + offset_y,
                },
                words: line_words,
            });
        }
        Ok(out)
    }
}

fn map_ocr_err(e: OcrError) -> Error {
    Error::Ocr(e.to_string())
}

fn word_tensor(image: &GrayImage) -> Result<NdTensor<f32, 4>> {
    let mut data = vec![-0.5f32; (IN_H * IN_W) as usize];
    let scale = (IN_W as f32 / image.width as f32).min(IN_H as f32 / image.height as f32);
    let rw = (image.width as f32 * scale).round().max(1.0) as u32;
    let rh = (image.height as f32 * scale).round().max(1.0) as u32;
    let ox = (IN_W - rw) / 2;
    let oy = (IN_H - rh) / 2;
    for y in 0..rh {
        for x in 0..rw {
            let sx = ((x as f32) / scale).floor() as u32;
            let sy = ((y as f32) / scale).floor() as u32;
            let p = image.get(
                sx.min(image.width.saturating_sub(1)),
                sy.min(image.height.saturating_sub(1)),
            );
            let v = (p as f32) / 255.0 - 0.5;
            let dx = ox + x;
            let dy = oy + y;
            if dx < IN_W && dy < IN_H {
                data[dy as usize * IN_W as usize + dx as usize] = v;
            }
        }
    }
    Ok(NdTensor::from_data(
        [1, 1, IN_H as usize, IN_W as usize],
        data,
    ))
}

pub fn vertical_projection_words(image: &GrayImage, min_gap: u32) -> Vec<BBox> {
    let mut cols = vec![0u32; image.width as usize];
    for x in 0..image.width {
        let mut dark = 0u32;
        for y in 0..image.height {
            if image.get(x, y) < 128 {
                dark += 1;
            }
        }
        cols[x as usize] = dark;
    }
    let mut words = Vec::new();
    let mut x = 0u32;
    while x < image.width {
        while x < image.width && cols[x as usize] == 0 {
            x += 1;
        }
        if x >= image.width {
            break;
        }
        let x0 = x;
        while x < image.width && cols[x as usize] > 0 {
            x += 1;
        }
        let x1 = x;
        if x1 > x0 {
            words.push(BBox {
                x0: x0 as f32,
                y0: 0.0,
                x1: x1 as f32,
                y1: image.height as f32,
            });
        }
        x += min_gap.max(1);
    }
    if words.is_empty() && image.width > 0 && image.height > 0 {
        words.push(BBox {
            x0: 0.0,
            y0: 0.0,
            x1: image.width as f32,
            y1: image.height as f32,
        });
    }
    words
}

impl Recognizer for HtrRecognizer {
    fn name(&self) -> &str {
        "htr"
    }

    fn recognize(
        &self,
        image: &GrayImage,
        region: Option<BBox>,
        cancel: &CancelToken,
    ) -> Result<Vec<Line>> {
        cancel.check()?;
        let (work, ox, oy) = if let Some(r) = region {
            (crop_gray(image, r), r.x0, r.y0)
        } else {
            (image.clone(), 0.0, 0.0)
        };
        let det = self.lines_from_detection(&work, ox, oy, cancel)?;
        if !det.is_empty() {
            return Ok(det);
        }
        self.lines_from_projection(&work, ox, oy, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_words_single_blob() {
        let w = 60u32;
        let h = 20u32;
        let mut px = vec![255u8; (w * h) as usize];
        for y in 0..h {
            for x in 10..50 {
                px[y as usize * w as usize + x as usize] = 0;
            }
        }
        let img = GrayImage::new(w, h, px).unwrap();
        let words = vertical_projection_words(&img, 2);
        assert_eq!(words.len(), 1);
    }
}
