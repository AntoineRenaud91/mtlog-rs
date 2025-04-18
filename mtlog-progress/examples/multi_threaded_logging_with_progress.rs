use mtlog::logger_config;
use mtlog_progress::LogProgressBar;


fn main() {
    std::fs::remove_file("/tmp/log_with_progress.log").ok();
    logger_config()
        .with_log_file("/tmp/log_with_progress.log").unwrap()
        .init_global();
    log::info!("Hello, Top !");
    std::thread::spawn(move || {
        let pb = LogProgressBar::new(100, "Thread1")
            .with_min_timestep_ms(5.);
        for _ in 0..50 {
            pb.inc(1);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        log::logger().flush();
    });
    std::thread::sleep(std::time::Duration::from_millis(2));
    log::info!("Hello, Middle !");
    std::thread::spawn(move || {
        let pb = LogProgressBar::new(100, "Thread2")
            .with_min_timestep_ms(5.);
        for _ in 0..100 {
            pb.inc(1);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        pb.finish();
        log::logger().flush();
    });
    std::thread::sleep(std::time::Duration::from_millis(2));
    log::info!("Hello, Bottom !");
    log::logger().flush();
    std::thread::sleep(std::time::Duration::from_millis(500));
    let content = std::fs::read_to_string("/tmp/log_with_progress.log").unwrap();
    let lines = content.trim_end().lines().collect::<Vec<&str>>();
    println!("{}",lines.join("\n"));
    assert!(lines[0].ends_with("Hello, Top !"),"{}",lines[0]);
    assert!(lines[1].ends_with(" 50/100  50%"),"{}",lines[1]);
    assert!(lines[2].ends_with("Hello, Middle !"),"{}",lines[2]);
    assert!(lines[3].ends_with("100/100 100%"),"{}",lines[3]);
    assert!(lines[4].ends_with("Hello, Bottom !"),"{}",lines[4]);
}