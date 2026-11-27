use super::*;
use crate::db::{cache::Cache, games::NewGame, games::Store, mode::Mode, open_memory};

const FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const LINES: &str = r#"[{"multipv":1,"cp":35,"pv":["e2e4","e7e5"],"san":"e4"}]"#;

fn partida_exemplo() -> NewGame {
    NewGame {
        pgn: "1. e4 e5".to_string(),
        white: "Brancas".to_string(),
        black: "Pretas".to_string(),
        result: "1-0".to_string(),
        plies: 2,
        engine_tier: "balanced".to_string(),
        mode: Mode::Depth,
        analysis_kind: "manual".to_string(),
        depth: 20,
        multipv: 1,
        accuracy_white: 98.5,
        accuracy_black: 91.0,
        review_json: r#"{"positions":[],"moves":[]}"#.to_string(),
    }
}

#[test]
fn storage_stats_reporta_bytes_das_tabelas() {
    let conn = open_memory().expect("cria banco em memória");
    let vazio = Stats::new(&conn).compute().expect("calcula stats vazias");
    assert_eq!(vazio.cache_bytes, 0, "banco vazio: cache em zero bytes");
    assert_eq!(vazio.games_bytes, 0, "banco vazio: games em zero bytes");

    Cache::new(&conn)
        .store(FEN, Mode::Depth, 20, 1, 20, 35, LINES)
        .expect("grava posição no cache");
    Store::new(&conn)
        .save(&partida_exemplo())
        .expect("grava partida");

    let populado = Stats::new(&conn)
        .compute()
        .expect("calcula stats populadas");
    assert!(
        populado.cache_bytes >= (FEN.len() + LINES.len()) as u64,
        "cache_bytes deve refletir ao menos fen + lines_json: got {}",
        populado.cache_bytes
    );
    assert!(
        populado.games_bytes
            >= "1. e4 e5".len() as u64 + r#"{"positions":[],"moves":[]}"#.len() as u64,
        "games_bytes deve refletir ao menos pgn + review_json: got {}",
        populado.games_bytes
    );
}

#[test]
fn storage_stats_zera_apos_clear_de_ambas_as_tabelas() {
    let conn = open_memory().expect("cria banco em memória");
    Cache::new(&conn)
        .store(FEN, Mode::Depth, 20, 1, 20, 35, LINES)
        .expect("grava posição no cache");
    Store::new(&conn)
        .save(&partida_exemplo())
        .expect("grava partida");

    // Pré-condição: ambas as tabelas com bytes > 0.
    let antes = Stats::new(&conn)
        .compute()
        .expect("calcula stats antes do clear");
    assert!(antes.cache_bytes > 0, "pré-condição: cache populado");
    assert!(antes.games_bytes > 0, "pré-condição: games populado");

    Cache::new(&conn).clear().expect("limpa cache");
    Store::new(&conn).clear().expect("limpa histórico");

    let depois = Stats::new(&conn)
        .compute()
        .expect("calcula stats após clear");
    assert_eq!(depois.cache_bytes, 0, "cache_bytes volta a zero após clear");
    assert_eq!(depois.games_bytes, 0, "games_bytes volta a zero após clear");
}
