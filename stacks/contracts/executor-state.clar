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
      (p-as-string (contract-call?
        'SP1E0XBN9T4B10E9QMR7XMFJPMA19D77WY3KP2QKC.self-listing-helper-v3
        principal-to-string stacks-addr
      ))
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
