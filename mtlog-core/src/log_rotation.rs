use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use chrono::Utc;
use uuid::Uuid;

use crate::log_writer::{LogFile, LogWriter, replace_line_in_file};

#[cfg(not(test))]
mod limits {
    pub const MIN_ROTATION_DURATION_MS: u64 = 1_000;
    pub const MIN_FILE_SIZE: u64 = 4_096;
    pub const MIN_LINES: u64 = 10;
}

#[cfg(test)]
mod limits {
    pub const MIN_ROTATION_DURATION_MS: u64 = 10;
    pub const MIN_FILE_SIZE: u64 = 1_024;
    pub const MIN_LINES: u64 = 1;
}

/// A file logger that supports single-file, time-based rotation, or size-based rotation.
pub enum FileLogger {
    Single(LogFile),
    TimeRotation(LogFileTimeRotation),
    SizeRotation(LogFileSizeRotation),
}

impl LogWriter for FileLogger {
    fn regular(&mut self, line: &str) {
        match self {
            FileLogger::Single(w) => w.regular(line),
            FileLogger::TimeRotation(w) => w.regular(line),
            FileLogger::SizeRotation(w) => w.regular(line),
        }
    }

    fn progress(&mut self, line: &str, id: Uuid) {
        match self {
            FileLogger::Single(w) => w.progress(line, id),
            FileLogger::TimeRotation(w) => w.progress(line, id),
            FileLogger::SizeRotation(w) => w.progress(line, id),
        }
    }

    fn finished(&mut self, id: Uuid) {
        match self {
            FileLogger::Single(w) => w.finished(id),
            FileLogger::TimeRotation(w) => w.finished(id),
            FileLogger::SizeRotation(w) => w.finished(id),
        }
    }

    fn flush(&mut self) {
        match self {
            FileLogger::Single(w) => w.flush(),
            FileLogger::TimeRotation(w) => w.flush(),
            FileLogger::SizeRotation(w) => w.flush(),
        }
    }
}

/// Configuration for time-based log file rotation.
pub struct TimeRotationConfig {
    pub folder: PathBuf,
    pub filename: String,
    pub extension: String,
    pub rotation_duration: Duration,
    pub cleanup_after: Duration,
}

/// A log file writer that rotates files based on time intervals.
pub struct LogFileTimeRotation {
    folder: PathBuf,
    filename: String,
    extension: String,
    rotation_duration: Duration,
    cleanup_after: Duration,
    current_file: BufWriter<File>,
    file_opened_at: Instant,
    progress_positions: HashMap<Uuid, u64>,
    progress_content: HashMap<Uuid, String>,
}

impl LogFileTimeRotation {
    pub fn new(config: TimeRotationConfig) -> Result<Self, std::io::Error> {
        if config.rotation_duration.as_millis() < limits::MIN_ROTATION_DURATION_MS as u128 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "rotation_duration must be at least {} ms",
                    limits::MIN_ROTATION_DURATION_MS
                ),
            ));
        }
        fs::create_dir_all(&config.folder)?;
        let file = open_timestamped_file(&config.folder, &config.filename, &config.extension)?;
        Ok(Self {
            folder: config.folder,
            filename: config.filename,
            extension: config.extension,
            rotation_duration: config.rotation_duration,
            cleanup_after: config.cleanup_after,
            current_file: file,
            file_opened_at: Instant::now(),
            progress_positions: HashMap::new(),
            progress_content: HashMap::new(),
        })
    }

    fn should_rotate(&self) -> bool {
        self.file_opened_at.elapsed() >= self.rotation_duration
    }

    fn rotate(&mut self) {
        self.current_file.flush().unwrap();
        let new_file =
            open_timestamped_file(&self.folder, &self.filename, &self.extension).unwrap();
        self.current_file = new_file;
        self.file_opened_at = Instant::now();

        // Migrate active progress bars to the new file
        let mut new_positions = HashMap::new();
        for (id, content) in &self.progress_content {
            self.current_file.flush().unwrap();
            let pos = self.current_file.get_ref().metadata().unwrap().len();
            writeln!(self.current_file, "{content}").unwrap();
            new_positions.insert(*id, pos);
        }
        self.current_file.flush().unwrap();
        self.progress_positions = new_positions;

        self.cleanup();
    }

    fn cleanup(&self) {
        let Ok(entries) = fs::read_dir(&self.folder) else {
            return;
        };
        let prefix = format!("{}_{}", self.filename, ""); // e.g. "myproject_"
        let suffix = format!(".{}", self.extension);
        let now = Utc::now();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(&prefix) || !name.ends_with(&suffix) {
                continue;
            }
            let timestamp_str = &name[prefix.len()..name.len() - suffix.len()];
            if let Ok(file_time) =
                chrono::NaiveDateTime::parse_from_str(timestamp_str, "%Y%m%d%H%M%S%6f")
            {
                let file_utc = file_time.and_utc();
                if let Ok(age) = (now - file_utc).to_std()
                    && age > self.cleanup_after
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
}

impl LogWriter for LogFileTimeRotation {
    fn regular(&mut self, line: &str) {
        if self.should_rotate() {
            self.rotate();
        }
        writeln!(self.current_file, "{line}").unwrap();
    }

    fn progress(&mut self, line: &str, id: Uuid) {
        if self.should_rotate() {
            self.rotate();
        }
        self.current_file.flush().unwrap();
        if let Some(pos) = self.progress_positions.get(&id) {
            replace_line_in_file(&mut self.current_file, line, *pos);
        } else {
            let pos = self.current_file.get_ref().metadata().unwrap().len();
            self.progress_positions.insert(id, pos);
            writeln!(self.current_file, "{line}").unwrap();
        }
        self.progress_content.insert(id, line.to_string());
    }

    fn finished(&mut self, id: Uuid) {
        self.progress_positions.remove(&id);
        self.progress_content.remove(&id);
        self.current_file.flush().unwrap();
    }

    fn flush(&mut self) {
        self.current_file.flush().unwrap();
    }
}

/// Configuration for size-based log file rotation.
pub struct SizeRotationConfig {
    pub folder: PathBuf,
    pub filename: String,
    pub extension: String,
    pub max_file_size: Option<u64>,
    pub max_lines: Option<u64>,
    pub max_files: u32,
}

/// A log file writer that rotates files based on size or line count.
pub struct LogFileSizeRotation {
    folder: PathBuf,
    filename: String,
    extension: String,
    max_file_size: Option<u64>,
    max_lines: Option<u64>,
    max_files: u32,
    current_file: BufWriter<File>,
    current_lines: u64,
    progress_positions: HashMap<Uuid, u64>,
    progress_content: HashMap<Uuid, String>,
}

impl LogFileSizeRotation {
    pub fn new(config: SizeRotationConfig) -> Result<Self, std::io::Error> {
        if let Some(max_size) = config.max_file_size
            && max_size < limits::MIN_FILE_SIZE
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "max_file_size must be at least {} bytes",
                    limits::MIN_FILE_SIZE
                ),
            ));
        }
        if let Some(max_lines) = config.max_lines
            && max_lines < limits::MIN_LINES
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("max_lines must be at least {}", limits::MIN_LINES),
            ));
        }
        if config.max_files < 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "max_files must be at least 1",
            ));
        }
        if config.max_file_size.is_none() && config.max_lines.is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "at least one of max_file_size or max_lines must be set",
            ));
        }
        fs::create_dir_all(&config.folder)?;
        let file = open_timestamped_file(&config.folder, &config.filename, &config.extension)?;
        Ok(Self {
            folder: config.folder,
            filename: config.filename,
            extension: config.extension,
            max_file_size: config.max_file_size,
            max_lines: config.max_lines,
            max_files: config.max_files,
            current_file: file,
            current_lines: 0,
            progress_positions: HashMap::new(),
            progress_content: HashMap::new(),
        })
    }

    fn should_rotate(&mut self) -> bool {
        if let Some(max_lines) = self.max_lines
            && self.current_lines >= max_lines
        {
            return true;
        }
        if let Some(max_size) = self.max_file_size {
            self.current_file.flush().unwrap();
            if self.current_file.get_ref().metadata().unwrap().len() >= max_size {
                return true;
            }
        }
        false
    }

    fn rotate(&mut self) {
        self.current_file.flush().unwrap();
        let new_file =
            open_timestamped_file(&self.folder, &self.filename, &self.extension).unwrap();
        self.current_file = new_file;
        self.current_lines = 0;

        // Migrate active progress bars to the new file
        let mut new_positions = HashMap::new();
        for (id, content) in &self.progress_content {
            self.current_file.flush().unwrap();
            let pos = self.current_file.get_ref().metadata().unwrap().len();
            writeln!(self.current_file, "{content}").unwrap();
            new_positions.insert(*id, pos);
            self.current_lines += 1;
        }
        self.current_file.flush().unwrap();
        self.progress_positions = new_positions;

        self.cleanup();
    }

    fn cleanup(&self) {
        let Ok(entries) = fs::read_dir(&self.folder) else {
            return;
        };
        let prefix = format!("{}_", self.filename);
        let suffix = format!(".{}", self.extension);
        let mut matching_files: Vec<PathBuf> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&prefix) && name.ends_with(&suffix) {
                    Some(entry.path())
                } else {
                    None
                }
            })
            .collect();
        // Sort lexicographically (chronological due to timestamp naming)
        matching_files.sort();
        // Delete oldest files until count <= max_files
        while matching_files.len() > self.max_files as usize {
            if let Some(oldest) = matching_files.first() {
                let _ = fs::remove_file(oldest);
            }
            matching_files.remove(0);
        }
    }
}

impl LogWriter for LogFileSizeRotation {
    fn regular(&mut self, line: &str) {
        if self.should_rotate() {
            self.rotate();
        }
        writeln!(self.current_file, "{line}").unwrap();
        self.current_lines += 1;
    }

    fn progress(&mut self, line: &str, id: Uuid) {
        if self.should_rotate() {
            self.rotate();
        }
        self.current_file.flush().unwrap();
        if let Some(pos) = self.progress_positions.get(&id) {
            replace_line_in_file(&mut self.current_file, line, *pos);
        } else {
            let pos = self.current_file.get_ref().metadata().unwrap().len();
            self.progress_positions.insert(id, pos);
            writeln!(self.current_file, "{line}").unwrap();
            self.current_lines += 1;
        }
        self.progress_content.insert(id, line.to_string());
    }

    fn finished(&mut self, id: Uuid) {
        self.progress_positions.remove(&id);
        self.progress_content.remove(&id);
        self.current_file.flush().unwrap();
    }

    fn flush(&mut self) {
        self.current_file.flush().unwrap();
    }
}

fn open_timestamped_file(
    folder: &Path,
    filename: &str,
    extension: &str,
) -> Result<BufWriter<File>, std::io::Error> {
    use std::io::{Seek, SeekFrom};
    let timestamp = Utc::now().format("%Y%m%d%H%M%S%6f");
    let path = folder.join(format!("{filename}_{timestamp}.{extension}"));
    let mut file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    file.seek(SeekFrom::End(0))?;
    Ok(BufWriter::new(file))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn test_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from(format!("/tmp/mtlog_test_{name}"));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn count_log_files(dir: &PathBuf, filename: &str, extension: &str) -> usize {
        let prefix = format!("{filename}_");
        let suffix = format!(".{extension}");
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with(&prefix) && name.ends_with(&suffix)
            })
            .count()
    }

    fn read_all_log_content(dir: &PathBuf, filename: &str, extension: &str) -> String {
        let prefix = format!("{filename}_");
        let suffix = format!(".{extension}");
        let mut files: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with(&prefix) && name.ends_with(&suffix) {
                    Some(e.path())
                } else {
                    None
                }
            })
            .collect();
        files.sort();
        let mut content = String::new();
        for f in files {
            content.push_str(&fs::read_to_string(f).unwrap());
        }
        content
    }

    #[test]
    fn test_time_rotation_creates_multiple_files() {
        let dir = test_dir("time_rotation");
        let mut writer = LogFileTimeRotation::new(TimeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            rotation_duration: Duration::from_millis(50),
            cleanup_after: Duration::from_secs(3600),
        })
        .unwrap();

        writer.regular("line1");
        writer.flush();
        assert_eq!(count_log_files(&dir, "app", "log"), 1);

        thread::sleep(Duration::from_millis(60));
        writer.regular("line2");
        writer.flush();
        assert_eq!(count_log_files(&dir, "app", "log"), 2);

        thread::sleep(Duration::from_millis(60));
        writer.regular("line3");
        writer.flush();
        assert_eq!(count_log_files(&dir, "app", "log"), 3);

        let content = read_all_log_content(&dir, "app", "log");
        assert!(content.contains("line1"));
        assert!(content.contains("line2"));
        assert!(content.contains("line3"));
    }

    #[test]
    fn test_size_rotation_by_lines() {
        let dir = test_dir("size_rotation_lines");
        let mut writer = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: None,
            max_lines: Some(3),
            max_files: 10,
        })
        .unwrap();

        for i in 0..9 {
            writer.regular(&format!("line{i}"));
        }
        writer.flush();

        assert_eq!(count_log_files(&dir, "app", "log"), 3);

        let content = read_all_log_content(&dir, "app", "log");
        for i in 0..9 {
            assert!(content.contains(&format!("line{i}")));
        }
    }

    #[test]
    fn test_size_rotation_cleanup_max_files() {
        let dir = test_dir("size_rotation_cleanup");
        let mut writer = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: None,
            max_lines: Some(1),
            max_files: 3,
        })
        .unwrap();

        // Write 6 lines → should create 6 files but cleanup keeps only 3
        for i in 0..6 {
            // Small sleep to ensure unique timestamps
            thread::sleep(Duration::from_millis(2));
            writer.regular(&format!("line{i}"));
        }
        writer.flush();

        assert_eq!(count_log_files(&dir, "app", "log"), 3);
    }

    #[test]
    fn test_time_rotation_cleanup() {
        let dir = test_dir("time_rotation_cleanup");
        // Create some fake "old" files manually
        fs::create_dir_all(&dir).unwrap();
        let old_timestamp = "20200101000000000000";
        let old_file = dir.join(format!("app_{old_timestamp}.log"));
        File::create(&old_file).unwrap();

        let mut writer = LogFileTimeRotation::new(TimeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            rotation_duration: Duration::from_millis(50),
            cleanup_after: Duration::from_secs(1),
        })
        .unwrap();

        writer.regular("line1");
        writer.flush();

        // Trigger rotation so cleanup runs
        thread::sleep(Duration::from_millis(60));
        writer.regular("line2");
        writer.flush();

        // Old file should have been cleaned up
        assert!(!old_file.exists());
        // Current files should still exist
        assert!(count_log_files(&dir, "app", "log") >= 1);
    }

    #[test]
    fn test_progress_bar_migration_on_rotation() {
        let dir = test_dir("progress_migration");
        let mut writer = LogFileTimeRotation::new(TimeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            rotation_duration: Duration::from_millis(50),
            cleanup_after: Duration::from_secs(3600),
        })
        .unwrap();

        let progress_id = Uuid::new_v4();
        // Use same-length strings (progress bars use fixed-width content)
        writer.progress("progress: 050%", progress_id);
        writer.flush();

        // Trigger rotation
        thread::sleep(Duration::from_millis(60));
        writer.regular("after rotation");
        writer.flush();

        assert_eq!(count_log_files(&dir, "app", "log"), 2);

        // The new file should contain the migrated progress bar
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("app_") && name.ends_with(".log") {
                    Some(e.path())
                } else {
                    None
                }
            })
            .collect();
        files.sort();
        let newest = fs::read_to_string(files.last().unwrap()).unwrap();
        assert!(newest.contains("progress: 050%"));
        assert!(newest.contains("after rotation"));

        // Update progress after rotation should work (same byte length)
        writer.progress("progress: 100%", progress_id);
        writer.flush();
        let newest = fs::read_to_string(files.last().unwrap()).unwrap();
        assert!(newest.contains("progress: 100%"));
        assert!(!newest.contains("progress: 050%"));
    }

    #[test]
    fn test_validation_time_rotation_duration() {
        let dir = test_dir("validation_time");
        let result = LogFileTimeRotation::new(TimeRotationConfig {
            folder: dir,
            filename: "app".into(),
            extension: "log".into(),
            rotation_duration: Duration::from_millis(1), // too small
            cleanup_after: Duration::from_secs(3600),
        });
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_validation_size_rotation_file_size() {
        let dir = test_dir("validation_size");
        let result = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir,
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: Some(100), // too small
            max_lines: None,
            max_files: 5,
        });
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_validation_size_rotation_max_files() {
        let dir = test_dir("validation_max_files");
        let result = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir,
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: Some(4096),
            max_lines: None,
            max_files: 0, // too small
        });
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_validation_no_rotation_trigger() {
        let dir = test_dir("validation_no_trigger");
        let result = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir,
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: None,
            max_lines: None, // neither set
            max_files: 5,
        });
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_file_logger_enum_delegates() {
        let dir = test_dir("file_logger_enum");
        fs::create_dir_all(&dir).unwrap();
        let log_file = LogFile::new(dir.join("test.log")).unwrap();
        let mut logger = FileLogger::Single(log_file);
        logger.regular("hello");
        logger.flush();
        assert!(
            fs::read_to_string(dir.join("test.log"))
                .unwrap()
                .contains("hello")
        );
    }

    #[test]
    fn test_size_rotation_by_file_size() {
        let dir = test_dir("size_rotation_file_size");
        let mut writer = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: Some(1024),
            max_lines: None,
            max_files: 20,
        })
        .unwrap();

        // Each line is ~100 bytes; 30 lines = ~3000 bytes → should create >= 3 files
        for i in 0..30 {
            thread::sleep(Duration::from_millis(1));
            writer.regular(&format!(
                "line{i:03} padding to make this line about one hundred bytes long............."
            ));
        }
        writer.flush();

        let file_count = count_log_files(&dir, "app", "log");
        assert!(
            file_count >= 3,
            "expected >= 3 files from file-size rotation, got {file_count}"
        );

        let content = read_all_log_content(&dir, "app", "log");
        for i in 0..30 {
            assert!(content.contains(&format!("line{i:03}")));
        }
    }

    #[test]
    fn test_size_rotation_lines_trigger_first() {
        let dir = test_dir("size_rotation_lines_first");
        let mut writer = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: Some(10240), // won't be hit
            max_lines: Some(2),         // triggers first
            max_files: 20,
        })
        .unwrap();

        for i in 0..6 {
            thread::sleep(Duration::from_millis(1));
            writer.regular(&format!("short{i}"));
        }
        writer.flush();

        assert_eq!(count_log_files(&dir, "app", "log"), 3);
    }

    #[test]
    fn test_size_rotation_bytes_trigger_first() {
        let dir = test_dir("size_rotation_bytes_first");
        let mut writer = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: Some(1024), // triggers first
            max_lines: Some(1000),     // won't be hit
            max_files: 20,
        })
        .unwrap();

        // ~200 bytes per line; 20 lines = ~4000 bytes → should create >= 3 files
        for i in 0..20 {
            thread::sleep(Duration::from_millis(1));
            writer.regular(&format!(
                "line{i:03} this is padded to roughly two hundred bytes of content so that file size triggers before line count does, padding padding padding padding padding padding padding pad"
            ));
        }
        writer.flush();

        let file_count = count_log_files(&dir, "app", "log");
        assert!(
            file_count >= 3,
            "expected >= 3 files from byte-size rotation, got {file_count}"
        );

        let content = read_all_log_content(&dir, "app", "log");
        for i in 0..20 {
            assert!(content.contains(&format!("line{i:03}")));
        }
    }

    #[test]
    fn test_size_rotation_progress_bar_migration() {
        let dir = test_dir("size_progress_migration");
        let mut writer = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: None,
            max_lines: Some(3),
            max_files: 20,
        })
        .unwrap();

        let progress_id = Uuid::new_v4();
        writer.progress("progress: 050%", progress_id);
        writer.regular("filler line one..");
        writer.regular("filler line two..");
        // 3 lines written (1 progress + 2 regular) → next write triggers rotation
        thread::sleep(Duration::from_millis(1));
        writer.regular("after rotation..");
        writer.flush();

        assert_eq!(count_log_files(&dir, "app", "log"), 2);

        // The new file should contain the migrated progress bar
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("app_") && name.ends_with(".log") {
                    Some(e.path())
                } else {
                    None
                }
            })
            .collect();
        files.sort();
        let newest = fs::read_to_string(files.last().unwrap()).unwrap();
        assert!(newest.contains("progress: 050%"));
        assert!(newest.contains("after rotation.."));

        // Update progress after rotation (same byte length)
        writer.progress("progress: 100%", progress_id);
        writer.flush();
        let newest = fs::read_to_string(files.last().unwrap()).unwrap();
        assert!(newest.contains("progress: 100%"));
        assert!(!newest.contains("progress: 050%"));
    }

    #[test]
    fn test_multiple_progress_bars_migration() {
        let dir = test_dir("multi_progress_migration");
        let mut writer = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: None,
            max_lines: Some(5),
            max_files: 20,
        })
        .unwrap();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();
        writer.progress("bar1: 000%", id1);
        writer.progress("bar2: 000%", id2);
        writer.progress("bar3: 000%", id3);
        writer.regular("filler line 01");
        writer.regular("filler line 02");
        // 5 lines → next write triggers rotation
        thread::sleep(Duration::from_millis(1));
        writer.regular("after rotation");
        writer.flush();

        assert_eq!(count_log_files(&dir, "app", "log"), 2);

        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("app_") && name.ends_with(".log") {
                    Some(e.path())
                } else {
                    None
                }
            })
            .collect();
        files.sort();
        let newest = fs::read_to_string(files.last().unwrap()).unwrap();
        assert!(newest.contains("bar1: 000%"));
        assert!(newest.contains("bar2: 000%"));
        assert!(newest.contains("bar3: 000%"));

        // Update each bar independently (same byte length)
        writer.progress("bar1: 100%", id1);
        writer.progress("bar2: 050%", id2);
        writer.progress("bar3: 075%", id3);
        writer.flush();
        let newest = fs::read_to_string(files.last().unwrap()).unwrap();
        assert!(newest.contains("bar1: 100%"));
        assert!(newest.contains("bar2: 050%"));
        assert!(newest.contains("bar3: 075%"));
        assert!(!newest.contains("bar1: 000%"));
        assert!(!newest.contains("bar2: 000%"));
        assert!(!newest.contains("bar3: 000%"));
    }

    #[test]
    fn test_finished_prevents_migration() {
        let dir = test_dir("finished_no_migrate");
        let mut writer = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: None,
            max_lines: Some(4),
            max_files: 20,
        })
        .unwrap();

        let active_id = Uuid::new_v4();
        let finished_id = Uuid::new_v4();
        writer.progress("active bar 50%", active_id);
        writer.progress("done bar 100%%", finished_id);
        writer.finished(finished_id);
        writer.regular("filler line 01.");
        writer.regular("filler line 02.");
        // 4 lines → next write triggers rotation
        thread::sleep(Duration::from_millis(1));
        writer.regular("after rotation.");
        writer.flush();

        assert_eq!(count_log_files(&dir, "app", "log"), 2);

        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("app_") && name.ends_with(".log") {
                    Some(e.path())
                } else {
                    None
                }
            })
            .collect();
        files.sort();
        let newest = fs::read_to_string(files.last().unwrap()).unwrap();
        // Active bar should be migrated
        assert!(newest.contains("active bar 50%"));
        // Finished bar should NOT be migrated
        assert!(!newest.contains("done bar 100%"));
    }

    #[test]
    fn test_validation_size_rotation_max_lines() {
        let dir = test_dir("validation_max_lines");
        let result = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir,
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: None,
            max_lines: Some(0), // below MIN_LINES=1 in test mode
            max_files: 5,
        });
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_integration_spawn_log_thread_with_size_rotation() {
        use crate::utils::{LogMessage, spawn_log_thread_file};
        use log::Level;
        use std::sync::Arc;

        let dir = test_dir("integration_spawn_size");
        let writer = LogFileSizeRotation::new(SizeRotationConfig {
            folder: dir.clone(),
            filename: "app".into(),
            extension: "log".into(),
            max_file_size: None,
            max_lines: Some(3),
            max_files: 20,
        })
        .unwrap();

        let logger = FileLogger::SizeRotation(writer);
        let sender = spawn_log_thread_file(logger);

        for i in 0..9 {
            sender
                .send(Arc::new(LogMessage {
                    message: format!("msg{i}"),
                    level: Level::Info,
                    name: Some("test".into()),
                }))
                .unwrap();
        }
        // Send shutdown
        sender
            .send(Arc::new(LogMessage {
                message: "___SHUTDOWN___".into(),
                level: Level::Info,
                name: None,
            }))
            .unwrap();
        // shutdown() joins the thread
        sender.shutdown();

        let file_count = count_log_files(&dir, "app", "log");
        assert!(
            file_count >= 2,
            "expected multiple files from integration test, got {file_count}"
        );

        let content = read_all_log_content(&dir, "app", "log");
        for i in 0..9 {
            assert!(
                content.contains(&format!("msg{i}")),
                "missing msg{i} in output"
            );
        }
    }
}
