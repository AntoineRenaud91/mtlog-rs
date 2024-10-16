use mtlog::logger_config;
use mtlog_progress::LogProgressBar;


fn main() {
    logger_config()
        .with_log_file("/tmp/log_with_progress.log").unwrap()
        .init_global();
    log::info!("Hello, Top !");
    std::thread::spawn(move || {
        let pb = LogProgressBar::new(100, "Thread1");
        for _ in 0..50 {
            pb.inc(1);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    });
    std::thread::sleep(std::time::Duration::from_millis(2));
    log::info!("Hello, Middle !");
    std::thread::spawn(move || {
        let pb = LogProgressBar::new(100, "Thread1");
        for _ in 0..100 {
            pb.inc(1);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        pb.finish();
    });
    std::thread::sleep(std::time::Duration::from_millis(2));
    log::info!("Hello, Bottom !");
    std::thread::sleep(std::time::Duration::from_millis(200));
    let content = std::fs::read_to_string("/tmp/log_with_progress.log").unwrap();
    let mut lines = content.trim_end().lines().collect::<Vec<&str>>();
    lines = lines[lines.len()-5..].to_vec();
    assert!(lines[0].ends_with("Hello, Top !"));
    assert!(lines[1].ends_with(" 50/100  50%"));
    assert!(lines[2].ends_with("Hello, Middle !"));
    assert!(lines[3].ends_with("100/100 100%"));
    assert!(lines[4].ends_with("Hello, Bottom !"));
}