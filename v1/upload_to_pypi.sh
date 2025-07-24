#!/bin/bash
set -e

echo "Uploading package to PyPI using twine..."
twine upload dist/*
echo "Upload complete!" 