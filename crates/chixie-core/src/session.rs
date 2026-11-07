//! Account session worker: one thread owns the backend client (blocking HTTP), so the UI never
//! blocks. Same shape as `device.rs`: commands in over `mpsc`, events out over `async-channel`.
//!
//! The worker caches decrypted fonts in memory for the session, shared as `Arc<Font>` so layout
//! can use them on any thread. The GUI also keeps each downloaded font in the local
//! `font_store` (decrypted glyph map on this computer, BOSS's decision 2026-11-07), so login is
//! only needed to download.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::api::{self, FontInfo};
use crate::font::Font;
use crate::{Error, Result};

/// ChiXie3 sends a heartbeat periodically; the interval constant wasn't recoverable, so use a
/// conservative minute. VERIFY.
pub const HEARTBEAT_EVERY: Duration = Duration::from_secs(60);

/// What the worker needs from the backend; `api::Client` in production, a fake in tests.
pub trait Backend: Send {
    /// Returns (username, role_code).
    fn login(&mut self, account: &str, password: &str) -> Result<(String, String)>;
    fn logout(&mut self) -> Result<()>;
    fn heartbeat(&self) -> Result<()>;
    fn collected_fonts(&self, page: i64) -> Result<Vec<FontInfo>>;
    fn online_fonts(&self, page: i64, search: &str) -> Result<Vec<FontInfo>>;
    fn font(&self, id: i64) -> Result<Font>;
}

impl Backend for api::Client {
    fn login(&mut self, account: &str, password: &str) -> Result<(String, String)> {
        let l = api::Client::login(self, account, password)?;
        Ok((l.username.clone(), l.role_code.clone()))
    }
    fn logout(&mut self) -> Result<()> {
        api::Client::logout(self)
    }
    fn heartbeat(&self) -> Result<()> {
        api::Client::heartbeat(self)
    }
    fn collected_fonts(&self, page: i64) -> Result<Vec<FontInfo>> {
        api::Client::collected_fonts(self, page)
    }
    fn online_fonts(&self, page: i64, search: &str) -> Result<Vec<FontInfo>> {
        api::Client::online_fonts(self, page, search)
    }
    fn font(&self, id: i64) -> Result<Font> {
        api::Client::font(self, id)
    }
}

#[derive(Debug)]
pub enum Command {
    /// The password is used for this request only and never stored by the worker.
    Login { account: String, password: String },
    Logout,
    CollectedFonts,
    /// Online catalog page (1-based, 9 per page) with an optional name filter.
    OnlineFonts { page: i64, search: String },
    /// Fetch + decrypt a font (served from cache if already loaded this session).
    LoadFont { id: i64 },
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontListKind {
    Collected,
    Online,
}

#[derive(Clone)]
pub enum Event {
    LoggedIn { username: String, role: String },
    LoginFailed(String),
    LoggedOut,
    /// Heartbeat failed: the session is probably gone (expired or logged in elsewhere).
    SessionLost(String),
    Fonts { kind: FontListKind, page: i64, fonts: Vec<FontInfo> },
    FontLoaded { id: i64, font: Arc<Font> },
    Error(String),
}

impl std::fmt::Debug for Event {
    // Fonts are large; keep Debug output (logs, test failures) readable.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::FontLoaded { id, font } => write!(f, "FontLoaded {{ id: {id}, glyphs: {} }}", font.glyphs.len()),
            Event::Fonts { kind, page, fonts } => write!(f, "Fonts {{ {kind:?}, page {page}, {} fonts }}", fonts.len()),
            Event::LoggedIn { username, role } => write!(f, "LoggedIn {{ {username}, {role} }}"),
            Event::LoginFailed(m) => write!(f, "LoginFailed({m})"),
            Event::LoggedOut => write!(f, "LoggedOut"),
            Event::SessionLost(m) => write!(f, "SessionLost({m})"),
            Event::Error(m) => write!(f, "Error({m})"),
        }
    }
}

#[derive(Clone)]
pub struct SessionHandle {
    tx: Sender<Command>,
}

impl SessionHandle {
    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }
}

/// Spawn the worker. `heartbeat_every` is `HEARTBEAT_EVERY` in production.
pub fn spawn<B: Backend + 'static>(
    backend: B,
    heartbeat_every: Duration,
) -> (SessionHandle, async_channel::Receiver<Event>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let (ev_tx, ev_rx) = async_channel::unbounded();
    let w = Worker { backend, rx, ev: ev_tx, logged_in: false, cache: HashMap::new(), heartbeat_every };
    let join = std::thread::Builder::new()
        .name("chixie-session".into())
        .spawn(move || w.run())
        .expect("spawn session thread");
    (SessionHandle { tx }, ev_rx, join)
}

struct Worker<B: Backend> {
    backend: B,
    rx: Receiver<Command>,
    ev: async_channel::Sender<Event>,
    logged_in: bool,
    cache: HashMap<i64, Arc<Font>>,
    heartbeat_every: Duration,
}

impl<B: Backend> Worker<B> {
    fn emit(&self, e: Event) {
        let _ = self.ev.send_blocking(e);
    }

    fn run(mut self) {
        let mut next_beat = Instant::now() + self.heartbeat_every;
        loop {
            let wait = next_beat.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(wait) {
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(c) => self.handle(c),
                Err(RecvTimeoutError::Timeout) => {
                    if self.logged_in
                        && let Err(e) = self.backend.heartbeat()
                    {
                        self.logged_in = false;
                        self.cache.clear();
                        self.emit(Event::SessionLost(e.to_string()));
                    }
                    next_beat = Instant::now() + self.heartbeat_every;
                }
            }
        }
    }

    fn require_login(&self) -> Result<()> {
        if self.logged_in { Ok(()) } else { Err(Error::Api("not logged in".into())) }
    }

    fn handle(&mut self, c: Command) {
        let r: Result<()> = (|| {
            match c {
                Command::Login { account, password } => {
                    self.cache.clear();
                    match self.backend.login(&account, &password) {
                        Ok((username, role)) => {
                            self.logged_in = true;
                            self.emit(Event::LoggedIn { username, role });
                        }
                        Err(e) => {
                            self.logged_in = false;
                            self.emit(Event::LoginFailed(e.to_string()));
                        }
                    }
                }
                Command::Logout => {
                    let r = if self.logged_in { self.backend.logout() } else { Ok(()) };
                    self.logged_in = false;
                    self.cache.clear();
                    self.emit(Event::LoggedOut);
                    r?;
                }
                Command::CollectedFonts => {
                    self.require_login()?;
                    let fonts = self.backend.collected_fonts(0)?;
                    self.emit(Event::Fonts { kind: FontListKind::Collected, page: 0, fonts });
                }
                Command::OnlineFonts { page, search } => {
                    self.require_login()?;
                    let fonts = self.backend.online_fonts(page.max(1), &search)?;
                    self.emit(Event::Fonts { kind: FontListKind::Online, page: page.max(1), fonts });
                }
                Command::LoadFont { id } => {
                    self.require_login()?;
                    let font = match self.cache.get(&id) {
                        Some(f) => f.clone(),
                        None => {
                            let f = Arc::new(self.backend.font(id)?);
                            self.cache.insert(id, f.clone());
                            f
                        }
                    };
                    self.emit(Event::FontLoaded { id, font });
                }
                Command::Shutdown => unreachable!("handled in run"),
            }
            Ok(())
        })();
        if let Err(e) = r {
            self.emit(Event::Error(e.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;

    #[derive(Default)]
    struct Fake {
        font_fetches: Arc<AtomicUsize>,
        heartbeat_fails: Arc<AtomicBool>,
        seen_passwords: Arc<Mutex<Vec<String>>>,
    }

    impl Backend for Fake {
        fn login(&mut self, account: &str, password: &str) -> Result<(String, String)> {
            self.seen_passwords.lock().unwrap().push(password.to_string());
            if password == "right" {
                Ok((format!("user-{account}"), "vip_user".into()))
            } else {
                Err(Error::Api("user/login: HTTP 400: 密码错误".into()))
            }
        }
        fn logout(&mut self) -> Result<()> {
            Ok(())
        }
        fn heartbeat(&self) -> Result<()> {
            if self.heartbeat_fails.load(Ordering::SeqCst) { Err(Error::Api("expired".into())) } else { Ok(()) }
        }
        fn collected_fonts(&self, _page: i64) -> Result<Vec<FontInfo>> {
            Ok(vec![FontInfo { id: 1, font_name: "艾玛体".into(), character_number: 6899, default_load_size: None }])
        }
        fn online_fonts(&self, page: i64, search: &str) -> Result<Vec<FontInfo>> {
            Ok(vec![FontInfo { id: page * 100, font_name: format!("{search}{page}"), character_number: 1, default_load_size: None }])
        }
        fn font(&self, _id: i64) -> Result<Font> {
            self.font_fetches.fetch_add(1, Ordering::SeqCst);
            Ok(Font::from_json(include_bytes!("../tests/fixtures/font_fixture.json")).unwrap())
        }
    }

    fn next(rx: &async_channel::Receiver<Event>) -> Event {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(e) = rx.try_recv() {
                return e;
            }
            assert!(Instant::now() < deadline, "no event");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn login_failure_then_success_then_fonts() {
        let fake = Fake::default();
        let (h, rx, _) = spawn(fake, Duration::from_secs(3600));
        h.send(Command::CollectedFonts);
        assert!(matches!(next(&rx), Event::Error(m) if m.contains("not logged in")));
        h.send(Command::Login { account: "138".into(), password: "wrong".into() });
        assert!(matches!(next(&rx), Event::LoginFailed(m) if m.contains("密码错误")));
        h.send(Command::Login { account: "138".into(), password: "right".into() });
        assert!(matches!(next(&rx), Event::LoggedIn { username, .. } if username == "user-138"));
        h.send(Command::CollectedFonts);
        assert!(matches!(next(&rx), Event::Fonts { kind: FontListKind::Collected, fonts, .. } if fonts[0].id == 1));
        h.send(Command::OnlineFonts { page: 0, search: "宋".into() });
        // Page is clamped to 1 (the online catalog is 1-based; page 0 is HTTP 400 server-side).
        assert!(matches!(next(&rx), Event::Fonts { kind: FontListKind::Online, page: 1, fonts } if fonts[0].font_name == "宋1"));
        h.send(Command::Shutdown);
    }

    #[test]
    fn fonts_are_cached_per_session() {
        let fake = Fake::default();
        let fetches = fake.font_fetches.clone();
        let (h, rx, _) = spawn(fake, Duration::from_secs(3600));
        h.send(Command::Login { account: "a".into(), password: "right".into() });
        next(&rx);
        for _ in 0..3 {
            h.send(Command::LoadFont { id: 1 });
            assert!(matches!(next(&rx), Event::FontLoaded { id: 1, .. }));
        }
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
        // Logging out drops the cache (fonts are tied to the session's backup token).
        h.send(Command::Logout);
        assert!(matches!(next(&rx), Event::LoggedOut));
        h.send(Command::Login { account: "a".into(), password: "right".into() });
        next(&rx);
        h.send(Command::LoadFont { id: 1 });
        next(&rx);
        assert_eq!(fetches.load(Ordering::SeqCst), 2);
        h.send(Command::Shutdown);
    }

    #[test]
    fn failed_heartbeat_reports_session_lost() {
        let fake = Fake::default();
        let fails = fake.heartbeat_fails.clone();
        let (h, rx, _) = spawn(fake, Duration::from_millis(20));
        h.send(Command::Login { account: "a".into(), password: "right".into() });
        assert!(matches!(next(&rx), Event::LoggedIn { .. }));
        fails.store(true, Ordering::SeqCst);
        assert!(matches!(next(&rx), Event::SessionLost(m) if m.contains("expired")));
        h.send(Command::CollectedFonts);
        assert!(matches!(next(&rx), Event::Error(m) if m.contains("not logged in")));
        h.send(Command::Shutdown);
    }
}
