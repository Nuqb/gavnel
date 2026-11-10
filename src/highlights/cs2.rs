use super::*;
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
#[derive(Default)]
pub struct State {
    previous: Option<Value>,
}
impl State {
    pub fn update(&mut self, data: Value, ticks: u64) -> Vec<GameEvent> {
        if data["player"]["steamid"] != data["provider"]["steamid"]
            || data["player"]["activity"] != "playing"
        {
            self.previous = None;
            return vec![];
        }
        let mut events = vec![];
        if let Some(old) = &self.previous {
            let mut emit = |kind| {
                events.push(GameEvent {
                    game: Game::Cs2,
                    kind,
                    ticks,
                    source: "CS2 game-state integration".into(),
                    confidence: 1.0,
                })
            };
            for (field, kind) in [("kills", EventKind::Kill), ("assists", EventKind::Assist)] {
                let current = data["player"]["match_stats"][field].as_u64();
                let previous = old["player"]["match_stats"][field].as_u64();
                if current
                    .zip(previous)
                    .is_some_and(|(c, p)| c > p && c - p < 10)
                {
                    emit(kind);
                }
            }
            if data["player"]["state"]["health"] == 0
                && old["player"]["state"]["health"]
                    .as_u64()
                    .is_some_and(|h| h > 0)
            {
                emit(EventKind::Death);
            }
            if data["round"]["phase"] == "live" && old["round"]["phase"] != "live" {
                emit(EventKind::RoundStart);
            }
            if data["round"]["phase"] == "over"
                && old["round"]["phase"] != "over"
                && let Some(winner) = data["round"]["win_team"].as_str()
            {
                emit(if Some(winner) == data["player"]["team"].as_str() {
                    EventKind::RoundWin
                } else {
                    EventKind::RoundLoss
                });
            }
            if data["map"]["phase"] == "live" && old["map"]["phase"] != "live" {
                emit(EventKind::MatchStart);
            }
            if data["map"]["phase"] == "gameover" && old["map"]["phase"] != "gameover" {
                emit(EventKind::MatchEnd);
            }
        }
        self.previous = Some(data);
        events
    }
}
fn read_request(stream: &mut TcpStream) -> anyhow::Result<Value> {
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    let mut bytes = Vec::new();
    let mut block = [0; 4096];
    let (offset, length) = loop {
        let n = stream.read(&mut block)?;
        anyhow::ensure!(n > 0, "Incomplete event request");
        bytes.extend_from_slice(&block[..n]);
        anyhow::ensure!(bytes.len() <= 65536, "Event request too large");
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&bytes[..end])?;
            anyhow::ensure!(headers.starts_with("POST / "), "Invalid event request");
            let length = headers
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().ok())
                        .flatten()
                })
                .ok_or_else(|| anyhow::anyhow!("Missing event length"))?;
            anyhow::ensure!(length <= 60000, "Event body too large");
            break (end + 4, length);
        }
    };
    while bytes.len() < offset + length {
        let n = stream.read(&mut block)?;
        anyhow::ensure!(n > 0, "Incomplete event body");
        bytes.extend_from_slice(&block[..n]);
        anyhow::ensure!(bytes.len() <= 65536, "Event body too large");
    }
    Ok(serde_json::from_slice(&bytes[offset..offset + length])?)
}
pub(super) fn listen(
    settings: &HighlightSettings,
    clock: crate::audio::Clock,
    tx: crossbeam_channel::Sender<GameEvent>,
    stop: Arc<AtomicBool>,
    status: Arc<Mutex<String>>,
) -> anyhow::Result<std::thread::JoinHandle<()>> {
    anyhow::ensure!(
        !settings.cs2_token.is_empty(),
        "Connect CS2 before enabling highlights."
    );
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, settings.cs2_port))?;
    listener.set_nonblocking(true)?;
    let token = settings.cs2_token.clone();
    let worker = std::thread::Builder::new()
        .name("gavnel-cs2-events".into())
        .spawn(move || {
            let mut state = State::default();
            while !stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let request = read_request(&mut stream);
                        let authorized = request
                            .as_ref()
                            .is_ok_and(|v| v["auth"]["token"].as_str() == Some(&token));
                        let code = if authorized {
                            "200 OK"
                        } else {
                            "403 Forbidden"
                        };
                        let _ = write!(
                            stream,
                            "HTTP/1.1 {code}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                        if authorized {
                            *status.lock().unwrap() = "Receiving CS2 events".into();
                            for event in state.update(request.unwrap(), clock.now()) {
                                let _ = tx.try_send(event);
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(30))
                    }
                    Err(e) => {
                        *status.lock().unwrap() = e.to_string();
                        break;
                    }
                }
            }
        })?;
    Ok(worker)
}
pub fn connect(
    folder: &std::path::Path,
    settings: &mut HighlightSettings,
) -> anyhow::Result<std::path::PathBuf> {
    let cfg = if folder.join("game/csgo/cfg").is_dir() {
        folder.join("game/csgo/cfg")
    } else {
        folder.to_owned()
    };
    anyhow::ensure!(
        cfg.is_dir()
            && cfg
                .file_name()
                .is_some_and(|n| n.eq_ignore_ascii_case("cfg")),
        "Choose Counter-Strike 2's game/csgo/cfg folder."
    );
    if settings.cs2_token.is_empty() {
        settings.cs2_token = format!("{:?}", unsafe {
            windows::Win32::System::Com::CoCreateGuid()?
        });
    }
    let path = cfg.join("gamestate_integration_gavnel.cfg");
    let text = format!(
        "\"Gavnel\"\n{{\n\"uri\" \"http://127.0.0.1:{}/\"\n\"timeout\" \"1.0\"\n\"buffer\" \"0.1\"\n\"throttle\" \"0.1\"\n\"heartbeat\" \"10.0\"\n\"auth\" {{ \"token\" \"{}\" }}\n\"data\" {{ \"provider\" \"1\" \"map\" \"1\" \"round\" \"1\" \"player_id\" \"1\" \"player_state\" \"1\" \"player_match_stats\" \"1\" }}\n}}\n",
        settings.cs2_port, settings.cs2_token
    );
    if path.exists() {
        anyhow::ensure!(
            std::fs::read_to_string(&path)?.contains(&settings.cs2_token),
            "An existing Gavnel integration uses a different token. Remove that file before reconnecting."
        );
    }
    std::fs::write(&path, text)?;
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicates_and_spectators_do_not_fire() {
        let mut s = State::default();
        let mut v = serde_json::json!({"provider":{"steamid":"me"},"player":{"steamid":"me","activity":"playing","match_stats":{"kills":1,"assists":0},"state":{"health":100}}});
        assert!(s.update(v.clone(), 0).is_empty());
        v["player"]["match_stats"]["kills"] = 2.into();
        assert_eq!(s.update(v.clone(), 1)[0].kind, EventKind::Kill);
        assert!(s.update(v.clone(), 2).is_empty());
        v["player"]["steamid"] = "someone else".into();
        assert!(s.update(v, 3).is_empty());
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    fn post(port: u16, value: &Value) -> String {
        let mut stream = TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let body = serde_json::to_vec(value).unwrap();
        write!(
            stream,
            "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(&body).unwrap();
        // Content-Length is zero: the response ends with its headers, not EOF.
        // Windows may reset a closed connection after the complete response.
        let mut response = Vec::new();
        let mut block = [0; 512];
        while !response.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let count = stream.read(&mut block).unwrap();
            assert!(count > 0, "Incomplete HTTP response");
            response.extend_from_slice(&block[..count]);
        }
        String::from_utf8(response).unwrap()
    }
    #[test]
    fn authenticated_events_deduplicate_and_port_reopens() {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let mut settings = HighlightSettings {
            cs2_port: port,
            cs2_token: "fixture-token".into(),
            ..Default::default()
        };
        settings
            .games
            .iter_mut()
            .find(|g| g.game == Game::Cs2)
            .unwrap()
            .enabled = true;
        let runtime =
            super::super::Runtime::new(Game::Cs2, &settings, crate::audio::Clock::new().unwrap());
        let mut value = serde_json::json!({"auth":{"token":"wrong"},"provider":{"steamid":"me"},"player":{"steamid":"me","activity":"playing","team":"CT","state":{"health":100},"match_stats":{"kills":0,"assists":0}},"map":{"phase":"live"},"round":{"phase":"live"}});
        assert!(post(port, &value).contains("403 Forbidden"));
        value["auth"]["token"] = "fixture-token".into();
        assert!(post(port, &value).contains("200 OK"));
        assert!(runtime.events.try_recv().is_err());
        value["player"]["match_stats"]["kills"] = 1.into();
        post(port, &value);
        assert_eq!(
            runtime
                .events
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .kind,
            EventKind::Kill
        );
        post(port, &value);
        assert!(runtime.events.try_recv().is_err());
        value["player"]["state"]["health"] = 0.into();
        post(port, &value);
        assert_eq!(
            runtime
                .events
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .kind,
            EventKind::Death
        );
        drop(runtime);
        let rebound = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
        drop(rebound);
    }
}
