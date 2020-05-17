# cmri_ip_to_485

![Build](https://github.com/sciguy16/cmri_ip_to_485/workflows/Build/badge.svg?branch=master)

Userland service that runs on a Raspberry Pi and acts as a proxy between JMRI on TCP and a C/MRI network running over RS485.

## Usage
Starting the program with `cargo run --release` will start a TCP listener on localhost port 4000

## Installation and removal
There are two handy scripts - `install.sh` and `uninstall.sh`. The installation script checks that Cargo is installed and then compiles the binary and copies it to `/usr/local/bin`. It then installs `ip_to_485` as a SystemD service to run at boot and starts it off.

Similarly, `uninstall.sh` stops and deregisters the SystemD service and removes the program from `/usr/local/bin`.