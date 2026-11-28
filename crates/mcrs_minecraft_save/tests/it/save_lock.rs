use mcrs_minecraft_save::{SaveError, SaveLock};

#[test]
fn a_held_save_cannot_be_taken_again_until_its_lock_drops() {
    let root = std::env::temp_dir().join(format!("mcrs-save-lock-{}", uuid::Uuid::new_v4()));
    let dir = root.join("world");
    assert!(!SaveLock::is_held(&dir).unwrap());

    let lock = SaveLock::take(&dir).unwrap();
    assert!(SaveLock::is_held(&dir).unwrap());
    assert!(matches!(SaveLock::take(&dir), Err(SaveError::Locked { dir: named }) if named == dir));
    assert!(
        SaveLock::is_held(&dir).unwrap(),
        "a failed take keeps the lock"
    );

    drop(lock);
    assert!(!SaveLock::is_held(&dir).unwrap());
    assert_eq!(
        std::fs::read(dir.join("session.lock")).unwrap(),
        [0xE2, 0x98, 0x83]
    );
    drop(SaveLock::take(&dir).unwrap());
    std::fs::remove_dir_all(root).unwrap();
}
