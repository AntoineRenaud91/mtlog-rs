use std::path::PathBuf;

use mtlog::{SizeRotationConfig, logger_config};

fn main() {
    let dir = PathBuf::from("/tmp/mtlog_example_rotation");
    let _ = std::fs::remove_dir_all(&dir);

    let _guard = logger_config()
        .with_name("rotation-demo")
        .with_size_rotation(SizeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: None,
            max_lines: Some(10),
            max_files: 3,
        })
        .unwrap()
        .init_global();

    for i in 0..50 {
        log::info!("Log message number {i}");
    }

    drop(_guard);

    let files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("app_") && name.ends_with(".log") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    println!("\n--- Rotation Summary ---");
    println!("Log directory: {}", dir.display());
    println!("Files remaining (max_files=3): {}", files.len());
    for f in &files {
        println!("  {f}");
    }
    assert!(files.len() <= 3, "max_files cleanup should keep at most 3");
}
