import sys
with open(sys.argv[1]) as f:
    lines = f.readlines()
depth = 0
base = None
for i, line in enumerate(lines, 1):
    opens = line.count('{')
    closes = line.count('}')
    depth += opens - closes
    if i == 73:
        base = depth
    if base is not None and depth > base + 1 and i < 532:
        print(f"Line {i}: depth={depth} (base={base}): {line.rstrip()[:80]}")
