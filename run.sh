#!/bin/bash
cd "$(dirname "$0")"

if command -v x-terminal-emulator >/dev/null 2>&1; then
    x-terminal-emulator -e ./target/release/cant_see
elif command -v gnome-terminal >/dev/null 2>&1; then
    gnome-terminal -- ./target/release/cant_see
elif command -v konsole >/dev/null 2>&1; then
    konsole -e ./target/release/cant_see
elif command -v xterm >/dev/null 2>&1; then
    xterm -e ./target/release/cant_see
else
    ./target/release/cant_see
fi