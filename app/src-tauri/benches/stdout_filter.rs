use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

#[path = "../src/engine.rs"]
mod engine;

/// Implementação que estava no leitor antes da otimização. Ela é mantida só
/// como baseline do benchmark; a aplicação usa `engine::consume_stdout_lines`.
fn consume_stdout_lines_legacy(stdout: &mut Vec<u8>, mut on_line: impl FnMut(String)) {
    while let Some(end) = stdout.iter().position(|byte| *byte == b'\n') {
        let raw = stdout.drain(..=end).collect::<Vec<_>>();
        let line = String::from_utf8_lossy(&raw).trim().to_string();
        if !line.is_empty() {
            on_line(line);
        }
    }
}

fn verbose_stockfish_chunk(line_count: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(line_count * 80);
    for depth in 1..=line_count {
        output.extend_from_slice(
            format!(
                "info depth {depth} multipv 1 score cp 20 nodes 123456 nps 100000 pv e2e4 e7e5\n"
            )
            .as_bytes(),
        );
    }
    output
}

fn benchmark_stdout_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("stockfish_stdout_chunk");

    for line_count in [100, 1_000, 10_000] {
        let chunk = verbose_stockfish_chunk(line_count);
        group.bench_with_input(
            BenchmarkId::new("legacy_drain_per_line", line_count),
            &chunk,
            |b, chunk| {
                b.iter(|| {
                    let mut stdout = chunk.clone();
                    let mut forwarded = 0;
                    consume_stdout_lines_legacy(&mut stdout, |_| forwarded += 1);
                    black_box((stdout, forwarded));
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("batched_prefix_drain", line_count),
            &chunk,
            |b, chunk| {
                b.iter(|| {
                    let mut stdout = chunk.clone();
                    let mut forwarded = 0;
                    engine::consume_stdout_lines(&mut stdout, |_| forwarded += 1);
                    black_box((stdout, forwarded));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_stdout_filter);
criterion_main!(benches);
