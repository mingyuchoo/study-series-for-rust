#!/usr/bin/env python3
"""Loan early-repayment posting service (next generation).

JSON-lines contract:
  stdin : {"id", "initial_state", "input", "options"}
  stdout: {"output", "state_change", "events", "external_calls"}
"""
import hashlib, json, math, re, sys

ISO_DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")

DAYS_IN_YEAR = 365
FEE_RATE_STD = 0.015
FEE_RATE_VIP = 0.005
RETRYABLE_FAULTS = ("db_timeout", "api_timeout", "slow_response")
ROLLBACK_FAULTS = ("partial_commit",)


def round_half_up(x):
    """COBOL ROUNDED semantics (half-up) for non-negative amounts."""
    return int(math.floor(x + 0.5))


def num(v):
    if isinstance(v, bool) or v is None:
        return None
    if isinstance(v, (int, float)):
        return v
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def reject(reason, trace):
    return {"output": {"status": "REJECTED", "reason": reason, "trace_id": trace}, "state_change": {}, "events": [{"type": "REJECTED", "reason": reason}], "external_calls": []}


def early_repayment_fee(grade, age_years, amount):
    """BR-LOAN-000183/184/187: VIP loans older than 3 years pay no fee; VIP 0.5%, others 1.5%; floor 1,000."""
    if grade == "VIP" and age_years > 3:
        return 0, True
    rate = FEE_RATE_VIP if grade == "VIP" else FEE_RATE_STD
    fee = round_half_up(amount * rate)
    return fee, False


def accrued_interest(principal, rate, days):
    """BR-LOAN-000185: ACT/365, rounded half-up to the won."""
    return round_half_up(principal * rate * days / DAYS_IN_YEAR)


def post_repayment(state, inp, options):
    fault = (options or {}).get("fault")
    cust = inp.get("customer") or {}
    loan = inp.get("loan") or {}
    rep = inp.get("repayment") or {}
    rid = inp.get("request_id")
    grade = cust.get("grade")
    principal = num(loan.get("principal"))
    rate = num(loan.get("annual_rate"))
    age = num(loan.get("age_years"))
    days = num(loan.get("days_since_last_accrual"))
    amount = num(rep.get("amount"))
    rtype = rep.get("type")
    date = rep.get("date")
    trace = "NX-" + hashlib.sha256(json.dumps(inp, sort_keys=True).encode()).hexdigest()[:10]

    if amount is None or amount <= 0:
        return reject("INVALID_AMOUNT", trace)
    if principal is None or principal <= 0 or rate is None or age is None or days is None or grade is None or rtype is None or rid is None:
        return reject("INVALID_INPUT", trace)
    if days < 0 or rate < 0 or age < 0:
        return reject("INVALID_INPUT", trace)
    if not isinstance(date, str) or not ISO_DATE.match(date):
        return reject("INVALID_DATE", trace)
    if rid in (state.get("processed_request_ids") or []):
        return {"output": {"status": "DUPLICATE_IGNORED", "request_id": rid, "trace_id": trace}, "state_change": {}, "events": [{"type": "DUPLICATE_IGNORED"}], "external_calls": []}
    if fault in RETRYABLE_FAULTS:
        return {"output": {"status": "RETRY_SCHEDULED", "retry_after_ms": 500, "trace_id": trace}, "state_change": {}, "events": [{"type": "RETRY_SCHEDULED", "fault": fault}], "external_calls": []}

    interest = accrued_interest(principal, rate, days)
    fee, waived = (0, False)
    if rtype == "EARLY":
        fee, waived = early_repayment_fee(grade, age, amount)
    if amount < interest + fee:
        return reject("AMOUNT_BELOW_CHARGES", trace)
    principal_after = principal - (amount - interest - fee)
    if principal_after < 0:
        return reject("OVERPAYMENT", trace)
    if fault in ROLLBACK_FAULTS:
        return {"output": {"status": "ROLLED_BACK", "trace_id": trace}, "state_change": {}, "events": [{"type": "ROLLED_BACK", "fault": fault}], "external_calls": []}

    txid = "TX-" + hashlib.sha1(str(rid).encode()).hexdigest()[:12].upper()
    events = [{"type": "REPAYMENT_POSTED", "loan_id": loan.get("id"), "amount": amount}]
    if waived:
        events.append({"type": "FEE_WAIVED", "loan_id": loan.get("id")})
    credit = [
        {"account": "INTEREST_INCOME", "amount": interest},
        {"account": "FEE_INCOME", "amount": fee},
        {"account": "LOAN_PRINCIPAL", "amount": amount - interest - fee},
    ]
    output = {
        "status": "POSTED",
        "transaction_id": txid,
        "timestamp": f"{date}T00:00:00Z",
        "interest": interest,
        "fee": fee,
        "principal_reduction": amount - interest - fee,
        "principal_after": principal_after,
        "debit": [{"account": "CASH", "amount": amount}],
        "credit": credit,
        "trace_id": trace,
    }
    return {
        "output": output,
        "state_change": {"loan_balance_before": principal, "loan_balance_after": principal_after},
        "events": events,
        "external_calls": [{"interface": "IF-CALL-IF238", "loan_id": loan.get("id"), "principal_after": principal_after}],
    }


def run_schedule(state, inp, sched):
    """Optimistic concurrency: a write commits only if the row version is unchanged since the read."""
    balance = num(state.get("balance")) or 0
    version = 0
    txns = {t["id"]: t for t in inp.get("transactions", [])}
    snapshots = {}
    outcomes = {}
    for step in sched.get("steps", []):
        t, op = step["txn"], step["op"]
        if op.startswith("r"):
            snapshots[t] = (balance, version)
        else:
            amt = num(txns[t]["amount"]) or 0
            snap_balance, snap_version = snapshots.get(t, (balance, version))
            if snap_version != version:
                snap_balance = balance  # retry read under the current version
            if snap_balance - amt >= 0:
                balance = snap_balance - amt
                version += 1
                outcomes[t] = "COMMITTED"
            else:
                outcomes[t] = "REJECTED"
    return {"output": {"final_balance": balance, "outcomes": outcomes}, "state_change": {"balance": balance}, "events": [], "external_calls": []}


def handle(case):
    options = case.get("options") or {}
    if options.get("schedule"):
        return run_schedule(case.get("initial_state") or {}, case.get("input") or {}, options["schedule"])
    return post_repayment(case.get("initial_state") or {}, case.get("input") or {}, options)


def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            case = json.loads(line)
            res = handle(case)
            res["id"] = case.get("id")
            print(json.dumps(res))
        except Exception as e:  # noqa: BLE001
            print(json.dumps({"error": f"{type(e).__name__}: {e}"}))
        sys.stdout.flush()


if __name__ == "__main__":
    main()
