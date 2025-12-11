#!/usr/bin/env node
/**
 * register-protocol.js - Registers kdb:// protocol handler
 *
 * Uses protocol-registry npm package to register kdb:// URLs
 * with the kdb_handler binary.
 *
 * This is called during npm install (via postinstall) to enable
 * one-click terminal automation from the browser.
 *
 * Non-fatal: If registration fails, user can still use manual setup.
 */

import ProtocolRegistry from 'protocol-registry';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { platform } from 'os';
import { existsSync, accessSync, constants } from 'fs';

// Get current directory (ESM equivalent of __dirname)
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Get package root directory (parent of scripts/)
const packageRoot = dirname(__dirname);

/**
 * Get the path to the kdb_handler binary
 * @returns {string|null} Path to binary or null if not found
 */
function getHandlerPath() {
    const os = platform();

    // Binary name varies by platform
    const binaryName = os === 'win32' ? 'kdb_handler.exe' : 'kdb_handler';

    // Check multiple possible locations
    const possiblePaths = [
        // npm package bin directory
        join(packageRoot, 'bin', binaryName),
        // Development build location
        join(packageRoot, '..', 'kdb-mcp', 'target', 'release', binaryName),
        // Global npm installation
        join(dirname(process.execPath), binaryName),
    ];

    for (const path of possiblePaths) {
        if (existsSync(path)) {
            // Verify it's executable on Unix
            if (os !== 'win32') {
                try {
                    accessSync(path, constants.X_OK);
                } catch {
                    console.log(`[kdb] Found ${path} but not executable, skipping`);
                    continue;
                }
            }
            return path;
        }
    }

    return null;
}

/**
 * Register kdb:// protocol with the system
 * @returns {Promise<boolean>} True if successful
 */
async function registerProtocol() {
    const handlerPath = getHandlerPath();

    if (!handlerPath) {
        console.log('[kdb] Handler binary not found, skipping protocol registration');
        console.log('[kdb] This is normal during first install - run again after build');
        return false;
    }

    console.log(`[kdb] Registering kdb:// protocol handler: ${handlerPath}`);

    try {
        // Register the protocol
        // Note: protocol-registry handles platform-specific registration:
        // - Windows: Registry keys under HKEY_CLASSES_ROOT\kdb
        // - macOS: Info.plist modifications (LSHandlerURLTypes)
        // - Linux: XDG .desktop file in ~/.local/share/applications/
        await ProtocolRegistry.register({
            protocol: 'kdb',
            command: `"${handlerPath}" "$_URL_"`,
            override: true,
            terminal: false,  // We launch terminal ourselves
            script: false,    // Path is to compiled binary
        });

        console.log('[kdb] Successfully registered kdb:// protocol');
        return true;

    } catch (error) {
        // Non-fatal - registration is optional enhancement
        console.log(`[kdb] Protocol registration failed (non-fatal): ${error.message}`);

        if (platform() === 'linux') {
            console.log('[kdb] On Linux, you may need to run: update-desktop-database ~/.local/share/applications/');
        }

        return false;
    }
}

/**
 * Check if protocol is already registered
 * @returns {Promise<boolean>} True if registered
 */
async function isRegistered() {
    try {
        const registered = await ProtocolRegistry.checkIfExists('kdb');
        return registered;
    } catch {
        return false;
    }
}

/**
 * Unregister kdb:// protocol
 * @returns {Promise<boolean>} True if successful
 */
async function unregisterProtocol() {
    try {
        // Check if registered first
        if (!await isRegistered()) {
            console.log('[kdb] Protocol not registered, nothing to unregister');
            return true;
        }

        // Currently protocol-registry doesn't have unregister
        // Users need to manually remove:
        // - Windows: Registry key HKEY_CLASSES_ROOT\kdb
        // - macOS: Edit ~/Library/Preferences/com.apple.LaunchServices.plist
        // - Linux: Remove ~/.local/share/applications/kdb.desktop

        console.log('[kdb] Automatic unregistration not supported.');
        console.log('[kdb] To manually unregister:');

        const os = platform();
        if (os === 'win32') {
            console.log('  Run: reg delete HKEY_CLASSES_ROOT\\kdb /f');
        } else if (os === 'darwin') {
            console.log('  Edit: ~/Library/Preferences/com.apple.LaunchServices.plist');
        } else {
            console.log('  Remove: ~/.local/share/applications/kdb.desktop');
            console.log('  Then run: update-desktop-database ~/.local/share/applications/');
        }

        return false;

    } catch (error) {
        console.error(`[kdb] Unregister error: ${error.message}`);
        return false;
    }
}

// Main execution
const args = process.argv.slice(2);

if (args.includes('--unregister')) {
    unregisterProtocol()
        .then(success => process.exit(success ? 0 : 1))
        .catch(() => process.exit(1));
} else if (args.includes('--check')) {
    isRegistered()
        .then(registered => {
            console.log(`[kdb] Protocol registered: ${registered}`);
            process.exit(registered ? 0 : 1);
        })
        .catch(() => process.exit(1));
} else if (args.includes('--help') || args.includes('-h')) {
    console.log(`
kdb Protocol Registration

Usage: register-protocol.js [options]

Options:
  --help, -h       Show this help
  --unregister     Unregister kdb:// protocol
  --check          Check if protocol is registered

Examples:
  node register-protocol.js           # Register protocol
  node register-protocol.js --check   # Check registration
  node register-protocol.js --unregister  # Unregister

The kdb:// protocol enables one-click terminal automation from web pages.
When a user clicks a kdb://setup?license=XXX link, their terminal opens
with the setup command ready for confirmation.
`);
    process.exit(0);
} else {
    // Default: register
    registerProtocol()
        .then(success => {
            // Non-fatal - exit 0 even if registration fails
            process.exit(0);
        })
        .catch(error => {
            console.error(`[kdb] Unexpected error: ${error.message}`);
            // Non-fatal - exit 0 even on error
            process.exit(0);
        });
}
