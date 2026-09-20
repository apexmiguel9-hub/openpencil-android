use std::fmt;
use std::time::Instant;

use zeroize::Zeroizing;

use crate::{
    ChunkError, SharedReassemblyBudget, SharedReassemblyReservation, TimeoutConfig,
    MAX_CONTROL_TRANSFER_BYTES, MAX_NOISE_PLAINTEXT_BYTES, MAX_SNAPSHOT_TRANSFER_BYTES,
    MAX_TXN_TRANSFER_BYTES,
};

pub const CHUNK_HEADER_BYTES: usize = 24;
pub const MAX_CHUNK_PAYLOAD: usize = MAX_NOISE_PLAINTEXT_BYTES - CHUNK_HEADER_BYTES;
const CHUNK_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TransferClass {
    Control = 1,
    Ticket = 2,
    Txn = 3,
    Snapshot = 4,
}

impl TransferClass {
    pub const fn max_transfer_bytes(self) -> usize {
        match self {
            Self::Control | Self::Ticket => MAX_CONTROL_TRANSFER_BYTES,
            Self::Txn => MAX_TXN_TRANSFER_BYTES,
            Self::Snapshot => MAX_SNAPSHOT_TRANSFER_BYTES,
        }
    }

    pub const fn timeout(self, timeouts: TimeoutConfig) -> std::time::Duration {
        match self {
            Self::Snapshot => timeouts.snapshot_transfer,
            Self::Control | Self::Ticket | Self::Txn => timeouts.ordinary_transfer,
        }
    }
}

impl TryFrom<u8> for TransferClass {
    type Error = ChunkError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Control),
            2 => Ok(Self::Ticket),
            3 => Ok(Self::Txn),
            4 => Ok(Self::Snapshot),
            unknown => Err(ChunkError::UnknownClass(unknown)),
        }
    }
}

impl From<TransferClass> for u8 {
    fn from(value: TransferClass) -> Self {
        value as Self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkHeader {
    pub class: TransferClass,
    pub transfer_id: u64,
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub total_len: u32,
}

impl ChunkHeader {
    pub fn encode(self) -> [u8; CHUNK_HEADER_BYTES] {
        let mut encoded = [0_u8; CHUNK_HEADER_BYTES];
        encoded[0] = CHUNK_VERSION;
        encoded[1] = self.class.into();
        encoded[2..4].copy_from_slice(&0_u16.to_be_bytes());
        encoded[4..12].copy_from_slice(&self.transfer_id.to_be_bytes());
        encoded[12..16].copy_from_slice(&self.chunk_index.to_be_bytes());
        encoded[16..20].copy_from_slice(&self.chunk_count.to_be_bytes());
        encoded[20..24].copy_from_slice(&self.total_len.to_be_bytes());
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, ChunkError> {
        if encoded.len() != CHUNK_HEADER_BYTES {
            return Err(ChunkError::HeaderLength(encoded.len()));
        }
        if encoded[0] != CHUNK_VERSION {
            return Err(ChunkError::UnsupportedVersion(encoded[0]));
        }
        if encoded[2] != 0 || encoded[3] != 0 {
            return Err(ChunkError::ReservedBits);
        }

        let class = TransferClass::try_from(encoded[1])?;
        let transfer_id = u64::from_be_bytes(encoded[4..12].try_into().expect("fixed slice"));
        if transfer_id == 0 {
            return Err(ChunkError::ZeroTransferId);
        }
        let chunk_index = u32::from_be_bytes(encoded[12..16].try_into().expect("fixed slice"));
        let chunk_count = u32::from_be_bytes(encoded[16..20].try_into().expect("fixed slice"));
        let total_len = u32::from_be_bytes(encoded[20..24].try_into().expect("fixed slice"));
        validate_transfer_shape(class, total_len as usize, chunk_count)?;

        Ok(Self {
            class,
            transfer_id,
            chunk_index,
            chunk_count,
            total_len,
        })
    }
}

pub struct CompletedTransfer {
    pub(crate) class: TransferClass,
    pub(crate) transfer_id: u64,
    pub(crate) bytes: Zeroizing<Vec<u8>>,
    /// Keeps the aggregate reservation charged while the completed bytes are
    /// decoded or retained by the caller.
    pub(crate) _reservation: Option<SharedReassemblyReservation>,
}

impl PartialEq for CompletedTransfer {
    fn eq(&self, other: &Self) -> bool {
        self.class == other.class
            && self.transfer_id == other.transfer_id
            && self.bytes == other.bytes
    }
}

impl Eq for CompletedTransfer {}

impl CompletedTransfer {
    pub const fn class(&self) -> TransferClass {
        self.class
    }

    pub const fn transfer_id(&self) -> u64 {
        self.transfer_id
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn encoded_len(&self) -> usize {
        self.bytes.len()
    }
}

impl fmt::Debug for CompletedTransfer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletedTransfer")
            .field("class", &self.class)
            .field("transfer_id", &self.transfer_id)
            .field("encoded_len", &self.encoded_len())
            .finish()
    }
}

pub struct TransferChunkIter<'a> {
    class: TransferClass,
    transfer_id: u64,
    bytes: &'a [u8],
    chunk_count: u32,
    next_index: u32,
}

pub struct TransferChunk(Zeroizing<Vec<u8>>);

impl std::ops::Deref for TransferChunk {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Debug for TransferChunk {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransferChunk")
            .field("encoded_len", &self.0.len())
            .finish()
    }
}

impl fmt::Debug for TransferChunkIter<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransferChunkIter")
            .field("class", &self.class)
            .field("transfer_id", &self.transfer_id)
            .field("encoded_len", &self.bytes.len())
            .field("chunk_count", &self.chunk_count)
            .field("next_index", &self.next_index)
            .finish()
    }
}

impl<'a> TransferChunkIter<'a> {
    pub fn new(
        class: TransferClass,
        transfer_id: u64,
        bytes: &'a [u8],
    ) -> Result<Self, ChunkError> {
        if transfer_id == 0 {
            return Err(ChunkError::ZeroTransferId);
        }
        let chunk_count = expected_chunk_count(bytes.len())?;
        validate_transfer_shape(class, bytes.len(), chunk_count)?;

        Ok(Self {
            class,
            transfer_id,
            bytes,
            chunk_count,
            next_index: 0,
        })
    }

    pub const fn chunk_count(&self) -> u32 {
        self.chunk_count
    }
}

impl Iterator for TransferChunkIter<'_> {
    type Item = TransferChunk;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index >= self.chunk_count {
            return None;
        }

        let start = (self.next_index as usize) * MAX_CHUNK_PAYLOAD;
        let end = start
            .saturating_add(MAX_CHUNK_PAYLOAD)
            .min(self.bytes.len());
        let total_len = u32::try_from(self.bytes.len()).expect("validated by constructor");
        let header = ChunkHeader {
            class: self.class,
            transfer_id: self.transfer_id,
            chunk_index: self.next_index,
            chunk_count: self.chunk_count,
            total_len,
        };
        self.next_index += 1;

        let mut plaintext = Zeroizing::new(Vec::with_capacity(CHUNK_HEADER_BYTES + end - start));
        plaintext.extend_from_slice(&header.encode());
        plaintext.extend_from_slice(&self.bytes[start..end]);
        Some(TransferChunk(plaintext))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.chunk_count - self.next_index) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for TransferChunkIter<'_> {}

struct InFlightTransfer {
    header: ChunkHeader,
    next_index: u32,
    started_at: Instant,
    bytes: Zeroizing<Vec<u8>>,
    /// Aggregate reservation for `header.total_len`. Completion transfers it
    /// alongside the bytes; abort, timeout, or reassembler drop releases it.
    _reservation: Option<SharedReassemblyReservation>,
}

impl fmt::Debug for InFlightTransfer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InFlightTransfer")
            .field("header", &self.header)
            .field("next_index", &self.next_index)
            .field("started_at", &self.started_at)
            .field("encoded_len", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug)]
pub struct Reassembler {
    timeouts: TimeoutConfig,
    budget: Option<SharedReassemblyBudget>,
    in_flight: Option<InFlightTransfer>,
    last_started_id: Option<u64>,
}

impl Reassembler {
    /// Builds an unbudgeted reassembler. Production connections use
    /// [`Self::with_budget`] so their declared allocations are visible to the
    /// aggregate inbound bound.
    pub const fn new(timeouts: TimeoutConfig) -> Self {
        Self {
            timeouts,
            budget: None,
            in_flight: None,
            last_started_id: None,
        }
    }

    pub fn with_budget(timeouts: TimeoutConfig, budget: SharedReassemblyBudget) -> Self {
        Self {
            timeouts,
            budget: Some(budget),
            in_flight: None,
            last_started_id: None,
        }
    }

    pub fn push(
        &mut self,
        now: Instant,
        plaintext: &[u8],
    ) -> Result<Option<CompletedTransfer>, ChunkError> {
        let result = self.push_inner(now, plaintext);
        if result.is_err() {
            self.in_flight = None;
        }
        result
    }

    pub fn reset(&mut self) {
        self.in_flight = None;
    }

    /// Returns the absolute deadline for the current logical transfer.
    pub fn next_deadline(&self) -> Option<Instant> {
        let in_flight = self.in_flight.as_ref()?;
        in_flight
            .started_at
            .checked_add(in_flight.header.class.timeout(self.timeouts))
    }

    /// Expires an incomplete transfer even when no subsequent chunk arrives.
    pub fn check_timeout(&mut self, now: Instant) -> Result<(), ChunkError> {
        let Some(in_flight) = self.in_flight.as_ref() else {
            return Ok(());
        };
        let timeout = in_flight.header.class.timeout(self.timeouts);
        if now.saturating_duration_since(in_flight.started_at) < timeout {
            return Ok(());
        }
        self.in_flight = None;
        Err(ChunkError::TimedOut(timeout))
    }

    fn push_inner(
        &mut self,
        now: Instant,
        plaintext: &[u8],
    ) -> Result<Option<CompletedTransfer>, ChunkError> {
        self.check_timeout(now)?;
        if plaintext.len() < CHUNK_HEADER_BYTES {
            return Err(ChunkError::HeaderLength(plaintext.len()));
        }

        let header = ChunkHeader::decode(&plaintext[..CHUNK_HEADER_BYTES])?;
        let payload = &plaintext[CHUNK_HEADER_BYTES..];

        if let Some(in_flight) = &self.in_flight {
            if header.class != in_flight.header.class
                || header.transfer_id != in_flight.header.transfer_id
                || header.chunk_count != in_flight.header.chunk_count
                || header.total_len != in_flight.header.total_len
            {
                return Err(ChunkError::InFlightMismatch);
            }
            if header.chunk_index != in_flight.next_index {
                return Err(ChunkError::UnexpectedChunkIndex {
                    actual: header.chunk_index,
                    expected: in_flight.next_index,
                });
            }
        } else {
            if let Some(previous) = self.last_started_id {
                if header.transfer_id <= previous {
                    return Err(ChunkError::ReplayedTransferId {
                        actual: header.transfer_id,
                        previous,
                    });
                }
            }
            if header.chunk_index != 0 {
                return Err(ChunkError::UnexpectedChunkIndex {
                    actual: header.chunk_index,
                    expected: 0,
                });
            }
            // The declared total is charged against the aggregate inbound bound
            // before it is allocated, so a peer cannot reserve memory the
            // process-wide budget cannot see.
            let declared = header.total_len as usize;
            let reservation = match &self.budget {
                Some(budget) => Some(budget.reserve(declared).map_err(|_| {
                    ChunkError::InboundBudgetExhausted {
                        class: header.class,
                        requested: declared,
                    }
                })?),
                None => None,
            };
            self.last_started_id = Some(header.transfer_id);
            self.in_flight = Some(InFlightTransfer {
                header,
                next_index: 0,
                started_at: now,
                bytes: Zeroizing::new(Vec::with_capacity(declared)),
                _reservation: reservation,
            });
        }

        let in_flight = self.in_flight.as_mut().expect("initialized above");
        let expected_payload =
            expected_payload_len(in_flight.header.total_len as usize, header.chunk_index)?;
        if payload.len() != expected_payload {
            return Err(ChunkError::InvalidPayloadLength {
                actual: payload.len(),
                expected: expected_payload,
            });
        }

        in_flight.bytes.extend_from_slice(payload);
        in_flight.next_index = in_flight
            .next_index
            .checked_add(1)
            .ok_or(ChunkError::LengthOverflow)?;

        if in_flight.next_index != in_flight.header.chunk_count {
            return Ok(None);
        }

        let completed = self.in_flight.take().expect("present until completion");
        if completed.bytes.len() != completed.header.total_len as usize {
            return Err(ChunkError::InvalidPayloadLength {
                actual: completed.bytes.len(),
                expected: completed.header.total_len as usize,
            });
        }
        Ok(Some(CompletedTransfer {
            class: completed.header.class,
            transfer_id: completed.header.transfer_id,
            bytes: completed.bytes,
            _reservation: completed._reservation,
        }))
    }
}

fn validate_transfer_shape(
    class: TransferClass,
    total_len: usize,
    chunk_count: u32,
) -> Result<(), ChunkError> {
    if total_len == 0 {
        return Err(ChunkError::EmptyTransfer);
    }
    let maximum = class.max_transfer_bytes();
    if total_len > maximum {
        return Err(ChunkError::TransferTooLarge {
            class,
            actual: total_len,
            maximum,
        });
    }
    let expected = expected_chunk_count(total_len)?;
    if chunk_count != expected {
        return Err(ChunkError::InvalidChunkCount {
            actual: chunk_count,
            expected,
        });
    }
    Ok(())
}

fn expected_chunk_count(total_len: usize) -> Result<u32, ChunkError> {
    if total_len == 0 {
        return Err(ChunkError::EmptyTransfer);
    }
    let count = total_len.div_ceil(MAX_CHUNK_PAYLOAD);
    u32::try_from(count).map_err(|_| ChunkError::LengthOverflow)
}

fn expected_payload_len(total_len: usize, chunk_index: u32) -> Result<usize, ChunkError> {
    let offset = (chunk_index as usize)
        .checked_mul(MAX_CHUNK_PAYLOAD)
        .ok_or(ChunkError::LengthOverflow)?;
    let remaining = total_len
        .checked_sub(offset)
        .ok_or(ChunkError::LengthOverflow)?;
    Ok(remaining.min(MAX_CHUNK_PAYLOAD))
}

#[cfg(test)]
#[path = "chunk_tests.rs"]
mod tests;
