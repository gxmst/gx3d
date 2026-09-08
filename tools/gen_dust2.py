# Generates assets/scenes/dust2.json — an original practice level that
# follows the classic Dust II spatial relationships much more closely than
# the previous hand-authored version (no original game assets; all geometry
# is procedural primitives).
#
# Layout convention: +x east, -z north. T spawn is the south plateau,
# CT spawn north, A site north-east, B site north-west.
# Re-run after tweaking numbers: python tools/gen_dust2.py
import json, io

E = []
COUNTERS = {}


def uname(base):
    COUNTERS[base] = COUNTERS.get(base, 0) + 1
    n = COUNTERS[base]
    return base if n == 1 else "%s %d" % (base, n)


def ent(name, mesh, mat, pos, scale=None, rot=None, physics=None, interaction=None):
    e = {"name": uname(name), "mesh": mesh, "material": mat,
         "transform": {"position": [round(v, 3) for v in pos]}}
    if scale:
        e["transform"]["scale"] = [round(v, 3) for v in scale]
    if rot:
        e["transform"]["rotation"] = [round(v, 3) for v in rot]
    if physics:
        e["physics"] = physics
    if interaction:
        e["interaction"] = interaction
    E.append(e)


def block(name, mat, cx, cy, cz, sx, sy, sz, rot=None, mass=0.0):
    ent(name, "cube", mat, [cx, cy, cz], scale=[sx, sy, sz], rot=rot,
        physics={"shape": {"cuboid": [round(sx / 2, 3), round(sy / 2, 3), round(sz / 2, 3)]},
                 "mass": mass})


def deco(name, mesh, mat, pos, scale, rot=None):
    ent(name, mesh, mat, pos, scale=scale, rot=rot)


def slab(name, mat, x0, x1, z0, z1, y_top, thickness=0.6):
    block(name, mat, (x0 + x1) / 2, y_top - thickness / 2, (z0 + z1) / 2,
          x1 - x0, thickness, z1 - z0)


def wall_x(name, mat, x0, x1, z, h, y0=0.0, t=0.7):
    block(name, mat, (x0 + x1) / 2, y0 + h / 2, z, x1 - x0, h, t)


def wall_z(name, mat, x, z0, z1, h, y0=0.0, t=0.7):
    block(name, mat, x, y0 + h / 2, (z0 + z1) / 2, t, h, z1 - z0)


def stairs(name, mat, x, z, width, steps, step_h, step_d, axis, sign, y0=0.0):
    # Each step is a full-height block from y0, matching the old scene's style.
    for i in range(steps):
        h = step_h * (i + 1)
        off = (i + 0.5) * step_d * sign
        if axis == "x":
            block(name, mat, x + off, y0 + h / 2, z, step_d, h, width)
        else:
            block(name, mat, x, y0 + h / 2, z + off, width, h, step_d)


def ramp(name, mat, cx, cy, cz, sx, sy, sz, pitch, axis="x"):
    rot = [pitch, 0, 0] if axis == "x" else [0, 0, pitch]
    block(name, mat, cx, cy, cz, sx, sy, sz, rot=rot)


def crate(name, x, y, z, s, mat="box", mass=0.0, rot_y=0.0):
    block(name, mat, x, y + s / 2, z, s, s, s,
          rot=[0, rot_y, 0] if rot_y else None, mass=mass)


def barrel(name, x, z, y=0.0, explosive=False, mass=9.0):
    e = {"shape": {"cylinder": [0.36, 0.62]}, "mass": mass}
    inter = {"type": "explosive", "radius": 6.5, "impulse": 26.0} if explosive else None
    ent(name, "cylinder", "barrel_ex" if explosive else "rust",
        [x, y + 0.62, z], scale=[0.72, 1.24, 0.72], physics=e, interaction=inter)


def lamp(name, x, y, z):
    deco(name, "cube", "lamp", [x, y, z], [0.32, 0.22, 0.32])


def sandbag_row(name, x, z, n, axis, y=0.0, rot=8.0):
    for i in range(n):
        px = x + (i * 0.95 if axis == "x" else 0)
        pz = z + (i * 0.95 if axis == "z" else 0)
        block(name, "sandbag", px, y + 0.26, pz, 0.95, 0.52, 0.5,
              rot=[0, rot * ((i % 3) - 1), 0])


H = 5.0        # standard wall height
HB = 8.0       # boundary wall height
T_Y = 1.6      # T spawn plateau top
LONG_Y = 0.8   # long A raised floor
SITE_Y = 1.8   # A site plateau top
CAT_Y = 2.4    # catwalk top
TUN_Y = 2.2    # upper tunnel floor
CT_Y = 0.4     # CT spawn top

# === Ground & boundaries ==================================================
block("Ground", "ground", -2, -0.5, 3, 86, 1, 84)
wall_x("North boundary", "sandstone", -45, 41, -38.6, HB)
wall_x("South boundary", "sandstone", -45, 41, 44.6, HB)
wall_z("West boundary", "sandstone", -44.6, -39, 45, HB)
wall_z("East boundary", "sandstone", 40.6, -39, 45, HB)

# === T spawn plateau (south) =============================================
slab("T spawn plateau", "sandstone", -14, 10, 30, 44, T_Y, 1.8)
# Plateau edge walls except the three exits.
wall_x("T plateau edge W", "sandstone", -14, -4, 30, 1.6, 0)      # low retaining
wall_x("T plateau edge E", "sandstone", 4, 6, 30, 1.6, 0)
wall_z("T plateau side", "sandstone", -14, 36, 44, H, 0)
# Suicide ramp: center, down to outside mid.
ramp("T mid ramp", "ground", 0, 0.78, 27, 8, 0.5, 6.8, -13.2)
# East stairs down toward outside long.
stairs("T long stairs", "stone", 8, 32, 4.0, 4, 0.4, 1.0, "z", -1, 0)
# West ramp into upper tunnels.
ramp("T tuns ramp", "stone", -16.5, 1.9, 33.5, 6.5, 0.5, 5, 5.5, "z")
# Spawn props: the classic ball pit + crates.
crate("T spawn crate", -11, T_Y, 40, 1.6)
crate("T spawn crate", -11, T_Y + 1.6, 40, 1.2)
crate("T spawn crate", -8.5, T_Y, 41, 1.4)
for i, (mx, mat, mass) in enumerate([(-2.0, "bouncy_rubber", 1.2), (-0.8, "ice", 2.0),
                                     (0.4, "heavy_metal", 14.0), (1.6, "bouncy_rubber", 0.7)]):
    ent("T ball", "sphere", mat, [mx, T_Y + 0.5, 40.5], scale=[0.7, 0.7, 0.7],
        physics={"shape": {"sphere": 0.35}, "mass": mass,
                 "surface": "rubber" if mat == "bouncy_rubber" else "default"})

# === Outside mid ==========================================================
wall_z("Mid west wall", "plaster", -8, 6, 26, H)
wall_z("Mid east wall", "plaster", 8, 8, 26, H)
# Mid doors frame at z=4.
wall_x("Mid doors crosswall L", "plaster_light", -8, -3.4, 4, H)
wall_x("Mid doors crosswall R", "plaster_light", 3.4, 8, 4, H)
block("Mid door left pillar", "wood", -3.1, 2.1, 4, 0.6, 4.2, 0.9)
block("Mid door right pillar", "wood", 3.1, 2.1, 4, 0.6, 4.2, 0.9)
block("Mid door lintel", "wood", 0, 4.05, 4, 6.8, 0.5, 0.9)
for side, sgn in (("left", -1), ("right", 1)):
    ent("Mid door %s" % side, "cube", "door",
        [1.5 * sgn, 1.65, 4], scale=[2.6, 3.3, 0.22], rot=[0, 10 * sgn, 0],
        physics={"shape": {"cuboid": [1.3, 1.65, 0.11]}, "mass": 0},
        interaction={"type": "door", "open_rotation": [0, 84 * sgn, 0],
                     "hinge_offset": [1.3 * sgn, 0, 0], "speed": 5.5})
deco("Mid doors awning", "cube", "awning", [0, 4.75, 5.2], [7.5, 0.16, 2.2], rot=[7, 0, 0])
# Xbox on top mid.
crate("Xbox", 0, 0, 0.5, 1.9)
# Mid corridor walls to CT.
wall_z("Top mid west wall", "plaster", -6, -14, -8, H)
wall_z("Top mid west wall", "plaster", -6, -4, 4, H)          # gap z -8..-4 = B hall
wall_z("Top mid east wall", "plaster", 4, -14, -4, H)
wall_z("Top mid east wall", "plaster", 4, 0, 4, H)            # gap z -4..0 = catwalk stairs
# CT mid stairs up to CT plateau.
stairs("CT mid stairs", "stone", 0, -15.5, 9.0, 2, 0.2, 1.0, "z", -1, 0)

# === Catwalk (A short) ====================================================
stairs("Catwalk stairs", "wood_light", 4.6, -2, 3.6, 5, 0.48, 1.05, "x", 1, 0)
slab("Catwalk", "wood_light", 10, 20, -4.2, -0.2, CAT_Y, 0.5)
wall_z("Catwalk rail", "wood", 10.2, -4.2, -0.2, 0.9, CAT_Y, 0.18)
for i in range(5):
    deco("Catwalk rail post", "cube", "wood", [10.2, CAT_Y + 0.45, -4.0 + i * 0.95],
         [0.14, 0.9, 0.14])
wall_x("Catwalk south wall", "plaster", 8, 20, 0.6, H)
crate("Catwalk crate", 17.5, CAT_Y, -3.5, 1.5)
# Short boost step onto site.
block("Short boost", "stone", 20.8, CAT_Y / 2 - 0.2, -2.2, 1.6, CAT_Y - 0.4, 3.2)

# === A site plateau =======================================================
slab("A site plateau", "sandstone", 14, 32, -28, -8, SITE_Y, 1.8)
block("A back building", "sandstone", 23, 3.5, -33, 18, 7, 10)   # solid backdrop
deco("A roof", "wedge", "brick", [23, 8.2, -33], [18, 2.4, 10], rot=[0, 0, 0])
wall_z("A site west wall", "plaster", 14, -28, -24, H, SITE_Y - 1.8)
wall_x("A site south wall", "plaster_light", 22, 30, -8, H - 1.8, SITE_Y)
# CT-to-A stairs on the west edge.
stairs("A CT stairs", "stone", 13.4, -20, 5.0, 4, 0.35, 1.0, "x", -1, 0.4)
# Bombsite dressing.
deco("A mark", "cube", "paint_white", [23, SITE_Y + 0.04, -18], [4.2, 0.06, 4.2])
deco("A letter", "cube", "paint_a", [23, SITE_Y + 0.08, -18], [1.6, 0.05, 2.6])
crate("Goose crate", 28.5, SITE_Y, -25, 2.0)
crate("Goose crate", 28.5, SITE_Y + 2.0, -25, 1.6)
crate("Goose crate", 26.2, SITE_Y, -25.5, 1.7, rot_y=18)
crate("A site crate", 17, SITE_Y, -13, 1.8)
crate("A site crate", 17, SITE_Y, -11.1, 1.4, rot_y=-12)
barrel("A explosive barrel", 20.5, -23, SITE_Y, explosive=True)
barrel("A junk barrel", 30.5, -11, SITE_Y)
sandbag_row("A sandbags", 15.2, -16, 3, "z", SITE_Y)

# === Long A ===============================================================
# Outside long: open ground east of T plateau.
wall_z("Outside long west wall", "sandstone", 12, 20, 32, H)
wall_x("Outside long south wall", "sandstone", 12, 40, 33, H)
# Blue container corner.
block("Long container", "paint_b", 15, 1.5, 22.5, 4.5, 3.0, 2.6)
deco("Long container lid", "cube", "metal", [15, 3.1, 22.5], [4.7, 0.2, 2.8])
# Long doors: frame + double doors at z=18.
wall_x("Long doors crosswall L", "plaster_light", 22, 27, 18, H)
wall_x("Long doors crosswall R", "plaster_light", 33, 40, 18, H)
block("Long door left pillar", "wood", 27.3, 2.1, 18, 0.6, 4.2, 0.9)
block("Long door right pillar", "wood", 32.7, 2.1, 18, 0.6, 4.2, 0.9)
block("Long door lintel", "wood", 30, 4.05, 18, 6.0, 0.5, 0.9)
for side, sgn in (("left", -1), ("right", 1)):
    ent("Long door %s" % side, "cube", "door",
        [30 + 1.35 * sgn, 1.65, 18], scale=[2.3, 3.3, 0.2], rot=[0, 14 * sgn, 0],
        physics={"shape": {"cuboid": [1.15, 1.65, 0.1]}, "mass": 0},
        interaction={"type": "door", "open_rotation": [0, 86 * sgn, 0],
                     "hinge_offset": [1.15 * sgn, 0, 0], "speed": 4.5})
deco("Long doors awning", "cube", "awning", [30, 4.7, 19.4], [7.0, 0.16, 2.4], rot=[8, 0, 0])
# Ramp up through the doors onto the raised long floor.
ramp("Long entry ramp", "ground", 30, 0.35, 15.8, 11, 0.5, 4.4, -10.5)
# Long corridor raised floor.
slab("Long floor", "ground", 22, 38, -16, 14, LONG_Y, 1.2)
wall_z("Long west wall", "sandstone", 22, -8, 18, H)      # separates from mid building
block("Mid building", "plaster", 14, 3, 9, 16, 6, 16)     # solid central building
deco("Mid building roof", "wedge", "brick", [14, 7.2, 9], [16, 2.2, 16], rot=[0, 90, 0])
# Pit: ground-level nook below the long ledge at the NE corner.
wall_x("Pit ledge", "stone", 33, 39, -16, 1.1, 0)
sandbag_row("Pit sandbags", 34.2, -17.2, 4, "x")
barrel("Pit junk barrel", 38.5, -18.5)
# A ramp: climbs west from long floor to the site.
ramp("A ramp", "stone", 29, 1.35, -11.5, 8, 0.7, 8, 0, "z")
E[-1]["transform"]["rotation"] = [0, 0, 7.2]
crate("Long crate", 36.5, LONG_Y, 10.5, 1.8)
crate("Long crate", 36.5, LONG_Y, 8.6, 1.5, rot_y=22)
barrel("Long explosive barrel", 24.5, 2, LONG_Y, explosive=True)
lamp("Long doors lamp", 30, 4.4, 17.4)

# === CT spawn =============================================================
slab("CT spawn plateau", "concrete", -6, 10, -36, -26, CT_Y, 0.8)
wall_x("CT north wall", "concrete", -6, 10, -36.5, H)
crate("CT crate", 8, CT_Y, -34, 1.6)
crate("CT crate", 8, CT_Y + 1.6, -34, 1.2)
barrel("CT junk barrel", -4.5, -34.5, CT_Y)
sandbag_row("CT sandbags", 5, -27.2, 3, "x", CT_Y)
lamp("CT lamp", 2, 3.6, -35.8)

# === CT -> B hall and B doors ============================================
wall_x("CT B hall south wall", "concrete", -20, -6, -26, H)
wall_x("CT B hall north wall", "concrete", -20, -6, -32, H)
wall_z("B corridor east wall", "concrete", -14, -26, -12, H)
wall_z("B corridor west wall", "concrete", -20, -26, -10, H)
# B doors: double doors from mid hall into B site (x = -8 plane).
wall_x("B hall north wall", "plaster", -18, -8, -8, H)
wall_x("B hall south wall", "plaster", -18, -8, -4, H)
block("B door pillar N", "wood", -18, 2.1, -7.6, 0.9, 4.2, 0.6)
block("B door pillar S", "wood", -18, 2.1, -4.4, 0.9, 4.2, 0.6)
ent("B hall door", "cube", "door", [-18, 1.65, -6], scale=[0.22, 3.3, 2.7],
    rot=[0, -8, 0],
    physics={"shape": {"cuboid": [0.11, 1.65, 1.35]}, "mass": 0},
    interaction={"type": "door", "open_rotation": [0, -85, 0],
                 "hinge_offset": [0, 0, -1.35], "speed": 5.0})
lamp("B hall lamp", -13, 3.6, -6)

# === B site ===============================================================
wall_x("B site north wall", "brick", -42, -20, -24, H)
wall_z("B site east wall", "brick", -20, -24, -12, H)
wall_x("B site south wall", "brick", -42, -26, -4, H)
# Back plat along the west wall.
slab("B back plat", "concrete", -42, -37, -24, -10, 1.2, 1.2)
stairs("B plat stairs", "concrete", -36.4, -12, 4.0, 3, 0.4, 0.9, "x", -1, 0)
# The car: body + cabin + wheels.
block("B car body", "paint_b", -32, 0.75, -8, 4.6, 1.1, 2.2)
block("B car cabin", "metal", -31.2, 1.7, -8, 2.2, 0.8, 2.0)
for i, (dx, dz) in enumerate([(-1.6, -1.0), (1.6, -1.0), (-1.6, 1.0), (1.6, 1.0)]):
    deco("B car wheel", "cylinder", "rubber_dark", [-32 + dx, 0.4, -8 + dz],
         [0.8, 0.35, 0.8], rot=[90, 0, 0])
# Bombsite dressing.
deco("B mark", "cube", "paint_white", [-31, 0.04, -16], [4.2, 0.06, 4.2])
deco("B letter", "cube", "paint_b", [-31, 0.08, -16], [1.6, 0.05, 2.6])
crate("B site crate", -25, 0, -20, 1.9)
crate("B site crate", -25, 1.9, -20, 1.5)
crate("B site crate", -22.8, 0, -19.4, 1.6, rot_y=15)
crate("B window crate", -40.5, 1.2, -18, 1.7)
barrel("B explosive barrel", -27, -6.5, explosive=True)
barrel("B junk barrel", -39, -21, 1.2)
deco("B broken roof", "wedge", "concrete", [-30, 6.2, -20], [9, 2.4, 8], rot=[0, 180, 0])
lamp("B site lamp", -31, 4.2, -23.4)
sandbag_row("B sandbags", -24.5, -13, 3, "z")

# === Upper tunnels ========================================================
slab("Tuns south floor", "stone", -34, -14, 26, 32, TUN_Y, 0.9)
slab("Tuns west floor", "stone", -34, -28, -2, 26, TUN_Y, 0.9)
wall_x("Tuns south outer wall", "brick", -34, -14, 32.5, H, 0)
wall_x("Tuns south inner wall", "brick", -34, -20, 25.5, H - 1, TUN_Y)
wall_z("Tuns west outer wall", "brick", -34.5, -2, 32, H, 0)
wall_z("Tuns east inner wall", "brick", -27.5, 2, 25.5, H - 1, TUN_Y)
# Ceilings make it read as a tunnel.
slab("Tuns south ceiling", "sandstone", -34, -14, 26, 32, TUN_Y + 3.6, 0.5)
slab("Tuns west ceiling", "sandstone", -34, -27, 2, 26, TUN_Y + 3.6, 0.5)
# Exit ramp down into B site.
ramp("Tuns B ramp", "stone", -27, 1.2, -3.2, 6, 0.5, 5.6, 21, "x")
E[-1]["transform"]["rotation"] = [21, 0, 0]
lamp("Tuns lamp", -31, TUN_Y + 2.6, 28)
lamp("Tuns lamp", -31, TUN_Y + 2.6, 10)
crate("Tuns crate", -32.5, TUN_Y, 29.5, 1.5)
barrel("Tuns junk barrel", -30, 24, TUN_Y)

# === Lower tunnels ========================================================
wall_x("Lower tuns north wall", "brick", -26, -8, 10, H)
wall_x("Lower tuns south wall", "brick", -26, -8, 16, H)
slab("Lower tuns ceiling", "stone", -26, -8, 10, 16, 3.4, 0.5)
stairs("Lower tuns stairs", "stone", -30, 12.8, 5.2, 5, 0.44, 0.95, "x", -1, 0)
wall_z("Lower stairs north wall", "brick", -34.5, 2, 10, H)
lamp("Lower tuns lamp", -17, 2.9, 13)
barrel("Lower tuns barrel", -12, 14.5, 0, explosive=True)

# === Architectural detail pass ============================================
# Crenellated parapets along the boundary tops (sawtooth silhouette).
for i in range(14):
    x = -42.0 + i * 6.2
    deco("Parapet tooth N", "cube", "sandstone", [x, HB + 0.45, -38.6], [1.6, 0.9, 0.8])
    deco("Parapet tooth S", "cube", "sandstone", [x, HB + 0.45, 44.6], [1.6, 0.9, 0.8])
for i in range(13):
    z = -36.0 + i * 6.4
    deco("Parapet tooth W", "cube", "sandstone", [-44.6, HB + 0.45, z], [0.8, 0.9, 1.6])
    deco("Parapet tooth E", "cube", "sandstone", [40.6, HB + 0.45, z], [0.8, 0.9, 1.6])

# Arch approximations over the two door frames: corner wedges under the
# lintels turn the rectangular opening into a chamfered arch silhouette.
for x0, z, w in [(0.0, 4.0, 3.05), (30.0, 18.0, 2.7)]:
    deco("Arch corner L", "wedge", "wood", [x0 - w + 0.35, 3.55, z], [0.7, 0.5, 0.86],
         rot=[0, 0, 180])
    deco("Arch corner R", "wedge", "wood", [x0 + w - 0.35, 3.55, z], [0.7, 0.5, 0.86],
         rot=[180, 0, 0])

# Recessed window boxes (dark inset + sill + lintel) on large blank walls.
WINDOWS = [
    (-7.6, 3.2, 18.0, 90), (-7.6, 3.2, 10.0, 90),      # mid west wall
    (8.4, 3.4, 14.0, 90),                                # mid east wall
    (22.4, 3.0, -2.0, 90),                               # long west wall
    (14.35, 3.4, -20.0, 90),                             # A site west wall
    (-20.35, 3.0, -18.0, 90), (-34.0, 3.2, -14.0, 90),  # B site walls
]
for i, (x, y, z, ry) in enumerate(WINDOWS):
    deco("Window inset", "cube", "rubber_dark", [x, y, z], [0.12, 1.5, 1.1] if ry else [1.1, 1.5, 0.12])
    deco("Window sill", "cube", "stone", [x, y - 0.85, z], [0.3, 0.14, 1.3] if ry else [1.3, 0.14, 0.3])
    deco("Window lintel", "cube", "stone", [x, y + 0.85, z], [0.24, 0.14, 1.3] if ry else [1.3, 0.14, 0.24])
    # Alternate windows get a wooden shutter leaning open.
    if i % 2 == 0:
        deco("Window shutter", "cube", "wood", [x + 0.15, y, z + 0.75], [0.06, 1.4, 0.55],
             rot=[0, 24, 0])

# Overhead cables sagging across mid and B site (three-segment approximation).
def cable(name, x0, y0, z0, x1, y1, z1, sag):
    mx, mz = (x0 + x1) / 2, (z0 + z1) / 2
    my = (y0 + y1) / 2 - sag
    for (ax, ay, az, bx, by, bz) in [(x0, y0, z0, mx, my, mz), (mx, my, mz, x1, y1, z1)]:
        import math as _m
        dx, dy, dz = bx - ax, by - ay, bz - az
        length = _m.sqrt(dx * dx + dy * dy + dz * dz)
        yaw = _m.degrees(_m.atan2(dx, dz))
        pitch = -_m.degrees(_m.asin(dy / length)) if length > 0 else 0
        deco(name, "cube", "rubber_dark",
             [(ax + bx) / 2, (ay + by) / 2, (az + bz) / 2],
             [0.045, 0.045, round(length, 2)], rot=[round(pitch, 1), round(yaw, 1), 0])

cable("Mid cable", -7.6, 4.6, 14.0, 8.4, 4.8, 12.0, 0.55)
cable("Mid cable", -7.6, 4.4, 20.0, 8.4, 4.6, 21.5, 0.6)
cable("B cable", -20.4, 4.4, -14.0, -34.0, 4.6, -12.0, 0.5)

# Awning row over the mid west windows (shop-front feel).
for z in [10.0, 18.0]:
    deco("Mid awning", "cube", "awning", [-7.0, 4.1, z], [1.4, 0.1, 2.3], rot=[0, 0, -16])

# Drainpipes down building corners.
for (x, z) in [(-7.7, 25.6), (8.3, 25.6), (22.3, -7.7), (-20.3, -23.6)]:
    deco("Drainpipe", "cylinder", "rust", [x, 2.5, z], [0.14, 5.0, 0.14])

# Roof clutter on the mid building: vents and a water tank.
deco("Roof vent", "cube", "metal", [10.5, 6.6, 6.0], [0.9, 1.1, 0.9])
deco("Roof vent 2", "cube", "metal", [17.0, 6.5, 12.5], [0.7, 0.9, 0.7])
deco("Roof tank", "cylinder", "rust", [13.0, 7.0, 13.5], [1.6, 1.6, 1.6])
deco("Roof tank cap", "cylinder", "metal", [13.0, 7.9, 13.5], [1.7, 0.15, 1.7])

# Cracked wall patches: thin dark plates at plinth height break up plaster.
for (x, z, w, ry) in [(-7.55, 8.0, 2.6, 90), (8.35, 18.0, 3.2, 90),
                      (22.35, 6.0, 2.2, 90), (-20.3, -20.5, 2.4, 90)]:
    deco("Wall crack", "cube", "concrete", [x, 1.1, z],
         [0.05, 1.4, w] if ry else [w, 1.4, 0.05], rot=[0, 0, 6])

# Market stall near T mid exit: table + awning + crates underneath.
deco("Stall table", "cube", "wood_light", [-5.5, 0.95, 23.0], [2.6, 0.12, 1.2])
for dx in (-1.1, 1.1):
    deco("Stall leg", "cube", "wood", [-5.5 + dx, 0.45, 23.0], [0.12, 0.9, 0.12])
deco("Stall awning", "cube", "awning", [-5.5, 2.5, 23.2], [3.0, 0.08, 1.8], rot=[12, 0, 0])
crate("Stall crate", -5.9, 0, 22.6, 0.8, mat="box", mass=3.0, rot_y=14)
crate("Stall crate", -4.9, 0, 23.3, 0.7, mat="box", mass=2.5, rot_y=-20)

# === Detail pass: trims, graffiti, rubble =================================
for i, (x, z, ln, ax) in enumerate([
        (-2, 26.2, 12, "x"), (-2, 4.4, 10, "x"), (22.4, 0, 22, "z"),
        (-6.4, -6, 14, "z"), (-20.4, -16, 14, "z"), (14.4, -18, 16, "z"),
        (30, 18.4, 9, "x"), (-31, -24.4, 16, "x")]):
    if ax == "x":
        deco("Base trim", "cube", "stone", [x, 0.18, z], [ln, 0.36, 0.28])
    else:
        deco("Base trim", "cube", "stone", [x, 0.18, z], [0.28, 0.36, ln])
graffiti = [("graffiti_cyan", -7.5, 2.2, 14, 0), ("graffiti_pink", 7.5, 2.4, 20, 0),
            ("graffiti_cyan", 21.9, 1.9, 6, 90), ("graffiti_pink", -19.9, 2.1, -18, 90),
            ("graffiti_cyan", -33.9, TUN_Y + 1.6, 12, 90)]
for mat, x, y, z, ry in graffiti:
    deco("Graffiti", "cube", mat, [x, y, z], [0.06, 1.1, 1.9] if ry else [1.9, 1.1, 0.06])
for i, (x, z, s, ry) in enumerate([(-6.5, -9.5, 1.1, 34), (5.5, -12, 0.9, -18),
                                   (25, -15.5, 1.2, 52), (-23, -2.5, 1.0, -40),
                                   (12.5, 30.5, 1.3, 12)]):
    ent("Rubble wedge", "wedge", "concrete", [x, 0.45, z], scale=[s * 2, s, s * 2.4],
        rot=[0, ry, 0],
        physics={"shape": {"wedge": [s, s / 2, s * 1.2]}, "mass": 0})
# Dynamic junk boxes for the sandbox feel.
for i, (x, z, y) in enumerate([(2.5, 12, 0), (-3, 18, 0), (28, 6, LONG_Y),
                               (-28, -14, 0), (18, -20, SITE_Y), (6, -30, CT_Y)]):
    crate("Loose crate", x, y, z, 0.9, mass=4.0, rot_y=(i * 31) % 60 - 30)
# Wall lamps at spawns.
lamp("T spawn lamp", 0, 3.4, 43.8)
lamp("Mid lamp", 0, 4.2, 4.9)
# Welcome sign near T spawn.
deco("T sign", "cube", "sign_glow", [-6, T_Y + 2.4, 43.6], [4.2, 1.0, 0.24])

SCENE = {
    "player": {"position": [0.0, T_Y + 1.4, 38.0], "height": 1.6,
               "move_speed": 8.0, "mouse_sensitivity": 0.002},
    "meshes": [
        {"name": "cube", "source": "cube"},
        {"name": "sphere", "source": {"sphere": {"segments": 32, "rings": 20}}},
        {"name": "cylinder", "source": {"cylinder": {"segments": 32}}},
        {"name": "wedge", "source": "wedge"},
        {"name": "humanoid", "source": "humanoid"},
        {"name": "rifle", "source": "rifle"},
    ],
    "materials": [
        {"name": "ground", "color": [0.52, 0.43, 0.28], "roughness": 0.92},
        {"name": "sandstone", "color": [0.68, 0.54, 0.34], "roughness": 0.88},
        {"name": "plaster", "color": [0.76, 0.69, 0.56], "roughness": 0.78},
        {"name": "plaster_light", "color": [0.88, 0.81, 0.67], "roughness": 0.72},
        {"name": "stone", "color": [0.38, 0.34, 0.29], "roughness": 0.96},
        {"name": "wood", "color": [0.38, 0.22, 0.10], "roughness": 0.62},
        {"name": "wood_light", "color": [0.61, 0.39, 0.17], "roughness": 0.58},
        {"name": "door", "color": [0.20, 0.12, 0.065], "roughness": 0.48, "metallic": 0.05},
        {"name": "metal", "color": [0.20, 0.24, 0.29], "roughness": 0.26, "metallic": 0.86},
        {"name": "paint_a", "color": [0.72, 0.16, 0.07], "roughness": 0.52},
        {"name": "paint_b", "color": [0.08, 0.30, 0.55], "roughness": 0.48},
        {"name": "paint_white", "color": [0.82, 0.82, 0.74], "roughness": 0.68},
        {"name": "lamp", "color": [0.95, 0.70, 0.30], "roughness": 0.22, "metallic": 0.15,
         "emissive": [3.8, 2.1, 0.55]},
        {"name": "enemy", "color": [0.78, 0.06, 0.04], "roughness": 0.38, "metallic": 0.18},
        {"name": "team_blue", "color": [0.10, 0.30, 0.78], "roughness": 0.38, "metallic": 0.18},
        {"name": "box", "color": [0.56, 0.32, 0.12], "roughness": 0.58},
        {"name": "bouncy_rubber", "color": [0.08, 0.62, 0.22], "roughness": 0.28},
        {"name": "ice", "color": [0.30, 0.72, 0.92], "roughness": 0.08},
        {"name": "heavy_metal", "color": [0.12, 0.16, 0.21], "roughness": 0.20, "metallic": 0.92},
        {"name": "brick", "color": [0.44, 0.20, 0.11], "roughness": 0.94},
        {"name": "concrete", "color": [0.29, 0.30, 0.29], "roughness": 0.98},
        {"name": "rust", "color": [0.35, 0.105, 0.035], "roughness": 0.82, "metallic": 0.55},
        {"name": "barrel_ex", "color": [0.72, 0.14, 0.1], "roughness": 0.45, "metallic": 0.55},
        {"name": "sandbag", "color": [0.55, 0.5, 0.38], "roughness": 0.96},
        {"name": "rubber_dark", "color": [0.025, 0.03, 0.035], "roughness": 0.83},
        {"name": "awning", "color": [0.12, 0.34, 0.39], "roughness": 0.76},
        {"name": "graffiti_cyan", "color": [0.02, 0.72, 0.92], "roughness": 0.28,
         "emissive": [0.3, 2.8, 4.2]},
        {"name": "graffiti_pink", "color": [0.95, 0.03, 0.38], "roughness": 0.30,
         "emissive": [4.0, 0.12, 1.2]},
        {"name": "sign_glow", "color": [0.08, 0.30, 0.34], "roughness": 0.24, "metallic": 0.18,
         "emissive": [0.2, 2.4, 2.8]},
    ],
    "lights": [
        {"type": "directional", "direction": [0.42, -1.0, 0.26],
         "color": [1.0, 0.88, 0.68], "intensity": 3.8},
        {"type": "point", "position": [0.0, 6.0, 5.0], "color": [1.0, 0.74, 0.45],
         "intensity": 40.0, "range": 18.0},
        {"type": "point", "position": [30.0, 5.5, 17.0], "color": [1.0, 0.78, 0.48],
         "intensity": 40.0, "range": 18.0},
        {"type": "point", "position": [23.0, 6.0, -18.0], "color": [1.0, 0.72, 0.4],
         "intensity": 52.0, "range": 24.0},
        {"type": "point", "position": [-31.0, 5.0, -16.0], "color": [0.72, 0.82, 1.0],
         "intensity": 46.0, "range": 22.0},
        {"type": "point", "position": [-30.0, TUN_Y + 3.0, 18.0], "color": [1.0, 0.58, 0.26],
         "intensity": 30.0, "range": 15.0},
        {"type": "point", "position": [2.0, 4.0, -31.0], "color": [0.78, 0.88, 1.0],
         "intensity": 28.0, "range": 14.0},
        {"type": "point", "position": [0.0, 5.0, 38.0], "color": [1.0, 0.7, 0.4],
         "intensity": 30.0, "range": 16.0},
        {"type": "point", "position": [36.0, 3.0, -17.0], "color": [1.0, 0.42, 0.16],
         "intensity": 20.0, "range": 10.0},
    ],
    "entities": E,
    "match_mode": {
        "mesh": "humanoid",
        "team_a_material": "team_blue",
        "team_b_material": "enemy",
        "team_a_spawns": [
            [0.0, T_Y + 1.3, 38.0],
            [-4.0, T_Y + 1.3, 39.5],
            [4.0, T_Y + 1.3, 39.5],
            [-8.0, T_Y + 1.3, 41.0],
            [8.0, T_Y + 1.3, 41.0],
        ],
        "team_b_spawns": [
            [0.0, CT_Y + 1.3, -31.0],
            [-4.0, CT_Y + 1.3, -33.0],
            [4.0, CT_Y + 1.3, -33.0],
            [23.0, SITE_Y + 1.3, -20.0],
            [-31.0, 1.3, -16.0],
        ],
        "waypoints": [
            [0.0, 1.3, 8.0], [0.0, 1.3, -8.0], [1.0, 1.3, -20.0],
            [8.0, 1.3, -2.0], [15.0, CAT_Y + 1.3, -2.0],
            [23.0, SITE_Y + 1.3, -18.0], [29.0, LONG_Y + 1.3, -12.0],
            [30.0, LONG_Y + 1.3, 8.0], [30.0, 1.3, 22.0],
            [2.0, CT_Y + 1.3, -30.0], [-13.0, 1.3, -29.0], [-13.0, 1.3, -6.0],
            [-31.0, 1.3, -14.0], [-24.0, 1.3, -8.0], [-30.0, TUN_Y + 1.3, 20.0],
            [-17.0, 1.3, 13.0],
        ],
        "rounds_to_win": 5,
        "bomb_site": [23.0, SITE_Y + 0.2, -18.0],
        "bomb_site_radius": 6.5,
    },
}

io.open("assets/scenes/dust2.json", "w", encoding="utf-8", newline="\n").write(
    json.dumps(SCENE, indent=1, ensure_ascii=False))
print("entities:", len(E))
