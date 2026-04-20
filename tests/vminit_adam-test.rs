use bund_blobstore::BUND;
use bund_blobstore::vm::init_adam;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_adam_idempotency() {
        // First call: Should initialize successfully
        let result1 = init_adam();
        assert!(result1.is_ok(), "First initialization should succeed");

        // Second call: Should return Ok(()) and log that it's already initialized
        let result2 = init_adam();
        assert!(
            result2.is_ok(),
            "Second initialization should also succeed (no-op)"
        );

        // Verify the global state is actually set
        assert!(BUND.get().is_some());
    }
}
