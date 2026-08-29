#!/bin/sh
set -eu

stty -echo
printf 'runner-ready\r\n'
IFS= read -r line
printf 'input:%s\r\n' "${line}"
