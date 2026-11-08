//! Retrieval (tech doc 3.2 `retrieval`; PRD QA-4).
//!
//! Search is restricted to one session's documents, combines vector similarity
//! and keyword (FTS5) ranking with reciprocal rank fusion, and a context pack
//! adds the last minutes of transcript and the rolling notes. Order of use in a
//! prompt (QA-4): transcript and notes, then documents, then web search.

pub mod brief;
pub mod fusion;
pub mod pack;
pub mod search;

pub use brief::{BRIEF_CHARS, Brief, BriefSource};
pub use pack::{ContextChunk, ContextOptions, ContextPack, build_pack};
pub use search::{Hit, keyword, vector};
