use docsuite_ocr::{BBox, Candidate, Line, Word};

use crate::types::TextRun;

#[derive(Clone, Debug)]
struct WordBox {
    text: String,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

fn word_text(w: &Word) -> String {
    w.candidates
        .first()
        .map(|c| c.text.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_owned()
}

fn y_center(w: &WordBox) -> f64 {
    (w.y0 + w.y1) * 0.5
}

fn x_center(w: &WordBox) -> f64 {
    (w.x0 + w.x1) * 0.5
}

fn word_height(w: &WordBox) -> f64 {
    (w.y1 - w.y0).max(1.0)
}

fn word_width(w: &WordBox) -> f64 {
    (w.x1 - w.x0).max(1.0)
}

fn collect_from_lines(lines: &[Line]) -> Vec<WordBox> {
    let mut out = Vec::new();
    for line in lines {
        for w in &line.words {
            let text = word_text(w);
            if text.is_empty() {
                continue;
            }
            let b = &w.bbox;
            out.push(WordBox {
                text,
                x0: f64::from(b.x0),
                y0: f64::from(b.y0),
                x1: f64::from(b.x1),
                y1: f64::from(b.y1),
            });
        }
    }
    out
}

fn collect_from_runs(runs: &[TextRun]) -> Vec<WordBox> {
    runs.iter()
        .filter(|r| !r.text.is_empty())
        .map(|r| {
            let h = r.font_size.max(1.0);
            WordBox {
                text: r.text.clone(),
                x0: r.x,
                y0: r.y,
                x1: r.x + r.width.max(1.0),
                y1: r.y + h,
            }
        })
        .collect()
}

fn adjacent_same_metric(words: &[WordBox], metric: fn(&WordBox) -> f64, tol: f64) -> f64 {
    if words.len() < 2 {
        return 1.0;
    }
    let mut hits = 0usize;
    for i in 1..words.len() {
        if (metric(&words[i]) - metric(&words[i - 1])).abs() <= tol {
            hits += 1;
        }
    }
    hits as f64 / (words.len() - 1) as f64
}

fn normalize_axes(words: &mut [WordBox]) {
    if words.len() < 3 {
        return;
    }
    let tol = median_word_height(words) * 0.6;
    let mut by_y = words.to_vec();
    by_y.sort_by(|a, b| {
        y_center(a)
            .partial_cmp(&y_center(b))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut by_x = words.to_vec();
    by_x.sort_by(|a, b| {
        x_center(a)
            .partial_cmp(&x_center(b))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.y0.partial_cmp(&b.y0).unwrap_or(std::cmp::Ordering::Equal))
    });
    let y_coherence = adjacent_same_metric(&by_y, y_center, tol);
    let x_coherence = adjacent_same_metric(&by_x, x_center, tol);
    if x_coherence > y_coherence + 0.25 {
        for w in words.iter_mut() {
            std::mem::swap(&mut w.x0, &mut w.y0);
            std::mem::swap(&mut w.x1, &mut w.y1);
        }
    }
    let mut ys: Vec<f64> = words.iter().map(y_center).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let y_span = ys.last().copied().unwrap_or(0.0) - ys.first().copied().unwrap_or(0.0);
    if y_span > 1.0 {
        let mut sorted = words.to_vec();
        sorted.sort_by(|a, b| {
            y_center(a)
                .partial_cmp(&y_center(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let top_mean = sorted
            .iter()
            .take(sorted.len().min(3))
            .map(y_center)
            .sum::<f64>()
            / sorted.len().clamp(1, 3) as f64;
        let bot_mean = sorted
            .iter()
            .rev()
            .take(sorted.len().min(3))
            .map(y_center)
            .sum::<f64>()
            / sorted.len().clamp(1, 3) as f64;
        if top_mean > bot_mean + y_span * 0.15 {
            let min_y = words.iter().map(|w| w.y0).fold(f64::INFINITY, f64::min);
            let max_y = words.iter().map(|w| w.y1).fold(f64::NEG_INFINITY, f64::max);
            for w in words.iter_mut() {
                let cy = y_center(w);
                let h = word_height(w);
                let new_cy = min_y + max_y - cy;
                w.y0 = new_cy - h * 0.5;
                w.y1 = new_cy + h * 0.5;
            }
        }
    }
    let mut xs: Vec<f64> = words.iter().map(x_center).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let x_span = xs.last().copied().unwrap_or(0.0) - xs.first().copied().unwrap_or(0.0);
    if x_span > 1.0 {
        let mut sorted = words.to_vec();
        sorted.sort_by(|a, b| {
            x_center(a)
                .partial_cmp(&x_center(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let left_mean = sorted
            .iter()
            .take(sorted.len().min(3))
            .map(x_center)
            .sum::<f64>()
            / sorted.len().clamp(1, 3) as f64;
        let right_mean = sorted
            .iter()
            .rev()
            .take(sorted.len().min(3))
            .map(x_center)
            .sum::<f64>()
            / sorted.len().clamp(1, 3) as f64;
        if left_mean > right_mean + x_span * 0.15 {
            let min_x = words.iter().map(|w| w.x0).fold(f64::INFINITY, f64::min);
            let max_x = words.iter().map(|w| w.x1).fold(f64::NEG_INFINITY, f64::max);
            for w in words.iter_mut() {
                let cx = x_center(w);
                let wd = word_width(w);
                let new_cx = min_x + max_x - cx;
                w.x0 = new_cx - wd * 0.5;
                w.x1 = new_cx + wd * 0.5;
            }
        }
    }
}

fn median_word_width(words: &[WordBox]) -> f64 {
    let mut ws: Vec<f64> = words.iter().map(word_width).collect();
    ws.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ws.get(ws.len() / 2).copied().unwrap_or(40.0)
}

fn median_word_height(words: &[WordBox]) -> f64 {
    let mut hs: Vec<f64> = words.iter().map(word_height).collect();
    hs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    hs.get(hs.len() / 2).copied().unwrap_or(12.0)
}

fn group_into_lines_sorted(words: &[WordBox], line_tol: f64) -> Vec<Vec<WordBox>> {
    if words.is_empty() {
        return Vec::new();
    }
    let mut sorted = words.to_vec();
    sorted.sort_by(|a, b| {
        y_center(a)
            .partial_cmp(&y_center(b))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal))
    });
    group_into_lines_walk(sorted, line_tol)
}

fn group_into_lines_preserve(words: &[WordBox], line_tol: f64) -> Vec<Vec<WordBox>> {
    if words.is_empty() {
        return Vec::new();
    }
    group_into_lines_walk(words.to_vec(), line_tol)
}

fn group_into_lines_walk(sorted: Vec<WordBox>, line_tol: f64) -> Vec<Vec<WordBox>> {
    let mut groups: Vec<Vec<WordBox>> = Vec::new();
    let mut current: Vec<WordBox> = Vec::new();
    let mut line_y = y_center(&sorted[0]);
    for w in sorted {
        if !current.is_empty() && (y_center(&w) - line_y).abs() > line_tol {
            current.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
            groups.push(std::mem::take(&mut current));
            line_y = y_center(&w);
        }
        current.push(w);
        let n = current.len() as f64;
        line_y = (line_y * (n - 1.0) + y_center(current.last().expect("word"))) / n;
    }
    if !current.is_empty() {
        current.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
        groups.push(current);
    }
    groups
}

fn group_into_lines(words: &[WordBox], line_tol: f64) -> Vec<Vec<WordBox>> {
    group_into_lines_sorted(words, line_tol)
}

fn largest_x_gap(line: &[WordBox]) -> Option<(f64, usize)> {
    if line.len() < 2 {
        return None;
    }
    let mut best_gap = 0.0;
    let mut split_at = 0;
    for i in 0..line.len() - 1 {
        let gap = line[i + 1].x0 - line[i].x1;
        if gap > best_gap {
            best_gap = gap;
            split_at = i + 1;
        }
    }
    if best_gap > 0.0 {
        Some((best_gap, split_at))
    } else {
        None
    }
}

fn split_two_columns(groups: Vec<Vec<WordBox>>, median_w: f64) -> Vec<Vec<WordBox>> {
    let gap_thresh = median_w * 1.5;
    let mut split: Vec<Vec<WordBox>> = Vec::new();
    for mut line in groups {
        if let Some((gap, at)) = largest_x_gap(&line) {
            if gap >= gap_thresh && at > 0 && at < line.len() {
                let right = line.split_off(at);
                split.push(line);
                split.push(right);
                continue;
            }
        }
        split.push(line);
    }
    split
}

fn two_column_page(words: &[WordBox], median_w: f64) -> Option<f64> {
    if words.len() < 4 {
        return None;
    }
    let mut xs: Vec<f64> = words.iter().map(x_center).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut best_gap = 0.0;
    let mut split_x = 0.0;
    for i in 0..xs.len() - 1 {
        let gap = xs[i + 1] - xs[i];
        if gap > best_gap {
            best_gap = gap;
            split_x = (xs[i] + xs[i + 1]) * 0.5;
        }
    }
    if best_gap < median_w * 2.0 {
        return None;
    }
    let left = words.iter().filter(|w| x_center(w) < split_x).count();
    let right = words.len() - left;
    if left >= 2 && right >= 2 {
        Some(split_x)
    } else {
        None
    }
}

fn order_word_boxes(mut words: Vec<WordBox>) -> Vec<WordBox> {
    if words.is_empty() {
        return words;
    }
    normalize_axes(&mut words);
    let line_tol = median_word_height(&words) * 0.6;
    let median_w = median_word_width(&words);
    if let Some(split_x) = two_column_page(&words, median_w) {
        let left: Vec<WordBox> = words
            .iter()
            .filter(|w| x_center(w) < split_x)
            .cloned()
            .collect();
        let right: Vec<WordBox> = words
            .iter()
            .filter(|w| x_center(w) >= split_x)
            .cloned()
            .collect();
        let mut out = Vec::new();
        out.append(&mut order_word_boxes_single(left, line_tol, median_w));
        out.append(&mut order_word_boxes_single(right, line_tol, median_w));
        return out;
    }
    order_word_boxes_single(words, line_tol, median_w)
}

fn order_word_boxes_single(words: Vec<WordBox>, line_tol: f64, median_w: f64) -> Vec<WordBox> {
    let groups = group_into_lines(&words, line_tol);
    let groups = split_two_columns(groups, median_w);
    let mut out = Vec::new();
    for line in groups {
        out.extend(line);
    }
    out
}

fn line_from_words(words: &[WordBox]) -> Line {
    let ocr_words: Vec<Word> = words
        .iter()
        .map(|w| Word {
            bbox: BBox {
                x0: w.x0 as f32,
                y0: w.y0 as f32,
                x1: w.x1 as f32,
                y1: w.y1 as f32,
            },
            candidates: vec![Candidate {
                text: w.text.clone(),
                confidence: 1.0,
            }],
        })
        .collect();
    let x0 = words.iter().map(|w| w.x0).fold(f64::INFINITY, f64::min) as f32;
    let y0 = words.iter().map(|w| w.y0).fold(f64::INFINITY, f64::min) as f32;
    let x1 = words.iter().map(|w| w.x1).fold(f64::NEG_INFINITY, f64::max) as f32;
    let y1 = words.iter().map(|w| w.y1).fold(f64::NEG_INFINITY, f64::max) as f32;
    Line {
        bbox: BBox { x0, y0, x1, y1 },
        words: ocr_words,
    }
}

pub fn order_ocr_lines(lines: &[Line]) -> Vec<Line> {
    let words = collect_from_lines(lines);
    if words.is_empty() {
        return Vec::new();
    }
    let line_tol = median_word_height(&words) * 0.6;
    let ordered = order_word_boxes(words);
    let groups = group_into_lines_preserve(&ordered, line_tol);
    groups.iter().map(|g| line_from_words(g)).collect()
}

pub fn order_text_runs(runs: &[TextRun]) -> Vec<TextRun> {
    let words = collect_from_runs(runs);
    if words.is_empty() {
        return Vec::new();
    }
    let ordered = order_word_boxes(words);
    let mut run_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, r) in runs.iter().enumerate() {
        run_map.insert(format!("{}:{}:{}", r.text, r.x, r.y), i);
    }
    let mut out = Vec::new();
    for w in ordered {
        let key = format!("{}:{}:{}", w.text, w.x0, w.y0);
        if let Some(&idx) = run_map.get(&key) {
            out.push(runs[idx].clone());
        } else if let Some(r) = runs.iter().find(|r| r.text == w.text) {
            out.push(r.clone());
        }
    }
    if out.len() < runs.len() {
        for r in runs {
            if !out
                .iter()
                .any(|o| o.text == r.text && (o.x - r.x).abs() < 0.5)
            {
                out.push(r.clone());
            }
        }
    }
    out
}

pub fn ordered_plain_from_runs(runs: &[TextRun]) -> String {
    let words = order_word_boxes(collect_from_runs(runs));
    let line_tol = if words.is_empty() {
        8.0
    } else {
        median_word_height(&words) * 0.6
    };
    let groups = group_into_lines_preserve(&words, line_tol);
    let mut out = String::new();
    for line in groups {
        for (i, w) in line.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(&w.text);
        }
        if !line.is_empty() {
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use docsuite_ocr::{BBox, Candidate, Line, Word};

    fn w(text: &str, x0: f32, y0: f32, x1: f32, y1: f32) -> Word {
        Word {
            bbox: BBox { x0, y0, x1, y1 },
            candidates: vec![Candidate {
                text: text.to_owned(),
                confidence: 0.99,
            }],
        }
    }

    #[test]
    fn scrambled_words_read_left_to_right() {
        let lines = vec![Line {
            bbox: BBox {
                x0: 0.0,
                y0: 0.0,
                x1: 400.0,
                y1: 30.0,
            },
            words: vec![
                w("OF", 120.0, 4.0, 150.0, 24.0),
                w("DEPARTMENT", 10.0, 2.0, 100.0, 22.0),
                w("HIGHER", 160.0, 5.0, 220.0, 25.0),
                w("EDUCATION", 230.0, 1.0, 320.0, 21.0),
            ],
        }];
        let ordered = order_ocr_lines(&lines);
        let text: String = ordered
            .iter()
            .flat_map(|l| l.words.iter())
            .filter_map(|w| w.candidates.first().map(|c| c.text.as_str()))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(text, "DEPARTMENT OF HIGHER EDUCATION");
    }

    #[test]
    fn ragged_baseline_still_one_line() {
        let lines = vec![Line {
            bbox: BBox {
                x0: 0.0,
                y0: 0.0,
                x1: 300.0,
                y1: 40.0,
            },
            words: vec![
                w("alpha", 10.0, 10.0, 60.0, 28.0),
                w("beta", 70.0, 14.0, 110.0, 32.0),
                w("gamma", 120.0, 8.0, 170.0, 26.0),
            ],
        }];
        let ordered = order_ocr_lines(&lines);
        assert_eq!(ordered.len(), 1);
        let joined: String = ordered[0]
            .words
            .iter()
            .filter_map(|w| w.candidates.first().map(|c| c.text.as_str()))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(joined, "alpha beta gamma");
    }

    #[test]
    fn two_column_left_then_right() {
        let lines = vec![Line {
            bbox: BBox {
                x0: 0.0,
                y0: 0.0,
                x1: 500.0,
                y1: 80.0,
            },
            words: vec![
                w("LeftA", 20.0, 10.0, 80.0, 30.0),
                w("RightA", 280.0, 10.0, 340.0, 30.0),
                w("LeftB", 20.0, 50.0, 80.0, 70.0),
                w("RightB", 280.0, 50.0, 340.0, 70.0),
            ],
        }];
        let ordered = order_ocr_lines(&lines);
        let flat: Vec<String> = ordered
            .iter()
            .flat_map(|l| l.words.iter())
            .filter_map(|w| w.candidates.first().map(|c| c.text.clone()))
            .collect();
        let joined = flat.join("|");
        assert!(
            joined.contains("LeftA|LeftB")
                && joined.find("LeftB").unwrap() < joined.find("RightA").unwrap(),
            "got {joined}"
        );
    }
}
