//! Library-level integration tests

#[cfg(test)]
mod lib_tests {
    use crate::*;

    #[test]
    fn test_error_types_compile() {
        let err = ClapiError::BudgetExhausted {
            requested: 100,
            available: 50,
        };
        assert!(err.to_string().contains("Budget exhausted"));
    }

    #[test]
    fn test_all_exports() {
        // Ensure all capsules are exported
        let _req = RequestCapsule128::new(1000);
        let _rte = RoutingCapsule128::new();
        let _res = ResponseCapsule256::new();
        let _ale = AuditLogEntry128::new(1, 100, 10, 0, 0);
        let _et = EpochTile1024::new(1);
    }
}
