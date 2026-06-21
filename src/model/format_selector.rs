use tracing::warn;

#[derive(Debug, Clone)]
pub struct FileCandidate {
    pub name: String,
    pub size: u64,
}

/// Quality rank for GGUF quantization levels. Higher = better quality.
pub fn quant_quality_rank(name: &str) -> u8 {
    let n = name.to_lowercase();
    if n.contains("q8_0") { 11 }
    else if n.contains("q6_k") { 10 }
    else if n.contains("q5_k_m") || n.contains("q5km") { 9 }
    else if n.contains("q5_k_s") || n.contains("q5ks") { 8 }
    else if n.contains("q4_k_m") || n.contains("q4km") { 7 }
    else if n.contains("q4_k_s") || n.contains("q4ks") { 6 }
    else if n.contains("q4_0") { 5 }
    else if n.contains("q3_k_l") { 4 }
    else if n.contains("q3_k_m") { 3 }
    else if n.contains("q3_k_s") { 2 }
    else if n.contains("q2_k") { 1 }
    else { 0 }
}

/// Select the best-fitting GGUF from candidates given RAM and optional VRAM budget.
/// Returns the full filename of the winner, or None if no .gguf files exist.
/// Falls back to smallest .gguf if nothing fits (machine too small).
pub fn select_best_gguf(
    candidates: &[FileCandidate],
    ram_budget_bytes: u64,
    vram_bytes: Option<u64>,
) -> Option<String> {
    let gguf: Vec<&FileCandidate> = candidates
        .iter()
        .filter(|f| f.name.to_lowercase().ends_with(".gguf"))
        .collect();

    if gguf.is_empty() {
        return None;
    }

    let effective_budget = vram_bytes.map_or(ram_budget_bytes, |v| v.max(ram_budget_bytes));
    // Reserve 25% for KV cache + runtime overhead
    let budget = effective_budget * 75 / 100;

    let fits: Vec<&FileCandidate> = gguf.iter().copied().filter(|f| f.size <= budget).collect();

    let winner: Option<&FileCandidate> = if fits.is_empty() {
        warn!(
            "No GGUF fits budget ({}MB). Falling back to smallest.",
            budget / 1_048_576
        );
        gguf.iter().copied().min_by_key(|f| f.size)
    } else {
        fits.into_iter().max_by_key(|f| quant_quality_rank(&f.name))
    };

    winner.map(|f| f.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(name: &str, size_mb: u64) -> FileCandidate {
        FileCandidate { name: name.to_string(), size: size_mb * 1_048_576 }
    }

    #[test]
    fn test_quant_quality_rank_order() {
        assert!(quant_quality_rank("model-q8_0.gguf") > quant_quality_rank("model-q6_k.gguf"));
        assert!(quant_quality_rank("model-q6_k.gguf") > quant_quality_rank("model-q5_k_m.gguf"));
        assert!(quant_quality_rank("model-q4_k_m.gguf") > quant_quality_rank("model-q4_k_s.gguf"));
        assert!(quant_quality_rank("model-q4_0.gguf") > quant_quality_rank("model-q3_k_m.gguf"));
        assert!(quant_quality_rank("model-q2_k.gguf") > quant_quality_rank("model-unknown.gguf"));
    }

    #[test]
    fn test_select_best_gguf_picks_highest_quality_that_fits() {
        let candidates = vec![
            candidate("model-q2_k.gguf", 1_500),
            candidate("model-q4_k_m.gguf", 4_000),
            candidate("model-q8_0.gguf", 8_000),
        ];
        // 6GB budget → q4_k_m fits (4GB), q8_0 doesn't (8GB > 6GB*0.75=4.5GB)
        let budget = 6 * 1024 * 1024 * 1024u64;
        let result = select_best_gguf(&candidates, budget, None);
        assert_eq!(result.as_deref(), Some("model-q4_k_m.gguf"));
    }

    #[test]
    fn test_select_best_gguf_large_budget_picks_q8() {
        let candidates = vec![
            candidate("model-q4_k_m.gguf", 4_000),
            candidate("model-q8_0.gguf", 8_000),
        ];
        let budget = 16 * 1024 * 1024 * 1024u64;
        let result = select_best_gguf(&candidates, budget, None);
        assert_eq!(result.as_deref(), Some("model-q8_0.gguf"));
    }

    #[test]
    fn test_select_best_gguf_too_small_picks_smallest() {
        let candidates = vec![
            candidate("model-q4_k_m.gguf", 4_000),
            candidate("model-q8_0.gguf", 8_000),
            candidate("model-q2_k.gguf", 1_500),
        ];
        // 1GB budget — nothing fits, fallback to smallest
        let budget = 1024 * 1024 * 1024u64;
        let result = select_best_gguf(&candidates, budget, None);
        assert_eq!(result.as_deref(), Some("model-q2_k.gguf"));
    }

    #[test]
    fn test_select_best_gguf_vram_budget() {
        let candidates = vec![
            candidate("model-q4_k_m.gguf", 4_000),
            candidate("model-q8_0.gguf", 8_000),
        ];
        // 4GB RAM, 12GB VRAM → uses VRAM budget, q8_0 fits
        let ram = 4 * 1024 * 1024 * 1024u64;
        let vram = Some(12 * 1024 * 1024 * 1024u64);
        let result = select_best_gguf(&candidates, ram, vram);
        assert_eq!(result.as_deref(), Some("model-q8_0.gguf"));
    }

    #[test]
    fn test_select_best_gguf_no_gguf_returns_none() {
        let candidates = vec![
            candidate("model.safetensors", 10_000),
            candidate("config.json", 1),
        ];
        let result = select_best_gguf(&candidates, 16 * 1024 * 1024 * 1024, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_select_best_gguf_empty_candidates() {
        let result = select_best_gguf(&[], 16 * 1024 * 1024 * 1024, None);
        assert!(result.is_none());
    }
}
