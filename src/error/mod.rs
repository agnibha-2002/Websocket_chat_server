// Add custom error types here if needed
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;