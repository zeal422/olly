# Refactor Baseline Matrix

## Purpose
Baseline guardrails for refactor execution. This captures behavior that must remain unchanged while phases from `REFACTOR_PLAN.md` are implemented.

## Baseline Snapshot
- Date: 2026-03-30
- Branch: `REFA`
- Quality gate command: `./scripts/check.ps1`
- Expected result before/after each phase: pass (`fmt`, `clippy -D warnings`, `build`, full tests)

## Startup Precedence (Must Not Change)
Model selection precedence at startup:
1. CLI `--model` override
2. Persisted runtime state (`ui_state.json` `model`)
3. `config.toml` `[defaults].model`

## Critical Journeys (Manual Smoke)
- Launch app and restore last workspace/session context.
- Send prompt and receive streaming response with persistence.
- Switch model and verify footer/status coherence.
- `/workspace` create/switch/rename/delete + delete modal Enter/Esc.
- `/session` create/switch/rename/delete + delete modal Enter/Esc.
- `/themes` modal open, live preview, confirm/cancel.
- Restart app and verify restored `ctx:used/max` telemetry.
- Windows startup/resize scrollbar behavior remains stable.

## Regression Tests Confirmed in Baseline
- UI state backward compatibility (older `ui_state.json` shape deserializes safely).
- Startup model precedence with/without CLI override.
- Restart context telemetry restore (`ctx:used/max`) for matching session/model.
