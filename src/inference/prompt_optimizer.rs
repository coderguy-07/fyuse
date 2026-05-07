use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{FuseError, Result};

/// Category of a prompt template.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemplateCategory {
    General,
    Code,
    Creative,
    Factual,
}

/// A prompt template with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub name: String,
    pub template: String,
    pub model_family: String,
    pub category: TemplateCategory,
}

impl PromptTemplate {
    pub fn new(
        name: impl Into<String>,
        template: impl Into<String>,
        model_family: impl Into<String>,
        category: TemplateCategory,
    ) -> Self {
        Self {
            name: name.into(),
            template: template.into(),
            model_family: model_family.into(),
            category,
        }
    }

    /// Render the template by substituting `{prompt}` with the given input.
    pub fn render(&self, prompt: &str) -> String {
        self.template.replace("{prompt}", prompt)
    }
}

/// A library that stores and retrieves prompt templates.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PromptLibrary {
    templates: HashMap<String, PromptTemplate>,
}

impl PromptLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a library pre-populated with built-in templates.
    pub fn with_builtins() -> Self {
        let mut lib = Self::new();
        let builtins = vec![
            PromptTemplate::new(
                "general",
                "You are a helpful assistant.\n\n{prompt}",
                "generic",
                TemplateCategory::General,
            ),
            PromptTemplate::new(
                "code",
                "You are an expert programmer. Write clean, well-documented code.\n\n{prompt}",
                "generic",
                TemplateCategory::Code,
            ),
            PromptTemplate::new(
                "creative",
                "You are a creative writer. Be imaginative and engaging.\n\n{prompt}",
                "generic",
                TemplateCategory::Creative,
            ),
            PromptTemplate::new(
                "factual",
                "You are a factual assistant. Provide accurate, sourced information.\n\n{prompt}",
                "generic",
                TemplateCategory::Factual,
            ),
            PromptTemplate::new(
                "llama-general",
                "[INST] You are a helpful assistant. {prompt} [/INST]",
                "llama",
                TemplateCategory::General,
            ),
            PromptTemplate::new(
                "llama-code",
                "[INST] You are an expert programmer. {prompt} [/INST]",
                "llama",
                TemplateCategory::Code,
            ),
        ];
        for t in builtins {
            // Safe: builtins always have unique names
            let _ = lib.add(t);
        }
        lib
    }

    pub fn add(&mut self, template: PromptTemplate) -> Result<()> {
        if template.name.is_empty() {
            return Err(FuseError::ValidationError(
                "Template name cannot be empty".to_string(),
            ));
        }
        self.templates.insert(template.name.clone(), template);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Result<&PromptTemplate> {
        self.templates
            .get(name)
            .ok_or_else(|| FuseError::ModelNotFound(format!("Template not found: {name}")))
    }

    pub fn remove(&mut self, name: &str) -> Result<PromptTemplate> {
        self.templates
            .remove(name)
            .ok_or_else(|| FuseError::ModelNotFound(format!("Template not found: {name}")))
    }

    pub fn list(&self) -> Vec<&PromptTemplate> {
        self.templates.values().collect()
    }

    pub fn list_by_category(&self, category: &TemplateCategory) -> Vec<&PromptTemplate> {
        self.templates
            .values()
            .filter(|t| &t.category == category)
            .collect()
    }

    pub fn list_by_model_family(&self, family: &str) -> Vec<&PromptTemplate> {
        self.templates
            .values()
            .filter(|t| t.model_family == family)
            .collect()
    }
}

/// Selects the best prompt template for a given model family and category.
#[derive(Debug)]
pub struct PromptOptimizer {
    library: PromptLibrary,
}

impl PromptOptimizer {
    pub fn new(library: PromptLibrary) -> Self {
        Self { library }
    }

    /// Select the best template for the given model family and category.
    /// Falls back to generic family if no family-specific template exists.
    pub fn select(
        &self,
        model_family: &str,
        category: &TemplateCategory,
    ) -> Result<&PromptTemplate> {
        // Try family-specific first
        let family_matches: Vec<_> = self
            .library
            .list_by_model_family(model_family)
            .into_iter()
            .filter(|t| &t.category == category)
            .collect();

        if let Some(t) = family_matches.first() {
            return Ok(t);
        }

        // Fall back to generic
        let generic_matches: Vec<_> = self
            .library
            .list_by_model_family("generic")
            .into_iter()
            .filter(|t| &t.category == category)
            .collect();

        generic_matches.first().copied().ok_or_else(|| {
            FuseError::ModelNotFound(format!(
                "No template found for family={model_family}, category={category:?}"
            ))
        })
    }

    /// Optimize a prompt by selecting the best template and rendering it.
    pub fn optimize(
        &self,
        prompt: &str,
        model_family: &str,
        category: &TemplateCategory,
    ) -> Result<String> {
        let template = self.select(model_family, category)?;
        Ok(template.render(prompt))
    }

    pub fn library(&self) -> &PromptLibrary {
        &self.library
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_creation() {
        let t = PromptTemplate::new(
            "test",
            "Hello {prompt}",
            "generic",
            TemplateCategory::General,
        );
        assert_eq!(t.name, "test");
        assert_eq!(t.model_family, "generic");
        assert_eq!(t.category, TemplateCategory::General);
    }

    #[test]
    fn test_template_render() {
        let t = PromptTemplate::new(
            "test",
            "System: {prompt}",
            "generic",
            TemplateCategory::General,
        );
        assert_eq!(t.render("hi"), "System: hi");
    }

    #[test]
    fn test_template_render_no_placeholder() {
        let t = PromptTemplate::new(
            "test",
            "No placeholder",
            "generic",
            TemplateCategory::General,
        );
        assert_eq!(t.render("hi"), "No placeholder");
    }

    #[test]
    fn test_library_add_and_get() {
        let mut lib = PromptLibrary::new();
        let t = PromptTemplate::new("t1", "tmpl", "generic", TemplateCategory::Code);
        lib.add(t).unwrap();
        let retrieved = lib.get("t1").unwrap();
        assert_eq!(retrieved.name, "t1");
    }

    #[test]
    fn test_library_add_empty_name_fails() {
        let mut lib = PromptLibrary::new();
        let t = PromptTemplate::new("", "tmpl", "generic", TemplateCategory::Code);
        assert!(lib.add(t).is_err());
    }

    #[test]
    fn test_library_get_missing() {
        let lib = PromptLibrary::new();
        assert!(lib.get("nonexistent").is_err());
    }

    #[test]
    fn test_library_remove() {
        let mut lib = PromptLibrary::new();
        lib.add(PromptTemplate::new(
            "r1",
            "tmpl",
            "generic",
            TemplateCategory::General,
        ))
        .unwrap();
        let removed = lib.remove("r1").unwrap();
        assert_eq!(removed.name, "r1");
        assert!(lib.get("r1").is_err());
    }

    #[test]
    fn test_library_remove_missing() {
        let mut lib = PromptLibrary::new();
        assert!(lib.remove("nope").is_err());
    }

    #[test]
    fn test_library_builtins() {
        let lib = PromptLibrary::with_builtins();
        assert!(lib.get("general").is_ok());
        assert!(lib.get("code").is_ok());
        assert!(lib.get("creative").is_ok());
        assert!(lib.get("factual").is_ok());
    }

    #[test]
    fn test_library_list_by_category() {
        let lib = PromptLibrary::with_builtins();
        let code_templates = lib.list_by_category(&TemplateCategory::Code);
        assert!(code_templates.len() >= 2); // generic + llama
    }

    #[test]
    fn test_library_list_by_model_family() {
        let lib = PromptLibrary::with_builtins();
        let llama = lib.list_by_model_family("llama");
        assert!(llama.len() >= 2);
    }

    #[test]
    fn test_optimizer_select_family_specific() {
        let lib = PromptLibrary::with_builtins();
        let opt = PromptOptimizer::new(lib);
        let t = opt.select("llama", &TemplateCategory::Code).unwrap();
        assert_eq!(t.model_family, "llama");
    }

    #[test]
    fn test_optimizer_select_fallback_to_generic() {
        let lib = PromptLibrary::with_builtins();
        let opt = PromptOptimizer::new(lib);
        let t = opt.select("mistral", &TemplateCategory::Creative).unwrap();
        assert_eq!(t.model_family, "generic");
    }

    #[test]
    fn test_optimizer_optimize() {
        let lib = PromptLibrary::with_builtins();
        let opt = PromptOptimizer::new(lib);
        let result = opt
            .optimize("Write hello world", "generic", &TemplateCategory::Code)
            .unwrap();
        assert!(result.contains("Write hello world"));
        assert!(result.contains("expert programmer"));
    }

    #[test]
    fn test_serde_roundtrip() {
        let lib = PromptLibrary::with_builtins();
        let json = serde_json::to_string(&lib).unwrap();
        let deserialized: PromptLibrary = serde_json::from_str(&json).unwrap();
        assert!(deserialized.get("general").is_ok());
        assert!(deserialized.get("code").is_ok());
    }
}
