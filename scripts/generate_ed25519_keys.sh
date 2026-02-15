#!/bin/bash
# Script to generate Ed25519 keys for testing HTTP signatures in PKCS#8 v2 format
# This format is required for use with ring's Ed25519KeyPair::from_pkcs8() method
# Usage: ./generate_ed25519_keys.sh [output_directory]

set -e

# Set output directory (default to current directory)
OUTPUT_DIR=${1:-.}
mkdir -p "$OUTPUT_DIR"

PRIVATE_KEY_PATH="$OUTPUT_DIR/ed25519_private_key.pem"
PUBLIC_KEY_PATH="$OUTPUT_DIR/ed25519_public_key.pem"
TEMP_DER_PATH="$OUTPUT_DIR/temp_key.der"

# Check OpenSSL version
OPENSSL_VERSION=$(openssl version | awk '{print $2}')
OPENSSL_MAJOR=$(echo $OPENSSL_VERSION | cut -d. -f1)

echo "Generating Ed25519 keys in PKCS#8 v2 format..."

# For OpenSSL 3.0+, the standard command generates PKCS#8 v2 format
if [ "$OPENSSL_MAJOR" -ge "3" ]; then
    echo "Using OpenSSL 3.x native PKCS#8 v2 generation..."
    openssl genpkey -algorithm ED25519 -out "$PRIVATE_KEY_PATH"
else
    # For older OpenSSL versions, we need a specific approach to get PKCS#8 v2
    echo "Using OpenSSL $OPENSSL_VERSION with PKCS#8 v2 conversion..."
    
    # Generate raw Ed25519 key
    openssl genpkey -algorithm ED25519 -outform DER -out "$TEMP_DER_PATH"
    
    # Convert to PKCS#8 v2 format with explicit v2 flag
    openssl pkcs8 -topk8 -inform DER -outform PEM -in "$TEMP_DER_PATH" -out "$PRIVATE_KEY_PATH" -nocrypt -v2 prf
    
    # Remove temporary file
    rm -f "$TEMP_DER_PATH"
fi

# Extract public key
echo "Extracting Ed25519 public key..."
openssl pkey -in "$PRIVATE_KEY_PATH" -pubout -out "$PUBLIC_KEY_PATH"

# Verify the key format (look for PKCS#8 v2 indicators)
echo -e "\nVerifying PKCS#8 format version:"
openssl asn1parse -in "$PRIVATE_KEY_PATH" | grep "PRIVATE KEY"

# Display key information
echo -e "\nPrivate key (PKCS#8 v2 format) generated at: $PRIVATE_KEY_PATH"
echo "Public key generated at: $PUBLIC_KEY_PATH"

echo -e "\nPrivate key info:"
openssl pkey -in "$PRIVATE_KEY_PATH" -text -noout

echo -e "\nPublic key info:"
openssl pkey -in "$PUBLIC_KEY_PATH" -pubin -text -noout

# Make the keys readable only by the owner
chmod 600 "$PRIVATE_KEY_PATH"
chmod 644 "$PUBLIC_KEY_PATH"

echo -e "\nKeys generated successfully in PKCS#8 v2 format!"
echo "This format includes both private and public key components in the private key file,"
echo "which is required for ring's Ed25519KeyPair::from_pkcs8() method."

# Instructions for testing
echo -e "\nTo use with the HttpSignature tests:"
echo "1. Copy these files to the test-data directory"
echo "2. Rename them to ed25519_test_key.pem and ed25519_test_public_key.pem"
