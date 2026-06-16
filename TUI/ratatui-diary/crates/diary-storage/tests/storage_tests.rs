use chrono::NaiveDate;
use diary_storage::Storage;
use tempfile::TempDir;

#[test]
fn test_new_creates_entries_directory() {
    let temp = TempDir::new().unwrap();
    let _storage = Storage::with_dir(temp.path()).unwrap();

    let entries_dir = temp.path().join("entries");
    assert!(entries_dir.exists());
    assert!(entries_dir.is_dir());
}

#[test]
fn test_save_creates_markdown_file() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();

    let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
    let content = "Test diary content";

    storage.save(date, content).unwrap();

    let file_path = temp.path().join("entries/2026-02-14.md");
    assert!(file_path.exists());

    let saved = std::fs::read_to_string(file_path).unwrap();
    assert_eq!(saved, content);
}

#[test]
fn test_load_existing_diary() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();

    let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
    let content = "Existing diary";
    storage.save(date, content).unwrap();

    let loaded = storage.load(date).unwrap();
    assert_eq!(loaded, content);
}

#[test]
fn test_load_nonexistent_diary() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();

    let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
    let result = storage.load(date);

    assert!(result.is_err());
}

#[test]
fn test_delete_diary() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();

    let date = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
    storage.save(date, "test").unwrap();

    storage.delete(date).unwrap();

    let result = storage.load(date);
    assert!(result.is_err());
}

#[test]
fn test_scan_entries() {
    let temp = TempDir::new().unwrap();
    let storage = Storage::with_dir(temp.path()).unwrap();

    let date1 = NaiveDate::from_ymd_opt(2026, 2, 14).unwrap();
    let date2 = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
    storage.save(date1, "test1").unwrap();
    storage.save(date2, "test2").unwrap();

    let entries = storage.scan_entries().unwrap();

    assert_eq!(entries.len(), 2);
    assert!(entries.contains(&date1));
    assert!(entries.contains(&date2));
}

#[test]
fn test_new_uses_system_data_dir() {
    // Given: 시스템 데이터 디렉토리가 존재
    // When: Storage::new() 호출
    let result = Storage::new();

    // Then: 성공적으로 생성되거나 에러 반환
    match result {
        | Ok(storage) => {
            // 생성된 storage는 유효해야 함
            assert!(storage.scan_entries().is_ok());
        },
        | Err(e) => {
            // 에러 메시지 검증
            assert!(e.to_string().contains("Cannot find local data directory") || e.kind() == std::io::ErrorKind::NotFound);
        },
    }
}

#[test]
fn test_new_with_none_dir() {
    // Given: 데이터 디렉토리를 찾을 수 없는 상황 시뮬레이션
    // When: new_with_none_dir() 호출
    let result = Storage::new_with_none_dir();

    // Then: 에러가 반환되어야 함
    assert!(result.is_err());
    if let Err(error) = result {
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
        assert_eq!(error.to_string(), "Cannot find local data directory");
    }
}
