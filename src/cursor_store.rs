use std::collections::HashMap;
use serde_json::Value;
use crate::error::AppError;

pub type CursorValues = Vec<Value>;

#[derive(Default)]
pub struct CursorStore {
    inner: HashMap<String, CursorValues>,
}

impl CursorStore {
    pub fn get(&self, table: &str) -> Result<&CursorValues, AppError> {
        self.inner.get(table).ok_or_else(|| AppError::CursorMissing(table.to_string()))
    }

    pub fn set(&mut self, table: &str, values: CursorValues) {
        self.inner.insert(table.to_string(), values);
    }
}
