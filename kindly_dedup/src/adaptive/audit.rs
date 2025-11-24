/// Q34 Audit Trail for Adaptive Pipeline Selection Decision
use std::time::SystemTime;

/// SelectionAuditEntry - Captures metadata about a selection decision
#[derive(Debug, Clone)]
pub struct SelectionAuditEntry {
    pub timestamp: SystemTime,
    pub corpus_path: String,
    pub num_docs: usize,
    pub available_ram_gb: f64,
    pub required_ram_gb: f64,
    pub selected_impl: String,
    pub reason: String,
    pub force_flag: Option<String>,
}

impl SelectionAuditEntry {
    /// Convert to JSON string for logging
    pub fn to_json(&self) -> String {
        format!(
            "{{\"corpus_path\": \"{}\", \"num_docs\": {}, \"selected_impl\": \"{}\"}}",
            self.corpus_path, self.num_docs, self.selected_impl
        )
    }

    /// Log selection decision
    pub fn log(&self) {
        println!("[AUDIT] Pipeline Selection:");
        println!("  Corpus: {} ({} docs)", self.corpus_path, self.num_docs);
        println!("  RAM: {:.1} GB available, {:.2} GB required", self.available_ram_gb, self.required_ram_gb);
        println!("  Selected: {} ({})", self.selected_impl, self.reason);
        if let Some(flag) = &self.force_flag {
            println!("  Override: {}", flag);
        }
    }

    /// Verify entry consistency
    pub fn verify(&self) -> Result<(), String> {
        if self.available_ram_gb <= 0.0 {
            return Err("available_ram_gb must be > 0".to_string());
        }
        if self.required_ram_gb <= 0.0 {
            return Err("required_ram_gb must be > 0".to_string());
        }
        if self.num_docs == 0 {
            return Err("num_docs must be > 0".to_string());
        }
        if self.selected_impl == "Fast" && self.available_ram_gb < self.required_ram_gb {
            return Err(format!(
                "Fast selected but insufficient RAM ({:.1} < {:.2})",
                self.available_ram_gb, self.required_ram_gb
            ));
        }
        Ok(())
    }
}

/// SelectionAuditLogger - Collects and exports selection decisions
pub struct SelectionAuditLogger {
    entries: Vec<SelectionAuditEntry>,
}

impl SelectionAuditLogger {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Log a selection decision
    pub fn log_selection(&mut self, entry: SelectionAuditEntry) {
        if let Err(e) = entry.verify() {
            eprintln!("[AUDIT ERROR] Consistency check failed: {}", e);
        }
        entry.log();
        self.entries.push(entry);
    }

    /// Write entries to file (JSONL format)
    pub fn write_to_file(&self, path: &str) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;
        for entry in &self.entries {
            writeln!(file, "{}", entry.to_json())?;
        }
        Ok(())
    }

    pub fn entries(&self) -> &[SelectionAuditEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for SelectionAuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_json() {
        let entry = SelectionAuditEntry {
            timestamp: SystemTime::now(),
            corpus_path: "test.jsonl".to_string(),
            num_docs: 1_000_000,
            available_ram_gb: 64.0,
            required_ram_gb: 0.81,
            selected_impl: "Fast".to_string(),
            reason: "RAM sufficient (10x headroom)".to_string(),
            force_flag: None,
        };
        let json = entry.to_json();
        assert!(json.contains("corpus_path"));
        assert!(json.contains("num_docs"));
        assert!(json.contains("selected_impl"));
        assert!(json.contains("Fast"));
    }

    #[test]
    fn test_audit_logger() {
        let mut logger = SelectionAuditLogger::new();
        assert_eq!(logger.len(), 0);
        assert!(logger.is_empty());
        let entry = SelectionAuditEntry {
            timestamp: SystemTime::now(),
            corpus_path: "corpus1.jsonl".to_string(),
            num_docs: 1_000_000,
            available_ram_gb: 64.0,
            required_ram_gb: 0.81,
            selected_impl: "Fast".to_string(),
            reason: "RAM sufficient".to_string(),
            force_flag: None,
        };
        logger.log_selection(entry);
        assert_eq!(logger.len(), 1);
        assert!(!logger.is_empty());
    }

    #[test]
    fn test_audit_entry_verify_success() {
        let entry = SelectionAuditEntry {
            timestamp: SystemTime::now(),
            corpus_path: "test.jsonl".to_string(),
            num_docs: 1_000_000,
            available_ram_gb: 64.0,
            required_ram_gb: 0.81,
            selected_impl: "Fast".to_string(),
            reason: "RAM sufficient".to_string(),
            force_flag: None,
        };
        assert!(entry.verify().is_ok());
    }

    #[test]
    fn test_audit_entry_verify_zero_ram() {
        let entry = SelectionAuditEntry {
            timestamp: SystemTime::now(),
            corpus_path: "test.jsonl".to_string(),
            num_docs: 1_000_000,
            available_ram_gb: 0.0,
            required_ram_gb: 0.81,
            selected_impl: "Streaming".to_string(),
            reason: "RAM detection failed".to_string(),
            force_flag: None,
        };
        assert!(entry.verify().is_err());
    }

    #[test]
    fn test_audit_entry_verify_fast_insufficient_ram() {
        let entry = SelectionAuditEntry {
            timestamp: SystemTime::now(),
            corpus_path: "test.jsonl".to_string(),
            num_docs: 100_000_000,
            available_ram_gb: 8.0,
            required_ram_gb: 61.2,
            selected_impl: "Fast".to_string(),
            reason: "Risky override".to_string(),
            force_flag: Some("--fast".to_string()),
        };
        let result = entry.verify();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("insufficient RAM"));
    }

    #[test]
    fn test_audit_entry_verify_streaming_ok() {
        let entry = SelectionAuditEntry {
            timestamp: SystemTime::now(),
            corpus_path: "test.jsonl".to_string(),
            num_docs: 100_000_000,
            available_ram_gb: 8.0,
            required_ram_gb: 61.2,
            selected_impl: "Streaming".to_string(),
            reason: "Safe default (O(1) memory)".to_string(),
            force_flag: None,
        };
        assert!(entry.verify().is_ok());
    }
}
