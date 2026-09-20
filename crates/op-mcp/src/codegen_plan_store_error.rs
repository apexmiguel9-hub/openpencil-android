//! Typed failures for the in-memory codegen plan store
//! (`codegen_plan_store.rs` + `codegen_plan_store_assembly.rs`).
//!
//! Style follows `ProgramError`: a plain enum plus a hand-written `Display`,
//! no `thiserror` and no new dependency. Each variant's `Display` reproduces
//! the exact sentence the stringly-typed store produced — `codegen_tools.rs`
//! embeds them as `"codegen_plan failed: {message}"` (and the TS routes this
//! is a port of 400 with the same text), so not one character may move.
//!
//! What the enum buys over `String` is the CLASSIFICATION: malformed input
//! shapes (`Chunk*` / `Result*` / `ChunksNotArray`), plan-lifecycle faults
//! (`PlanNotFound` / `ChunkNotInPlan` / `PlanIncomplete`), and dependency
//! faults (`ChunkBlocked`) are separable without matching prose — and the
//! validation errors keep their structure as a `Vec` instead of a
//! pre-joined blob.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PlanStoreError {
    /// A plan chunk has no string `id`.
    ChunkMissingId,
    /// A plan chunk has no `dependencies` array. `chunk_id` is the id as it
    /// renders in the message (`"?"` when the chunk has none).
    ChunkMissingDependencies { chunk_id: String },
    /// A submitted chunk result is not a JSON object.
    ResultNotObject,
    /// A submitted chunk result has no string `code`.
    ResultCodeNotString,
    /// A submitted chunk result has no `contract` object.
    ResultContractNotObject,
    /// `plan.chunks` is absent or not an array.
    ChunksNotArray,
    /// `plan.chunks` is an empty array.
    NoChunks,
    /// `validate_plan` rejected the plan. Kept structured; `Display` joins
    /// with `"; "` exactly as the TS route did.
    Validation(Vec<String>),
    /// No plan is registered under this id (expired, cleaned, or never
    /// created).
    PlanNotFound { plan_id: String },
    /// A submitted chunk result has no string `chunkId`.
    ResultChunkIdNotString,
    /// The submitted `chunkId` is not one of the plan's chunks.
    ChunkNotInPlan { chunk_id: String, plan_id: String },
    /// The chunk's dependency chain contains failed / skipped chunks.
    ChunkBlocked {
        chunk_id: String,
        blockers: Vec<String>,
    },
    /// Assembly was requested while chunks are still pending.
    PlanIncomplete {
        plan_id: String,
        pending: Vec<String>,
    },
    /// Every chunk was failed / skipped / blocked, so assembly has nothing
    /// usable to emit.
    NoUsableChunks {
        plan_id: String,
        omitted: Vec<String>,
    },
}

impl fmt::Display for PlanStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanStoreError::ChunkMissingId => f.write_str("every plan chunk needs a string id"),
            PlanStoreError::ChunkMissingDependencies { chunk_id } => {
                write!(f, "Chunk {chunk_id} needs a dependencies array")
            }
            PlanStoreError::ResultNotObject => f.write_str("result must be an object"),
            PlanStoreError::ResultCodeNotString => f.write_str("result.code must be a string"),
            PlanStoreError::ResultContractNotObject => {
                f.write_str("result.contract must be an object")
            }
            PlanStoreError::ChunksNotArray => f.write_str("plan.chunks must be an array"),
            PlanStoreError::NoChunks => f.write_str("Plan needs at least one chunk"),
            PlanStoreError::Validation(errors) => f.write_str(&errors.join("; ")),
            PlanStoreError::PlanNotFound { plan_id } => write!(f, "Plan {plan_id} not found"),
            PlanStoreError::ResultChunkIdNotString => {
                f.write_str("result.chunkId must be a string")
            }
            PlanStoreError::ChunkNotInPlan { chunk_id, plan_id } => write!(
                f,
                "Chunk {chunk_id} is not part of plan {plan_id}; use a chunkId from executionPlan"
            ),
            PlanStoreError::ChunkBlocked { chunk_id, blockers } => write!(
                f,
                "Chunk {chunk_id} is blocked by failed/skipped dependencies: {}. Retry those dependencies first; the plan remains available.",
                blockers.join(", ")
            ),
            PlanStoreError::PlanIncomplete { plan_id, pending } => write!(
                f,
                "Plan {plan_id} is incomplete; pending chunks: {}. Submit each ready chunk before assembling. The plan remains available.",
                pending.join(", ")
            ),
            PlanStoreError::NoUsableChunks { plan_id, omitted } => write!(
                f,
                "Plan {plan_id} has no usable chunk code (failed/skipped/blocked: {}). Resubmit failed dependencies before assembling. The plan remains available.",
                omitted.join(", ")
            ),
        }
    }
}

impl std::error::Error for PlanStoreError {}
