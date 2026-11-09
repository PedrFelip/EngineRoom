# Experimental crate architecture

Current normal dependency graph:

```text
engineroom-rune -> engineroom-backend -> engineroom-application -> engineroom-domain
                        |                    ^                         ^
                        +-> engineroom-sqlite+-------------------------+
                        +-> engineroom-uci --+
                        +-> HTTP imports
```

`engineroom-domain` owns analysis types, mode, chess transformations, scoring
and adaptive profile selection. Its sole profile data file is
`crates/engineroom-domain/src/review/profiles.json`. Its normal dependency tree
contains no SQLite, process or desktop dependencies.

`engineroom-application` owns the engine and repository ports, UCI protocol,
review pipeline, worker pool, automatic scheduling, session and saved-review
normalization. `engineroom-sqlite` owns database operations, cache decoding and
`SqliteRepository`; `engineroom-uci` owns the native process adapter. Both
adapters depend on application ports. `engineroom-backend` remains the facade
for existing review and database paths and owns Chess.com imports.

SQLite converts mode values at its boundary using `SqlMode`. The domain `Mode`
intentionally does not implement `rusqlite::ToSql` or `FromSql`; database
operations preserve the existing storage representation without adding SQLite
to the domain.
