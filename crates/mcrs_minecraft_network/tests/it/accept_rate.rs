use mcrs_minecraft_network::connect::{ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC, TokenBucket};

#[test]
fn accept_rate_limit() {
    let mut bucket = TokenBucket::new(ACCEPT_BUCKET_CAP);
    for i in 0..ACCEPT_BUCKET_CAP {
        let result = bucket.consume(ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC);
        assert!(result, "expected accept on call {i}, got reject");
    }
    for i in 0..5 {
        let result = bucket.consume(ACCEPT_BUCKET_CAP, ACCEPT_REFILL_PER_SEC);
        assert!(
            !result,
            "expected reject on call {} after exhaustion, got accept",
            ACCEPT_BUCKET_CAP + i
        );
    }
}
