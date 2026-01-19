#!/bin/bash
# SPDX-FileCopyrightText: 2026 AprilNEA LLC
#
# SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

set -e

echo "Stopping PostgreSQL processes..."
pkill -f postgres 2>/dev/null || true
sleep 2

echo "Cleaning up shared memory..."
ipcrm -a 2>/dev/null || true

echo "Removing database lock file..."
rm -f "$HOME/Library/Application Support/chatgpui/db/data/postmaster.pid" 2>/dev/null || true

echo "Cleanup complete!"
