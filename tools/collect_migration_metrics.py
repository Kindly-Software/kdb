#!/usr/bin/env python3
"""
Migration Metrics Collection System

Purpose: Track and report metrics for Phase 4 derive macro migration.

UCE34 Q28 Compliance: Measure simplification impact (87.5% code reduction).
B32 Framework: Honest metrics with statistical rigor.
T28 Testing: Validate migration success across all test tiers.

Usage:
    python3 tools/collect_migration_metrics.py analyze atomic_capsule
    python3 tools/collect_migration_metrics.py compare atomic_capsule before after
    python3 tools/collect_migration_metrics.py report atomic_capsule
    python3 tools/collect_migration_metrics.py dashboard
"""

import re
import json
import sys
import subprocess
from pathlib import Path
from typing import Dict, List, Tuple
from dataclasses import dataclass, asdict
from datetime import datetime

@dataclass
class MigrationMetrics:
    """Comprehensive migration metrics for a single project."""
    project: str
    timestamp: str

    # Code metrics
    macros_before: int
    macros_after: int
    macros_removed: int
    files_modified: int
    lines_removed: int
    lines_added: int
    net_loc_change: int

    # Compilation metrics
    compile_time_before_ms: int
    compile_time_after_ms: int
    compile_time_delta_ms: int
    compile_time_delta_percent: float

    # Test metrics
    tests_before_total: int
    tests_before_passed: int
    tests_before_failed: int
    tests_after_total: int
    tests_after_passed: int
    tests_after_failed: int
    test_pass_rate_before: float
    test_pass_rate_after: float
    test_pass_rate_delta: float

    # Performance metrics (B32 Framework)
    benchmark_avg_before_ns: float
    benchmark_avg_after_ns: float
    benchmark_delta_ns: float
    benchmark_delta_percent: float

    # Quality metrics
    clippy_warnings_before: int
    clippy_warnings_after: int
    binary_size_before_kb: int
    binary_size_after_kb: int
    binary_size_delta_percent: float

    # Migration success indicators
    migration_success: bool
    rollback_required: bool
    validation_errors: List[str]


class MigrationMetricsCollector:
    """Collect comprehensive migration metrics for Phase 4."""

    def __init__(self, project_path: Path):
        self.project_path = project_path
        self.project_name = project_path.name
        self.baseline_dir = Path(f"migration_baselines/{self.project_name}")
        self.baseline_dir.mkdir(parents=True, exist_ok=True)

    def count_manual_macros(self) -> int:
        """Count manual verification macros in project."""
        patterns = [
            "verify_capsule_properties!",
            "verify_alignment_only!",
            "verify_simd_capsule!",
        ]

        count = 0
        for rs_file in self.project_path.rglob("*.rs"):
            if "/target/" in str(rs_file):
                continue

            content = rs_file.read_text()
            for pattern in patterns:
                count += content.count(pattern)

        return count

    def count_derive_macros(self) -> int:
        """Count derive macros in project."""
        count = 0
        for rs_file in self.project_path.rglob("*.rs"):
            if "/target/" in str(rs_file):
                continue

            content = rs_file.read_text()
            count += content.count("#[derive(ComputationalCapsule)]")

        return count

    def measure_compilation_time(self) -> int:
        """Measure compilation time in milliseconds (B32 Framework)."""
        import time

        # Clean build for accurate timing
        subprocess.run(
            ["cargo", "clean"],
            cwd=self.project_path,
            capture_output=True,
        )

        # Measure compilation time
        start = time.time()
        result = subprocess.run(
            ["cargo", "build", "--all-features", "--quiet"],
            cwd=self.project_path,
            capture_output=True,
        )
        elapsed_ms = int((time.time() - start) * 1000)

        if result.returncode != 0:
            print(f"WARNING: Compilation failed:\n{result.stderr.decode()}")
            return -1

        return elapsed_ms

    def run_tests(self) -> Tuple[int, int, int]:
        """Run tests and return (total, passed, failed)."""
        result = subprocess.run(
            ["cargo", "test", "--all-features", "--", "--format=json"],
            cwd=self.project_path,
            capture_output=True,
            text=True,
        )

        # Parse JSON test output
        passed = 0
        failed = 0
        for line in result.stdout.splitlines():
            if not line.strip():
                continue
            try:
                event = json.loads(line)
                if event.get("type") == "test":
                    if event.get("event") == "ok":
                        passed += 1
                    elif event.get("event") == "failed":
                        failed += 1
            except json.JSONDecodeError:
                continue

        total = passed + failed
        return (total, passed, failed)

    def run_benchmarks(self) -> float:
        """Run benchmarks and return average time in nanoseconds."""
        result = subprocess.run(
            ["cargo", "bench", "--all-features"],
            cwd=self.project_path,
            capture_output=True,
            text=True,
        )

        # Parse benchmark output
        # Example: "test bench_dual_atomic_u64 ... bench:          9,768 ns/iter (+/- 234)"
        bench_times = []
        pattern = r"bench:\s+([\d,]+)\s+ns/iter"
        for match in re.finditer(pattern, result.stdout):
            time_str = match.group(1).replace(",", "")
            bench_times.append(float(time_str))

        if bench_times:
            return sum(bench_times) / len(bench_times)
        else:
            return 0.0

    def run_clippy(self) -> int:
        """Run clippy and return warning count."""
        result = subprocess.run(
            ["cargo", "clippy", "--all-features", "--", "-D", "warnings"],
            cwd=self.project_path,
            capture_output=True,
            text=True,
        )

        # Count warnings in output
        warning_count = result.stderr.count("warning:")
        return warning_count

    def measure_binary_size(self) -> int:
        """Measure binary size in KB."""
        # Build release binary
        subprocess.run(
            ["cargo", "build", "--release", "--all-features"],
            cwd=self.project_path,
            capture_output=True,
        )

        # Find binary
        binary_path = self.project_path / "target" / "release" / self.project_name
        if binary_path.exists():
            size_bytes = binary_path.stat().st_size
            return size_bytes // 1024  # Convert to KB
        else:
            return 0

    def collect_baseline(self) -> Dict:
        """Collect baseline metrics before migration."""
        print(f"Collecting baseline metrics for {self.project_name}...")

        baseline = {
            "project": self.project_name,
            "timestamp": datetime.now().isoformat(),
            "macros": self.count_manual_macros(),
            "compile_time_ms": self.measure_compilation_time(),
            "tests": self.run_tests(),
            "benchmark_avg_ns": self.run_benchmarks(),
            "clippy_warnings": self.run_clippy(),
            "binary_size_kb": self.measure_binary_size(),
        }

        # Save baseline
        baseline_file = self.baseline_dir / "baseline.json"
        with open(baseline_file, "w") as f:
            json.dump(baseline, f, indent=2)

        print(f"✓ Baseline saved to {baseline_file}")
        return baseline

    def collect_after_migration(self) -> Dict:
        """Collect metrics after migration."""
        print(f"Collecting post-migration metrics for {self.project_name}...")

        after = {
            "project": self.project_name,
            "timestamp": datetime.now().isoformat(),
            "macros": self.count_manual_macros(),
            "derives": self.count_derive_macros(),
            "compile_time_ms": self.measure_compilation_time(),
            "tests": self.run_tests(),
            "benchmark_avg_ns": self.run_benchmarks(),
            "clippy_warnings": self.run_clippy(),
            "binary_size_kb": self.measure_binary_size(),
        }

        # Save post-migration metrics
        after_file = self.baseline_dir / "after_migration.json"
        with open(after_file, "w") as f:
            json.dump(after, f, indent=2)

        print(f"✓ Post-migration metrics saved to {after_file}")
        return after

    def compare_metrics(self, before: Dict, after: Dict) -> MigrationMetrics:
        """Compare before/after metrics and generate report."""
        tests_before = before["tests"]
        tests_after = after["tests"]

        macros_before = before["macros"]
        macros_after = after.get("macros", 0)
        derives_after = after.get("derives", 0)

        # Calculate test pass rates
        test_pass_rate_before = (
            tests_before[1] / tests_before[0] if tests_before[0] > 0 else 0.0
        )
        test_pass_rate_after = (
            tests_after[1] / tests_after[0] if tests_after[0] > 0 else 0.0
        )

        # Compilation time delta
        compile_time_delta_ms = after["compile_time_ms"] - before["compile_time_ms"]
        compile_time_delta_percent = (
            (compile_time_delta_ms / before["compile_time_ms"]) * 100
            if before["compile_time_ms"] > 0
            else 0.0
        )

        # Benchmark delta
        benchmark_delta_ns = after["benchmark_avg_ns"] - before["benchmark_avg_ns"]
        benchmark_delta_percent = (
            (benchmark_delta_ns / before["benchmark_avg_ns"]) * 100
            if before["benchmark_avg_ns"] > 0
            else 0.0
        )

        # Binary size delta
        binary_size_delta_percent = (
            ((after["binary_size_kb"] - before["binary_size_kb"]) / before["binary_size_kb"]) * 100
            if before["binary_size_kb"] > 0
            else 0.0
        )

        # Validation: Migration successful?
        validation_errors = []
        migration_success = True

        if tests_after[2] > 0:  # Failed tests
            validation_errors.append(f"Tests failed: {tests_after[2]} failures")
            migration_success = False

        if test_pass_rate_after < test_pass_rate_before - 0.01:
            validation_errors.append(
                f"Test pass rate decreased: {test_pass_rate_before:.1%} → {test_pass_rate_after:.1%}"
            )
            migration_success = False

        if abs(benchmark_delta_percent) > 5.0:
            validation_errors.append(
                f"Performance regression: {benchmark_delta_percent:+.1%}"
            )
            migration_success = False

        if after["clippy_warnings"] > before["clippy_warnings"]:
            validation_errors.append(
                f"Clippy warnings increased: {before['clippy_warnings']} → {after['clippy_warnings']}"
            )

        rollback_required = not migration_success

        metrics = MigrationMetrics(
            project=self.project_name,
            timestamp=datetime.now().isoformat(),
            macros_before=macros_before,
            macros_after=macros_after,
            macros_removed=macros_before - macros_after,
            files_modified=0,  # TODO: Calculate from git diff
            lines_removed=macros_before * 1,  # Estimate: 1 line per macro
            lines_added=derives_after * 2,  # Estimate: 2 lines per derive
            net_loc_change=(derives_after * 2) - (macros_before * 1),
            compile_time_before_ms=before["compile_time_ms"],
            compile_time_after_ms=after["compile_time_ms"],
            compile_time_delta_ms=compile_time_delta_ms,
            compile_time_delta_percent=compile_time_delta_percent,
            tests_before_total=tests_before[0],
            tests_before_passed=tests_before[1],
            tests_before_failed=tests_before[2],
            tests_after_total=tests_after[0],
            tests_after_passed=tests_after[1],
            tests_after_failed=tests_after[2],
            test_pass_rate_before=test_pass_rate_before,
            test_pass_rate_after=test_pass_rate_after,
            test_pass_rate_delta=test_pass_rate_after - test_pass_rate_before,
            benchmark_avg_before_ns=before["benchmark_avg_ns"],
            benchmark_avg_after_ns=after["benchmark_avg_ns"],
            benchmark_delta_ns=benchmark_delta_ns,
            benchmark_delta_percent=benchmark_delta_percent,
            clippy_warnings_before=before["clippy_warnings"],
            clippy_warnings_after=after["clippy_warnings"],
            binary_size_before_kb=before["binary_size_kb"],
            binary_size_after_kb=after["binary_size_kb"],
            binary_size_delta_percent=binary_size_delta_percent,
            migration_success=migration_success,
            rollback_required=rollback_required,
            validation_errors=validation_errors,
        )

        return metrics

    def print_report(self, metrics: MigrationMetrics):
        """Print comprehensive migration report."""
        print("\n" + "=" * 70)
        print(f"  Migration Metrics Report: {metrics.project}")
        print("=" * 70)
        print()

        # Code metrics
        print("Code Metrics:")
        print(f"  Manual macros removed: {metrics.macros_removed}")
        print(f"  Derive macros added: {metrics.macros_after}")
        print(f"  Files modified: {metrics.files_modified}")
        print(f"  Lines removed: {metrics.lines_removed}")
        print(f"  Lines added: {metrics.lines_added}")
        print(f"  Net LOC change: {metrics.net_loc_change:+d}")
        print()

        # Compilation metrics
        print("Compilation Metrics:")
        print(f"  Before: {metrics.compile_time_before_ms}ms")
        print(f"  After: {metrics.compile_time_after_ms}ms")
        print(f"  Delta: {metrics.compile_time_delta_ms:+d}ms ({metrics.compile_time_delta_percent:+.1f}%)")
        print()

        # Test metrics
        print("Test Metrics:")
        print(f"  Before: {metrics.tests_before_passed}/{metrics.tests_before_total} passed ({metrics.test_pass_rate_before:.1%})")
        print(f"  After: {metrics.tests_after_passed}/{metrics.tests_after_total} passed ({metrics.test_pass_rate_after:.1%})")
        print(f"  Delta: {metrics.test_pass_rate_delta:+.1%}")
        print()

        # Performance metrics (B32 Framework)
        print("Performance Metrics (B32 Framework):")
        print(f"  Benchmark avg before: {metrics.benchmark_avg_before_ns:.1f}ns")
        print(f"  Benchmark avg after: {metrics.benchmark_avg_after_ns:.1f}ns")
        print(f"  Delta: {metrics.benchmark_delta_ns:+.1f}ns ({metrics.benchmark_delta_percent:+.1f}%)")
        print()

        # Quality metrics
        print("Quality Metrics:")
        print(f"  Clippy warnings before: {metrics.clippy_warnings_before}")
        print(f"  Clippy warnings after: {metrics.clippy_warnings_after}")
        print(f"  Binary size before: {metrics.binary_size_before_kb}KB")
        print(f"  Binary size after: {metrics.binary_size_after_kb}KB")
        print(f"  Binary size delta: {metrics.binary_size_delta_percent:+.1f}%")
        print()

        # Migration success
        status = "✓ SUCCESS" if metrics.migration_success else "✗ FAILURE"
        print(f"Migration Status: {status}")

        if metrics.validation_errors:
            print("\nValidation Errors:")
            for error in metrics.validation_errors:
                print(f"  - {error}")

        if metrics.rollback_required:
            print("\n⚠️  ROLLBACK REQUIRED - Migration failed validation!")
            print("   Run: git restore {metrics.project}/**/*.rs")
        else:
            print("\n✓ Migration successful - Ready to commit")

        print("\n" + "=" * 70)


def generate_dashboard(projects: List[str]):
    """Generate comprehensive dashboard for all projects."""
    print("\n" + "=" * 80)
    print("  Phase 4 Migration Dashboard")
    print("=" * 80)
    print()

    # Table header
    print(f"{'Project':<20} {'Macros':<10} {'Tests':<15} {'Compile Time':<15} {'Benchmarks':<15} {'Status':<10}")
    print("-" * 80)

    total_macros = 0
    total_tests = 0
    passed_tests = 0

    for project_name in projects:
        baseline_file = Path(f"migration_baselines/{project_name}/baseline.json")
        after_file = Path(f"migration_baselines/{project_name}/after_migration.json")

        if not baseline_file.exists() or not after_file.exists():
            print(f"{project_name:<20} {'N/A':<10} {'N/A':<15} {'N/A':<15} {'N/A':<15} {'⏳ Pending':<10}")
            continue

        with open(baseline_file) as f:
            before = json.load(f)
        with open(after_file) as f:
            after = json.load(f)

        macros_removed = before["macros"] - after.get("macros", 0)
        tests = after["tests"]
        test_status = f"{tests[1]}/{tests[0]} ✅" if tests[2] == 0 else f"{tests[1]}/{tests[0]} ✗"

        compile_delta = after["compile_time_ms"] - before["compile_time_ms"]
        compile_status = f"{compile_delta:+d}ms ({compile_delta / before['compile_time_ms'] * 100:+.1f}%)"

        bench_delta = after["benchmark_avg_ns"] - before["benchmark_avg_ns"]
        bench_status = f"{bench_delta:+.1f}ns ({bench_delta / before['benchmark_avg_ns'] * 100:+.1f}%)"

        status = "✅ PASS" if tests[2] == 0 else "✗ FAIL"

        print(f"{project_name:<20} {macros_removed:<10} {test_status:<15} {compile_status:<15} {bench_status:<15} {status:<10}")

        total_macros += macros_removed
        total_tests += tests[0]
        passed_tests += tests[1]

    print("-" * 80)
    print(f"{'TOTAL':<20} {total_macros:<10} {passed_tests}/{total_tests} ✅")
    print()

    # Summary statistics
    print("Summary Statistics:")
    print(f"  Total macros removed: {total_macros}")
    print(f"  Total tests: {total_tests} ({passed_tests} passed, {total_tests - passed_tests} failed)")
    print(f"  Overall pass rate: {passed_tests / total_tests * 100:.1f}%")
    print()

    print("=" * 80)


def main():
    """Main entry point."""
    if len(sys.argv) < 2:
        print("Usage:")
        print("  python3 tools/collect_migration_metrics.py analyze <project>")
        print("  python3 tools/collect_migration_metrics.py compare <project>")
        print("  python3 tools/collect_migration_metrics.py report <project>")
        print("  python3 tools/collect_migration_metrics.py dashboard")
        sys.exit(1)

    command = sys.argv[1]

    if command == "analyze":
        if len(sys.argv) < 3:
            print("Usage: python3 tools/collect_migration_metrics.py analyze <project>")
            sys.exit(1)

        project_name = sys.argv[2]
        project_path = Path(project_name)

        if not project_path.exists():
            print(f"Error: Project not found: {project_path}")
            sys.exit(1)

        collector = MigrationMetricsCollector(project_path)
        baseline = collector.collect_baseline()

        print(f"\n✓ Baseline metrics collected for {project_name}")
        print(f"  Manual macros: {baseline['macros']}")
        print(f"  Compilation time: {baseline['compile_time_ms']}ms")
        print(f"  Tests: {baseline['tests'][1]}/{baseline['tests'][0]} passed")

    elif command == "compare":
        if len(sys.argv) < 3:
            print("Usage: python3 tools/collect_migration_metrics.py compare <project>")
            sys.exit(1)

        project_name = sys.argv[2]
        project_path = Path(project_name)

        collector = MigrationMetricsCollector(project_path)

        # Load baseline
        baseline_file = collector.baseline_dir / "baseline.json"
        if not baseline_file.exists():
            print(f"Error: Baseline not found. Run 'analyze {project_name}' first.")
            sys.exit(1)

        with open(baseline_file) as f:
            before = json.load(f)

        # Collect post-migration metrics
        after = collector.collect_after_migration()

        # Compare
        metrics = collector.compare_metrics(before, after)

        # Save comparison
        comparison_file = collector.baseline_dir / "comparison.json"
        with open(comparison_file, "w") as f:
            json.dump(asdict(metrics), f, indent=2)

        # Print report
        collector.print_report(metrics)

    elif command == "report":
        if len(sys.argv) < 3:
            print("Usage: python3 tools/collect_migration_metrics.py report <project>")
            sys.exit(1)

        project_name = sys.argv[2]
        comparison_file = Path(f"migration_baselines/{project_name}/comparison.json")

        if not comparison_file.exists():
            print(f"Error: Comparison not found. Run 'compare {project_name}' first.")
            sys.exit(1)

        with open(comparison_file) as f:
            metrics_dict = json.load(f)

        metrics = MigrationMetrics(**metrics_dict)
        collector = MigrationMetricsCollector(Path(project_name))
        collector.print_report(metrics)

    elif command == "dashboard":
        projects = [
            "atomic_capsule",
            "clapi_core",
            "kindly_hft",
            "kindly-db",
            "kiang",
        ]
        generate_dashboard(projects)

    else:
        print(f"Unknown command: {command}")
        sys.exit(1)


if __name__ == "__main__":
    main()
