//! Framing for complete UCI lines and partial stdout chunks.
/// Consome as linhas UCI completas acumuladas em `stdout`.
///
/// Um evento do shell pode conter várias linhas. Guardamos o sufixo sem `\n`
/// para o próximo evento e removemos todo o prefixo já consumido apenas uma
/// vez; remover cada linha individualmente deslocaria o restante do buffer a
/// cada `drain`.
pub(crate) fn consume_stdout_lines(stdout: &mut Vec<u8>, mut on_line: impl FnMut(String)) {
    let mut start = 0;

    for end in 0..stdout.len() {
        if stdout[end] != b'\n' {
            continue;
        }

        let line = String::from_utf8_lossy(&stdout[start..end])
            .trim()
            .to_string();
        if !line.is_empty() {
            on_line(line);
        }
        start = end + 1;
    }

    if start > 0 {
        stdout.drain(..start);
    }
}
