//! AI-powered model recommendation engine.
//!
//! Recommends the best model + quantization based on:
//! - Available hardware (RAM, GPU, CPU)
//! - Task type (chat, code, embedding, creative)
//! - Quality vs speed preference

use serde::{Deserialize, Serialize};

/// Task type for model selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    Chat,
    Code,
    Embedding,
    Creative,
    Reasoning,
    Summarization,
    Translation,
}

/// Hardware profile for recommendations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub ram_gb: u64,
    pub gpu_vram_gb: Option<u64>,
    pub cpu_cores: usize,
    pub has_avx2: bool,
    pub has_neon: bool,
}

impl Default for HardwareProfile {
    fn default() -> Self {
        Self {
            ram_gb: 8,
            gpu_vram_gb: None,
            cpu_cores: 4,
            has_avx2: true,
            has_neon: false,
        }
    }
}

/// A model recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub model_name: String,
    pub parameter_size: String,
    pub quantization: String,
    pub estimated_ram_gb: f64,
    pub estimated_speed_tok_s: f64,
    pub quality_score: f64,
    pub reason: String,
}

/// Model recommender.
pub struct ModelRecommender;

impl ModelRecommender {
    pub fn new() -> Self {
        Self
    }

    /// Recommend models for a given hardware profile and task.
    pub fn recommend(
        &self,
        hardware: &HardwareProfile,
        task: TaskType,
        max_results: usize,
    ) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();
        let available_ram = hardware.ram_gb as f64 * 0.7; // Leave 30% for OS

        // Determine model sizes that fit in RAM
        let model_configs = self.get_model_configs(task);

        for (name, params, quant, ram_needed, quality) in model_configs {
            if ram_needed <= available_ram {
                let speed = self.estimate_speed(hardware, ram_needed);
                recommendations.push(Recommendation {
                    model_name: name.to_string(),
                    parameter_size: params.to_string(),
                    quantization: quant.to_string(),
                    estimated_ram_gb: ram_needed,
                    estimated_speed_tok_s: speed,
                    quality_score: quality,
                    reason: self.generate_reason(hardware, task, ram_needed, quality),
                });
            }
        }

        // Sort by quality (descending), then by speed
        recommendations.sort_by(|a, b| {
            b.quality_score
                .partial_cmp(&a.quality_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        recommendations.truncate(max_results);
        recommendations
    }

    fn get_model_configs(&self, task: TaskType) -> Vec<(&str, &str, &str, f64, f64)> {
        // (model_name, param_size, quantization, ram_gb_needed, quality_score)
        match task {
            TaskType::Chat => vec![
                ("llama3.2", "3B", "Q4_K_M", 2.5, 0.75),
                ("llama3.2", "3B", "Q8_0", 3.5, 0.82),
                ("llama3.1", "8B", "Q4_K_M", 5.0, 0.88),
                ("llama3.1", "8B", "Q5_K_M", 6.0, 0.90),
                ("llama3.1", "8B", "Q8_0", 8.5, 0.92),
                ("llama3.1", "70B", "Q4_K_M", 42.0, 0.96),
            ],
            TaskType::Code => vec![
                ("codellama", "7B", "Q4_K_M", 4.5, 0.85),
                ("codellama", "13B", "Q4_K_M", 8.0, 0.90),
                ("deepseek-coder", "6.7B", "Q4_K_M", 4.0, 0.88),
                ("deepseek-coder", "33B", "Q4_K_M", 20.0, 0.95),
            ],
            TaskType::Embedding => vec![
                ("nomic-embed-text", "137M", "F16", 0.5, 0.85),
                ("mxbai-embed-large", "335M", "F16", 1.0, 0.92),
            ],
            TaskType::Creative => vec![
                ("llama3.1", "8B", "Q5_K_M", 6.0, 0.85),
                ("mistral", "7B", "Q5_K_M", 5.5, 0.87),
                ("llama3.1", "70B", "Q4_K_M", 42.0, 0.95),
            ],
            _ => vec![
                ("llama3.2", "3B", "Q4_K_M", 2.5, 0.75),
                ("llama3.1", "8B", "Q4_K_M", 5.0, 0.88),
            ],
        }
    }

    fn estimate_speed(&self, hardware: &HardwareProfile, ram_needed: f64) -> f64 {
        let base_speed = if let Some(vram) = hardware.gpu_vram_gb {
            if vram as f64 >= ram_needed {
                80.0 // Full GPU offload
            } else {
                40.0 // Partial GPU
            }
        } else {
            15.0 // CPU only
        };

        // Adjust for CPU capabilities
        let cpu_factor = if hardware.has_avx2 || hardware.has_neon {
            1.2
        } else {
            1.0
        };

        // Larger models are slower
        let size_factor = 1.0 / (ram_needed / 5.0).max(1.0).sqrt();

        base_speed * cpu_factor * size_factor
    }

    fn generate_reason(
        &self,
        hardware: &HardwareProfile,
        task: TaskType,
        ram_needed: f64,
        quality: f64,
    ) -> String {
        let ram_pct = (ram_needed / hardware.ram_gb as f64 * 100.0) as u32;
        let gpu_str = if hardware.gpu_vram_gb.is_some() {
            "GPU acceleration available"
        } else {
            "CPU inference"
        };

        format!(
            "{:?} task, uses ~{}% of RAM, quality {:.0}%, {}",
            task,
            ram_pct,
            quality * 100.0,
            gpu_str
        )
    }

    /// Recommend the best GGUF file from actual file candidates.
    /// Works for any repo without needing to know model name or param count.
    /// Uses file sizes to fit within hardware budget.
    pub fn recommend_from_files(
        &self,
        candidates: &[crate::model::format_selector::FileCandidate],
        hardware: &HardwareProfile,
    ) -> Option<crate::model::format_selector::FileCandidate> {
        let ram_bytes = hardware.ram_gb * 1024 * 1024 * 1024;
        let vram_bytes = hardware.gpu_vram_gb.map(|g| g * 1024 * 1024 * 1024);
        let ram_budget = ram_bytes.saturating_sub(2 * 1024 * 1024 * 1024);

        let winner_name =
            crate::model::format_selector::select_best_gguf(candidates, ram_budget, vram_bytes)?;

        candidates
            .iter()
            .find(|f| f.name == winner_name)
            .map(|f| crate::model::format_selector::FileCandidate {
                name: f.name.clone(),
                size: f.size,
            })
    }
}

impl Default for ModelRecommender {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommend_for_8gb_ram() {
        let recommender = ModelRecommender::new();
        let hw = HardwareProfile {
            ram_gb: 8,
            ..Default::default()
        };
        let recs = recommender.recommend(&hw, TaskType::Chat, 5);
        assert!(!recs.is_empty());
        // All recommendations should fit in RAM
        for rec in &recs {
            assert!(rec.estimated_ram_gb <= 8.0 * 0.7);
        }
    }

    #[test]
    fn test_recommend_for_16gb_ram() {
        let recommender = ModelRecommender::new();
        let hw = HardwareProfile {
            ram_gb: 16,
            ..Default::default()
        };
        let recs = recommender.recommend(&hw, TaskType::Chat, 5);
        assert!(recs.len() > 1);
        // Should include 8B models
        assert!(recs.iter().any(|r| r.parameter_size == "8B"));
    }

    #[test]
    fn test_recommend_for_4gb_ram() {
        let recommender = ModelRecommender::new();
        let hw = HardwareProfile {
            ram_gb: 4,
            ..Default::default()
        };
        let recs = recommender.recommend(&hw, TaskType::Chat, 5);
        // Should only recommend small models
        for rec in &recs {
            assert!(rec.estimated_ram_gb <= 3.0);
        }
    }

    #[test]
    fn test_recommend_with_gpu() {
        let recommender = ModelRecommender::new();
        let hw = HardwareProfile {
            ram_gb: 16,
            gpu_vram_gb: Some(8),
            ..Default::default()
        };
        let recs = recommender.recommend(&hw, TaskType::Chat, 3);
        // GPU models should have higher speed estimates
        assert!(recs.iter().all(|r| r.estimated_speed_tok_s > 10.0));
    }

    #[test]
    fn test_recommend_code_task() {
        let recommender = ModelRecommender::new();
        let hw = HardwareProfile {
            ram_gb: 16,
            ..Default::default()
        };
        let recs = recommender.recommend(&hw, TaskType::Code, 3);
        assert!(!recs.is_empty());
        assert!(recs
            .iter()
            .any(|r| r.model_name.contains("code") || r.model_name.contains("deepseek")));
    }

    #[test]
    fn test_recommend_embedding_task() {
        let recommender = ModelRecommender::new();
        let hw = HardwareProfile {
            ram_gb: 4,
            ..Default::default()
        };
        let recs = recommender.recommend(&hw, TaskType::Embedding, 3);
        assert!(!recs.is_empty());
        // Embedding models should be small
        for rec in &recs {
            assert!(rec.estimated_ram_gb <= 2.0);
        }
    }

    #[test]
    fn test_recommendations_sorted_by_quality() {
        let recommender = ModelRecommender::new();
        let hw = HardwareProfile {
            ram_gb: 16,
            ..Default::default()
        };
        let recs = recommender.recommend(&hw, TaskType::Chat, 5);
        for w in recs.windows(2) {
            assert!(w[0].quality_score >= w[1].quality_score);
        }
    }

    #[test]
    fn test_max_results_limit() {
        let recommender = ModelRecommender::new();
        let hw = HardwareProfile {
            ram_gb: 64,
            ..Default::default()
        };
        let recs = recommender.recommend(&hw, TaskType::Chat, 2);
        assert!(recs.len() <= 2);
    }

    #[test]
    fn test_recommendation_has_reason() {
        let recommender = ModelRecommender::new();
        let hw = HardwareProfile::default();
        let recs = recommender.recommend(&hw, TaskType::Chat, 1);
        assert!(!recs.is_empty());
        assert!(!recs[0].reason.is_empty());
    }

    #[test]
    fn test_hardware_profile_default() {
        let hw = HardwareProfile::default();
        assert_eq!(hw.ram_gb, 8);
        assert!(hw.gpu_vram_gb.is_none());
        assert_eq!(hw.cpu_cores, 4);
    }
}
