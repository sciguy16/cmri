#!/bin/bash

echo "Stopping the service..."
sudo systemctl stop ip_to_485.service
sudo systemctl disable ip_to_485.service
sudo systemctl status ip_to_485.service

echo "Removing systemd service..."
sudo rm -v /lib/systemd/system/ip_to_485.service

echo "Removing from /usr/local/bin..."
sudo rm -v /usr/local/bin/ip_to_485
