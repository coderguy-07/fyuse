//! Token sampling strategies — temperature, top-k, top-p, min-p, repetition penalty.

use crate::error::{FuseError, Result};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Sampling parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingParams {
    pub temperature: f32,
    pub top_k: usize,
    pub top_p: f32,
    pub min_p: f32,
    pub repetition_penalty: f32,
    pub frequency_penalty: f32,
    pub presence_penalty: f32,
}

impl Default for SamplingParams {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_k: 40,
            top_p: 0.9,
            min_p: 0.0,
            repetition_penalty: 1.0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
        }
    }
}

/// Token sampler that applies various sampling strategies to logits.
pub struct Sampler {
    params: SamplingParams,
}

impl Sampler {
    pub fn new(params: SamplingParams) -> Self {
        Self { params }
    }

    /// Sample a token ID from raw logits.
    pub fn sample(&self, logits: &[f32], previous_tokens: &[u32]) -> Result<u32> {
        if logits.is_empty() {
            return Err(FuseError::InferenceError("Empty logits".to_string()));
        }

        let mut logits = logits.to_vec();

        // Apply repetition penalty
        if self.params.repetition_penalty != 1.0 {
            apply_repetition_penalty(&mut logits, previous_tokens, self.params.repetition_penalty);
        }

        // Apply frequency and presence penalties
        if self.params.frequency_penalty != 0.0 || self.params.presence_penalty != 0.0 {
            apply_frequency_presence_penalty(
                &mut logits,
                previous_tokens,
                self.params.frequency_penalty,
                self.params.presence_penalty,
            );
        }

        // Temperature = 0 means greedy
        if self.params.temperature == 0.0 {
            return Ok(argmax(&logits));
        }

        // Apply temperature
        if self.params.temperature != 1.0 {
            for logit in &mut logits {
                *logit /= self.params.temperature;
            }
        }

        // Convert to probabilities
        let mut probs = softmax(&logits);

        // Apply top-k
        if self.params.top_k > 0 && self.params.top_k < probs.len() {
            apply_top_k(&mut probs, self.params.top_k);
        }

        // Apply top-p (nucleus sampling)
        if self.params.top_p < 1.0 {
            apply_top_p(&mut probs, self.params.top_p);
        }

        // Apply min-p
        if self.params.min_p > 0.0 {
            apply_min_p(&mut probs, self.params.min_p);
        }

        // Renormalize
        let sum: f32 = probs.iter().sum();
        if sum > 0.0 {
            for p in &mut probs {
                *p /= sum;
            }
        } else {
            // Fallback: uniform over all tokens
            let uniform = 1.0 / probs.len() as f32;
            probs.fill(uniform);
        }

        // Weighted random sampling
        let mut rng = rand::rng();
        let r: f32 = rng.random();
        let mut cumsum = 0.0;
        for (i, &p) in probs.iter().enumerate() {
            cumsum += p;
            if cumsum >= r {
                return Ok(i as u32);
            }
        }

        Ok((probs.len() - 1) as u32)
    }
}

/// Argmax over a slice.
fn argmax(values: &[f32]) -> u32 {
    values
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i as u32)
        .unwrap_or(0)
}

/// Softmax transformation.
fn softmax(logits: &[f32]) -> Vec<f32> {
    let max = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = logits.iter().map(|&x| (x - max).exp()).collect();
    let sum: f32 = exps.iter().sum();
    exps.into_iter().map(|x| x / sum).collect()
}

/// Apply repetition penalty to logits for tokens that appeared before.
fn apply_repetition_penalty(logits: &mut [f32], previous_tokens: &[u32], penalty: f32) {
    for &token in previous_tokens {
        let idx = token as usize;
        if idx < logits.len() {
            if logits[idx] > 0.0 {
                logits[idx] /= penalty;
            } else {
                logits[idx] *= penalty;
            }
        }
    }
}

/// Apply frequency and presence penalties.
fn apply_frequency_presence_penalty(
    logits: &mut [f32],
    previous_tokens: &[u32],
    frequency_penalty: f32,
    presence_penalty: f32,
) {
    let mut counts = std::collections::HashMap::new();
    for &token in previous_tokens {
        *counts.entry(token).or_insert(0u32) += 1;
    }
    for (&token, &count) in &counts {
        let idx = token as usize;
        if idx < logits.len() {
            logits[idx] -= frequency_penalty * count as f32;
            logits[idx] -= presence_penalty;
        }
    }
}

/// Zero out probabilities below the top-k threshold.
fn apply_top_k(probs: &mut [f32], k: usize) {
    let mut indexed: Vec<(usize, f32)> = probs.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let threshold = indexed.get(k).map(|x| x.1).unwrap_or(0.0);
    for p in probs.iter_mut() {
        if *p < threshold {
            *p = 0.0;
        }
    }
}

/// Zero out probabilities outside the top-p nucleus.
fn apply_top_p(probs: &mut [f32], top_p: f32) {
    let mut indexed: Vec<(usize, f32)> = probs.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut cumsum = 0.0;
    let mut cutoff_idx = indexed.len();
    for (i, &(_, p)) in indexed.iter().enumerate() {
        cumsum += p;
        if cumsum > top_p {
            cutoff_idx = i + 1;
            break;
        }
    }

    // Zero out tokens beyond the cutoff
    let kept: std::collections::HashSet<usize> =
        indexed[..cutoff_idx].iter().map(|&(i, _)| i).collect();
    for (i, p) in probs.iter_mut().enumerate() {
        if !kept.contains(&i) {
            *p = 0.0;
        }
    }
}

/// Zero out probabilities below min_p * max_prob.
fn apply_min_p(probs: &mut [f32], min_p: f32) {
    let max_prob = probs.iter().cloned().fold(0.0f32, f32::max);
    let threshold = min_p * max_prob;
    for p in probs.iter_mut() {
        if *p < threshold {
            *p = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greedy_sampling() {
        let sampler = Sampler::new(SamplingParams {
            temperature: 0.0,
            ..Default::default()
        });

        // Token 2 has highest logit
        let logits = vec![1.0, 2.0, 5.0, 0.5];
        let token = sampler.sample(&logits, &[]).unwrap();
        assert_eq!(token, 2);
    }

    #[test]
    fn test_temperature_zero_is_deterministic() {
        let sampler = Sampler::new(SamplingParams {
            temperature: 0.0,
            ..Default::default()
        });

        let logits = vec![1.0, 3.0, 2.0, 0.5];
        let results: Vec<u32> = (0..10)
            .map(|_| sampler.sample(&logits, &[]).unwrap())
            .collect();

        // All should be the same (deterministic)
        assert!(results.iter().all(|&r| r == results[0]));
        assert_eq!(results[0], 1); // Index of max logit
    }

    #[test]
    fn test_top_k_limits_vocabulary() {
        let sampler = Sampler::new(SamplingParams {
            temperature: 1.0,
            top_k: 2,
            top_p: 1.0,
            min_p: 0.0,
            ..Default::default()
        });

        // With top_k=2, only the top 2 tokens should be sampled
        let logits = vec![0.1, 10.0, 9.0, 0.1, 0.1];
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            let token = sampler.sample(&logits, &[]).unwrap();
            seen.insert(token);
        }

        // Should mostly see tokens 1 and 2 (highest logits)
        assert!(seen.contains(&1));
        assert!(seen.contains(&2));
    }

    #[test]
    fn test_repetition_penalty() {
        let logits = vec![1.0, 1.0, 1.0, 1.0];
        let mut modified = logits.clone();
        apply_repetition_penalty(&mut modified, &[0, 1], 2.0);

        // Tokens 0 and 1 should be penalized (divided by 2.0)
        assert!((modified[0] - 0.5).abs() < 1e-6);
        assert!((modified[1] - 0.5).abs() < 1e-6);
        assert!((modified[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_softmax_sums_to_one() {
        let logits = vec![1.0, 2.0, 3.0, 4.0];
        let probs = softmax(&logits);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_softmax_monotonic() {
        let logits = vec![1.0, 2.0, 3.0, 4.0];
        let probs = softmax(&logits);
        for i in 1..probs.len() {
            assert!(probs[i] > probs[i - 1]);
        }
    }

    #[test]
    fn test_top_p_filtering() {
        let mut probs = vec![0.5, 0.3, 0.1, 0.05, 0.05];
        apply_top_p(&mut probs, 0.8);

        // Top-p=0.8: should keep 0.5 + 0.3 = 0.8
        assert!(probs[0] > 0.0);
        assert!(probs[1] > 0.0);
        // Others should be zeroed
        assert_eq!(probs[3], 0.0);
        assert_eq!(probs[4], 0.0);
    }

    #[test]
    fn test_min_p_filtering() {
        let mut probs = vec![0.5, 0.3, 0.01, 0.005];
        apply_min_p(&mut probs, 0.1);

        // min_p=0.1, max=0.5, threshold=0.05
        assert!(probs[0] > 0.0);
        assert!(probs[1] > 0.0);
        assert_eq!(probs[2], 0.0); // 0.01 < 0.05
        assert_eq!(probs[3], 0.0); // 0.005 < 0.05
    }

    #[test]
    fn test_empty_logits() {
        let sampler = Sampler::new(SamplingParams::default());
        assert!(sampler.sample(&[], &[]).is_err());
    }

    #[test]
    fn test_frequency_presence_penalty() {
        let mut logits = vec![1.0, 1.0, 1.0, 1.0];
        // Token 0 appears 3 times, token 1 appears 1 time
        apply_frequency_presence_penalty(&mut logits, &[0, 0, 0, 1], 0.5, 0.1);

        // Token 0: 1.0 - 0.5*3 - 0.1 = -0.6
        assert!((logits[0] - (-0.6)).abs() < 1e-5);
        // Token 1: 1.0 - 0.5*1 - 0.1 = 0.4
        assert!((logits[1] - 0.4).abs() < 1e-5);
        // Token 2: unchanged
        assert!((logits[2] - 1.0).abs() < 1e-5);
    }
}
