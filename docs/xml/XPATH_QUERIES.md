# XPath Query Reference for UCE34 XML Documentation

This guide provides XPath queries for navigating the UCE34 XML documentation suite.

## File Overview

| File | Lines | Purpose |
|------|-------|---------|
| `capsule-connections.xml` | 373 | Connection types (direct, pipeline, broadcast, mesh, request-response) and inter-tier rules |
| `capsule-api-template.xml` | 538 | Standard API specification format for capsule documentation |
| `metacapsule-patterns.xml` | 851 | 4 metacapsule patterns, 4 topologies, 8 lifecycle states, 4 coordination protocols |
| `origin/computational-capsule-philosophy.xml` | 669 | Core philosophy (T0-T10), principles, anti-patterns, UCE33 |
| `origin/atomic-capsule-patterns.xml` | 523 | 16 atomic capsule patterns (ACB-64, APC-512, etc), SWeMR, breaker pattern |
| `origin/key-innovations.xml` | 850 | 10 validated + 17 unexploited innovations, B32 benchmarks |
| `../METACAPSULE_ARCHITECTURE.xml` | 369 | Metacapsule v2.0 architecture summary |

---

## 1. Tier Queries

### Find all tier definitions
```xpath
//tier
//tier-system/tier
```

### Find a specific tier by ID (e.g., T1 Atomic)
```xpath
//tier[@id='T1']
//tier-system/tier[@id='T1']
```

### Find all tiers with speedup claims
```xpath
//tier[speedup]
//tier[@speedup]
```

### Find tiers by category (e.g., Coordination)
```xpath
//tier[@category='Coordination']
//tier[@category='Lockfree Coordination']
```

### Find unexploited tiers
```xpath
//tier[@status='UNEXPLOITED']
```

### Find tiers with proven results
```xpath
//tier[proven-results]
//tier/proven-results/result
```

---

## 2. Capsule Pattern Queries

### Find all named patterns (atomic capsule patterns)
```xpath
//named-patterns/pattern
//pattern[@id]
```

### Find patterns by size
```xpath
//pattern[@size='64 bits']
//pattern[@size='512 bits']
//pattern[@size='1024 bits']
```

### Find patterns with specific purpose
```xpath
//pattern[contains(purpose, 'Risk')]
//pattern[contains(purpose, 'Position')]
//pattern[contains(purpose, 'Circuit')]
```

### Find the Circuit Breaker pattern (ACB-64)
```xpath
//pattern[@id='ACB-64']
//pattern[@name='Circuit Breaker']
```

### Find all patterns with latency specifications
```xpath
//pattern[latency]
//pattern/latency
```

### Find patterns with speedup claims
```xpath
//pattern[speedup]
//pattern[@speedup]
```

---

## 3. Metacapsule Pattern Queries

### Find all metacapsule patterns
```xpath
//pattern-catalog/pattern
```

### Find metacapsule patterns by topology
```xpath
//pattern[@topology='pipeline']
//pattern[@topology='mesh']
//pattern[@topology='fanout']
```

### Find patterns by sub-capsule count range
```xpath
//pattern[sub-capsule-count/@min <= 6 and sub-capsule-count/@max >= 10]
```

### Find canonical examples in metacapsule patterns
```xpath
//pattern/canonical-example
//pattern/canonical-example[@name]
```

### Find all stages in encoder pattern
```xpath
//pattern[@id='encoder']/stages/stage
```

### Find patterns with specific tier composition
```xpath
//pattern[contains(tier, 'T6')]
//pattern[contains(tier, 'T1+T2')]
```

---

## 4. Connection Type Queries

### Find all connection types
```xpath
//connection-types/type
```

### Find connection by ID
```xpath
//type[@id='direct']
//type[@id='pipeline']
//type[@id='broadcast']
//type[@id='mesh']
//type[@id='request-response']
```

### Find connections by latency
```xpath
//type[latency='<10ns']
//type[contains(latency, '50ns')]
```

### Find connection APIs
```xpath
//type[@id='direct']/api/signature
//type/api/signature
```

### Find connection characteristics
```xpath
//type/characteristics
//type[@id='pipeline']/characteristics/thread-safety
```

### Find connections by thread-safety type
```xpath
//type[characteristics/thread-safety='SPSC optimized, MPSC supported']
//type[contains(characteristics/thread-safety, 'MPMC')]
```

---

## 5. Inter-Tier Connection Rules

### Find all inter-tier rules
```xpath
//inter-tier-rules/rule
```

### Find rules from specific tier
```xpath
//rule[@from='T1']
//rule[@from='T2']
```

### Find rules to specific tier
```xpath
//rule[@to='T2']
//rule[@to='T9']
```

### Find rules for specific connection type
```xpath
//rule[connection='Pipeline']
//rule[connection='Direct']
//rule[connection='Request-Response']
```

### Find T6 metacapsule connection rules
```xpath
//rule[@from='T6']
```

---

## 6. Innovation Queries

### Find all validated innovations
```xpath
//validated-innovations/innovation
//innovation[@status='validated']
```

### Find innovations by tier
```xpath
//innovation[@tier='T1']
//innovation[@tier='T2']
//innovation[contains(@tier, 'T2')]
```

### Find innovations with specific speedup
```xpath
//innovation[contains(@speedup, '7x')]
//innovation//speedup[contains(., '19x')]
```

### Find all unexploited innovations
```xpath
//unexploited-innovations/innovation
//innovation[@status='UNEXPLOITED']
```

### Find innovations by category
```xpath
//new-tiers/innovation
//hardware-capabilities/innovation
//tier6-patterns/innovation
//rust-features/innovation
```

### Find proven results in innovations
```xpath
//innovation/proven-results/result
//innovation//result[@speedup]
```

---

## 7. Lifecycle State Queries

### Find all lifecycle states
```xpath
//lifecycle-states/states/state
//lifecycle/states/state
```

### Find state by ID
```xpath
//state[@id='ready']
//state[@id='processing']
//state[@id='error']
```

### Find terminal states
```xpath
//state[@terminal='true']
```

### Find initial states
```xpath
//state[@initial='true']
```

### Find all state transitions
```xpath
//transitions/transition
//lifecycle/transitions/transition
```

### Find transitions from specific state
```xpath
//transition[@from='ready']
//transition[@from='processing']
```

### Find transitions to specific state
```xpath
//transition[@to='error']
//transition[@to='failed']
```

### Find transitions by trigger
```xpath
//transition[@trigger='drain()']
//transition[contains(@trigger, 'error')]
```

---

## 8. API Template Queries

### Find all capsule API definitions
```xpath
//capsule-api
//capsule-api[@name]
```

### Find capsule APIs by tier
```xpath
//capsule-api[@tier='T1']
//capsule-api[@tier='T2']
```

### Find capsule APIs by size
```xpath
//capsule-api[@size='8B']
//capsule-api[@size='64B']
//capsule-api[@size='256B']
```

### Find all method groups
```xpath
//method-group
//method-group[@category='read']
//method-group[@category='write']
//method-group[@category='atomic']
//method-group[@category='batch']
```

### Find methods by thread-safety
```xpath
//method[thread-safety='MPMC']
//method[thread-safety='SWeMR']
```

### Find methods by latency
```xpath
//method[contains(latency, '<10ns')]
//method[contains(latency, '<50ns')]
```

### Find all constructors
```xpath
//constructors/constructor
//constructor[@name='new']
```

### Find const-evaluable constructors
```xpath
//constructor[const-evaluable='Yes']
```

---

## 9. Anti-Pattern Queries

### Find all anti-patterns
```xpath
//anti-patterns/anti-pattern
//anti-pattern[@id]
```

### Find anti-patterns by severity
```xpath
//anti-pattern[@severity='critical']
//anti-pattern[@severity='high']
//anti-pattern[@severity='medium']
```

### Find anti-patterns in philosophy document
```xpath
//computational-capsule-philosophy//anti-pattern
```

### Find anti-patterns in metacapsule patterns
```xpath
//metacapsule-patterns//anti-pattern
```

---

## 10. Performance and Validation Queries

### Find all performance claims
```xpath
//performance
//performance/claim
//claim[@speedup]
```

### Find B32-validated claims
```xpath
//claim[@validation='B32-Validated']
//validation[contains(., 'B32')]
```

### Find performance guidelines
```xpath
//performance-guidelines/guideline
//guideline[@connection]
```

### Find expected latency specs
```xpath
//expected-latency
//guideline/expected-latency
```

### Find throughput metrics
```xpath
//throughput
//guideline/throughput
```

---

## 11. Topology Queries

### Find all topology definitions
```xpath
//topology-definitions/topology
//topologies/topology
```

### Find topology by ID
```xpath
//topology[@id='pipeline']
//topology[@id='mesh']
//topology[@id='fanout']
//topology[@id='hierarchical']
```

### Find topology characteristics
```xpath
//topology/characteristics
//topology[@id='pipeline']/characteristics/latency
```

### Find topology use cases
```xpath
//topology/use-cases/use-case
//topology[@id='mesh']/use-cases
```

---

## 12. Coordination Protocol Queries

### Find all coordination protocols
```xpath
//coordination-protocols/protocol
```

### Find protocol by ID
```xpath
//protocol[@id='sequential']
//protocol[@id='parallel']
//protocol[@id='pipelined']
//protocol[@id='speculative']
```

### Find protocols by complexity
```xpath
//protocol[@complexity='O(1)']
//protocol[contains(@complexity, 'O(n)')]
```

---

## 13. ASSUM Safety Queries

### Find all ASSUM tags
```xpath
//assum-tags
//assum-tags/assume
//assum-tags/verify
```

### Find assumptions by category
```xpath
//assume[@category='MEMORY_ORDERING']
//assume[@category='TOCTOU_PREVENTION']
//assume[@category='ALIGNMENT']
//assume[@category='PANIC_SAFETY']
```

### Find verified assumptions
```xpath
//verify[@references]
```

### Find ASSUM coverage statistics
```xpath
//assum-coverage/statistic
//assum-safety/assumption
```

---

## 14. Compound Queries (Advanced)

### Find all capsules with both latency and speedup
```xpath
//pattern[latency and speedup]
//innovation[.//latency and .//speedup]
```

### Find T1 atomic capsules with sub-10ns latency
```xpath
//tier[@id='T1']//result[@latency[contains(., 'ns')]]
//proven-results/result[contains(@latency, '9.8ns')]
```

### Find metacapsule patterns with 10+ sub-capsules
```xpath
//pattern[canonical-example/sub-capsules/@count >= 10]
```

### Find all connection types with MPMC thread safety
```xpath
//type[characteristics/thread-safety[contains(., 'MPMC')]]
```

### Find innovations requiring specific hardware
```xpath
//innovation[hardware]
//innovation[contains(hardware, 'CUDA')]
//innovation[contains(hardware, 'AVX')]
```

---

## 15. Cross-Document Queries

These queries work across multiple documents when loaded together:

### Find all T2 SIMD-related content
```xpath
//tier[@id='T2'] | //innovation[@tier='T2'] | //capsule[@tier='T2']
```

### Find all lockfree coordination patterns
```xpath
//*[contains(., 'lockfree')] | //*[contains(., 'Lockfree')]
```

### Find all generation counter usage
```xpath
//*[contains(., 'generation counter')] | //*[contains(., 'Generation counter')]
```

### Find all DualAtomicU64 patterns
```xpath
//*[contains(., 'DualAtomicU64')]
```

---

## Usage Examples

### Using xmllint
```bash
# Find all tier definitions
xmllint --xpath "//tier" origin/computational-capsule-philosophy.xml

# Find connection types
xmllint --xpath "//connection-types/type/@id" capsule-connections.xml

# Find all innovations with speedup
xmllint --xpath "//innovation[@speedup]/@name" origin/key-innovations.xml
```

### Using Python lxml
```python
from lxml import etree

tree = etree.parse('capsule-connections.xml')
connections = tree.xpath('//connection-types/type')
for conn in connections:
    print(f"ID: {conn.get('id')}, Latency: {conn.findtext('latency')}")
```

### Using Rust quick-xml (with xpath crate)
```rust
use quick_xml::Reader;
use xpath::Value;

// Load and query XML...
```

---

## Quick Reference Card

| Query Purpose | XPath Pattern |
|--------------|---------------|
| All tiers | `//tier` |
| Tier by ID | `//tier[@id='T1']` |
| All patterns | `//pattern` |
| Pattern by name | `//pattern[@name='Circuit Breaker']` |
| All connections | `//connection-types/type` |
| Connection by ID | `//type[@id='direct']` |
| All innovations | `//innovation` |
| Validated innovations | `//innovation[@status='validated']` |
| All lifecycle states | `//state` |
| State transitions | `//transition` |
| All anti-patterns | `//anti-pattern` |
| Methods by safety | `//method[thread-safety='MPMC']` |
| Performance claims | `//claim[@speedup]` |

---

## Document Namespaces

Most documents use the UCE34 namespace:
```xml
xmlns="http://kindly.ai/uce34/v6.0"
```

For namespace-aware queries:
```xpath
//*[local-name()='tier']
```

Or register the namespace:
```xpath
//uce:tier
```
(where `uce` is bound to `http://kindly.ai/uce34/v6.0`)
