use super::*;

use std::path::PathBuf;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "ivy-usage-ledger-{:016x}",
            u64::from_le_bytes(nonce)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> PathBuf {
        self.0.join("usage.sqlite")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn observation(
    key: u32,
    source: Key,
    evidence: u32,
    input: Option<i64>,
    output: i64,
) -> Observation {
    fn key_bytes(counter: u32) -> Key {
        let mut bytes = [0; 32];
        bytes[..4].copy_from_slice(&counter.to_be_bytes());
        bytes
    }

    Observation {
        provider: Provider::Claude,
        event_key: key_bytes(key),
        source_key: source,
        evidence_key: key_bytes(evidence),
        at_ms: Some(1000),
        model: Some("claude-sonnet-4-6".to_owned()),
        model_key: Some([4; 32]),
        counts: Counts {
            uncached_input: input,
            cache_read: Some(30),
            cache_write: Some(10),
            cache_write_1h: Some(0),
            output: Some(output),
            ..Counts::default()
        },
    }
}

fn publish(ledger: &mut Ledger, fingerprint: Key) {
    while !ledger.reconcile_slice().unwrap() {}
    ledger
        .publish(Checkpoint {
            provider: Provider::Claude,
            root_key: [8; 32],
            fingerprint: Some(fingerprint),
            prefix_bytes: 1000,
            coverage: 0,
        })
        .unwrap();
    ledger
        .mark_provider_refresh_success(Provider::Claude, 42)
        .unwrap();
}

fn read(ledger: &mut Ledger) -> Vec<(Resolution, Option<i128>)> {
    let mut daily = Vec::new();
    let mut by_model = Vec::new();
    ledger
        .visit(
            0,
            10000,
            Arc::default(),
            |item| {
                assert_eq!(item.at_ms, 1000);
                daily.push((item.resolution, item.counts.known_tokens()));
                Ok(())
            },
            |item| {
                assert_eq!(
                    item.model.as_deref(),
                    (item.resolution == Resolution::Resolved).then_some("claude-sonnet-4-6")
                );
                by_model.push((item.resolution, item.counts.known_tokens()));
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(daily, by_model);
    daily
}

#[test]
fn pending_epoch_is_invisible_and_recovery_reopens_old_versions() {
    let fixture = Fixture::new();
    let path = fixture.path();
    assert!(matches!(Ledger::open_reader(&path), Err(Error::Missing)));
    fs::File::create(&path).unwrap();
    let source = [7; 32];
    let mut writer = Ledger::open_writer(&path).unwrap();
    writer.begin(source).unwrap();
    let base = observation(1, source, 1, Some(100), 20);
    writer.stage(&[base.clone(), base.clone()]).unwrap();
    publish(&mut writer, [1; 32]);

    let mut reader = Ledger::open_reader(&path).unwrap();
    assert_eq!(read(&mut reader), [(Resolution::Resolved, Some(160))]);
    assert!(reader.begin([9; 32]).is_err());
    let store_id = writer.info().unwrap().store_id;

    writer.begin(source).unwrap();
    writer
        .stage(&[
            observation(1, source, 2, Some(100), 40),
            observation(2, source, 1, Some(100), 40),
        ])
        .unwrap();
    while !writer.reconcile_slice().unwrap() {}
    assert_eq!(read(&mut reader), [(Resolution::Resolved, Some(160))]);
    drop(writer);

    let mut writer = Ledger::open_writer(&path).unwrap();
    while !writer.recover_slice().unwrap() {}
    assert_eq!(writer.info().unwrap().store_id, store_id);
    assert_eq!(writer.info().unwrap().published_epoch, 1);
    assert_eq!(
        writer.checkpoint(source).unwrap().unwrap().fingerprint,
        Some([1; 32])
    );
    writer.begin(source).unwrap();
    writer.stage(&[base]).unwrap();
    publish(&mut writer, [2; 32]);
    assert_eq!(read(&mut reader), [(Resolution::Resolved, Some(160))]);
}

#[test]
fn revision_cursor_preserves_conflict_across_null_and_slice_boundaries() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let a = [7; 32];
    let b = [8; 32];
    for (first, second) in [(a, b), (b, a)] {
        let path = path.with_extension(format!("{}.sqlite", first[0]));
        let mut ledger = Ledger::open_writer(&path).unwrap();
        for source in [first, second] {
            ledger.begin(source).unwrap();
            if source == a {
                let mut evidence: Vec<_> = (0..u32::try_from(WRITE_BATCH).unwrap())
                    .map(|id| observation(1, source, id, None, 10))
                    .collect();
                evidence.push(observation(1, source, 700, Some(100), 20));
                evidence.push(observation(1, source, 701, Some(110), 20));
                ledger.stage(&evidence).unwrap();
            } else {
                ledger
                    .stage(&[observation(1, source, 0, Some(110), 10)])
                    .unwrap();
            }
            publish(&mut ledger, source);
        }
        drop(ledger);
        let mut reader = Ledger::open_reader(&path).unwrap();
        assert_eq!(read(&mut reader), [(Resolution::Conflicted, None)]);
    }
}

#[test]
fn future_versions_and_wal_sidecars_are_refused() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let ledger = Ledger::open_writer(&path).unwrap();
    ledger
        .connection
        .pragma_update(None, "user_version", VERSION + 1)
        .unwrap();
    drop(ledger);
    assert!(matches!(
        Ledger::open_reader(&path),
        Err(Error::FutureVersion)
    ));
    assert!(matches!(
        Ledger::open_writer(&path),
        Err(Error::FutureVersion)
    ));

    fs::File::create(sidecar(&path, "-shm")).unwrap();
    assert!(matches!(Ledger::open_writer(&path), Err(Error::Invalid)));
}

#[test]
fn failed_commit_against_pinned_reader_does_not_publish() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let mut writer = Ledger::open_writer(&path).unwrap();
    let source = [7; 32];
    writer.begin(source).unwrap();
    writer
        .stage(&[observation(1, source, 1, Some(100), 20)])
        .unwrap();
    publish(&mut writer, [1; 32]);
    writer.begin(source).unwrap();
    writer
        .stage(&[observation(1, source, 2, Some(100), 40)])
        .unwrap();
    while !writer.reconcile_slice().unwrap() {}

    let mut reader = Ledger::open_reader(&path).unwrap();
    let pinned = reader.connection.transaction().unwrap();
    let pinned_meta = info(&pinned).unwrap();
    assert_eq!(pinned_meta.published_epoch, 1);
    let new_checkpoint = Checkpoint {
        provider: Provider::Claude,
        root_key: [8; 32],
        fingerprint: Some([2; 32]),
        prefix_bytes: 2000,
        coverage: 0,
    };
    let result = writer.publish(new_checkpoint.clone());
    assert!(
        matches!(result, Err(Error::Sql(error)) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy))
    );
    assert_eq!(info(&pinned).unwrap().last_success_at_ms, Some(42));
    pinned.commit().unwrap();
    assert_eq!(read(&mut reader), [(Resolution::Resolved, Some(160))]);
    writer.publish(new_checkpoint).unwrap();
    assert_eq!(writer.info().unwrap().last_success_at_ms, Some(42));
    writer
        .mark_provider_refresh_success(Provider::Claude, 43)
        .unwrap();
    assert_eq!(writer.info().unwrap().last_success_at_ms, Some(43));
    assert_eq!(read(&mut reader), [(Resolution::Resolved, Some(180))]);
}

#[test]
fn cancellation_and_undated_coverage_preserve_snapshot_identity() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let mut ledger = Ledger::open_writer(&path).unwrap();
    let source = [7; 32];
    ledger.begin(source).unwrap();
    let mut value = observation(1, source, 1, Some(100), 20);
    value.at_ms = None;
    ledger.stage(&[value]).unwrap();
    publish(&mut ledger, [1; 32]);
    let cancelled = ledger.visit(
        0,
        10_000,
        Arc::new(AtomicBool::new(true)),
        |_| Ok(()),
        |_| Ok(()),
    );
    assert!(matches!(cancelled, Err(Error::Cancelled)));
    let coverage = ledger
        .visit(
            0,
            10_000,
            Arc::default(),
            |_| panic!("undated row in the daily interval"),
            |_| panic!("undated row in the model interval"),
        )
        .unwrap();
    assert_eq!(coverage.meta.published_epoch, 1);
    assert_eq!(coverage.for_provider(Provider::Claude).source_count, 1);
    assert!(coverage.undated);
}

#[test]
fn page_limit_failure_recovers_without_eviction() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let mut ledger = Ledger::open_writer(&path).unwrap();
    let source = [7; 32];
    ledger.begin(source).unwrap();
    ledger
        .stage(&[observation(1, source, 1, Some(100), 20)])
        .unwrap();
    publish(&mut ledger, [1; 32]);

    let pages = pragma_number(&ledger.connection, "page_count").unwrap();
    ledger
        .connection
        .pragma_update(None, "max_page_count", pages + 2)
        .unwrap();
    ledger.begin(source).unwrap();
    let pending: Vec<_> = (2..400)
        .map(|id| observation(id, source, 1, Some(100), 20))
        .collect();
    assert!(matches!(ledger.stage(&pending), Err(Error::Limit)));
    assert_eq!(read(&mut ledger), [(Resolution::Resolved, Some(160))]);
    while !ledger.recover_slice().unwrap() {}
    assert_eq!(read(&mut ledger), [(Resolution::Resolved, Some(160))]);
    assert_eq!(ledger.info().unwrap().published_epoch, 1);
}

#[test]
fn no_spill_delete_journal_stays_inside_its_original_page_envelope() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let mut ledger = Ledger::open_writer(&path).unwrap();
    let source = [7; 32];
    ledger.begin(source).unwrap();
    let pending: Vec<_> = (0..20_000)
        .map(|id| observation(id, source, 1, Some(100), 20))
        .collect();
    ledger.stage(&pending).unwrap();
    let original_size = fs::metadata(&path).unwrap().len();
    let original_pages = pragma_number(&ledger.connection, "page_count").unwrap();
    assert!(original_size > 4 * 1024 * 1024);
    let transaction = ledger.connection.transaction().unwrap();
    transaction
        .execute("UPDATE usage_observations SET output=output+1", [])
        .unwrap();

    let journal = sidecar(&path, "-journal");
    let journal_size = fs::metadata(&journal).unwrap().len();
    assert_eq!(original_size, fs::metadata(&path).unwrap().len());
    assert!(journal_size > 4 * 1024 * 1024);
    assert!(journal_size <= original_pages * (PAGE_SIZE + 8) + 65_536);
    transaction.rollback().unwrap();
    assert!(!journal.exists());
    let output: i64 = ledger
        .connection
        .query_row("SELECT MAX(output) FROM usage_observations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(output, 20);
}

#[test]
fn upgrade_preserves_claude_and_reconciles_codex_reasoning_independently() {
    let fixture = Fixture::new();
    let path = fixture.path();
    let mut writer = Ledger::open_writer(&path).unwrap();
    let claude_source = [1; 32];
    let codex_source = [2; 32];

    writer.begin(claude_source).unwrap();
    writer
        .stage(&[observation(1, claude_source, 1, Some(100), 20)])
        .unwrap();
    assert!(matches!(writer.upgrade_for_codex(), Err(Error::Invalid)));
    publish(&mut writer, [3; 32]);
    assert_eq!(checked_version(&writer.connection).unwrap(), 1);

    let mut reader = Ledger::open_reader(&path).unwrap();
    assert_eq!(read(&mut reader), [(Resolution::Resolved, Some(160))]);
    writer.upgrade_for_codex().unwrap();
    assert_eq!(checked_version(&writer.connection).unwrap(), 2);

    let mut missing = observation(2, codex_source, 1, Some(70), 20);
    missing.provider = Provider::Codex;
    missing.counts.cache_write = Some(0);
    let mut known = missing.clone();
    known.evidence_key = [6; 32];
    known.counts.reasoning_output = Some(5);
    writer.begin(codex_source).unwrap();
    writer.stage(&[known, missing.clone()]).unwrap();
    while !writer.reconcile_slice().unwrap() {}
    writer
        .publish(Checkpoint {
            provider: Provider::Codex,
            root_key: [8; 32],
            fingerprint: Some([4; 32]),
            prefix_bytes: 1000,
            coverage: 2,
        })
        .unwrap();
    writer
        .mark_provider_refresh_success(Provider::Codex, 99)
        .unwrap();
    assert_eq!(
        writer.checkpoint(codex_source).unwrap().unwrap().provider,
        Provider::Codex
    );

    let mut contribution = None;
    let coverage = reader
        .visit(
            0,
            2000,
            Arc::new(AtomicBool::new(false)),
            |row| {
                if row.provider == Provider::Codex {
                    contribution = Some(row);
                }
                Ok(())
            },
            |_| Ok(()),
        )
        .unwrap();
    let contribution = contribution.unwrap();
    assert_eq!(contribution.counts.reasoning_output, Some(5));
    assert_eq!(contribution.counts.known_tokens(), Some(120));
    assert_eq!(coverage.for_provider(Provider::Claude).source_flags, 0);
    assert_eq!(coverage.for_provider(Provider::Codex).source_flags, 2);
    assert_eq!(coverage.meta.last_success_at_ms, Some(42));
    assert_eq!(coverage.meta.codex_last_success_at_ms, Some(99));

    missing.evidence_key = [7; 32];
    missing.counts.reasoning_output = Some(4);
    writer.begin(codex_source).unwrap();
    writer.stage(&[missing]).unwrap();
    while !writer.reconcile_slice().unwrap() {}
    writer
        .publish(Checkpoint {
            provider: Provider::Codex,
            root_key: [8; 32],
            fingerprint: Some([5; 32]),
            prefix_bytes: 1000,
            coverage: 2,
        })
        .unwrap();
    let mut states = Vec::new();
    reader
        .visit(
            0,
            2000,
            Arc::new(AtomicBool::new(false)),
            |row| {
                states.push((row.provider, row.resolution, row.counts.known_tokens()));
                Ok(())
            },
            |_| Ok(()),
        )
        .unwrap();
    assert_eq!(
        states,
        [
            (Provider::Claude, Resolution::Resolved, Some(160)),
            (Provider::Codex, Resolution::Conflicted, None)
        ]
    );
}
