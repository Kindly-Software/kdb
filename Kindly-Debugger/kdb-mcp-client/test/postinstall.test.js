#!/usr/bin/env node
/**
 * postinstall.test.js - Integration tests for postinstall smart auto-configure
 *
 * Tests verify:
 * - Environment variable handling (KDB_SKIP_POSTINSTALL, KDB_AUTO_CONFIGURE_ON_INSTALL)
 * - License detection from ~/.kdb/license
 * - MCP client config detection
 * - Non-fatal error handling (always exit 0)
 *
 * Run: node test/postinstall.test.js
 */

import { execSync, spawnSync } from 'child_process';
import { existsSync, mkdirSync, writeFileSync, rmSync, readFileSync } from 'fs';
import { join, dirname } from 'path';
import { tmpdir, homedir, platform } from 'os';
import { fileURLToPath } from 'url';
import assert from 'assert';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// ============================================================================
// Test Utilities
// ============================================================================

const POSTINSTALL_SCRIPT = join(__dirname, '..', 'scripts', 'postinstall.js');

/**
 * Run postinstall script with given environment variables.
 * Returns { stdout, stderr, exitCode }
 */
function runPostinstall(env = {}) {
    const result = spawnSync('node', [POSTINSTALL_SCRIPT], {
        env: {
            ...process.env,
            // Ensure we don't accidentally configure the real system
            HOME: env.HOME || tmpdir(),
            USERPROFILE: env.HOME || tmpdir(),
            ...env,
        },
        encoding: 'utf8',
        timeout: 30000,
    });

    return {
        stdout: result.stdout || '',
        stderr: result.stderr || '',
        exitCode: result.status,
    };
}

/**
 * Create a temporary directory with optional license file.
 */
function createTempHome(options = {}) {
    const tempHome = join(tmpdir(), `kdb-test-${Date.now()}-${Math.random().toString(36).slice(2)}`);
    mkdirSync(tempHome, { recursive: true });

    if (options.license) {
        const kdbDir = join(tempHome, '.kdb');
        mkdirSync(kdbDir, { recursive: true });
        writeFileSync(join(kdbDir, 'license'), options.license);
    }

    if (options.mcpConfig) {
        // Create a mock Claude Code config
        const configDir = join(tempHome, '.config', 'claude-code');
        mkdirSync(configDir, { recursive: true });
        writeFileSync(join(configDir, 'mcp.json'), JSON.stringify(options.mcpConfig, null, 2));
    }

    return tempHome;
}

/**
 * Clean up temporary directory.
 */
function cleanupTempHome(tempHome) {
    try {
        rmSync(tempHome, { recursive: true, force: true });
    } catch {
        // Ignore cleanup errors
    }
}

// ============================================================================
// Test Cases
// ============================================================================

let passed = 0;
let failed = 0;

function test(name, fn) {
    try {
        fn();
        console.log(`  [PASS] ${name}`);
        passed++;
    } catch (error) {
        console.log(`  [FAIL] ${name}`);
        console.log(`         ${error.message}`);
        failed++;
    }
}

console.log('\nPostinstall Tests\n' + '='.repeat(50));

// Test 1: Skips when KDB_SKIP_POSTINSTALL=true
test('skips when KDB_SKIP_POSTINSTALL=true', () => {
    const result = runPostinstall({
        KDB_SKIP_POSTINSTALL: 'true',
    });

    assert.strictEqual(result.exitCode, 0, 'Should exit 0');
    assert.strictEqual(result.stdout, '', 'Should produce no output');
});

// Test 2: Skips when KDB_SKIP_POSTINSTALL=1
test('skips when KDB_SKIP_POSTINSTALL=1', () => {
    const result = runPostinstall({
        KDB_SKIP_POSTINSTALL: '1',
    });

    assert.strictEqual(result.exitCode, 0, 'Should exit 0');
    assert.strictEqual(result.stdout, '', 'Should produce no output');
});

// Test 3: Shows signup URL when no license
test('shows signup URL when no license', () => {
    const tempHome = createTempHome({ license: null });

    try {
        const result = runPostinstall({
            HOME: tempHome,
            NO_COLOR: '1',
        });

        assert.strictEqual(result.exitCode, 0, 'Should exit 0');
        assert.ok(result.stdout.includes('kindly.software/signup'), 'Should show signup URL');
        assert.ok(result.stdout.includes('7-day trial'), 'Should mention trial');
        assert.ok(result.stdout.includes('No credit card'), 'Should mention no credit card');
    } finally {
        cleanupTempHome(tempHome);
    }
});

// Test 4: Shows instructions when license exists (default conservative mode)
test('shows instructions when license exists (default)', () => {
    const tempHome = createTempHome({ license: 'KDB-HOBBY-12345678-abcdef' });

    try {
        const result = runPostinstall({
            HOME: tempHome,
            NO_COLOR: '1',
            // KDB_AUTO_CONFIGURE_ON_INSTALL NOT set
        });

        assert.strictEqual(result.exitCode, 0, 'Should exit 0');
        assert.ok(result.stdout.includes('License detected'), 'Should detect license');
        assert.ok(result.stdout.includes('npx kdb-configure'), 'Should show configure command');
    } finally {
        cleanupTempHome(tempHome);
    }
});

// Test 5: Silent when already configured
test('silent when already configured', () => {
    const tempHome = createTempHome({
        license: 'KDB-HOBBY-12345678-abcdef',
        mcpConfig: {
            mcpServers: {
                kdb: {
                    command: 'npx',
                    args: ['@kindly-software-inc/kdb'],
                },
            },
        },
    });

    try {
        const result = runPostinstall({
            HOME: tempHome,
            NO_COLOR: '1',
        });

        assert.strictEqual(result.exitCode, 0, 'Should exit 0');
        assert.strictEqual(result.stdout, '', 'Should produce no output when already configured');
    } finally {
        cleanupTempHome(tempHome);
    }
});

// Test 6: Non-fatal on errors (exit 0)
test('non-fatal on errors (exit 0)', () => {
    // Even with a malformed/unreadable home, should exit 0
    const result = runPostinstall({
        HOME: '/nonexistent/path/that/does/not/exist',
        NO_COLOR: '1',
    });

    assert.strictEqual(result.exitCode, 0, 'Should always exit 0');
});

// Test 7: Skips in CI environment by default
test('skips in CI environment by default', () => {
    const tempHome = createTempHome({ license: null });

    try {
        const result = runPostinstall({
            HOME: tempHome,
            CI: 'true',
            NO_COLOR: '1',
        });

        assert.strictEqual(result.exitCode, 0, 'Should exit 0');
        assert.strictEqual(result.stdout, '', 'Should skip silently in CI');
    } finally {
        cleanupTempHome(tempHome);
    }
});

// Test 8: Detects invalid license format
test('detects invalid license format', () => {
    const tempHome = createTempHome({ license: 'INVALID-LICENSE-KEY' });

    try {
        const result = runPostinstall({
            HOME: tempHome,
            NO_COLOR: '1',
        });

        assert.strictEqual(result.exitCode, 0, 'Should exit 0');
        // Should show signup URL because license is invalid
        assert.ok(result.stdout.includes('kindly.software/signup'), 'Should show signup for invalid license');
    } finally {
        cleanupTempHome(tempHome);
    }
});

// Test 9: Handles empty license file
test('handles empty license file', () => {
    const tempHome = createTempHome({ license: '' });

    try {
        const result = runPostinstall({
            HOME: tempHome,
            NO_COLOR: '1',
        });

        assert.strictEqual(result.exitCode, 0, 'Should exit 0');
        assert.ok(result.stdout.includes('kindly.software/signup'), 'Should show signup for empty license');
    } finally {
        cleanupTempHome(tempHome);
    }
});

// Test 10: Detects @kindly-software-inc/kdb in config
test('detects @kindly-software-inc/kdb in config', () => {
    const tempHome = createTempHome({
        license: 'KDB-PRO-87654321-xyz123',
        mcpConfig: {
            mcpServers: {
                'my-kdb': {
                    command: 'npx',
                    args: ['@kindly-software-inc/kdb'],
                },
            },
        },
    });

    try {
        const result = runPostinstall({
            HOME: tempHome,
            NO_COLOR: '1',
        });

        assert.strictEqual(result.exitCode, 0, 'Should exit 0');
        // Should be silent because @kindly-software-inc/kdb is already in config
        assert.strictEqual(result.stdout, '', 'Should be silent when package is in config');
    } finally {
        cleanupTempHome(tempHome);
    }
});

// Test 11: Respects NO_COLOR environment variable
test('respects NO_COLOR environment variable', () => {
    const tempHome = createTempHome({ license: null });

    try {
        const result = runPostinstall({
            HOME: tempHome,
            NO_COLOR: '1',
        });

        assert.strictEqual(result.exitCode, 0, 'Should exit 0');
        // Should not contain ANSI escape codes
        assert.ok(!result.stdout.includes('\x1b['), 'Should not contain ANSI escape codes');
    } finally {
        cleanupTempHome(tempHome);
    }
});

// Test 12: Different license tier prefixes work
test('accepts different license tier prefixes', () => {
    const tiers = ['HOBBY', 'PRO', 'ENGINEER', 'TEAMS', 'ENTERPRISE'];

    for (const tier of tiers) {
        const tempHome = createTempHome({ license: `KDB-${tier}-12345678-test` });

        try {
            const result = runPostinstall({
                HOME: tempHome,
                NO_COLOR: '1',
            });

            assert.strictEqual(result.exitCode, 0, `Should exit 0 for ${tier}`);
            assert.ok(result.stdout.includes('License detected'), `Should detect ${tier} license`);
        } finally {
            cleanupTempHome(tempHome);
        }
    }
});

// ============================================================================
// Summary
// ============================================================================

console.log('\n' + '='.repeat(50));
console.log(`Results: ${passed} passed, ${failed} failed`);
console.log('='.repeat(50) + '\n');

process.exit(failed > 0 ? 1 : 0);
