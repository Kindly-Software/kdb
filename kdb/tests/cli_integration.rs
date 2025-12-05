//! CLI Integration Tests (T28 Framework)
//!
//! Tests for kdb CLI: command parsing, dispatching, audit trail, and full REPL sessions.

use kdb::cli::{AuditLogCapsule, Command, CommandDispatcherCapsule, REPLCapsule};

// ============================================================================
// T28-Q1-Q7: Unit Tests (Command Parsing)
// ============================================================================

#[test]
fn test_command_parse_attach() {
    let cmd = Command::parse("attach 12345");
    assert_eq!(cmd, Command::Attach(12345));
}

#[test]
fn test_command_parse_attach_invalid_pid() {
    let cmd = Command::parse("attach invalid");
    assert!(matches!(cmd, Command::Invalid(_)));
}

#[test]
fn test_command_parse_break() {
    let cmd = Command::parse("break main");
    assert_eq!(cmd, Command::Break("main".to_string()));

    let cmd = Command::parse("break 0x401234");
    assert_eq!(cmd, Command::Break("0x401234".to_string()));
}

#[test]
fn test_command_parse_break_missing_arg() {
    let cmd = Command::parse("break");
    assert!(matches!(cmd, Command::Invalid(_)));
}

#[test]
fn test_command_parse_continue() {
    assert_eq!(Command::parse("continue"), Command::Continue);
    assert_eq!(Command::parse("c"), Command::Continue);
}

#[test]
fn test_command_parse_step() {
    assert_eq!(Command::parse("step"), Command::Step);
    assert_eq!(Command::parse("s"), Command::Step);
}

#[test]
fn test_command_parse_back() {
    assert_eq!(Command::parse("back"), Command::Back);
}

#[test]
fn test_command_parse_snapshot() {
    assert_eq!(Command::parse("snapshot"), Command::Snapshot);
    assert_eq!(Command::parse("snap"), Command::Snapshot);
}

#[test]
fn test_command_parse_stack() {
    assert_eq!(Command::parse("stack"), Command::Stack);
    assert_eq!(Command::parse("bt"), Command::Stack);
}

#[test]
fn test_command_parse_quit() {
    assert_eq!(Command::parse("quit"), Command::Quit);
    assert_eq!(Command::parse("q"), Command::Quit);
    assert_eq!(Command::parse("exit"), Command::Quit);
}

#[test]
fn test_command_parse_help() {
    assert_eq!(Command::parse("help"), Command::Help(None));
    assert_eq!(Command::parse("help attach"), Command::Help(Some("attach".to_string())));
}

#[test]
fn test_command_parse_empty() {
    assert!(matches!(Command::parse(""), Command::Invalid(_)));
}

#[test]
fn test_command_parse_whitespace_only() {
    assert!(matches!(Command::parse("   "), Command::Invalid(_)));
}

#[test]
fn test_command_parse_unknown() {
    assert!(matches!(Command::parse("badcommand"), Command::Invalid(_)));
}

// ============================================================================
// T28-Q8-Q14: Property Tests (Command Parsing Properties)
// ============================================================================

#[test]
fn test_attach_command_always_requires_pid() {
    let inputs = vec!["attach", "attach  ", "attach notanumber"];
    for input in inputs {
        let cmd = Command::parse(input);
        // Should be either valid Attach with u32, or Invalid
        match cmd {
            Command::Attach(_) => {} // Valid
            Command::Invalid(_) => {}  // Also valid (parse error)
            _ => panic!("Unexpected command type for input: {}", input),
        }
    }
}

#[test]
fn test_commands_case_insensitive() {
    // Lowercase
    assert_eq!(Command::parse("continue"), Command::Continue);
    // Mixed case should still work (we do .to_lowercase())
    assert_eq!(Command::parse("CONTINUE"), Command::Continue);
    assert_eq!(Command::parse("CoNtInUe"), Command::Continue);
}

#[test]
fn test_shorthand_aliases() {
    // All shorthands should map to full commands
    assert_eq!(Command::parse("c"), Command::Continue);
    assert_eq!(Command::parse("s"), Command::Step);
    assert_eq!(Command::parse("q"), Command::Quit);
    assert_eq!(Command::parse("bt"), Command::Stack);
    // Note: "b" alone requires an argument, so it will fail parsing
    assert!(matches!(Command::parse("b"), Command::Invalid(_)));
    // But "b 0x401234" should work
    assert_eq!(Command::parse("b 0x401234"), Command::Break("0x401234".to_string()));
    assert_eq!(Command::parse("snap"), Command::Snapshot);
}

#[test]
fn test_help_command_variations() {
    assert_eq!(Command::parse("help"), Command::Help(None));
    assert_eq!(Command::parse("h"), Command::Help(None));
    assert_eq!(Command::parse("?"), Command::Help(None));
}

// ============================================================================
// T28-Q15-Q21: Integration Tests (Dispatcher & Audit)
// ============================================================================

#[test]
fn test_dispatcher_attach_and_detach() {
    let mut dispatcher = CommandDispatcherCapsule::new();
    assert_eq!(dispatcher.attached_pid(), None);

    let result = dispatcher.dispatch(&Command::Attach(12345));
    assert!(result.is_ok());
    assert_eq!(dispatcher.attached_pid(), Some(12345));

    dispatcher.detach();
    assert_eq!(dispatcher.attached_pid(), None);
}

#[test]
fn test_dispatcher_not_attached_error() {
    let mut dispatcher = CommandDispatcherCapsule::new();

    let result = dispatcher.dispatch(&Command::Stack);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Not attached"));
}

#[test]
fn test_dispatcher_continue_when_attached() {
    let mut dispatcher = CommandDispatcherCapsule::new();
    let _ = dispatcher.dispatch(&Command::Attach(12345));

    let result = dispatcher.dispatch(&Command::Continue);
    assert!(result.is_ok());
    assert!(result.unwrap().contains("Continued"));
}

#[test]
fn test_audit_log_single_entry() {
    let mut audit = AuditLogCapsule::new();
    let hash = audit.log_command("attach 12345");

    assert_ne!(hash, 0);
    assert_eq!(audit.entries().len(), 1);
    assert_eq!(audit.root_hash(), hash);
}

#[test]
fn test_audit_log_chain_verification() {
    let mut audit = AuditLogCapsule::new();
    audit.log_command("attach 12345");
    audit.log_command("break main");
    audit.log_command("continue");

    assert!(audit.verify_chain());
    assert!(audit.verify_recent());
}

#[test]
fn test_audit_log_tamper_detection() {
    let mut audit = AuditLogCapsule::new();
    audit.log_command("cmd1");
    audit.log_command("cmd2");

    // Verify chain is valid before tampering
    assert!(audit.verify_chain());

    // Simulate tampering
    if let Some(entry) = audit.entries_mut().front_mut() {
        entry.hash ^= 1; // Flip one bit
    }

    // Chain should now be invalid
    assert!(!audit.verify_chain());
}

#[test]
fn test_audit_log_export_json() {
    let mut audit = AuditLogCapsule::new();
    audit.log_command("attach 12345");
    audit.log_command("break main");

    let json = audit.export_json();
    assert!(json.contains("\"audit_trail\""));
    assert!(json.contains("\"attach 12345\""));
    assert!(json.contains("\"break main\""));
    assert!(json.contains("\"chain_valid\": true"));
}

#[test]
fn test_repl_capsule_creation() {
    let repl = REPLCapsule::new();
    assert_eq!(repl.command_count(), 0);
    assert!(!repl.should_exit());
    assert_eq!(repl.dispatcher().attached_pid(), None);
    assert_eq!(repl.audit_log().entries().len(), 0);
}

#[test]
fn test_repl_audit_integration() {
    let mut repl = REPLCapsule::new();
    let _ = repl.dispatcher_mut().dispatch(&Command::Attach(12345));

    // Audit log should not have entries yet (manual logging required)
    assert_eq!(repl.audit_log().entries().len(), 0);

    // Log a command via audit capsule
    repl.audit_log_mut().log_command("attach 12345");
    assert_eq!(repl.audit_log().entries().len(), 1);
    assert!(repl.audit_log().verify_chain());
}

// ============================================================================
// T28-Q22-Q28: Production Tests (Full Workflows)
// ============================================================================

#[test]
fn test_workflow_attach_and_break() {
    let mut repl = REPLCapsule::new();
    let mut audit = AuditLogCapsule::new();

    // Attach
    let cmd = Command::parse("attach 12345");
    let result = repl.dispatcher_mut().dispatch(&cmd);
    assert!(result.is_ok());
    audit.log_command("attach 12345");

    // Break
    let cmd = Command::parse("break main");
    let result = repl.dispatcher_mut().dispatch(&cmd);
    assert!(result.is_ok());
    audit.log_command("break main");

    // Verify audit trail
    assert!(audit.verify_chain());
    assert_eq!(audit.entries().len(), 2);
}

#[test]
fn test_workflow_full_debugging_session() {
    let mut repl = REPLCapsule::new();
    let mut audit = AuditLogCapsule::new();

    let commands = vec!["attach 12345", "break main", "continue", "stack", "quit"];

    for cmd_str in commands {
        let cmd = Command::parse(cmd_str);
        let result = repl.dispatcher_mut().dispatch(&cmd);

        // Most commands should succeed (except quit which exits)
        if !cmd_str.contains("quit") {
            assert!(result.is_ok(), "Failed to execute: {}", cmd_str);
        }

        audit.log_command(cmd_str);
    }

    // Verify audit trail integrity
    assert!(audit.verify_chain());
    assert_eq!(audit.entries().len(), 5);
}

#[test]
fn test_help_command() {
    let mut dispatcher = CommandDispatcherCapsule::new();

    let result = dispatcher.dispatch(&Command::Help(None));
    assert!(result.is_ok());
    let help_text = result.unwrap();
    assert!(help_text.contains("Commands:"));
    assert!(help_text.contains("attach"));
}

#[test]
fn test_help_with_topic() {
    let mut dispatcher = CommandDispatcherCapsule::new();

    let result = dispatcher.dispatch(&Command::Help(Some("attach".to_string())));
    assert!(result.is_ok());
    let help_text = result.unwrap();
    assert!(help_text.contains("attach"));
    assert!(help_text.contains("<pid>") || help_text.contains("12345"));
}

#[test]
fn test_audit_log_wraparound() {
    let mut audit = AuditLogCapsule::new();

    // Fill beyond capacity (1,024)
    for i in 0..1100 {
        audit.log_command(&format!("cmd{}", i));
    }

    // Should have exactly 1,024 entries (ring buffer)
    assert_eq!(audit.entries().len(), 1024);

    // Chain should still verify correctly
    assert!(audit.verify_chain());
}

#[test]
fn test_invalid_command_handling() {
    let mut dispatcher = CommandDispatcherCapsule::new();

    let result = dispatcher.dispatch(&Command::Invalid("bad input".to_string()));
    assert!(result.is_err());
}

#[test]
fn test_multiple_commands_sequence() {
    let mut dispatcher = CommandDispatcherCapsule::new();
    let mut audit = AuditLogCapsule::new();

    let commands = vec![
        Command::Attach(12345),
        Command::Break("0x401234".to_string()),
        Command::Step,
        Command::Step,
        Command::Back,
        Command::Stack,
    ];

    for cmd in commands {
        let result = dispatcher.dispatch(&cmd);
        if dispatcher.attached_pid().is_some() {
            assert!(result.is_ok());
        }
        audit.log_command(&format!("{:?}", cmd));
    }

    // All commands logged
    assert!(audit.entries().len() > 0);
    assert!(audit.verify_chain());
}

#[test]
fn test_q34_compliance_hash_chain() {
    let mut audit = AuditLogCapsule::new();

    // Log commands
    audit.log_command("step 1");
    audit.log_command("step 2");
    audit.log_command("step 3");

    // Verify Q34 compliance properties
    let entries = audit.entries();

    // 1. Sequential IDs
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry.id, i as u64);
    }

    // 2. Timestamps are monotonic (or equal)
    for i in 1..entries.len() {
        assert!(entries[i].timestamp >= entries[i - 1].timestamp);
    }

    // 3. Hash chain is valid
    assert!(audit.verify_chain());

    // 4. Root hash differs from individual hashes
    let root = audit.root_hash();
    for entry in entries.iter() {
        if entry.id < entries.len() as u64 - 1 {
            assert_ne!(root, entry.hash);
        }
    }
}

#[test]
fn test_command_help_text() {
    let attach_cmd = Command::Attach(12345);
    let help = attach_cmd.help_text();
    assert!(help.contains("attach"));

    let break_cmd = Command::Break("main".to_string());
    let help = break_cmd.help_text();
    assert!(help.contains("break"));

    let general_help = Command::general_help();
    assert!(general_help.contains("Commands:"));
}

// ============================================================================
// Additional Safety & Edge Cases
// ============================================================================

#[test]
fn test_command_parse_multi_word_symbol() {
    let cmd = Command::parse("break my_symbol_name");
    match cmd {
        Command::Break(sym) => assert_eq!(sym, "my_symbol_name"),
        _ => panic!("Expected Break command"),
    }
}

#[test]
fn test_audit_log_empty_command() {
    let mut audit = AuditLogCapsule::new();
    audit.log_command("");
    assert_eq!(audit.entries().len(), 1);
    assert!(audit.verify_chain());
}

#[test]
fn test_dispatcher_reattach() {
    let mut dispatcher = CommandDispatcherCapsule::new();

    // First attach
    let _ = dispatcher.dispatch(&Command::Attach(111));
    assert_eq!(dispatcher.attached_pid(), Some(111));

    // Try to attach to different PID
    let result = dispatcher.dispatch(&Command::Attach(222));
    assert!(result.is_err()); // Should fail: already attached

    // Reattach to same PID
    let result = dispatcher.dispatch(&Command::Attach(111));
    assert!(result.is_ok()); // Should succeed: same PID
}

#[test]
fn test_audit_entry_structure() {
    let mut audit = AuditLogCapsule::new();
    audit.log_command("test command");

    let entries = audit.entries();
    assert_eq!(entries.len(), 1);

    let entry = &entries[0];
    assert_eq!(entry.id, 0);
    assert_eq!(entry.command, "test command");
    assert_ne!(entry.hash, 0);
    assert_eq!(entry.prev_hash, 0); // First entry
}
