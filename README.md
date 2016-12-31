# simplelink
Simple message protocol for KISS based Ham Radio TNCs

This library is a Rust-based implementation of a message protocal over KISS TNCs for amateur radio communication. It is a protocol built for low bandwidth text based communcation.

## Features
* CRC verification of message contents.
* Automatic retry with progressive back-off.
* Congestion control.
* Confirmation of receipt.
* Broadcast and per node routing scheme up to 16 addresses.
* Low overhead Rust implementation with minimal allocation.
* Electron based, cross-platform UI.

## Supported TNCs
* Mobilinkd Bluetooth TNC - http://www.mobilinkd.com/
* Kenwood TM-D710
* KISS compliant TNC
