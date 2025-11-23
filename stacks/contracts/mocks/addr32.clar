;; Title: addr32
;; Version: final (CANNOT BE UPDATED)

;; This contract provides 32-byte addressing for the Stacks blockchain
;; A Stacks contract principal can be longer than 32 bytes, and some protocols can't handle that
;; We can generate a unique 32-byte address for any Stacks principal by hashing it
;; This allows us to use existing protocols unmodified

(define-constant ERR_INVALID_ADDRESS (err u901))

;; Registered principals
(define-map registry
  (buff 32)  ;; keccak256(principal)
  principal  ;; Stacks principal
)

;; @desc Get or register 32-byte address
(define-public (register (p principal))
  (if (is-standard p)
    ;; Address matches network, this is expected
    (inner-register p)
    ;; Address does not match network, need to support for unit tests
    (let ((addr32 (hash p)))
      (match (lookup addr32)
        val (ok {
          created: false,
          addr32: addr32
        })
        ERR_INVALID_ADDRESS))))

;; @desc Hash a Stacks principal to generate addr32
(define-read-only (hash (p principal))
  (keccak256 (string-ascii-to-buff (principal-to-string p))))

;; @desc Lookup Stacks principal for given addr32
(define-read-only (lookup (addr32 (buff 32)))
  (map-get? registry addr32))

;; @desc Lookup to see if Stacks principal is registered
(define-read-only (reverse-lookup (p principal))
  (let ((addr32 (hash p)))
    {
      registered: (is-some (map-get? registry addr32)),
      addr32: addr32
    }))

;; Constants for principal-to-string conversion
(define-constant C32 "0123456789ABCDEFGHJKMNPQRSTVWXYZ")
(define-constant LIST_15 (list 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0))
(define-constant LIST_24 (list 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0))
(define-constant LIST_39 (concat LIST_24 LIST_15))

;; TODO: Replace with `to-ascii?` in Clarity 4
(define-private (principal-to-string (p principal))
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
      (low-part (get s (fold c32-to-string-iter LIST_24 { s: "", r: (buff-to-uint-be (unwrap-panic (as-max-len? (unwrap-panic (slice? data u9 u24)) u16)))})))
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

(define-private (string-ascii-to-buff (s (string-ascii 256)))
  (let ((cb (unwrap-panic (to-consensus-buff? s))))
    (unwrap-panic (slice? cb u5 (len cb)))))

;; @desc Bypass checks, used in unit tests
(define-private (inner-register (p principal))
  (let ((addr32 (hash p)))
    (ok {
      created: (map-insert registry addr32 p),
      addr32: addr32
    })))
