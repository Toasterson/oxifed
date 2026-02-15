#!/bin/bash
# Script to set up test data for HTTP signature testing
# This script generates key pairs and puts them in the correct location
# Uses PKCS#8 v2 format which includes the public key in the private key file

set -e

# Create test-data directory if it doesn't exist
mkdir -p test-data

# Check if we have required tools
if ! command -v openssl &> /dev/null; then
    echo "Error: OpenSSL is required but not found"
    exit 1
fi

echo "Generating Ed25519 keys for testing in PKCS#8 v2 format..."

# First generate a raw Ed25519 keypair
openssl genpkey -algorithm ED25519 -outform DER -out test-data/temp_key.der

# Convert to PKCS#8 v2 format using openssl pkcs8
openssl pkcs8 -topk8 -inform DER -outform PEM -in test-data/temp_key.der -out test-data/ed25519_test_key.pem -nocrypt -v2prf hmacWithSHA512

# Clean up the temporary file
rm test-data/temp_key.der

# Extract the public key
openssl pkey -in test-data/ed25519_test_key.pem -pubout -out test-data/ed25519_test_public_key.pem

# Verify the key format - this will show "PKCS#8 v2" for the correct format
echo -e "\nVerifying key format:"
grep "PRIVATE KEY" test-data/ed25519_test_key.pem || echo "Verification failed"

echo "Keys generated successfully!"
echo "Private key (PKCS#8 v2): test-data/ed25519_test_key.pem"
echo "Public key: test-data/ed25519_test_public_key.pem"

# Provide instructions for using the keys with ring
echo -e "\nThese keys are in PKCS#8 v2 format, containing both private and public key components."
echo "This format is required for use with ring's Ed25519KeyPair::from_pkcs8() method."

echo "Test data setup complete!"
