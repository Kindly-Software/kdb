# Changelog

All notable changes to kindly-debugger will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2024-12-05

### Added

- Initial public release of kindly-debugger
- **Debugger Tools**
  - `debugger/attach` - Attach to running processes
  - `debugger/set_breakpoint` - Set breakpoints at memory addresses
  - `debugger/continue` - Continue execution
  - `debugger/step_forward` - Single-step forward execution
  - `debugger/step_backward` - Time-travel backward stepping
  - `debugger/get_stack_trace` - Retrieve call stacks
  - `debugger/get_variables` - Inspect variable values
  - `debugger/find_similar_bugs` - Bug pattern detection
  - `debugger/export_trace` - Export execution traces
- **Admin Tools**
  - `debugger/quota_status` - Check API usage
  - `debugger/license_info` - View license details
  - `debugger/get_comprehensive_audit` - Retrieve audit trails
- **Document Tools**
  - `xpath_query` - Query XML documents
  - `validate_schema` - Validate against XSD schemas
  - `cache_stats` - View cache statistics
  - `preload_documents` - Preload documents into cache
- MCP 2024-11-05 protocol support
- API key authentication
- Rate limiting by plan tier
- Cryptographic audit trails for compliance

### Security

- All communications over HTTPS
- API keys with configurable permissions
- IP allowlisting (Enterprise)
- Audit logging for all operations

---

For more information, visit [kindly.services](https://kindly.services)
