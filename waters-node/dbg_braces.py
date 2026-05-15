with open("/home/constructor/projects/waters-node/src/gossip.rs") as f:
    lines = f.readlines()
in_str = False
depth = 0
for i in range(185, 240):
    line = lines[i - 1]
    opens = 0
    closes = 0
    for j, ch in enumerate(line):
        if ch == chr(34) and (j == 0 or line[j-1] != chr(92)):
            in_str = not in_str
        if not in_str:
            if ch == chr(123):
                opens += 1
            elif ch == chr(125):
                closes += 1
    depth += opens - closes
    m = " <<<" if opens > 0 or closes > 0 else ""
    print(f"{i:3d} d={depth:2d} {line.rstrip()[:120]}{m}")
