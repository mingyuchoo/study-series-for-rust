-- Business invariants checked on BOTH legacy and next outputs (design §12).
-- A legacy bug ported faithfully still fails here.

RULE ACCOUNTING-001 WHEN status == "POSTED" THEN SUM(debit.amount) == SUM(credit.amount)

RULE LOAN-002 WHEN status == "POSTED" THEN
  principal_after == input.loan.principal - (input.repayment.amount - interest - fee)

RULE LOAN-003 WHEN status == "POSTED" THEN principal_after >= 0 AND fee >= 0 AND interest >= 0

RULE LOAN-004 WHEN status == "POSTED" THEN
  state_change.loan_balance_after == principal_after AND state_change.loan_balance_before == input.loan.principal

RULE LOAN-005 WHEN status == "POSTED" AND input.repayment.type == "SCHEDULED" THEN fee == 0

RULE LOAN-006 WHEN status == "REJECTED" THEN COUNT(state_change) == 0
