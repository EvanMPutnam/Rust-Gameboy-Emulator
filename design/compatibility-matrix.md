# Compatibility Matrix Baseline

This baseline was generated for the phase-10 compatibility harness introduced in this repo.

## Tier Summary

| Tier | Total | Pass | Fail | Timeout | Unsupported |
| --- | ---: | ---: | ---: | ---: | ---: |
| smoke | 1 | 1 | 0 | 0 | 0 |
| core | 1 | 1 | 0 | 0 | 0 |
| full | 1 | 1 | 0 | 0 | 0 |

## Known Blockers

- No blockers currently recorded by the seeded baseline manifests.
- Failure categorization is active in `gb-headless` and will classify new regressions as they are added.

## Notes

- Baseline manifests live in `testdata/compat/manifests`.
- Additional commercial/regression ROMs should be added incrementally with expected outcomes and tags.
- Every compatibility bug fix must add a corresponding regression test under `crates/gb-core/tests/regression`.
