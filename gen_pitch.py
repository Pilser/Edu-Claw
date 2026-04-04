"""
EduClaw Funder Pitch Deck — PDF generator (ReportLab)
Produces a polished, visual, non-technical PDF pitch deck.
"""

from reportlab.lib.pagesizes import A4
from reportlab.lib import colors
from reportlab.lib.units import mm
from reportlab.pdfgen import canvas
from reportlab.lib.colors import HexColor, white, black
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
import math, os

OUT = "/home/pilser/Edu-Claw/EduClaw_Pitch_Deck.pdf"
W, H = A4  # 595.28 x 841.89 pts

# ── Palette ────────────────────────────────────────────────────────────────────
NAVY      = HexColor("#0D1B3E")
TEAL      = HexColor("#0D7A6F")
TEAL_LT   = HexColor("#E0F4F2")
TEAL_MID  = HexColor("#1BAA9B")
BLUE      = HexColor("#1A4B8C")
BLUE_LT   = HexColor("#E8EEF8")
AMBER     = HexColor("#F5A623")
AMBER_LT  = HexColor("#FFF8E6")
WHITE     = HexColor("#FFFFFF")
OFFWHITE  = HexColor("#F7F9FC")
DARK      = HexColor("#1A1A2E")
GREY      = HexColor("#6B7280")
GREY_LT   = HexColor("#F3F4F6")

# ── Helpers ────────────────────────────────────────────────────────────────────
def page(c):
    c.showPage()

def rect(c, x, y, w, h, fill, radius=0, stroke=None, stroke_w=1):
    c.setFillColor(fill)
    if stroke:
        c.setStrokeColor(stroke)
        c.setLineWidth(stroke_w)
    if radius:
        c.roundRect(x, y, w, h, radius, fill=1, stroke=1 if stroke else 0)
    else:
        c.rect(x, y, w, h, fill=1, stroke=1 if stroke else 0)

def text(c, txt, x, y, size=11, color=DARK, bold=False, align="left"):
    c.setFillColor(color)
    face = "Helvetica-Bold" if bold else "Helvetica"
    c.setFont(face, size)
    if align == "center":
        c.drawCentredString(x, y, txt)
    elif align == "right":
        c.drawRightString(x, y, txt)
    else:
        c.drawString(x, y, txt)

def multiline(c, txt, x, y, size=10, color=DARK, bold=False,
              max_width=None, leading=14, align="left"):
    face = "Helvetica-Bold" if bold else "Helvetica"
    c.setFont(face, size)
    c.setFillColor(color)
    if max_width is None:
        c.drawString(x, y, txt)
        return y - leading
    words = txt.split()
    line  = ""
    cy    = y
    for w in words:
        test = (line + " " + w).strip()
        if c.stringWidth(test, face, size) <= max_width:
            line = test
        else:
            if align == "center":
                c.drawCentredString(x, cy, line)
            elif align == "right":
                c.drawRightString(x, cy, line)
            else:
                c.drawString(x, cy, line)
            cy -= leading
            line = w
    if line:
        if align == "center":
            c.drawCentredString(x, cy, line)
        elif align == "right":
            c.drawRightString(x, cy, line)
        else:
            c.drawString(x, cy, line)
        cy -= leading
    return cy

def circle(c, cx, cy, r, fill, stroke=None, stroke_w=1):
    c.setFillColor(fill)
    if stroke:
        c.setStrokeColor(stroke)
        c.setLineWidth(stroke_w)
    c.circle(cx, cy, r, fill=1, stroke=1 if stroke else 0)

def header_band(c, title, subtitle=None):
    """Dark navy top band for section slides."""
    rect(c, 0, H - 90, W, 90, NAVY)
    text(c, title, W/2, H - 48, size=26, color=WHITE, bold=True, align="center")
    if subtitle:
        text(c, subtitle, W/2, H - 66, size=12, color=HexColor("#9BB4D8"), align="center")

def page_number(c, n):
    text(c, str(n), W - 20*mm, 8*mm, size=9, color=GREY, align="right")

def teal_pill(c, label, x, y, w=120, h=22):
    rect(c, x, y, w, h, TEAL, radius=11)
    text(c, label, x + w/2, y + 7, size=9, color=WHITE, bold=True, align="center")

def amber_pill(c, label, x, y, w=120, h=22):
    rect(c, x, y, w, h, AMBER, radius=11)
    text(c, label, x + w/2, y + 7, size=9, color=WHITE, bold=True, align="center")

def card(c, x, y, w, h, fill=OFFWHITE, stroke=None, radius=8):
    rect(c, x, y, w, h, fill, radius=radius, stroke=stroke, stroke_w=1.5)

def icon_circle(c, cx, cy, r, bg, glyph, glyph_size=18):
    circle(c, cx, cy, r, bg)
    text(c, glyph, cx, cy - glyph_size*0.35, size=glyph_size, color=WHITE,
         bold=True, align="center")

# ── SLIDE 1 — Cover ────────────────────────────────────────────────────────────
def slide_cover(c):
    # Full bleed gradient-ish background
    rect(c, 0, 0, W, H, NAVY)
    # Decorative arcs
    c.setStrokeColor(HexColor("#1A4B8C"))
    c.setLineWidth(60)
    c.setFillColor(HexColor("#00000000"))
    for r in [280, 370, 460]:
        c.circle(W + 40, -40, r, fill=0, stroke=1)
    c.setStrokeColor(TEAL)
    c.setLineWidth(3)
    for r in [200, 240]:
        c.circle(W + 40, -40, r, fill=0, stroke=1)

    # Logo area
    circle(c, W/2, H/2 + 80, 54, TEAL)
    text(c, "EC", W/2, H/2 + 68, size=32, color=WHITE, bold=True, align="center")

    # Title
    text(c, "EduClaw", W/2, H/2 + 10, size=44, color=WHITE, bold=True, align="center")
    text(c, "AI-Powered Learning, Built for Every Student",
         W/2, H/2 - 18, size=16, color=HexColor("#9BB4D8"), align="center")

    # Separator line
    c.setStrokeColor(TEAL)
    c.setLineWidth(2)
    c.line(W/2 - 80, H/2 - 38, W/2 + 80, H/2 - 38)

    # Tagline
    text(c, "A private AI tutor inside every student's screen.",
         W/2, H/2 - 58, size=12, color=HexColor("#CBD5E1"), align="center")

    # Bottom strip
    rect(c, 0, 0, W, 54, TEAL)
    text(c, "CONFIDENTIAL  ·  INVESTOR BRIEFING",
         W/2, 20, size=9, color=WHITE, align="center")
    page_number(c, 1)

# ── SLIDE 2 — The Problem ──────────────────────────────────────────────────────
def slide_problem(c):
    rect(c, 0, 0, W, H, OFFWHITE)
    header_band(c, "The Problem", "Students are falling behind — and teachers can't be everywhere")

    stats = [
        ("1 in 3", "students feels\nlost in class"),
        ("47%",    "say they never ask\nfor help when stuck"),
        ("1 : 30", "teacher-to-student\nratio on average"),
        ("$40B",   "spent on tutoring\nannually, inaccessible\nto most families"),
    ]

    bw = (W - 60) / 4 - 8
    bh = 140
    bx_start = 30
    by = H - 90 - 24 - bh

    for i, (big, label) in enumerate(stats):
        bx = bx_start + i * (bw + 10)
        card(c, bx, by, bw, bh, WHITE, stroke=TEAL)
        # Accent top bar
        rect(c, bx, by + bh - 10, bw, 10, TEAL, radius=0)
        # Big number
        text(c, big, bx + bw/2, by + bh - 50, size=28, color=NAVY,
             bold=True, align="center")
        # Label
        lines = label.split("\n")
        for j, ln in enumerate(lines):
            text(c, ln, bx + bw/2, by + bh - 70 - j*14, size=10,
                 color=GREY, align="center")

    # Narrative paragraph
    ny = by - 30
    multiline(c,
        "Today's classrooms are overwhelmed. A single teacher cannot give "
        "personal attention to every student. Private tutors are expensive. "
        "Students who fall behind often stay behind.",
        40, ny, size=12, color=DARK, max_width=W - 80, leading=17)

    # Pain callout
    rect(c, 30, 70, W - 60, 54, AMBER_LT, radius=8,
         stroke=AMBER, stroke_w=1.5)
    text(c, "The gap between students who get support and those who don't",
         W/2, 107, size=11, color=DARK, bold=True, align="center")
    text(c, "is not a talent gap — it is an access gap.",
         W/2, 91, size=11, color=AMBER, bold=True, align="center")

    page_number(c, 2)

# ── SLIDE 3 — The Solution ────────────────────────────────────────────────────
def slide_solution(c):
    rect(c, 0, 0, W, H, OFFWHITE)
    header_band(c, "The Solution", "EduClaw — a personal AI tutor, always on, always in context")

    # Central visual: laptop screen mockup
    sw, sh = 320, 200
    sx = (W - sw) / 2
    sy = H - 90 - 30 - sh
    rect(c, sx - 10, sy - 20, sw + 20, sh + 30, DARK, radius=10)
    rect(c, sx, sy, sw, sh, HexColor("#1E1E2E"), radius=4)

    # Fake terminal lines
    lines_left = [
        (TEAL_MID,  "EduClaw  >  Hello, Alex!"),
        (WHITE,     "I see you're working on Chapter 3."),
        (WHITE,     "Want me to walk you through it?"),
        (AMBER,     "Alex  >  yes please"),
        (TEAL_MID,  "EduClaw  >  Sure! Let's start with"),
        (WHITE,     "the key concept: Newton's 2nd Law..."),
    ]
    for i, (col, ln) in enumerate(lines_left):
        text(c, ln, sx + 10, sy + sh - 20 - i * 16, size=8, color=col)

    # File panel on right
    fpx = sx + sw - 100
    rect(c, fpx, sy, 100, sh, HexColor("#252535"), radius=4)
    text(c, "My Files", fpx + 8, sy + sh - 14, size=7,
         color=HexColor("#9BB4D8"), bold=True)
    for i, fn in enumerate(["chapter3.md", "notes.py", "diagram.png"]):
        text(c, "  " + fn, fpx + 6, sy + sh - 28 - i*14,
             size=7, color=HexColor("#7DD3D0"))

    # Laptop base
    c.setFillColor(DARK)
    c.roundRect(sx - 30, sy - 32, sw + 60, 14, 4, fill=1, stroke=0)

    # Three benefit columns below laptop
    cols = [
        (TEAL,  "Knows Every Student",
         "The AI knows the student's name, class, and exactly what they are studying."),
        (BLUE,  "Always Available",
         "24/7 support, no waiting. Every student gets the same quality of help."),
        (AMBER, "Safe & Private",
         "Each student has their own completely separate, secure workspace."),
    ]
    cw = (W - 80) / 3
    cy_top = sy - 50
    for i, (col, title, desc) in enumerate(cols):
        cx = 30 + i * (cw + 10)
        card(c, cx, cy_top - 72, cw, 76, WHITE, stroke=col)
        circle(c, cx + 18, cy_top - 10, 9, col)
        text(c, title, cx + 34, cy_top - 14, size=10, color=DARK, bold=True)
        multiline(c, desc, cx + 8, cy_top - 30, size=9, color=GREY,
                  max_width=cw - 16, leading=13)

    page_number(c, 3)

# ── SLIDE 4 — How It Works (Visual Flow) ──────────────────────────────────────
def slide_how(c):
    rect(c, 0, 0, W, H, OFFWHITE)
    header_band(c, "How It Works", "Three simple steps — for the student")

    steps = [
        ("1", TEAL,  "Student logs in",
         "Open a browser. Enter your school login. That's it."),
        ("2", BLUE,  "AI tutor appears",
         "A personal AI assistant greets the student by name, knows their subject, and is ready to help immediately."),
        ("3", AMBER, "Student learns",
         "Ask questions, run code, see diagrams — all inside one screen. Files save automatically."),
    ]

    sw_box = (W - 80) / 3
    box_h  = 220
    by     = H - 90 - 30 - box_h

    for i, (num, col, title, desc) in enumerate(steps):
        bx = 30 + i * (sw_box + 10)
        card(c, bx, by, sw_box, box_h, WHITE, stroke=col, radius=10)
        # Top colour strip
        rect(c, bx, by + box_h - 12, sw_box, 12, col, radius=0)
        # Number circle
        circle(c, bx + sw_box/2, by + box_h - 42, 20, col)
        text(c, num, bx + sw_box/2, by + box_h - 49, size=18, color=WHITE,
             bold=True, align="center")
        # Title
        text(c, title, bx + sw_box/2, by + box_h - 74, size=13, color=NAVY,
             bold=True, align="center")
        # Desc
        multiline(c, desc, bx + 12, by + box_h - 96, size=10, color=GREY,
                  max_width=sw_box - 24, leading=14)

    # Arrows between boxes
    for i in range(2):
        ax = 30 + (i+1) * (sw_box + 10) - 5
        ay = by + box_h/2
        c.setFillColor(TEAL)
        c.setStrokeColor(TEAL)
        c.setLineWidth(2)
        c.line(ax - 4, ay, ax + 4, ay)
        # arrowhead
        p = c.beginPath()
        p.moveTo(ax + 4, ay)
        p.lineTo(ax - 2, ay + 5)
        p.lineTo(ax - 2, ay - 5)
        p.close()
        c.drawPath(p, fill=1, stroke=0)

    # Bottom what-the-teacher-sees strip
    ty = by - 20
    rect(c, 30, ty - 70, W - 60, 74, NAVY, radius=10)
    text(c, "What the teacher sees", 50, ty - 16, size=11,
         color=TEAL_MID, bold=True)
    items = [
        "Every student's progress in real time",
        "Instant alerts if a student is struggling",
        "Full control — same classroom, smarter tools",
    ]
    for i, item in enumerate(items):
        text(c, "  \u2022  " + item, 50, ty - 32 - i*16, size=10, color=WHITE)

    page_number(c, 4)

# ── SLIDE 5 — What Makes It Different ────────────────────────────────────────
def slide_difference(c):
    rect(c, 0, 0, W, H, OFFWHITE)
    header_band(c, "Why EduClaw is Different",
                "Not another chatbot. A full learning environment.")

    rows = [
        ("Generic AI chatbots",    "No memory of who you are",    "Knows student name, class, progress"),
        ("Online courses",          "No real help when stuck",     "Always-on AI tutor, answers instantly"),
        ("Private tutors",          "Expensive — $40–$100/hr",    "Available to every student, any time"),
        ("School IT tools",         "No AI, no personalisation",  "Built for AI-first learning from day one"),
        ("Traditional LMS",        "Static — no interaction",     "Live, interactive, two-way AI session"),
    ]

    # Table header
    col_w = [(W - 60) * f for f in [0.32, 0.33, 0.35]]
    headers = ["Others", "Their gap", "EduClaw"]
    hx = 30
    hy = H - 90 - 30
    for i, (hdr, w) in enumerate(zip(headers, col_w)):
        bg = GREY if i < 2 else TEAL
        rect(c, hx, hy - 22, w - 4, 22, bg, radius=4)
        text(c, hdr, hx + w/2 - 2, hy - 14, size=10, color=WHITE,
             bold=True, align="center")
        hx += w

    for r_idx, row in enumerate(rows):
        rx = 30
        ry = hy - 24 - (r_idx + 1) * 30
        shade = GREY_LT if r_idx % 2 == 0 else WHITE
        for c_idx, (cell, cw) in enumerate(zip(row, col_w)):
            bg = shade if c_idx < 2 else (TEAL_LT if r_idx % 2 == 0 else WHITE)
            col_txt = DARK if c_idx < 2 else TEAL
            bld     = c_idx == 2
            rect(c, rx, ry, cw - 4, 26, bg, radius=3,
                 stroke=HexColor("#E5E7EB"), stroke_w=0.5)
            text(c, cell, rx + 8, ry + 9, size=9, color=col_txt, bold=bld)
            rx += cw

    # Bottom callout
    rect(c, 30, 50, W - 60, 56, NAVY, radius=10)
    text(c, "EduClaw is the only platform that gives every student",
         W/2, 88, size=12, color=WHITE, bold=True, align="center")
    text(c, "a personalised AI teacher — inside a safe, school-managed environment.",
         W/2, 70, size=11, color=TEAL_MID, align="center")

    page_number(c, 5)

# ── SLIDE 6 — The Experience (Visual Mockup) ──────────────────────────────────
def slide_experience(c):
    rect(c, 0, 0, W, H, DARK)
    # Subtle grid background
    c.setStrokeColor(HexColor("#1A2540"))
    c.setLineWidth(0.5)
    for xi in range(0, int(W), 40):
        c.line(xi, 0, xi, H)
    for yi in range(0, int(H), 40):
        c.line(0, yi, W, yi)

    # Top label
    text(c, "THE STUDENT EXPERIENCE", W/2, H - 28, size=11,
         color=TEAL_MID, bold=True, align="center")

    # Split screen mockup
    screen_w = W - 80
    screen_h = H - 120
    screen_x = 40
    screen_y = 60

    # Screen frame
    rect(c, screen_x - 6, screen_y - 6, screen_w + 12, screen_h + 12,
         HexColor("#0D1B3E"), radius=12)
    rect(c, screen_x, screen_y, screen_w, screen_h,
         HexColor("#0F172A"), radius=8)

    # Window bar
    rect(c, screen_x, screen_y + screen_h - 24, screen_w, 24,
         HexColor("#1E293B"), radius=0)
    for ci, col in enumerate([HexColor("#FF5F57"), HexColor("#FFBD2E"), HexColor("#28CA41")]):
        circle(c, screen_x + 14 + ci * 18, screen_y + screen_h - 12, 5, col)
    text(c, "EduClaw — Alex's Workspace — Physics 101",
         screen_x + screen_w / 2, screen_y + screen_h - 17,
         size=8, color=HexColor("#94A3B8"), align="center")

    # Left panel (terminal/chat)
    lw = int(screen_w * 0.55)
    lx = screen_x
    ly = screen_y
    lh = screen_h - 24
    rect(c, lx, ly, lw, lh, HexColor("#0F172A"))

    chat = [
        (TEAL_MID, "EduClaw",  "Hi Alex! I can see you're on Chapter 3 — Forces."),
        (TEAL_MID, "EduClaw",  "Would you like me to explain Newton's 2nd Law visually?"),
        (AMBER,    "Alex",     "yes and can you show the formula?"),
        (TEAL_MID, "EduClaw",  "Of course! Here it is:   F = m × a"),
        (TEAL_MID, "EduClaw",  "Force = mass × acceleration. Let me run an example..."),
        (HexColor("#4ADE80"), "Python", "mass = 5  # kg"),
        (HexColor("#4ADE80"), "Python", "accel = 3  # m/s²"),
        (HexColor("#4ADE80"), "Python", "print(mass * accel)   # →  15 N"),
        (TEAL_MID, "EduClaw",  "There — 15 Newtons. Want to try changing the numbers?"),
    ]
    for i, (col, who, msg) in enumerate(chat):
        ty_pos = ly + lh - 20 - i * 18
        if ty_pos < ly + 10:
            break
        text(c, who + ":", lx + 10, ty_pos, size=7, color=col, bold=True)
        text(c, msg, lx + 58, ty_pos, size=7.5, color=WHITE)

    # Input bar at bottom
    rect(c, lx + 8, ly + 8, lw - 16, 20, HexColor("#1E293B"), radius=6)
    text(c, "Ask anything about your lesson...", lx + 18, ly + 14,
         size=7, color=HexColor("#475569"))

    # Right panel (file browser)
    rw = screen_w - lw
    rx2 = lx + lw
    ry2 = ly
    rect(c, rx2, ry2, rw, lh, HexColor("#131C2E"))
    rect(c, rx2, ry2 + lh - 28, rw, 28, HexColor("#1E293B"))
    text(c, "My Files  &  Class Materials", rx2 + rw/2,
         ry2 + lh - 14, size=7.5, color=HexColor("#9BB4D8"),
         bold=True, align="center")

    # File tree
    file_items = [
        (TEAL_MID, "\u25be My Files"),
        (WHITE,    "  notes_chapter3.py"),
        (WHITE,    "  my_diagram.png"),
        (HexColor("#9BB4D8"), "\u25be Class: Physics 101"),
        (WHITE,    "  chapter3.md"),
        (WHITE,    "  chapter3_fig_0.png"),
        (WHITE,    "  assignment2.pdf"),
    ]
    for i, (col, fn) in enumerate(file_items):
        text(c, fn, rx2 + 10, ry2 + lh - 46 - i * 16, size=7.5, color=col)

    # Image preview box
    pby = ry2 + 50
    pbx = rx2 + 10
    pbw = rw - 20
    pbh = 90
    rect(c, pbx, pby, pbw, pbh, HexColor("#1E293B"), radius=4)
    # Fake graph
    gx, gy = pbx + 14, pby + 14
    gw, gh = pbw - 28, pbh - 28
    rect(c, gx, gy, gw, gh, HexColor("#0F172A"), radius=2)
    c.setStrokeColor(TEAL_MID)
    c.setLineWidth(1.5)
    pts = [(0, 0), (0.2, 0.04), (0.4, 0.16), (0.6, 0.36),
           (0.8, 0.64), (1.0, 1.0)]
    path = c.beginPath()
    for xi_f, yi_f in pts:
        px2 = gx + xi_f * gw
        py2 = gy + yi_f * gh
        if xi_f == 0:
            path.moveTo(px2, py2)
        else:
            path.lineTo(px2, py2)
    c.drawPath(path, fill=0, stroke=1)
    text(c, "F = m\u00d7a graph — auto-generated",
         pbx + pbw/2, pby + 6, size=6.5, color=HexColor("#64748B"),
         align="center")

    # Caption bottom
    text(c, "Every student sees their own private workspace — "
         "files, AI chat, and course materials, all in one screen.",
         W/2, 30, size=9, color=HexColor("#94A3B8"), align="center")

    page_number(c, 6)

# ── SLIDE 7 — Privacy & Safety ────────────────────────────────────────────────
def slide_safety(c):
    rect(c, 0, 0, W, H, OFFWHITE)
    header_band(c, "Privacy & Safety First",
                "Every student's work is completely separate and secure")

    # Central diagram: 3 bubbles = 3 students, no overlap
    cx_c = W / 2
    cy_c = H / 2 - 10
    r_big = 90
    student_r = 52
    angles = [210, 330, 90]
    student_names = ["Alex", "Maria", "James"]
    student_cols  = [TEAL, BLUE, AMBER]

    # Server/platform hub in centre
    circle(c, cx_c, cy_c, r_big + 28, HexColor("#E8EEF8"),
           stroke=BLUE, stroke_w=1.5)
    circle(c, cx_c, cy_c, 36, NAVY)
    text(c, "EduClaw", cx_c, cy_c - 5, size=9, color=WHITE,
         bold=True, align="center")
    text(c, "Platform", cx_c, cy_c - 17, size=8, color=TEAL_MID,
         align="center")

    for angle_deg, name, col in zip(angles, student_names, student_cols):
        rad = math.radians(angle_deg)
        scx = cx_c + (r_big + 80) * math.cos(rad)
        scy = cy_c + (r_big + 80) * math.sin(rad)

        # Connecting line
        c.setStrokeColor(col)
        c.setLineWidth(1.5)
        c.setDash(4, 3)
        c.line(cx_c + 36 * math.cos(rad), cy_c + 36 * math.sin(rad),
               scx - student_r * math.cos(rad), scy - student_r * math.sin(rad))
        c.setDash()

        # Student bubble
        circle(c, scx, scy, student_r, HexColor("#FFFFFF"),
               stroke=col, stroke_w=2)
        circle(c, scx, scy + 16, 16, col)
        text(c, name[0], scx, scy + 9, size=14, color=WHITE,
             bold=True, align="center")
        text(c, name, scx, scy - 4, size=10, color=DARK,
             bold=True, align="center")
        text(c, "Private workspace", scx, scy - 18, size=8,
             color=GREY, align="center")
        text(c, "Cannot see others", scx, scy - 30, size=8,
             color=col, align="center")

    # Lock badge on dividing lines
    lbx = cx_c - 10
    for angle_deg in angles:
        rad = math.radians(angle_deg)
        lbcx = cx_c + (r_big + 44) * math.cos(rad)
        lbcy = cy_c + (r_big + 44) * math.sin(rad)
        circle(c, lbcx, lbcy, 9, WHITE, stroke=TEAL, stroke_w=1)
        text(c, "\U0001F512", lbcx, lbcy - 5, size=9, align="center")

    # Four guarantee cards at bottom
    guarantees = [
        (TEAL,  "Data stays in school",  "Nothing leaves your institution's server"),
        (BLUE,  "No cross-student access", "Students cannot see each other's files or chats"),
        (AMBER, "Teacher oversight",       "Admins can monitor, pause, or reset any session"),
        (NAVY,  "No internet for students","Each workspace is isolated from the open web"),
    ]
    gw = (W - 70) / 4
    gx0 = 30
    gy0 = 72
    for i, (col, title, desc) in enumerate(guarantees):
        gx = gx0 + i * (gw + 6)
        card(c, gx, gy0, gw, 70, WHITE, stroke=col, radius=6)
        rect(c, gx, gy0 + 60, gw, 10, col, radius=0)
        text(c, title, gx + gw/2, gy0 + 48, size=9, color=DARK,
             bold=True, align="center")
        multiline(c, desc, gx + 6, gy0 + 32, size=8, color=GREY,
                  max_width=gw - 12, leading=12)

    page_number(c, 7)

# ── SLIDE 8 — Market Opportunity ─────────────────────────────────────────────
def slide_market(c):
    rect(c, 0, 0, W, H, OFFWHITE)
    header_band(c, "Market Opportunity", "Education technology is one of the fastest-growing sectors globally")

    # Three concentric circles (TAM / SAM / SOM)
    circles_data = [
        (120, HexColor("#DBEAFE"), BLUE,    "$7.5T",  "Global Education Market"),
        (86,  BLUE_LT,             TEAL,    "$350B",  "EdTech Market (2025)"),
        (54,  TEAL_LT,             TEAL,    "$42B",   "AI Tutoring & Personalised\nLearning (2028 est.)"),
    ]
    ccx = W / 3
    ccy = H / 2 + 20

    for r, fill, stroke_col, amount, label in circles_data:
        circle(c, ccx, ccy, r, fill, stroke=stroke_col, stroke_w=2)

    # Labels with lines pointing out
    offsets = [(80, 60), (20, 100), (-60, 120)]
    for (r, fill, stroke_col, amount, label), (ox, oy) in \
            zip(circles_data, offsets):
        lx = ccx + ox
        ly = ccy + oy
        c.setStrokeColor(stroke_col)
        c.setLineWidth(1)
        c.line(ccx + r * 0.6, ccy + r * 0.4, lx, ly)
        text(c, amount, lx, ly + 6, size=12, color=stroke_col,
             bold=True)
        for li, ln in enumerate(label.split("\n")):
            text(c, ln, lx, ly - 6 - li * 12, size=8, color=DARK)

    # Right side: key stats
    rx2 = W / 2 + 20
    stats2 = [
        (TEAL,  "1.6B",   "School-age students worldwide"),
        (BLUE,  "3×",     "Faster learning with 1-on-1 tutoring\n(Bloom's 2-sigma study)"),
        (AMBER, "68%",    "Of parents willing to pay for\nAI tutoring tools"),
        (NAVY,  "$4,200", "Average annual spend per student\non supplemental learning (US)"),
    ]
    sy0 = H - 110
    for i, (col, big, desc) in enumerate(stats2):
        sy = sy0 - i * 88
        card(c, rx2, sy - 72, W - rx2 - 30, 76, WHITE, stroke=col, radius=8)
        rect(c, rx2, sy - 72 + 66, W - rx2 - 30, 10, col, radius=0)
        text(c, big, rx2 + 16, sy - 20, size=22, color=col, bold=True)
        multiline(c, desc, rx2 + 16, sy - 38, size=9, color=GREY,
                  max_width=W - rx2 - 50, leading=13)

    page_number(c, 8)

# ── SLIDE 9 — Business Model ──────────────────────────────────────────────────
def slide_model(c):
    rect(c, 0, 0, W, H, OFFWHITE)
    header_band(c, "Business Model", "School-first SaaS — simple, scalable pricing")

    tiers = [
        (BLUE_LT,  BLUE,  "Starter",
         "Free",
         ["Up to 30 students", "1 classroom", "Core AI tutoring", "Basic teacher dashboard"]),
        (TEAL_LT,  TEAL,  "School",
         "$8 / student / month",
         ["Unlimited students", "All subjects & classes", "PDF course materials", "Admin controls & reports", "Priority support"]),
        (NAVY,     AMBER, "District",
         "Custom pricing",
         ["Multi-school deployment", "Curriculum integration", "Dedicated success manager", "Custom AI personas", "SLA guarantee"]),
    ]
    tw = (W - 80) / 3
    tx0 = 30
    ty_top = H - 100

    for i, (bg, col, name, price, features) in enumerate(tiers):
        tx = tx0 + i * (tw + 10)
        th = 290
        ty = ty_top - th
        card(c, tx, ty, tw, th, bg, stroke=col, radius=12)
        # Top band
        rect(c, tx, ty + th - 48, tw, 48, col, radius=12)
        rect(c, tx, ty + th - 48, tw, 24, col, radius=0)
        text(c, name, tx + tw/2, ty + th - 18, size=14, color=WHITE,
             bold=True, align="center")
        # Price
        font_col = WHITE if i == 2 else col
        text(c, price, tx + tw/2, ty + th - 72, size=12, color=font_col,
             bold=True, align="center")
        # Features
        for j, feat in enumerate(features):
            fy = ty + th - 96 - j * 22
            circle(c, tx + 16, fy + 4, 5, col)
            text(c, feat, tx + 28, fy, size=9, color=DARK)

    # Revenue model note
    rect(c, 30, 42, W - 60, 50, NAVY, radius=8)
    text(c, "Primary revenue: per-student monthly subscription  ·  "
         "Secondary: district data insights (anonymised)  ·  Tertiary: content marketplace",
         W/2, 70, size=9, color=WHITE, align="center")
    text(c, "Target: 500 schools in Year 2  =  $15M ARR at average school size of 400 students",
         W/2, 52, size=9, color=TEAL_MID, align="center")

    page_number(c, 9)

# ── SLIDE 10 — Roadmap ────────────────────────────────────────────────────────
def slide_roadmap(c):
    rect(c, 0, 0, W, H, OFFWHITE)
    header_band(c, "Roadmap to Launch", "14 weeks from today to first paying school")

    phases = [
        (TEAL,  "Phase 1\nWeeks 1–3",   "Foundation",
         ["Core AI tutor running", "Student login & accounts", "Secure private workspaces"]),
        (BLUE,  "Phase 2\nWeeks 4–6",   "Classrooms",
         ["Teacher dashboard", "Session management", "Idle auto-save"]),
        (NAVY,  "Phase 3\nWeeks 7–9",   "AI Integration",
         ["Full AI tutor context", "Course material upload", "Smart hints & guidance"]),
        (AMBER, "Phase 4\nWeeks 10–11", "Student Experience",
         ["File browser & viewer", "Image & diagram display", "One-click file sharing"]),
        (HexColor("#7C3AED"), "Phase 5\nWeeks 12–14", "Launch Ready",
         ["Admin controls", "Security hardening", "First 5 pilot schools"]),
    ]

    # Timeline spine
    spine_y   = H / 2
    spine_x0  = 40
    spine_x1  = W - 40
    c.setStrokeColor(HexColor("#D1D5DB"))
    c.setLineWidth(3)
    c.line(spine_x0, spine_y, spine_x1, spine_y)

    pw = (spine_x1 - spine_x0) / len(phases)

    for i, (col, period, title, items) in enumerate(phases):
        px = spine_x0 + i * pw + pw / 2
        above = i % 2 == 0

        # Node on spine
        circle(c, px, spine_y, 12, col)
        text(c, str(i+1), px, spine_y - 5, size=9, color=WHITE,
             bold=True, align="center")

        # Connector
        c.setStrokeColor(col)
        c.setLineWidth(1.5)
        if above:
            c.line(px, spine_y + 12, px, spine_y + 30)
        else:
            c.line(px, spine_y - 12, px, spine_y - 30)

        # Card
        cw2 = pw - 16
        ch2 = 140
        if above:
            cy2 = spine_y + 30
        else:
            cy2 = spine_y - 30 - ch2
        card(c, px - cw2/2, cy2, cw2, ch2, WHITE, stroke=col, radius=8)
        rect(c, px - cw2/2, cy2 + ch2 - 10, cw2, 10, col, radius=0)

        # Period label
        for li, ln in enumerate(period.split("\n")):
            text(c, ln, px, cy2 + ch2 - 22 - li*13, size=8,
                 color=col, bold=True, align="center")
        text(c, title, px, cy2 + ch2 - 52, size=11, color=DARK,
             bold=True, align="center")

        for j, item in enumerate(items):
            text(c, "\u2022 " + item, px - cw2/2 + 8,
                 cy2 + ch2 - 70 - j * 18, size=8.5, color=GREY)

    page_number(c, 10)

# ── SLIDE 11 — The Ask ────────────────────────────────────────────────────────
def slide_ask(c):
    rect(c, 0, 0, W, H, NAVY)
    # Decorative
    c.setStrokeColor(HexColor("#1A4B8C"))
    c.setLineWidth(50)
    for r in [300, 420]:
        c.circle(-40, H + 40, r, fill=0, stroke=1)
    c.setStrokeColor(TEAL)
    c.setLineWidth(2)
    c.circle(-40, H + 40, 240, fill=0, stroke=1)

    text(c, "The Ask", W/2, H - 60, size=32, color=WHITE,
         bold=True, align="center")
    text(c, "We are raising a seed round to bring EduClaw to 500 schools.",
         W/2, H - 88, size=13, color=HexColor("#9BB4D8"), align="center")

    # Central big number
    rect(c, W/2 - 100, H/2 + 20, 200, 80, TEAL, radius=14)
    text(c, "$2.5M", W/2, H/2 + 78, size=34, color=WHITE,
         bold=True, align="center")
    text(c, "Seed Round", W/2, H/2 + 54, size=13, color=TEAL_LT,
         align="center")

    # How it will be used
    uses = [
        (TEAL,  "45%", "Engineering &\nProduct Build"),
        (BLUE,  "25%", "School Sales &\nPartnerships"),
        (AMBER, "20%", "Infrastructure\n& Security"),
        (HexColor("#7C3AED"), "10%", "Operations\n& Legal"),
    ]
    uw = (W - 80) / 4
    ux0 = 30
    uy_top = H/2 - 10

    for i, (col, pct, label) in enumerate(uses):
        ux = ux0 + i * (uw + 8)
        uy = uy_top - 80
        card(c, ux, uy, uw - 4, 82, HexColor("#112240"), radius=8)
        text(c, pct, ux + (uw-4)/2, uy + 52, size=22, color=col,
             bold=True, align="center")
        for li, ln in enumerate(label.split("\n")):
            text(c, ln, ux + (uw-4)/2, uy + 34 - li*14, size=9,
                 color=WHITE, align="center")

    # Milestones with funding
    text(c, "What this funding achieves:", 40, 180, size=11,
         color=TEAL_MID, bold=True)
    milestones = [
        "Launch with 5 pilot schools  —  Month 3",
        "50 paying schools, break-even on infra  —  Month 9",
        "Series A ready at 500 schools  —  Month 18",
    ]
    for i, m in enumerate(milestones):
        circle(c, 52, 158 - i * 26, 6, TEAL)
        text(c, m, 66, 154 - i * 26, size=10, color=WHITE)

    # Bottom CTA
    rect(c, 30, 26, W - 60, 36, TEAL, radius=8)
    text(c, "contact@educlaw.io  ·  Schedule a demo at educlaw.io/demo",
         W/2, 47, size=11, color=WHITE, bold=True, align="center")

    page_number(c, 11)

# ── SLIDE 12 — Thank You ──────────────────────────────────────────────────────
def slide_thankyou(c):
    rect(c, 0, 0, W, H, OFFWHITE)
    header_band(c, "Thank You", "")

    # Quote
    rect(c, 40, H/2 + 20, W - 80, 110, TEAL_LT, radius=12,
         stroke=TEAL, stroke_w=1.5)
    c.setFont("Helvetica-Oblique", 15)
    c.setFillColor(DARK)
    c.drawCentredString(W/2, H/2 + 108, "\u201cEvery child deserves a great teacher.")
    c.drawCentredString(W/2, H/2 + 86, "EduClaw makes sure every child has one.\u201d")
    text(c, "— EduClaw Mission Statement", W/2, H/2 + 60,
         size=10, color=TEAL, align="center")

    # Summary bullets
    bullets = [
        ("\U0001F9E0  AI that knows each student personally"),
        ("\U0001F512  Completely private and secure by design"),
        ("\U0001F4DA  Works with any subject, any curriculum"),
        ("\U0001F3EB  Built for schools — not for consumers"),
        ("\U0001F680  14 weeks to first paying customers"),
    ]
    bx = W/2 - 160
    for i, b in enumerate(bullets):
        by = H/2 - 10 - i * 28
        card(c, bx, by, 320, 22, WHITE, stroke=TEAL, radius=6)
        text(c, b, bx + 12, by + 6, size=10, color=DARK)

    # Contact block
    rect(c, 60, 60, W - 120, 72, NAVY, radius=12)
    text(c, "Ready to talk?", W/2, 114, size=14, color=WHITE,
         bold=True, align="center")
    text(c, "contact@educlaw.io  ·  educlaw.io  ·  +1 (555) 000-0000",
         W/2, 90, size=10, color=TEAL_MID, align="center")
    text(c, "Schedule a live demo:  educlaw.io/demo",
         W/2, 72, size=10, color=HexColor("#9BB4D8"), align="center")

    page_number(c, 12)

# ── BUILD ──────────────────────────────────────────────────────────────────────
def build():
    c = canvas.Canvas(OUT, pagesize=A4)
    c.setTitle("EduClaw — Investor Pitch Deck")
    c.setAuthor("EduClaw")
    c.setSubject("Funder Pitch Deck")

    slides = [
        slide_cover,
        slide_problem,
        slide_solution,
        slide_how,
        slide_difference,
        slide_experience,
        slide_safety,
        slide_market,
        slide_model,
        slide_roadmap,
        slide_ask,
        slide_thankyou,
    ]

    for fn in slides:
        fn(c)
        c.showPage()

    c.save()
    print(f"Saved: {OUT}")

if __name__ == "__main__":
    build()
