#!/bin/bash
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "usage: $0 <cargo-cmd> [args...]" >&2
  exit 1
fi

cmd=$1
shift

if [ "${APC_DISABLE_CHECKSUM:-0}" = "1" ]; then
  need_flag=1
  for arg in "$@"; do
    if [ "$arg" = "--no-default-features" ]; then
      need_flag=0
      break
    fi
  done
  if [ $need_flag -eq 1 ]; then
    exec cargo "$cmd" --no-default-features "$@"
  else
    exec cargo "$cmd" "$@"
  fi
else
  exec cargo "$cmd" "$@"
fi
