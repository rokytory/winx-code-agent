use serde::{Deserialize, Serialize};

use crate::error::{WinxError, WinxResult};

/// Modeling Definition Language (MDL) representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinxMDL {
    pub models: Vec<Model>,
    pub relationships: Vec<Relationship>,
    pub metrics: Vec<Metric>,
    pub views: Vec<View>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    pub name: String,
    pub columns: Vec<Column>,
    pub table_reference: TableReference,
    pub primary_key: Option<String>,
    pub calculated_fields: Vec<CalculatedField>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub description: Option<String>,
    pub nullable: bool,
    pub primary_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    String,
    Integer,
    Float,
    Boolean,
    Date,
    Timestamp,
    Json,
    Array(Box<DataType>),
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableReference {
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub table: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatedField {
    pub name: String,
    pub expression: String,
    pub data_type: DataType,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub name: String,
    pub from_model: String,
    pub from_column: String,
    pub to_model: String,
    pub to_column: String,
    pub relationship_type: RelationshipType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipType {
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub expression: String,
    pub description: Option<String>,
    pub base_model: Option<String>,
    pub aggregation: Option<Aggregation>,
    pub filters: Vec<FilterExpression>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Aggregation {
    Sum,
    Average,
    Count,
    Min,
    Max,
    DistinctCount,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterExpression {
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct View {
    pub name: String,
    pub base_query: String,
    pub description: Option<String>,
    pub columns: Vec<ViewColumn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewColumn {
    pub name: String,
    pub data_type: DataType,
    pub expression: String,
}

impl WinxMDL {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            relationships: Vec::new(),
            metrics: Vec::new(),
            views: Vec::new(),
        }
    }

    pub fn load(path: &std::path::Path) -> WinxResult<Self> {
        let content = std::fs::read_to_string(path)?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "json" => Ok(serde_json::from_str(&content)?),
            "yaml" | "yml" => Ok(serde_yaml::from_str(&content)?),
            _ => Err(WinxError::invalid_argument(format!(
                "Unsupported MDL format: {}",
                ext
            ))),
        }
    }

    pub fn save(&self, path: &std::path::Path) -> WinxResult<()> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let content = match ext {
            "json" => serde_json::to_string_pretty(self)?,
            "yaml" | "yml" => serde_yaml::to_string(self)?,
            _ => {
                return Err(WinxError::invalid_argument(format!(
                    "Unsupported MDL format: {}",
                    ext
                )))
            }
        };

        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn validate(&self) -> WinxResult<()> {
        let mut model_names = std::collections::HashSet::new();

        // Check for duplicate model names
        for model in &self.models {
            if !model_names.insert(&model.name) {
                return Err(WinxError::other(format!(
                    "Duplicate model name: {}",
                    model.name
                )));
            }
        }

        // Validate relationships
        for relationship in &self.relationships {
            if !model_names.contains(&relationship.from_model) {
                return Err(WinxError::other(format!(
                    "Relationship '{}' references non-existent model: {}",
                    relationship.name, relationship.from_model
                )));
            }
            if !model_names.contains(&relationship.to_model) {
                return Err(WinxError::other(format!(
                    "Relationship '{}' references non-existent model: {}",
                    relationship.name, relationship.to_model
                )));
            }
        }

        // Validate metrics
        for metric in &self.metrics {
            if let Some(base_model) = &metric.base_model {
                if !model_names.contains(base_model) {
                    return Err(WinxError::other(format!(
                        "Metric '{}' references non-existent model: {}",
                        metric.name, base_model
                    )));
                }
            }
        }

        Ok(())
    }

    pub fn get_model(&self, name: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.name == name)
    }

    pub fn get_relationships_for_model(&self, model_name: &str) -> Vec<&Relationship> {
        self.relationships
            .iter()
            .filter(|r| r.from_model == model_name || r.to_model == model_name)
            .collect()
    }
}

impl Default for WinxMDL {
    fn default() -> Self {
        Self::new()
    }
}
