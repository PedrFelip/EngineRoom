use super::*;

#[test]
fn mode_serializa_lowercase_no_wire() {
    assert_eq!(
        serde_json::to_string(&Mode::Depth).expect("serializa depth"),
        "\"depth\""
    );
    assert_eq!(
        serde_json::to_string(&Mode::Time).expect("serializa time"),
        "\"time\""
    );
}

#[test]
fn mode_desserializa_de_strings_lowercase() {
    assert_eq!(
        serde_json::from_str::<Mode>("\"depth\"").expect("desserializa depth"),
        Mode::Depth
    );
    assert_eq!(
        serde_json::from_str::<Mode>("\"time\"").expect("desserializa time"),
        Mode::Time
    );
}

#[test]
fn mode_rejeita_string_invalida_no_wire() {
    assert!(serde_json::from_str::<Mode>("\"timer\"").is_err());
    // Case-sensitive: uppercase é rejeitado (wire é lowercase).
    assert!(serde_json::from_str::<Mode>("\"DEPTH\"").is_err());
    assert!(serde_json::from_str::<Mode>("\"\"").is_err());
}

#[test]
fn mode_roundtrip_pelo_sqlite() {
    let conn = rusqlite::Connection::open_in_memory().expect("cria banco em memória");
    conn.execute_batch("CREATE TABLE m (mode TEXT NOT NULL);")
        .expect("cria tabela");
    conn.execute("INSERT INTO m (mode) VALUES (?1)", [Mode::Time])
        .expect("insere modo");

    let lido: Mode = conn
        .query_row("SELECT mode FROM m", [], |r| r.get(0))
        .expect("lê modo");
    assert_eq!(lido, Mode::Time);
}

#[test]
fn from_sql_rejeita_texto_invalido() {
    let conn = rusqlite::Connection::open_in_memory().expect("cria banco em memória");
    conn.execute_batch(
        "CREATE TABLE m (mode TEXT);
         INSERT INTO m (mode) VALUES ('timer');",
    )
    .expect("cria tabela com modo inválido");

    let res = conn.query_row("SELECT mode FROM m", [], |r| r.get::<_, Mode>(0));
    assert!(res.is_err(), "texto inválido deve falhar ao desserializar");
}
