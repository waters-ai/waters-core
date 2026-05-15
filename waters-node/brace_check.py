import sys, re
with open(sys.argv[1]) as f:
    content = f.read()
no_str = re.sub(r'"(?:[^"\\]|\\.)*"', '', content)
depth = 0
for i, ch in enumerate(no_str):
    if ch == '{':
        depth += 1
    elif ch == '}':
        depth -= 1
    if depth < 0:
        ln = content[:i].count('\n') + 1
        print(f'Extra brace at line {ln}')
        sys.exit(1)
if depth > 0:
    print(f'Final depth: {depth} (unclosed braces)')
    # Find unclosed by checking each block
    depth = 0
    last_open = 0
    for ln, line in enumerate(content.split('\n'), 1):
        opens = line.count('{')
        closes = line.count('}')
        depth += opens - closes
        if depth > 0 and opens > closes:
            last_open = ln
    print(f'Last opening at line: {last_open}')
else:
    print('All balanced!')
