#!/bin/bash
# install_termcall.sh
# Usage: bash install_termcall.sh
# Cleans up old builds, dependencies, and installs TermCall in editable mode

set -e

# Uninstall old termcall
echo "Uninstalling old termcall..."
pip uninstall -y termcall || true

# Remove build artifacts and caches
echo "Cleaning build artifacts and caches..."
rm -rf build dist *.egg-info src/termcall/__pycache__ ~/.termcall || true

# Install required dependencies
echo "Installing dependencies (pyrebase4, keyring, cryptography)..."
pip install pyrebase4 keyring cryptography

# Install package in editable mode
echo "Installing termcall in editable mode..."
pip install -e .

echo "TermCall installation complete!" 