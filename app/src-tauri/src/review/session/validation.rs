//! Session payload validation at the IPC boundary.
use super::*;

pub(super) fn validate(config: &ReviewConfig) -> Result<()> {
    if !(1..=128).contains(&config.engine.depth)
        || !(1..=5).contains(&config.lines)
        || (config.mode == crate::db::mode::Mode::Time
            && config.movetime_ms.is_some_and(|v| v == 0 || v > 30000))
    {
        return Err(ReviewError::new(
            "invalidPayload",
            "session.open",
            "Parâmetros de análise inválidos.",
        ));
    }
    if let Some(result) = &config.initial_result {
        crate::review::repository::validate_review(result)?;
    }
    Ok(())
}
pub(super) fn validate_live(settings: &LiveSettings) -> Result<()> {
    if !(1..=30).contains(&settings.search_seconds)
        || !(1..=5).contains(&settings.lines)
        || !(1..=256).contains(&settings.threads)
        || !(16..=4096).contains(&settings.memory_mb)
    {
        Err(ReviewError::new(
            "invalidPayload",
            "session.live",
            "Parâmetros de análise ao vivo inválidos.",
        ))
    } else {
        Ok(())
    }
}
