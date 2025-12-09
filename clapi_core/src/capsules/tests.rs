//! Capsule module integration tests
//!
//! Basic smoke tests ensuring all capsules compile and basic operations work

#[cfg(test)]
mod capsule_integration_tests {
    use super::super::*;

    #[test]
    fn test_all_capsules_compile() {
        // REQ-128
        let req = RequestCapsule128::new(1000_00);
        assert_eq!(req.budget(), 1000_00);

        // RTE-128
        let rte = RoutingCapsule128::new();
        assert_eq!(rte.available_providers(), 8);

        // RES-256
        let res = ResponseCapsule256::new();
        assert_eq!(res.cost(), 0);

        // ALE-128
        let ale = AuditLogEntry128::new(1, 100, 10_00, 0, 0);
        assert_eq!(ale.entry_id(), 1);

        // ET-1KB
        let et = EpochTile1024::new(1);
        assert_eq!(et.epoch_id(), 1);
    }

    #[test]
    fn test_capsule_sizes() {
        assert_eq!(std::mem::size_of::<RequestCapsule128>(), 128);
        assert_eq!(std::mem::size_of::<RoutingCapsule128>(), 128);
        assert_eq!(std::mem::size_of::<ResponseCapsule256>(), 256);
        assert_eq!(std::mem::size_of::<AuditLogEntry128>(), 128);
        assert_eq!(std::mem::size_of::<EpochTile1024>(), 1024);
    }

    #[test]
    fn test_capsule_alignments() {
        assert_eq!(std::mem::align_of::<RequestCapsule128>(), 128);
        assert_eq!(std::mem::align_of::<RoutingCapsule128>(), 128);
        assert_eq!(std::mem::align_of::<ResponseCapsule256>(), 256);
        assert_eq!(std::mem::align_of::<AuditLogEntry128>(), 128);
        assert_eq!(std::mem::align_of::<EpochTile1024>(), 1024);
    }
}
