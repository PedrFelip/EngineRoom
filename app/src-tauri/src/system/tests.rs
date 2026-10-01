use super::*;

#[test]
fn detects_nonzero_resources() {
    let r = system_resources();
    assert!(r.threads >= 1);
    assert!(r.memory_mb >= 512, "memory_mb too low: {}", r.memory_mb);
    eprintln!("threads={} memory_mb={}", r.threads, r.memory_mb);
}
