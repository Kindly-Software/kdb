//! Common test utilities for integration tests
//! Provides stack size configuration for large DebuggerCapsule allocation

/// Creates a DebuggerCapsule on the heap using a larger stack
/// This works around the stack overflow issue when creating 1.09MB structs
pub fn create_debugger(pid: u64) -> Box<kdb::DebuggerCapsule> {
    // Create on heap by moving through thread spawn
    // This avoids stack allocation entirely
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .stack_size(4 * 1024 * 1024)  // 4MB stack
        .spawn(move || {
            let debugger = std::boxed::Box::new(kdb::DebuggerCapsule::new(pid));
            tx.send(debugger).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
    
    rx.recv().unwrap()
}
