#!/bin/bash
set -e

# Loopal binary is pre-copied to /installed-agent/loopal by the adapter.
chmod +x /installed-agent/loopal
cp /installed-agent/loopal /usr/local/bin/loopal

# Verify installation
loopal --help > /dev/null 2>&1 && echo "Loopal installed successfully" || echo "INSTALL_FAIL_STATUS"
