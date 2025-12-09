# Incident Playbook: Budget Exhaustion

## Alert: BudgetExhaustionRate (>50% budgets exhausted)
**Severity**: CRITICAL (>50%), WARNING (>25%)

## Initial Response (1 minute)

```bash
# Check exhaustion rate
curl http://localhost:8080/metrics | grep -E "(budget_exhausted_count|budget_active_count)"

# Calculate: exhausted / active
```

## Root Cause Analysis

### Scenario 1: Legitimate High Usage
**Symptoms**: High traffic, expected usage

**Action**:
1. Increase budget limits in config
2. Enable auto-refill (if applicable)
3. Scale budget allocation (add more slots)

### Scenario 2: Budget Leak
**Symptoms**: Budgets exhausted but no corresponding usage

**Check**:
```bash
# Compare budget deductions to actual provider requests
curl http://localhost:8080/metrics | grep -E "(budget_deductions_total|proxy_requests_total)"

# Should be roughly equal
# If deductions >> requests, leak detected
```

**Fix**: Restart service, audit deduction logic

### Scenario 3: Refill Not Working
**Symptoms**: Budgets not refilling on schedule

**Check**:
```bash
# Check last refill timestamp (if implemented)
curl http://localhost:8080/metrics | grep budget_last_refill_timestamp

# Should be <1 hour ago
```

**Fix**: Restart refill scheduler

## Mitigation

### Immediate
1. **Increase budget limits**:
   ```toml
   [server]
   default_budget_cents = 500_00  # Increase from $100 to $500
   ```

2. **Manual refill** (if API exists):
   ```bash
   curl -X POST http://localhost:8080/admin/budgets/refill_all
   ```

### Long-term
1. **Auto-scaling budgets**: Dynamic budget allocation
2. **Usage alerts**: Warn before exhaustion
3. **Budget forecasting**: Predict exhaustion

## Verification
```bash
# Exhaustion rate should drop
curl http://localhost:8080/metrics | grep budget_exhausted_count
```

## Related Playbooks
- [High Error Rate](high_error_rate_playbook.md)
- [High Contention](high_contention_playbook.md)
