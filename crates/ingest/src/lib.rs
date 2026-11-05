//! Document ingest (tech doc 3.2 `ingest`, 3.3 "Document upload"; PRD CTX-2, CTX-3).
//!
//! `parse` → pages of text; `chunk` → ~500-token chunks with 50-token overlap,
//! never crossing a page (so citations have the right page); `pipeline` stores
//! the document, its chunks (keyword index via triggers) and their vectors.
//! Original files are not kept, only extracted text (tech doc 4.1).

pub mod chunk;
pub mod html;
pub mod parse;
pub mod pipeline;
pub mod repo;

pub use pipeline::{Owner, Progress, Source, ingest};
pub use repo::{DocStatus, DocumentInfo};
