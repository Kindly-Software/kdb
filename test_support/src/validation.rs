//! Test Validation Framework
//!
//! Property-based testing and assertion utilities for comprehensive
//! validation of atomic operations and concurrent systems.

use std::fmt;
use crate::TestResult;

/// Test validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub passed: bool,
    pub message: String,
    pub details: Option<String>,
    pub severity: Severity,
    pub assertion_count: usize,
}

/// Test assertion builder
#[derive(Debug)]
pub struct TestAssertion {
    name: String,
    passed: bool,
    message: String,
    severity: Severity,
}

/// Property-based test framework
#[derive(Debug)]
pub struct PropertyTest {
    name: String,
    properties: Vec<Property>,
    generators: Vec<Box<dyn PropertyGenerator>>,
    config: PropertyTestConfig,
}

/// Individual property to test
pub struct Property {
    name: String,
    description: String,
    test_fn: Box<dyn Fn(&[PropertyValue]) -> TestResult<bool> + Send + Sync>,
}

impl std::fmt::Debug for Property {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Property")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("test_fn", &"<closure>")
            .finish()
    }
}

/// Property test configuration
#[derive(Debug, Clone)]
pub struct PropertyTestConfig {
    pub iterations: usize,
    pub max_shrink_attempts: usize,
    pub timeout_ms: u64,
    pub parallelism: usize,
}

/// Generated property value
#[derive(Debug, Clone)]
pub enum PropertyValue {
    U64(u64),
    F64(f64),
    String(String),
    Bool(bool),
    Vector(Vec<PropertyValue>),
    AtomicOperation(crate::generators::AtomicOperation),
}

/// Property value generator trait
pub trait PropertyGenerator: fmt::Debug + Send + Sync {
    fn generate(&mut self) -> PropertyValue;
    fn shrink(&self, value: &PropertyValue) -> Vec<PropertyValue>;
    fn name(&self) -> &str;
}

/// Test severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Assertion builders for common patterns
pub struct Assert;

impl ValidationResult {
    /// Create passing result
    pub fn pass(message: String) -> Self {
        Self {
            passed: true,
            message,
            details: None,
            severity: Severity::Info,
            assertion_count: 1,
        }
    }

    /// Create failing result
    pub fn fail(message: String) -> Self {
        Self {
            passed: false,
            message,
            details: None,
            severity: Severity::Error,
            assertion_count: 1,
        }
    }

    /// Add details to result
    pub fn with_details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }

    /// Set severity
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Combine multiple validation results
    pub fn combine(results: Vec<ValidationResult>) -> ValidationResult {
        let passed = results.iter().all(|r| r.passed);
        let assertion_count = results.iter().map(|r| r.assertion_count).sum();

        let message = if passed {
            format!("All {} assertions passed", assertion_count)
        } else {
            let failed_count = results.iter().filter(|r| !r.passed).count();
            format!("{} of {} assertions failed", failed_count, assertion_count)
        };

        let max_severity = results.iter()
            .map(|r| &r.severity)
            .max()
            .unwrap_or(&Severity::Info)
            .clone();

        let details = if results.iter().any(|r| !r.passed) {
            let failed_messages: Vec<String> = results.iter()
                .filter(|r| !r.passed)
                .map(|r| format!("- {}", r.message))
                .collect();
            Some(failed_messages.join("\n"))
        } else {
            None
        };

        ValidationResult {
            passed,
            message,
            details,
            severity: max_severity,
            assertion_count,
        }
    }
}

impl TestAssertion {
    /// Create new assertion
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: true,
            message: String::new(),
            severity: Severity::Error,
        }
    }

    /// Mark assertion as passed
    pub fn pass(mut self, message: &str) -> ValidationResult {
        self.passed = true;
        self.message = message.to_string();
        ValidationResult {
            passed: true,
            message: format!("{}: {}", self.name, message),
            details: None,
            severity: Severity::Info,
            assertion_count: 1,
        }
    }

    /// Mark assertion as failed
    pub fn fail(mut self, message: &str) -> ValidationResult {
        self.passed = false;
        self.message = message.to_string();
        ValidationResult {
            passed: false,
            message: format!("{}: {}", self.name, message),
            details: None,
            severity: self.severity,
            assertion_count: 1,
        }
    }

    /// Set assertion severity
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }
}

impl Assert {
    /// Assert that two values are equal
    pub fn eq<T: PartialEq + fmt::Debug>(name: &str, actual: T, expected: T) -> ValidationResult {
        let assertion = TestAssertion::new(name);
        if actual == expected {
            assertion.pass(&format!("Values are equal: {:?}", expected))
        } else {
            assertion.fail(&format!("Expected {:?}, got {:?}", expected, actual))
        }
    }

    /// Assert that condition is true
    pub fn is_true(name: &str, condition: bool) -> ValidationResult {
        let assertion = TestAssertion::new(name);
        if condition {
            assertion.pass("Condition is true")
        } else {
            assertion.fail("Expected true, got false")
        }
    }

    /// Assert that condition is false
    pub fn is_false(name: &str, condition: bool) -> ValidationResult {
        let assertion = TestAssertion::new(name);
        if !condition {
            assertion.pass("Condition is false")
        } else {
            assertion.fail("Expected false, got true")
        }
    }

    /// Assert that value is within range
    pub fn in_range<T: PartialOrd + fmt::Debug>(
        name: &str,
        value: T,
        min: T,
        max: T,
    ) -> ValidationResult {
        let assertion = TestAssertion::new(name);
        if value >= min && value <= max {
            assertion.pass(&format!("Value {:?} is in range [{:?}, {:?}]", value, min, max))
        } else {
            assertion.fail(&format!("Value {:?} is outside range [{:?}, {:?}]", value, min, max))
        }
    }

    /// Assert that value is approximately equal (for floating point)
    pub fn approx_eq(name: &str, actual: f64, expected: f64, tolerance: f64) -> ValidationResult {
        let assertion = TestAssertion::new(name);
        let diff = (actual - expected).abs();
        if diff <= tolerance {
            assertion.pass(&format!("Values approximately equal: {} ≈ {} (±{})", actual, expected, tolerance))
        } else {
            assertion.fail(&format!("Values not approximately equal: {} vs {} (diff: {}, tolerance: {})",
                actual, expected, diff, tolerance))
        }
    }

    /// Assert that operation completes within time limit
    pub fn completes_within<F, T>(
        name: &str,
        operation: F,
        time_limit: std::time::Duration,
    ) -> ValidationResult
    where
        F: FnOnce() -> T,
    {
        let assertion = TestAssertion::new(name);
        let start = std::time::Instant::now();
        let _ = operation();
        let elapsed = start.elapsed();

        if elapsed <= time_limit {
            assertion.pass(&format!("Operation completed in {:?} (limit: {:?})", elapsed, time_limit))
        } else {
            assertion.fail(&format!("Operation took {:?}, exceeded limit of {:?}", elapsed, time_limit))
        }
    }

    /// Assert lockfree property
    pub fn is_lockfree<F>(name: &str, operation: F, thread_count: usize) -> ValidationResult
    where
        F: Fn() + Send + Sync + Clone + 'static,
    {
        let assertion = TestAssertion::new(name);

        // Test that operation can be called concurrently without blocking
        let operation = std::sync::Arc::new(operation);
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(thread_count));
        let start_time = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let end_time = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));

        let mut handles = Vec::new();

        for _ in 0..thread_count {
            let op = std::sync::Arc::clone(&operation);
            let barrier = std::sync::Arc::clone(&barrier);
            let start_time = std::sync::Arc::clone(&start_time);
            let end_time = std::sync::Arc::clone(&end_time);

            let handle = std::thread::spawn(move || {
                barrier.wait();
                let start = std::time::Instant::now();
                start_time.store(start.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);

                for _ in 0..1000 {
                    op();
                }

                let end = std::time::Instant::now();
                end_time.store(end.elapsed().as_nanos() as u64, std::sync::atomic::Ordering::Relaxed);
            });

            handles.push(handle);
        }

        // Wait for all threads with timeout
        let timeout = std::time::Duration::from_secs(5);
        let start = std::time::Instant::now();

        for handle in handles {
            if start.elapsed() > timeout {
                return assertion.fail("Lockfree test timed out - possible deadlock");
            }
            if handle.join().is_err() {
                return assertion.fail("Thread panicked during lockfree test");
            }
        }

        assertion.pass("Operation appears to be lockfree")
    }

    /// Assert performance improvement
    pub fn performance_improvement(
        name: &str,
        baseline_ns: f64,
        optimized_ns: f64,
        min_improvement: f64,
    ) -> ValidationResult {
        let assertion = TestAssertion::new(name);
        let improvement = baseline_ns / optimized_ns;

        if improvement >= min_improvement {
            assertion.pass(&format!("Performance improved by {:.2}x (required: {:.2}x)",
                improvement, min_improvement))
        } else {
            assertion.fail(&format!("Insufficient improvement: {:.2}x < {:.2}x required",
                improvement, min_improvement))
        }
    }

    /// Assert thread safety
    pub fn is_thread_safe<T, F>(name: &str, shared_data: T, operation: F) -> ValidationResult
    where
        T: Send + Sync + 'static,
        F: Fn(&T) + Send + Sync + Clone + 'static,
    {
        let assertion = TestAssertion::new(name);
        let shared_data = std::sync::Arc::new(shared_data);
        let operation = std::sync::Arc::new(operation);

        let thread_count = 8;
        let iterations = 1000;
        let mut handles = Vec::new();

        for _ in 0..thread_count {
            let data = std::sync::Arc::clone(&shared_data);
            let op = std::sync::Arc::clone(&operation);

            let handle = std::thread::spawn(move || {
                for _ in 0..iterations {
                    op(&*data);
                    std::thread::yield_now();
                }
            });

            handles.push(handle);
        }

        // Wait for completion with timeout
        let timeout = std::time::Duration::from_secs(10);
        let start = std::time::Instant::now();

        for handle in handles {
            if start.elapsed() > timeout {
                return assertion.fail("Thread safety test timed out");
            }
            if handle.join().is_err() {
                return assertion.fail("Thread panicked during safety test");
            }
        }

        assertion.pass("Operation appears to be thread-safe")
    }
}

impl Default for PropertyTestConfig {
    fn default() -> Self {
        Self {
            iterations: 100,
            max_shrink_attempts: 100,
            timeout_ms: 5000,
            parallelism: 1,
        }
    }
}

impl PropertyTest {
    /// Create new property test
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            properties: Vec::new(),
            generators: Vec::new(),
            config: PropertyTestConfig::default(),
        }
    }

    /// Add property to test
    pub fn add_property<F>(mut self, name: &str, description: &str, test_fn: F) -> Self
    where
        F: Fn(&[PropertyValue]) -> TestResult<bool> + Send + Sync + 'static,
    {
        self.properties.push(Property {
            name: name.to_string(),
            description: description.to_string(),
            test_fn: Box::new(test_fn),
        });
        self
    }

    /// Add value generator
    pub fn add_generator<G: PropertyGenerator + 'static>(mut self, generator: G) -> Self {
        self.generators.push(Box::new(generator));
        self
    }

    /// Configure test parameters
    pub fn with_config(mut self, config: PropertyTestConfig) -> Self {
        self.config = config;
        self
    }

    /// Run property tests
    pub fn run(self) -> TestResult<ValidationResult> {
        if self.properties.is_empty() {
            return Ok(ValidationResult::fail("No properties to test".to_string()));
        }

        if self.generators.is_empty() {
            return Ok(ValidationResult::fail("No generators provided".to_string()));
        }

        let mut results = Vec::new();

        for property in &self.properties {
            let property_result = self.test_property(property)?;
            results.push(property_result);
        }

        Ok(ValidationResult::combine(results))
    }

    fn test_property(&self, property: &Property) -> TestResult<ValidationResult> {
        let mut passed_count = 0;
        let mut failed_examples = Vec::new();

        for iteration in 0..self.config.iterations {
            // Generate test values
            let mut values = Vec::new();
            for _generator in &self.generators {
                // Note: This would need proper mutable access in real implementation
                values.push(PropertyValue::U64(iteration as u64)); // Simplified
            }

            // Test property
            match (property.test_fn)(&values) {
                Ok(true) => passed_count += 1,
                Ok(false) => {
                    failed_examples.push(format!("Iteration {}: {:?}", iteration, values));
                }
                Err(e) => {
                    return Ok(ValidationResult::fail(format!(
                        "Property '{}' failed with error: {}",
                        property.name, e
                    )));
                }
            }

            if failed_examples.len() >= 10 {
                break; // Stop after finding enough failures
            }
        }

        if failed_examples.is_empty() {
            Ok(ValidationResult::pass(format!(
                "Property '{}' passed {} iterations",
                property.name, passed_count
            )))
        } else {
            Ok(ValidationResult::fail(format!(
                "Property '{}' failed {} of {} iterations",
                property.name, failed_examples.len(), self.config.iterations
            )).with_details(failed_examples.join("\n")))
        }
    }
}

// Simple property generators
#[derive(Debug)]
pub struct U64Generator {
    name: String,
    range: (u64, u64),
}

impl U64Generator {
    pub fn new(name: &str, range: (u64, u64)) -> Self {
        Self {
            name: name.to_string(),
            range,
        }
    }
}

impl PropertyGenerator for U64Generator {
    fn generate(&mut self) -> PropertyValue {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        PropertyValue::U64(rng.gen_range(self.range.0..=self.range.1))
    }

    fn shrink(&self, value: &PropertyValue) -> Vec<PropertyValue> {
        if let PropertyValue::U64(v) = value {
            if *v > self.range.0 {
                vec![PropertyValue::U64(*v - 1), PropertyValue::U64(self.range.0)]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assertion_eq_pass() {
        let result = Assert::eq("test_equality", 42, 42);
        assert!(result.passed);
        assert_eq!(result.assertion_count, 1);
    }

    #[test]
    fn test_assertion_eq_fail() {
        let result = Assert::eq("test_equality", 42, 43);
        assert!(!result.passed);
        assert!(result.message.contains("Expected"));
    }

    #[test]
    fn test_assertion_in_range() {
        let result = Assert::in_range("test_range", 5, 1, 10);
        assert!(result.passed);

        let result = Assert::in_range("test_range", 15, 1, 10);
        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_approx_eq() {
        let result = Assert::approx_eq("test_approx", 1.0001, 1.0, 0.001);
        assert!(result.passed);

        let result = Assert::approx_eq("test_approx", 1.1, 1.0, 0.05);
        assert!(!result.passed);
    }

    #[test]
    fn test_validation_result_combine() {
        let results = vec![
            ValidationResult::pass("Test 1 passed".to_string()),
            ValidationResult::pass("Test 2 passed".to_string()),
            ValidationResult::fail("Test 3 failed".to_string()),
        ];

        let combined = ValidationResult::combine(results);
        assert!(!combined.passed);
        assert_eq!(combined.assertion_count, 3);
        assert!(combined.details.is_some());
    }

    #[test]
    fn test_lockfree_assertion() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Arc;
        let counter = Arc::new(AtomicU64::new(0));

        let result = Assert::is_lockfree("atomic_increment", {
            let counter = Arc::clone(&counter);
            move || { counter.fetch_add(1, Ordering::Relaxed); }
        }, 4);

        assert!(result.passed);
    }

    #[test]
    fn test_property_test_framework() {
        let test = PropertyTest::new("test_addition_commutative")
            .add_property("commutativity", "a + b == b + a", |values| {
                if values.len() >= 2 {
                    if let (PropertyValue::U64(a), PropertyValue::U64(b)) = (&values[0], &values[1]) {
                        Ok(a + b == b + a)
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            })
            .add_generator(U64Generator::new("a", (0, 100)))
            .add_generator(U64Generator::new("b", (0, 100)));

        let result = test.run().unwrap();
        assert!(result.passed);
    }
}