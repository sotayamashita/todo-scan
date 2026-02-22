#!/usr/bin/env bash
set -euo pipefail

BINARY="./target/release/todo-scan"
CORPUS_DIR=$(mktemp -d)
NUM_FILES=1000
RUNS=5

# Generate test corpus
echo "Generating $NUM_FILES test files..."
for i in $(seq 1 $NUM_FILES); do
  dir="$CORPUS_DIR/src/module_$((i % 20))"
  mkdir -p "$dir"
  cat > "$dir/file_$i.rs" <<RUST
// Module file $i
fn function_$i() {
    // TODO: implement feature $i
    let x = $i;
    // FIXME(author): fix edge case $i
    println!("{}", x);
}
RUST
done

# Build release binary
echo "Building release binary..."
cargo build --release

# Cold scan (--no-cache)
echo ""
echo "=== Cold Scan (--no-cache) ==="
cold_total=0
for r in $(seq 1 $RUNS); do
  start=$(python3 -c 'import time; print(time.time())')
  $BINARY list --no-cache --root "$CORPUS_DIR" > /dev/null
  end=$(python3 -c 'import time; print(time.time())')
  elapsed=$(python3 -c "print(f'{($end - $start)*1000:.1f}')")
  echo "  Run $r: ${elapsed}ms"
  cold_total=$(python3 -c "print($cold_total + $end - $start)")
done
cold_avg=$(python3 -c "print(f'{($cold_total / $RUNS)*1000:.1f}')")

# Warm cache creation (clear any existing cache first)
rm -rf ~/.cache/todo-scan
$BINARY list --root "$CORPUS_DIR" > /dev/null

# Warm scan (cache hit)
echo ""
echo "=== Warm Scan (cached) ==="
warm_total=0
for r in $(seq 1 $RUNS); do
  start=$(python3 -c 'import time; print(time.time())')
  $BINARY list --root "$CORPUS_DIR" > /dev/null
  end=$(python3 -c 'import time; print(time.time())')
  elapsed=$(python3 -c "print(f'{($end - $start)*1000:.1f}')")
  echo "  Run $r: ${elapsed}ms"
  warm_total=$(python3 -c "print($warm_total + $end - $start)")
done
warm_avg=$(python3 -c "print(f'{($warm_total / $RUNS)*1000:.1f}')")

# Partial warm scan (1 file changed)
echo "modified" >> "$CORPUS_DIR/src/module_0/file_1.rs"
echo ""
echo "=== Partial Warm Scan (1 file changed) ==="
partial_total=0
for r in $(seq 1 $RUNS); do
  start=$(python3 -c 'import time; print(time.time())')
  $BINARY list --root "$CORPUS_DIR" > /dev/null
  end=$(python3 -c 'import time; print(time.time())')
  elapsed=$(python3 -c "print(f'{($end - $start)*1000:.1f}')")
  echo "  Run $r: ${elapsed}ms"
  partial_total=$(python3 -c "print($partial_total + $end - $start)")
done
partial_avg=$(python3 -c "print(f'{($partial_total / $RUNS)*1000:.1f}')")

# Results
speedup=$(python3 -c "print(f'{float($cold_avg)/float($warm_avg):.1f}')")
echo ""
echo "=== Results ($NUM_FILES files, avg of $RUNS runs) ==="
echo "| Scenario              | Avg Time  | vs Cold  |"
echo "|-----------------------|-----------|----------|"
echo "| Cold (--no-cache)     | ${cold_avg}ms |   1.0x   |"
echo "| Warm (all cached)     | ${warm_avg}ms |   ${speedup}x   |"
echo "| Partial (1 changed)   | ${partial_avg}ms |          |"

# Cleanup
rm -rf "$CORPUS_DIR"
