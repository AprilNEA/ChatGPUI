#!/bin/bash

# SPDX-FileCopyrightText: 2026 AprilNEA LLC
#
# SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

# Retrieve the newly stored file
STAGED_FILES=$(git diff --cached --name-only --diff-filter=A | grep -E '\.(ts|tsx|js|jsx|py|go|rs)$')

if [ -z "$STAGED_FILES" ]; then
    exit 0
fi

for FILE in $STAGED_FILES; do
    # Check if SPDX header already exists
    if ! grep -q "SPDX-License-Identifier" "$FILE"; then
        echo "Adding SPDX header to: $FILE"
        reuse annotate \
            --license "AGPL-3.0-only OR LicenseRef-Commercial" \
            --copyright "AprilNEA LLC" \
            "$FILE"
        git add "$FILE"
    fi
done
