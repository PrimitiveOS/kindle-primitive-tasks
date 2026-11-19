#!/bin/sh
# stage.sh [armhf|armel|RUST_TARGET] [STAGE_DIR]
set -eu
cd "$(dirname "$0")/.."
case "${1:-armhf}" in
    armhf|armv7-unknown-linux-musleabihf) target=armv7-unknown-linux-musleabihf ;;
    armel|armv7-unknown-linux-musleabi) target=armv7-unknown-linux-musleabi ;;
    *) echo "Unsupported package architecture: $1" >&2; exit 1 ;;
esac
stage=${2:-build/stage}
target_dir=${CARGO_TARGET_DIR:-target}
bin=$target_dir/$target/release/tasks-kindle
[ -x "$bin" ] || { echo "Build $bin before staging." >&2; exit 1; }
install -d "$stage/tasks/bin" "$stage/tasks/licenses" "$stage/documents"
install -m 755 "$bin" "$stage/tasks/bin/tasks-kindle"
install -m 755 deploy/Tasks.sh "$stage/documents/Tasks.sh"
install -m 644 licenses/chrono-LICENSE.txt "$stage/tasks/licenses/chrono-LICENSE.txt"
