use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use glob::glob;
use tokio::process::Command;
use tokio::spawn;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::time::sleep;

pub struct NukeTaskConfig {
    pub service: Vec<String>,
    pub path: Vec<String>,
}

pub struct NukeTaskMessage {
    pub no_later_than: Instant,
}

pub struct NukeTask {
    tx: mpsc::Sender<NukeTaskMessage>,
}

impl NukeTask {
    pub async fn nuke(&self, no_later_than: Instant) {
        let message = NukeTaskMessage { no_later_than };
        self.tx.send(message).await.expect("NukeTask.tx failed to send");
    }

    pub fn spawn(config: Arc<NukeTaskConfig>) -> NukeTask {
        let (tx, mut rx) = mpsc::channel::<NukeTaskMessage>(128);

        spawn(async move {
            let mut next_nuke: Option<Instant> = None;

            loop {
                loop {
                    match rx.try_recv() {
                        Ok(message) => {
                            if next_nuke.map_or(true, |next| message.no_later_than < next) {
                                next_nuke = Some(message.no_later_than);
                            }
                        },
                        Err(TryRecvError::Disconnected) => { return },
                        Err(TryRecvError::Empty) => { break },
                    };
                }

                if next_nuke.map_or(false, |next| next <= Instant::now()) {
                    nuke(config.clone()).await;
                    next_nuke = None;
                }

                sleep(Duration::from_secs(1)).await;
            }
        });

        NukeTask { tx }
    }
}

async fn nuke(config: Arc<NukeTaskConfig>) {
    log::info!("Nuking the cache...");

    for service in &config.service {
        log::info!("Stopping service {}...", service);

        let command = Command::new("/usr/bin/systemctl")
            .arg("stop")
            .arg(service)
            .status();

        match command.await {
            Err(err) => log::error!("\"systemctl stop {service}\" command failed to run: {err}"),
            Ok(status) => {
                if status.success() {
                    log::info!("Stopped service {}", service);
                } else {
                    log::error!("\"systemctl stop {service}\" command failed: {status}");
                }
            }
        }
    }

    {
        let config = config.clone();

        tokio::task::spawn_blocking(move || {
            for path in &config.path {
                let paths = match glob(path) {
                    Err(err) => {
                        log::error!("Invalid glob pattern \"{path}\": {err}");
                        continue;
                    },
                    Ok(paths) => paths,
                };

                for path in paths {
                    let path = match path {
                        Err(err) => {
                            let path = err.path().display();
                            log::error!("Failed to read path at \"{path}\": {err}");
                            continue;
                        },
                        Ok(x) => x,
                    };
                    let path_display = path.display();

                    if path.as_path() == Path::new("/") {
                        log::error!("Refusing to delete '/'");
                        continue;
                    }

                    let metadata = match fs::metadata(&path) {
                        Err(err) => {
                            log::error!("Failed to stat \"{path_display}\": {err}");
                            continue;
                        },
                        Ok(x) => x,
                    };

                    if metadata.is_dir() {
                        if let Err(err) = fs::remove_dir_all(&path) {
                            log::error!("Failed to recursively delete \"{path_display}\": {err}");
                        }
                    }
                    else {
                        if let Err(err) = fs::remove_file(&path) {
                            log::error!("Failed to delete \"{path_display}\": {err}");
                        }
                    }

                    log::info!("Deleted {path_display}");
                }
            }
        }).await.expect("spawn_blocking failed during nuke()");
    }

    for service in &config.service {
        log::info!("Starting service {}...", service);

        let command = Command::new("/usr/bin/systemctl")
            .arg("start")
            .arg(service)
            .status();

        match command.await {
            Err(err) => log::error!("\"systemctl start {service}\" command failed to run: {err}"),
            Ok(status) => {
                if status.success() {
                    log::info!("Started service {}", service);
                } else {
                    log::error!("\"systemctl start {service}\" command failed: {status}");
                }
            }
        }
    }
}
