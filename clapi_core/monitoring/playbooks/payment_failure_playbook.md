# Incident Playbook: Payment Failure

## Alert: PaymentFailureRate (>5% payment failures)
**Severity**: CRITICAL

## Initial Response (1 minute)

```bash
# Check payment failure rate
curl http://localhost:8080/metrics | grep -E "(payments_failed_total|payments_recorded_total)"

# Check Stripe webhook status
curl http://localhost:8080/health | grep stripe_webhook_status
```

## Root Cause Analysis

### Scenario 1: Stripe Webhook Failure
**Symptoms**: Payments recorded but not confirmed

**Action**:
```bash
# Check Stripe webhook logs
journalctl -u clapi_core -n 100 | grep stripe_webhook

# Verify webhook signature validation
```

**Fix**: Re-verify Stripe webhook secret, restart webhook handler

### Scenario 2: Amount Precision Error
**Symptoms**: Fixed-point arithmetic underflow

**Check**:
```bash
# Check for sub-cent amounts (Q16.16 precision issue)
curl http://localhost:8080/metrics | grep payment_amount_q16

# If amounts <1/65536 (~$0.000015), precision error
```

**Fix**: Validate minimum payment amount ($0.01)

### Scenario 3: Idempotency Key Collision
**Symptoms**: Duplicate payment rejection

**Check**: Hash collision on payment_id
```bash
# Check for duplicate payment_ids
curl http://localhost:8080/metrics | grep payment_idempotency_conflicts_total
```

**Fix**: Increase hash entropy, add timestamp to hash

## Mitigation

### Immediate
1. **Retry failed payments**:
   ```bash
   curl -X POST http://localhost:8080/admin/payments/retry_failed
   ```

2. **Manual confirmation**:
   ```bash
   curl -X POST http://localhost:8080/admin/payments/confirm \
     -H "Content-Type: application/json" \
     -d '{"payment_id": "pi_xxx"}'
   ```

### Long-term
1. **Webhook monitoring**: Alert on webhook failures
2. **Payment reconciliation**: Daily Stripe API sync
3. **Testing**: Add sub-cent amount tests (T28 Q8-Q14)

## Verification
```bash
# Payment failure rate should drop to <0.5%
curl http://localhost:8080/metrics | grep payments_failed_total
```

## Related Playbooks
- [OAuth Failure](oauth_failure_playbook.md)
- [High Error Rate](high_error_rate_playbook.md)
