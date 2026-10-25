//! Chess.com-only HTTP adapter.
use super::chess_com::*;
use std::{sync::Mutex, time::Duration};
use tokio::time::Instant;

pub struct ChessComHttp {
    client: reqwest::Client,
    cooldown: Mutex<Option<Instant>>,
}
impl ChessComHttp {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(concat!(
                "EngineRoom/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/PedrFelip/EngineRoom)"
            ))
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .build()
            .map_err(network)?;
        Ok(Self {
            client,
            cooldown: Mutex::new(None),
        })
    }
}
fn network(error: reqwest::Error) -> ImportError {
    if error.is_timeout() {
        ImportError::new(
            ErrorCode::Timeout,
            "O Chess.com demorou demais para responder.",
        )
    } else {
        ImportError::new(
            ErrorCode::Network,
            "Não foi possível conectar ao Chess.com. Verifique sua conexão.",
        )
    }
}
fn status_error(status: u16, retry_after: Option<&str>) -> Option<ImportError> {
    match status {
        200 => None,
        404 | 410 => Some(ImportError::new(
            ErrorCode::Unavailable,
            "Usuário ou arquivo indisponível no Chess.com. Confira o nome informado.",
        )),
        429 => Some(ImportError {
            code: ErrorCode::RateLimited,
            message: "O Chess.com limitou as consultas. Aguarde antes de tentar novamente.".into(),
            retry_after_seconds: Some(
                retry_after
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60)
                    .clamp(1, 86400),
            ),
        }),
        500..=599 => Some(ImportError::new(
            ErrorCode::Network,
            "O Chess.com está temporariamente indisponível.",
        )),
        _ => Some(ImportError::new(
            ErrorCode::InvalidResponse,
            "Resposta inesperada do Chess.com.",
        )),
    }
}
async fn read_body(mut response: reqwest::Response, max_bytes: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network)? {
        if chunk.len() > max_bytes.saturating_sub(bytes.len()) {
            return Err(ImportError::new(
                ErrorCode::ResponseTooLarge,
                "O arquivo do Chess.com excedeu o limite da busca.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
impl Http for ChessComHttp {
    fn get<'a>(&'a self, input: &'a str, max_bytes: usize) -> Task<'a, Result<Vec<u8>>> {
        Box::pin(async move {
            let url = reqwest::Url::parse(input).map_err(|_| {
                ImportError::new(ErrorCode::InvalidInput, "Endereço da API inválido.")
            })?;
            if url.scheme() != "https"
                || url.host_str() != Some("api.chess.com")
                || !url.username().is_empty()
                || url.password().is_some()
                || url.port().is_some()
                || url.query().is_some()
                || url.fragment().is_some()
                || !url.path().starts_with("/pub/player/")
            {
                return Err(ImportError::new(
                    ErrorCode::InvalidInput,
                    "Endereço da API inválido.",
                ));
            }
            let cooldown = *self.cooldown.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(until) = cooldown.filter(|until| *until > Instant::now()) {
                return Err(ImportError {
                    code: ErrorCode::RateLimited,
                    message: "Aguarde antes de consultar o Chess.com novamente.".into(),
                    retry_after_seconds: Some(
                        until.saturating_duration_since(Instant::now()).as_secs() + 1,
                    ),
                });
            }
            let response = self
                .client
                .get(url)
                .header(reqwest::header::ACCEPT, "application/json")
                .send()
                .await
                .map_err(network)?;
            if let Some(error) = status_error(
                response.status().as_u16(),
                response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|h| h.to_str().ok()),
            ) {
                if let Some(seconds) = error.retry_after_seconds {
                    *self.cooldown.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some(Instant::now() + Duration::from_secs(seconds));
                }
                return Err(error);
            }
            read_body(response, max_bytes).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn maps_statuses_and_rate_limit_waits() {
        assert_eq!(
            status_error(404, None).unwrap().code,
            ErrorCode::Unavailable
        );
        assert_eq!(
            status_error(410, None).unwrap().code,
            ErrorCode::Unavailable
        );
        assert_eq!(status_error(503, None).unwrap().code, ErrorCode::Network);
        assert_eq!(
            status_error(301, None).unwrap().code,
            ErrorCode::InvalidResponse
        );
        let error = status_error(429, Some("90")).unwrap();
        assert_eq!(error.retry_after_seconds, Some(90));
        assert_eq!(
            status_error(429, None).unwrap().retry_after_seconds,
            Some(60)
        );
        assert!(status_error(200, None).is_none());
    }

    #[tokio::test]
    async fn bounds_chunked_bodies_without_content_length() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            socket.read(&mut request).unwrap();
            socket.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\n1234\r\n4\r\n5678\r\n0\r\n\r\n").unwrap();
        });
        let response = reqwest::get(format!("http://{address}")).await.unwrap();
        assert_eq!(
            read_body(response, 6).await.unwrap_err().code,
            ErrorCode::ResponseTooLarge
        );
        server.join().unwrap();
    }

    #[tokio::test]
    async fn rejects_non_api_destinations_before_connecting() {
        let http = ChessComHttp::new().unwrap();
        for url in [
            "http://api.chess.com/pub/player/a/games/archives",
            "https://evil.test/",
            "https://api.chess.com@evil.test/",
            "https://api.chess.com:444/pub/player/a/games/archives",
        ] {
            assert_eq!(
                http.get(url, 10).await.unwrap_err().code,
                ErrorCode::InvalidInput
            );
        }
    }
}
