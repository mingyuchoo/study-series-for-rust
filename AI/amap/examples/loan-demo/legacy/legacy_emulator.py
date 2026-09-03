#!/usr/bin/env python3
"""Legacy replay adapter: emulates LOAN231.cbl semantics for differential testing.

In a real engagement this adapter drives the actual legacy runtime (mainframe replay,
recorded-response server). It speaks the AMAP JSON-lines protocol:
  stdin : {"id", "initial_state", "input", "options"}
  stdout: {"output", "state_change", "events", "external_calls"}
"""
import hashlib, json, math, re, sys

ISO_DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")

DAYS_IN_YEAR = 365
FEE_RATE_STD = 0.015
FEE_RATE_VIP = 0.005
FEE_FLOOR = 1000


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


def process(state, inp, options):
    fault = (options or {}).get("fault")
    sched = (options or {}).get("schedule")
    if sched:
        return schedule(state, inp, sched)
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
    trace = "LGCY-" + hashlib.sha1(json.dumps(inp, sort_keys=True).encode()).hexdigest()[:8]

    def reject(reason):
        return {"output": {"status": "REJECTED", "reason": reason, "trace_id": trace}, "state_change": {}, "events": [{"type": "REJECTED", "reason": reason}], "external_calls": []}

    # 1000-VALIDATE
    if amount is None or amount <= 0:
        return reject("INVALID_AMOUNT")
    if principal is None or principal <= 0 or rate is None or age is None or days is None or grade is None or rtype is None or rid is None:
        return reject("INVALID_INPUT")
    if days < 0 or rate < 0 or age < 0:
        return reject("INVALID_INPUT")
    if not isinstance(date, str) or not ISO_DATE.match(date):
        return reject("INVALID_DATE")
    # 2500-CHECK-DUPLICATE
    if rid in (state.get("processed_request_ids") or []):
        return {"output": {"status": "DUPLICATE_IGNORED", "request_id": rid, "trace_id": trace}, "state_change": {}, "events": [{"type": "DUPLICATE_IGNORED"}], "external_calls": []}
    if fault in ("db_timeout", "api_timeout", "slow_response"):
        return {"output": {"status": "RETRY_SCHEDULED", "retry_after_ms": 500, "trace_id": trace}, "state_change": {}, "events": [{"type": "RETRY_SCHEDULED", "fault": fault}], "external_calls": []}
    # 3000-CALC-INTEREST (ACT/365, half-up)
    interest = round_half_up(principal * rate * days / DAYS_IN_YEAR)
    # 4000-CALC-FEE
    fee = 0
    waived = False
    if rtype == "EARLY":
        if grade == "VIP":
            if age > 3:
                fee = 0
                waived = True
            else:
                fee = round_half_up(amount * FEE_RATE_VIP)
        else:
            fee = round_half_up(amount * FEE_RATE_STD)
        if fee > 0 and fee < FEE_FLOOR:
            fee = FEE_FLOOR
    # 4500-CHECK-CHARGES
    if amount < interest + fee:
        return reject("AMOUNT_BELOW_CHARGES")
    principal_after = principal - (amount - interest - fee)
    if principal_after < 0:
        return reject("OVERPAYMENT")
    if fault in ("partial_commit",):
        return {"output": {"status": "ROLLED_BACK", "trace_id": trace}, "state_change": {}, "events": [{"type": "ROLLED_BACK", "fault": fault}], "external_calls": []}
    # 5000-POST / 6000-WRITE-LEDGER
    txid = "TX-" + hashlib.sha1(str(rid).encode()).hexdigest()[:12].upper()
    events = [{"type": "REPAYMENT_POSTED", "loan_id": loan.get("id"), "amount": amount}]
    if waived:
        events.append({"type": "FEE_WAIVED", "loan_id": loan.get("id")})
    output = {
        "status": "POSTED",
        "transaction_id": txid,
        "timestamp": f"{date}T09:00:00+09:00",
        "interest": interest,
        "fee": fee,
        "principal_reduction": amount - interest - fee,
        "principal_after": principal_after,
        "debit": [{"account": "CASH", "amount": amount}],
        "credit": [
            {"account": "LOAN_PRINCIPAL", "amount": amount - interest - fee},
            {"account": "INTEREST_INCOME", "amount": interest},
            {"account": "FEE_INCOME", "amount": fee},
        ],
        "trace_id": trace,
    }
    return {
        "output": output,
        "state_change": {"loan_balance_before": principal, "loan_balance_after": principal_after},
        "events": events,
        "external_calls": [{"interface": "IF-CALL-IF238", "loan_id": loan.get("id"), "principal_after": principal_after}],
    }


def schedule(state, inp, sched):
    """Concurrency semantics: row-level pessimistic lock + balance check (legacy CICS behaviour)."""
    balance = num(state.get("balance")) or 0
    version = 0
    txns = {t["id"]: t for t in inp.get("transactions", [])}
    seen = {}
    outcomes = {}
    for step in sched.get("steps", []):
        t, op = step["txn"], step["op"]
        if op.startswith("r"):
            seen[t] = (balance, version)
        else:
            amt = num(txns[t]["amount"]) or 0
            snap_balance, snap_version = seen.get(t, (balance, version))
            if snap_version != version:
                # re-read under lock (legacy locks the row on update)
                snap_balance = balance
            if snap_balance - amt >= 0:
                balance = snap_balance - amt
                version += 1
                outcomes[t] = "COMMITTED"
            else:
                outcomes[t] = "REJECTED"
    return {"output": {"final_balance": balance, "outcomes": outcomes}, "state_change": {"balance": balance}, "events": [], "external_calls": []}


def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            case = json.loads(line)
            res = process(case.get("initial_state") or {}, case.get("input") or {}, case.get("options") or {})
            res["id"] = case.get("id")
            print(json.dumps(res))
        except Exception as e:  # noqa: BLE001
            print(json.dumps({"error": f"{type(e).__name__}: {e}"}))
        sys.stdout.flush()


if __name__ == "__main__":
    main()
