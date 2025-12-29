#!/bin/bash
# Verify agentReagents setup is complete

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  Verifying agentReagents Setup                                       ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

ERRORS=0
WARNINGS=0

# Check directory structure
echo "📁 Checking directory structure..."
for dir in isos images/base images/cloud images/intermediates images/templates debs/remote-desktop bins tars configs scripts; do
    if [ -d "${REAGENTS_ROOT}/${dir}" ]; then
        echo "  ✅ ${dir}/"
    else
        echo "  ❌ ${dir}/ MISSING"
        ((ERRORS++))
    fi
done

# Check ISOs
echo ""
echo "💿 Checking ISOs..."
for iso in "pop-os_22.04_amd64_nvidia_22.iso" "pop-os_24.04_amd64_nvidia_22.iso" "ubuntu-24.04.3-desktop-amd64.iso"; do
    if [ -f "${REAGENTS_ROOT}/isos/${iso}" ]; then
        SIZE=$(du -h "${REAGENTS_ROOT}/isos/${iso}" | cut -f1)
        echo "  ✅ ${iso} (${SIZE})"
    else
        echo "  ⚠️  ${iso} MISSING (run: bash scripts/download-isos.sh)"
        ((WARNINGS++))
    fi
done

# Check cloud images
echo ""
echo "☁️  Checking cloud images..."
for img in "ubuntu-24.04-server-cloudimg-amd64.img" "ubuntu-22.04-server-cloudimg-amd64.img"; do
    if [ -f "${REAGENTS_ROOT}/images/cloud/${img}" ]; then
        SIZE=$(du -h "${REAGENTS_ROOT}/images/cloud/${img}" | cut -f1)
        echo "  ✅ ${img} (${SIZE})"
    else
        echo "  ⚠️  ${img} MISSING (run: bash scripts/download-cloud-images.sh)"
        ((WARNINGS++))
    fi
done

# Check packages
echo ""
echo "📦 Checking packages..."
if [ -f "${REAGENTS_ROOT}/debs/remote-desktop/rustdesk-1.2.3-x86_64.deb" ]; then
    SIZE=$(du -h "${REAGENTS_ROOT}/debs/remote-desktop/rustdesk-1.2.3-x86_64.deb" | cut -f1)
    echo "  ✅ rustdesk-1.2.3-x86_64.deb (${SIZE})"
else
    echo "  ⚠️  rustdesk-1.2.3-x86_64.deb MISSING (run: bash scripts/download-packages.sh)"
    ((WARNINGS++))
fi

# Check templates (optional)
echo ""
echo "🖼️  Checking templates (optional)..."
TEMPLATE_COUNT=$(find "${REAGENTS_ROOT}/images/templates" -name "*.qcow2" 2>/dev/null | wc -l)
if [ $TEMPLATE_COUNT -gt 0 ]; then
    echo "  ✅ Found ${TEMPLATE_COUNT} template(s)"
    find "${REAGENTS_ROOT}/images/templates" -name "*.qcow2" -exec du -h {} \; | sed 's/^/     /'
else
    echo "  ℹ️  No templates built yet (optional: run build scripts)"
fi

# Check scripts are executable
echo ""
echo "🔧 Checking scripts..."
for script in setup-reagents.sh download-isos.sh download-cloud-images.sh download-packages.sh verify-setup.sh; do
    if [ -x "${REAGENTS_ROOT}/scripts/${script}" ]; then
        echo "  ✅ ${script} (executable)"
    else
        echo "  ⚠️  ${script} (not executable, fixing...)"
        chmod +x "${REAGENTS_ROOT}/scripts/${script}" 2>/dev/null || true
    fi
done

# Summary
echo ""
echo "═══════════════════════════════════════════════════════════════════════"
if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo "✅ Setup Complete! All required components present."
    echo ""
    echo "Next steps:"
    echo "  • Build templates: sudo bash scripts/build-cosmic-cloud-automated.sh"
    echo "  • Run validation: cd ../ionChannel && cargo run --bin ab-validation"
    exit 0
elif [ $ERRORS -eq 0 ]; then
    echo "⚠️  Setup Mostly Complete (${WARNINGS} warnings)"
    echo ""
    echo "Optional downloads missing. Run:"
    echo "  bash scripts/setup-reagents.sh"
    exit 0
else
    echo "❌ Setup Incomplete (${ERRORS} errors, ${WARNINGS} warnings)"
    echo ""
    echo "Fix errors by running:"
    echo "  bash scripts/setup-reagents.sh"
    exit 1
fi

