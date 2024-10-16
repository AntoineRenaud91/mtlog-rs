use mtlog_tokio::logger_config;
use mtlog_progress::LogProgressBar;

#[tokio::main]
async fn main() {
    logger_config()
        .with_log_file("/tmp/log_with_progress.log").unwrap()
        .scope_global(async move {
            log::info!("Hello, Top !");
            let h1 = tokio::spawn(async move {
                logger_config()
                    .scope_local(async move {
                        let pb = LogProgressBar::new(100, "Thread1");
                        for _ in 0..50 {
                            pb.inc(1);
                            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                        }
                    }).await;
            });
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            log::info!("Hello, Middle !");
            let h2 = tokio::spawn(async move {
                logger_config()
                    .scope_local(async move {
                        let pb = LogProgressBar::new(100, "Thread2");
                        for _ in 0..100 {
                            pb.inc(1);
                            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                        }
                        pb.finish();
                    }).await;
            });
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            log::info!("Hello, Bottom !");
            h1.await.unwrap();
            h2.await.unwrap();
        }).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let content = std::fs::read_to_string("/tmp/log_with_progress.log").unwrap();
    let mut lines = content.trim_end().lines().collect::<Vec<&str>>();
    lines = lines[lines.len()-5..].to_vec();
    assert!(lines[0].ends_with("Hello, Top !"));
    assert!(lines[1].ends_with(" 50/100  50%"));
    assert!(lines[2].ends_with("Hello, Middle !"));
    assert!(lines[3].ends_with("100/100 100%"));
    assert!(lines[4].ends_with("Hello, Bottom !"));
}