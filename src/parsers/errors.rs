use thiserror::Error;

#[derive(Error, Debug)]
pub enum DelaParseError {
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("XML parsing error: {0}")]
    Xml(#[from] roxmltree::Error),

    #[error("Syntax error: {0}")]
    Syntax(String),

    #[error("Validation error: {0}")]
    Validation(String),
}
