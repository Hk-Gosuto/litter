//! Legacy platform registration retained for binding compatibility.
use std::collections::HashMap;
#[uniffi::export]
pub fn register_android_tools(_tools: HashMap<String, String>) {}
