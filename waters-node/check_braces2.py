import sys
with open(sys.argv[1]) as f:
    content = f.read()
depth = 0
in_string = False
for i, ch in enumerate(content):
    if ch == chr(34) and (i == 0 or content[i-1] != chr(92)):
        in_string = not in_string
    if not in_string:
        if ch == '{':
            depth += 1
        elif ch == '}':
            depth -= 1
    if depth < 0:
        ln = content[:i].count(chr(10)) + 1
        print(f"Extra brace at line {ln}")
        sys.exit(1)
print(f"OK depth={depth}")
