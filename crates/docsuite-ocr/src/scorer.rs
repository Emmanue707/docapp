use std::collections::HashSet;

use crate::predictor::{apply_confusions_for_scoring, predict_type};
use crate::types::{Candidate, FieldType, ScoredCandidate};

pub fn score_candidates(
    candidates: &[Candidate],
    lexicon: Option<&HashSet<String>>,
) -> Vec<ScoredCandidate> {
    let mut scored: Vec<ScoredCandidate> = candidates
        .iter()
        .map(|c| {
            let guess = predict_type(&c.text);
            let confused = apply_confusions_for_scoring(&c.text, guess.field_type);
            let confused_guess = if confused != c.text {
                predict_type(&confused)
            } else {
                guess.clone()
            };
            let best = if confused_guess.score > guess.score {
                confused_guess
            } else {
                guess
            };
            let lex_boost = lexicon_hit(lexicon, &c.text, best.field_type);
            let type_boost = best.score;
            let combined = (c.confidence * 0.55 + type_boost * 0.35 + lex_boost * 0.1).min(1.0);
            ScoredCandidate {
                text: c.text.clone(),
                confidence: c.confidence,
                combined_score: combined,
                guess: Some(best),
            }
        })
        .collect();
    scored.sort_by(|a, b| {
        b.combined_score
            .partial_cmp(&a.combined_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
}

fn lexicon_hit(lexicon: Option<&HashSet<String>>, text: &str, field: FieldType) -> f32 {
    let Some(words) = lexicon else {
        return 0.0;
    };
    let key = text.trim().to_ascii_lowercase();
    if words.contains(&key) {
        return 1.0;
    }
    if matches!(field, FieldType::Text) {
        return 0.0;
    }
    0.0
}

pub fn rerank_top(
    candidates: &[Candidate],
    lexicon: Option<&HashSet<String>>,
) -> Vec<ScoredCandidate> {
    score_candidates(candidates, lexicon)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FieldType;

    #[test]
    fn lower_raw_wins_when_valid_date() {
        let candidates = vec![
            Candidate {
                text: "l2/Ol/2O26".to_owned(),
                confidence: 0.92,
            },
            Candidate {
                text: "12/01/2026".to_owned(),
                confidence: 0.75,
            },
        ];
        let ranked = score_candidates(&candidates, None);
        assert_eq!(ranked[0].text, "12/01/2026");
        assert_eq!(ranked[0].confidence, 0.75);
        assert_eq!(
            ranked[0].guess.as_ref().map(|g| g.field_type),
            Some(FieldType::Date)
        );
        assert!(ranked[0].combined_score > ranked[1].combined_score);
    }
}
