//! Tracked deterministic Windows pipe peer, built only to exercise P01 tests.
#[cfg(not(windows))]
fn main() {}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::{Value, json};
    use std::{path::PathBuf, time::Duration};
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::windows::named_pipe::ServerOptions,
        runtime::Builder,
    };
    let mode = std::env::current_exe()?
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("normal")
        .to_owned();
    let path = std::env::args_os()
        .find_map(|arg| {
            arg.to_str()
                .and_then(|arg| arg.strip_prefix("--input-ipc-server="))
                .map(PathBuf::from)
        })
        .ok_or("missing test pipe")?;
    let runtime = Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?;
    runtime.block_on(async {
        if mode == "missing-pipe" {
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(path)?;
        server.connect().await?;
        let mut server = BufReader::new(server);
        let mut count = 0;
        loop {
            let mut line = String::new();
            if server.read_line(&mut line).await? == 0 {
                break;
            }
            let message: Value = serde_json::from_str(&line)?;
            let command = message["command"][0].as_str().unwrap_or("");
            let property = message["command"][1].as_str().unwrap_or("");
            if command == "quit" {
                if mode == "stall-quit" {
                    continue;
                }
                break;
            }
            if mode == "slow-startup" || (property == "playlist-count" && mode == "slow-command") {
                for _ in 0..100 {
                    server.get_mut().write_all(b" ").await?;
                    tokio::time::sleep(Duration::from_millis(120)).await;
                }
            }
            if property == "playlist-count" {
                if mode == "die" {
                    std::process::exit(1);
                }
                if mode == "malformed" {
                    server.get_mut().write_all(b"invalid\n").await?;
                    continue;
                }
                if mode == "oversized" {
                    server.get_mut().write_all(&vec![b' '; 65_537]).await?;
                    continue;
                }
            }
            if command == "loadfile" {
                if message["command"][2] == "append" {
                    count += 1;
                } else {
                    count = 1;
                }
            }
            let mut error = "success";
            let mut data = if property == "playlist-count" {
                json!(count)
            } else if property == "mpv-version" {
                json!("fixture")
            } else {
                Value::Null
            };
            if property == "time-pos" {
                match mode.as_str() {
                    "unavailable-position" => error = "property unavailable",
                    "rejected-position" => error = "property not found",
                    "negative-position" => data = json!(-1),
                    "normal" => data = json!(0),
                    _ => {}
                }
            }
            // Events and unrelated responses must not complete the current request.
            let event = json!({"event":"property-change", "data": "fixture-only"});
            let stale = json!({"request_id":0, "error":"success", "data":999});
            for reply in [
                event,
                stale,
                json!({"request_id":message["request_id"], "error":error, "data":data}),
            ] {
                let mut bytes = serde_json::to_vec(&reply)?;
                bytes.push(b'\n');
                server.get_mut().write_all(&bytes).await?;
            }
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })
}
