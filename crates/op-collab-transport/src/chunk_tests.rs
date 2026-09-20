use std::time::Duration;

use super::*;
use static_assertions::assert_not_impl_any;

assert_not_impl_any!(CompletedTransfer: Clone);
assert_not_impl_any!(TransferChunk: Clone);
assert_not_impl_any!(TransferChunkIter<'static>: Clone);

fn mutate_header(mut chunk: Vec<u8>, offset: usize, value: u8) -> Vec<u8> {
    chunk[offset] = value;
    chunk
}

#[test]
fn header_encoding_is_exact_and_big_endian() {
    let header = ChunkHeader {
        class: TransferClass::Txn,
        transfer_id: 0x0102_0304_0506_0708,
        chunk_index: 0x1112_1314,
        chunk_count: 0x0000_0002,
        total_len: 0x0000_f001,
    };
    let encoded = header.encode();
    assert_eq!(
        encoded,
        [1, 3, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 0x11, 0x12, 0x13, 0x14, 0, 0, 0, 2, 0, 0, 0xf0, 1,]
    );
    assert_eq!(ChunkHeader::decode(&encoded), Ok(header));
}

#[test]
fn iterator_and_reassembler_cover_chunk_boundaries() {
    let timeouts = TimeoutConfig::default();
    let now = Instant::now();
    for (class, len) in [
        (TransferClass::Control, 1),
        (TransferClass::Control, MAX_CHUNK_PAYLOAD),
        (TransferClass::Control, MAX_CHUNK_PAYLOAD + 1),
        (TransferClass::Txn, MAX_TXN_TRANSFER_BYTES),
    ] {
        let source = vec![0xa5; len];
        let chunks = TransferChunkIter::new(class, 7, &source)
            .unwrap()
            .collect::<Vec<_>>();
        assert_eq!(chunks.len(), len.div_ceil(MAX_CHUNK_PAYLOAD));
        assert!(chunks
            .iter()
            .all(|chunk| chunk.len() <= MAX_NOISE_PLAINTEXT_BYTES));

        let mut reassembler = Reassembler::new(timeouts);
        let mut completed = None;
        for chunk in chunks {
            completed = reassembler.push(now, &chunk).unwrap();
        }
        assert_eq!(completed.unwrap().bytes.as_slice(), source);
    }
}

#[test]
fn ticket_chunk_debug_only_reports_class_and_lengths() {
    let source = vec![211_u8; MAX_CHUNK_PAYLOAD + 1];
    let mut chunks = TransferChunkIter::new(TransferClass::Ticket, 7, &source).unwrap();
    let iter_debug = format!("{chunks:?}");
    assert!(iter_debug.contains("class: Ticket"));
    assert!(iter_debug.contains(&format!("encoded_len: {}", source.len())));
    assert!(!iter_debug.contains("211, 211"));

    let now = Instant::now();
    let mut reassembler = Reassembler::new(TimeoutConfig::default());
    assert_eq!(
        reassembler.push(now, &chunks.next().unwrap()).unwrap(),
        None
    );
    let in_flight_debug = format!("{reassembler:?}");
    assert!(in_flight_debug.contains("class: Ticket"));
    assert!(!in_flight_debug.contains("211, 211"));

    let completed = reassembler
        .push(now, &chunks.next().unwrap())
        .unwrap()
        .unwrap();
    let completed_debug = format!("{completed:?}");
    assert!(completed_debug.contains("class: Ticket"));
    assert!(completed_debug.contains(&format!("encoded_len: {}", source.len())));
    assert!(!completed_debug.contains("211, 211"));
}

#[test]
fn class_caps_are_enforced_before_splitting_or_allocating() {
    for class in [
        TransferClass::Control,
        TransferClass::Ticket,
        TransferClass::Txn,
        TransferClass::Snapshot,
    ] {
        let oversized = vec![0; class.max_transfer_bytes() + 1];
        let exact =
            TransferChunkIter::new(class, 1, &oversized[..class.max_transfer_bytes()]).unwrap();
        assert_eq!(
            exact.chunk_count() as usize,
            class.max_transfer_bytes().div_ceil(MAX_CHUNK_PAYLOAD)
        );
        assert_eq!(
            TransferChunkIter::new(class, 1, &oversized).unwrap_err(),
            ChunkError::TransferTooLarge {
                class,
                actual: oversized.len(),
                maximum: class.max_transfer_bytes(),
            }
        );
    }
}

#[test]
fn malformed_headers_fail_closed() {
    let valid = TransferChunkIter::new(TransferClass::Control, 1, b"x")
        .unwrap()
        .next()
        .unwrap();
    let mut reassembler = Reassembler::new(TimeoutConfig::default());
    let now = Instant::now();

    assert_eq!(
        reassembler.push(now, &[0; 23]),
        Err(ChunkError::HeaderLength(23))
    );
    assert_eq!(
        reassembler.push(now, &mutate_header(valid.to_vec(), 0, 2)),
        Err(ChunkError::UnsupportedVersion(2))
    );
    assert_eq!(
        reassembler.push(now, &mutate_header(valid.to_vec(), 2, 1)),
        Err(ChunkError::ReservedBits)
    );
    assert_eq!(
        reassembler.push(now, &mutate_header(valid.to_vec(), 1, 99)),
        Err(ChunkError::UnknownClass(99))
    );
}

#[test]
fn order_mismatch_clears_in_flight_state() {
    let source = vec![9; MAX_CHUNK_PAYLOAD + 1];
    let chunks = TransferChunkIter::new(TransferClass::Control, 4, &source)
        .unwrap()
        .collect::<Vec<_>>();
    let now = Instant::now();
    let mut reassembler = Reassembler::new(TimeoutConfig::default());

    assert_eq!(reassembler.push(now, &chunks[0]).unwrap(), None);
    assert_eq!(
        reassembler.push(now, &chunks[0]),
        Err(ChunkError::UnexpectedChunkIndex {
            actual: 0,
            expected: 1,
        })
    );
    assert_eq!(
        reassembler.push(now, &chunks[1]),
        Err(ChunkError::ReplayedTransferId {
            actual: 4,
            previous: 4,
        })
    );
    let next = TransferChunkIter::new(TransferClass::Control, 5, &source)
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(reassembler.push(now, &next[0]).unwrap(), None);
    assert!(reassembler.push(now, &next[1]).unwrap().is_some());
}

#[test]
fn completed_transfer_ids_must_increase() {
    let now = Instant::now();
    let mut reassembler = Reassembler::new(TimeoutConfig::default());
    let transfer = |id| {
        TransferChunkIter::new(TransferClass::Control, id, b"x")
            .unwrap()
            .next()
            .unwrap()
    };

    assert!(reassembler.push(now, &transfer(10)).unwrap().is_some());
    assert_eq!(
        reassembler.push(now, &transfer(10)),
        Err(ChunkError::ReplayedTransferId {
            actual: 10,
            previous: 10,
        })
    );
    assert_eq!(
        reassembler.push(now, &transfer(9)),
        Err(ChunkError::ReplayedTransferId {
            actual: 9,
            previous: 10,
        })
    );
    assert!(reassembler.push(now, &transfer(11)).unwrap().is_some());
}

#[test]
fn transfer_timeout_uses_class_specific_deadline_and_clears_state() {
    let timeouts = TimeoutConfig {
        ordinary_transfer: Duration::from_secs(2),
        snapshot_transfer: Duration::from_secs(8),
        ..TimeoutConfig::default()
    };
    let now = Instant::now();
    let mut reassembler = Reassembler::new(timeouts);
    let source = vec![1; MAX_CHUNK_PAYLOAD + 1];
    let control = TransferChunkIter::new(TransferClass::Control, 1, &source)
        .unwrap()
        .collect::<Vec<_>>();

    assert_eq!(reassembler.push(now, &control[0]).unwrap(), None);
    assert_eq!(
        reassembler.next_deadline(),
        Some(now + Duration::from_secs(2))
    );
    assert_eq!(
        reassembler.push(now + Duration::from_secs(2), &control[1]),
        Err(ChunkError::TimedOut(Duration::from_secs(2)))
    );
    let retry = TransferChunkIter::new(TransferClass::Control, 2, &source)
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(reassembler.push(now, &retry[0]).unwrap(), None);
    assert!(reassembler.push(now, &retry[1]).unwrap().is_some());

    let snapshot = TransferChunkIter::new(TransferClass::Snapshot, 3, &source)
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(reassembler.push(now, &snapshot[0]).unwrap(), None);
    assert_eq!(
        reassembler
            .push(now + Duration::from_secs(2), &snapshot[1])
            .unwrap()
            .unwrap()
            .transfer_id,
        3
    );
}

#[test]
fn transfer_timeout_fires_without_another_chunk() {
    let timeouts = TimeoutConfig {
        ordinary_transfer: Duration::from_secs(2),
        snapshot_transfer: Duration::from_secs(8),
        ..TimeoutConfig::default()
    };
    let start = Instant::now();
    let source = vec![1; MAX_CHUNK_PAYLOAD + 1];
    let first = TransferChunkIter::new(TransferClass::Snapshot, 1, &source)
        .unwrap()
        .next()
        .unwrap();
    let mut reassembler = Reassembler::new(timeouts);

    assert_eq!(reassembler.push(start, &first).unwrap(), None);
    assert_eq!(
        reassembler.check_timeout(start + Duration::from_secs(7)),
        Ok(())
    );
    assert_eq!(
        reassembler.check_timeout(start + Duration::from_secs(8)),
        Err(ChunkError::TimedOut(Duration::from_secs(8)))
    );
    assert_eq!(reassembler.next_deadline(), None);
}

#[test]
fn exact_payload_lengths_are_required() {
    let source = vec![3; MAX_CHUNK_PAYLOAD + 1];
    let mut chunks = TransferChunkIter::new(TransferClass::Control, 3, &source)
        .unwrap()
        .collect::<Vec<_>>();
    chunks[0].0.pop();
    let mut reassembler = Reassembler::new(TimeoutConfig::default());

    assert_eq!(
        reassembler.push(Instant::now(), &chunks[0]),
        Err(ChunkError::InvalidPayloadLength {
            actual: MAX_CHUNK_PAYLOAD - 1,
            expected: MAX_CHUNK_PAYLOAD,
        })
    );
}

#[test]
fn forged_count_or_total_is_rejected_and_clears_state() {
    let source = vec![4; MAX_CHUNK_PAYLOAD + 1];
    let chunks = TransferChunkIter::new(TransferClass::Control, 12, &source)
        .unwrap()
        .collect::<Vec<_>>();
    let now = Instant::now();
    let mut reassembler = Reassembler::new(TimeoutConfig::default());

    let mut forged_count = chunks[0].to_vec();
    forged_count[16..20].copy_from_slice(&3_u32.to_be_bytes());
    assert_eq!(
        reassembler.push(now, &forged_count),
        Err(ChunkError::InvalidChunkCount {
            actual: 3,
            expected: 2,
        })
    );

    assert_eq!(reassembler.push(now, &chunks[0]).unwrap(), None);
    let mut forged_total = chunks[1].to_vec();
    forged_total[20..24].copy_from_slice(&(source.len() as u32 - 1).to_be_bytes());
    assert_eq!(
        reassembler.push(now, &forged_total),
        Err(ChunkError::InvalidChunkCount {
            actual: 2,
            expected: 1,
        })
    );

    let retry = TransferChunkIter::new(TransferClass::Control, 13, &source)
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(reassembler.push(now, &retry[0]).unwrap(), None);
    assert!(reassembler.push(now, &retry[1]).unwrap().is_some());
}

#[test]
fn completed_transfer_holds_the_declared_reservation_until_drop() {
    let source = vec![7_u8; MAX_CONTROL_TRANSFER_BYTES];
    let chunks = TransferChunkIter::new(TransferClass::Control, 1, &source)
        .unwrap()
        .collect::<Vec<_>>();
    let budget = SharedReassemblyBudget::new(source.len()).unwrap();
    let mut reassembler = Reassembler::with_budget(TimeoutConfig::default(), budget.clone());
    let now = Instant::now();

    assert_eq!(reassembler.push(now, &chunks[0]).unwrap(), None);
    assert_eq!(
        budget.used().unwrap(),
        source.len(),
        "the declared total is charged on chunk 0, before the buffer grows"
    );
    let completed = reassembler.push(now, &chunks[1]).unwrap().unwrap();
    assert_eq!(completed.bytes(), source.as_slice());
    assert_eq!(budget.used().unwrap(), source.len());
    drop(completed);
    assert_eq!(budget.used().unwrap(), 0);
}

#[test]
fn inbound_reassembly_budget_rejects_over_aggregate_reservations() {
    let source = vec![9_u8; MAX_CHUNK_PAYLOAD + 1];
    let chunks = TransferChunkIter::new(TransferClass::Control, 1, &source)
        .unwrap()
        .collect::<Vec<_>>();
    let budget = SharedReassemblyBudget::new(source.len()).unwrap();
    let mut first = Reassembler::with_budget(TimeoutConfig::default(), budget.clone());
    let mut second = Reassembler::with_budget(TimeoutConfig::default(), budget.clone());
    let now = Instant::now();

    assert_eq!(first.push(now, &chunks[0]).unwrap(), None);
    assert_eq!(
        second.push(now, &chunks[0]),
        Err(ChunkError::InboundBudgetExhausted {
            class: TransferClass::Control,
            requested: source.len(),
        })
    );
    assert_eq!(budget.used().unwrap(), source.len());

    // The rejected transfer never started, so the same id may be retried once
    // the aggregate has room again.
    drop(first);
    assert_eq!(budget.used().unwrap(), 0);
    assert_eq!(second.push(now, &chunks[0]).unwrap(), None);
    assert!(second.push(now, &chunks[1]).unwrap().is_some());
}

#[test]
fn inbound_reassembly_budget_releases_on_abort_timeout_and_drop() {
    let timeouts = TimeoutConfig {
        ordinary_transfer: Duration::from_secs(2),
        ..TimeoutConfig::default()
    };
    let source = vec![5_u8; MAX_CHUNK_PAYLOAD + 1];
    let chunks = TransferChunkIter::new(TransferClass::Control, 1, &source)
        .unwrap()
        .collect::<Vec<_>>();
    let budget = SharedReassemblyBudget::new(source.len() * 4).unwrap();
    let now = Instant::now();

    let mut aborted = Reassembler::with_budget(timeouts, budget.clone());
    assert_eq!(aborted.push(now, &chunks[0]).unwrap(), None);
    assert_eq!(budget.used().unwrap(), source.len());
    assert!(aborted.push(now, &chunks[0]).is_err());
    assert_eq!(budget.used().unwrap(), 0, "an aborted transfer releases");

    let mut expired = Reassembler::with_budget(timeouts, budget.clone());
    let later = TransferChunkIter::new(TransferClass::Control, 2, &source)
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(expired.push(now, &later).unwrap(), None);
    assert_eq!(budget.used().unwrap(), source.len());
    assert_eq!(
        expired.check_timeout(now + Duration::from_secs(2)),
        Err(ChunkError::TimedOut(Duration::from_secs(2)))
    );
    assert_eq!(budget.used().unwrap(), 0, "a timed-out transfer releases");

    let mut dropped = Reassembler::with_budget(timeouts, budget.clone());
    let orphan = TransferChunkIter::new(TransferClass::Control, 3, &source)
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(dropped.push(now, &orphan).unwrap(), None);
    assert_eq!(budget.used().unwrap(), source.len());
    drop(dropped);
    assert_eq!(
        budget.used().unwrap(),
        0,
        "dropping the reassembler releases"
    );

    let mut unbudgeted = Reassembler::new(timeouts);
    let untracked = TransferChunkIter::new(TransferClass::Control, 4, &source)
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(unbudgeted.push(now, &untracked).unwrap(), None);
    assert_eq!(budget.used().unwrap(), 0);
}
