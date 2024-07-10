#!/bin/bash
# set -x

root_dir=$(git rev-parse --show-toplevel)
cur_dir=$root_dir/tests/e2e
source $cur_dir/util.sh

cleanup() {
  echo "Cleaning up..."
  kill -SIGINT $server_pid
  kill -SIGINT $client_pid
}

trap cleanup EXIT

exec ./target/debug/tunneld &
server_pid=$!
wait_port 6610

exec ./target/debug/tunnel tcp 53 --remote-port 10053 --local-addr 8.8.8.8 &
client_pid=$!
wait_port 10053

output=$(dig @127.0.0.1 -p 10053 +tcp +noedns google.com)
if echo "$output" | grep -q "ANSWER SECTION"; then
    echo "Test passed: ANSWER SECTION found in dig output"
else
    echo "Test failed: ANSWER SECTION not found in dig output"
    echo "Output:"
    echo "$output"
    exit 1
fi
