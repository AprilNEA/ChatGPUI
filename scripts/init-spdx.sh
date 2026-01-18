#!/bin/bash

# SPDX-FileCopyrightText: 2026 AprilNEA LLC
#
# SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

set -e

COPYRIGHT="AprilNEA LLC"
LICENSE="AGPL-3.0-only OR LicenseRef-Commercial"

echo "Starting to add SPDX header..."

# Rust
find . -name "*.rs" -not -path "./target/*" \
    -exec reuse annotate --license "$LICENSE" --copyright "$COPYRIGHT" {} \;

echo "Done! Run reuse lint to verify..."
reuse lint
