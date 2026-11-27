//! Replayable position context: initial FEN, UCI path and known PGN history.
//!
//! A bare FEN loses the moves that led to a position, so an engine cannot
//! account for repetition when it evaluates it. [`PositionContext`] keeps the
//! starting FEN and the full UCI path, together with the prefix that is known
//! from the PGN mainline, so callers can send the history to the engine.
use super::{Game, fen, invalid, position as parse_position};
use crate::review::types::Result;
use shakmaty::{Chess, Position, uci::UciMove};

/// A position identified by the FEN it started from and the moves played since.
///
/// The first `known_history` entries of `uci_path` are the PGN mainline prefix;
/// any remaining entries are moves played in a variation. Every entry is a
/// legal UCI move from the position reached by the preceding ones, so the
/// context always describes a reachable position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionContext {
    /// FEN of the game's starting position (PGN `FEN` tag or the standard setup).
    pub initial_fen: String,
    /// Full UCI path from `initial_fen` to the position being evaluated.
    pub uci_path: Vec<String>,
    /// Number of leading `uci_path` moves known from the PGN mainline.
    pub known_history: usize,
}

impl PositionContext {
    /// Builds and validates a context from its three parts.
    ///
    /// Rejects an invalid initial FEN, an illegal UCI move in the path, and a
    /// known history longer than the path itself.
    pub fn new(
        initial_fen: impl Into<String>,
        uci_path: impl Into<Vec<String>>,
        known_history: usize,
    ) -> Result<Self> {
        let initial_fen = initial_fen.into();
        let uci_path = uci_path.into();
        if known_history > uci_path.len() {
            return Err(invalid("História conhecida excede o caminho UCI."));
        }
        Self::replay(&initial_fen, &uci_path)?;
        Ok(Self {
            initial_fen,
            uci_path,
            known_history,
        })
    }

    /// FEN-only context: no history is supplied to the engine.
    pub fn from_fen(fen: &str) -> Result<Self> {
        Self::new(fen, Vec::new(), 0)
    }

    /// Context at `ply` of a parsed game, using its mainline as known history.
    ///
    /// `ply` counts played moves: ply 0 is the initial position and
    /// `ply == game.moves.len()` is the position after the last move. The
    /// resulting context replays exactly the same mainline the PGN recorded.
    pub fn from_game(game: &Game, ply: usize) -> Result<Self> {
        if ply > game.moves.len() {
            return Err(invalid("Ply além do fim da partida."));
        }
        let uci_path = game.moves[..ply]
            .iter()
            .map(|m| m.uci.clone())
            .collect::<Vec<_>>();
        Self::new(game.fens[0].clone(), uci_path, ply)
    }

    /// Appends UCI moves to the path, keeping the known PGN history unchanged.
    pub fn extend(&self, extra: &[String]) -> Result<Self> {
        let mut uci_path = self.uci_path.clone();
        uci_path.extend_from_slice(extra);
        Self::new(self.initial_fen.clone(), uci_path, self.known_history)
    }

    /// Parses the initial FEN and replays every UCI move, returning the target position.
    pub fn position(&self) -> Result<Chess> {
        Self::replay(&self.initial_fen, &self.uci_path)
    }

    /// FEN of the position reached by replaying the whole path.
    pub fn target_fen(&self) -> Result<String> {
        Ok(fen(&self.position()?))
    }

    /// Moves from the PGN mainline prefix.
    pub fn known_history_moves(&self) -> &[String] {
        &self.uci_path[..self.known_history]
    }

    /// Moves beyond the known PGN history, played in a variation.
    pub fn variation_moves(&self) -> &[String] {
        &self.uci_path[self.known_history..]
    }

    /// History ply for evidence, or `None` when no PGN prefix was supplied.
    pub fn history_ply(&self) -> Option<usize> {
        (self.known_history > 0).then_some(self.known_history)
    }

    fn replay(initial_fen: &str, path: &[String]) -> Result<Chess> {
        let mut pos = parse_position(initial_fen)?;
        for uci in path {
            let parsed = uci
                .parse::<UciMove>()
                .map_err(|e| invalid(format!("UCI inválido '{uci}': {e}")))?;
            let move_ = parsed
                .to_move(&pos)
                .map_err(|e| invalid(format!("Lance ilegal '{uci}': {e}")))?;
            pos = pos
                .play(move_)
                .map_err(|e| invalid(format!("Lance ilegal '{uci}': {e}")))?;
        }
        Ok(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::core::extract;

    const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn game_context_replays_mainline_and_reports_known_history() {
        let game = extract("1. e4 e5 2. Nf3 Nc6 3. Bb5").expect("valid game");
        let context = PositionContext::from_game(&game, 3).expect("valid context");

        assert_eq!(context.initial_fen, START);
        assert_eq!(context.uci_path, ["e2e4", "e7e5", "g1f3"]);
        assert_eq!(context.known_history, 3);
        assert_eq!(
            context.target_fen().expect("reachable position"),
            game.fens[3]
        );
        assert_eq!(context.history_ply(), Some(3));
    }

    #[test]
    fn game_context_keeps_setup_fen_and_handles_castling_and_promotion() {
        let game =
            extract("[FEN \"7k/8/8/8/8/8/8/R6K w - - 0 1\"]\n1. Ra2").expect("valid setup game");
        let context = PositionContext::from_game(&game, 1).expect("valid context");
        assert_eq!(context.initial_fen, game.fens[0]);
        assert_eq!(context.uci_path, ["a1a2"]);
        assert_eq!(context.target_fen().unwrap(), game.fens[1]);

        let castled = extract("1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 4. O-O").expect("valid game");
        let context = PositionContext::from_game(&castled, 7).expect("valid context");
        assert_eq!(context.uci_path.last().map(String::as_str), Some("e1g1"));
        assert_eq!(context.target_fen().unwrap(), castled.fens[7]);

        let promoted =
            extract("[FEN \"7k/P7/8/8/8/8/4K3/8 w - - 0 1\"]\n1. a8=Q").expect("valid game");
        let context = PositionContext::from_game(&promoted, 1).expect("valid context");
        assert_eq!(context.uci_path, ["a7a8q"]);
        assert_eq!(context.target_fen().unwrap(), promoted.fens[1]);
    }

    #[test]
    fn en_passant_path_replays_to_the_pgn_fen() {
        let game = extract("1. e4 a6 2. e5 d5 3. exd6").expect("valid game");
        let context = PositionContext::from_game(&game, 5).expect("valid context");
        assert_eq!(context.uci_path[4], "e5d6");
        assert_eq!(context.target_fen().unwrap(), game.fens[5]);
    }

    #[test]
    fn fen_only_context_has_no_history() {
        let context = PositionContext::from_fen(START).expect("valid context");
        assert!(context.uci_path.is_empty());
        assert_eq!(context.known_history, 0);
        assert!(context.history_ply().is_none());
        assert!(context.known_history_moves().is_empty());
        assert!(context.variation_moves().is_empty());
        assert_eq!(context.target_fen().unwrap(), START);
    }

    #[test]
    fn extension_adds_variation_moves_without_growing_known_history() {
        let game = extract("1. e4 e5 2. Nf3 Nc6 3. Bb5").expect("valid game");
        let mainline = PositionContext::from_game(&game, 2).expect("valid context");
        let variation = mainline
            .extend(&["g1f3".to_owned()])
            .expect("legal variation move");

        assert_eq!(variation.uci_path, ["e2e4", "e7e5", "g1f3"]);
        assert_eq!(variation.known_history, 2);
        assert_eq!(variation.known_history_moves(), ["e2e4", "e7e5"]);
        assert_eq!(variation.variation_moves(), ["g1f3"]);
        assert_eq!(variation.history_ply(), Some(2));
        assert_eq!(variation.target_fen().unwrap(), game.fens[3]);
        assert_eq!(mainline.uci_path, ["e2e4", "e7e5"]);
    }

    #[test]
    fn rejects_invalid_fen_illegal_moves_and_oversized_history() {
        assert!(PositionContext::from_fen("not a fen").is_err());
        assert!(PositionContext::new(START, ["e2e5".to_owned()], 0).is_err());
        assert!(PositionContext::new(START, ["e2e4".to_owned()], 2).is_err());
        assert!(
            PositionContext::from_game(&extract("1. e4").unwrap(), 2).is_err(),
            "ply past the end of the game"
        );
    }
}
