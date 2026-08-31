#!/usr/bin/env python3
"""Generate a large synthetic CSV for manually exercising the SchemaCast CLI
at scale. Output is synthetic only; never point this at real customer data.

Usage: scripts/gen_large_csv.py <output-path> [row-count]
"""
import sys

def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)
    out_path = sys.argv[1]
    rows = int(sys.argv[2]) if len(sys.argv) > 2 else 5_000_000

    with open(out_path, "w") as f:
        f.write("id,score,category,is_active\n")
        for i in range(rows):
            f.write(f"{i},{i % 1000}.5,category_{i % 50},{'true' if i % 2 == 0 else 'false'}\n")

if __name__ == "__main__":
    main()
