# Loan Early Repayment — Business Requirements (excerpt)

REQ-1203 Early repayment posting must compute accrued interest on an ACT/365 basis, rounded
half-up to the won (**Accrued interest ACT/365**).

REQ-1204 **VIP early repayment fee waiver**: VIP customers whose loan is older than three years pay
no early repayment fee. Other VIP loans pay 0.5% (**VIP early repayment fee**); non-VIP customers pay
1.5% (**Standard early repayment fee**). When a fee applies it is never below 1,000 won
(**Early repayment fee floor**). Scheduled repayments carry no fee.

REQ-1205 A repayment must cover accrued interest and fee (**Amount covers charges**) and must not
exceed the outstanding principal (**No overpayment**).

REQ-1206 Every posting produces balanced double-entry ledger lines and notifies interface IF238.

REQ-1207 Requests are idempotent by request id (**Duplicate request ignored**).
