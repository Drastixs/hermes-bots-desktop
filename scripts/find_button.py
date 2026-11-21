"""Print the centre of the first solid accent (#ffbd38) button in a screenshot (for e2e clicks).
A button is a run of >= 60 accent pixels on 3 rows in a row."""
import subprocess, sys
w, h = 1280, 800
raw = subprocess.run(["convert", sys.argv[1], "-depth", "8", "rgb:-"], capture_output=True, check=True).stdout
def acc(x, y):
    i = (y * w + x) * 3
    r, g, b = raw[i], raw[i + 1], raw[i + 2]
    return r > 240 and 175 < g < 205 and b < 80
def run_at(y):
    best, start = None, None
    for x in range(280, w):
        if acc(x, y):
            start = x if start is None else start
            if x - start >= 60:
                return start
        else:
            start = None
    return None
for y in range(100, h - 90):
    x = run_at(y)
    if x is not None and run_at(y + 2) is not None and run_at(y + 4) is not None:
        print(x + 40, y + 10); sys.exit(0)
sys.exit(1)
