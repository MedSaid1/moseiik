#!/bin/bash

echo "Running tests..."
cargo test --release

# Message en cas de succès
if [ $? -eq 0 ]; then
    echo "All tests passed successfully!"
else
    echo "Some tests failed. Please review the output."
fi
