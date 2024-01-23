mod nuke_task;

use clap::Parser;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::cmp;
#[macro_use] extern crate rocket;
use rocket::State;
use rocket::serde::{Serialize, Deserialize, json::Json};
use crate::nuke_task::{NukeTask, NukeTaskConfig};

const AFTER_HELP: &str = r#"
Example:
  simplecachenuker --port 1234 --path '/var/cache/foo/*' --service foo.service

This will start the daemon on port 1234. A POST request can then be
sent to http://localhost:1234/ with a JSON request body. This will
trigger the cache nuke. All files, directories, symlinks beneath
/var/cache/foo/ will be removed, after which the foo service will be
restarted.

An empty JSON document "{}" will trigger the cache nuke immediately.
By specifying the "delay" property, the trigger will be delayed by the
given amount of seconds. Any further cache nuke requests will be
combined until the delay is over. This makes it easy to implement
"debouncing": Simply send a cache nuke request for every change to the
content, and the delay property will make sure the cache is cleared
only sparingly.

Example:
  curl -X POST -H "Content-Type: application/json" --data '{"delay": 10}' http://localhost:1234/
"#;

/// Simple daemon which clears cache directories and restarts systemd services
/// after receiving a HTTP request.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, after_help=AFTER_HELP, arg_required_else_help = true)]
struct Args {
    /// Port to listen on
    #[arg(long)]
    port: u16,

    /// Name of a systemd service to stop and start. May be specified multiple times.
    #[arg(long)]
    service: Vec<String>,

    /// Glob pattern to match files to delete while the service(s) have been stopped.
    /// Directories are deleted recursively. May be specified multiple times.
    #[arg(long)]
    path: Vec<String>,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct IndexResult {
    name: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct NukeRequest {
    /// The maximum amount of seconds the cache nuke may be delayed
    #[serde(default)]
    delay: u32,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct NukeResult {
    ok: bool,
}


#[get("/")]
fn index() -> Json<IndexResult> {
    Json(IndexResult {
        name: "simplecachenuker".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[post("/", data = "<request>")]
async fn index_post(nuke_task: &State<NukeTask>, request: Json<NukeRequest>) -> Json<NukeResult> {
    let delay = cmp::max(request.delay, 0);
    let no_later_than = Instant::now() + Duration::from_secs(delay.into());
    nuke_task.nuke(no_later_than).await;
    Json(NukeResult { ok: true })
}

#[launch]
async fn rocket() -> _ {
    let args = Args::parse();
    let config = NukeTaskConfig {
        service: args.service,
        path: args.path,
    };

    let nuke_task = NukeTask::spawn(Arc::new(config));

    rocket::build()
        .configure(
            rocket::Config::figment()
                .merge(("port", args.port))
                .merge(("address", "127.0.0.1"))
                .merge(("log_level", "normal"))
                .merge(("ident", "simplecachenuker"))
        )
        .manage(nuke_task)
        .mount("/", routes![index])
        .mount("/", routes![index_post])
}
