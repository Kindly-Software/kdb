#!/usr/bin/env node
/**
 * kdb postinstall - Smart Auto-Configuration
 *
 * Automatically configures MCP clients if license is detected.
 *
 * Behavior:
 * - If KDB_SKIP_POSTINSTALL=true: Skip entirely
 * - If license exists + not configured + KDB_AUTO_CONFIGURE_ON_INSTALL=true: Auto-configure
 * - If license exists + not configured: Show instructions
 * - If no license: Show signup URL
 * - If already configured: Silent success
 *
 * Environment Variables:
 * - KDB_SKIP_POSTINSTALL=true     Skip postinstall entirely
 * - KDB_AUTO_CONFIGURE_ON_INSTALL=true  Auto-configure when license exists
 * - NO_COLOR=1                    Disable colored output
 * - CI=true                       Detected as CI environment (skip interactive)
 *
 * Exit Codes:
 * - 0: Always (non-fatal, never breaks npm install)
 */

import { execSync, spawn } from 'child_process';
import { existsSync, readFileSync } from 'fs';
import { join } from 'path';
import { homedir, platform } from 'os';

// ============================================================================
// Configuration
// ============================================================================

const SIGNUP_URL = 'https://kindly.software/signup';
const DOCS_URL = 'https://kindly.software/docs/setup';
const LICENSE_PREFIX = 'KDB-';

// ============================================================================
// Color Support
// ============================================================================

/**
 * Check if terminal supports colors.
 * Respects NO_COLOR standard: https://no-color.org/
 */
function supportsColor() {
    // NO_COLOR takes precedence
    if (process.env.NO_COLOR !== undefined) {
        return false;
    }
    // CI environments often don't support colors well
    if (process.env.CI === 'true') {
        return false;
    }
    // Check TERM
    if (process.env.TERM === 'dumb') {
        return false;
    }
    // Check if stdout is a TTY
    return process.stdout.isTTY === true;
}

const USE_COLOR = supportsColor();

const colors = {
    reset: USE_COLOR ? '\x1b[0m' : '',
    bold: USE_COLOR ? '\x1b[1m' : '',
    dim: USE_COLOR ? '\x1b[2m' : '',
    red: USE_COLOR ? '\x1b[31m' : '',
    green: USE_COLOR ? '\x1b[32m' : '',
    yellow: USE_COLOR ? '\x1b[33m' : '',
    blue: USE_COLOR ? '\x1b[34m' : '',
    cyan: USE_COLOR ? '\x1b[36m' : '',
};

// ============================================================================
// Path Utilities
// ============================================================================

/**
 * Get the home directory cross-platform.
 */
function getHomeDir() {
    return process.env.HOME || process.env.USERPROFILE || homedir();
}

/**
 * Get the kdb data directory (~/.kdb).
 */
function getKdbDataDir() {
    return join(getHomeDir(), '.kdb');
}

/**
 * Get the license file path.
 */
function getLicensePath() {
    return join(getKdbDataDir(), 'license');
}

// ============================================================================
// MCP Client Config Paths
// ============================================================================

/**
 * Get all known MCP client config paths for the current platform.
 * Returns an array of { name, path } objects.
 */
function getMcpClientConfigPaths() {
    const home = getHomeDir();
    const os = platform();

    const paths = [];

    // Claude Code
    if (os === 'darwin') {
        // macOS
        paths.push({
            name: 'Claude Code',
            path: join(home, 'Library', 'Application Support', 'claude-code', 'mcp.json'),
        });
        paths.push({
            name: 'Claude Desktop',
            path: join(home, 'Library', 'Application Support', 'Claude', 'claude_desktop_config.json'),
        });
    } else if (os === 'win32') {
        // Windows
        const appData = process.env.APPDATA || join(home, 'AppData', 'Roaming');
        paths.push({
            name: 'Claude Code',
            path: join(appData, 'claude-code', 'mcp.json'),
        });
        paths.push({
            name: 'Claude Desktop',
            path: join(appData, 'Claude', 'claude_desktop_config.json'),
        });
    } else {
        // Linux and others
        paths.push({
            name: 'Claude Code',
            path: join(home, '.config', 'claude-code', 'mcp.json'),
        });
        paths.push({
            name: 'Claude Desktop',
            path: join(home, '.config', 'Claude', 'claude_desktop_config.json'),
        });
    }

    // Cursor (cross-platform in home directory)
    paths.push({
        name: 'Cursor',
        path: join(home, '.cursor', 'mcp.json'),
    });

    // VS Code MCP (cross-platform in home directory)
    paths.push({
        name: 'VS Code',
        path: join(home, '.vscode', 'mcp.json'),
    });

    // Continue.dev
    paths.push({
        name: 'Continue.dev',
        path: join(home, '.continue', 'config.json'),
    });

    return paths;
}

// ============================================================================
// Detection Functions
// ============================================================================

/**
 * Check if a file exists.
 */
function fileExists(filepath) {
    try {
        return existsSync(filepath);
    } catch {
        return false;
    }
}

/**
 * Read file contents, returns null on error.
 */
function readFile(filepath) {
    try {
        return readFileSync(filepath, 'utf8');
    } catch {
        return null;
    }
}

/**
 * Check if kdb is configured in a config file.
 * Looks for "kdb" key inside "mcpServers".
 */
function isKdbConfigured(configPath) {
    const content = readFile(configPath);
    if (!content) {
        return false;
    }

    try {
        // Check for kdb in mcpServers (standard MCP config format)
        if (content.includes('"kdb"') && content.includes('mcpServers')) {
            return true;
        }
        // Also check for our package name
        if (content.includes('@kindly-software-inc/kdb')) {
            return true;
        }
        return false;
    } catch {
        return false;
    }
}

/**
 * Check if a valid license exists.
 * License must start with KDB- prefix.
 */
function hasValidLicense() {
    const licensePath = getLicensePath();
    const content = readFile(licensePath);

    if (!content) {
        return false;
    }

    const license = content.trim();
    return license.startsWith(LICENSE_PREFIX) && license.length > 10;
}

/**
 * Check if any MCP client is already configured with kdb.
 */
function isAnyClientConfigured() {
    const configPaths = getMcpClientConfigPaths();
    return configPaths.some(({ path }) => isKdbConfigured(path));
}

/**
 * Get list of detected MCP clients (config file exists).
 */
function getDetectedClients() {
    const configPaths = getMcpClientConfigPaths();
    return configPaths.filter(({ path }) => {
        // Config file exists, or parent directory exists (can be configured)
        return fileExists(path) || fileExists(join(path, '..'));
    });
}

// ============================================================================
// Output Functions
// ============================================================================

/**
 * Print a message with kdb prefix.
 */
function log(message) {
    console.log(`${colors.dim}[kdb]${colors.reset} ${message}`);
}

/**
 * Print a blank line.
 */
function blank() {
    console.log('');
}

/**
 * Print the signup message with trial info.
 */
function printSignupMessage() {
    blank();
    log(`${colors.bold}Get your FREE kdb license:${colors.reset}`);
    log(`   ${colors.cyan}${SIGNUP_URL}${colors.reset}`);
    log('   ');
    log(`   ${colors.green}*${colors.reset} 7-day trial with ALL features`);
    log(`   ${colors.green}*${colors.reset} No credit card required`);
    log('   ');
    log(`   After signup, run: ${colors.cyan}npx kdb-configure --auto${colors.reset}`);
    blank();
}

/**
 * Print instructions for manual configuration.
 */
function printConfigureInstructions() {
    blank();
    log(`${colors.green}License detected at ~/.kdb/license${colors.reset}`);
    log(`   Run setup: ${colors.cyan}npx kdb-configure --auto${colors.reset}`);
    log(`   Or enable auto-config: ${colors.dim}export KDB_AUTO_CONFIGURE_ON_INSTALL=true${colors.reset}`);
    blank();
}

/**
 * Print success message after auto-configuration.
 */
function printAutoConfigSuccess() {
    blank();
    log(`${colors.green}Auto-configuration complete!${colors.reset}`);
    log(`   kdb is now available in your MCP clients.`);
    log(`   Restart your editor to activate.`);
    blank();
}

/**
 * Print error message for auto-configuration failure.
 */
function printAutoConfigError(errorMessage) {
    blank();
    log(`${colors.yellow}Auto-configure failed:${colors.reset} ${errorMessage}`);
    log(`   Run manually: ${colors.cyan}npx kdb-configure --auto${colors.reset}`);
    blank();
}

// ============================================================================
// Auto-Configuration
// ============================================================================

/**
 * Run kdb-configure with auto-approve flags.
 * Returns true on success, false on failure.
 */
function runAutoConfiguration() {
    try {
        // Use npx to run kdb-configure from the same package
        // --auto: auto-approve all prompts
        // Inherit stdio so user sees progress
        execSync('npx kdb-configure --auto', {
            stdio: 'inherit',
            env: {
                ...process.env,
                KDB_AUTO_CONFIGURE: 'true',
            },
            timeout: 60000, // 60 second timeout
        });
        return true;
    } catch (error) {
        // Log error but don't fail
        return false;
    }
}

// ============================================================================
// Main Logic
// ============================================================================

/**
 * Main postinstall logic.
 */
function main() {
    // Skip if explicitly disabled
    if (process.env.KDB_SKIP_POSTINSTALL === 'true' || process.env.KDB_SKIP_POSTINSTALL === '1') {
        process.exit(0);
    }

    // Skip in CI environments unless explicitly enabled
    if (process.env.CI === 'true' && process.env.KDB_AUTO_CONFIGURE_ON_INSTALL !== 'true') {
        // Silent skip in CI
        process.exit(0);
    }

    // Check current state
    const hasLicense = hasValidLicense();
    const alreadyConfigured = isAnyClientConfigured();
    const autoConfigEnabled =
        process.env.KDB_AUTO_CONFIGURE_ON_INSTALL === 'true' ||
        process.env.KDB_AUTO_CONFIGURE_ON_INSTALL === '1';

    // Decision tree
    if (alreadyConfigured) {
        // Already configured - silent success
        // Don't spam the user on every npm install
        process.exit(0);
    }

    if (hasLicense && !alreadyConfigured) {
        // License exists but not configured
        if (autoConfigEnabled) {
            // Aggressive mode: auto-configure
            log(`${colors.cyan}License detected, auto-configuring MCP clients...${colors.reset}`);
            log(`   ${colors.dim}(Set KDB_SKIP_POSTINSTALL=true to disable)${colors.reset}`);

            const success = runAutoConfiguration();

            if (success) {
                printAutoConfigSuccess();
            } else {
                printAutoConfigError('kdb-configure exited with error');
            }
        } else {
            // Conservative mode: show instructions
            printConfigureInstructions();
        }
    } else if (!hasLicense) {
        // No license found - show signup URL
        printSignupMessage();
    }

    // Always exit 0 - never break npm install
    process.exit(0);
}

// ============================================================================
// Entry Point
// ============================================================================

// Run main with error handling
try {
    main();
} catch (error) {
    // Non-fatal: log error but exit 0
    if (process.env.KDB_DEBUG === 'true') {
        console.error('[kdb] Postinstall error:', error.message);
    }
    process.exit(0);
}
