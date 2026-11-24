use std::collections::HashSet;

use docsuite_pdf::{PageRanges, PdfRect};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ViewerInteractionMode {
    #[default]
    Normal,
    Crop,
    Signature,
}

#[derive(Clone, Debug)]
pub struct UndoStack {
    cap: usize,
    past: Vec<Vec<u8>>,
    future: Vec<Vec<u8>>,
}

impl UndoStack {
    pub fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            past: Vec::new(),
            future: Vec::new(),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn push_checkpoint(&mut self, bytes: &[u8]) {
        self.past.push(bytes.to_vec());
        while self.past.len() > self.cap {
            self.past.remove(0);
        }
        self.future.clear();
    }

    pub fn undo(&mut self, current: &[u8]) -> Option<Vec<u8>> {
        let prev = self.past.pop()?;
        self.future.push(current.to_vec());
        Some(prev)
    }

    #[allow(dead_code)]
    pub fn redo(&mut self, current: &[u8]) -> Option<Vec<u8>> {
        let next = self.future.pop()?;
        self.past.push(current.to_vec());
        Some(next)
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
    }
}

#[derive(Clone, Debug, Default)]
pub struct PageSelection {
    selected: HashSet<u32>,
    anchor: Option<u32>,
}

impl PageSelection {
    pub fn selected_vec(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.selected.iter().copied().collect();
        v.sort_unstable();
        v
    }

    pub fn toggle(&mut self, page: u32, extend: bool, page_count: u32) {
        if page == 0 || page > page_count {
            return;
        }
        if extend {
            if let Some(anchor) = self.anchor {
                let lo = anchor.min(page);
                let hi = anchor.max(page);
                for p in lo..=hi {
                    self.selected.insert(p);
                }
            } else {
                self.selected.insert(page);
                self.anchor = Some(page);
            }
        } else {
            if self.selected.contains(&page) {
                self.selected.remove(&page);
            } else {
                self.selected.insert(page);
            }
            self.anchor = Some(page);
        }
    }

    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    pub fn set_order_pages(&mut self, pages: &[u32]) {
        self.selected.clear();
        for p in pages {
            self.selected.insert(*p);
        }
    }

    pub fn apply_reorder_drag(&mut self, from: usize, to: usize, order: &mut Vec<u32>) {
        if from >= order.len() || to >= order.len() || from == to {
            return;
        }
        let item = order.remove(from);
        order.insert(to, item);
    }
}

pub fn parse_tool_split_ranges(spec: &str) -> Result<Vec<PageRanges>, String> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Err("empty split ranges".to_owned());
    }
    let mut out = Vec::new();
    for part in trimmed.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        if part.contains(',') {
            for group in part.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                out.extend(PageRanges::parse_groups(group).map_err(|e| e.to_string())?);
            }
        } else {
            out.push(PageRanges::parse(part).map_err(|e| e.to_string())?);
        }
    }
    Ok(out)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPoint {
    pub x: f32,
    pub y: f32,
}

pub fn screen_to_page_pt(
    screen: ScreenPoint,
    page_screen_x: f32,
    page_screen_y: f32,
    scale: f32,
    page_h_pt: f32,
) -> (f64, f64) {
    let x_pt = (screen.x - page_screen_x) as f64 / scale as f64;
    let y_from_top_pt = (screen.y - page_screen_y) as f64 / scale as f64;
    let y_pt = page_h_pt as f64 - y_from_top_pt;
    (x_pt.max(0.0), y_pt.max(0.0))
}

pub fn page_pt_rect_to_pdf_rect(x0: f64, y0: f64, x1: f64, y1: f64, page_h_pt: f32) -> PdfRect {
    let llx = x0.min(x1);
    let urx = x0.max(x1);
    let lly = y0.min(y1).min(page_h_pt as f64);
    let ury = y0.max(y1).min(page_h_pt as f64);
    PdfRect { llx, lly, urx, ury }
}

pub fn screen_drag_to_pdf_rect(
    a: ScreenPoint,
    b: ScreenPoint,
    page_screen_x: f32,
    page_screen_y: f32,
    scale: f32,
    page_h_pt: f32,
) -> PdfRect {
    let (x0, y0) = screen_to_page_pt(a, page_screen_x, page_screen_y, scale, page_h_pt);
    let (x1, y1) = screen_to_page_pt(b, page_screen_x, page_screen_y, scale, page_h_pt);
    page_pt_rect_to_pdf_rect(x0, y0, x1, y1, page_h_pt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_stack_bounds() {
        let mut u = UndoStack::new(2);
        u.push_checkpoint(b"a");
        u.push_checkpoint(b"b");
        u.push_checkpoint(b"c");
        assert_eq!(u.past.len(), 2);
        let prev = u.undo(b"current").expect("undo");
        assert_eq!(prev, b"c");
        let again = u.redo(b"x").expect("redo");
        assert_eq!(again, b"current");
    }

    #[test]
    fn selection_shift_range() {
        let mut s = PageSelection::default();
        s.toggle(2, false, 5);
        s.toggle(4, true, 5);
        let v = s.selected_vec();
        assert_eq!(v, vec![2, 3, 4]);
    }

    #[test]
    fn reorder_thumb() {
        let mut order = vec![1u32, 2, 3];
        let mut sel = PageSelection::default();
        sel.apply_reorder_drag(0, 2, &mut order);
        assert_eq!(order, vec![2, 3, 1]);
    }

    #[test]
    fn split_ranges_groups() {
        let groups = parse_tool_split_ranges("1-2;3,5-7").expect("parse");
        assert_eq!(groups.len(), 3);
    }

    #[test]
    fn screen_to_pdf_coords() {
        let rect = screen_drag_to_pdf_rect(
            ScreenPoint { x: 10.0, y: 10.0 },
            ScreenPoint { x: 110.0, y: 60.0 },
            0.0,
            0.0,
            1.0,
            792.0,
        );
        assert!(rect.urx > rect.llx);
        assert!(rect.ury > rect.lly);
        let (x, y) = screen_to_page_pt(ScreenPoint { x: 0.0, y: 792.0 }, 0.0, 0.0, 1.0, 792.0);
        assert!((y - 0.0).abs() < 0.01);
        let _ = x;
    }
}
