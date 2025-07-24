#!/bin/bash
set -e

# Step 1: Clean previous builds
echo "[1/5] Cleaning previous builds..."
rm -rf dist/ build/ termcall.egg-info/

# Step 2: Build the package
echo "[2/5] Building the package (sdist and wheel)..."
python3 -m pip install --upgrade setuptools wheel > /dev/null
python3 setup.py sdist bdist_wheel

# Step 3: Install the package locally
echo "[3/5] Installing the package locally..."
pip3 install --force-reinstall dist/termcall-*.whl

# Step 4: Run 'termcall --help' to verify CLI works
echo "[4/5] Verifying CLI with 'termcall --help'..."
termcall --help

# Step 5: Done
echo "[5/5] Build, install, and verification complete!" 