#!/usr/bin/env python3
"""Generate synthetic production traces (Behavior Records) from the legacy emulator."""
import json, random, sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "legacy"))
from legacy_emulator import process  # noqa: E402

random.seed(20260904)
FN = "FN-LOAN-0001"
out = []
for n in range(1, 301):
    grade = random.choice(["VIP", "VIP", "GOLD", "STD", "STD"])
    age = random.randint(0, 8)
    rtype = random.choice(["EARLY", "EARLY", "EARLY", "SCHEDULED"])
    principal = random.randint(10, 1000) * 100000
    amount = random.randint(1, 50) * 100000
    if amount > principal:
        amount = principal // 2
    inp = {
        "request_id": f"REQ-{n:06d}",
        "customer": {"id": f"C{random.randint(1, 9999):05d}", "grade": grade},
        "loan": {"id": f"L{n:06d}", "principal": principal, "annual_rate": round(random.uniform(0.025, 0.085), 4), "age_years": age, "days_since_last_accrual": random.randint(1, 92)},
        "repayment": {"type": rtype, "amount": amount, "date": f"2026-{random.randint(1, 8):02d}-{random.randint(1, 28):02d}"},
    }
    state = {"loan_balance": principal, "processed_request_ids": []}
    res = process(state, inp, {})
    out.append({
        "id": f"BH-{92180000 + n}",
        "function_id": FN,
        "initial_state": state,
        "input": inp,
        "legacy_output": res["output"],
        "db_state_change": res["state_change"],
        "events": res["events"],
        "external_calls": res["external_calls"],
        "timing_ms": random.randint(12, 90),
        "related_rules": [],
        "priority": "P0" if amount >= 1000000 else "P1",
    })
with open(os.path.join(os.path.dirname(__file__), "traces", "production.jsonl"), "w") as f:
    for r in out:
        f.write(json.dumps(r) + "\n")
print(f"wrote {len(out)} behavior records")
