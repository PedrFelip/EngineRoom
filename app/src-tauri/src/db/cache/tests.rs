use super::*;
use crate::db::open_memory;

const FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const LINES: &str = r#"[{"multipv":1,"cp":35,"pv":["e2e4","e7e5"],"san":"e4"}]"#;

#[test]
fn cache_put_depois_get_devolve_posicao_armazenada() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();

    let hit = cache.lookup(FEN, Mode::Depth, 20, 1).unwrap();

    assert_eq!(
        hit,
        Some(CachedPosition {
            cp: 35,
            lines_json: LINES.to_string(),
            reached_depth: 20,
        })
    );
}

#[test]
fn cache_armazena_reached_distinto_do_source_value_em_mode_time() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    // Modo tempo: source_value (movetimeMs) = 5000, reached_depth (plies) = 28.
    cache
        .store(FEN, Mode::Time, 5000, 1, 28, 35, LINES)
        .unwrap();

    let hit = cache.lookup(FEN, Mode::Time, 5000, 1).unwrap();

    assert_eq!(
        hit,
        Some(CachedPosition {
            cp: 35,
            lines_json: LINES.to_string(),
            reached_depth: 28,
        })
    );
}

#[test]
fn cache_get_em_posicao_desconhecida_devolve_none() {
    let conn = open_memory().unwrap();

    assert_eq!(
        Cache::new(&conn).lookup(FEN, Mode::Depth, 20, 1).unwrap(),
        None
    );
}

#[test]
fn cache_cobre_depth_menor_com_entry_mais_profunda() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    // Posição analisada até reached_depth=20.
    cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();

    // Pedido de depth 15 → deve acertar (20 >= 15).
    let hit = cache.lookup(FEN, Mode::Depth, 15, 1).unwrap();
    assert!(
        hit.is_some(),
        "depth menor deve ser coberto pela entry mais profunda"
    );
    assert_eq!(hit.unwrap().cp, 35);
}

#[test]
fn cache_nao_cobre_depth_maior_que_o_reached() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();

    // Pedido de depth 25 → miss (20 < 25).
    assert_eq!(cache.lookup(FEN, Mode::Depth, 25, 1).unwrap(), None);
}

#[test]
fn cache_depth_coberto_por_entry_de_time_mode_cross_mode() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    // Entry de time: reached_depth=28 (source_value/movetime não importa
    // para cobrir depth, só reached_depth conta).
    cache
        .store(FEN, Mode::Time, 5000, 1, 28, 35, LINES)
        .unwrap();

    // Pedido de depth 20 → hit: 28 >= 20, independente do source_mode.
    let hit = cache.lookup(FEN, Mode::Depth, 20, 1).unwrap();
    assert!(hit.is_some(), "entry de time cobre depth");
    assert_eq!(hit.unwrap().reached_depth, 28);
}

#[test]
fn cache_desempata_pela_entry_mais_rasa_em_depth() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    cache.store(FEN, Mode::Depth, 20, 1, 20, 20, LINES).unwrap();
    cache.store(FEN, Mode::Depth, 30, 1, 30, 99, "[]").unwrap();

    // Pedido de depth 15 → ambas cobrem; deve devolver a mais rasa (20).
    let hit = cache.lookup(FEN, Mode::Depth, 15, 1).unwrap().unwrap();
    assert_eq!(hit.reached_depth, 20);
    assert_eq!(hit.cp, 20);
}

#[test]
fn cache_cobre_multipv_menor_com_entry_de_mais_linhas() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    let lines4 = r#"[
        {"multipv":1,"cp":35,"pv":["e2e4"],"san":"e4"},
        {"multipv":2,"cp":30,"pv":["d2d4"],"san":"d4"},
        {"multipv":3,"cp":28,"pv":["c2c4"],"san":"c4"},
        {"multipv":4,"cp":25,"pv":["g1f3"],"san":"Nf3"}
    ]"#;
    cache
        .store(FEN, Mode::Depth, 20, 4, 20, 35, lines4)
        .unwrap();

    // Pedido multipv=2 → coberto pela entry multipv=4.
    let hit = cache.lookup(FEN, Mode::Depth, 20, 2).unwrap();
    assert!(hit.is_some(), "entry com mais multipv cobre pedido menor");
}

#[test]
fn cache_nao_cobre_multipv_maior_que_o_armazenado() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();

    // Pedido multipv=3 com entry multipv=1 → miss.
    assert_eq!(cache.lookup(FEN, Mode::Depth, 20, 3).unwrap(), None);
}

#[test]
fn cache_time_coberto_por_entry_com_movetime_maior() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    cache
        .store(FEN, Mode::Time, 5000, 1, 28, 35, LINES)
        .unwrap();

    // Pedido de time 3000ms → hit: 5000 >= 3000.
    let hit = cache.lookup(FEN, Mode::Time, 3000, 1).unwrap();
    assert!(hit.is_some(), "movetime maior cobre pedido menor");
    assert_eq!(hit.unwrap().reached_depth, 28);
}

#[test]
fn cache_time_nao_coberto_por_entry_de_depth_mode() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    // Entry de depth: reached_depth=28 mas sem orçamento temporal.
    cache.store(FEN, Mode::Depth, 28, 1, 28, 35, LINES).unwrap();

    // Pedido de time → miss: depth entries não têm source_mode='time'.
    assert_eq!(cache.lookup(FEN, Mode::Time, 1000, 1).unwrap(), None);
}

#[test]
fn cache_coalescer_entries_de_modos_diferentes_no_mesmo_reached() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    // Depth entry com reached=28.
    cache.store(FEN, Mode::Depth, 28, 1, 28, 35, LINES).unwrap();
    // Time entry no mesmo reached=28 → coalesce (PK é fen+reached+multipv),
    // último write vence.
    cache.store(FEN, Mode::Time, 5000, 1, 28, 42, "[]").unwrap();

    // Consulta de depth 20 acha a entry coalescida.
    let hit = cache.lookup(FEN, Mode::Depth, 20, 1).unwrap().unwrap();
    assert_eq!(hit.cp, 42, "último write vence no coalesce");

    // Exatamente 1 row para esta FEN (não duas, uma por modo).
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM position_cache WHERE fen = ?1",
            [FEN],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "entries dos dois modos coalesceram num único row");
}

#[test]
fn cache_put_repetido_sobrescreve_entrada() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();
    cache.store(FEN, Mode::Depth, 20, 1, 20, 42, "[]").unwrap();

    let hit = cache.lookup(FEN, Mode::Depth, 20, 1).unwrap().unwrap();

    assert_eq!(hit.cp, 42);
    assert_eq!(hit.lines_json, "[]");
}

#[test]
fn clear_cache_esvazia_tabela_de_posicoes() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();
    cache
        .store(FEN, Mode::Time, 5000, 1, 28, 35, LINES)
        .unwrap();
    assert!(
        cache.lookup(FEN, Mode::Depth, 20, 1).unwrap().is_some(),
        "pré-condição: cache populado"
    );

    cache.clear().unwrap();

    assert_eq!(
        cache.lookup(FEN, Mode::Depth, 20, 1).unwrap(),
        None,
        "entrada depth removida"
    );
    assert_eq!(
        cache.lookup(FEN, Mode::Time, 5000, 1).unwrap(),
        None,
        "entrada time removida",
    );
}

const FEN_B: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";

#[test]
fn lookup_bulk_devolve_um_resultado_por_fen_na_ordem_preservando_cobertura() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    // fen_a armazenado em reached 20; fen_b desconhecido.
    cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();

    let out = cache
        .lookup_bulk(&[FEN.to_string(), FEN_B.to_string()], Mode::Depth, 15, 1)
        .unwrap();

    assert_eq!(out.len(), 2, "um resultado por fen, na ordem de entrada");
    // fen_a cobre depth 15 (reached 20 >= 15), independente de source_mode.
    let hit_a = out[0].as_ref().expect("fen_a deve ser coberto");
    assert_eq!(hit_a.cp, 35);
    assert_eq!(hit_a.reached_depth, 20);
    // fen_b desconhecido → miss.
    assert!(out[1].is_none(), "fen_b desconhecido deve ser miss");
}

#[test]
fn store_many_grava_todas_as_entries_numa_única_transação() {
    let conn = open_memory().unwrap();
    let cache = Cache::new(&conn);
    let entries = vec![
        CachedPositionPut {
            fen: FEN.to_string(),
            reached_depth: 20,
            cp: 35,
            lines_json: LINES.to_string(),
        },
        CachedPositionPut {
            fen: FEN_B.to_string(),
            reached_depth: 18,
            cp: 12,
            lines_json: "[]".to_string(),
        },
    ];

    cache.store_many(&entries, Mode::Depth, 20, 1).unwrap();

    // Ambas ficam buscáveis, com a cobertura esperada.
    assert!(cache.lookup(FEN, Mode::Depth, 20, 1).unwrap().is_some());
    let hit_b = cache.lookup(FEN_B, Mode::Depth, 15, 1).unwrap();
    assert!(hit_b.is_some(), "fen_b cobre depth 15 (reached 18)");
    assert_eq!(hit_b.unwrap().cp, 12);
}
