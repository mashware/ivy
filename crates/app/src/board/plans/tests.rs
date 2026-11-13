#![cfg(test)]
use super::*;
use std::{
    fs,
    sync::mpsc,
    time::{Duration, UNIX_EPOCH},
};
fn identity() -> Identity {
    Identity {
        key: Key::Repository("/sample/.git".into()),
        path: "/sample/.git".into(),
        name: "Sample".into(),
    }
}
fn set(text: &str) -> What {
    let (plan, dropped) = panel_contract::plan::parse_document_noting(text).unwrap();
    What::PlanSet {
        document: text.into(),
        plan,
        dropped,
    }
}
fn temp() -> PathBuf {
    let mut bytes = [0; 8];
    getrandom::fill(&mut bytes).unwrap();
    let p = std::env::temp_dir().join(format!("ivy-plan-test-{:x}", u64::from_ne_bytes(bytes)));
    fs::create_dir(&p).unwrap();
    p
}
#[test]
fn committed_plan_roundtrips_without_republication_and_retirement_removes_it() {
    let directory = temp();
    let bank = Bank::default();
    let identity = identity();
    let document = r#"{"title":"Test","milestones":[],"producerExtra":true}"#;
    let mut disk = disk::Disk::new(Some(directory.clone()), true);
    assert!(
        matches!(apply(&bank,&mut disk,&identity,set(document)),Answer::Text(s) if s=="published for Sample")
    );
    let at = bank
        .lock()
        .unwrap()
        .get(&identity.key)
        .unwrap()
        .published_at;
    let mut restored_disk = disk::Disk::new(Some(directory.clone()), true);
    let restored = restored_disk.restore();
    let published = restored.get(&identity.key).unwrap();
    assert_eq!(published.document, document);
    assert_eq!(
        published
            .published_at
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        at.duration_since(UNIX_EPOCH).unwrap().as_secs()
    );
    let restored = Arc::new(Mutex::new(restored));
    assert!(
        matches!(apply(&restored,&mut restored_disk,&identity,What::PlanGet),Answer::Document(s) if s==document)
    );
    assert!(
        matches!(apply(&restored,&mut restored_disk,&identity,What::PlanRetire),Answer::Text(s) if s=="retired for Sample")
    );
    assert!(restored_disk.restore().is_empty());
    assert!(
        matches!(apply(&restored,&mut restored_disk,&identity,What::PlanGet),Answer::Text(s) if s=="no plan published for Sample")
    );
    fs::remove_dir_all(directory).unwrap();
}
#[test]
fn worker_answers_from_committed_state_and_names_dropped_words() {
    let ctx = eframe::egui::Context::default();
    let mut store = Store::at(None, false, ctx.clone());
    let (tx, rx) = mpsc::sync_channel(1);
    store.request(
        Ok(identity()),
        set(r#"{"milestones":[{"title":"M","items":[{"text":"T","mark":"magic"}]}]}"#),
        tx,
        &ctx,
    );
    assert!(
        matches!(rx.recv_timeout(Duration::from_secs(3)).unwrap(),Answer::Text(s) if s.contains("published for Sample")&&s.contains("magic")&&s.contains("not saved"))
    );
    assert!(store.published(&identity().key).is_some());
    let (tx, rx) = mpsc::sync_channel(1);
    store.request(
        Err("this console's project is not known yet".into()),
        What::PlanRetire,
        tx,
        &ctx,
    );
    assert!(matches!(rx.recv().unwrap(), Answer::Refused(_)));
    assert!(store.published(&identity().key).is_some());
}
#[test]
fn unknown_version_is_preserved_and_measurement_never_overwrites() {
    let directory = temp();
    let identity = identity();
    let mut disk = disk::Disk::new(Some(directory.clone()), true);
    disk.save(
        &identity,
        "{\"milestones\":[]}",
        UNIX_EPOCH + Duration::from_secs(100),
    )
    .unwrap();
    let path = fs::read_dir(&directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original = fs::read_to_string(&path)
        .unwrap()
        .replace("\"v\":1", "\"v\":2");
    fs::write(&path, &original).unwrap();
    assert!(disk.restore().is_empty());
    assert!(disk.save(&identity, "{}", SystemTime::now()).is_err());
    assert!(disk.retire(&identity).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    let mut measured = disk::Disk::new(Some(directory.clone()), false);
    assert!(measured.save(&identity, "{}", SystemTime::now()).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    fs::remove_dir_all(directory).unwrap();
}
