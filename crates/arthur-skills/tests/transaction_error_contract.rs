#![cfg(unix)]

use std::error::Error;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use arthur_skills::transaction::{SignalFlags, TransactionEngine, TransactionLock, snapshot_path};

#[test]
fn transaction_io_boundaries_fail_closed() -> Result<(), Box<dyn Error>> {
    let temp = tempfile::tempdir()?;
    let overlong = temp.path().join("x".repeat(300));
    assert!(snapshot_path(&overlong).is_err());
    assert!(TransactionLock::acquire(&overlong).is_err());

    let state = temp.path().join("state");
    fs::create_dir(&state)?;
    fs::set_permissions(&state, fs::Permissions::from_mode(0o700))?;
    let journal = state.join("transaction.json");
    fs::write(&journal, b"not-json")?;
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o600))?;
    let engine = TransactionEngine::new(state, SignalFlags::default());
    assert!(engine.journal_state().is_err());
    Ok(())
}
