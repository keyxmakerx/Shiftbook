"""Build the round 15 quality-of-life mockups (one small HTML file per idea).

Usage: python3 tools/signoff/mocks/r15_qol_build.py
Writes tools/signoff/mocks/r15-qol-<id>.html. Each file is one still scene in the Shiftbook
look, 1000 px wide, scaled down to fit narrower frames. All names and tickets are made up.
"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
W = 1000

CSS = """
:root{--desk:#e9ecef;--paper:#fffdf8;--paper-2:#f6f3ec;--ink:#1d2126;--ink-2:#4a525b;--mut:#5f6872;
--rule:rgba(29,33,38,.1);--glass:rgba(255,255,255,.8);--accent:#3d5bb0;--ok:#2e8a5a;--warn:#9f5a06;
--warn-wash:#fcf1df;--bad:#c2412d;--chip:#e4e9f6;--teal:#148c79;--amber:#de9930;--plum:#8a4c7b;
--shadow:0 1px 1px rgba(29,33,38,.06),0 10px 24px -14px rgba(29,33,38,.35);
--lift:0 2px 4px rgba(29,33,38,.08),0 22px 40px -18px rgba(29,33,38,.45);
--ui:'Segoe UI',system-ui,-apple-system,Ubuntu,sans-serif;--display:Georgia,'Times New Roman',serif;
--mono:Consolas,'DejaVu Sans Mono',monospace}
@media (prefers-color-scheme:dark){:root{--desk:#14171b;--paper:#23272d;--paper-2:#2a2f36;--ink:#e7eaee;
--ink-2:#b9c0c8;--mut:#949da7;--rule:rgba(255,255,255,.09);--glass:rgba(28,32,37,.8);--accent:#8ea6ee;
--ok:#6ccb95;--warn:#f0b45a;--warn-wash:#3a2c14;--bad:#f08a78;--chip:#2b3450;--teal:#4cc2ad;
--shadow:0 1px 1px rgba(0,0,0,.4),0 12px 28px -14px rgba(0,0,0,.8);--lift:0 2px 4px rgba(0,0,0,.4),0 24px 44px -16px rgba(0,0,0,.9)}}
*{box-sizing:border-box}html,body{margin:0}
body{background:var(--desk);color:var(--ink);font:14px/1.4 var(--ui);overflow:hidden}
#fit{position:relative;width:100%}
.app{width:%Wpx;transform-origin:0 0;padding:10px 12px 0;display:flex;flex-direction:column;gap:10px;position:absolute;left:0;top:0}
.top{display:flex;align-items:center;gap:10px;background:var(--glass);border-radius:12px;padding:8px 12px;box-shadow:var(--shadow)}
.brand{font:700 18px var(--display)}.brand small{font:600 10px var(--ui);letter-spacing:.1em;color:var(--mut);margin-left:6px}
.cap{flex:0 1 280px;margin-left:auto;border:1px solid var(--rule);border-radius:8px;padding:6px 10px;color:var(--mut);background:var(--paper)}
.cap kbd{float:right}
.pill{border:1px solid var(--rule);border-radius:8px;padding:5px 9px;background:var(--paper);font-weight:600;font-size:12px}
.tabs{display:flex;background:var(--paper-2);border-radius:9px;padding:3px}.tabs span{padding:5px 11px;border-radius:7px;color:var(--ink-2)}
.tabs .on{background:var(--paper);color:var(--ink);font-weight:600;box-shadow:var(--shadow)}
kbd{font:11px var(--mono);border:1px solid var(--rule);border-bottom-width:2px;border-radius:4px;padding:0 5px;background:var(--paper-2);color:var(--ink-2)}
.main{display:flex;gap:10px;align-items:flex-start}
.desk{position:relative;flex:1;height:%Hpx;border-radius:12px;background:var(--desk);overflow:hidden;
background-image:radial-gradient(var(--rule) 1px,transparent 1px);background-size:22px 22px}
.side{width:300px;display:flex;flex-direction:column;gap:10px}
.panel{background:var(--paper);border-radius:10px;box-shadow:var(--shadow);padding:12px 14px;display:flex;flex-direction:column;gap:8px}
.panel h4{margin:0;font:600 13px var(--ui)}.lbl{font:600 10.5px var(--ui);letter-spacing:.09em;text-transform:uppercase;color:var(--mut)}
.card{position:absolute;width:178px;background:var(--paper);border-radius:8px;box-shadow:var(--shadow);padding:9px 11px;border-top:4px solid var(--c,var(--accent));font-size:12.5px}
.card .k{font:600 10px var(--ui);letter-spacing:.06em;text-transform:uppercase;color:var(--mut)}
.card .t{display:block;font-weight:600;font-size:13.5px;margin:2px 0}
.card.dim{opacity:.25}.card.focus{outline:3px solid var(--accent);outline-offset:3px}
.card.jot{--c:var(--amber);background:#fff6df}
@media (prefers-color-scheme:dark){.card.jot{background:#3a3220}}
.tk{display:inline-block;font:600 11.5px var(--mono);background:var(--chip);color:var(--accent);padding:2px 6px;border-radius:5px}
.tk.open::before{content:'\\25CB  ';}
.btn{display:inline-flex;align-items:center;gap:5px;font:600 12.5px var(--ui);border:1px solid var(--rule);background:var(--paper);color:var(--ink);border-radius:7px;padding:6px 10px;white-space:nowrap}
.btn.dark{background:var(--ink);color:var(--paper);border-color:var(--ink)}.btn.mono{font-family:var(--mono)}
.btn.link{border:0;background:none;color:var(--accent);padding:6px 4px}
.pop{position:absolute;background:var(--paper);border-radius:10px;box-shadow:var(--lift);padding:12px 14px;display:flex;flex-direction:column;gap:8px;z-index:5}
.row{display:flex;gap:6px;align-items:center;flex-wrap:wrap}
.menu{position:absolute;background:var(--paper);border-radius:8px;box-shadow:var(--lift);padding:6px;min-width:200px;z-index:6;font-size:13px}
.menu div{padding:6px 10px;border-radius:5px;display:flex;justify-content:space-between;gap:16px}.menu .on{background:var(--chip)}
.menu .sep{height:1px;background:var(--rule);padding:0;margin:4px 0}.menu small{color:var(--mut)}
.bottom{display:flex;align-items:center;gap:8px;background:var(--glass);border-radius:10px 10px 0 0;padding:6px 10px;box-shadow:var(--shadow);font-size:12.5px}
.bottom .r{margin-left:auto;display:flex;gap:10px;align-items:center}
.toast{position:absolute;left:50%;transform:translateX(-50%);bottom:14px;background:var(--ink);color:var(--paper);border-radius:9px;padding:9px 14px;display:flex;gap:14px;align-items:center;font-size:13px;z-index:8;white-space:nowrap}
.toast b{color:var(--paper)}.toast .a{color:#a9bdf5;font-weight:600}
.note{font-size:12px;color:var(--mut)}
.ptr{position:absolute;width:0;height:0;border-left:7px solid transparent;border-right:7px solid transparent;border-bottom:14px solid var(--ink);transform:rotate(-30deg);z-index:9}
.sw{display:inline-block;width:32px;height:18px;border-radius:9px;background:var(--rule);position:relative;vertical-align:middle}
.sw::after{content:'';position:absolute;top:2px;left:2px;width:14px;height:14px;border-radius:50%;background:var(--paper);box-shadow:var(--shadow)}
.sw.on{background:var(--teal)}.sw.on::after{left:16px}
.settings{flex:1;display:flex;gap:10px;height:%Hpx}
.snav{width:180px;background:var(--paper);border-radius:10px;box-shadow:var(--shadow);padding:10px;display:flex;flex-direction:column;gap:2px}
.snav span{padding:6px 10px;border-radius:6px;color:var(--ink-2)}.snav .on{background:var(--chip);color:var(--ink);font-weight:600}
.sbody{flex:1;background:var(--paper);border-radius:10px;box-shadow:var(--shadow);padding:18px 22px;display:flex;flex-direction:column;gap:14px}
.sbody h3{margin:0;font:700 20px var(--display)}
.field{display:grid;grid-template-columns:200px 1fr;gap:10px;align-items:start;padding:10px 0;border-top:1px solid var(--rule)}
.field .h{font-weight:600}.field .h small{display:block;font-weight:400;color:var(--mut);font-size:12px}
.hl{outline:2px dashed var(--accent);outline-offset:6px;border-radius:6px}
.callout{position:absolute;background:var(--accent);color:#fff;font-size:12px;font-weight:600;border-radius:6px;padding:4px 8px;z-index:9;white-space:nowrap}
"""

FIT = """<script>(function(){var a=document.querySelector('.app'),f=document.getElementById('fit');
function s(){var k=Math.min(1,innerWidth/%W);a.style.transform='scale('+k+')';f.style.height=Math.ceil(a.offsetHeight*k)+'px'}
addEventListener('resize',s);s()})()</script>"""


def top(tab="Desk", cap="Log or note something…"):
    tabs = "".join(
        f'<span class="{"on" if t == tab else ""}">{t}</span>' for t in ["Desk", "Day", "Timesheet", "Notes", "Settings"]
    )
    return (
        f'<div class="top"><span class="brand">Shiftbook<small>ALPHA</small></span>'
        f'<span class="cap">{cap}<kbd>Ctrl Space</kbd></span><span class="pill">● Protected</span>'
        f'<span class="pill">Lock</span><div class="tabs">{tabs}</div></div>'
    )


def bottom(extra="", right="5.5 h today · 🔔 3"):
    return f'<div class="bottom"><span class="btn">Fit everything</span><span class="btn">Tidy into stacks</span><span class="btn">Boards</span>{extra}<span class="r">{right}</span></div>'


def card(x, y, kind, title, sub="", c="var(--accent)", cls=""):
    return f'<div class="card {cls}" style="left:{x}px;top:{y}px;--c:{c}"><span class="k">{kind}</span><span class="t">{title}</span>{sub}</div>'


DESK_CARDS = [
    (20, 20, "Entry · 2.5 h", "Switch replacement, floor 2", '<span class="tk">INC0045678</span>', "var(--accent)"),
    (220, 30, "Entry · 1.5 h · Tue", "Laptop imaging", '<span class="note">Laptop refresh</span>', "var(--plum)"),
    (420, 16, "Ticket · still open", "INC0045690", '<span class="note">Printer, tray 2 roller</span>', "var(--teal)"),
    (40, 150, "Entry · 1 h · Mon", "Vendor call about the firewall", "", "var(--mut)"),
    (250, 170, "Page", "Guest Wi-Fi", '<span class="note">Network · wifi</span>', "var(--plum)"),
    (450, 160, "Brought up today", "Call the vendor back", "", "var(--amber)"),
]


def desk_cards(dim=None, focus=None):
    out = []
    for i, (x, y, k, t, s, c) in enumerate(DESK_CARDS):
        cls = ("dim " if dim and i in dim else "") + ("focus" if focus == i else "")
        out.append(card(x, y, k, t, s, c, cls))
    return "".join(out)


def page(body, h=380):
    css = CSS.replace("%W", str(W)).replace("%H", str(h))
    return (
        '<!doctype html><html lang="en"><head><meta charset="utf-8">'
        '<meta name="viewport" content="width=device-width, initial-scale=1"><title>Shiftbook mockup</title>'
        f"<style>{css}</style></head><body><div id=\"fit\"><div class=\"app\">{body}</div></div>"
        + FIT.replace("%W", str(W))
        + "</body></html>"
    )


def side_default():
    return (
        '<div class="side"><div class="panel"><span class="lbl">Today</span><div style="font:700 26px var(--mono)">5.5 <small style="font-size:12px;color:var(--mut)">of 8 h</small></div></div>'
        '<div class="panel"><span class="lbl">Log work</span><div class="cap" style="flex:none;margin:0">Paste a ticket or say what you did</div>'
        '<div class="row"><span class="btn mono">15m</span><span class="btn mono">30m</span><span class="btn mono">45m</span><span class="btn mono">1h</span><span class="btn mono">2h</span></div></div></div>'
    )


MOCKS = {}

# ---------- batch 1 ----------
MOCKS["paste"] = page(
    top()
    + '<div class="main"><div class="desk">'
    + desk_cards()
    + '<div class="ptr" style="left:395px;top:300px"></div>'
    + '<div class="pop" style="left:330px;top:110px;width:300px"><div><span class="tk open">INC0045712</span> <span class="tk open">RITM0012399</span></div>'
    + '<b>Added to Tickets. Time on them so far?</b><div class="row"><span class="btn mono">5m</span><span class="btn mono dark">15m</span><span class="btn mono">30m</span><span class="btn mono">1h</span><span class="btn mono">2h</span></div>'
    + '<span class="note">Enter for 15m · Esc to skip</span></div>'
    + card(380, 300, "Jot · just now", "guest wifi drops again after 5pm", "", "var(--amber)", "jot")
    + '<div class="callout" style="left:150px;top:330px">Pasted text → a jot where the pointer is</div>'
    + "</div>"
    + side_default()
    + "</div>"
    + bottom()
)

MOCKS["find"] = page(
    top(cap="")
    + '<div class="main"><div class="desk" style="opacity:.55">'
    + desk_cards()
    + "</div>"
    + side_default()
    + "</div>"
    + '<div class="pop" style="left:250px;top:70px;width:500px;padding:10px">'
    + '<div style="border:2px solid var(--accent);border-radius:8px;padding:8px 10px;font:15px var(--mono)">fri<span style="opacity:.5">|</span></div>'
    + '<span class="lbl" style="margin-top:4px">Days</span>'
    + '<div class="menu" style="position:static;box-shadow:none;padding:0"><div class="on">Go to Friday 23 October <small>Day · 7.5 h</small></div><div>Timesheet, week of 19 October <small>not entered yet</small></div></div>'
    + '<span class="lbl">Do</span>'
    + '<div class="menu" style="position:static;box-shadow:none;padding:0"><div>Wrap up the day <small>wrap-up</small></div><div>Back up now <small>Backups</small></div><div>Copy yesterday for stand-up <small>clipboard</small></div></div>'
    + '<span class="lbl">Settings</span>'
    + '<div class="menu" style="position:static;box-shadow:none;padding:0"><div>Your day: Friday\'s target <small>Settings</small></div></div>'
    + '<span class="note">↑ ↓ to choose · Enter to go · type "help" for everything Find can do</span></div>'
    + '<div class="callout" style="left:770px;top:82px">Press / or Ctrl+Space</div>'
    + bottom(),
    360,
)

MOCKS["bringup"] = page(
    top()
    + '<div class="main"><div class="desk">'
    + desk_cards(focus=0)
    + '<div class="menu" style="left:205px;top:40px"><div>Open</div><div>Pin to the bottom bar</div><div class="on">Bring up… <small>›</small></div><div>Tick done</div><div class="sep"></div><div>More time</div><div>Archive</div></div>'
    + '<div class="menu" style="left:413px;top:96px;min-width:220px"><div class="on">Tomorrow <small>Mon 26 Oct</small></div><div>Wednesday <small>28 Oct</small></div><div>Friday <small>30 Oct</small></div><div>Next week <small>Mon 2 Nov</small></div><div class="sep"></div><div>Pick a day…</div></div>'
    + "</div>"
    + side_default()
    + "</div>"
    + '<div class="note" style="padding:0 6px">Same short list as a pin\'s chase day. A day that falls on a weekend moves to your next work day, and the list says so.</div>'
    + bottom()
)

MOCKS["standup"] = page(
    top()
    + '<div class="main"><div class="desk">'
    + desk_cards()
    + '<div class="pop" style="left:330px;top:120px;width:300px"><span class="lbl">On your clipboard</span>'
    + '<pre style="margin:0;font:12.5px/1.55 var(--mono);white-space:pre-wrap">Yesterday\n- Switch replacement, floor 2 (INC0045678)\n- Laptop imaging\n- 6 tickets closed\nToday\n- Finish the printer ticket</pre>'
    + '<span class="note">Settings → Your day sets the words: "Yesterday / Today", or your team\'s own.</span></div>'
    + '<div class="toast">Copied 6 lines for stand-up</div>'
    + "</div>"
    + '<div class="side"><div class="panel" style="border-top:4px solid var(--accent)"><span class="lbl">Where was I?</span><h4>Finish the printer ticket</h4>'
    + '<span class="note">From your wrap-up on Friday · <span class="tk open">INC0045690</span></span>'
    + '<div class="row"><span class="btn dark">Carry on</span><span class="btn">Later</span></div>'
    + '<div class="row hl"><span class="btn">⧉ Copy for stand-up</span></div></div></div>'
    + "</div>"
    + bottom()
)

MOCKS["undo"] = page(
    top()
    + '<div class="main"><div class="desk">'
    + desk_cards()
    + '<div class="pop" style="left:150px;top:80px;width:430px;padding:10px"><div style="border:2px solid var(--accent);border-radius:8px;padding:8px 10px;font:15px var(--mono)">undo</div>'
    + '<span class="lbl">Recent changes, newest first</span>'
    + '<div class="menu" style="position:static;box-shadow:none;padding:0"><div class="on">Undo: filed Laptop imaging in Network refresh <small>2 min ago</small></div>'
    + "<div>Undo: ticked INC0045691 done <small>9 min ago</small></div><div>Undo: archived the jot \"order toner\" <small>20 min ago</small></div><div>Undo: moved 3 cards into Site A move <small>1 h ago</small></div></div>"
    + '<span class="note">Undoing an older change also undoes the ones after it, and the list says which.</span></div>'
    + '<div class="toast">Undone: filed Laptop imaging · Ctrl+Z again for the one before <span class="a">Redo</span></div>'
    + "</div>"
    + side_default()
    + "</div>"
    + bottom()
)

MOCKS["times"] = page(
    top(tab="Settings")
    + '<div class="main"><div class="settings"><div class="snav"><span>Look</span><span class="on">Your day</span><span>Projects</span><span>Timesheet</span><span>Notes</span><span>Fields</span><span>Responses</span><span>Meetings</span><span>Backups</span><span>Security</span></div>'
    + '<div class="sbody"><h3>Your day</h3>'
    + '<div class="field"><div class="h">Daily target<small>The ring on the Desk and Day</small></div><div class="row"><span class="btn mono">Mon 8</span><span class="btn mono">Tue 8</span><span class="btn mono">Wed 8</span><span class="btn mono">Thu 8</span><span class="btn mono">Fri 7.5</span></div></div>'
    + '<div class="field hl"><div class="h">Time buttons<small>Up to five, in Log work, jots and tickets</small></div><div class="col" style="display:flex;flex-direction:column;gap:8px">'
    + '<div class="row"><span class="btn mono">10m ✕</span><span class="btn mono">20m ✕</span><span class="btn mono">1h ✕</span><span class="btn mono">3h ✕</span><span class="btn mono" style="border-style:dashed;color:var(--mut)">+ add</span><span class="btn link">Back to 15m 30m 45m 1h 2h</span></div>'
    + '<span class="note">Preview in Log work:</span><div class="row"><span class="cap" style="flex:0 0 260px;margin:0">Paste a ticket or say what you did</span><span class="btn mono">10m</span><span class="btn mono">20m</span><span class="btn mono">1h</span><span class="btn mono">3h</span></div></div></div>'
    + '<div class="field"><div class="h">Wrap-up<small>End-of-day card</small></div><div><span class="sw on"></span> at 17:00</div></div>'
    + "</div></div></div>"
)

MOCKS["meetlog"] = page(
    top()
    + '<div class="main"><div class="desk" style="opacity:.6">'
    + desk_cards()
    + "</div>"
    + '<div class="side"><div class="panel" style="gap:10px"><h4>🔔 Coming up and waiting</h4>'
    + '<div class="panel hl" style="box-shadow:none;background:var(--paper-2)"><span class="lbl">Just finished</span><b>Weekly ops, 10:00 to 10:30</b>'
    + '<div class="row"><span class="btn dark">Log 30m</span><span class="btn">Other time…</span><span class="btn link">Skip</span></div><span class="note">Logs to Operations with the meeting note attached</span></div>'
    + '<span class="lbl">Today</span><div class="row" style="justify-content:space-between"><span>Change board at 14:00</span><span class="btn">Open note</span></div>'
    + '<div class="row" style="justify-content:space-between"><span>Call the vendor back</span><span class="btn">Open</span></div>'
    + '<div class="row" style="justify-content:space-between"><span><span class="tk open">INC0045690</span> still open</span><span class="btn">Open</span></div></div></div>'
    + "</div>"
    + '<div class="note" style="padding:0 6px">Settings → Meetings gets a "Usually lasts" box (30 minutes by default) and a project for each series.</div>'
    + bottom()
)

MOCKS["title"] = page(
    '<div style="display:flex;flex-direction:column;gap:14px;padding:10px 0">'
    + '<span class="lbl">Windows taskbar, hovering Shiftbook</span>'
    + '<div style="display:flex;gap:8px;align-items:flex-end;background:#1f2329;border-radius:10px;padding:10px 14px">'
    + '<div style="background:#2d333b;color:#e7eaee;border-radius:8px;padding:8px 12px;font-size:13px">Inbox, Mail</div>'
    + '<div style="background:#3d4652;color:#fff;border-radius:8px;padding:8px 12px;font-size:13px;outline:2px solid #8ea6ee">Shiftbook · 5.5 h</div>'
    + '<div style="background:#2d333b;color:#e7eaee;border-radius:8px;padding:8px 12px;font-size:13px">Ticket queue, Browser</div></div>'
    + '<span class="lbl">The window\'s own title bar</span>'
    + '<div style="background:var(--paper);border-radius:8px;box-shadow:var(--shadow);padding:8px 12px;display:flex;justify-content:space-between"><span>Shiftbook · 5.5 h</span><span style="color:var(--mut)">—  ☐  ✕</span></div>'
    + '<span class="lbl">Settings → Look</span>'
    + '<div class="panel"><div class="row" style="justify-content:space-between"><span><b>Show today\'s hours in the window title</b><br><span class="note">Only the hours. Hidden while Shiftbook is locked, when it just says "Shiftbook".</span></span><span class="sw on"></span></div></div>'
    + "</div>"
)

MOCKS["nudge"] = page(
    top()
    + '<div class="main"><div class="desk">'
    + desk_cards()
    + "</div>"
    + '<div class="side"><div class="panel hl" style="background:var(--warn-wash)"><b>Nothing logged since 10:30. Anything to log?</b>'
    + '<div class="row"><span class="btn dark">Log something</span><span class="btn">Not now</span><span class="btn link">Turn off</span></div>'
    + '<span class="note">Shows once, after 90 minutes. "Not now" waits another 90.</span></div>'
    + '<div class="panel"><span class="lbl">Today</span><div style="font:700 26px var(--mono)">2.5 <small style="font-size:12px;color:var(--mut)">of 8 h</small></div></div></div>'
    + "</div>"
    + bottom(right="2.5 h today · 🔔 1")
)

MOCKS["unlock"] = page(
    '<div style="height:420px;display:grid;place-items:center">'
    + '<div class="panel" style="width:380px;align-items:center;text-align:center;padding:26px;gap:14px">'
    + '<span class="brand">Shiftbook</span><b>Locked while you were away</b>'
    + '<div class="row" style="justify-content:center">'
    + "".join(
        f'<span style="width:40px;height:48px;border:1px solid var(--rule);border-radius:8px;background:var(--paper-2);display:grid;place-items:center;font:20px var(--mono)">{c}</span>'
        for c in ["•", "•", "•", "", "", ""]
    )
    + "</div>"
    + '<span class="note">Enter your PIN · 3 tries, then your password</span><span class="btn link">Use my password instead</span></div></div>'
)

MOCKS["today"] = page(
    top()
    + '<div class="main"><div class="desk">'
    + desk_cards(dim={1, 3, 4})
    + "</div>"
    + side_default()
    + "</div>"
    + bottom(
        extra='<span class="btn hl" style="background:var(--chip)">◐ Just today <kbd>T</kbd></span><span class="note">Showing today, still open, brought up and pinned</span>'
    )
)

MOCKS["friday"] = page(
    top()
    + '<div class="main"><div class="desk" style="opacity:.6">'
    + desk_cards()
    + "</div>"
    + '<div class="side"><div class="panel" style="gap:10px"><h4>🔔 Coming up and waiting</h4>'
    + '<div class="panel hl" style="box-shadow:none;background:var(--warn-wash)"><b>This week\'s timesheet isn\'t entered yet</b><span class="note">37.5 h logged · 1 entry needs a code</span>'
    + '<div class="row"><span class="btn dark">Open the crib sheet</span><span class="btn link">Remind me at 16:00</span></div></div>'
    + '<span class="lbl">Today</span><div class="row" style="justify-content:space-between"><span>Change board at 14:00</span><span class="btn">Open note</span></div></div></div>'
    + "</div>"
    + '<div class="note" style="padding:0 6px">From midday on your last work day. Stamping the week ENTERED on the Timesheet clears it.</div>'
    + bottom(right="7.5 h today · 🔔 2")
)

MOCKS["recap"] = page(
    top(tab="Day")
    + '<div class="main"><div class="panel" style="flex:1;height:380px"><span class="lbl">Today</span><h3 style="margin:0;font:700 26px var(--display)">Friday 23 October</h3><span class="note">Entries for the day…</span></div>'
    + '<div class="side" style="width:380px"><div class="panel" style="border-top:4px solid var(--accent)"><h4 style="font:700 17px var(--display)">Wrapping up Friday</h4>'
    + '<div class="row" style="gap:22px"><span><b style="font:700 20px var(--mono)">7.5 h</b><br><span class="lbl">today</span></span><span><b style="font:700 20px var(--mono)">0</b><br><span class="lbl">need a code</span></span></div>'
    + '<div class="hl" style="display:flex;flex-direction:column;gap:6px;background:var(--paper-2);padding:10px;border-radius:8px"><span class="lbl">Your week</span>'
    + '<div style="display:flex;height:12px;border-radius:6px;overflow:hidden"><span style="flex:18;background:var(--accent)"></span><span style="flex:10;background:var(--plum)"></span><span style="flex:6;background:var(--teal)"></span><span style="flex:3.5;background:var(--mut)"></span></div>'
    + '<span class="note">Network refresh 18 h · Laptop refresh 10 h · Tickets 6 h · No project 3.5 h</span>'
    + "<span><b>23 tickets closed</b> · busiest day Tuesday, 9 h</span></div>"
    + '<span class="lbl">Monday I start with…</span><div class="cap" style="flex:none;margin:0">Finish the printer ticket</div>'
    + '<div class="row" style="justify-content:flex-end"><span class="btn">Not today</span><span class="btn dark">Done for the week</span></div></div></div></div>'
)

MOCKS["switches"] = page(
    top(tab="Settings")
    + '<div class="main"><div class="settings"><div class="snav"><span class="on">Look</span><span>Your day</span><span>Projects</span><span>Timesheet</span><span>Notes</span><span>Fields</span><span>Responses</span><span>Meetings</span><span>Backups</span><span>Security</span></div>'
    + '<div class="sbody"><h3>Look</h3><div class="field"><div class="h">Theme</div><div class="row"><span class="btn">Follow my computer</span><span class="btn">Light</span><span class="btn">Dark</span></div></div>'
    + '<div class="field hl"><div class="h">On my Desk<small>Switch off what you don\'t use. Nothing is deleted; switching back brings it all back.</small></div><div style="display:grid;grid-template-columns:1fr 1fr;gap:10px 24px">'
    + "".join(
        f'<span class="row" style="justify-content:space-between"><span>{n}<br><span class="note">{d}</span></span><span class="sw {"on" if on else ""}"></span></span>'
        for n, d, on in [
            ("Links between cards", "lines for shared tickets and words", True),
            ("Toolbox", "Move, Select and Box on the left", True),
            ("Drawers", "the cabinet for filing cards", True),
            ("Pins", "chips on the bottom bar", True),
            ("Timeline button", "lay cards out by day", False),
            ("Done today pile", "ticked tickets gather in one pile", True),
        ]
    )
    + "</div></div></div></div></div>"
)

MOCKS["deskkeys"] = page(
    top()
    + '<div class="main"><div class="desk">'
    + desk_cards(focus=2)
    + '<div class="pop" style="left:120px;top:250px;width:560px;flex-direction:row;flex-wrap:wrap;gap:6px 22px;font-size:13px">'
    + "".join(
        f'<span style="width:250px"><kbd>{k}</kbd> {w}</span>'
        for k, w in [
            ("← → ↑ ↓", "next card that way"),
            ("Shift + arrows", "nudge the card"),
            ("Enter", "open it"),
            ("Space", "notepad and options"),
            ("D", "tick done / still open"),
            ("B", "bring up…"),
            ("P", "pin to the bottom bar"),
            ("F", "file in a drawer…"),
        ]
    )
    + '<span class="note" style="width:100%">Shown while you hold Ctrl on the Desk</span></div>'
    + "</div>"
    + side_default()
    + "</div>"
    + bottom()
)

# ---------- batch 2 ----------
MOCKS["todos"] = page(
    top()
    + '<div class="main"><div class="desk">'
    + desk_cards()
    + '<div class="pop" style="left:200px;top:60px;width:380px"><span class="lbl">Notepad · Switch replacement, floor 2</span>'
    + '<div style="font-size:13.5px;line-height:1.8">Old switch out, new one racked.<br>'
    + '<span>☑ <s style="color:var(--mut)">Patch panel labels</s></span><br><span>☐ Update the port map</span><br><span>☐ Tell the floor 2 team it\'s done</span></div>'
    + '<span class="note">Type [] at the start of a line to make a to-do. Click the box to tick it.</span></div>'
    + '<div class="callout" style="left:200px;top:300px">Open to-dos show on the card and in the bell</div>'
    + "</div>"
    + side_default()
    + "</div>"
    + bottom(right="5.5 h today · 🔔 3 · 2 to-dos")
)

MOCKS["age"] = page(
    top(tab="Day")
    + '<div class="main"><div class="panel" style="flex:1"><span class="lbl">Tickets</span>'
    + "".join(
        f'<div class="row" style="justify-content:space-between;border-top:1px solid var(--rule);padding:8px 0"><span><span class="tk open">{t}</span> <span class="note">{n}</span></span><span class="row">{a}<span class="btn mono">− 15 +</span></span></div>'
        for t, n, a in [
            ("INC0045690", "printer, tray 2 roller", '<span class="tk" style="background:var(--warn-wash);color:var(--warn)">open 6 days</span>'),
            ("RITM0012399", "new starter laptop", '<span class="tk" style="background:var(--warn-wash);color:var(--warn)">open 4 days</span>'),
            ("INC0045712", "guest wifi", '<span class="note">since today</span>'),
        ]
    )
    + '<span class="note">Turns amber after 3 days (Settings → Your day). The oldest go to the top.</span></div>'
    + side_default()
    + "</div>"
)

MOCKS["snooze"] = page(
    top(tab="Day")
    + '<div class="main"><div class="panel" style="flex:1;position:relative"><span class="lbl">Tickets</span>'
    + '<div class="row" style="justify-content:space-between;border-top:1px solid var(--rule);padding:8px 0"><span><span class="tk open">INC0045712</span> <span class="note">guest wifi</span></span><span class="btn mono">− 15 +</span></div>'
    + '<div class="row" style="justify-content:space-between;border-top:1px solid var(--rule);padding:8px 0;background:var(--chip);border-radius:6px"><span><span class="tk open">INC0045690</span> <span class="note">waiting on the part</span></span><span class="btn mono">− 15 +</span></div>'
    + '<div class="menu" style="left:300px;top:90px"><div>Tick done</div><div class="on">Snooze until… <small>›</small></div><div>Give it a project</div></div>'
    + '<div class="menu" style="left:508px;top:120px"><div>Tomorrow</div><div class="on">Thursday <small>29 Oct</small></div><div>Next week</div><div>Pick a day…</div></div>'
    + '<div style="margin-top:110px;border-top:1px dashed var(--rule);padding-top:8px" class="note">▸ Snoozed (2): back on their day, still counted as open</div></div>'
    + side_default()
    + "</div>"
)

MOCKS["fill"] = page(
    top(tab="Day")
    + '<div class="main"><div class="panel" style="flex:1"><span class="lbl">Today</span><h3 style="margin:0;font:700 26px var(--display)">Friday 23 October</h3>'
    + '<div class="row" style="gap:16px"><span style="font:700 26px var(--mono)">6.25</span><span class="note">of 7.5 h</span></div>'
    + '<div class="panel hl" style="box-shadow:none;background:var(--paper-2)"><b>Fill the last 1.25 h with…</b>'
    + '<div class="row"><span class="btn">Tickets (today\'s roll-up)</span><span class="btn">Admin and email</span><span class="btn">Switch replacement, floor 2</span><span class="btn link">Something else…</span></div>'
    + '<span class="note">Adds 1.25 h to the one you pick. Only offered at wrap-up time or from Find ("fill").</span></div></div>'
    + side_default()
    + "</div>"
)

MOCKS["split"] = page(
    top(tab="Day")
    + '<div class="main"><div class="panel" style="flex:1;position:relative;height:360px"><span class="lbl">Entries</span>'
    + '<div class="row" style="justify-content:space-between;padding:8px 0;border-top:1px solid var(--rule)"><span><b>Switch replacement, floor 2</b> <span class="note">Network refresh</span></span><span style="font:700 15px var(--mono)">2.5</span></div>'
    + '<div class="pop" style="left:140px;top:80px;width:470px"><b>Split 2.5 h into two entries</b>'
    + '<div class="row"><span class="cap" style="flex:1;margin:0;color:var(--ink)">Switch replacement, floor 2</span><span class="btn mono">1.5 h</span><span class="btn">Network refresh</span></div>'
    + '<div class="row"><span class="cap" style="flex:1;margin:0;color:var(--ink)">Cable tidy, floor 2</span><span class="btn mono">1 h</span><span class="btn">Site A move</span></div>'
    + '<span class="note">Tickets stay on the first one unless you drag them across. Merge does the reverse from a right-click on two entries.</span>'
    + '<div class="row" style="justify-content:flex-end"><span class="btn">Cancel</span><span class="btn dark">Split</span></div></div></div>'
    + side_default()
    + "</div>"
)

MOCKS["multiline"] = page(
    top(tab="Day")
    + '<div class="main"><div class="panel" style="flex:1"><span class="lbl">Log work</span>'
    + '<div style="border:2px solid var(--accent);border-radius:8px;padding:8px 10px;font:13px/1.6 var(--mono)">printer queue fixes 30m<br>INC0045712 guest wifi<br>stand-up 15m</div>'
    + '<b>3 entries from 3 lines</b>'
    + "".join(
        f'<div class="row" style="justify-content:space-between;border-top:1px solid var(--rule);padding:6px 0"><span>{t}</span><span class="row"><span class="btn mono{" dark" if m == "?" else ""}">{m if m != "?" else "pick a time"}</span></span></div>'
        for t, m in [("Printer queue fixes", "30m"), ('<span class="tk">INC0045712</span> guest wifi', "?"), ("Stand-up", "15m")]
    )
    + '<div class="row" style="justify-content:flex-end"><span class="btn">Cancel</span><span class="btn dark">Log all 3</span></div></div>'
    + side_default()
    + "</div>"
)

MOCKS["blanks"] = page(
    top(tab="Day")
    + '<div class="main"><div class="panel" style="flex:1;position:relative;height:340px"><span class="lbl">Tickets</span>'
    + '<div class="row" style="justify-content:space-between;border-top:1px solid var(--rule);padding:8px 0"><span><span class="tk open">INC0045690</span> <span class="note">printer, tray 2 roller</span></span><span class="btn">⧉ Responses</span></div>'
    + '<div class="pop" style="left:220px;top:70px;width:460px"><b>Fixed and confirmed</b>'
    + '<div style="background:var(--paper-2);border-radius:8px;padding:10px;font-size:13.5px">Hi <span class="hl" style="background:var(--chip);padding:0 4px">Sam</span>, <span class="tk">INC0045690</span> is fixed: replaced the tray 2 roller. Confirmed working with you on <b>Fri 23 Oct</b>. Closing the ticket.</div>'
    + '<span class="note">{ticket} and {date} fill in by themselves. {name} asks once: <span class="cap" style="display:inline-block;margin:0;padding:2px 8px;color:var(--ink)">Sam</span></span>'
    + '<div class="row" style="justify-content:flex-end"><span class="btn dark">Copy</span></div></div></div>'
    + side_default()
    + "</div>"
)

MOCKS["daily"] = page(
    top(tab="Notes")
    + '<div class="main"><div class="panel" style="flex:1;gap:10px"><h3 style="margin:0;font:700 22px var(--display)">Monday 26 October</h3>'
    + '<div class="hl" style="background:var(--paper-2);border-radius:8px;padding:10px 12px;font-size:13.5px;line-height:1.75">'
    + "<b>Meetings</b><br>09:30 Stand-up · 14:00 Change board<br><b>Brought up for today</b><br>☐ Call the vendor back<br>☐ <span class=\"tk open\">INC0045690</span> printer roller<br><b>Notes</b><br><span style=\"color:var(--mut)\">|</span></div>"
    + '<span class="note">Written when you open today\'s note for the first time. Edit the template in Settings → Notes, or switch it off.</span></div>'
    + side_default()
    + "</div>"
)

MOCKS["timeoff"] = page(
    top()
    + '<div class="main"><div class="desk" style="opacity:.6">'
    + desk_cards()
    + "</div>"
    + side_default()
    + "</div>"
    + '<div class="pop" style="left:260px;top:90px;width:470px"><b style="font:700 18px var(--display)">Book time off</b>'
    + '<div class="row"><span class="btn">Mon 2 Nov</span> to <span class="btn">Fri 6 Nov</span><span class="btn mono">8 h a day</span></div>'
    + '<div class="row">' + "".join(f'<span class="tk" style="padding:6px 8px">{d}</span>' for d in ["Mon 2", "Tue 3", "Wed 4", "Thu 5", "Fri 6"]) + "</div>"
    + '<span class="note">5 work days, 40 h on your holiday code. Weekends and days you already logged are skipped. The ribbon shows them in grey, and the bell stays quiet.</span>'
    + '<div class="row" style="justify-content:flex-end"><span class="btn">Cancel</span><span class="btn dark">Book 5 days</span></div></div>'
    + bottom()
)

MOCKS["recent"] = page(
    top()
    + '<div class="main"><div class="desk" style="opacity:.55">'
    + desk_cards()
    + "</div>"
    + side_default()
    + "</div>"
    + '<div class="pop" style="left:300px;top:90px;width:400px;padding:10px"><span class="lbl">Recent · hold Ctrl, press Tab to move</span>'
    + '<div class="menu" style="position:static;box-shadow:none;padding:0">'
    + '<div>Weekly ops · Mon 19 Oct <small>meeting note</small></div><div class="on">Switch replacement, floor 2 <small>notepad</small></div><div><span class="tk open">INC0045690</span> <small>ticket</small></div><div>Guest Wi-Fi <small>page</small></div><div>Timesheet, week of 19 Oct <small>view</small></div></div>'
    + '<span class="note">Let go of Ctrl to open it.</span></div>'
    + bottom()
)

MOCKS["lockwrap"] = page(
    top()
    + '<div class="main"><div class="desk" style="opacity:.55">'
    + desk_cards()
    + "</div>"
    + side_default()
    + "</div>"
    + '<div class="pop" style="left:290px;top:110px;width:420px"><b style="font:700 18px var(--display)">Wrap up before you go?</b>'
    + '<span>It\'s 17:20 and you\'ve logged 6.5 of 8 h. Two minutes now saves the morning.</span>'
    + '<div class="row"><span class="btn dark">Wrap up</span><span class="btn">Just lock</span><span class="btn link">Not today</span></div>'
    + '<span class="note">Only after your wrap-up time, once a day, and only if you haven\'t wrapped up yet.</span></div>'
    + bottom()
)

MOCKS["rightdrag"] = page(
    top(tab="Settings")
    + '<div class="main"><div class="settings"><div class="snav"><span class="on">Look</span><span>Your day</span><span>Projects</span><span>Timesheet</span><span>Notes</span><span>Fields</span><span>Responses</span><span>Meetings</span><span>Backups</span><span>Security</span></div>'
    + '<div class="sbody"><h3>Look</h3><div class="field"><div class="h">Theme</div><div class="row"><span class="btn">Follow my computer</span><span class="btn">Light</span><span class="btn">Dark</span></div></div>'
    + '<div class="field"><div class="h">Text size</div><div class="row"><span class="btn">Small</span><span class="btn">Normal</span><span class="btn">Large</span><span class="btn">Larger</span></div></div>'
    + '<div class="field hl"><div class="h">Mouse<small>Off to start with.</small></div>'
    + '<span class="row" style="justify-content:space-between;align-items:flex-start"><span><b>Drag with the right button too</b><br>'
    + '<span class="note">Hold the right button and move to drag cards, piles and areas, slide the Desk, or move a window. '
    + 'A right-click without moving still opens the menu.</span></span><span class="sw on"></span></span></div>'
    + "</div></div></div></div>",
    h=330,
)

MOCKS["rightdrag-desk"] = page(
    top()
    + '<div class="main"><div class="desk">'
    + "".join(
        card(x, y, k, t, s, c, "dim" if i == 1 else "")
        for i, (x, y, k, t, s, c) in enumerate(DESK_CARDS)
    )
    + '<div class="card" style="left:300px;top:96px;--c:var(--plum);box-shadow:var(--lift);transform:rotate(-1.5deg);z-index:4">'
    + '<span class="k">Entry · 1.5 h · Tue</span><span class="t">Laptop imaging</span><span class="note">Laptop refresh</span></div>'
    + '<span class="ptr" style="left:392px;top:140px"></span>'
    + '<span class="callout" style="left:404px;top:162px">Right button held: dragging</span>'
    + '<div class="pop" style="left:14px;top:280px;width:600px;padding:8px 12px;flex-direction:row;gap:22px;font-size:12.5px">'
    + '<span><b>Right-click, let go</b><br><span class="note">the card\'s menu, same as now</span></span>'
    + '<span><b>Right button, move</b><br><span class="note">drags it; no menu afterwards</span></span>'
    + '<span><b>Right button on empty Desk</b><br><span class="note">slides the Desk</span></span></div>'
    + "</div>"
    + side_default()
    + "</div>"
    + bottom()
)

if __name__ == "__main__":
    for k, html in MOCKS.items():
        with open(os.path.join(HERE, f"r15-qol-{k}.html"), "w", encoding="utf-8") as f:
            f.write(html)
    print(len(MOCKS), "mockups written")
