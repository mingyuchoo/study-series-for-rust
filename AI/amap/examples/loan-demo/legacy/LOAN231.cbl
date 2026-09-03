       IDENTIFICATION DIVISION.
       PROGRAM-ID. LOAN231.
      *================================================================*
      * LOAN EARLY REPAYMENT POSTING                                    *
      * - VALIDATES REQUEST, COMPUTES ACCRUED INTEREST AND EARLY        *
      *   REPAYMENT FEE, POSTS LEDGER ENTRIES, NOTIFIES IF238           *
      *================================================================*
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       COPY LOANREC.
       01  WS-CUSTOMER.
           05  WS-CUST-ID            PIC X(10).
           05  WS-GRADE              PIC X(03).
       01  WS-LOAN.
           05  WS-LOAN-ID            PIC X(10).
           05  WS-PRINCIPAL          PIC 9(13)V99.
           05  WS-RATE               PIC 9V9(5).
           05  WS-AGE-YEARS          PIC 9(02).
           05  WS-DAYS               PIC 9(03).
       01  WS-REPAY.
           05  WS-REPAY-TYPE         PIC X(09).
           05  WS-AMOUNT             PIC 9(13)V99.
           05  WS-REPAY-DATE         PIC X(10).
       01  WS-CALC.
           05  WS-INTEREST           PIC 9(13).
           05  WS-FEE                PIC 9(13).
           05  WS-PRINCIPAL-AFTER    PIC S9(13).
           05  WS-STATUS             PIC X(20).
           05  WS-REASON             PIC X(30).
       01  WS-CONST.
           05  WS-DAYS-IN-YEAR       PIC 9(03) VALUE 365.
           05  WS-FEE-RATE-STD       PIC 9V9(3) VALUE 0.015.
           05  WS-FEE-RATE-VIP       PIC 9V9(3) VALUE 0.005.
           05  WS-FEE-FLOOR          PIC 9(04) VALUE 1000.
       PROCEDURE DIVISION.
       0000-MAIN.
           PERFORM 1000-VALIDATE
           IF WS-STATUS = 'REJECTED'
              PERFORM 7000-REPLY
              STOP RUN
           END-IF
           PERFORM 2000-READ-CUSTOMER
           PERFORM 2500-CHECK-DUPLICATE
           IF WS-STATUS = 'DUPLICATE_IGNORED'
              PERFORM 7000-REPLY
              STOP RUN
           END-IF
           PERFORM 3000-CALC-INTEREST
           PERFORM 4000-CALC-FEE
           PERFORM 4500-CHECK-CHARGES
           IF WS-STATUS = 'REJECTED'
              PERFORM 7000-REPLY
              STOP RUN
           END-IF
           PERFORM 5000-POST
           PERFORM 6000-WRITE-LEDGER
           PERFORM 7000-REPLY
           STOP RUN.
       1000-VALIDATE.
      *    BR: AMOUNT MUST BE POSITIVE, PRINCIPAL POSITIVE, DAYS >= 0
           MOVE 'POSTED' TO WS-STATUS
           IF WS-AMOUNT NOT > 0
              MOVE 'REJECTED' TO WS-STATUS
              MOVE 'INVALID_AMOUNT' TO WS-REASON
           END-IF
           IF WS-PRINCIPAL NOT > 0
              MOVE 'REJECTED' TO WS-STATUS
              MOVE 'INVALID_INPUT' TO WS-REASON
           END-IF
           IF WS-DAYS < 0
              MOVE 'REJECTED' TO WS-STATUS
              MOVE 'INVALID_INPUT' TO WS-REASON
           END-IF.
       2000-READ-CUSTOMER.
           EXEC SQL
              SELECT GRADE INTO :WS-GRADE
              FROM CUSTOMER
              WHERE CUST_ID = :WS-CUST-ID
           END-EXEC.
       2500-CHECK-DUPLICATE.
           EXEC SQL
              SELECT COUNT(*) INTO :WS-DUP-COUNT
              FROM REQUEST_LOG
              WHERE REQUEST_ID = :WS-REQUEST-ID
           END-EXEC
           IF WS-DUP-COUNT > 0
              MOVE 'DUPLICATE_IGNORED' TO WS-STATUS
           END-IF.
       3000-CALC-INTEREST.
      *    BR: ACCRUED INTEREST, ACT/365, ROUNDED HALF-UP TO WON
           COMPUTE WS-INTEREST ROUNDED =
              WS-PRINCIPAL * WS-RATE * WS-DAYS / WS-DAYS-IN-YEAR.
       4000-CALC-FEE.
      *    BR: EARLY REPAYMENT FEE
      *        VIP AND LOAN AGE > 3Y      -> FEE 0 (WAIVED)
      *        VIP AND LOAN AGE <= 3Y     -> 0.5% OF AMOUNT
      *        OTHER GRADES               -> 1.5% OF AMOUNT
      *        FEE FLOOR 1,000 WON WHEN A FEE APPLIES
      *        SCHEDULED REPAYMENT        -> NO FEE
           MOVE 0 TO WS-FEE
           IF WS-REPAY-TYPE = 'EARLY'
              IF WS-GRADE = 'VIP'
                 IF WS-AGE-YEARS > 3
                    MOVE 0 TO WS-FEE
                 ELSE
                    COMPUTE WS-FEE ROUNDED = WS-AMOUNT * WS-FEE-RATE-VIP
                 END-IF
              ELSE
                 COMPUTE WS-FEE ROUNDED = WS-AMOUNT * WS-FEE-RATE-STD
              END-IF
              IF WS-FEE > 0 AND WS-FEE < WS-FEE-FLOOR
                 MOVE WS-FEE-FLOOR TO WS-FEE
              END-IF
           END-IF.
       4500-CHECK-CHARGES.
      *    BR: AMOUNT MUST COVER INTEREST + FEE; NO OVERPAYMENT
           IF WS-AMOUNT < WS-INTEREST + WS-FEE
              MOVE 'REJECTED' TO WS-STATUS
              MOVE 'AMOUNT_BELOW_CHARGES' TO WS-REASON
           END-IF
           COMPUTE WS-PRINCIPAL-AFTER =
              WS-PRINCIPAL - (WS-AMOUNT - WS-INTEREST - WS-FEE)
           IF WS-PRINCIPAL-AFTER < 0
              MOVE 'REJECTED' TO WS-STATUS
              MOVE 'OVERPAYMENT' TO WS-REASON
           END-IF.
       5000-POST.
           EXEC SQL
              UPDATE LOAN_MASTER
              SET PRINCIPAL = :WS-PRINCIPAL-AFTER
              WHERE LOAN_ID = :WS-LOAN-ID
           END-EXEC
           EXEC SQL
              INSERT INTO REQUEST_LOG (REQUEST_ID) VALUES (:WS-REQUEST-ID)
           END-EXEC
           CALL 'IF238' USING WS-LOAN WS-CALC.
       6000-WRITE-LEDGER.
      *    BR: DOUBLE-ENTRY - DEBIT CASH, CREDIT PRINCIPAL/INTEREST/FEE
           EXEC SQL
              INSERT INTO LEDGER (ACCOUNT, DR, CR)
              VALUES ('CASH', :WS-AMOUNT, 0)
           END-EXEC
           EXEC SQL
              INSERT INTO LEDGER (ACCOUNT, DR, CR)
              VALUES ('LOAN_PRINCIPAL', 0, :WS-AMOUNT - :WS-INTEREST - :WS-FEE)
           END-EXEC
           EXEC SQL
              INSERT INTO LEDGER (ACCOUNT, DR, CR)
              VALUES ('INTEREST_INCOME', 0, :WS-INTEREST)
           END-EXEC
           EXEC SQL
              INSERT INTO LEDGER (ACCOUNT, DR, CR)
              VALUES ('FEE_INCOME', 0, :WS-FEE)
           END-EXEC.
       7000-REPLY.
           DISPLAY WS-STATUS WS-REASON.
       9000-LEGACY-REPRICE.
      *    DEAD SINCE 2014 - NEVER PERFORMED
           COMPUTE WS-FEE ROUNDED = WS-AMOUNT * 0.02.
