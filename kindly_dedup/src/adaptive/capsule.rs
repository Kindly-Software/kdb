//! AdaptiveDedupPipelineCapsule - Unified API with automatic pipeline selection

use crate::adaptive::traits::{DedupPipelineTrait, PipelineError, PipelineSelection};
use crate::legacy::dedup_pipeline::DedupPipeline;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct SelectionMetadata {
    pub available_ram_bytes: u64,
    pub estimated_ram_bytes: u64,
    pub corpus_size: u32,
    pub threshold: f64,
    pub timestamp: SystemTime,
    pub reason: String,
}

#[repr(C, align(64))]
pub struct AdaptiveDedupPipelineCapsule {
    inner: Box<dyn DedupPipelineTrait>,
    selected_impl: PipelineSelection,
    selection_metadata: SelectionMetadata,
    _padding: [u8; 8],
}

impl AdaptiveDedupPipelineCapsule {
    pub fn new(
        _corpus_path: &str,
        num_documents: u32,
        threshold: f64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::new_with_options(_corpus_path, num_documents, threshold, None, false, false)
    }

    pub fn new_with_options(
        _corpus_path: &str,
        num_documents: u32,
        threshold: f64,
        available_ram_gb: Option<f64>,
        force_fast: bool,
        force_streaming: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        use crate::adaptive::selection::PipelineSelectorCapsule;

        if num_documents == 0 {
            return Err("num_documents must be > 0".into());
        }
        if !(0.0..=1.0).contains(&threshold) {
            return Err("threshold must be 0.0 to 1.0".into());
        }

        let selection = PipelineSelectorCapsule::select(
            num_documents,
            available_ram_gb,
            force_fast,
            force_streaming,
        );

        let available_ram_bytes = if let Some(gb) = available_ram_gb {
            (gb * 1e9) as u64
        } else {
            use crate::adaptive::selection::RamDetectorCapsule;
            RamDetectorCapsule::available_ram_bytes().unwrap_or(0)
        };

        let estimated_ram_bytes = PipelineSelectorCapsule::estimate_dedup_pipeline_memory(num_documents);

        let reason = match selection {
            PipelineSelection::Fast => {
                if force_fast {
                    "Force Fast (manual override)".to_string()
                } else {
                    let headroom = available_ram_bytes as f64 / estimated_ram_bytes as f64;
                    format!("RAM sufficient ({:.1}× headroom)", headroom)
                }
            }
            PipelineSelection::Streaming => {
                if force_streaming {
                    "Force Streaming (manual override)".to_string()
                } else if available_ram_bytes == 0 {
                    "RAM detection failed (safe default)".to_string()
                } else {
                    let shortfall = estimated_ram_bytes as f64 / available_ram_bytes as f64;
                    format!("RAM insufficient ({:.2}× required)", shortfall)
                }
            }
        };

        let selection_metadata = SelectionMetadata {
            available_ram_bytes,
            estimated_ram_bytes,
            corpus_size: num_documents,
            threshold,
            timestamp: SystemTime::now(),
            reason: reason.clone(),
        };

        let inner: Box<dyn DedupPipelineTrait> = match selection {
            PipelineSelection::Fast => {
                Box::new(DedupPipeline::new(num_documents as usize, &Default::default()))
            }
            PipelineSelection::Streaming => {
                return Err(
                    "Streaming pipeline not yet implemented. Use Force Fast or implement StreamingDedupPipelineCapsule first.".into(),
                );
            }
        };

        Ok(Self {
            inner,
            selected_impl: selection,
            selection_metadata,
            _padding: [0u8; 8],
        })
    }

    pub fn selection_metadata(&self) -> &SelectionMetadata {
        &self.selection_metadata
    }

    pub fn selected_impl(&self) -> PipelineSelection {
        self.selected_impl
    }

    pub fn is_fast(&self) -> bool {
        self.selected_impl == PipelineSelection::Fast
    }

    pub fn is_streaming(&self) -> bool {
        self.selected_impl == PipelineSelection::Streaming
    }
}

impl DedupPipelineTrait for AdaptiveDedupPipelineCapsule {
    fn add_document(&mut self, doc_id: u32, text: &str) -> Result<(), PipelineError> {
        self.inner.add_document(doc_id, text)
    }

    fn find_duplicates(&mut self) -> Result<Vec<Vec<u32>>, PipelineError> {
        self.inner.find_duplicates()
    }

    fn memory_usage_mb(&self) -> f64 {
        self.inner.memory_usage_mb()
    }

    fn throughput_docs_per_sec(&self) -> f64 {
        self.inner.throughput_docs_per_sec()
    }

    fn implementation_name(&self) -> &'static str {
        self.inner.implementation_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_requires_positive_documents() {
        let result = AdaptiveDedupPipelineCapsule::new("test.jsonl", 0, 0.85);
        assert!(result.is_err());
    }

    #[test]
    fn test_new_requires_valid_threshold() {
        let result = AdaptiveDedupPipelineCapsule::new("test.jsonl", 1_000_000, 1.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_force_fast_small_corpus() {
        let pipeline = AdaptiveDedupPipelineCapsule::new_with_options(
            "test.jsonl",
            100_000,
            0.85,
            Some(64.0),
            true,
            false,
        );
        assert!(pipeline.is_ok());
        let pipeline = pipeline.unwrap();
        assert!(pipeline.is_fast());
    }

    #[test]
    fn test_selection_metadata_captured() {
        let pipeline = AdaptiveDedupPipelineCapsule::new_with_options(
            "corpus.jsonl",
            10_000_000,
            0.85,
            Some(64.0),
            true,
            false,
        ).unwrap();

        let metadata = pipeline.selection_metadata();
        assert_eq!(metadata.corpus_size, 10_000_000);
        assert_eq!(metadata.threshold, 0.85);
        assert!(metadata.available_ram_bytes > 0);
        assert!(metadata.estimated_ram_bytes > 0);
    }
}
