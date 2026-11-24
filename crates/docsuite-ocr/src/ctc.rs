const CHARS: &[char] = &[
    '!', '"', '#', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', '0', '1', '2', '3', '4', '5',
    '6', '7', '8', '9', ':', ';', '?', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e',
    'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x',
    'y', 'z',
];

pub const NUM_CLASSES: usize = CHARS.len() + 1;
pub const BLANK: usize = CHARS.len();

#[derive(Clone)]
struct Beam {
    text: String,
    last: usize,
    blank_ending: bool,
    logp: f32,
}

pub struct DecodedCandidate {
    pub text: String,
    pub confidence: f32,
}

fn log_sum_exp(a: f32, b: f32) -> f32 {
    if a > b {
        a + (1.0 + (b - a).exp()).ln()
    } else {
        b + (1.0 + (a - b).exp()).ln()
    }
}

pub fn beam_top_n(
    log_probs: &[f32],
    seq_len: usize,
    width: usize,
    top_n: usize,
) -> Vec<DecodedCandidate> {
    if seq_len == 0 || top_n == 0 {
        return Vec::new();
    }
    debug_assert_eq!(width, NUM_CLASSES);
    let mut beams = vec![Beam {
        text: String::new(),
        last: BLANK,
        blank_ending: true,
        logp: 0.0,
    }];
    let beam_w = width.min(32);
    for t in 0..seq_len {
        let offset = t * width;
        let step = &log_probs[offset..offset + width];
        let mut next = Vec::new();
        for beam in &beams {
            let p_blank = step[BLANK];
            next.push(Beam {
                text: beam.text.clone(),
                last: beam.last,
                blank_ending: true,
                logp: beam.logp + p_blank,
            });
            for (i, &ch) in CHARS.iter().enumerate() {
                let lp = step[i];
                if i == beam.last && !beam.blank_ending {
                    let mut extended = beam.text.clone();
                    extended.push(ch);
                    next.push(Beam {
                        text: extended,
                        last: i,
                        blank_ending: false,
                        logp: beam.logp + lp,
                    });
                } else if i != beam.last || beam.blank_ending {
                    let mut extended = beam.text.clone();
                    if beam.blank_ending || i != beam.last {
                        extended.push(ch);
                    }
                    next.push(Beam {
                        text: extended,
                        last: i,
                        blank_ending: false,
                        logp: beam.logp + lp,
                    });
                }
            }
        }
        next.sort_by(|a, b| {
            b.logp
                .partial_cmp(&a.logp)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut merged: Vec<Beam> = Vec::new();
        for b in next {
            if let Some(prev) = merged.iter_mut().find(|p: &&mut Beam| p.text == b.text) {
                prev.logp = log_sum_exp(prev.logp, b.logp);
            } else if merged.len() < beam_w {
                merged.push(b);
            }
        }
        beams = merged;
    }
    let mut out: Vec<DecodedCandidate> = beams
        .into_iter()
        .map(|b| {
            let conf = (b.logp / seq_len as f32).exp().min(1.0);
            DecodedCandidate {
                text: b.text,
                confidence: conf.max(1e-6),
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out.dedup_by(|a, b| a.text == b.text);
    out.truncate(top_n);
    out
}

pub fn greedy_decode(log_probs: &[f32], seq_len: usize, width: usize) -> DecodedCandidate {
    let mut prev = BLANK;
    let mut text = String::new();
    let mut logp = 0.0f32;
    for t in 0..seq_len {
        let offset = t * width;
        let step = &log_probs[offset..offset + width];
        let (best_idx, &best_lp) = step
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((BLANK, &step[BLANK]));
        logp += best_lp;
        if best_idx != BLANK
            && best_idx != prev
            && let Some(&ch) = CHARS.get(best_idx)
        {
            text.push(ch);
        }
        prev = best_idx;
    }
    let conf = (logp / seq_len as f32).exp().min(1.0);
    DecodedCandidate {
        text,
        confidence: conf.max(1e-6),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_blank_run(seq_len: usize) -> Vec<f32> {
        let mut v = vec![-20.0f32; seq_len * NUM_CLASSES];
        for t in 0..seq_len {
            v[t * NUM_CLASSES + BLANK] = 0.0;
        }
        v
    }

    #[test]
    fn blank_sequence_decodes_empty() {
        let lp = synth_blank_run(8);
        let d = greedy_decode(&lp, 8, NUM_CLASSES);
        assert!(d.text.is_empty());
    }

    #[test]
    fn single_char_spike() {
        let mut lp = synth_blank_run(4);
        let a_idx = CHARS.iter().position(|&c| c == 'a').unwrap();
        lp[NUM_CLASSES + a_idx] = 0.0;
        lp[NUM_CLASSES + BLANK] = -10.0;
        let d = greedy_decode(&lp, 4, NUM_CLASSES);
        assert_eq!(d.text, "a");
    }
}
