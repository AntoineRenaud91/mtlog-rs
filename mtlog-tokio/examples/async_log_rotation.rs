use std::path::PathBuf;
use std::time::Duration;

use mtlog_tokio::{logger_config, TimeRotationConfig};

#[tokio::main]
async fn main() {
    let dir = PathBuf::from("/tmp/mtlog_example_async_rotation");
    let _ = std::fs::remove_dir_all(&dir);

    logger_config()
        .with_name("async-rotation-demo")
        .with_time_rotation(TimeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            rotation_duration: Duration::from_secs(1),
            cleanup_after: Duration::from_secs(3600),
        })
        .unwrap()
        .scope_global(async move {
            for batch in 0..3 {
                log::info!("Batch {batch} - message A");
                log::info!("Batch {batch} - message B");
                if batch < 2 {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                }
            }
        })
        .await;

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

    println!("\n--- Async Rotation Summary ---");
    println!("Log directory: {}", dir.display());
    println!("Files created: {}", files.len());
    for f in &files {
        println!("  {f}");
    }
    assert!(files.len() >= 2, "expected at least 2 rotated files");
}
