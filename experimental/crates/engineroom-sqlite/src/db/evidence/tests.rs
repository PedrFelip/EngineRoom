use super::*;
use crate::db::{cache::Cache, open_memory};

const KEY_A: &str = "fen-a|history:6|engine:stockfish-19|scope:mainline";
const KEY_B: &str = "fen-a|history:6|engine:stockfish-19|scope:fen-only";
const LINES: &str = r#"[{"multipv":1,"cp":35,"pv":["e2e4","e7e5"],"san":"e4"}]"#;
const PAYLOAD: &str = r#"{"historyPly":6,"scope":"mainline","origin":"engine","engine":"Stockfish 19","requested":{"mode":"time","value":1000,"multipv":1},"reachedDepth":20,"observedLines":1}"#;

fn put(key: &str) -> EvidencePut {
    EvidencePut {
        context_key: key.to_string(),
        reached_depth: 20,
        cp: 35,
        lines_json: LINES.to_string(),
        payload_json: PAYLOAD.to_string(),
    }
}

#[test]
fn store_depois_lookup_devolve_evidencia_com_payload_opaco() {
    let conn = open_memory().unwrap();
    let cache = EvidenceCache::new(&conn);
    cache
        .store(KEY_A, Mode::Depth, 20, 1, 20, 35, LINES, PAYLOAD)
        .unwrap();

    let hit = cache.lookup(KEY_A, Mode::Depth, 20, 1).unwrap();

    assert_eq!(
        hit,
        Some(CachedEvidence {
            cp: 35,
            lines_json: LINES.to_string(),
            reached_depth: 20,
            payload_json: PAYLOAD.to_string(),
        })
    );
    // O payload é JSON verificável pelo chamador, não pelo crate.
    let parsed: serde_json::Value = serde_json::from_str(&hit.unwrap().payload_json).unwrap();
    assert_eq!(parsed["scope"], "mainline");
    assert_eq!(parsed["observedLines"], 1);
}

#[test]
fn lookup_em_chave_desconhecida_devolve_none() {
    let conn = open_memory().unwrap();
    assert_eq!(
        EvidenceCache::new(&conn)
            .lookup(KEY_A, Mode::Depth, 20, 1)
            .unwrap(),
        None
    );
}

#[test]
fn contexto_opaco_separa_evidencias_da_mesma_posicao() {
    let conn = open_memory().unwrap();
    let cache = EvidenceCache::new(&conn);
    cache
        .store(KEY_A, Mode::Depth, 20, 1, 20, 35, LINES, PAYLOAD)
        .unwrap();
    cache
        .store(
            KEY_B,
            Mode::Depth,
            20,
            1,
            20,
            42,
            "[]",
            r#"{"scope":"fen-only"}"#,
        )
        .unwrap();

    let a = cache.lookup(KEY_A, Mode::Depth, 20, 1).unwrap().unwrap();
    let b = cache.lookup(KEY_B, Mode::Depth, 20, 1).unwrap().unwrap();
    assert_eq!(a.cp, 35);
    assert_eq!(b.cp, 42);
    assert_eq!(b.payload_json, r#"{"scope":"fen-only"}"#);
}

#[test]
fn depth_e_coberto_por_entry_mais_profunda_e_por_entry_de_tempo() {
    let conn = open_memory().unwrap();
    let cache = EvidenceCache::new(&conn);
    cache
        .store(KEY_A, Mode::Depth, 20, 1, 20, 35, LINES, PAYLOAD)
        .unwrap();
    cache
        .store(KEY_A, Mode::Time, 5000, 1, 28, 42, "[]", "{}")
        .unwrap();

    // Pedido de depth 15: ambas cobrem; desempata pela mais rasa (20).
    let hit = cache.lookup(KEY_A, Mode::Depth, 15, 1).unwrap().unwrap();
    assert_eq!(hit.reached_depth, 20);
    assert_eq!(hit.cp, 35);

    // Pedido de depth 25: só a entry de tempo, com reached 28.
    let hit = cache.lookup(KEY_A, Mode::Depth, 25, 1).unwrap().unwrap();
    assert_eq!(hit.reached_depth, 28);
    assert_eq!(hit.cp, 42);
}

#[test]
fn depth_maior_que_o_reached_e_multipv_maior_que_o_armazenado_sao_miss() {
    let conn = open_memory().unwrap();
    let cache = EvidenceCache::new(&conn);
    cache
        .store(KEY_A, Mode::Depth, 20, 1, 20, 35, LINES, PAYLOAD)
        .unwrap();

    assert_eq!(cache.lookup(KEY_A, Mode::Depth, 21, 1).unwrap(), None);
    assert_eq!(cache.lookup(KEY_A, Mode::Depth, 20, 2).unwrap(), None);
}

#[test]
fn multipv_maior_na_entry_cobre_pedido_menor() {
    let conn = open_memory().unwrap();
    let cache = EvidenceCache::new(&conn);
    cache
        .store(KEY_A, Mode::Depth, 20, 4, 20, 35, LINES, PAYLOAD)
        .unwrap();

    assert!(
        cache.lookup(KEY_A, Mode::Depth, 20, 2).unwrap().is_some(),
        "entry com mais linhas cobre pedido menor"
    );
}

#[test]
fn time_so_e_coberto_por_entry_de_tempo_com_orcamento_suficiente() {
    let conn = open_memory().unwrap();
    let cache = EvidenceCache::new(&conn);
    cache
        .store(KEY_A, Mode::Time, 5000, 1, 28, 35, LINES, PAYLOAD)
        .unwrap();

    assert_eq!(
        cache.lookup(KEY_A, Mode::Time, 3000, 1).unwrap(),
        Some(CachedEvidence {
            cp: 35,
            lines_json: LINES.to_string(),
            reached_depth: 28,
            payload_json: PAYLOAD.to_string(),
        })
    );
    assert_eq!(cache.lookup(KEY_A, Mode::Time, 6000, 1).unwrap(), None);

    // Uma entry de depth não pode cobrir pedido de tempo.
    cache
        .store(KEY_B, Mode::Depth, 28, 1, 28, 35, LINES, PAYLOAD)
        .unwrap();
    assert_eq!(cache.lookup(KEY_B, Mode::Time, 1000, 1).unwrap(), None);
}

#[test]
fn upsert_preserva_entry_de_tempo_mais_forte() {
    let conn = open_memory().unwrap();
    let cache = EvidenceCache::new(&conn);
    cache
        .store(KEY_A, Mode::Time, 2000, 1, 20, 35, LINES, PAYLOAD)
        .unwrap();

    cache
        .store(KEY_A, Mode::Time, 500, 1, 20, -10, "[]", "{}")
        .unwrap();
    cache
        .store(KEY_A, Mode::Depth, 20, 1, 20, -20, "[]", "{}")
        .unwrap();
    cache.store_many(&[put(KEY_A)], Mode::Time, 500, 1).unwrap();

    let hit = cache.lookup(KEY_A, Mode::Time, 2000, 1).unwrap();
    assert_eq!(
        hit,
        Some(CachedEvidence {
            cp: 35,
            lines_json: LINES.to_string(),
            reached_depth: 20,
            payload_json: PAYLOAD.to_string(),
        }),
        "escritas fracas não devem rebaixar a cobertura temporal existente"
    );
}

#[test]
fn time_maior_ou_sobre_depth_atualiza_entrada_na_mesma_pk() {
    let conn = open_memory().unwrap();
    let cache = EvidenceCache::new(&conn);
    cache
        .store(KEY_A, Mode::Depth, 20, 1, 20, 15, "[]", PAYLOAD)
        .unwrap();

    cache
        .store(KEY_A, Mode::Time, 2000, 1, 20, 35, LINES, "{}")
        .unwrap();

    let hit = cache.lookup(KEY_A, Mode::Time, 2000, 1).unwrap().unwrap();
    assert_eq!(hit.cp, 35);
    assert_eq!(hit.lines_json, LINES);
    assert_eq!(hit.payload_json, "{}");
}

#[test]
fn lookup_bulk_devolve_um_resultado_por_chave_na_ordem() {
    let conn = open_memory().unwrap();
    let cache = EvidenceCache::new(&conn);
    cache
        .store(KEY_A, Mode::Depth, 20, 1, 20, 35, LINES, PAYLOAD)
        .unwrap();

    let out = cache
        .lookup_bulk(&[KEY_A.to_string(), KEY_B.to_string()], Mode::Depth, 15, 1)
        .unwrap();

    assert_eq!(out.len(), 2);
    assert_eq!(out[0].as_ref().unwrap().cp, 35);
    assert!(out[1].is_none());
}

#[test]
fn store_many_grava_todas_as_entries_numa_unica_transacao() {
    let conn = open_memory().unwrap();
    let cache = EvidenceCache::new(&conn);
    cache
        .store_many(&[put(KEY_A), put(KEY_B)], Mode::Depth, 20, 1)
        .unwrap();

    assert!(cache.lookup(KEY_A, Mode::Depth, 20, 1).unwrap().is_some());
    assert!(cache.lookup(KEY_B, Mode::Depth, 20, 1).unwrap().is_some());
}

#[test]
fn store_many_covered_aceita_cobertura_distinta_por_entry() {
    let conn = open_memory().unwrap();
    let cache = EvidenceCache::new(&conn);
    let mut weak = put(KEY_A);
    weak.cp = 12;
    cache
        .store_many_covered(&[(put(KEY_A), 3), (weak, 1)], Mode::Time, 1000)
        .unwrap();

    // A entry de 1 linha cobre pedido de 1 linha; só a de 3 cobre pedido de 2.
    assert!(
        cache.lookup(KEY_A, Mode::Time, 1000, 1).unwrap().is_some(),
        "qualquer uma das entries cobre multipv=1"
    );
    assert_eq!(
        cache
            .lookup(KEY_A, Mode::Time, 1000, 2)
            .unwrap()
            .unwrap()
            .cp,
        35
    );
    assert_eq!(cache.lookup(KEY_A, Mode::Time, 1000, 4).unwrap(), None);
}

#[test]
fn falha_no_lote_reverte_todas_as_entries() {
    let conn = open_memory().unwrap();
    conn.execute_batch(
        "CREATE TRIGGER reject_evidence_write BEFORE INSERT ON evidence_cache
         WHEN NEW.context_key = 'fail' BEGIN SELECT RAISE(ABORT, 'write failed'); END;",
    )
    .unwrap();
    let cache = EvidenceCache::new(&conn);
    let mut bad = put("fail");
    bad.payload_json = "{}".to_string();

    let error = cache
        .store_many(&[put(KEY_A), bad], Mode::Depth, 20, 1)
        .unwrap_err();
    assert_eq!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::ConstraintViolation)
    );
    assert_eq!(cache.lookup(KEY_A, Mode::Depth, 20, 1).unwrap(), None);
}

#[test]
fn clear_remove_apenas_evidencia_contextual() {
    let conn = open_memory().unwrap();
    let evidence = EvidenceCache::new(&conn);
    let legacy = Cache::new(&conn);
    evidence
        .store(KEY_A, Mode::Depth, 20, 1, 20, 35, LINES, PAYLOAD)
        .unwrap();
    legacy
        .store(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            Mode::Depth,
            20,
            1,
            20,
            35,
            LINES,
        )
        .unwrap();

    evidence.clear().unwrap();

    assert_eq!(evidence.lookup(KEY_A, Mode::Depth, 20, 1).unwrap(), None);
    assert!(
        legacy
            .lookup(
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                Mode::Depth,
                20,
                1
            )
            .unwrap()
            .is_some(),
        "limpar evidência não pode apagar o cache legado"
    );

    // E o inverso: limpar o cache legado não apaga a evidência.
    evidence
        .store(KEY_A, Mode::Depth, 20, 1, 20, 35, LINES, PAYLOAD)
        .unwrap();
    legacy.clear().unwrap();
    assert!(
        evidence
            .lookup(KEY_A, Mode::Depth, 20, 1)
            .unwrap()
            .is_some(),
        "limpar cache legado não pode apagar a evidência contextual"
    );
}
