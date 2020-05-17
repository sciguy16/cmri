#!/bin/bash

echo "Checking for updates..."
git pull

echo "Checking for Cargo..."
which cargo || sudo apt install cargo

echo "Compiling..."
cargo build --release

echo "Installing to /usr/local/bin..."
sudo mkdir -p /usr/local/bin
sudo cp target/release/ip_to_485 /usr/local/bin/

echo "Installing systemd service..."
sudo cp ip_to_485.service /lib/systemd/system/

echo "Starting the service..."
sudo systemctl enable ip_to_485.service
sudo systemctl start ip_to_485.service
sudo systemctl status ip_to_485.service