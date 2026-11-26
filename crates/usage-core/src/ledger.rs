use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, Transaction, params};

use crate::accounting::{Canonical, Counts, Observation, Reducer, Resolution};

const VERSION: i64 = 1;
const PARSER_VERSION: i64 = 1;
const PAGE_SIZE: u64 = 4096;
const MAX_PAGES: u64 = 49_152;
const DB_BYTES: u64 = PAGE_SIZE * MAX_PAGES;
// The pinned pager writes at most one original-page frame per page and a
// sector (up to 64 KiB) header. No cache spill means no additional headers.
const JOURNAL_BYTES: u64 = MAX_PAGES * (PAGE_SIZE + 8) + 65_536;
const PAGER_SOURCE_ID: &str =
    "2024-05-23 13:25:27 96c92aba00c8375bc32fafcdf12429c58bd8aabfcadab6683e35bbb9cdebf19e";
const MAX_EVENTS: i64 = 1_000_000;
const WRITE_BATCH: usize = 128;
const QUERY_TIME: Duration = Duration::from_secs(2);
type Key = [u8; 32];

#[derive(Debug)]
pub(crate) enum Error {
    Sql(rusqlite::Error),
    Io(std::io::Error),
    Missing,
    FutureVersion,
    Invalid,
    Limit,
    Cancelled,
    Deadline,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Sql(_) => "usage database operation failed",
            Self::Io(_) => "usage database could not be accessed",
            Self::Missing => "usage database has not been initialized",
            Self::FutureVersion => "usage database needs a newer version of Ivy",
            Self::Invalid => "usage database format or state is unsupported",
            Self::Limit => "local usage import limit reached",
            Self::Cancelled => "usage query cancelled",
            Self::Deadline => "usage query deadline reached",
        };
        f.write_str(label)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sql(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DiskFull) {
            return Self::Limit;
        }
        Self::Sql(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub(crate) struct Meta {
    pub(crate) store_id: Key,
    pub(crate) identity_salt: Key,
    pub(crate) published_epoch: i64,
    pub(crate) last_success_at_ms: Option<i64>,
}

#[derive(Clone, Debug)]
pub(crate) struct Checkpoint {
    pub(crate) root_key: Key,
    pub(crate) fingerprint: Option<Key>,
    pub(crate) prefix_bytes: u64,
    pub(crate) coverage: i64,
}

#[derive(Debug)]
pub(crate) struct Coverage {
    pub(crate) meta: Meta,
    pub(crate) source_flags: i64,
    pub(crate) source_count: u64,
    pub(crate) undated: bool,
}

#[derive(Debug)]
pub(crate) struct Contribution {
    pub(crate) at_ms: i64,
    pub(crate) model: Option<String>,
    pub(crate) counts: Counts,
    pub(crate) resolution: Resolution,
}

#[derive(Default)]
struct Reconciliation {
    after_event: Option<Key>,
    event: Option<EventReduction>,
    done: bool,
}

struct EventReduction {
    key: Key,
    cursor: Option<RevisionCursor>,
    reducer: Reducer,
}

struct RevisionCursor {
    tokens: [Option<i64>; 5],
    id: i64,
}

pub(crate) struct Ledger {
    connection: Connection,
    reconciliation: Reconciliation,
}

impl Ledger {
    pub(crate) fn open_writer(path: &Path) -> Result<Self> {
        let exists = check_files(path)?;
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        connection.busy_timeout(Duration::ZERO)?;
        let source_id: String =
            connection.query_row("SELECT sqlite_source_id()", [], |row| row.get(0))?;
        // The byte bound depends on this DELETE pager's no-spill commit path.
        if source_id != PAGER_SOURCE_ID {
            return Err(Error::Invalid);
        }
        let version = checked_version(&connection)?;
        if exists && version == 0 && pragma_number(&connection, "page_count")? != 0 {
            return Err(Error::Invalid);
        }
        if version != 0 {
            validate_existing(&connection)?;
        }
        connection.execute_batch(
            "PRAGMA synchronous=FULL;
             PRAGMA cache_spill=OFF;
             PRAGMA cache_size=-4096;
             PRAGMA mmap_size=0;
             PRAGMA temp_store=MEMORY;
             PRAGMA page_size=4096;",
        )?;
        let journal_mode: String =
            connection.query_row("PRAGMA journal_mode=DELETE", [], |row| row.get(0))?;
        if journal_mode != "delete"
            || pragma_number(&connection, "cache_spill")? != 0
            || connection.query_row("PRAGMA max_page_count=49152", [], |row| {
                row.get::<_, u64>(0)
            })? != MAX_PAGES
        {
            return Err(Error::Invalid);
        }
        let mut ledger = Self::new(connection);
        if version == 0 {
            ledger.initialize()?;
        }
        Ok(ledger)
    }

    pub(crate) fn open_reader(path: &Path) -> Result<Self> {
        if !check_files(path)? {
            return Err(Error::Missing);
        }
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        connection.busy_timeout(Duration::ZERO)?;
        if checked_version(&connection)? == 0 {
            return Err(Error::Missing);
        }
        validate_existing(&connection)?;
        connection.execute_batch(
            "PRAGMA query_only=ON; PRAGMA cache_size=-4096; PRAGMA mmap_size=0;
             PRAGMA temp_store=MEMORY;",
        )?;
        Ok(Self::new(connection))
    }

    fn new(connection: Connection) -> Self {
        Self {
            connection,
            reconciliation: Reconciliation::default(),
        }
    }

    fn initialize(&mut self) -> Result<()> {
        let mut store_id = [0u8; 32];
        let mut identity_salt = [0u8; 32];
        getrandom::fill(&mut store_id).map_err(|_| Error::Invalid)?;
        getrandom::fill(&mut identity_salt).map_err(|_| Error::Invalid)?;

        let transaction = self.connection.transaction()?;
        transaction.execute_batch(include_str!("schema.sql"))?;
        transaction.execute(
            "INSERT INTO usage_meta
             (singleton, store_id, identity_salt, parser_version, published_epoch, stable_event_count)
             VALUES (1, ?1, ?2, ?3, 0, 0)",
            params![store_id, identity_salt, PARSER_VERSION],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn info(&self) -> Result<Meta> {
        info(&self.connection)
    }

    pub(crate) fn checkpoint(&self, source_key: Key) -> Result<Option<Checkpoint>> {
        Ok(self
            .connection
            .query_row(
                "SELECT root_key, published_fingerprint, published_prefix_bytes, coverage
                 FROM usage_sources WHERE source_key=?1",
                [source_key],
                |row| {
                    Ok(Checkpoint {
                        root_key: row.get(0)?,
                        fingerprint: row.get(1)?,
                        prefix_bytes: row.get(2)?,
                        coverage: row.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    pub(crate) fn begin(&mut self, source_key: Key) -> Result<()> {
        if self.connection.execute(
            "UPDATE usage_meta SET preparing_source_key=?1
             WHERE singleton=1 AND preparing_source_key IS NULL",
            [source_key],
        )? != 1
        {
            return Err(Error::Invalid);
        }
        self.reconciliation = Reconciliation::default();
        Ok(())
    }

    pub(crate) fn stage(&mut self, observations: &[Observation]) -> Result<()> {
        if self.reconciliation.after_event.is_some()
            || self.reconciliation.event.is_some()
            || self.reconciliation.done
        {
            return Err(Error::Invalid);
        }
        for batch in observations.chunks(WRITE_BATCH) {
            let transaction = self.connection.transaction()?;
            let candidate = candidate(&transaction)?.ok_or(Error::Invalid)?;
            stage_batch(&transaction, &candidate, batch)?;
            transaction.commit()?;
        }
        Ok(())
    }

    pub(crate) fn reconcile_slice(&mut self) -> Result<bool> {
        if self.reconciliation.done {
            return Ok(true);
        }
        // A retry can rebuild candidate rows from scratch; committed epochs stay fixed.
        let mut state = std::mem::take(&mut self.reconciliation);
        let transaction = self.connection.transaction()?;
        let candidate = candidate(&transaction)?.ok_or(Error::Invalid)?;
        reconcile_batch(&transaction, candidate.epoch, &mut state)?;
        transaction.commit()?;
        self.reconciliation = state;
        Ok(self.reconciliation.done)
    }

    pub(crate) fn publish(&mut self, checkpoint: Checkpoint) -> Result<()> {
        if !self.reconciliation.done {
            return Err(Error::Invalid);
        }
        let transaction = self.connection.transaction()?;
        let candidate = candidate(&transaction)?.ok_or(Error::Invalid)?;
        transaction.execute(
            "INSERT INTO usage_sources
             (source_key, root_key, published_fingerprint, published_prefix_bytes, coverage)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(source_key) DO UPDATE SET root_key=excluded.root_key,
               published_fingerprint=excluded.published_fingerprint,
               published_prefix_bytes=excluded.published_prefix_bytes, coverage=excluded.coverage",
            params![
                candidate.source,
                checkpoint.root_key,
                checkpoint.fingerprint,
                checkpoint.prefix_bytes,
                checkpoint.coverage,
            ],
        )?;
        transaction.execute(
            "UPDATE usage_meta SET published_epoch=?1, preparing_source_key=NULL WHERE singleton=1",
            [candidate.epoch],
        )?;
        transaction.commit()?;
        self.reconciliation = Reconciliation::default();
        Ok(())
    }

    pub(crate) fn mark_refresh_success(&mut self, timestamp_ms: i64) -> Result<()> {
        if self.connection.execute(
            "UPDATE usage_meta SET last_success_at_ms=?1
             WHERE singleton=1 AND preparing_source_key IS NULL",
            [timestamp_ms],
        )? != 1
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    pub(crate) fn recover_slice(&mut self) -> Result<bool> {
        self.reconciliation = Reconciliation::default();
        let transaction = self.connection.transaction()?;
        let Some(candidate) = candidate(&transaction)? else {
            return Ok(true);
        };
        let done = recover_batch(&transaction, candidate.epoch)?;
        transaction.commit()?;
        Ok(done)
    }

    pub(crate) fn reap_slice(&mut self) -> Result<bool> {
        let transaction = self.connection.transaction()?;
        let removed = transaction.execute(
            "DELETE FROM usage_canonical WHERE id IN (
                SELECT id FROM usage_canonical WHERE until_epoch <= (
                    SELECT published_epoch FROM usage_meta WHERE singleton=1
                ) ORDER BY until_epoch, id LIMIT ?1
             )",
            [WRITE_BATCH],
        )?;
        transaction.commit()?;
        Ok(removed < WRITE_BATCH)
    }

    pub(crate) fn visit(
        &mut self,
        start_ms: i64,
        end_ms: i64,
        cancel: Arc<AtomicBool>,
        daily: impl FnMut(Contribution) -> Result<()>,
        model: impl FnMut(Contribution) -> Result<()>,
    ) -> Result<Coverage> {
        let deadline = Instant::now() + QUERY_TIME;
        let interrupt = Arc::clone(&cancel);
        self.connection.progress_handler(
            1000,
            Some(move || interrupt.load(Ordering::Relaxed) || Instant::now() >= deadline),
        );

        let result = visit(
            &mut self.connection,
            VisitRange {
                start_ms,
                end_ms,
                deadline,
                cancel: &cancel,
            },
            daily,
            model,
        );
        self.connection.progress_handler(0, None::<fn() -> bool>);
        check_deadline(&cancel, deadline)?;
        result
    }
}

fn check_files(path: &Path) -> Result<bool> {
    let exists = check_regular_size(path, DB_BYTES)?;
    let journal = check_regular_size(&sidecar(path, "-journal"), JOURNAL_BYTES)?;
    let wal = check_regular_size(&sidecar(path, "-wal"), 0)?;
    let shm = check_regular_size(&sidecar(path, "-shm"), 0)?;
    if wal || shm || (!exists && journal) {
        return Err(Error::Invalid);
    }
    Ok(exists)
}

fn sidecar(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    name.into()
}

fn check_regular_size(path: &Path, max: u64) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && meta.len() <= max => Ok(true),
        Ok(_) => Err(Error::Invalid),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn pragma_number(connection: &Connection, name: &str) -> Result<u64> {
    Ok(connection.pragma_query_value(None, name, |row| row.get(0))?)
}

fn checked_version(connection: &Connection) -> Result<i64> {
    let version = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        0 | VERSION => Ok(version),
        n if n > VERSION => Err(Error::FutureVersion),
        _ => Err(Error::Invalid),
    }
}

fn validate_existing(connection: &Connection) -> Result<()> {
    if pragma_number(connection, "page_size")? != PAGE_SIZE
        || pragma_number(connection, "page_count")? > MAX_PAGES
    {
        return Err(Error::Invalid);
    }
    let journal_mode: String =
        connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    if journal_mode != "delete" {
        return Err(Error::Invalid);
    }
    let parser: i64 = connection.query_row(
        "SELECT parser_version FROM usage_meta WHERE singleton=1",
        [],
        |row| row.get(0),
    )?;
    match parser {
        PARSER_VERSION => Ok(()),
        n if n > PARSER_VERSION => Err(Error::FutureVersion),
        _ => Err(Error::Invalid),
    }
}

fn info(connection: &Connection) -> Result<Meta> {
    Ok(connection.query_row(
        "SELECT store_id, identity_salt, published_epoch, last_success_at_ms
         FROM usage_meta WHERE singleton=1",
        [],
        |row| {
            Ok(Meta {
                store_id: row.get(0)?,
                identity_salt: row.get(1)?,
                published_epoch: row.get(2)?,
                last_success_at_ms: row.get(3)?,
            })
        },
    )?)
}

struct Candidate {
    epoch: i64,
    source: Key,
    count: i64,
}

fn candidate(connection: &Connection) -> Result<Option<Candidate>> {
    let (epoch, source, count): (i64, Option<Key>, i64) = connection.query_row(
        "SELECT published_epoch, preparing_source_key, stable_event_count
         FROM usage_meta WHERE singleton=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let epoch = epoch.checked_add(1).ok_or(Error::Invalid)?;
    Ok(source.map(|source| Candidate {
        epoch,
        source,
        count,
    }))
}

fn stage_batch(
    transaction: &Transaction<'_>,
    candidate: &Candidate,
    observations: &[Observation],
) -> Result<()> {
    let mut count = candidate.count;
    let mut insert_key = transaction.prepare_cached(
        "INSERT INTO usage_keys(event_key, first_epoch) VALUES (?1, ?2)
         ON CONFLICT(event_key) DO NOTHING",
    )?;
    let mut insert_observation = transaction.prepare_cached(
        "INSERT INTO usage_observations (
            event_key, source_key, evidence_key, first_epoch, event_at_ms, model, model_key,
            uncached_input, cache_read, cache_write, cache_write_1h, output, category_flags
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(event_key, source_key, evidence_key) DO NOTHING",
    )?;
    for observation in observations {
        if observation.source_key != candidate.source {
            return Err(Error::Invalid);
        }
        if insert_key.execute(params![observation.event_key, candidate.epoch])? == 1 {
            count += 1;
            if count > MAX_EVENTS {
                return Err(Error::Limit);
            }
        }
        let counts = &observation.counts;
        insert_observation.execute(params![
            observation.event_key,
            observation.source_key,
            observation.evidence_key,
            candidate.epoch,
            observation.at_ms,
            observation.model,
            observation.model_key,
            counts.uncached_input,
            counts.cache_read,
            counts.cache_write,
            counts.cache_write_1h,
            counts.output,
            counts.flags,
        ])?;
    }
    transaction.execute(
        "UPDATE usage_meta SET stable_event_count=?1 WHERE singleton=1",
        [count],
    )?;
    Ok(())
}

fn reconcile_batch(
    transaction: &Transaction<'_>,
    epoch: i64,
    state: &mut Reconciliation,
) -> Result<()> {
    let mut remaining = WRITE_BATCH;
    while remaining != 0 {
        let mut current = match state.event.take() {
            Some(current) => current,
            None => {
                let Some(key) = next_dirty_event(transaction, epoch, state.after_event)? else {
                    state.done = true;
                    return Ok(());
                };
                EventReduction {
                    key,
                    cursor: None,
                    reducer: Reducer::default(),
                }
            }
        };
        let consumed = read_revisions(transaction, epoch, &mut current, remaining)?;
        remaining -= consumed;
        if remaining == 0 {
            state.event = Some(current);
            break;
        }

        let canonical = current.reducer.finish().ok_or(Error::Invalid)?;
        stage_canonical(transaction, current.key, epoch, canonical)?;
        state.after_event = Some(current.key);
    }
    Ok(())
}

fn next_dirty_event(
    transaction: &Transaction<'_>,
    epoch: i64,
    after: Option<Key>,
) -> Result<Option<Key>> {
    let result = if let Some(after) = after {
        transaction
            .prepare_cached(
                "SELECT event_key FROM usage_observations INDEXED BY usage_observations_epoch
                 WHERE first_epoch=?1 AND event_key>?2 ORDER BY event_key LIMIT 1",
            )?
            .query_row(params![epoch, after], |row| row.get(0))
    } else {
        transaction
            .prepare_cached(
                "SELECT event_key FROM usage_observations INDEXED BY usage_observations_epoch
                 WHERE first_epoch=?1 ORDER BY event_key LIMIT 1",
            )?
            .query_row([epoch], |row| row.get(0))
    };
    Ok(result.optional()?)
}

const REVISION_COLUMNS: [&str; 5] = [
    "uncached_input",
    "cache_read",
    "cache_write",
    "cache_write_1h",
    "output",
];
const REVISION_SELECT: &str =
    "SELECT id, source_key, evidence_key, event_at_ms, model, model_key, uncached_input,
            cache_read, cache_write, cache_write_1h, output, category_flags
     FROM usage_observations INDEXED BY usage_observations_revision
     WHERE event_key=? AND first_epoch<=?";
const REVISION_ORDER: &str =
    " ORDER BY uncached_input, cache_read, cache_write, cache_write_1h, output, id LIMIT ?";

fn read_revisions(
    transaction: &Transaction<'_>,
    epoch: i64,
    current: &mut EventReduction,
    limit: usize,
) -> Result<usize> {
    let Some(cursor) = current.cursor.take() else {
        let mut statement =
            transaction.prepare_cached(&format!("{REVISION_SELECT}{REVISION_ORDER}"))?;
        let mut rows = statement.query(params![current.key, epoch, limit])?;
        return reduce_rows(current, &mut rows);
    };

    // SQLite tuple comparison with a NULL member is unknown. Walk disjoint
    // index ranges from the longest equal prefix, including NULL -> non-NULL.
    let mut consumed = 0;
    for pivot in (0..=REVISION_COLUMNS.len()).rev() {
        let mut sql = REVISION_SELECT.to_owned();
        let mut values: Vec<&dyn rusqlite::ToSql> = vec![&current.key, &epoch];
        for (index, column) in REVISION_COLUMNS.iter().take(pivot).enumerate() {
            sql.push_str(&format!(" AND {column} IS ?"));
            values.push(&cursor.tokens[index]);
        }
        if let Some(column) = REVISION_COLUMNS.get(pivot) {
            if cursor.tokens[pivot].is_some() {
                sql.push_str(&format!(" AND {column}>?"));
                values.push(&cursor.tokens[pivot]);
            } else {
                sql.push_str(&format!(" AND {column} IS NOT NULL"));
            }
        } else {
            sql.push_str(" AND id>?");
            values.push(&cursor.id);
        }
        let remaining = limit - consumed;
        sql.push_str(REVISION_ORDER);
        values.push(&remaining);
        let mut statement = transaction.prepare_cached(&sql)?;
        let mut rows = statement.query(values.as_slice())?;
        consumed += reduce_rows(current, &mut rows)?;
        if consumed == limit {
            break;
        }
    }
    Ok(consumed)
}

fn reduce_rows(current: &mut EventReduction, rows: &mut rusqlite::Rows<'_>) -> Result<usize> {
    let mut consumed = 0;
    while let Some(row) = rows.next()? {
        let counts = counts_from_row(row, 6)?;
        current.cursor = Some(RevisionCursor {
            tokens: [
                counts.uncached_input,
                counts.cache_read,
                counts.cache_write,
                counts.cache_write_1h,
                counts.output,
            ],
            id: row.get(0)?,
        });
        current.reducer.push_sorted(Observation {
            event_key: current.key,
            source_key: row.get(1)?,
            evidence_key: row.get(2)?,
            at_ms: row.get(3)?,
            model: row.get(4)?,
            model_key: row.get(5)?,
            counts,
        });
        consumed += 1;
    }
    Ok(consumed)
}

fn stage_canonical(
    transaction: &Transaction<'_>,
    key: Key,
    epoch: i64,
    value: Canonical,
) -> Result<()> {
    transaction
        .prepare_cached(
            "UPDATE usage_canonical SET until_epoch=?2
         WHERE id=(
            SELECT id FROM usage_canonical INDEXED BY usage_canonical_identity
            WHERE event_key=?1 AND from_epoch<?2 AND (until_epoch IS NULL OR until_epoch=?2)
            ORDER BY from_epoch DESC LIMIT 1
         )",
        )?
        .execute(params![key, epoch])?;
    let counts = value.counts;
    transaction
        .prepare_cached(
            "INSERT INTO usage_canonical
            (event_key, from_epoch, event_at_ms, model, uncached_input, cache_read,
             cache_write, cache_write_1h, output, category_flags, resolution)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(event_key, from_epoch) DO UPDATE SET
            event_at_ms=excluded.event_at_ms, model=excluded.model,
            uncached_input=excluded.uncached_input, cache_read=excluded.cache_read,
            cache_write=excluded.cache_write, cache_write_1h=excluded.cache_write_1h,
            output=excluded.output, category_flags=excluded.category_flags,
            resolution=excluded.resolution",
        )?
        .execute(params![
            key,
            epoch,
            value.at_ms,
            value.model,
            counts.uncached_input,
            counts.cache_read,
            counts.cache_write,
            counts.cache_write_1h,
            counts.output,
            counts.flags,
            value.resolution as i64,
        ])?;
    Ok(())
}

fn recover_batch(transaction: &Transaction<'_>, epoch: i64) -> Result<bool> {
    let removed = transaction.execute(
        "DELETE FROM usage_observations WHERE id IN (
            SELECT id FROM usage_observations WHERE first_epoch=?1 ORDER BY event_key, id LIMIT ?2
         )",
        params![epoch, WRITE_BATCH],
    )?;
    let staged = transaction.execute(
        "DELETE FROM usage_canonical WHERE id IN (
            SELECT id FROM usage_canonical WHERE from_epoch=?1 ORDER BY id LIMIT ?2
         )",
        params![epoch, WRITE_BATCH],
    )?;
    let reopened = transaction.execute(
        "UPDATE usage_canonical SET until_epoch=NULL WHERE id IN (
            SELECT id FROM usage_canonical WHERE until_epoch=?1 ORDER BY id LIMIT ?2
         )",
        params![epoch, WRITE_BATCH],
    )?;
    let keys = transaction.execute(
        "DELETE FROM usage_keys WHERE event_key IN (
            SELECT event_key FROM usage_keys WHERE first_epoch=?1 ORDER BY event_key LIMIT ?2
         )",
        params![epoch, WRITE_BATCH],
    )?;
    let done = [removed, staged, reopened, keys]
        .into_iter()
        .all(|count| count < WRITE_BATCH);
    transaction.execute(
        "UPDATE usage_meta SET stable_event_count=stable_event_count-?1,
            preparing_source_key=CASE WHEN ?2 THEN NULL ELSE preparing_source_key END
         WHERE singleton=1",
        params![keys, done],
    )?;
    Ok(done)
}

struct VisitRange<'a> {
    start_ms: i64,
    end_ms: i64,
    deadline: Instant,
    cancel: &'a AtomicBool,
}

fn visit(
    connection: &mut Connection,
    range: VisitRange<'_>,
    daily: impl FnMut(Contribution) -> Result<()>,
    model: impl FnMut(Contribution) -> Result<()>,
) -> Result<Coverage> {
    check_deadline(range.cancel, range.deadline)?;
    let transaction = connection.transaction()?;
    let meta = info(&transaction)?;
    let epoch = meta.published_epoch;
    visit_contributions(
        &transaction,
        "SELECT event_at_ms, model, uncached_input, cache_read, cache_write,
                cache_write_1h, output, category_flags, resolution
         FROM usage_canonical INDEXED BY usage_canonical_day
         WHERE event_at_ms>=?1 AND event_at_ms<?2 AND from_epoch<=?3
           AND (until_epoch IS NULL OR until_epoch>?3)
         ORDER BY event_at_ms, id",
        &range,
        epoch,
        daily,
    )?;
    visit_contributions(
        &transaction,
        "SELECT event_at_ms, model, uncached_input, cache_read, cache_write,
                cache_write_1h, output, category_flags, resolution
         FROM usage_canonical INDEXED BY usage_canonical_model
         WHERE event_at_ms>=?1 AND event_at_ms<?2 AND from_epoch<=?3
           AND (until_epoch IS NULL OR until_epoch>?3)
         ORDER BY model, event_at_ms, id",
        &range,
        epoch,
        model,
    )?;
    let undated = transaction.query_row(
        "SELECT EXISTS(
            SELECT id FROM usage_canonical INDEXED BY usage_canonical_day
            WHERE event_at_ms IS NULL AND from_epoch<=?1 AND (until_epoch IS NULL OR until_epoch>?1)
         )",
        [epoch],
        |row| row.get(0),
    )?;
    let (mut source_flags, mut source_count) = (0, 0);
    {
        let mut statement =
            transaction.prepare_cached("SELECT coverage FROM usage_sources ORDER BY source_key")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            check_deadline(range.cancel, range.deadline)?;
            source_flags |= row.get::<_, i64>(0)?;
            source_count += 1;
        }
    }
    transaction.commit()?;
    Ok(Coverage {
        meta,
        source_flags,
        source_count,
        undated,
    })
}

fn visit_contributions(
    transaction: &Transaction<'_>,
    sql: &str,
    range: &VisitRange<'_>,
    epoch: i64,
    mut emit: impl FnMut(Contribution) -> Result<()>,
) -> Result<()> {
    let mut statement = transaction.prepare_cached(sql)?;
    let mut rows = statement.query(params![range.start_ms, range.end_ms, epoch])?;
    while let Some(row) = rows.next()? {
        check_deadline(range.cancel, range.deadline)?;
        let resolution = Resolution::try_from(row.get::<_, i64>(8)?).map_err(|_| Error::Invalid)?;
        emit(Contribution {
            at_ms: row.get(0)?,
            model: row.get(1)?,
            counts: counts_from_row(row, 2)?,
            resolution,
        })?;
    }
    Ok(())
}

fn counts_from_row(row: &Row<'_>, start: usize) -> rusqlite::Result<Counts> {
    Ok(Counts {
        uncached_input: row.get(start)?,
        cache_read: row.get(start + 1)?,
        cache_write: row.get(start + 2)?,
        cache_write_1h: row.get(start + 3)?,
        output: row.get(start + 4)?,
        flags: row.get(start + 5)?,
    })
}

fn check_deadline(cancel: &AtomicBool, deadline: Instant) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(Error::Cancelled)
    } else if Instant::now() >= deadline {
        Err(Error::Deadline)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
