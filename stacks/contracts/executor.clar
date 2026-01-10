;; title: executor
;; version:
;; summary:
;; description:

;; traits
;;

;; token definitions
;;

;; constants
(define-constant EXECUTOR-VERSION "Executor-0.0.1")
(define-constant OUR-CHAIN u60) ;; Must be manually updated before deployment. Worth doing it this way or an initialize call?

;; addr32 contract reference - update before deployment
;; Local/Test: .addr32
;; Testnet: 'ST2W4SFFKXMGFJW7K7NZFK3AH52ZTXDB74HKV9MRA.addr32
 
;; errors
(define-constant ERR-QUOTE-SRC-CHAIN-MISMATCH (err u1001))
(define-constant ERR-QUOTE-DST-CHAIN-MISMATCH (err u1002))
(define-constant ERR-QUOTE-EXPIRED (err u1003))
(define-constant ERR-UNREGISTERED-PAYEE (err u1004))
(define-constant ERR-INVALID-PAYEE-ADDRESS (err u1005))
(define-constant ERR-BUFFER-PARSE-ERROR (err u1006))
(define-constant ERR-STACKS-TIMESTAMP (err u1007))
;;

;; data vars
;;

;; data maps
;;

;; public functions
(define-public (request-execution
    (dst-chain uint)
    (dst-addr (buff 32))
    (refund-addr principal)
    (signed-quote-bytes (buff 8192))
    (request-bytes (buff 8192))
    (relay-instructions (buff 8192))
    (payment uint)
  )
  ;; STX amount in microSTX
  (begin
    ;; Validate quote header
    (try! (validate-quote-header signed-quote-bytes dst-chain))

    ;; Extract quoter and payee info  
    (match (extract-quote-addresses signed-quote-bytes)
      quote-addresses (let ((payee-universal-addr (get payee quote-addresses)))
        ;; 1. Verify universal address is properly formatted (32 bytes, non-zero)
        (asserts! (is-eq (len payee-universal-addr) u32)
          ERR-INVALID-PAYEE-ADDRESS
        )

        ;; 2. Verify payee is registered and get principal
        (let ((payee-lookup-result (contract-call? .addr32 lookup
            payee-universal-addr
          )))
          (asserts! (is-some payee-lookup-result) ERR-UNREGISTERED-PAYEE)

          ;; 3. Extract the principal for payment
          (let ((payee-principal (unwrap-panic payee-lookup-result)))
            ;; 4. Perform the payment after all validations pass
            (try! (stx-transfer? payment tx-sender payee-principal))

            ;; Emit event for off-chain relayers
            (print {
              event: "RequestForExecution",
              quoter-address: (get quoter quote-addresses),
              amount-paid: payment,
              dst-chain: dst-chain,
              dst-addr: dst-addr,
              refund-addr: refund-addr,
              signed-quote: signed-quote-bytes,
              request-bytes: request-bytes,
              relay-instructions: relay-instructions,
            })

            (ok true)
          )
        )
      )
      err-case
      ERR-BUFFER-PARSE-ERROR
    )
  )
)
;;

;; read only functions
;; Extract uint16 from buffer at specific offset (big-endian) using efficient slice operation
(define-read-only (extract-uint16-be
    (data (buff 8192))
    (offset uint)
  )
  (let ((extracted (slice? data offset (+ offset u2))))
    (match extracted
      result (if (is-eq (len result) u2)
        (ok (buff-to-uint-be (unwrap-panic (as-max-len? result u2))))
        (err ERR-BUFFER-PARSE-ERROR)
      )
      (err ERR-BUFFER-PARSE-ERROR)
    )
  )
)

;; Extract uint64 from buffer at specific offset (big-endian) using efficient slice operation
(define-read-only (extract-uint64-be
    (data (buff 8192))
    (offset uint)
  )
  (let ((extracted (slice? data offset (+ offset u8))))
    (match extracted
      result (if (is-eq (len result) u8)
        (ok (buff-to-uint-be (unwrap-panic (as-max-len? result u8))))
        (err ERR-BUFFER-PARSE-ERROR)
      )
      (err ERR-BUFFER-PARSE-ERROR)
    )
  )
)

;; Extract bytes32 from buffer at specific offset
;; Returns a properly sized 32-byte buffer using efficient slice operation
(define-read-only (extract-bytes32
    (data (buff 8192))
    (offset uint)
  )
  (let ((extracted (slice? data offset (+ offset u32))))
    (match extracted
      result (if (is-eq (len result) u32)
        (ok (unwrap-panic (as-max-len? result u32)))
        (err ERR-BUFFER-PARSE-ERROR)
      )
      (err ERR-BUFFER-PARSE-ERROR)
    )
  )
)

;; Extract address (20 bytes) from buffer at specific offset
(define-read-only (extract-address
    (data (buff 8192))
    (offset uint)
  )
  (slice? data offset (+ offset u20))
)

;; Validate quote header
(define-read-only (validate-quote-header
    (signed-quote-bytes (buff 8192))
    (dst-chain uint)
  )
  (match (extract-uint16-be signed-quote-bytes u56)
    quote-src-chain (match (extract-uint16-be signed-quote-bytes u58)
      quote-dst-chain (match (extract-uint64-be signed-quote-bytes u60)
        expiry-time (if (is-eq quote-src-chain OUR-CHAIN)
          (if (is-eq quote-dst-chain dst-chain)
            ;; Get previous block's timestamp for reliable comparison
            (let ((current-time (unwrap! (get-stacks-block-info? time (- stacks-block-height u1)) ERR-STACKS-TIMESTAMP)))
              (if (> expiry-time current-time)
                (ok true)
                ERR-QUOTE-EXPIRED
              )
            )
            ERR-QUOTE-DST-CHAIN-MISMATCH
          )
          ERR-QUOTE-SRC-CHAIN-MISMATCH
        )
        err3
        ERR-BUFFER-PARSE-ERROR
      )
      err2
      ERR-BUFFER-PARSE-ERROR
    )
    err1
    ERR-BUFFER-PARSE-ERROR
  )
)

;; Extract quoter and payee addresses from quote
(define-read-only (extract-quote-addresses (signed-quote-bytes (buff 8192)))
  (match (extract-address signed-quote-bytes u4)
    quoter-addr (match (extract-bytes32 signed-quote-bytes u24)
      payee-addr-32 (ok {
        quoter: quoter-addr,
        payee: payee-addr-32,
      })
      err (err ERR-BUFFER-PARSE-ERROR)
    )
    (err ERR-BUFFER-PARSE-ERROR)
  )
)

;; Convert 32-byte universal address hash back to a Stacks principal
;; Uses the addr32 contract's registry
(define-read-only (universal-addr-to-principal (universal-addr (buff 32)))
  (contract-call? .addr32 lookup
    universal-addr
  )
)

;; Read-only functions for external access
(define-read-only (get-executor-version)
  EXECUTOR-VERSION
)

(define-read-only (get-our-chain)
  OUR-CHAIN
)
;;

;; private functions
;;
