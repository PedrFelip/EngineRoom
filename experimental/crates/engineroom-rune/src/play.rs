//! Local games against Stockfish, independent of review and variation state.
use crate::game::Game;
use shakmaty::{Chess, Color, EnPassantMode, Move, Position, fen::Fen, san::SanPlus, uci::UciMove};

pub const MIN_ELO: u32 = 1320;
pub const MAX_ELO: u32 = 3190;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Side {
    #[default]
    White,
    Black,
    Random,
}

#[derive(Clone)]
pub struct PlaySession {
    pub request: Option<u64>,
    pub key: u64,
    pub game: Match,
    pub cursor: usize,
    pub flipped: bool,
    pub error: Option<String>,
    pub promotion: Option<(usize, usize)>,
}

pub struct Play {
    pub sessions: Vec<PlaySession>,
    pub active: Option<u64>,
    pub next_key: u64,
    pub cursor: usize,
    pub rating: u32,
    pub side: Side,
    pub game: Option<Match>,
    pub request: u64,
    pub thinking: bool,
    pub error: Option<String>,
}
impl Default for Play {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            active: None,
            next_key: 0,
            cursor: 0,
            rating: 1500,
            side: Side::White,
            game: None,
            request: 0,
            thinking: false,
            error: None,
        }
    }
}

#[derive(Clone)]
pub struct Match {
    pub game: Game,
    pub human: Color,
    pub rating: u32,
    pub moves: Vec<Move>,
    pub result: Option<&'static str>,
    pub reason: Option<&'static str>,
}
impl Match {
    pub fn new(human: Color, rating: u32) -> Self {
        let mut game = Game::empty();
        let engine = format!("Stockfish {rating}");
        (game.white, game.black) = if human.is_white() {
            ("Você".into(), engine)
        } else {
            (engine, "Você".into())
        };
        game.ratings[usize::from(human.is_white())] = Some(rating.to_string());
        game.event = "Partida contra Stockfish".into();
        Self {
            game,
            human,
            rating,
            moves: Vec::new(),
            result: None,
            reason: None,
        }
    }
    pub fn position(&self) -> &Chess {
        self.game
            .positions
            .last()
            .expect("a match always has its starting position")
    }
    pub fn human_turn(&self) -> bool {
        self.result.is_none() && self.position().turn() == self.human
    }
    pub fn push(&mut self, m: Move) -> Result<(), String> {
        if self.result.is_some() || !self.position().is_legal(m) {
            return Err("O motor retornou um lance inválido.".into());
        }
        let san = SanPlus::from_move(self.position().clone(), m).to_string();
        let mut next = self.position().clone();
        next.play_unchecked(m);
        self.game.moves.push(san);
        self.moves.push(m);
        self.game.positions.push(next);
        self.finish_if_terminal();
        Ok(())
    }
    pub fn engine_move(&mut self, text: &str) -> Result<(), String> {
        if self.human_turn() || self.result.is_some() {
            return Err("Resposta do motor fora de turno.".into());
        }
        let m = text
            .parse::<UciMove>()
            .map_err(|e| e.to_string())?
            .to_move(self.position())
            .map_err(|e| e.to_string())?;
        self.push(m)
    }
    fn finish_if_terminal(&mut self) {
        let p = self.position();
        let terminal = if p.is_checkmate() {
            Some((
                if p.turn().is_white() { "0-1" } else { "1-0" },
                "Xeque-mate",
            ))
        } else if p.is_stalemate() {
            Some(("1/2-1/2", "Afogamento"))
        } else if p.is_insufficient_material() {
            Some(("1/2-1/2", "Material insuficiente"))
        } else if p.halfmoves() >= 100 {
            Some(("1/2-1/2", "Regra dos 50 lances"))
        } else {
            let fen = repetition_key(p);
            let repeats = self
                .game
                .positions
                .iter()
                .filter(|p| repetition_key(p) == fen)
                .count();
            (repeats >= 3).then_some(("1/2-1/2", "Repetição tripla"))
        };
        if let Some((result, reason)) = terminal {
            self.result = Some(result);
            self.reason = Some(reason);
        }
    }
    pub fn resign(&mut self) {
        if self.result.is_none() {
            self.result = Some(if self.human.is_white() { "0-1" } else { "1-0" });
            self.reason = Some("Desistência");
        }
    }
    pub fn position_command(&self) -> String {
        let moves = self
            .moves
            .iter()
            .map(|m| UciMove::from_standard(*m).to_string())
            .collect::<Vec<_>>()
            .join(" ");
        if moves.is_empty() {
            "position startpos".into()
        } else {
            format!("position startpos moves {moves}")
        }
    }
    pub fn pgn(&self) -> String {
        let result = self.result.unwrap_or("*");
        let rating_tag = if self.human.is_white() {
            "BlackElo"
        } else {
            "WhiteElo"
        };
        let mut pgn = format!(
            "[Event \"Partida contra Stockfish\"]\n[White \"{}\"]\n[Black \"{}\"]\n[{rating_tag} \"{}\"]\n[Result \"{result}\"]\n\n",
            self.game.white, self.game.black, self.rating
        );
        for (i, san) in self.game.moves.iter().enumerate() {
            if i % 2 == 0 {
                pgn.push_str(&format!("{}. ", i / 2 + 1));
            }
            pgn.push_str(san);
            pgn.push(' ');
        }
        pgn.push_str(result);
        pgn
    }
}
fn repetition_key(p: &Chess) -> String {
    Fen::from_position(p, EnPassantMode::Legal)
        .to_string()
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Makes a bounded, cancellable move request and always awaits engine teardown.
pub async fn best_move(
    factory: &dyn engineroom_backend::review::engine::EngineFactory,
    cancel: &engineroom_backend::review::engine::Cancellation,
    rating: u32,
    position: &str,
) -> Result<String, String> {
    let mut port = factory.acquire(cancel).await.map_err(|e| e.to_string())?;
    let result = search(port.as_mut(), cancel, rating, position).await;
    port.shutdown().await;
    result
}

async fn search(
    port: &mut dyn engineroom_backend::review::engine::EnginePort,
    cancel: &engineroom_backend::review::engine::Cancellation,
    rating: u32,
    position: &str,
) -> Result<String, String> {
    use engineroom_backend::review::engine;
    let options = engine::ask(port, "uci", "uciok", 10_000, cancel)
        .await
        .map_err(|e| e.to_string())?;
    if !options
        .iter()
        .any(|line| line.starts_with("option name UCI_LimitStrength type check "))
    {
        return Err("Este motor não oferece limitação de força por Elo.".into());
    }
    let elo = options
        .iter()
        .find(|line| line.starts_with("option name UCI_Elo type spin "))
        .ok_or("Este motor não oferece UCI_Elo.")?;
    let words: Vec<_> = elo.split_whitespace().collect();
    let bound = |name| {
        words
            .windows(2)
            .find(|pair| pair[0] == name)
            .and_then(|pair| pair[1].parse::<u32>().ok())
    };
    let min = bound("min").ok_or("Limite Elo inválido.")?;
    let max = bound("max").ok_or("Limite Elo inválido.")?;
    if !(min..=max).contains(&rating) {
        return Err(format!("Este Stockfish aceita Elo entre {min} e {max}."));
    }
    for command in [
        "setoption name Threads value 1".to_string(),
        "setoption name Hash value 32".into(),
        "setoption name MultiPV value 1".into(),
        "setoption name UCI_LimitStrength value true".into(),
        format!("setoption name UCI_Elo value {rating}"),
        "ucinewgame".into(),
    ] {
        engine::send(port, &command, cancel)
            .await
            .map_err(|e| e.to_string())?;
    }
    engine::ask(port, "isready", "readyok", 10_000, cancel)
        .await
        .map_err(|e| e.to_string())?;
    engine::send(port, position, cancel)
        .await
        .map_err(|e| e.to_string())?;
    let read = async {
        engine::send(port, "go movetime 1000", cancel)
            .await
            .map_err(|e| e.to_string())?;
        loop {
            let line = port.next().await.map_err(|e| e.to_string())?;
            if let Some(best) = line.strip_prefix("bestmove ") {
                return best
                    .split_whitespace()
                    .next()
                    .map(str::to_owned)
                    .ok_or_else(|| "Stockfish não retornou um lance.".into());
            }
        }
    };
    tokio::select! {
        _ = cancel.cancelled() => Err("Partida interrompida.".into()),
        result = tokio::time::timeout(std::time::Duration::from_secs(15), read) => result.unwrap_or_else(|_| Err("Stockfish excedeu o tempo de resposta.".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engineroom_backend::review::{
        engine::{Cancellation, EngineFactory, EnginePort, Task},
        types::Result as EngineResult,
    };
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    fn play(game: &mut Match, moves: &[&str]) {
        for text in moves {
            let m = text
                .parse::<UciMove>()
                .expect("UCI move")
                .to_move(game.position())
                .expect("legal move");
            game.push(m).expect("match accepts move");
        }
    }
    #[test]
    fn mate_and_resignation_export_reviewable_pgn() {
        let mut game = Match::new(Color::White, 1500);
        play(&mut game, &["f2f3", "e7e5", "g2g4", "d8h4"]);
        assert_eq!(game.result, Some("0-1"));
        assert_eq!(game.reason, Some("Xeque-mate"));
        let parsed = crate::game::parse(&game.pgn()).expect("review accepts played PGN");
        assert_eq!(parsed.moves, game.game.moves);
        assert_eq!(parsed.ratings[1].as_deref(), Some("1500"));
        assert!(game.engine_move("e2e4").is_err());
        let mut game = Match::new(Color::Black, 1700);
        assert!(!game.human_turn());
        game.engine_move("e2e4").expect("engine plays white");
        assert!(game.human_turn());
        assert!(game.engine_move("e7e5").is_err());
        game.resign();
        assert_eq!(game.result, Some("1-0"));
        assert!(crate::game::parse(&game.pgn()).is_ok());
    }
    #[test]
    fn repetition_and_fifty_move_rule_end_the_game() {
        let mut game = Match::new(Color::White, 1500);
        play(
            &mut game,
            &[
                "g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1", "f6g8",
            ],
        );
        assert_eq!(game.reason, Some("Repetição tripla"));
        let mut game = Match::new(Color::White, 1500);
        game.game.positions = vec![
            "7k/8/8/8/8/8/8/R3K3 w Q - 99 1"
                .parse::<Fen>()
                .expect("fen")
                .into_position(shakmaty::CastlingMode::Standard)
                .expect("position"),
        ];
        play(&mut game, &["a1a2"]);
        assert_eq!(game.reason, Some("Regra dos 50 lances"));
    }
    #[test]
    fn castling_and_underpromotion_are_legal_and_preserved() {
        let mut game = Match::new(Color::White, 1500);
        play(
            &mut game,
            &["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "g8f6", "e1g1"],
        );
        assert_eq!(game.game.moves.last().map(String::as_str), Some("O-O"));
        assert!(game.position_command().ends_with("e1g1"));
        let mut game = Match::new(Color::White, 1500);
        game.game.positions = vec![
            "7k/P7/8/8/8/8/8/4K3 w - - 0 1"
                .parse::<Fen>()
                .expect("fen")
                .into_position(shakmaty::CastlingMode::Standard)
                .expect("position"),
        ];
        play(&mut game, &["a7a8n"]);
        assert_eq!(
            game.position()
                .board()
                .piece_at(shakmaty::Square::A8)
                .expect("promoted piece")
                .role,
            shakmaty::Role::Knight
        );
        assert_eq!(game.result, Some("1/2-1/2"));
    }
    struct Fake {
        commands: Arc<Mutex<Vec<String>>>,
        cancel_on_go: Option<tokio::sync::watch::Sender<bool>>,
    }
    struct Port {
        commands: Arc<Mutex<Vec<String>>>,
        lines: VecDeque<String>,
        cancel_on_go: Option<tokio::sync::watch::Sender<bool>>,
    }
    impl EngineFactory for Fake {
        fn acquire<'a>(
            &'a self,
            _: &'a Cancellation,
        ) -> Task<'a, EngineResult<Box<dyn EnginePort>>> {
            Box::pin(async move {
                Ok(Box::new(Port {
                    commands: self.commands.clone(),
                    lines: [
                        "option name UCI_LimitStrength type check default false",
                        "option name UCI_Elo type spin default 1320 min 1320 max 3190",
                        "uciok",
                        "readyok",
                        "bestmove e2e4 ponder e7e5",
                    ]
                    .map(str::to_owned)
                    .into(),
                    cancel_on_go: self.cancel_on_go.clone(),
                }) as Box<dyn EnginePort>)
            })
        }
    }
    impl EnginePort for Port {
        fn send<'a>(&'a mut self, command: &'a str) -> Task<'a, EngineResult<()>> {
            self.commands.lock().expect("commands").push(command.into());
            if command.starts_with("go ")
                && let Some(tx) = &self.cancel_on_go
            {
                tx.send_replace(true);
            }
            Box::pin(async { Ok(()) })
        }
        fn next(&mut self) -> Task<'_, EngineResult<String>> {
            Box::pin(async move {
                if self.cancel_on_go.is_some()
                    && self
                        .commands
                        .lock()
                        .expect("commands")
                        .last()
                        .is_some_and(|c| c.starts_with("go "))
                {
                    return std::future::pending().await;
                }
                Ok(self.lines.pop_front().expect("scripted engine line"))
            })
        }
        fn shutdown(&mut self) -> Task<'_, ()> {
            self.commands
                .lock()
                .expect("commands")
                .push("shutdown".into());
            Box::pin(async {})
        }
    }
    #[test]
    fn uci_rating_configuration_cancellation_and_invalid_rating_await_teardown() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        for (rating, cancel_on_go) in [(1500, false), (1500, true), (500, false)] {
            let (tx, closed) = tokio::sync::watch::channel(false);
            let commands = Arc::new(Mutex::new(Vec::new()));
            let fake = Fake {
                commands: commands.clone(),
                cancel_on_go: cancel_on_go.then_some(tx.clone()),
            };
            let result = rt.block_on(best_move(
                &fake,
                &Cancellation {
                    closed,
                    revision: None,
                },
                rating,
                "position startpos",
            ));
            assert_eq!(result.is_ok(), rating == 1500 && !cancel_on_go);
            let commands = commands.lock().expect("commands");
            assert_eq!(commands.last().map(String::as_str), Some("shutdown"));
            if rating == 1500 {
                assert!(commands.contains(&"setoption name UCI_LimitStrength value true".into()));
                assert!(commands.contains(&"setoption name UCI_Elo value 1500".into()));
            } else {
                assert!(!commands.iter().any(|c| c.starts_with("go ")));
            }
        }
    }
    #[test]
    #[ignore = "requires ENGINE_ROOM_STOCKFISH pointing to a real Stockfish executable"]
    fn real_stockfish_returns_a_legal_move_at_rating_limits() {
        use engineroom_backend::review::native_engine::NativeEngineFactory;
        let factory = NativeEngineFactory::new(
            std::env::var_os("ENGINE_ROOM_STOCKFISH")
                .expect("set ENGINE_ROOM_STOCKFISH")
                .into(),
        );
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let (_tx, closed) = tokio::sync::watch::channel(false);
        for rating in [MIN_ELO, MAX_ELO] {
            let mut game = Match::new(Color::Black, rating);
            let text = rt
                .block_on(best_move(
                    &factory,
                    &Cancellation {
                        closed: closed.clone(),
                        revision: None,
                    },
                    rating,
                    &game.position_command(),
                ))
                .expect("Stockfish move");
            game.engine_move(&text).expect("legal move");
            assert_eq!(game.moves.len(), 1);
        }
    }
}
