use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{WinxError, WinxResult};

pub mod analyzer;
pub mod mdl;

pub use analyzer::SemanticAnalyzer;
pub use mdl::{Metric, Model, Relationship, View, WinxMDL};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticContext {
    pub models: HashMap<String, Model>,
    pub relationships: Vec<Relationship>,
    pub metrics: Vec<Metric>,
    pub views: Vec<View>,
}

impl SemanticContext {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            relationships: Vec::new(),
            metrics: Vec::new(),
            views: Vec::new(),
        }
    }

    pub fn from_mdl(mdl: &WinxMDL) -> WinxResult<Self> {
        let mut context = Self::new();

        // Add models to context
        for model in &mdl.models {
            context.models.insert(model.name.clone(), model.clone());
        }

        // Add relationships
        context.relationships = mdl.relationships.clone();

        // Add metrics
        context.metrics = mdl.metrics.clone();

        // Add views
        context.views = mdl.views.clone();

        Ok(context)
    }

    pub fn get_model(&self, name: &str) -> Option<&Model> {
        self.models.get(name)
    }

    pub fn get_related_models(&self, model_name: &str) -> Vec<&Model> {
        let mut related_models = Vec::new();

        for relationship in &self.relationships {
            if relationship.from_model == model_name {
                if let Some(model) = self.get_model(&relationship.to_model) {
                    related_models.push(model);
                }
            } else if relationship.to_model == model_name {
                if let Some(model) = self.get_model(&relationship.from_model) {
                    related_models.push(model);
                }
            }
        }

        related_models
    }

    pub fn validate(&self) -> WinxResult<()> {
        // Validate that all relationships reference valid models
        for relationship in &self.relationships {
            if !self.models.contains_key(&relationship.from_model) {
                return Err(WinxError::other(format!(
                    "Relationship references non-existent model: {}",
                    relationship.from_model
                )));
            }
            if !self.models.contains_key(&relationship.to_model) {
                return Err(WinxError::other(format!(
                    "Relationship references non-existent model: {}",
                    relationship.to_model
                )));
            }
        }

        // Validate that all metrics reference valid models
        for metric in &self.metrics {
            if let Some(base_model) = &metric.base_model {
                if !self.models.contains_key(base_model) {
                    return Err(WinxError::other(format!(
                        "Metric '{}' references non-existent model: {}",
                        metric.name, base_model
                    )));
                }
            }
        }

        Ok(())
    }
}

impl Default for SemanticContext {
    fn default() -> Self {
        Self::new()
    }
}
