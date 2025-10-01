//! Entity extraction and metadata enrichment
//!
//! This module provides:
//! - Configuration-driven entity extraction using PatternRegistry
//! - Host/service correlation graph for relationship tracking
//! - Metadata enrichment for captures and chunks
//!
//! All entity patterns are loaded from config-templates/entities.toml
//! ZERO hardcoded patterns - 100% configuration-driven design

mod extractor;
mod graph;
mod metadata;

pub use extractor::{Entity, EntityExtractor};
pub use graph::{CorrelationGraph, HostInfo, ServiceInfo};
pub use metadata::{CaptureMetadata, ChunkMetadata, MetadataEnricher};
