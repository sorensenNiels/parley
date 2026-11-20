"""A fake TN5250 host that shows one screen of GUI structured fields (Phase 22: R-DS-11,
R-DS-12, D57) and prints what Parley sends back, as no IBM i at hand sends them.

    python3 scripts/gui-demo-host.py [PORT]      # default 52350

Then in Parley 5250: a session to 127.0.0.1, that port, with "GUI elements (beta)" on. The
screen has a menu bar, radio buttons, check boxes, a list with a scroll bar, a window (R-DS-10)
and push buttons;
each record Parley sends is printed in hex. One connection, then it ends."""
import socket
import sys

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 52350
IAC, SB, SE, DO, EOR = 255, 250, 240, 253, 239


def e(text):
    return list(text.encode("cp037"))


def wdsf(kind, rest):
    body = [0xD9, kind] + rest
    n = 2 + len(body)
    return [0x15, n >> 8, n & 0xFF] + body


def minor(rest):
    return [len(rest) + 1] + rest


def sba(row, col):
    return [0x11, row, col]


def choices(items):
    out = []
    for flags1, text, extra in items:
        out += minor([0x10, flags1, 0x00, 0x80] + extra + e(text))
    return out


def selection(flags1, kind, gui, text_size, rows, cols, padding, items):
    major = [flags1, 0x00, 0x00, kind, gui, 0, 0, 0, 0, text_size, rows, cols, padding, 0, 0, 0]
    return wdsf(0x50, major + choices(items))


orders = []
# A menu bar at row 1: auto-enter, no indicators, mnemonics.
orders += sba(1, 2) + selection(0x04, 0x01, 0xF1, 6, 1, 3, 1, [
    (0x08, "File", [0]), (0x08, "Edit", [0]), (0x08, "Help", [0])])
orders += sba(3, 2) + [0x22] + e("Parley GUI elements (R-DS-11, R-DS-12)") + [0x20]
orders += sba(5, 2) + e("Ship by:")
# Radio buttons, three in a row, "Sea" selected.
orders += sba(5, 12) + selection(0x00, 0x11, 0x01, 5, 1, 3, 2, [
    (0x00, "Air", []), (0x80, "Sea", []), (0x00, "Road", [])])
orders += sba(8, 2) + e("Extras:")
# Check boxes, one a row: "Gift wrap" not available, "Tracking" selected.
orders += sba(8, 12) + selection(0x00, 0x12, 0x01, 10, 3, 1, 0, [
    (0x00, "Insurance", []), (0xC0, "Gift wrap", []), (0x80, "Tracking", [])])
# A list with a scroll bar beside it.
for i in range(9):
    orders += sba(12 + i, 4) + e(f"Item {11 + i:03}   Æble æøå {i * 7:>4}")
orders += sba(12, 40) + wdsf(0x53, [0x00, 0x00, 0, 0, 0, 50, 0, 0, 0, 10, 9])
# A window (R-DS-10) with a centred title and a line inside.
orders += sba(13, 48) + wdsf(0x51, [0x00, 0x00, 0x00, 3, 22] + minor([0x10, 0x00, 0x00, 0x00, 0x00] + e("Hjælp")))
orders += sba(15, 52) + e("Rows 11-19 of 50")
# Push buttons: OK sends Enter, Cancel sends F12.
orders += sba(22, 30) + selection(0x00, 0x41, 0x21, 8, 1, 2, 2, [
    (0x0C, "OK", [0, 0xF1]), (0x0C, "Cancel", [0, 0x3C])])
data = [0x04, 0x40, 0x04, 0x11, 0x00, 0x00] + orders + [0x13, 5, 16, 0x04, 0x52, 0x00, 0x00]


def frame(record_data):
    record = [0, 0, 0x12, 0xA0, 0, 0, 4, 0, 0, 0x03] + record_data
    record[0], record[1] = len(record) >> 8, len(record) & 0xFF
    out = []
    for b in record:
        out += [b, b] if b == IAC else [b]
    return bytes(out + [IAC, EOR])


listener = socket.socket()
listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
listener.bind(("127.0.0.1", PORT))
listener.listen(1)
print(f"listening on {PORT}", flush=True)
conn, _ = listener.accept()
conn.sendall(bytes([IAC, DO, 24, IAC, SB, 24, 1, IAC, SE]))
conn.settimeout(5)
try:
    conn.recv(4096)
except socket.timeout:
    pass
conn.sendall(frame(data))
conn.settimeout(60)
try:
    while chunk := conn.recv(4096):
        print("client sent:", chunk.hex(" "), flush=True)
except socket.timeout:
    pass
