use mcrs_network::connect::{
    AcceptOutcome, ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC, GLOBAL_HANDSHAKE_CAP, TokenBucket,
    accept_decision,
};

/// Verifies that a bucket with cap 5 allows exactly 5 consecutive accepts in
/// a tight loop (no real elapsed time, so no refill occurs) and then rejects.
#[test]
fn accept_rate_limit() {
    let mut bucket = TokenBucket::new(ACCEPT_BUCKET_CAP);
    for i in 0..ACCEPT_BUCKET_CAP {
        let result = bucket.consume(ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC);
        assert!(result, "expected accept on call {i}, got reject");
    }
    // Next 5 calls must all reject (bucket exhausted, negligible elapsed time)
    for i in 0..5 {
        let result = bucket.consume(ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC);
        assert!(!result, "expected reject on call {} after exhaustion, got accept", ACCEPT_BUCKET_CAP + i);
    }
}

/// Verifies that accept_decision rejects when in-flight count equals the cap,
/// and accepts when it is one below the cap.
#[test]
fn global_handshake_cap() {
    // At exactly the cap: should be rejected
    let mut bucket_full = TokenBucket::new(ACCEPT_BUCKET_CAP);
    assert_eq!(
        accept_decision(&mut bucket_full, GLOBAL_HANDSHAKE_CAP),
        AcceptOutcome::CapExceeded,
        "expected CapExceeded when inflight == GLOBAL_HANDSHAKE_CAP"
    );

    // One below the cap: should be accepted
    let mut bucket_ok = TokenBucket::new(ACCEPT_BUCKET_CAP);
    assert_eq!(
        accept_decision(&mut bucket_ok, GLOBAL_HANDSHAKE_CAP - 1),
        AcceptOutcome::Accept,
        "expected Accept when inflight == GLOBAL_HANDSHAKE_CAP - 1"
    );
}
