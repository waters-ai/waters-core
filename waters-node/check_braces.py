import sys
with open(sys.argv[1]) as f:
    content = f.read()

depth = 0
for i, ch in enumerate(content):
    if ch == '{':
        depth += 1
    elif ch == '}':
        depth -= 1
    if depth < 0:
        line_num = content[:i].count('\n') + 1
        print(f"Extra brace at line {line_num}, col offset {i}")
        sys.exit(1)

print(f"All balanced, final depth={depth}")
