#!/bin/bash
# Verify agentReagents setup is complete

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
source "$SCRIPT_DIR/../configs/defaults.env" 2>/dev/null || source "${REAGENTS_ROOT:-$(dirname "$SCRIPT_DIR")}/configs/defaults.env" 2>/dev/null || true

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  Verifying agentReagents Setup                                       ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

ERRORS=0
WARNINGS=0

# Check directory structure
echo "Checking directory structure..."
for dir in isos images/base images/cloud images/intermediates images/templates debs/remote-desktop bins tars configs scripts; do
    if [ -d "${REAGENTS_ROOT}/${dir}" ]; then
        echo "  OK: ${dir}/"
    else
        echo "  MISSING: ${dir}/"
        ERRORS=$((ERRORS + 1))
    fi
done

# Check ISOs
echo ""
echo "Checking ISOs..."
for iso in "pop-os_22.04_amd64_nvidia_22.iso" "pop-os_24.04_amd64_nvidia_22.iso" "ubuntu-24.04.3-desktop-amd64.iso"; do
    if [ -f "${REAGENTS_ROOT}/isos/${iso}" ]; then
        SIZE=$(du -h "${REAGENTS_ROOT}/isos/${iso}" | cut -f1)
        echo "  OK: ${iso} (${SIZE})"
    else
        echo "  WARN: ${iso} MISSING (run: bash scripts/download-isos.sh)"
        WARNINGS=$((WARNINGS + 1))
    fi
done

# Check cloud images
echo ""
echo "Checking cloud images..."
for img in "${CLOUD_IMAGE_UBUNTU_2404:-ubuntu-24.04-server-cloudimg-amd64.img}" "${CLOUD_IMAGE_UBUNTU_2204:-ubuntu-22.04-server-cloudimg-amd64.img}"; do
    if [ -f "${REAGENTS_ROOT}/images/cloud/${img}" ]; then
        SIZE=$(du -h "${REAGENTS_ROOT}/images/cloud/${img}" | cut -f1)
        echo "  OK: ${img} (${SIZE})"
    else
        echo "  WARN: ${img} MISSING (run: bash scripts/download-cloud-images.sh)"
        WARNINGS=$((WARNINGS + 1))
    fi
done

# Check packages
echo ""
echo "Checking packages..."
if [ -f "${REAGENTS_ROOT}/debs/remote-desktop/rustdesk-1.2.3-x86_64.deb" ]; then
    SIZE=$(du -h "${REAGENTS_ROOT}/debs/remote-desktop/rustdesk-1.2.3-x86_64.deb" | cut -f1)
    echo "  OK: rustdesk-1.2.3-x86_64.deb (${SIZE})"
else
    echo "  WARN: rustdesk-1.2.3-x86_64.deb MISSING (run: bash scripts/download-packages.sh)"
    WARNINGS=$((WARNINGS + 1))
fi

# Check templates (optional)
echo ""
echo "Checking templates (optional)..."
TEMPLATE_COUNT=$(find "${REAGENTS_ROOT}/images/templates" -name "*.qcow2" 2>/dev/null | wc -l)
if [ "$TEMPLATE_COUNT" -gt 0 ]; then
    echo "  Found ${TEMPLATE_COUNT} template(s)"
    find "${REAGENTS_ROOT}/images/templates" -name "*.qcow2" -exec du -h {} \; | sed 's/^/     /'
else
    echo "  No templates built yet (optional: run build scripts)"
fi

# Check scripts are executable
echo ""
echo "Checking scripts..."
for script in setup-reagents.sh download-isos.sh download-cloud-images.sh download-packages.sh verify-setup.sh; do
    if [ -x "${REAGENTS_ROOT}/scripts/${script}" ]; then
        echo "  OK: ${script} (executable)"
    else
        echo "  WARN: ${script} (not executable, fixing...)"
        chmod +x "${REAGENTS_ROOT}/scripts/${script}" 2>/dev/null || true
    fi
done

# Check configs for placeholder values
echo ""
echo "Checking configs..."
if [ -f "${REAGENTS_ROOT}/configs/ecoprimals-node.yaml" ]; then
    if grep -q "CHANGEME" "${REAGENTS_ROOT}/configs/ecoprimals-node.yaml"; then
        echo "  WARN: ecoprimals-node.yaml contains CHANGEME placeholder SSH key"
        echo "        Replace with your actual key before production use"
        WARNINGS=$((WARNINGS + 1))
    else
        echo "  OK: ecoprimals-node.yaml (SSH key configured)"
    fi
else
    echo "  WARN: ecoprimals-node.yaml not found"
    WARNINGS=$((WARNINGS + 1))
fi

# SHA256 checksum verification
echo ""
echo "Verifying checksums..."
CHECKSUMS_FILE="${REAGENTS_ROOT}/docs/CHECKSUMS.md"
CHECKSUM_PASS=0
CHECKSUM_FAIL=0
CHECKSUM_SKIP=0

if [ -f "$CHECKSUMS_FILE" ]; then
    while IFS= read -r line || [[ -n "${line}" ]]; do
        # Only parse lines that look like sha256 checksums (64 hex chars followed by two spaces and a path)
        if echo "$line" | grep -qE '^[0-9a-f]{64}  '; then
            expected_hash=$(echo "$line" | awk '{print $1}')
            filepath=$(echo "$line" | awk '{print $2}')
            if [ -f "${REAGENTS_ROOT}/${filepath}" ]; then
                actual_hash=$(sha256sum "${REAGENTS_ROOT}/${filepath}" | awk '{print $1}')
                if [ "$expected_hash" = "$actual_hash" ]; then
                    echo "  PASS: ${filepath}"
                    CHECKSUM_PASS=$((CHECKSUM_PASS + 1))
                else
                    echo "  FAIL: ${filepath} (checksum mismatch)"
                    echo "        expected: ${expected_hash}"
                    echo "        actual:   ${actual_hash}"
                    CHECKSUM_FAIL=$((CHECKSUM_FAIL + 1))
                fi
            else
                echo "  SKIP: ${filepath} (not downloaded)"
                CHECKSUM_SKIP=$((CHECKSUM_SKIP + 1))
            fi
        fi
    done < "$CHECKSUMS_FILE"

    if [ "$CHECKSUM_PASS" -eq 0 ] && [ "$CHECKSUM_FAIL" -eq 0 ] && [ "$CHECKSUM_SKIP" -eq 0 ]; then
        echo "  No checksums found in docs/CHECKSUMS.md"
    else
        echo "  Summary: ${CHECKSUM_PASS} passed, ${CHECKSUM_FAIL} failed, ${CHECKSUM_SKIP} skipped"
    fi

    if [ "$CHECKSUM_FAIL" -gt 0 ]; then
        ERRORS=$((ERRORS + 1))
    fi
else
    echo "  WARN: docs/CHECKSUMS.md not found"
    WARNINGS=$((WARNINGS + 1))
fi

# Summary
echo ""
echo "═══════════════════════════════════════════════════════════════════════"
if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo "Setup Complete! All required components present."
    echo ""
    echo "Next steps:"
    echo "  - Build templates: sudo bash scripts/build-cosmic-cloud-automated.sh"
    echo "  - Create lab:      cd ../benchScale && ./scripts/create-lab.sh --topology ecoprimals-tower-2node --name test"
    echo "  - Run validation:  cd ../../springs/primalSpring && ./scripts/validate_local_lab.sh"
    exit 0
elif [ $ERRORS -eq 0 ]; then
    echo "Setup Mostly Complete (${WARNINGS} warnings)"
    echo ""
    echo "Optional downloads missing. Run:"
    echo "  bash scripts/setup-reagents.sh"
    exit 0
else
    echo "Setup Incomplete (${ERRORS} errors, ${WARNINGS} warnings)"
    echo ""
    echo "Fix errors by running:"
    echo "  bash scripts/setup-reagents.sh"
    exit 1
fi
