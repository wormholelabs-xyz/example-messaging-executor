;; title: executor-state
;; version: 0.0.1
;; summary: State contract for cross-chain executor relayer registry
;; description: Simple relayer address mapping for the executor system

;;;; Constants

;; State contract errors
(define-constant ERR_STATE_RELAYER_EXISTS (err u20001))

;;;; Data maps

;; Map to track payee addresses for payments
;; Universal address is keccak256(stacks-principal-as-string)
(define-map universal-address-to-principal
  (buff 32) ;; Universal address (32-byte hash)
  principal ;; Stacks principal for STX payments
)

;;;; Public functions

;; @desc Register a payee's Stacks address for their universal address
;;       Anyone can call this to register themselves as a payee
(define-public (register-payee (stacks-addr principal))
  (let (
      (p-as-string (principal-to-string stacks-addr))
      (universal-addr (keccak256 (string-ascii-to-buff p-as-string)))
    )
    ;; Check if payee already exists
    (asserts! (is-none (universal-address-to-principal-get universal-addr))
      ERR_STATE_RELAYER_EXISTS
    )

    ;; Register the mapping
    (map-set universal-address-to-principal universal-addr stacks-addr)

    ;; Return the universal address for confirmation
    (ok universal-addr)
  )
)

;;;; Read-only functions

;; Constants for principal-to-string conversion (extracted from self-listing-helper-v3)
(define-constant C32 "0123456789ABCDEFGHJKMNPQRSTVWXYZ")
(define-constant LIST_15 (list 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0))
(define-constant LIST_24 (list 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0))
(define-constant LIST_39 (concat LIST_24 LIST_15))

;; @desc Convert principal to string representation
;;       Extracted from self-listing-helper-v3 to eliminate external dependencies
(define-read-only (principal-to-string (p principal))
  (let (
      (destructed (match (principal-destruct? p) ok-value ok-value err-value err-value))
      (checksum (unwrap-panic (slice? (sha256 (sha256 (concat (get version destructed) (get hash-bytes destructed)))) u0 u4)))
      (data (unwrap-panic (as-max-len? (concat (get hash-bytes destructed) checksum) u24)))
      (result (concat (concat "S" (unwrap-panic (element-at? C32 (buff-to-uint-be (get version destructed))))) (append-leading-0 data (trim-leading-0 (hash-bytes-to-string data)))))
    )
    (match (get name destructed) n (concat (concat result ".") n) result)
  )
)

;; Helper functions for principal-to-string conversion
(define-private (c32-to-string-iter (idx int) (it { s: (string-ascii 39), r: uint }))
  { s: (unwrap-panic (as-max-len? (concat (unwrap-panic (element-at? C32 (mod (get r it) u32))) (get s it)) u39)), r: (/ (get r it) u32) })

(define-private (hash-bytes-to-string (data (buff 24)))
  (let (
      ;; fixed-length: 8 * 15 / 5 = 24
      (low-part (get s (fold c32-to-string-iter LIST_24 { s: "", r: (buff-to-uint-be (unwrap-panic (as-max-len? (unwrap-panic (slice? data u9 u24)) u16)))})))
      ;; fixed-length: ceil(8 * 9 / 5) = 15
      (high-part (get s (fold c32-to-string-iter LIST_15 { s: "", r: (buff-to-uint-be (unwrap-panic (as-max-len? (unwrap-panic (slice? data u0 u9)) u16)))})))
    )
    (unwrap-panic (as-max-len? (concat high-part low-part) u39))
  )
)

(define-private (trim-leading-0-iter (idx int) (it (string-ascii 39)))
  (if (is-eq (element-at? it u0) (some "0")) (unwrap-panic (slice? it u1 (len it))) it))

(define-private (trim-leading-0 (s (string-ascii 39)))
  (fold trim-leading-0-iter LIST_39 s))

(define-private (append-leading-0-iter (idx int) (it { hash-bytes: (buff 24), address: (string-ascii 39)}))
  (if (is-eq (element-at? (get hash-bytes it) u0) (some 0x00))
    { hash-bytes: (unwrap-panic (slice? (get hash-bytes it) u1 (len (get hash-bytes it)))), address: (unwrap-panic (as-max-len? (concat "0" (get address it)) u39)) }
    it))

(define-private (append-leading-0 (hash-bytes (buff 24)) (s (string-ascii 39)))
  (get address (fold append-leading-0-iter LIST_24 { hash-bytes: hash-bytes, address: s })))

;; @desc Helper function to convert string to buffer for hashing
;;       Matches the exact implementation from Wormhole Core
(define-read-only (string-ascii-to-buff (s (string-ascii 256)))
  (let ((cb (unwrap-panic (to-consensus-buff? s))))
    ;; Consensus buff format for string:
    ;;   bytes[0]:     Consensus Buff Type
    ;;   bytes[1..4]:  String length
    ;;   bytes[5..]:   String data
    (unwrap-panic (slice? cb u5 (len cb)))
  )
)

;;;; Map getters

(define-read-only (universal-address-to-principal-get (universal-addr (buff 32)))
  (map-get? universal-address-to-principal universal-addr)
)
