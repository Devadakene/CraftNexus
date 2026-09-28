param(
    [string]$OutputPath
)

$description = @'
## Description

Make `get_escalation_checkpoints` return `None` when its persistent schedule key is missing, and refresh the key TTL on successful reads. Add host coverage for missing schedule storage before and after a dispute reaches a terminal state.

## Type of Change

- [x] Bug fix
- [ ] New feature
- [ ] Breaking change
- [x] Documentation update

## Files Modified

- `craft-nexus-contract/src/lib.rs`
- `craft-nexus-contract/src/dispute_escalation_timeout_test.rs`
- `craft-nexus-contract/docs/ERROR_CATALOG.md`
- `scripts/generate-pr-description.ps1`

## Testing

- [ ] Tested locally (Cargo compilation is blocked by pre-existing merge-conflict markers in the contract source)
- [x] Added unit tests
- [ ] Tested on Stellar Testnet (for wallet/contract changes)

## Code Quality checks

- [x] `git diff --check`
- [ ] Contract tests: attempted in Ubuntu WSL; blocked before test execution by pre-existing merge-conflict markers

# Behavioural Changes

`get_escalation_checkpoints` now returns `Option<EscalationCheckpoints>`: `Some` when an explicit schedule is stored and `None` when the storage key is absent. Internal dispute processing retains its existing default schedule.

## Related Issues

Closes #1308
'@

if ($OutputPath) {
    Set-Content -LiteralPath $OutputPath -Value $description -Encoding utf8
} else {
    Write-Output $description
}
