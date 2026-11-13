//! Best-effort Chess.com profile pictures, fetched off the UI thread.
use rune::{Image, ImageCache, Waker};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

type AvatarId = (String, usize);
type AvatarResponse = (AvatarId, Option<Vec<u8>>);

pub struct Avatars {
    requests: mpsc::Sender<Request>,
    responses: mpsc::Receiver<AvatarResponse>,
    pending: HashSet<AvatarId>,
    images: HashMap<AvatarId, Image>,
    cache: ImageCache,
}

struct Request {
    key: String,
    side: usize,
    name: Option<String>,
}

pub fn game_key(pgn: &str) -> String {
    blake3::hash(pgn.as_bytes()).to_hex().to_string()
}

impl Avatars {
    pub fn new(wake: Waker, directory: Option<PathBuf>) -> Self {
        let (requests, rx) = mpsc::channel::<Request>();
        let (tx, responses) = mpsc::channel();
        let worker_wake = wake.clone();
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            let Ok(client) = reqwest::Client::builder()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(6))
                .connect_timeout(Duration::from_secs(3))
                .user_agent("EngineRoom experimental (+https://github.com/PedrFelip/EngineRoom)")
                .build()
            else {
                return;
            };
            while let Ok(request) = rx.recv() {
                let bytes = directory
                    .as_deref()
                    .and_then(|dir| read_cached(dir, &request.key, request.side))
                    .or_else(|| {
                        let name = request.name.as_deref()?;
                        let bytes = runtime.block_on(fetch(&client, name))?;
                        if let Some(dir) = &directory {
                            let _ = write_cached(dir, &request.key, request.side, &bytes);
                        }
                        Some(bytes)
                    });
                if tx.send(((request.key, request.side), bytes)).is_err() {
                    break;
                }
                worker_wake();
            }
        });
        Self {
            requests,
            responses,
            pending: HashSet::new(),
            images: HashMap::new(),
            cache: ImageCache::new(wake),
        }
    }
    pub fn fixture(&mut self, name: &str) {
        let pixels: Vec<u8> = (0..32 * 32)
            .flat_map(|i| {
                let x = i % 32;
                let y = i / 32;
                if (x - 16i32).pow(2) + (y - 11i32).pow(2) < 49 || (y > 20 && x > 6 && x < 26) {
                    [225, 182, 146, 255]
                } else {
                    [57, 91, 113, 255]
                }
            })
            .collect();
        let image = self.cache.rgba(32, 32, pixels);
        let _ = image.wait();
        self.images.insert((name.to_owned(), 0), image);
    }
    pub fn request(&mut self, key: &str, side: usize, name: Option<&str>) {
        let name = name.map(str::to_ascii_lowercase);
        if name.as_ref().is_some_and(|name| !valid_name(name)) {
            return;
        }
        let id = (key.to_owned(), side);
        if side < 2 && !self.images.contains_key(&id) && self.pending.insert(id.clone()) {
            let _ = self.requests.send(Request {
                key: id.0,
                side,
                name,
            });
        }
    }
    pub fn get(&self, key: &str, side: usize) -> Option<&Image> {
        self.images.get(&(key.to_owned(), side))
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((id, bytes)) = self.responses.try_recv() {
            self.pending.remove(&id);
            if let Some(bytes) = bytes {
                self.images.insert(id, self.cache.load(bytes));
            }
            changed = true;
        }
        changed
    }
}
const IMAGE_LIMIT: usize = 1024 * 1024;
fn avatar_path(directory: &Path, key: &str, side: usize) -> PathBuf {
    directory
        .join("games")
        .join(key)
        .join(if side == 0 { "white" } else { "black" })
}
fn read_cached(directory: &Path, key: &str, side: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut reference = String::new();
    std::fs::File::open(avatar_path(directory, key, side))
        .ok()?
        .take(65)
        .read_to_string(&mut reference)
        .ok()?;
    if reference.len() != 64 || !reference.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::open(directory.join("images").join(&reference))
        .ok()?
        .take((IMAGE_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    (!bytes.is_empty()
        && bytes.len() <= IMAGE_LIMIT
        && blake3::hash(&bytes).to_hex().as_str() == reference)
        .then_some(bytes)
}
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing avatar directory"))?;
    std::fs::create_dir_all(parent)?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(temporary, path)
}
fn write_cached(directory: &Path, key: &str, side: usize, bytes: &[u8]) -> std::io::Result<()> {
    let hash = blake3::hash(bytes).to_hex().to_string();
    let image = directory.join("images").join(&hash);
    let valid_existing = std::fs::read(&image).is_ok_and(|stored| stored == bytes);
    if !valid_existing {
        write_atomic(&image, bytes)?;
    }
    write_atomic(&avatar_path(directory, key, side), hash.as_bytes())
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 50
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn valid_image_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.fragment().is_none()
        && url
            .host_str()
            .is_some_and(|host| host == "images.chesscomfiles.com")
}
async fn bounded(mut response: reqwest::Response, limit: usize) -> Option<Vec<u8>> {
    if !response.status().is_success() {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    Some(bytes)
}
async fn fetch(client: &reqwest::Client, name: &str) -> Option<Vec<u8>> {
    let profile = client
        .get(format!("https://api.chess.com/pub/player/{name}"))
        .send()
        .await
        .ok()?;
    let profile: serde_json::Value =
        serde_json::from_slice(&bounded(profile, 128 * 1024).await?).ok()?;
    let url = reqwest::Url::parse(profile.get("avatar")?.as_str()?).ok()?;
    if !valid_image_url(&url) {
        return None;
    }
    let response = client.get(url).send().await.ok()?;
    bounded(response, IMAGE_LIMIT).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stores_identical_photos_once_and_restores_each_game_offline() {
        let root = std::env::temp_dir().join(format!(
            "engineroom-avatars-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = game_key("first game");
        let second = game_key("second game");
        let photo = b"same photo bytes";
        write_cached(&root, &first, 0, photo).unwrap();
        write_cached(&root, &second, 1, photo).unwrap();
        assert_eq!(std::fs::read_dir(root.join("images")).unwrap().count(), 1);
        assert_eq!(
            read_cached(&root, &first, 0).as_deref(),
            Some(photo.as_slice())
        );
        assert_eq!(
            read_cached(&root, &second, 1).as_deref(),
            Some(photo.as_slice())
        );
        assert!(read_cached(&root, &second, 0).is_none());
        let mut reopened = Avatars::new(std::sync::Arc::new(|| {}), Some(root.clone()));
        reopened.request(&second, 1, None);
        let (id, bytes) = reopened
            .responses
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert_eq!(id, (second.clone(), 1));
        assert_eq!(bytes.as_deref(), Some(photo.as_slice()));
        std::fs::write(avatar_path(&root, &second, 1), "../../untrusted").unwrap();
        assert!(read_cached(&root, &second, 1).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_untrusted_profile_and_image_locations() {
        assert!(valid_name("test-user_1"));
        assert!(!valid_name("../other"));
        assert!(valid_image_url(
            &reqwest::Url::parse("https://images.chesscomfiles.com/uploads/a.jpg").unwrap()
        ));
        for url in [
            "http://images.chesscomfiles.com/a",
            "https://images.chesscomfiles.com.evil.test/a",
            "https://user@images.chesscomfiles.com/a",
            "https://images.chesscomfiles.com:444/a",
        ] {
            assert!(
                !valid_image_url(&reqwest::Url::parse(url).unwrap()),
                "{url}"
            );
        }
    }
}
