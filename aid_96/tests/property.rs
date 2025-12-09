use aid_96::{class, decode_base32, encode_base32, Aid96};
use proptest::prelude::*;

proptest! {
    #[test]
    fn base32_round_trip_random_bytes(bytes in any::<[u8; 12]>()) {
        let encoded = encode_base32(&bytes);
        let decoded = decode_base32(&encoded).expect("decode should succeed");
        prop_assert_eq!(decoded, bytes);
    }
}

proptest! {
    #[test]
    fn generated_ids_are_unique_within_batch(batch_size in 1usize..1_000usize) {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..batch_size {
            let id = Aid96::new(class::UNSPECIFIED).into_bytes();
            prop_assert!(seen.insert(id));
        }
    }
}
